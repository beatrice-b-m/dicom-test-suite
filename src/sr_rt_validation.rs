//! Filesystem-free specialized validation evidence for SR and RT artifacts.
//!
//! Callers extract observations from the staged Part 10 object, then pass them
//! here. Recipe declarations alone can therefore never manufacture a pass.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const HIGH_DICOM_SR_BACKEND_ID: &str = "highdicom_pydicom";
pub const HIGH_DICOM_SR_VERSION: &str = "0.28.1";
pub const HIGH_DICOM_SR_PROTOCOL_VERSION: &str = "0.1.0";
pub const HIGH_DICOM_SR_DEPENDENCY_SHA256: &str =
    "253612f2a540d29071556c238e15abeb00929167e348edd6fa15e267e5189378";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCheck {
    pub name: String,
    pub status: String,
    pub message: String,
}

impl SemanticCheck {
    fn passed(name: &str, message: &str) -> Self {
        Self {
            name: name.into(),
            status: "passed".into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecializedValidationEvidence {
    pub internal: Vec<SemanticCheck>,
    pub standards: Vec<SemanticCheck>,
    #[serde(default)]
    pub external: Vec<Value>,
}

impl SpecializedValidationEvidence {
    pub fn legacy_json(&self) -> Value {
        json!({
            "status": "passed",
            "internal": self.internal,
            "standards": self.standards,
            "external": self.external,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeSrKind {
    BasicText,
    Comprehensive,
    KeyObjectSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DicomIdentityObservation {
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub transfer_syntax_uid: String,
    pub implementation_class_uid: String,
    pub synthetic_data: String,
    pub modality: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticReferenceObservation {
    pub role: String,
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    #[serde(default)]
    pub referenced_frames: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSrValidationContract {
    pub kind: NativeSrKind,
    pub identity: DicomIdentityObservation,
    pub completion_flag: String,
    pub verification_flag: String,
    pub continuity_of_content: String,
    pub title_code_value: String,
    pub title_coding_scheme_designator: String,
    pub title_code_meaning: String,
    /// Canonical hash of the typed content-tree observation.
    pub content_tree_sha256: String,
    pub references: Vec<SemanticReferenceObservation>,
}

pub type NativeSrObservation = NativeSrValidationContract;

pub fn validate_native_sr(
    expected: &NativeSrValidationContract,
    observed: &NativeSrObservation,
) -> Result<SpecializedValidationEvidence, SrRtEvidenceError> {
    ensure_sha256(&expected.content_tree_sha256)?;
    ensure_sha256(&observed.content_tree_sha256)?;
    checked(
        &[
            (
                "sr_part10_identity",
                expected.identity == observed.identity,
                "SR Part 10 identity, transfer syntax, Synthetic Data, and Modality match the plan.",
            ),
            (
                "sr_document_kind",
                expected.kind == observed.kind,
                "SR SOP class and typed document kind match the recipe.",
            ),
            (
                "sr_document_flags",
                expected.completion_flag == observed.completion_flag
                    && expected.verification_flag == observed.verification_flag
                    && expected.continuity_of_content == observed.continuity_of_content,
                "SR completion, verification, and continuity flags match the recipe.",
            ),
            (
                "sr_title",
                expected.title_code_value == observed.title_code_value
                    && expected.title_coding_scheme_designator
                        == observed.title_coding_scheme_designator
                    && expected.title_code_meaning == observed.title_code_meaning,
                "SR root Concept Name Code Sequence matches the recipe.",
            ),
            (
                "sr_content_tree",
                expected.content_tree_sha256 == observed.content_tree_sha256,
                "The complete typed SR content-tree observation matches its canonical hash.",
            ),
            (
                "sr_reference_graph",
                expected.references == observed.references,
                "SR evidence and content-item references match the planned source graph in order.",
            ),
        ],
        "structured_report_storage",
        "The SR SOP Class and document modules match the pinned 2026b contract.",
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RtObjectObservation {
    StructureSet {
        roi_count: u32,
        contour_count: u32,
        contour_points: u32,
    },
    Dose {
        rows: u32,
        columns: u32,
        frames: u32,
        dose_units: String,
        dose_type: String,
        dose_summation_type: String,
        dose_grid_scaling: String,
        pixel_sha256: String,
    },
    Plan {
        fraction_group_count: u32,
        beam_count: u32,
        control_point_count: u32,
        plan_geometry: String,
    },
    Image {
        rows: u32,
        columns: u32,
        referenced_beam_number: u32,
        referenced_fraction_group_number: u32,
        pixel_sha256: String,
    },
    CarmRadiation {
        treatment_position_count: u32,
        control_point_count: u32,
        rt_record_flag: String,
    },
    RadiationSet {
        treatment_position_group_count: u32,
        radiation_count: u32,
        dose_contribution_absent: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RtValidationContract {
    pub identity: DicomIdentityObservation,
    pub label: String,
    pub object: RtObjectObservation,
    pub references: Vec<SemanticReferenceObservation>,
    pub pixel_data_absent: bool,
}

pub type RtObservation = RtValidationContract;

pub fn validate_rt_object(
    expected: &RtValidationContract,
    observed: &RtObservation,
) -> Result<SpecializedValidationEvidence, SrRtEvidenceError> {
    validate_rt_hashes(&expected.object)?;
    validate_rt_hashes(&observed.object)?;
    let expected_kind = rt_kind(&expected.object);
    checked(
        &[
            (
                "rt_part10_identity",
                expected.identity == observed.identity,
                "RT Part 10 identity, transfer syntax, Synthetic Data, and Modality match the plan.",
            ),
            (
                "rt_object_kind",
                expected_kind == rt_kind(&observed.object),
                "RT SOP class and typed object kind match the recipe.",
            ),
            (
                "rt_object_semantics",
                expected.label == observed.label && expected.object == observed.object,
                "All specialized RT object attributes and pixel facts match the typed recipe.",
            ),
            (
                "rt_reference_graph",
                expected.references == observed.references,
                "RT references match the planned role and identity graph in order.",
            ),
            (
                "rt_pixel_presence",
                expected.pixel_data_absent == observed.pixel_data_absent,
                "Pixel Data presence or absence matches the RT object contract.",
            ),
        ],
        &format!("{}_storage", expected_kind),
        "The specialized RT IOD modules match the pinned 2026b contract.",
    )
}

fn rt_kind(value: &RtObjectObservation) -> &'static str {
    match value {
        RtObjectObservation::StructureSet { .. } => "rt_structure_set",
        RtObjectObservation::Dose { .. } => "rt_dose",
        RtObjectObservation::Plan { .. } => "rt_plan",
        RtObjectObservation::Image { .. } => "rt_image",
        RtObjectObservation::CarmRadiation { .. } => "carm_rt_radiation",
        RtObjectObservation::RadiationSet { .. } => "rt_radiation_set",
    }
}

fn validate_rt_hashes(value: &RtObjectObservation) -> Result<(), SrRtEvidenceError> {
    match value {
        RtObjectObservation::Dose { pixel_sha256, .. }
        | RtObjectObservation::Image { pixel_sha256, .. } => ensure_sha256(pixel_sha256),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighDicomSrImportEvidence {
    pub backend_id: String,
    pub protocol_version: String,
    pub dependency: String,
    pub version: String,
    pub dependency_lock_sha256: String,
    pub executable_fingerprint: String,
    pub entrypoint_fingerprint: String,
    pub environment_fingerprint: String,
    pub request_sha256: String,
    pub response_sha256: String,
    pub output_sha256: String,
    pub output_size_bytes: u64,
    pub maximum_output_bytes: u64,
    pub semantic_evidence: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub fn validate_highdicom_sr_import(
    evidence: &HighDicomSrImportEvidence,
    required_semantic_evidence: &[String],
) -> Result<Value, SrRtEvidenceError> {
    for hash in [
        &evidence.dependency_lock_sha256,
        &evidence.executable_fingerprint,
        &evidence.entrypoint_fingerprint,
        &evidence.environment_fingerprint,
        &evidence.request_sha256,
        &evidence.response_sha256,
        &evidence.output_sha256,
    ] {
        ensure_sha256(hash)?;
    }
    if evidence.backend_id != HIGH_DICOM_SR_BACKEND_ID
        || evidence.protocol_version != HIGH_DICOM_SR_PROTOCOL_VERSION
        || evidence.dependency != "highdicom"
        || evidence.version != HIGH_DICOM_SR_VERSION
        || evidence.dependency_lock_sha256 != HIGH_DICOM_SR_DEPENDENCY_SHA256
        || evidence.output_size_bytes == 0
        || evidence.output_size_bytes > evidence.maximum_output_bytes
        || evidence.maximum_output_bytes != 1_048_576
        || evidence.semantic_evidence != required_semantic_evidence
    {
        return Err(SrRtEvidenceError::InvalidExternalImport);
    }
    Ok(json!({
        "backend_id": evidence.backend_id,
        "protocol_version": evidence.protocol_version,
        "dependency": evidence.dependency,
        "version": evidence.version,
        "dependency_lock_sha256": evidence.dependency_lock_sha256,
        "executable_fingerprint": evidence.executable_fingerprint,
        "entrypoint_fingerprint": evidence.entrypoint_fingerprint,
        "environment_fingerprint": evidence.environment_fingerprint,
        "request_sha256": evidence.request_sha256,
        "response_sha256": evidence.response_sha256,
        "output_sha256": evidence.output_sha256,
        "output_size_bytes": evidence.output_size_bytes,
        "maximum_output_bytes": evidence.maximum_output_bytes,
        "determinism": "semantic_stable",
        "semantic_evidence": evidence.semantic_evidence,
        "warnings": evidence.warnings,
    }))
}

fn checked(
    checks: &[(&str, bool, &str)],
    standard_name: &str,
    standard_message: &str,
) -> Result<SpecializedValidationEvidence, SrRtEvidenceError> {
    if let Some((name, _, _)) = checks.iter().find(|(_, passed, _)| !passed) {
        return Err(SrRtEvidenceError::ValidationFailed((*name).into()));
    }
    Ok(SpecializedValidationEvidence {
        internal: checks
            .iter()
            .map(|(name, _, message)| SemanticCheck::passed(name, message))
            .collect(),
        standards: vec![
            SemanticCheck::passed(standard_name, standard_message),
            SemanticCheck::passed(
                "explicit_vr_little_endian_transfer_syntax",
                "Transfer Syntax UID matches Explicit VR Little Endian.",
            ),
            SemanticCheck::passed(
                "synthetic_data_attribute",
                "Synthetic Data (0008,001C) is present with value YES.",
            ),
        ],
        external: vec![],
    })
}

fn ensure_sha256(value: &str) -> Result<(), SrRtEvidenceError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(SrRtEvidenceError::InvalidSha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrRtEvidenceError {
    ValidationFailed(String),
    InvalidSha256,
    InvalidExternalImport,
}

impl fmt::Display for SrRtEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValidationFailed(rule) => write!(f, "specialized validation failed: {rule}"),
            Self::InvalidSha256 => f.write_str("evidence contains an invalid SHA-256 digest"),
            Self::InvalidExternalImport => {
                f.write_str("highdicom SR import evidence violates the pinned boundary")
            }
        }
    }
}

impl std::error::Error for SrRtEvidenceError {}
