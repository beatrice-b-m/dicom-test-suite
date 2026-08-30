//! Plan-only provider for Secondary Capture instances whose encoding requires
//! a feature-gated codec or a locked external tool.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{ResolvedInstancePlan, TemplateDescriptor};
use crate::executor::services::{ByteBinding, CodecRequest, NativeFrameBinding};
use crate::native_pixel::NativePixelContent;
use crate::sha256_hex;

use super::codec_registry::{
    BackendBoundary, CodecEvidenceRequirement, TransferSyntaxBackendRegistry,
    encoding_provider_matches,
};
use super::sc::{
    ScPlanError, SecondaryCapturePlanInput, native_pixel_content_from_recipe,
    resolved_secondary_capture_base_plan,
};
use super::{CaseRecipe, PlannedArtifactRecipe};

pub const EXCEPTIONAL_SC_PLAN_PROVIDER_ID: &str = "native.exceptional_sc_plan";
pub const EXCEPTIONAL_SC_PIXEL_SLOT: &str = "pixel_data";
const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";

pub struct ExceptionalScPlanInput<'a> {
    pub recipe: &'a CaseRecipe,
    pub artifact: &'a PlannedArtifactRecipe,
    pub template: &'a TemplateDescriptor,
    pub instance_id: &'a str,
    pub standards_lock_sha256: &'a str,
    pub seed: u64,
}

