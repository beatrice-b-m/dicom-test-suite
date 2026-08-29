//! Direct plan providers for feature-free Enhanced CT, MR, and PET.
//!
//! All case-specific geometry, pixels, dimensions, and concatenation facts are
//! explicit typed input. Catalog ownership only authenticates identity; it is
//! never used as a hidden source of planning semantics.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue,
    ByteOrder as PlanByteOrder, CompositionUidRole, DicomVr, IdentityPlan,
    NativePixelPlan as PlanPixelPlan, PhotometricInterpretation as PlanPhotometric,
    PixelShape as PlanPixelShape, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
    SampleType, TemplateId, TemplateVersion, ValueOrigin, canonical_native_pixels,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, OutputPlan,
    OutputRelativePath, PlannedDicomArtifact, PreamblePolicy, SequenceLengthPolicy, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, NativeFrameBinding, SlotExecutionBinding,
};
use crate::native_pixel::{
    ByteOrder, NativePixelContent, NativePixelFactory, NativePixelLimits, NativePixelRequest,
    PhotometricInterpretation, PixelDataVr, PixelShape, StoredValueType,
};
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};
use crate::{IMPLEMENTATION_VERSION_NAME, PACKAGE_VERSION, sha256_hex};

use super::{
    AdvancedArtifactPlanningContext, AdvancedArtifactProvenance, AdvancedArtifactRole,
    AdvancedPlanProvider, AdvancedPlanProviderOutput, AdvancedPlanProviderRequest,
    AdvancedPlannedArtifact, AdvancedProviderContractError, AdvancedProviderFamily, CaseRecipe,
};

pub const ENHANCED_PLAN_PROVIDER_ID: &str = "native.enhanced_plan";
pub const ENHANCED_ALGORITHM_PROVIDER_ID: &str = "algorithm.enhanced";

const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const ENHANCED_CT_SOP: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const ENHANCED_MR_SOP: &str = "1.2.840.10008.5.1.4.1.1.4.1";
const ENHANCED_PET_SOP: &str = "1.2.840.10008.5.1.4.1.1.130";

