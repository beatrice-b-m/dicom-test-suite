//! Executable transfer-syntax/backend capability registry.
//!
//! This module is deliberately independent of generation frontends. It binds a
//! transfer syntax to one exact execution backend and the requirements which
//! must be satisfied before a codec request can be scheduled.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::Deserialize;

pub const CAPABILITY_MATRIX_JSON: &str =
    include_str!("../../transfer-syntax/capability-matrix.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendBoundary {
    DatasetWriter,
    EncodedFrames,
    LockedFullFileTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendAvailability {
    BuiltIn,
    FeatureGated,
    FeatureAndToolGated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendDeterminism {
    ByteStable,
    SemanticStable,
}

impl BackendDeterminism {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ByteStable => "byte_stable",
            Self::SemanticStable => "semantic_stable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceShape {
    Dataset,
    NativeFrames {
        samples_per_pixel: &'static [u16],
        bits_allocated: &'static [u16],
        photometric_interpretations: &'static [&'static str],
    },
    FullPart10 {
        source_transfer_syntax_uid: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CodecEvidenceRequirement {
    Part10TransferSyntax,
    EncapsulationLayout,
    ExactDecodedFrameHashes,
    LossySampleMetrics,
    RuntimeVersion,
    ExecutableSha256,
    ByteReproducibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecBackendDescriptor {
    pub transfer_syntax_uid: &'static str,
    pub backend_id: &'static str,
    pub boundary: BackendBoundary,
    pub decode_pixel: bool,
    pub availability: BackendAvailability,
    pub feature_gate: Option<&'static str>,
    pub external_tool: Option<&'static str>,
    pub determinism: BackendDeterminism,
    pub source_shape: SourceShape,
    pub evidence: &'static [CodecEvidenceRequirement],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecSourceRequest<'a> {
    Dataset,
    NativeFrame {
        samples_per_pixel: u16,
        bits_allocated: u16,
        photometric_interpretation: &'a str,
    },
    FullPart10 {
        transfer_syntax_uid: &'a str,
    },
}

#[derive(Debug)]
pub struct CodecDispatchRequest<'a> {
    pub transfer_syntax_uid: &'a str,
    pub backend_id: &'a str,
    pub enabled_features: &'a BTreeSet<String>,
    pub available_tools: &'a BTreeSet<String>,
    pub source: CodecSourceRequest<'a>,
}

const DATASET_EVIDENCE: &[CodecEvidenceRequirement] = &[
    CodecEvidenceRequirement::Part10TransferSyntax,
    CodecEvidenceRequirement::ByteReproducibility,
];
const LOSSLESS_FRAME_EVIDENCE: &[CodecEvidenceRequirement] = &[
    CodecEvidenceRequirement::Part10TransferSyntax,
    CodecEvidenceRequirement::EncapsulationLayout,
    CodecEvidenceRequirement::ExactDecodedFrameHashes,
];
const LOSSY_FRAME_EVIDENCE: &[CodecEvidenceRequirement] = &[
    CodecEvidenceRequirement::Part10TransferSyntax,
    CodecEvidenceRequirement::EncapsulationLayout,
    CodecEvidenceRequirement::LossySampleMetrics,
];
const EXTERNAL_LOSSLESS_EVIDENCE: &[CodecEvidenceRequirement] = &[
    CodecEvidenceRequirement::Part10TransferSyntax,
    CodecEvidenceRequirement::EncapsulationLayout,
    CodecEvidenceRequirement::ExactDecodedFrameHashes,
    CodecEvidenceRequirement::RuntimeVersion,
    CodecEvidenceRequirement::ExecutableSha256,
];
const EXTERNAL_LOSSY_EVIDENCE: &[CodecEvidenceRequirement] = &[
    CodecEvidenceRequirement::Part10TransferSyntax,
    CodecEvidenceRequirement::EncapsulationLayout,
    CodecEvidenceRequirement::LossySampleMetrics,
    CodecEvidenceRequirement::RuntimeVersion,
    CodecEvidenceRequirement::ExecutableSha256,
];
const EXTERNAL_FILE_EVIDENCE: &[CodecEvidenceRequirement] = &[
    CodecEvidenceRequirement::Part10TransferSyntax,
    CodecEvidenceRequirement::EncapsulationLayout,
    CodecEvidenceRequirement::ExactDecodedFrameHashes,
    CodecEvidenceRequirement::RuntimeVersion,
    CodecEvidenceRequirement::ExecutableSha256,
    CodecEvidenceRequirement::ByteReproducibility,
];

const MONO_8: SourceShape = SourceShape::NativeFrames {
    samples_per_pixel: &[1],
    bits_allocated: &[8],
    photometric_interpretations: &["MONOCHROME2"],
};
const MONO_16: SourceShape = SourceShape::NativeFrames {
    samples_per_pixel: &[1],
    bits_allocated: &[16],
    photometric_interpretations: &["MONOCHROME2"],
};
const RGB_8: SourceShape = SourceShape::NativeFrames {
    samples_per_pixel: &[3],
    bits_allocated: &[8],
    photometric_interpretations: &["RGB"],
};
const RLE_SOURCE: SourceShape = SourceShape::NativeFrames {
    samples_per_pixel: &[1, 3],
    bits_allocated: &[8, 16],
    photometric_interpretations: &[
        "MONOCHROME1",
        "MONOCHROME2",
        "RGB",
        "YBR_FULL",
        "PALETTE COLOR",
    ],
};
const DEFLATED_FRAME_SOURCE: SourceShape = SourceShape::NativeFrames {
    samples_per_pixel: &[1],
    bits_allocated: &[1, 8, 16],
    photometric_interpretations: &["MONOCHROME2"],
};

pub const BACKENDS: &[CodecBackendDescriptor] = &[
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2",
        backend_id: "dicom-rs.part10",
        boundary: BackendBoundary::DatasetWriter,
        decode_pixel: false,
        availability: BackendAvailability::BuiltIn,
        feature_gate: None,
        external_tool: None,
        determinism: BackendDeterminism::ByteStable,
        source_shape: SourceShape::Dataset,
        evidence: DATASET_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        backend_id: "dicom-rs.part10",
        boundary: BackendBoundary::DatasetWriter,
        decode_pixel: true,
        availability: BackendAvailability::BuiltIn,
        feature_gate: None,
        external_tool: None,
        determinism: BackendDeterminism::ByteStable,
        source_shape: SourceShape::Dataset,
        evidence: DATASET_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.2",
        backend_id: "encoding.native.explicit_vr_big_endian",
        boundary: BackendBoundary::DatasetWriter,
        decode_pixel: true,
        availability: BackendAvailability::BuiltIn,
        feature_gate: None,
        external_tool: None,
        determinism: BackendDeterminism::ByteStable,
        source_shape: SourceShape::Dataset,
        evidence: DATASET_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.1.99",
        backend_id: "dicom_rs_deflated_dataset_writer",
        boundary: BackendBoundary::DatasetWriter,
        decode_pixel: true,
        availability: BackendAvailability::FeatureGated,
        feature_gate: Some("deflate"),
        external_tool: None,
        determinism: BackendDeterminism::ByteStable,
        source_shape: SourceShape::Dataset,
        evidence: DATASET_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.50",
        backend_id: "dicom_rs_jpeg_baseline_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureGated,
        feature_gate: Some("jpeg"),
        external_tool: None,
        determinism: BackendDeterminism::SemanticStable,
        source_shape: RGB_8,
        evidence: LOSSY_FRAME_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.80",
        backend_id: "dicom_rs_charls_jpeg_ls_lossless_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureGated,
        feature_gate: Some("charls"),
        external_tool: None,
        determinism: BackendDeterminism::SemanticStable,
        source_shape: MONO_8,
        evidence: LOSSLESS_FRAME_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.90",
        backend_id: "project_openjp2_jpeg2000_lossless_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureGated,
        feature_gate: Some("jpeg2000"),
        external_tool: None,
        determinism: BackendDeterminism::SemanticStable,
        source_shape: MONO_16,
        evidence: LOSSLESS_FRAME_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.110",
        backend_id: "dicom_rs_jpegxl_lossless_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureGated,
        feature_gate: Some("jpegxl"),
        external_tool: None,
        determinism: BackendDeterminism::SemanticStable,
        source_shape: RGB_8,
        evidence: LOSSLESS_FRAME_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.112",
        backend_id: "cjxl_jpegxl_lossy_command_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureAndToolGated,
        feature_gate: Some("jpegxl"),
        external_tool: Some("cjxl"),
        determinism: BackendDeterminism::SemanticStable,
        source_shape: RGB_8,
        evidence: EXTERNAL_LOSSY_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.201",
        backend_id: "openjph_htj2k_lossless_command_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureAndToolGated,
        feature_gate: Some("htj2k_openjph"),
        external_tool: Some("ojph_compress"),
        determinism: BackendDeterminism::SemanticStable,
        source_shape: MONO_16,
        evidence: EXTERNAL_LOSSLESS_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.203",
        backend_id: "openjph_htj2k_lossy_command_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureAndToolGated,
        feature_gate: Some("htj2k_openjph"),
        external_tool: Some("ojph_compress"),
        determinism: BackendDeterminism::SemanticStable,
        source_shape: MONO_16,
        evidence: EXTERNAL_LOSSY_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.57",
        backend_id: "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
        boundary: BackendBoundary::LockedFullFileTransform,
        decode_pixel: true,
        availability: BackendAvailability::FeatureAndToolGated,
        feature_gate: Some("legacy_jpeg_dcmtk"),
        external_tool: Some("dcmcjpeg"),
        determinism: BackendDeterminism::SemanticStable,
        source_shape: SourceShape::FullPart10 {
            source_transfer_syntax_uid: "1.2.840.10008.1.2.1",
        },
        evidence: EXTERNAL_FILE_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.70",
        backend_id: "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
        boundary: BackendBoundary::LockedFullFileTransform,
        decode_pixel: true,
        availability: BackendAvailability::FeatureAndToolGated,
        feature_gate: Some("legacy_jpeg_dcmtk"),
        external_tool: Some("dcmcjpeg"),
        determinism: BackendDeterminism::SemanticStable,
        source_shape: SourceShape::FullPart10 {
            source_transfer_syntax_uid: "1.2.840.10008.1.2.1",
        },
        evidence: EXTERNAL_FILE_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.5",
        backend_id: "native_project_rle_encoder",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: false,
        availability: BackendAvailability::BuiltIn,
        feature_gate: None,
        external_tool: None,
        determinism: BackendDeterminism::ByteStable,
        source_shape: RLE_SOURCE,
        evidence: LOSSLESS_FRAME_EVIDENCE,
    },
    CodecBackendDescriptor {
        transfer_syntax_uid: "1.2.840.10008.1.2.8.1",
        backend_id: "dicom_rs_deflated_image_frame_writer",
        boundary: BackendBoundary::EncodedFrames,
        decode_pixel: true,
        availability: BackendAvailability::FeatureGated,
        feature_gate: Some("deflate"),
        external_tool: None,
        determinism: BackendDeterminism::ByteStable,
        source_shape: DEFLATED_FRAME_SOURCE,
        evidence: LOSSLESS_FRAME_EVIDENCE,
    },
];

#[derive(Debug)]
pub struct TransferSyntaxBackendRegistry {
    by_transfer_syntax: BTreeMap<&'static str, &'static CodecBackendDescriptor>,
    by_backend_id: BTreeMap<&'static str, Vec<&'static CodecBackendDescriptor>>,
}

impl TransferSyntaxBackendRegistry {
    pub fn load_committed() -> Result<Self, CodecRegistryError> {
        Self::from_capability_matrix(CAPABILITY_MATRIX_JSON)
    }

    pub fn from_capability_matrix(json: &str) -> Result<Self, CodecRegistryError> {
        let matrix: CapabilityMatrix = serde_json::from_str(json)
            .map_err(|error| CodecRegistryError::MatrixParse(error.to_string()))?;
        let mut entries = BTreeMap::new();
        for entry in matrix.entries {
            let uid = entry.uid.clone();
            if entries.insert(uid.clone(), entry).is_some() {
                return Err(CodecRegistryError::MatrixMismatch(format!(
                    "duplicate transfer syntax {uid}"
                )));
            }
        }
        let mut by_transfer_syntax = BTreeMap::new();
        let mut by_backend_id = BTreeMap::<_, Vec<_>>::new();
        for backend in BACKENDS {
            if by_transfer_syntax
                .insert(backend.transfer_syntax_uid, backend)
                .is_some()
            {
                return Err(CodecRegistryError::DuplicateTransferSyntax(
                    backend.transfer_syntax_uid.into(),
                ));
            }
            by_backend_id
                .entry(backend.backend_id)
                .or_default()
                .push(backend);
            validate_matrix_entry(backend, entries.get(backend.transfer_syntax_uid))?;
        }
        Ok(Self {
            by_transfer_syntax,
            by_backend_id,
        })
    }

    pub fn for_transfer_syntax(&self, uid: &str) -> Option<&'static CodecBackendDescriptor> {
        self.by_transfer_syntax.get(uid).copied()
    }

    pub fn for_backend_id(&self, backend_id: &str) -> &[&'static CodecBackendDescriptor] {
        self.by_backend_id
            .get(backend_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn resolve(
        &self,
        request: CodecDispatchRequest<'_>,
    ) -> Result<&'static CodecBackendDescriptor, CodecRegistryError> {
        let backend = self
            .for_transfer_syntax(request.transfer_syntax_uid)
            .ok_or_else(|| {
                CodecRegistryError::UnregisteredTransferSyntax(request.transfer_syntax_uid.into())
            })?;
        if backend.backend_id != request.backend_id {
            return Err(CodecRegistryError::BackendMismatch {
                transfer_syntax_uid: request.transfer_syntax_uid.into(),
                expected: backend.backend_id.into(),
                actual: request.backend_id.into(),
            });
        }
        if let Some(feature) = backend.feature_gate {
            if !request.enabled_features.contains(feature) {
                return Err(CodecRegistryError::MissingFeature(feature.into()));
            }
        }
        if let Some(tool) = backend.external_tool {
            if !request.available_tools.contains(tool) {
                return Err(CodecRegistryError::MissingTool(tool.into()));
            }
        }
        if !source_supported(backend.source_shape, request.source) {
            return Err(CodecRegistryError::UnsupportedSourceShape {
                backend_id: backend.backend_id.into(),
                source: format!("{:?}", request.source),
            });
        }
        Ok(backend)
    }

    pub fn validate_registry_requirements(
        &self,
        transfer_syntax_uid: &str,
        determinism: &str,
        features: &[String],
        external_codecs: &[String],
    ) -> Result<&'static CodecBackendDescriptor, CodecRegistryError> {
        let backend = self
            .for_transfer_syntax(transfer_syntax_uid)
            .ok_or_else(|| {
                CodecRegistryError::UnregisteredTransferSyntax(transfer_syntax_uid.into())
            })?;
        if backend.determinism.as_str() != determinism {
            return Err(CodecRegistryError::RegistryMismatch(format!(
                "{transfer_syntax_uid}: determinism is {determinism}, expected {}",
                backend.determinism.as_str()
            )));
        }
        let expected_features = backend.feature_gate.into_iter().collect::<BTreeSet<_>>();
        let actual_features = features.iter().map(String::as_str).collect::<BTreeSet<_>>();
        if expected_features != actual_features {
            return Err(CodecRegistryError::RegistryMismatch(format!(
                "{transfer_syntax_uid}: features {actual_features:?}, expected {expected_features:?}"
            )));
        }
        if let Some(tool) = backend.external_tool {
            if !external_codecs
                .iter()
                .any(|value| external_requirement_matches(tool, value))
            {
                return Err(CodecRegistryError::RegistryMismatch(format!(
                    "{transfer_syntax_uid}: missing external codec requirement for {tool}"
                )));
            }
        }
        Ok(backend)
    }
}

pub fn encoding_provider_matches(
    backend: &CodecBackendDescriptor,
    encoding_provider_id: &str,
) -> bool {
    backend.backend_id == encoding_provider_id
        || recipe_encoding_provider_id(backend.backend_id) == Some(encoding_provider_id)
        || matches!(
            (backend.transfer_syntax_uid, encoding_provider_id),
            ("1.2.840.10008.1.2.5", "encoding.native.rle_lossless")
        )
}

/// Qualified recipe-facing ID for an exact executable backend.
pub fn recipe_encoding_provider_id(backend_id: &str) -> Option<&'static str> {
    Some(match backend_id {
        "dicom_rs_deflated_dataset_writer" => "encoding.dicom_rs.deflated_dataset",
        "dicom_rs_jpeg_baseline_writer" => "encoding.dicom_rs.jpeg_baseline",
        "dicom_rs_charls_jpeg_ls_lossless_writer" => "encoding.dicom_rs.jpeg_ls_lossless",
        "project_openjp2_jpeg2000_lossless_writer" => "encoding.openjp2.jpeg2000_lossless",
        "dicom_rs_jpegxl_lossless_writer" => "encoding.dicom_rs.jpegxl_lossless",
        "cjxl_jpegxl_lossy_command_writer" => "encoding.cjxl.jpegxl_lossy",
        "openjph_htj2k_lossless_command_writer" => "encoding.openjph.htj2k_lossless",
        "openjph_htj2k_lossy_command_writer" => "encoding.openjph.htj2k_lossy",
        "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer" => {
            "encoding.dcmtk.jpeg_lossless_process_14"
        }
        "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer" => "encoding.dcmtk.jpeg_lossless_sv1",
        _ => return None,
    })
}

fn external_requirement_matches(tool: &str, requirement: &str) -> bool {
    match tool {
        "ojph_compress" => requirement.starts_with("OpenJPH"),
        "dcmcjpeg" => requirement.starts_with("DCMTK"),
        "cjxl" => requirement.starts_with("cjxl"),
        _ => requirement.starts_with(tool),
    }
}

fn source_supported(expected: SourceShape, actual: CodecSourceRequest<'_>) -> bool {
    match (expected, actual) {
        (SourceShape::Dataset, CodecSourceRequest::Dataset) => true,
        (
            SourceShape::FullPart10 {
                source_transfer_syntax_uid: expected,
            },
            CodecSourceRequest::FullPart10 {
                transfer_syntax_uid: actual,
            },
        ) => expected == actual,
        (
            SourceShape::NativeFrames {
                samples_per_pixel,
                bits_allocated,
                photometric_interpretations,
            },
            CodecSourceRequest::NativeFrame {
                samples_per_pixel: samples,
                bits_allocated: bits,
                photometric_interpretation: photometric,
            },
        ) => {
            samples_per_pixel.contains(&samples)
                && bits_allocated.contains(&bits)
                && photometric_interpretations.contains(&photometric)
        }
        _ => false,
    }
}

fn validate_matrix_entry(
    backend: &CodecBackendDescriptor,
    entry: Option<&CapabilityMatrixEntry>,
) -> Result<(), CodecRegistryError> {
    let entry = entry.ok_or_else(|| {
        CodecRegistryError::MatrixMismatch(format!(
            "{} ({}) is absent from capability matrix",
            backend.transfer_syntax_uid, backend.backend_id
        ))
    })?;
    if !matches!(entry.status.as_str(), "available" | "feature_gated") {
        return Err(CodecRegistryError::MatrixMismatch(format!(
            "{} is executable but matrix status is {}",
            entry.uid, entry.status
        )));
    }
    if !entry.read_dataset || !entry.write_dataset || !entry.encode_pixel {
        return Err(CodecRegistryError::MatrixMismatch(format!(
            "{} does not claim required read/write/encode capabilities",
            entry.uid
        )));
    }
    if entry.decode_pixel != backend.decode_pixel {
        return Err(CodecRegistryError::MatrixMismatch(format!(
            "{} decode capability {} does not match backend declaration {}",
            entry.uid, entry.decode_pixel, backend.decode_pixel
        )));
    }
    if entry.determinism != backend.determinism.as_str() {
        return Err(CodecRegistryError::MatrixMismatch(format!(
            "{} determinism {} does not match backend {}",
            entry.uid,
            entry.determinism,
            backend.determinism.as_str()
        )));
    }
    let expected_features = backend.feature_gate.into_iter().collect::<BTreeSet<_>>();
    let actual_features = entry
        .feature_flags
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if expected_features != actual_features {
        return Err(CodecRegistryError::MatrixMismatch(format!(
            "{} feature flags {actual_features:?} do not match {expected_features:?}",
            entry.uid
        )));
    }
    if let Some(tool) = backend.external_tool {
        if !entry
            .external_libraries
            .iter()
            .any(|value| external_requirement_matches(tool, value))
        {
            return Err(CodecRegistryError::MatrixMismatch(format!(
                "{} does not name the external tool for backend {}",
                entry.uid, backend.backend_id
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CapabilityMatrix {
    entries: Vec<CapabilityMatrixEntry>,
}

#[derive(Debug, Deserialize)]
struct CapabilityMatrixEntry {
    uid: String,
    status: String,
    read_dataset: bool,
    decode_pixel: bool,
    write_dataset: bool,
    encode_pixel: bool,
    feature_flags: Vec<String>,
    external_libraries: Vec<String>,
    determinism: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecRegistryError {
    MatrixParse(String),
    MatrixMismatch(String),
    RegistryMismatch(String),
    DuplicateTransferSyntax(String),
    UnregisteredTransferSyntax(String),
    BackendMismatch {
        transfer_syntax_uid: String,
        expected: String,
        actual: String,
    },
    MissingFeature(String),
    MissingTool(String),
    UnsupportedSourceShape {
        backend_id: String,
        source: String,
    },
}

impl fmt::Display for CodecRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatrixParse(message) => write!(formatter, "invalid capability matrix: {message}"),
            Self::MatrixMismatch(message) => {
                write!(formatter, "capability matrix mismatch: {message}")
            }
            Self::RegistryMismatch(message) => {
                write!(formatter, "registry codec mismatch: {message}")
            }
            Self::DuplicateTransferSyntax(uid) => {
                write!(formatter, "duplicate executable transfer syntax {uid}")
            }
            Self::UnregisteredTransferSyntax(uid) => {
                write!(formatter, "no executable backend for transfer syntax {uid}")
            }
            Self::BackendMismatch {
                transfer_syntax_uid,
                expected,
                actual,
            } => write!(
                formatter,
                "transfer syntax {transfer_syntax_uid} requires backend {expected}, not {actual}"
            ),
            Self::MissingFeature(feature) => {
                write!(formatter, "required feature {feature} is disabled")
            }
            Self::MissingTool(tool) => {
                write!(formatter, "required external tool {tool} is unavailable")
            }
            Self::UnsupportedSourceShape { backend_id, source } => {
                write!(
                    formatter,
                    "backend {backend_id} does not support source shape {source}"
                )
            }
        }
    }
}

impl Error for CodecRegistryError {}