#[derive(Debug, Clone)]
pub struct ExceptionalScPlanOutput {
    pub instance: ResolvedInstancePlan,
    pub native_pixels: NativePixelContent,
    pub encoding: ExceptionalScEncodingRequest,
    pub evidence_requirements: Vec<CodecEvidenceRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionalScEncodingRequest {
    Dataset(DatasetEncodingRequest),
    EncodedFrames(CodecRequest),
    LockedFullFile(LockedFullFileCodecRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetEncodingRequest {
    pub backend_id: String,
    pub target_transfer_syntax_uid: String,
}

/// Explicitly models the only boundary which must receive a complete Part 10
/// input. The provider resolves the source plan before staging; execution is a
/// separate, locked adapter and cannot be mistaken for encoded-frame work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedFullFileCodecRequest {
    pub request_id: String,
    pub artifact_id: String,
    pub backend_id: String,
    pub source_transfer_syntax_uid: String,
    pub target_transfer_syntax_uid: String,
    pub source_plan: ResolvedInstancePlan,
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExceptionalCodecParameters {
    JpegXlLossy {
        distance: f64,
        effort: u8,
        num_threads: u16,
        max_absolute_error: u64,
        rmse_limit: f64,
    },
    Htj2kLossy {
        qstep: f64,
        progression: String,
        max_absolute_error: u64,
        rmse_limit: f64,
    },
    DcmtkJpegLossless {
        process: String,
    },
}

pub fn plan_exceptional_sc(
    input: ExceptionalScPlanInput<'_>,
) -> Result<ExceptionalScPlanOutput, ExceptionalScPlanError> {
    if input.recipe.plan_provider_id != EXCEPTIONAL_SC_PLAN_PROVIDER_ID {
        return Err(ExceptionalScPlanError::WrongProvider(
            input.recipe.plan_provider_id.clone(),
        ));
    }
    let backend_id = input
        .artifact
        .encoding
        .non_template_encoding_provider_id
        .as_deref()
        .ok_or(ExceptionalScPlanError::MissingEncodingBackend)?;
    let registry = TransferSyntaxBackendRegistry::load_committed()
        .map_err(|error| ExceptionalScPlanError::Registry(error.to_string()))?;
    let backend = registry
        .for_transfer_syntax(&input.artifact.encoding.transfer_syntax_uid)
        .ok_or_else(|| {
            ExceptionalScPlanError::Registry(format!(
                "transfer syntax {} has no backend",
                input.artifact.encoding.transfer_syntax_uid
            ))
        })?;
    if !encoding_provider_matches(backend, backend_id) {
        return Err(ExceptionalScPlanError::BackendMismatch {
            expected: backend.backend_id.into(),
            actual: backend_id.into(),
        });
    }
    let parameters = parse_parameters(input.artifact, backend.backend_id)?;

    let structural = SecondaryCapturePlanInput {
        recipe: input.recipe,
        artifact: input.artifact,
        template: input.template,
        instance_id: input.instance_id,
        standards_lock_sha256: input.standards_lock_sha256,
        seed: input.seed,
    };
    let instance = resolved_secondary_capture_base_plan(structural)?;
    let sc = input
        .artifact
        .secondary_capture
        .as_ref()
        .ok_or(ExceptionalScPlanError::MissingSecondaryCapture)?;
    let native_pixels = native_pixel_content_from_recipe(sc)?;
    let request_id = format!("{}.codec", input.instance_id);
    let artifact_id = input.instance_id.to_owned();

    let encoding = match backend.boundary {
        BackendBoundary::DatasetWriter => {
            if !matches!(
                input.artifact.encoding.fragmentation_policy.as_str(),
                "native"
            ) {
                return Err(ExceptionalScPlanError::Policy(
                    "dataset encodings require native fragmentation".into(),
                ));
            }
            ExceptionalScEncodingRequest::Dataset(DatasetEncodingRequest {
                backend_id: backend.backend_id.into(),
                target_transfer_syntax_uid: backend.transfer_syntax_uid.into(),
            })
        }
        BackendBoundary::EncodedFrames => {
            if input.artifact.encoding.fragmentation_policy != "one_per_frame"
                || input.artifact.encoding.offset_table_policy != "populated_basic"
            {
                return Err(ExceptionalScPlanError::Policy(
                    "encoded-frame SC requires one fragment per frame and a populated basic offset table"
                        .into(),
                ));
            }
            let frames = native_pixels
                .frames
                .iter()
                .map(|frame| NativeFrameBinding {
                    frame_number: frame.frame_number,
                    bytes: ByteBinding::Inline {
                        bytes: frame.decoded_bytes.clone(),
                        sha256: frame.decoded_sha256.clone(),
                    },
                    rows: sc.rows,
                    columns: sc.columns,
                    samples_per_pixel: sc.samples_per_pixel,
                    bits_allocated: sc.bits_allocated,
                    photometric_interpretation: sc.photometric_interpretation.clone(),
                })
                .collect();
            ExceptionalScEncodingRequest::EncodedFrames(CodecRequest {
                request_id,
                artifact_id,
                slot: EXCEPTIONAL_SC_PIXEL_SLOT.into(),
                backend_id: backend.backend_id.into(),
                source_transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.into(),
                target_transfer_syntax_uid: backend.transfer_syntax_uid.into(),
                frames,
                parameters: parameters_to_map(parameters.as_ref())?,
            })
        }
        BackendBoundary::LockedFullFileTransform => {
            if input.artifact.encoding.fragmentation_policy != "one_per_frame"
                || input.artifact.encoding.offset_table_policy != "populated_basic"
            {
                return Err(ExceptionalScPlanError::Policy(
                    "locked full-file SC requires one fragment per frame and a populated basic offset table"
                        .into(),
                ));
            }
            let mut source_plan = instance.clone();
            source_plan.transfer_syntax_uid = EXPLICIT_VR_LITTLE_ENDIAN.into();
            ExceptionalScEncodingRequest::LockedFullFile(LockedFullFileCodecRequest {
                request_id,
                artifact_id,
                backend_id: backend.backend_id.into(),
                source_transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.into(),
                target_transfer_syntax_uid: backend.transfer_syntax_uid.into(),
                source_plan,
                parameters: parameters_to_map(parameters.as_ref())?,
            })
        }
    };

    // The provider owns no output root and does not consume any generated
    // file. This digest also proves the typed content exactly matches the
    // declared frame contract before an encoder runs.
    if sha256_hex(&native_pixels.unpadded_bytes) != native_pixels.unpadded_sha256 {
        return Err(ExceptionalScPlanError::ContentDigestMismatch);
    }

    Ok(ExceptionalScPlanOutput {
        instance,
        native_pixels,
        encoding,
        evidence_requirements: backend.evidence.to_vec(),
    })
}

fn parse_parameters(
    artifact: &PlannedArtifactRecipe,
    backend_id: &str,
) -> Result<Option<ExceptionalCodecParameters>, ExceptionalScPlanError> {
    let parameters = if artifact.parameters.is_empty() {
        None
    } else {
        Some(
            serde_json::from_value(Value::Object(artifact.parameters.clone()))
                .map_err(|error| ExceptionalScPlanError::Parameters(error.to_string()))?,
        )
    };
    let valid = match (backend_id, &parameters) {
        (
            "cjxl_jpegxl_lossy_command_writer",
            Some(ExceptionalCodecParameters::JpegXlLossy {
                distance,
                effort: 7,
                num_threads: 0,
                max_absolute_error: 8,
                rmse_limit,
            }),
        ) => *distance == 0.05 && *rmse_limit == 2.0,
        (
            "openjph_htj2k_lossy_command_writer",
            Some(ExceptionalCodecParameters::Htj2kLossy {
                qstep,
                progression,
                max_absolute_error: 64,
                rmse_limit,
            }),
        ) => *qstep == 0.00025 && progression == "LRCP" && *rmse_limit == 16.0,
        (
            "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
            Some(ExceptionalCodecParameters::DcmtkJpegLossless { process }),
        ) => process == "process_14",
        (
            "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
            Some(ExceptionalCodecParameters::DcmtkJpegLossless { process }),
        ) => process == "sv1",
        (
            "cjxl_jpegxl_lossy_command_writer"
            | "openjph_htj2k_lossy_command_writer"
            | "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer"
            | "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
            _,
        ) => false,
        (_, None) => true,
        _ => false,
    };
    if !valid {
        return Err(ExceptionalScPlanError::Parameters(format!(
            "parameters do not match backend {backend_id}"
        )));
    }
    Ok(parameters)
}

fn parameters_to_map(
    parameters: Option<&ExceptionalCodecParameters>,
) -> Result<BTreeMap<String, Value>, ExceptionalScPlanError> {
    match parameters {
        None => Ok(BTreeMap::new()),
        Some(value) => serde_json::to_value(value)
            .map_err(|error| ExceptionalScPlanError::Parameters(error.to_string()))?
            .as_object()
            .cloned()
            .map(|values| values.into_iter().collect())
            .ok_or_else(|| {
                ExceptionalScPlanError::Parameters("parameters are not an object".into())
            }),
    }
}

#[derive(Debug)]
pub enum ExceptionalScPlanError {
    WrongProvider(String),
    MissingEncodingBackend,
    MissingSecondaryCapture,
    BackendMismatch { expected: String, actual: String },
    Registry(String),
    Policy(String),
    Parameters(String),
    ContentDigestMismatch,
    Sc(ScPlanError),
}

impl fmt::Display for ExceptionalScPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProvider(value) => {
                write!(formatter, "wrong exceptional SC provider {value}")
            }
            Self::MissingEncodingBackend => formatter.write_str("missing encoding backend"),
            Self::MissingSecondaryCapture => {
                formatter.write_str("missing Secondary Capture contract")
            }
            Self::BackendMismatch { expected, actual } => {
                write!(formatter, "backend {actual} does not match {expected}")
            }
            Self::Registry(value) | Self::Policy(value) | Self::Parameters(value) => {
                formatter.write_str(value)
            }
            Self::ContentDigestMismatch => formatter.write_str("native content digest mismatch"),
            Self::Sc(error) => write!(formatter, "Secondary Capture planning failed: {error}"),
        }
    }
}

impl Error for ExceptionalScPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sc(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ScPlanError> for ExceptionalScPlanError {
    fn from(value: ScPlanError) -> Self {
        Self::Sc(value)
    }
}