pub const ENHANCED_CONCATENATION_PREDECESSOR_RELATIONSHIP: &str =
    "enhanced_concatenation_predecessor";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnhancedProviderInput {
    Ct(EnhancedCtInput),
    Mr(EnhancedMrInput),
    Pet(EnhancedPetInput),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedCommonInput {
    pub case_id: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub template_id: String,
    pub modality: String,
    pub study_id: String,
    pub device_serial_number: String,
    pub image_type: String,
    pub rows: u16,
    pub columns: u16,
    pub frame_type: String,
    pub pixel_presentation: String,
    pub volumetric_properties: String,
    pub volume_based_calculation_technique: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedNativePixels {
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedFrameGeometry {
    pub image_position_patient: String,
    pub dimension_index_value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedCtPartInput {
    pub template_id: String,
    pub output_path: OutputRelativePath,
    pub frames: Vec<EnhancedFrameGeometry>,
    pub pixels: EnhancedNativePixels,
    pub in_concatenation_number: Option<u16>,
    pub concatenation_frame_offset_number: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedCtInput {
    pub common: EnhancedCommonInput,
    pub pixel_spacing: String,
    pub image_orientation_patient: String,
    pub slice_thickness: String,
    pub spacing_between_slices: String,
    pub rescale_intercept: String,
    pub rescale_slope: String,
    pub rescale_type: String,
    pub parts: Vec<EnhancedCtPartInput>,
    pub concatenation: bool,
    pub stress: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnhancedMrFrameAxis {
    EffectiveEchoTime {
        values: Vec<f64>,
    },
    TemporalPositionTimeOffset {
        values: Vec<f64>,
    },
    VelocityEncoding {
        directions: Vec<[f64; 3]>,
        minimum: f64,
        maximum: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedMrInput {
    pub common: EnhancedCommonInput,
    pub output_path: OutputRelativePath,
    pub frames: Vec<EnhancedFrameGeometry>,
    pub pixels: EnhancedNativePixels,
    pub pixel_spacing: String,
    pub image_orientation_patient: String,
    pub slice_thickness: String,
    pub spacing_between_slices: String,
    pub rescale_intercept: String,
    pub rescale_slope: String,
    pub rescale_type: String,
    pub repetition_time: String,
    pub flip_angle: String,
    pub echo_train_length: String,
    pub rf_echo_train_length: u16,
    pub gradient_echo_train_length: u16,
    pub axis: EnhancedMrFrameAxis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnhancedPetInput {
    pub common: EnhancedCommonInput,
    pub output_path: OutputRelativePath,
    pub frames: Vec<EnhancedFrameGeometry>,
    pub temporal_position_indices: Vec<u32>,
    pub in_stack_position_numbers: Vec<u32>,
    pub stack_id: String,
    pub pixels: EnhancedNativePixels,
    pub pixel_spacing: String,
    pub image_orientation_patient: String,
    pub slice_thickness: String,
    pub spacing_between_slices: String,
    pub rescale_intercept: String,
    pub rescale_slope: String,
    pub units: String,
    pub counts_source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
enum EnhancedRecipeParameters {
    Ct {
        common: EnhancedDocumentCommon,
        pixel_spacing: String,
        image_orientation_patient: String,
        slice_thickness: String,
        spacing_between_slices: String,
        rescale_intercept: String,
        rescale_slope: String,
        rescale_type: String,
        concatenation: bool,
        stress: bool,
    },
    Mr {
        common: EnhancedDocumentCommon,
        pixel_spacing: String,
        image_orientation_patient: String,
        slice_thickness: String,
        spacing_between_slices: String,
        rescale_intercept: String,
        rescale_slope: String,
        rescale_type: String,
        repetition_time: String,
        flip_angle: String,
        echo_train_length: String,
        rf_echo_train_length: u16,
        gradient_echo_train_length: u16,
        axis: EnhancedMrFrameAxis,
    },
    Pet {
        common: EnhancedDocumentCommon,
        pixel_spacing: String,
        image_orientation_patient: String,
        slice_thickness: String,
        spacing_between_slices: String,
        rescale_intercept: String,
        rescale_slope: String,
        units: String,
        counts_source: String,
        stack_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnhancedDocumentCommon {
    modality: String,
    study_id: String,
    device_serial_number: String,
    image_type: String,
    rows: u16,
    columns: u16,
    frame_type: String,
    pixel_presentation: String,
    volumetric_properties: String,
    volume_based_calculation_technique: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnhancedArtifactParameters {
    frames: EnhancedFrameSource,
    pixels: EnhancedPixelSource,
    #[serde(default)]
    in_concatenation_number: Option<u16>,
    #[serde(default)]
    concatenation_frame_offset_number: Option<u32>,
    #[serde(default)]
    temporal_position_indices: Vec<u32>,
    #[serde(default)]
    in_stack_position_numbers: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
enum EnhancedFrameSource {
    Literal {
        values: Vec<EnhancedFrameGeometry>,
    },
    AxialLinear {
        frame_count: u32,
        start_z: f64,
        spacing: f64,
        first_dimension_index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
enum EnhancedPixelSource {
    Literal {
        stored_values: Vec<i64>,
        pixel_min: i64,
        pixel_max: i64,
    },
    ModuloRamp {
        modulus: u32,
    },
}

pub(crate) fn enhanced_input_from_recipe(
    recipe: &CaseRecipe,
) -> Result<Option<EnhancedProviderInput>, String> {
    if recipe.plan_provider_id != ENHANCED_PLAN_PROVIDER_ID {
        return Ok(None);
    }
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| "native.enhanced_plan requires DICOM artifacts".to_string())?;
    let parameters: EnhancedRecipeParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| format!("enhanced provider_parameters: {error}"))?;
    let artifacts = dicom
        .artifacts
        .iter()
        .map(|artifact| {
            let parameters: EnhancedArtifactParameters =
                serde_json::from_value(Value::Object(artifact.parameters.clone()))
                    .map_err(|error| format!("{} parameters: {error}", artifact.logical_id))?;
            let template_id = artifact
                .template
                .as_ref()
                .ok_or_else(|| format!("{} requires a template", artifact.logical_id))?
                .template_id
                .clone();
            let output_path =
                artifact.output.path.as_ref().ok_or_else(|| {
                    format!("{} requires an exact output path", artifact.logical_id)
                })?;
            Ok((
                parameters,
                template_id,
                OutputRelativePath::new(output_path.clone()).map_err(|error| error.to_string())?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let common = |value: EnhancedDocumentCommon, template_id: String| EnhancedCommonInput {
        case_id: recipe.binding.case_id.clone(),
        recipe_id: recipe.recipe_id.clone(),
        recipe_version: recipe.recipe_version.clone(),
        template_id,
        modality: value.modality,
        study_id: value.study_id,
        device_serial_number: value.device_serial_number,
        image_type: value.image_type,
        rows: value.rows,
        columns: value.columns,
        frame_type: value.frame_type,
        pixel_presentation: value.pixel_presentation,
        volumetric_properties: value.volumetric_properties,
        volume_based_calculation_technique: value.volume_based_calculation_technique,
    };
    let expand = |parameters: EnhancedArtifactParameters,
                  common: &EnhancedDocumentCommon|
     -> Result<
        (
            Vec<EnhancedFrameGeometry>,
            EnhancedNativePixels,
            Vec<u32>,
            Vec<u32>,
        ),
        String,
    > {
        let frames = match parameters.frames {
            EnhancedFrameSource::Literal { values } => values,
            EnhancedFrameSource::AxialLinear {
                frame_count,
                start_z,
                spacing,
                first_dimension_index,
            } => (0..frame_count)
                .map(|index| EnhancedFrameGeometry {
                    image_position_patient: format!(
                        "0\\0\\{}",
                        start_z + f64::from(index) * spacing
                    ),
                    dimension_index_value: first_dimension_index + index,
                })
                .collect(),
        };
        let sample_count = usize::from(common.rows)
            .checked_mul(usize::from(common.columns))
            .and_then(|value| value.checked_mul(frames.len()))
            .ok_or_else(|| "enhanced pixel cardinality overflows".to_string())?;
        let pixels = match parameters.pixels {
            EnhancedPixelSource::Literal {
                stored_values,
                pixel_min,
                pixel_max,
            } => EnhancedNativePixels {
                stored_values,
                pixel_min,
                pixel_max,
            },
            EnhancedPixelSource::ModuloRamp { modulus } => {
                if modulus == 0 {
                    return Err("enhanced modulo ramp requires a non-zero modulus".into());
                }
                EnhancedNativePixels {
                    stored_values: (0..sample_count)
                        .map(|index| {
                            i64::try_from(index % modulus as usize).expect("modulus is u32")
                        })
                        .collect(),
                    pixel_min: 0,
                    pixel_max: i64::from(modulus - 1),
                }
            }
        };
        if pixels.stored_values.len() != sample_count {
            return Err("enhanced pixel cardinality does not match geometry".into());
        }
        if pixels.stored_values.iter().min().copied() != Some(pixels.pixel_min)
            || pixels.stored_values.iter().max().copied() != Some(pixels.pixel_max)
        {
            return Err("enhanced pixel extrema do not match stored values".into());
        }
        Ok((
            frames,
            pixels,
            parameters.temporal_position_indices,
            parameters.in_stack_position_numbers,
        ))
    };
    match parameters {
        EnhancedRecipeParameters::Ct {
            common: document_common,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness,
            spacing_between_slices,
            rescale_intercept,
            rescale_slope,
            rescale_type,
            concatenation,
            stress,
        } => {
            let template = artifacts
                .first()
                .map(|value| value.1.clone())
                .ok_or_else(|| "enhanced CT requires artifacts".to_string())?;
            let mut parts = Vec::with_capacity(artifacts.len());
            for (parameters, template_id, output_path) in artifacts {
                let number = parameters.in_concatenation_number;
                let offset = parameters.concatenation_frame_offset_number;
                let (frames, pixels, temporal, stack) = expand(parameters, &document_common)?;
                if !temporal.is_empty() || !stack.is_empty() {
                    return Err("CT artifacts cannot declare PET indices".into());
                }
                parts.push(EnhancedCtPartInput {
                    template_id,
                    output_path,
                    frames,
                    pixels,
                    in_concatenation_number: number,
                    concatenation_frame_offset_number: offset,
                });
            }
            Ok(Some(EnhancedProviderInput::Ct(EnhancedCtInput {
                common: common(document_common, template),
                pixel_spacing,
                image_orientation_patient,
                slice_thickness,
                spacing_between_slices,
                rescale_intercept,
                rescale_slope,
                rescale_type,
                parts,
                concatenation,
                stress,
            })))
        }
        EnhancedRecipeParameters::Mr {
            common: document_common,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness,
            spacing_between_slices,
            rescale_intercept,
            rescale_slope,
            rescale_type,
            repetition_time,
            flip_angle,
            echo_train_length,
            rf_echo_train_length,
            gradient_echo_train_length,
            axis,
        } => {
            if artifacts.len() != 1 {
                return Err("enhanced MR requires exactly one artifact".into());
            }
            let (parameters, template_id, output_path) =
                artifacts.into_iter().next().expect("one artifact");
            let (frames, pixels, temporal, stack) = expand(parameters, &document_common)?;
            if !temporal.is_empty() || !stack.is_empty() {
                return Err("MR artifacts cannot declare PET indices".into());
            }
            Ok(Some(EnhancedProviderInput::Mr(EnhancedMrInput {
                common: common(document_common, template_id),
                output_path,
                frames,
                pixels,
                pixel_spacing,
                image_orientation_patient,
                slice_thickness,
                spacing_between_slices,
                rescale_intercept,
                rescale_slope,
                rescale_type,
                repetition_time,
                flip_angle,
                echo_train_length,
                rf_echo_train_length,
                gradient_echo_train_length,
                axis,
            })))
        }
        EnhancedRecipeParameters::Pet {
            common: document_common,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness,
            spacing_between_slices,
            rescale_intercept,
            rescale_slope,
            units,
            counts_source,
            stack_id,
        } => {
            if artifacts.len() != 1 {
                return Err("enhanced PET requires exactly one artifact".into());
            }
            let (parameters, template_id, output_path) =
                artifacts.into_iter().next().expect("one artifact");
            let (frames, pixels, temporal_position_indices, in_stack_position_numbers) =
                expand(parameters, &document_common)?;
            Ok(Some(EnhancedProviderInput::Pet(EnhancedPetInput {
                common: common(document_common, template_id),
                output_path,
                frames,
                temporal_position_indices,
                in_stack_position_numbers,
                stack_id,
                pixels,
                pixel_spacing,
                image_orientation_patient,
                slice_thickness,
                spacing_between_slices,
                rescale_intercept,
                rescale_slope,
                units,
                counts_source,
            })))
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnhancedPlanProvider {
    standards_lock_sha256: String,
    pixel_limits: NativePixelLimits,
}

impl EnhancedPlanProvider {
    pub fn new(standards_lock_sha256: impl Into<String>) -> Result<Self, EnhancedPlanError> {
        let standards_lock_sha256 = standards_lock_sha256.into();
        if standards_lock_sha256.len() != 64
            || !standards_lock_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(EnhancedPlanError::InvalidStandardsLockHash);
        }
        Ok(Self {
            standards_lock_sha256,
            pixel_limits: NativePixelLimits::default(),
        })
    }

    pub fn with_pixel_limits(mut self, limits: NativePixelLimits) -> Self {
        self.pixel_limits = limits;
        self
    }

    pub fn recipe_default_contexts(
        &self,
        input: &EnhancedProviderInput,
        seed: u64,
    ) -> Result<Vec<AdvancedArtifactPlanningContext>, EnhancedPlanError> {
        let (common, artifacts): (&EnhancedCommonInput, Vec<(String, u64, OutputRelativePath)>) =
            match input {
                EnhancedProviderInput::Ct(value) => (
                    &value.common,
                    value
                        .parts
                        .iter()
                        .enumerate()
                        .map(|(index, part)| {
                            (
                                format!(
                                    "advanced_{}_artifact_{}",
                                    value.common.recipe_id,
                                    index + 1
                                ),
                                index as u64,
                                part.output_path.clone(),
                            )
                        })
                        .collect(),
                ),
                EnhancedProviderInput::Mr(value) => (
                    &value.common,
                    vec![(
                        format!("advanced_{}_artifact_1", value.common.recipe_id),
                        0,
                        value.output_path.clone(),
                    )],
                ),
                EnhancedProviderInput::Pet(value) => (
                    &value.common,
                    vec![(
                        format!("advanced_{}_artifact_1", value.common.recipe_id),
                        0,
                        value.output_path.clone(),
                    )],
                ),
            };
        let shared = Uids::new(&self.standards_lock_sha256, common, seed, 0);
        let concatenation =
            matches!(input, EnhancedProviderInput::Ct(value) if value.concatenation).then(|| {
                (
                    generated_uid(
                        &self.standards_lock_sha256,
                        common,
                        seed,
                        0,
                        UidRole::Concatenation,
                    ),
                    generated_uid(
                        &self.standards_lock_sha256,
                        common,
                        seed,
                        0,
                        UidRole::ConcatenationSource,
                    ),
                )
            });
        artifacts
            .into_iter()
            .enumerate()
            .map(|(index, (recipe_id, order, path))| {
                let mut ids = Uids::new(&self.standards_lock_sha256, common, seed, index as u32);
                if index > 0 {
                    ids.share_non_instance(&shared);
                }
                let mut values = ids.identity_values();
                if let Some((uid, source)) = &concatenation {
                    values.push((CompositionUidRole::Concatenation, 0, uid.clone()));
                    values.push((CompositionUidRole::ConcatenationSource, 0, source.clone()));
                }
                let target_instance_id = recipe_id.clone();
                Ok(AdvancedArtifactPlanningContext {
                    recipe_artifact_logical_id: recipe_id,
                    target_instance_id: target_instance_id.clone(),
                    order,
                    output: OutputPlan {
                        relative_path: path,
                        role: "dicom_instance".into(),
                        publish: true,
                    },
                    identities: IdentityPlan::from_exact_values(target_instance_id, values)
                        .map_err(|error| EnhancedPlanError::Identity(error.to_string()))?,
                })
            })
            .collect()
    }

    pub fn plan_typed(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &EnhancedProviderInput,
    ) -> Result<AdvancedPlanProviderOutput, EnhancedPlanError> {
        request.validate().map_err(EnhancedPlanError::Contract)?;
        if request.family != AdvancedProviderFamily::Enhanced {
            return Err(EnhancedPlanError::WrongFamily);
        }
        let common = match input {
            EnhancedProviderInput::Ct(value) => &value.common,
            EnhancedProviderInput::Mr(value) => &value.common,
            EnhancedProviderInput::Pet(value) => &value.common,
        };
        validate_ownership(request, common)?;
        let output = match input {
            EnhancedProviderInput::Ct(value) => self.plan_ct(request, value)?,
            EnhancedProviderInput::Mr(value) => self.plan_mr(request, value)?,
            EnhancedProviderInput::Pet(value) => self.plan_pet(request, value)?,
        };
        output
            .validate(request)
            .map_err(EnhancedPlanError::Contract)?;
        Ok(output)
    }
}

impl AdvancedPlanProvider for EnhancedPlanProvider {
    type ProviderInput = EnhancedProviderInput;

    fn provider_id(&self) -> &str {
        ENHANCED_PLAN_PROVIDER_ID
    }

    fn plan(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &Self::ProviderInput,
    ) -> Result<AdvancedPlanProviderOutput, AdvancedProviderContractError> {
        self.plan_typed(request, input).map_err(|error| {
            AdvancedProviderContractError::InvalidProviderOutput(error.to_string())
        })
    }
}

fn validate_ownership(
    request: &AdvancedPlanProviderRequest,
    common: &EnhancedCommonInput,
) -> Result<(), EnhancedPlanError> {
    if request.case_id != common.case_id
        || request.recipe.recipe_id != common.recipe_id
        || request.recipe.recipe_version != common.recipe_version
    {
        return Err(EnhancedPlanError::RecipeIdentityMismatch);
    }
    if common.rows == 0 || common.columns == 0 {
        return Err(EnhancedPlanError::ZeroGeometry);
    }
    Ok(())
}

// Provider implementations and neutral attribute builders follow below.

impl EnhancedPlanProvider {
    fn plan_ct(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &EnhancedCtInput,
    ) -> Result<AdvancedPlanProviderOutput, EnhancedPlanError> {
        if input.parts.is_empty() {
            return Err(EnhancedPlanError::EmptyFrames);
        }
        if input.common.modality != "CT" {
            return Err(EnhancedPlanError::RecipeIdentityMismatch);
        }
        if input.concatenation != (input.parts.len() > 1) {
            return Err(EnhancedPlanError::InvalidConcatenation);
        }
        let first_recipe_id = format!("advanced_{}_artifact_1", input.common.recipe_id);
        let first_context = request
            .artifact_context(&first_recipe_id)
            .map_err(EnhancedPlanError::Contract)?;
        let concatenation_uid = first_context
            .identities
            .get(&CompositionUidRole::Concatenation, 0)
            .unwrap_or_default()
            .to_owned();
        let concatenation_source_uid = first_context
            .identities
            .get(&CompositionUidRole::ConcatenationSource, 0)
            .unwrap_or_default()
            .to_owned();
        if input.concatenation {
            if concatenation_uid.is_empty() || concatenation_source_uid.is_empty() {
                return Err(EnhancedPlanError::Identity(
                    "concatenation identities are required".into(),
                ));
            }
            for index in 0..input.parts.len() {
                let recipe_id =
                    format!("advanced_{}_artifact_{}", input.common.recipe_id, index + 1);
                let context = request
                    .artifact_context(&recipe_id)
                    .map_err(EnhancedPlanError::Contract)?;
                if context
                    .identities
                    .get(&CompositionUidRole::Concatenation, 0)
                    != Some(concatenation_uid.as_str())
                    || context
                        .identities
                        .get(&CompositionUidRole::ConcatenationSource, 0)
                        != Some(concatenation_source_uid.as_str())
                {
                    return Err(EnhancedPlanError::InvalidConcatenation);
                }
            }
        }
        let mut artifacts = Vec::with_capacity(input.parts.len());
        let mut bindings = Vec::with_capacity(input.parts.len());
        let mut expected_offset = 0_u32;
        for (index, part) in input.parts.iter().enumerate() {
            validate_frames(&part.frames)?;
            let expected_template = if input.concatenation {
                format!("enhanced/ct/concatenation-part-{}", index + 1)
            } else {
                "enhanced/ct".into()
            };
            if part.template_id != expected_template {
                return Err(EnhancedPlanError::RecipeIdentityMismatch);
            }
            let declared_number = part.in_concatenation_number;
            if input.concatenation {
                if declared_number != Some((index + 1) as u16)
                    || part.concatenation_frame_offset_number != Some(expected_offset)
                {
                    return Err(EnhancedPlanError::InvalidConcatenation);
                }
            } else if declared_number.is_some() || part.concatenation_frame_offset_number.is_some()
            {
                return Err(EnhancedPlanError::InvalidConcatenation);
            }
            expected_offset = expected_offset
                .checked_add(part.frames.len() as u32)
                .ok_or(EnhancedPlanError::ResourceOverflow)?;
            let recipe_logical_id =
                format!("advanced_{}_artifact_{}", input.common.recipe_id, index + 1);
            let context = request
                .artifact_context(&recipe_logical_id)
                .map_err(EnhancedPlanError::Contract)?;
            let ids = Uids::from_context(context)?;
            let native = self.native_pixels(&input.common, part.frames.len(), &part.pixels)?;
            let mut attributes = common_attributes(
                &input.common,
                ENHANCED_CT_SOP,
                &ids,
                part.frames.len(),
                false,
            )?;
            if !input.concatenation {
                push_text(&mut attributes, "PatientPosition", DicomVr::CS, "")?;
            }
            for (keyword, vr, value) in [
                (
                    "PixelPresentation",
                    DicomVr::CS,
                    input.common.pixel_presentation.as_str(),
                ),
                (
                    "VolumetricProperties",
                    DicomVr::CS,
                    input.common.volumetric_properties.as_str(),
                ),
                (
                    "VolumeBasedCalculationTechnique",
                    DicomVr::CS,
                    input.common.volume_based_calculation_technique.as_str(),
                ),
            ] {
                push_text(&mut attributes, keyword, vr, value)?;
            }
            if input.concatenation {
                push_text(
                    &mut attributes,
                    "ConcatenationUID",
                    DicomVr::UI,
                    &concatenation_uid,
                )?;
                push_us(
                    &mut attributes,
                    "InConcatenationNumber",
                    declared_number.expect("validated concatenation number"),
                )?;
                push_us(
                    &mut attributes,
                    "InConcatenationTotalNumber",
                    u16::try_from(input.parts.len())
                        .map_err(|_| EnhancedPlanError::ResourceOverflow)?,
                )?;
                push_ul(
                    &mut attributes,
                    "ConcatenationFrameOffsetNumber",
                    part.concatenation_frame_offset_number
                        .expect("validated concatenation offset"),
                )?;
                push_text(
                    &mut attributes,
                    "SOPInstanceUIDOfConcatenationSource",
                    DicomVr::UI,
                    &concatenation_source_uid,
                )?;
            } else {
                for (keyword, vr, value) in [
                    ("ContentQualification", DicomVr::CS, "RESEARCH"),
                    ("BurnedInAnnotation", DicomVr::CS, "NO"),
                    ("LossyImageCompression", DicomVr::CS, "00"),
                    ("PresentationLUTShape", DicomVr::CS, "IDENTITY"),
                ] {
                    push_text(&mut attributes, keyword, vr, value)?;
                }
            }
            attributes.push(dimension_organization(&ids.dimension)?);
            attributes.push(dimension_index(
                "ImagePositionPatient",
                "PlanePositionSequence",
                "SlicePosition",
                &ids.dimension,
            )?);
            attributes.push(ct_shared(input, &ids.irradiation)?);
            attributes.push(ct_per_frame(&part.frames)?);
            attributes.sort_by(|left, right| left.address.cmp(&right.address));
            let content = canonical_content(&native)?;
            let planned = planned(
                request,
                &input.common,
                context,
                &part.template_id,
                ids,
                ENHANCED_CT_SOP,
                attributes,
                content,
                part.frames.len(),
            )?;
            bindings.push(native_binding(&context.target_instance_id, &native)?);
            artifacts.push(AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::EnhancedInstance {
                    ordinal: (index + 1) as u32,
                },
                planned,
                provenance: AdvancedArtifactProvenance::Requested,
            });
        }
        let dependencies = if input.concatenation {
            artifacts
                .windows(2)
                .map(|pair| ArtifactDependency {
                    artifact_id: pair[1].planned.logical_id.clone(),
                    depends_on: pair[0].planned.logical_id.clone(),
                    relationship: ENHANCED_CONCATENATION_PREDECESSOR_RELATIONSHIP.into(),
                    frame_numbers: vec![],
                })
                .collect()
        } else {
            vec![]
        };
        Ok(AdvancedPlanProviderOutput {
            artifacts,
            dependencies,
            references: vec![],
            bindings,
        })
    }

    fn plan_mr(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &EnhancedMrInput,
    ) -> Result<AdvancedPlanProviderOutput, EnhancedPlanError> {
        validate_frames(&input.frames)?;
        if input.common.modality != "MR" || input.common.template_id != "enhanced/mr" {
            return Err(EnhancedPlanError::RecipeIdentityMismatch);
        }
        let axis_len = match &input.axis {
            EnhancedMrFrameAxis::EffectiveEchoTime { values }
            | EnhancedMrFrameAxis::TemporalPositionTimeOffset { values } => values.len(),
            EnhancedMrFrameAxis::VelocityEncoding { directions, .. } => directions.len(),
        };
        if axis_len != input.frames.len() {
            return Err(EnhancedPlanError::FrameCardinality);
        }
        let recipe_logical_id = format!("advanced_{}_artifact_1", input.common.recipe_id);
        let context = request
            .artifact_context(&recipe_logical_id)
            .map_err(EnhancedPlanError::Contract)?;
        let ids = Uids::from_context(context)?;
        let native = self.native_pixels(&input.common, input.frames.len(), &input.pixels)?;
        let mut attributes = common_attributes(
            &input.common,
            ENHANCED_MR_SOP,
            &ids,
            input.frames.len(),
            false,
        )?;
        for (keyword, vr, value) in [
            ("PatientPosition", DicomVr::CS, ""),
            (
                "PixelPresentation",
                DicomVr::CS,
                input.common.pixel_presentation.as_str(),
            ),
            (
                "VolumetricProperties",
                DicomVr::CS,
                input.common.volumetric_properties.as_str(),
            ),
            (
                "VolumeBasedCalculationTechnique",
                DicomVr::CS,
                input.common.volume_based_calculation_technique.as_str(),
            ),
            ("ContentQualification", DicomVr::CS, "RESEARCH"),
            ("ApplicableSafetyStandardAgency", DicomVr::CS, "IEC"),
            ("ComplexImageComponent", DicomVr::CS, "MAGNITUDE"),
            ("AcquisitionContrast", DicomVr::CS, "UNKNOWN"),
            ("BurnedInAnnotation", DicomVr::CS, "NO"),
            ("LossyImageCompression", DicomVr::CS, "00"),
            ("PresentationLUTShape", DicomVr::CS, "IDENTITY"),
        ] {
            push_text(&mut attributes, keyword, vr, value)?;
        }
        attributes.push(dimension_organization(&ids.dimension)?);
        let (index_pointer, group_pointer, label) = match &input.axis {
            EnhancedMrFrameAxis::EffectiveEchoTime { .. } => {
                ("EffectiveEchoTime", "MREchoSequence", "EffectiveEchoTime")
            }
            EnhancedMrFrameAxis::TemporalPositionTimeOffset { .. } => (
                "TemporalPositionTimeOffset",
                "TemporalPositionSequence",
                "TemporalPositionTimeOffset",
            ),
            EnhancedMrFrameAxis::VelocityEncoding { .. } => (
                "VelocityEncodingDirection",
                "MRVelocityEncodingSequence",
                "VelocityEncodingDirection",
            ),
        };
        attributes.push(dimension_index(
            index_pointer,
            group_pointer,
            label,
            &ids.dimension,
        )?);
        attributes.push(mr_shared(input)?);
        attributes.push(mr_per_frame(input)?);
        attributes.sort_by(|left, right| left.address.cmp(&right.address));
        let planned = planned(
            request,
            &input.common,
            context,
            &input.common.template_id,
            ids,
            ENHANCED_MR_SOP,
            attributes,
            canonical_content(&native)?,
            input.frames.len(),
        )?;
        Ok(AdvancedPlanProviderOutput {
            artifacts: vec![AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::EnhancedInstance { ordinal: 1 },
                planned,
                provenance: AdvancedArtifactProvenance::Requested,
            }],
            dependencies: vec![],
            references: vec![],
            bindings: vec![native_binding(&context.target_instance_id, &native)?],
        })
    }

    fn plan_pet(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &EnhancedPetInput,
    ) -> Result<AdvancedPlanProviderOutput, EnhancedPlanError> {
        validate_frames(&input.frames)?;
        if input.common.modality != "PT" || input.common.template_id != "enhanced/pet" {
            return Err(EnhancedPlanError::RecipeIdentityMismatch);
        }
        if input.temporal_position_indices.len() != input.frames.len()
            || input.in_stack_position_numbers.len() != input.frames.len()
            || input.stack_id.is_empty()
        {
            return Err(EnhancedPlanError::FrameCardinality);
        }
        let recipe_logical_id = format!("advanced_{}_artifact_1", input.common.recipe_id);
        let context = request
            .artifact_context(&recipe_logical_id)
            .map_err(EnhancedPlanError::Contract)?;
        let ids = Uids::from_context(context)?;
        let native = self.native_pixels(&input.common, input.frames.len(), &input.pixels)?;
        let mut attributes = common_attributes(
            &input.common,
            ENHANCED_PET_SOP,
            &ids,
            input.frames.len(),
            true,
        )?;
        for (keyword, vr, value) in [
            (
                "PixelPresentation",
                DicomVr::CS,
                input.common.pixel_presentation.as_str(),
            ),
            (
                "VolumetricProperties",
                DicomVr::CS,
                input.common.volumetric_properties.as_str(),
            ),
            (
                "VolumeBasedCalculationTechnique",
                DicomVr::CS,
                input.common.volume_based_calculation_technique.as_str(),
            ),
            ("ContentQualification", DicomVr::CS, "RESEARCH"),
            ("BurnedInAnnotation", DicomVr::CS, "NO"),
            ("LossyImageCompression", DicomVr::CS, "00"),
            ("PresentationLUTShape", DicomVr::CS, "IDENTITY"),
            ("BodyPartExamined", DicomVr::CS, "HEAD"),
            ("TableMotion", DicomVr::CS, "STATIC"),
            ("TimeOfFlightInformationUsed", DicomVr::CS, "FALSE"),
            ("CountsSource", DicomVr::CS, input.counts_source.as_str()),
        ] {
            push_text(&mut attributes, keyword, vr, value)?;
        }
        attributes.push(sequence(
            "ViewCodeSequence",
            vec![code_item("24422004", "SCT", "Axial")?],
        )?);
        for keyword in [
            "DecayCorrected",
            "AttenuationCorrected",
            "ScatterCorrected",
            "DeadTimeCorrected",
            "GantryMotionCorrected",
            "PatientMotionCorrected",
            "CountLossNormalizationCorrected",
            "RandomsCorrected",
            "NonUniformRadialSamplingCorrected",
            "SensitivityCalibrated",
            "DetectorNormalizationCorrection",
        ] {
            push_text(&mut attributes, keyword, DicomVr::CS, "NO")?;
        }
        attributes.push(pet_radiopharmaceutical()?);
        attributes.push(dimension_organization(&ids.dimension)?);
        attributes.push(dimension_index(
            "InStackPositionNumber",
            "FrameContentSequence",
            "InStackPosition",
            &ids.dimension,
        )?);
        attributes.push(pet_shared(input)?);
        attributes.push(pet_per_frame(input)?);
        attributes.sort_by(|left, right| left.address.cmp(&right.address));
        let planned = planned(
            request,
            &input.common,
            context,
            &input.common.template_id,
            ids,
            ENHANCED_PET_SOP,
            attributes,
            canonical_content(&native)?,
            input.frames.len(),
        )?;
        Ok(AdvancedPlanProviderOutput {
            artifacts: vec![AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::EnhancedInstance { ordinal: 1 },
                planned,
                provenance: AdvancedArtifactProvenance::Requested,
            }],
            dependencies: vec![],
            references: vec![],
            bindings: vec![native_binding(&context.target_instance_id, &native)?],
        })
    }
}

#[derive(Clone)]
struct Uids {
    study: String,
    series: String,
    sop: String,
    frame_of_reference: String,
    dimension: String,
    irradiation: String,
    implementation: String,
}

impl Uids {
    fn new(lock: &str, common: &EnhancedCommonInput, seed: u64, file_index: u32) -> Self {
        let make = |role| generated_uid(lock, common, seed, file_index, role);
        Self {
            study: make(UidRole::StudyInstance),
            series: make(UidRole::SeriesInstance),
            sop: make(UidRole::SopInstance),
            frame_of_reference: make(UidRole::FrameOfReference),
            dimension: make(UidRole::DimensionOrganization),
            irradiation: make(UidRole::IrradiationEvent),
            implementation: deterministic_uid(&DeterministicUidInput {
                standards_lock_sha256: lock,
                case_id: "dicom-test-suite/implementation",
                recipe_version: PACKAGE_VERSION,
                run_seed: 0,
                file_index: 0,
                frame_index: None,
                referenced_object_index: None,
                role: UidRole::ImplementationClass,
            }),
        }
    }

    fn share_non_instance(&mut self, shared: &Self) {
        self.study.clone_from(&shared.study);
        self.series.clone_from(&shared.series);
        self.frame_of_reference
            .clone_from(&shared.frame_of_reference);
        self.dimension.clone_from(&shared.dimension);
        self.irradiation.clone_from(&shared.irradiation);
    }

    fn identity_values(&self) -> Vec<(CompositionUidRole, u32, String)> {
        vec![
            (CompositionUidRole::StudyInstance, 0, self.study.clone()),
            (CompositionUidRole::SeriesInstance, 0, self.series.clone()),
            (CompositionUidRole::SopInstance, 0, self.sop.clone()),
            (
                CompositionUidRole::FrameOfReference,
                0,
                self.frame_of_reference.clone(),
            ),
            (
                CompositionUidRole::DimensionOrganization,
                0,
                self.dimension.clone(),
            ),
            (
                CompositionUidRole::IrradiationEvent,
                0,
                self.irradiation.clone(),
            ),
            (
                CompositionUidRole::ImplementationClass,
                0,
                self.implementation.clone(),
            ),
        ]
    }

    fn from_context(context: &AdvancedArtifactPlanningContext) -> Result<Self, EnhancedPlanError> {
        let get = |role| {
            context
                .identities
                .get(&role, 0)
                .map(str::to_owned)
                .ok_or_else(|| EnhancedPlanError::Identity(format!("missing {}", role.as_str())))
        };
        Ok(Self {
            study: get(CompositionUidRole::StudyInstance)?,
            series: get(CompositionUidRole::SeriesInstance)?,
            sop: get(CompositionUidRole::SopInstance)?,
            frame_of_reference: get(CompositionUidRole::FrameOfReference)?,
            dimension: get(CompositionUidRole::DimensionOrganization)?,
            irradiation: get(CompositionUidRole::IrradiationEvent)?,
            implementation: get(CompositionUidRole::ImplementationClass)?,
        })
    }
}

fn generated_uid(
    lock: &str,
    common: &EnhancedCommonInput,
    seed: u64,
    file_index: u32,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: &common.case_id,
        recipe_version: &common.recipe_version,
        run_seed: seed,
        file_index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

impl EnhancedPlanProvider {
    fn native_pixels(
        &self,
        common: &EnhancedCommonInput,
        frames: usize,
        pixels: &EnhancedNativePixels,
    ) -> Result<NativePixelContent, EnhancedPlanError> {
        let frames = u32::try_from(frames).map_err(|_| EnhancedPlanError::ResourceOverflow)?;
        NativePixelFactory
            .create_with_limits(
                NativePixelRequest {
                    shape: PixelShape {
                        rows: u32::from(common.rows),
                        columns: u32::from(common.columns),
                        frames,
                        samples_per_pixel: 1,
                        photometric_interpretation: PhotometricInterpretation::Monochrome2,
                        bits_allocated: 16,
                        bits_stored: 16,
                        high_bit: 15,
                        pixel_representation: 0,
                        stored_value_type: StoredValueType::U16,
                        byte_order: ByteOrder::Little,
                        pixel_data_vr: PixelDataVr::Ow,
                        color: None,
                    },
                    stored_values: pixels.stored_values.clone(),
                    declared_pixel_min: pixels.pixel_min,
                    declared_pixel_max: pixels.pixel_max,
                    expected_frame_sha256: vec![],
                    padding: None,
                    palette: None,
                },
                self.pixel_limits,
            )
            .map_err(|error| EnhancedPlanError::Pixels(error.to_string()))
    }
}

fn validate_frames(frames: &[EnhancedFrameGeometry]) -> Result<(), EnhancedPlanError> {
    if frames.is_empty() {
        return Err(EnhancedPlanError::EmptyFrames);
    }
    for (index, frame) in frames.iter().enumerate() {
        if frame.dimension_index_value == 0
            || (index > 0 && frame.dimension_index_value <= frames[index - 1].dimension_index_value)
        {
            return Err(EnhancedPlanError::InvalidDimensionIndex);
        }
    }
    Ok(())
}

fn canonical_content(
    native: &NativePixelContent,
) -> Result<crate::composition::CanonicalContent, EnhancedPlanError> {
    let shape = &native.plan.shape;
    let plan = PlanPixelPlan::plan(PlanPixelShape {
        rows: shape.rows,
        columns: shape.columns,
        frames: shape.frames,
        samples_per_pixel: 1,
        photometric_interpretation: PlanPhotometric::Monochrome2,
        sample_type: SampleType::UnsignedInteger,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        byte_order: PlanByteOrder::Little,
        planar_configuration: None,
    })
    .map_err(|error| EnhancedPlanError::Pixels(error.to_string()))?;
    Ok(canonical_native_pixels(
        &plan,
        native.unpadded_bytes.clone(),
        BTreeMap::new(),
    ))
}

fn native_binding(
    artifact_id: &str,
    native: &NativePixelContent,
) -> Result<ArtifactExecutionBindings, EnhancedPlanError> {
    let frames = native
        .frames
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            let span = native
                .plan
                .frame_spans
                .get(index)
                .ok_or(EnhancedPlanError::FrameCardinality)?;
            if span.bit_offset % 8 != 0 || span.bit_length % 8 != 0 {
                return Err(EnhancedPlanError::FrameCardinality);
            }
            let frame_bytes = usize::try_from(span.bit_length / 8)
                .map_err(|_| EnhancedPlanError::ResourceOverflow)?;
            let start = index
                .checked_mul(frame_bytes)
                .ok_or(EnhancedPlanError::ResourceOverflow)?;
            let end = start
                .checked_add(frame_bytes)
                .ok_or(EnhancedPlanError::ResourceOverflow)?;
            let bytes = native
                .unpadded_bytes
                .get(start..end)
                .ok_or(EnhancedPlanError::ResourceOverflow)?
                .to_vec();
            Ok(NativeFrameBinding {
                frame_number: frame.frame_number,
                bytes: ByteBinding::Inline {
                    sha256: sha256_hex(&bytes),
                    bytes,
                },
                rows: native.plan.shape.rows,
                columns: native.plan.shape.columns,
                samples_per_pixel: 1,
                bits_allocated: 16,
                photometric_interpretation: "MONOCHROME2".into(),
            })
        })
        .collect::<Result<Vec<_>, EnhancedPlanError>>()?;
    Ok(ArtifactExecutionBindings {
        artifact_id: artifact_id.into(),
        slots: BTreeMap::from([(
            "pixels".into(),
            SlotExecutionBinding::NativeFrames { frames },
        )]),
    })
}

#[allow(clippy::too_many_arguments)]
fn planned(
    request: &AdvancedPlanProviderRequest,
    common: &EnhancedCommonInput,
    context: &AdvancedArtifactPlanningContext,
    template_id: &str,
    ids: Uids,
    sop_class_uid: &str,
    attributes: Vec<ResolvedAttribute>,
    content: crate::composition::CanonicalContent,
    frames: usize,
) -> Result<PlannedDicomArtifact, EnhancedPlanError> {
    let logical_id = &context.target_instance_id;
    let pixel_bytes = u64::from(common.rows)
        .checked_mul(u64::from(common.columns))
        .and_then(|value| value.checked_mul(frames as u64))
        .and_then(|value| value.checked_mul(2))
        .ok_or(EnhancedPlanError::ResourceOverflow)?;
    Ok(PlannedDicomArtifact {
        logical_id: logical_id.clone(),
        order: context.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: request.case_id.clone(),
            recipe_id: request.recipe.recipe_id.clone(),
            recipe_version: request.recipe.recipe_version.clone(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: logical_id.clone(),
            template_id: TemplateId(template_id.into()),
            template_version: "1.0.0"
                .parse::<TemplateVersion>()
                .map_err(|error| EnhancedPlanError::Template(error.to_string()))?,
            sop_class_uid: sop_class_uid.into(),
            transfer_syntax_uid: EXPLICIT_VR_LE.into(),
            identities: context.identities.clone(),
            attributes,
            content: vec![content],
            references: vec![],
        },
        output: context.output.clone(),
        encoding: EncodingPlan {
            transfer_syntax_uid: EXPLICIT_VR_LE.into(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: ids.implementation,
                version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: ValidationPlan {
            rules: [
                "enhanced.functional_groups",
                "enhanced.dimensions",
                "content.native_pixels",
            ]
            .into_iter()
            .map(|rule_id| ValidationRule {
                rule_id: rule_id.into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            })
            .collect(),
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: format!("same-project:{logical_id}"),
                route_id: "builtin.strict".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: pixel_bytes
                .checked_add(256 * 1024)
                .ok_or(EnhancedPlanError::ResourceOverflow)?,
            peak_working_bytes: pixel_bytes
                .checked_mul(2)
                .and_then(|value| value.checked_add(512 * 1024))
                .ok_or(EnhancedPlanError::ResourceOverflow)?,
        },
    })
}

fn common_attributes(
    common: &EnhancedCommonInput,
    sop_class_uid: &str,
    ids: &Uids,
    frames: usize,
    include_series_datetime: bool,
) -> Result<Vec<ResolvedAttribute>, EnhancedPlanError> {
    let mut attributes = Vec::new();
    for (keyword, vr, value) in [
        ("SOPClassUID", DicomVr::UI, sop_class_uid),
        ("SOPInstanceUID", DicomVr::UI, ids.sop.as_str()),
        ("SyntheticData", DicomVr::CS, "YES"),
        ("PatientName", DicomVr::PN, "DTS^Synthetic^Patient001"),
        ("PatientID", DicomVr::LO, "DTS-PATIENT-001"),
        ("PatientBirthDate", DicomVr::DA, "19700101"),
        ("PatientSex", DicomVr::CS, "O"),
        ("StudyInstanceUID", DicomVr::UI, ids.study.as_str()),
        ("StudyDate", DicomVr::DA, "20260101"),
        ("StudyTime", DicomVr::TM, "000000"),
        ("ReferringPhysicianName", DicomVr::PN, ""),
        ("StudyID", DicomVr::SH, common.study_id.as_str()),
        ("AccessionNumber", DicomVr::SH, ""),
        ("Modality", DicomVr::CS, common.modality.as_str()),
        ("SeriesInstanceUID", DicomVr::UI, ids.series.as_str()),
        ("SeriesNumber", DicomVr::IS, "1"),
        (
            "FrameOfReferenceUID",
            DicomVr::UI,
            ids.frame_of_reference.as_str(),
        ),
        ("PositionReferenceIndicator", DicomVr::LO, ""),
        ("Manufacturer", DicomVr::LO, "dicom-test-suite"),
        (
            "ManufacturerModelName",
            DicomVr::LO,
            common.recipe_id.as_str(),
        ),
        (
            "DeviceSerialNumber",
            DicomVr::LO,
            common.device_serial_number.as_str(),
        ),
        ("SoftwareVersions", DicomVr::LO, PACKAGE_VERSION),
        ("ImageType", DicomVr::CS, common.image_type.as_str()),
        ("InstanceNumber", DicomVr::IS, "1"),
        ("ContentDate", DicomVr::DA, "20260101"),
        ("ContentTime", DicomVr::TM, "000000"),
        ("PhotometricInterpretation", DicomVr::CS, "MONOCHROME2"),
    ] {
        push_text(&mut attributes, keyword, vr, value)?;
    }
    push_text(
        &mut attributes,
        "NumberOfFrames",
        DicomVr::IS,
        &frames.to_string(),
    )?;
    if include_series_datetime {
        push_text(&mut attributes, "SeriesDate", DicomVr::DA, "20260101")?;
        push_text(&mut attributes, "SeriesTime", DicomVr::TM, "000000")?;
    }
    for (keyword, value) in [
        ("SamplesPerPixel", 1),
        ("Rows", common.rows),
        ("Columns", common.columns),
        ("BitsAllocated", 16),
        ("BitsStored", 16),
        ("HighBit", 15),
        ("PixelRepresentation", 0),
    ] {
        push_us(&mut attributes, keyword, value)?;
    }
    attributes.push(sequence("AcquisitionContextSequence", vec![])?);
    Ok(attributes)
}

fn dimension_organization(uid: &str) -> Result<ResolvedAttribute, EnhancedPlanError> {
    sequence(
        "DimensionOrganizationSequence",
        vec![vec![set_text(
            "DimensionOrganizationUID",
            DicomVr::UI,
            uid,
        )?]],
    )
}

fn dimension_index(
    index_pointer: &str,
    group_pointer: &str,
    label: &str,
    uid: &str,
) -> Result<ResolvedAttribute, EnhancedPlanError> {
    sequence(
        "DimensionIndexSequence",
        vec![vec![
            set_tag("DimensionIndexPointer", index_pointer)?,
            set_tag("FunctionalGroupPointer", group_pointer)?,
            set_text("DimensionOrganizationUID", DicomVr::UI, uid)?,
            set_text("DimensionDescriptionLabel", DicomVr::LO, label)?,
        ]],
    )
}

fn ct_shared(
    input: &EnhancedCtInput,
    irradiation_uid: &str,
) -> Result<ResolvedAttribute, EnhancedPlanError> {
    sequence(
        "SharedFunctionalGroupsSequence",
        vec![vec![
            sequence_op(
                "PixelMeasuresSequence",
                vec![vec![
                    set_text("PixelSpacing", DicomVr::DS, &input.pixel_spacing)?,
                    set_text("SliceThickness", DicomVr::DS, &input.slice_thickness)?,
                    set_text(
                        "SpacingBetweenSlices",
                        DicomVr::DS,
                        &input.spacing_between_slices,
                    )?,
                ]],
            )?,
            sequence_op(
                "PlaneOrientationSequence",
                vec![vec![set_text(
                    "ImageOrientationPatient",
                    DicomVr::DS,
                    &input.image_orientation_patient,
                )?]],
            )?,
            sequence_op(
                "FrameAnatomySequence",
                vec![vec![
                    set_text("FrameLaterality", DicomVr::CS, "U")?,
                    sequence_op(
                        "AnatomicRegionSequence",
                        vec![code_item("T-D3000", "SRT", "Chest")?],
                    )?,
                ]],
            )?,
            sequence_op(
                "IrradiationEventIdentificationSequence",
                vec![vec![set_text(
                    "IrradiationEventUID",
                    DicomVr::UI,
                    irradiation_uid,
                )?]],
            )?,
            sequence_op(
                "CTImageFrameTypeSequence",
                vec![vec![
                    set_text("FrameType", DicomVr::CS, &input.common.frame_type)?,
                    set_text(
                        "PixelPresentation",
                        DicomVr::CS,
                        &input.common.pixel_presentation,
                    )?,
                    set_text(
                        "VolumetricProperties",
                        DicomVr::CS,
                        &input.common.volumetric_properties,
                    )?,
                    set_text(
                        "VolumeBasedCalculationTechnique",
                        DicomVr::CS,
                        &input.common.volume_based_calculation_technique,
                    )?,
                ]],
            )?,
            sequence_op(
                "PixelValueTransformationSequence",
                vec![vec![
                    set_text("RescaleIntercept", DicomVr::DS, &input.rescale_intercept)?,
                    set_text("RescaleSlope", DicomVr::DS, &input.rescale_slope)?,
                    set_text("RescaleType", DicomVr::LO, &input.rescale_type)?,
                ]],
            )?,
        ]],
    )
}

fn ct_per_frame(frames: &[EnhancedFrameGeometry]) -> Result<ResolvedAttribute, EnhancedPlanError> {
    sequence(
        "PerFrameFunctionalGroupsSequence",
        frames
            .iter()
            .enumerate()
            .map(|(index, frame)| {
                Ok(vec![
                    sequence_op(
                        "FrameContentSequence",
                        vec![vec![
                            set_ul("DimensionIndexValues", frame.dimension_index_value)?,
                            set_us(
                                "FrameAcquisitionNumber",
                                u16::try_from(index + 1)
                                    .map_err(|_| EnhancedPlanError::ResourceOverflow)?,
                            )?,
                        ]],
                    )?,
                    sequence_op(
                        "PlanePositionSequence",
                        vec![vec![set_text(
                            "ImagePositionPatient",
                            DicomVr::DS,
                            &frame.image_position_patient,
                        )?]],
                    )?,
                ])
            })
            .collect::<Result<Vec<_>, EnhancedPlanError>>()?,
    )
}

fn mr_shared(input: &EnhancedMrInput) -> Result<ResolvedAttribute, EnhancedPlanError> {
    let operating_modes = [
        ("STATIC FIELD", "IEC_NORMAL"),
        ("RF", "IEC_NORMAL"),
        ("GRADIENT", "IEC_NORMAL"),
    ]
    .into_iter()
    .map(|(mode_type, mode)| {
        Ok(vec![
            set_text("OperatingModeType", DicomVr::CS, mode_type)?,
            set_text("OperatingMode", DicomVr::CS, mode)?,
        ])
    })
    .collect::<Result<Vec<_>, EnhancedPlanError>>()?;
    sequence(
        "SharedFunctionalGroupsSequence",
        vec![vec![
            sequence_op(
                "PixelMeasuresSequence",
                vec![vec![
                    set_text("PixelSpacing", DicomVr::DS, &input.pixel_spacing)?,
                    set_text("SliceThickness", DicomVr::DS, &input.slice_thickness)?,
                    set_text(
                        "SpacingBetweenSlices",
                        DicomVr::DS,
                        &input.spacing_between_slices,
                    )?,
                ]],
            )?,
            sequence_op(
                "PlaneOrientationSequence",
                vec![vec![set_text(
                    "ImageOrientationPatient",
                    DicomVr::DS,
                    &input.image_orientation_patient,
                )?]],
            )?,
            sequence_op(
                "FrameAnatomySequence",
                vec![vec![
                    set_text("FrameLaterality", DicomVr::CS, "U")?,
                    sequence_op(
                        "AnatomicRegionSequence",
                        vec![code_item("69536005", "SCT", "Head")?],
                    )?,
                ]],
            )?,
            sequence_op(
                "MRImageFrameTypeSequence",
                vec![vec![
                    set_text("FrameType", DicomVr::CS, &input.common.frame_type)?,
                    set_text(
                        "PixelPresentation",
                        DicomVr::CS,
                        &input.common.pixel_presentation,
                    )?,
                    set_text(
                        "VolumetricProperties",
                        DicomVr::CS,
                        &input.common.volumetric_properties,
                    )?,
                    set_text(
                        "VolumeBasedCalculationTechnique",
                        DicomVr::CS,
                        &input.common.volume_based_calculation_technique,
                    )?,
                    set_text("ComplexImageComponent", DicomVr::CS, "MAGNITUDE")?,
                    set_text("AcquisitionContrast", DicomVr::CS, "UNKNOWN")?,
                ]],
            )?,
            sequence_op(
                "PixelValueTransformationSequence",
                vec![vec![
                    set_text("RescaleIntercept", DicomVr::DS, &input.rescale_intercept)?,
                    set_text("RescaleSlope", DicomVr::DS, &input.rescale_slope)?,
                    set_text("RescaleType", DicomVr::LO, &input.rescale_type)?,
                ]],
            )?,
            sequence_op(
                "MRTimingAndRelatedParametersSequence",
                vec![vec![
                    set_text("RepetitionTime", DicomVr::DS, &input.repetition_time)?,
                    set_text("FlipAngle", DicomVr::DS, &input.flip_angle)?,
                    set_text("EchoTrainLength", DicomVr::IS, &input.echo_train_length)?,
                    set_us("RFEchoTrainLength", input.rf_echo_train_length)?,
                    set_us("GradientEchoTrainLength", input.gradient_echo_train_length)?,
                    sequence_op(
                        "SpecificAbsorptionRateSequence",
                        vec![vec![
                            set_text("SpecificAbsorptionRateDefinition", DicomVr::CS, "IEC_HEAD")?,
                            set_fd("SpecificAbsorptionRateValue", 0.1)?,
                        ]],
                    )?,
                    sequence_op("OperatingModeSequence", operating_modes)?,
                ]],
            )?,
        ]],
    )
}

fn mr_per_frame(input: &EnhancedMrInput) -> Result<ResolvedAttribute, EnhancedPlanError> {
    let mut items = Vec::with_capacity(input.frames.len());
    for (index, frame) in input.frames.iter().enumerate() {
        let mut frame_content = vec![
            set_ul("DimensionIndexValues", frame.dimension_index_value)?,
            set_us(
                "FrameAcquisitionNumber",
                u16::try_from(index + 1).map_err(|_| EnhancedPlanError::ResourceOverflow)?,
            )?,
        ];
        let axis = match &input.axis {
            EnhancedMrFrameAxis::EffectiveEchoTime { values } => sequence_op(
                "MREchoSequence",
                vec![vec![set_fd("EffectiveEchoTime", values[index])?]],
            )?,
            EnhancedMrFrameAxis::TemporalPositionTimeOffset { values } => {
                frame_content.push(set_ul(
                    "TemporalPositionIndex",
                    u32::try_from(index + 1).map_err(|_| EnhancedPlanError::ResourceOverflow)?,
                )?);
                sequence_op(
                    "TemporalPositionSequence",
                    vec![vec![set_fd("TemporalPositionTimeOffset", values[index])?]],
                )?
            }
            EnhancedMrFrameAxis::VelocityEncoding {
                directions,
                minimum,
                maximum,
            } => sequence_op(
                "MRVelocityEncodingSequence",
                vec![vec![
                    set_fd_multi("VelocityEncodingDirection", &directions[index])?,
                    set_fd("VelocityEncodingMinimumValue", *minimum)?,
                    set_fd("VelocityEncodingMaximumValue", *maximum)?,
                ]],
            )?,
        };
        items.push(vec![
            sequence_op("FrameContentSequence", vec![frame_content])?,
            sequence_op(
                "PlanePositionSequence",
                vec![vec![set_text(
                    "ImagePositionPatient",
                    DicomVr::DS,
                    &frame.image_position_patient,
                )?]],
            )?,
            axis,
        ]);
    }
    sequence("PerFrameFunctionalGroupsSequence", items)
}

fn pet_radiopharmaceutical() -> Result<ResolvedAttribute, EnhancedPlanError> {
    sequence(
        "RadiopharmaceuticalInformationSequence",
        vec![vec![
            set_us("RadiopharmaceuticalAgentNumber", 1)?,
            sequence_op(
                "RadionuclideCodeSequence",
                vec![code_item("77004003", "SCT", "^18^Fluorine")?],
            )?,
            sequence_op(
                "AdministrationRouteCodeSequence",
                vec![code_item("47625008", "SCT", "Intravenous route")?],
            )?,
            set_text(
                "RadiopharmaceuticalStartDateTime",
                DicomVr::DT,
                "20260101000000",
            )?,
            set_text("RadionuclideTotalDose", DicomVr::DS, "")?,
            set_text("RadionuclideHalfLife", DicomVr::DS, "6586.2")?,
            set_text("RadionuclidePositronFraction", DicomVr::DS, "0.967")?,
            sequence_op(
                "RadiopharmaceuticalCodeSequence",
                vec![code_item("35321007", "SCT", "Fluorodeoxyglucose F^18^")?],
            )?,
        ]],
    )
}

fn pet_shared(input: &EnhancedPetInput) -> Result<ResolvedAttribute, EnhancedPlanError> {
    let real_world_mapping = vec![
        set_us("RealWorldValueFirstValueMapped", 0)?,
        set_us("RealWorldValueLastValueMapped", 400)?,
        set_fd("RealWorldValueIntercept", 0.0)?,
        set_fd(
            "RealWorldValueSlope",
            input
                .rescale_slope
                .parse::<f64>()
                .map_err(|error| EnhancedPlanError::Attribute(error.to_string()))?,
        )?,
        set_text("LUTLabel", DicomVr::SH, &input.units)?,
        set_text("LUTExplanation", DicomVr::LO, "Activity concentration")?,
        sequence_op(
            "MeasurementUnitsCodeSequence",
            vec![code_item("Bq/ml", "UCUM", "Becquerels/milliliter")?],
        )?,
    ];
    sequence(
        "SharedFunctionalGroupsSequence",
        vec![vec![
            sequence_op(
                "PixelMeasuresSequence",
                vec![vec![
                    set_text("PixelSpacing", DicomVr::DS, &input.pixel_spacing)?,
                    set_text("SliceThickness", DicomVr::DS, &input.slice_thickness)?,
                    set_text(
                        "SpacingBetweenSlices",
                        DicomVr::DS,
                        &input.spacing_between_slices,
                    )?,
                ]],
            )?,
            sequence_op(
                "PlaneOrientationSequence",
                vec![vec![set_text(
                    "ImageOrientationPatient",
                    DicomVr::DS,
                    &input.image_orientation_patient,
                )?]],
            )?,
            sequence_op(
                "FrameAnatomySequence",
                vec![vec![
                    set_text("FrameLaterality", DicomVr::CS, "U")?,
                    sequence_op(
                        "AnatomicRegionSequence",
                        vec![code_item("69536005", "SCT", "Head")?],
                    )?,
                ]],
            )?,
            sequence_op(
                "PixelValueTransformationSequence",
                vec![vec![
                    set_text("RescaleIntercept", DicomVr::DS, &input.rescale_intercept)?,
                    set_text("RescaleSlope", DicomVr::DS, &input.rescale_slope)?,
                    set_text("RescaleType", DicomVr::LO, "US")?,
                ]],
            )?,
            sequence_op(
                "FrameVOILUTSequence",
                vec![vec![
                    set_text("WindowCenter", DicomVr::DS, "500")?,
                    set_text("WindowWidth", DicomVr::DS, "1000")?,
                ]],
            )?,
            sequence_op("RealWorldValueMappingSequence", vec![real_world_mapping])?,
            sequence_op(
                "RadiopharmaceuticalUsageSequence",
                vec![vec![set_us("RadiopharmaceuticalAgentNumber", 1)?]],
            )?,
            sequence_op(
                "PETFrameTypeSequence",
                vec![vec![
                    set_text("FrameType", DicomVr::CS, &input.common.frame_type)?,
                    set_text(
                        "PixelPresentation",
                        DicomVr::CS,
                        &input.common.pixel_presentation,
                    )?,
                    set_text(
                        "VolumetricProperties",
                        DicomVr::CS,
                        &input.common.volumetric_properties,
                    )?,
                    set_text(
                        "VolumeBasedCalculationTechnique",
                        DicomVr::CS,
                        &input.common.volume_based_calculation_technique,
                    )?,
                ]],
            )?,
            sequence_op("DerivationImageSequence", vec![])?,
        ]],
    )
}

fn pet_per_frame(input: &EnhancedPetInput) -> Result<ResolvedAttribute, EnhancedPlanError> {
    sequence(
        "PerFrameFunctionalGroupsSequence",
        input
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| {
                Ok(vec![
                    sequence_op(
                        "FrameContentSequence",
                        vec![vec![
                            set_ul("DimensionIndexValues", frame.dimension_index_value)?,
                            set_ul(
                                "TemporalPositionIndex",
                                input.temporal_position_indices[index],
                            )?,
                            set_text("StackID", DicomVr::SH, &input.stack_id)?,
                            set_ul(
                                "InStackPositionNumber",
                                input.in_stack_position_numbers[index],
                            )?,
                        ]],
                    )?,
                    sequence_op(
                        "PlanePositionSequence",
                        vec![vec![set_text(
                            "ImagePositionPatient",
                            DicomVr::DS,
                            &frame.image_position_patient,
                        )?]],
                    )?,
                ])
            })
            .collect::<Result<Vec<_>, EnhancedPlanError>>()?,
    )
}

fn code_item(
    value: &str,
    scheme: &str,
    meaning: &str,
) -> Result<Vec<AttributeOperation>, EnhancedPlanError> {
    Ok(vec![
        set_text("CodeValue", DicomVr::SH, value)?,
        set_text("CodingSchemeDesignator", DicomVr::SH, scheme)?,
        set_text("CodeMeaning", DicomVr::LO, meaning)?,
    ])
}

fn sequence(
    keyword: &str,
    items: Vec<Vec<AttributeOperation>>,
) -> Result<ResolvedAttribute, EnhancedPlanError> {
    Ok(ResolvedAttribute {
        address: address(keyword)?,
        vr: DicomVr::SQ,
        value: Some(AttributeValue::Sequence(
            items
                .into_iter()
                .map(|attributes| AttributeItem { attributes })
                .collect(),
        )),
        origin: ValueOrigin::InstanceOverride,
    })
}

fn sequence_op(
    keyword: &str,
    items: Vec<Vec<AttributeOperation>>,
) -> Result<AttributeOperation, EnhancedPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(
            items
                .into_iter()
                .map(|attributes| AttributeItem { attributes })
                .collect(),
        ),
    })
}

fn push_text(
    attributes: &mut Vec<ResolvedAttribute>,
    keyword: &str,
    vr: DicomVr,
    value: &str,
) -> Result<(), EnhancedPlanError> {
    attributes.push(resolved(set_text(keyword, vr, value)?));
    Ok(())
}

fn push_us(
    attributes: &mut Vec<ResolvedAttribute>,
    keyword: &str,
    value: u16,
) -> Result<(), EnhancedPlanError> {
    attributes.push(resolved(set_us(keyword, value)?));
    Ok(())
}

fn push_ul(
    attributes: &mut Vec<ResolvedAttribute>,
    keyword: &str,
    value: u32,
) -> Result<(), EnhancedPlanError> {
    attributes.push(resolved(set_ul(keyword, value)?));
    Ok(())
}

fn resolved(operation: AttributeOperation) -> ResolvedAttribute {
    let AttributeOperation::Set { address, vr, value } = operation else {
        unreachable!("enhanced helpers construct set operations")
    };
    ResolvedAttribute {
        address,
        vr,
        value: Some(value),
        origin: ValueOrigin::InstanceOverride,
    }
}

fn set_text(
    keyword: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, EnhancedPlanError> {
    let values = value
        .split('\\')
        .map(|value| PrimitiveValue::String(value.into()))
        .collect::<Vec<_>>();
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr,
        value: if values.len() == 1 {
            AttributeValue::Primitive(values.into_iter().next().expect("one value"))
        } else {
            AttributeValue::Multi(values)
        },
    })
}

fn set_us(keyword: &str, value: u16) -> Result<AttributeOperation, EnhancedPlanError> {
    set_unsigned(keyword, DicomVr::US, u64::from(value))
}

fn set_ul(keyword: &str, value: u32) -> Result<AttributeOperation, EnhancedPlanError> {
    set_unsigned(keyword, DicomVr::UL, u64::from(value))
}

fn set_unsigned(
    keyword: &str,
    vr: DicomVr,
    value: u64,
) -> Result<AttributeOperation, EnhancedPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    })
}

fn set_tag(keyword: &str, target: &str) -> Result<AttributeOperation, EnhancedPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr: DicomVr::AT,
        value: AttributeValue::Primitive(PrimitiveValue::Tag(address(target)?)),
    })
}

fn set_fd(keyword: &str, value: f64) -> Result<AttributeOperation, EnhancedPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr: DicomVr::FD,
        value: AttributeValue::Primitive(PrimitiveValue::Float64Bits(value.to_bits())),
    })
}

fn set_fd_multi(keyword: &str, values: &[f64]) -> Result<AttributeOperation, EnhancedPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr: DicomVr::FD,
        value: AttributeValue::Multi(
            values
                .iter()
                .map(|value| PrimitiveValue::Float64Bits(value.to_bits()))
                .collect(),
        ),
    })
}

fn address(keyword: &str) -> Result<AttributeAddress, EnhancedPlanError> {
    AttributeAddress::from_keyword(keyword)
        .map_err(|error| EnhancedPlanError::Attribute(error.to_string()))
}

#[derive(Debug)]
pub enum EnhancedPlanError {
    InvalidStandardsLockHash,
    WrongFamily,
    RecipeIdentityMismatch,
    ZeroGeometry,
    EmptyFrames,
    FrameCardinality,
    InvalidConcatenation,
    InvalidDimensionIndex,
    ResourceOverflow,
    Pixels(String),
    Attribute(String),
    Identity(String),
    Template(String),
    CorpusPlan(crate::corpus_plan::CorpusPlanError),
    Contract(AdvancedProviderContractError),
}

impl fmt::Display for EnhancedPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for EnhancedPlanError {}
