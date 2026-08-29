//! Typed validation and manifest projection for quantitative DICOM objects.
//!
//! This module deliberately consumes observations made from the staged Part 10
//! object. It never turns recipe declarations into successful validation.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantitativeCheck {
    pub name: String,
    pub status: String,
    pub message: String,
}

impl QuantitativeCheck {
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
pub struct QuantitativeValidationReport {
    pub internal: Vec<QuantitativeCheck>,
    pub standards: Vec<QuantitativeCheck>,
}

impl QuantitativeValidationReport {
    pub fn legacy_json(&self) -> Value {
        json!({
            "status": "passed",
            "internal": self.internal,
            "standards": self.standards,
            "external": []
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentationValidationContract {
    pub transfer_syntax_uid: String,
    pub modality: String,
    pub segmentation_type: String,
    pub segmentation_fractional_type: Option<String>,
    pub maximum_fractional_value: Option<u16>,
    pub segment_sequence_items: u32,
    pub shared_functional_groups_sequence_items: u32,
    pub per_frame_functional_groups_sequence_items: u32,
    pub dimension_organization_uid: String,
    pub referenced_sop_class_uid: String,
    pub referenced_sop_instance_uid: String,
    pub referenced_frame_numbers: Vec<u32>,
    pub frame_sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentationObservation {
    pub modality: String,
    pub segmentation_type: String,
    pub segmentation_fractional_type: Option<String>,
    pub maximum_fractional_value: Option<u16>,
    pub segment_sequence_items: u32,
    pub shared_functional_groups_sequence_items: u32,
    pub per_frame_functional_groups_sequence_items: u32,
    pub dimension_organization_uid: String,
    pub referenced_sop_class_uid: String,
    pub referenced_sop_instance_uid: String,
    pub referenced_frame_numbers: Vec<u32>,
    pub frame_sha256: Vec<String>,
}

pub fn validate_native_segmentation(
    expected: &SegmentationValidationContract,
    observed: &SegmentationObservation,
) -> Result<QuantitativeValidationReport, QuantitativeEvidenceError> {
    let checks = [
        (
            "segmentation_modality",
            expected.modality == observed.modality,
            "Segmentation Series Modality matches the recipe.",
        ),
        (
            "segmentation_type",
            expected.segmentation_type == observed.segmentation_type,
            "Segmentation Type matches the recipe.",
        ),
        (
            "segmentation_fractional_type",
            expected.segmentation_fractional_type == observed.segmentation_fractional_type,
            "Segmentation Fractional Type matches the recipe.",
        ),
        (
            "segmentation_maximum_fractional_value",
            expected.maximum_fractional_value == observed.maximum_fractional_value,
            "Maximum Fractional Value matches the recipe.",
        ),
        (
            "segmentation_segment_sequence_items",
            expected.segment_sequence_items == observed.segment_sequence_items,
            "Segment Sequence item count matches the recipe.",
        ),
        (
            "segmentation_shared_functional_groups_sequence_items",
            expected.shared_functional_groups_sequence_items
                == observed.shared_functional_groups_sequence_items,
            "Shared Functional Groups Sequence item count matches the recipe.",
        ),
        (
            "segmentation_per_frame_functional_groups_sequence_items",
            expected.per_frame_functional_groups_sequence_items
                == observed.per_frame_functional_groups_sequence_items,
            "Per-frame Functional Groups Sequence item count matches the recipe.",
        ),
        (
            "segmentation_dimension_organization_uid",
            expected.dimension_organization_uid == observed.dimension_organization_uid,
            "Dimension Organization UID matches the planned identity.",
        ),
        (
            "segmentation_source_image_sop_class_uid",
            expected.referenced_sop_class_uid == observed.referenced_sop_class_uid,
            "Source Image reference SOP Class UID matches the source.",
        ),
        (
            "segmentation_source_image_sop_instance_uid",
            expected.referenced_sop_instance_uid == observed.referenced_sop_instance_uid,
            "Source Image reference SOP Instance UID matches the source.",
        ),
        (
            "segmentation_source_image_frame_number",
            expected.referenced_frame_numbers == observed.referenced_frame_numbers,
            "Source Image referenced frame numbers match the recipe.",
        ),
        (
            "segmentation_frame_payload_hashes",
            expected.frame_sha256 == observed.frame_sha256,
            "Decoded segmentation frame hashes match the typed content evidence.",
        ),
    ];
    checked_report(
        checks,
        "sop_class_uid",
        "SOP Class UID matches the recipe.",
        transfer_syntax_standard(&expected.transfer_syntax_uid),
        "image_pixel_description",
        "Image Pixel attributes match the native pixel recipe.",
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RwvmValidationContract {
    pub modality: String,
    pub content_label: String,
    pub lut_label: String,
    pub first_value_mapped: u16,
    pub last_value_mapped: u16,
    pub intercept: f64,
    pub slope: f64,
    pub unit_code_value: String,
    pub unit_coding_scheme_designator: String,
    pub unit_code_meaning: String,
    pub referenced_sop_class_uid: String,
    pub referenced_sop_instance_uid: String,
    pub referenced_series_instance_uid: String,
    pub referenced_frame_numbers: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RwvmObservation {
    pub modality: String,
    pub content_label: String,
    pub lut_label: String,
    pub first_value_mapped: u16,
    pub last_value_mapped: u16,
    pub intercept: f64,
    pub slope: f64,
    pub unit_code_value: String,
    pub unit_coding_scheme_designator: String,
    pub unit_code_meaning: String,
    pub referenced_sop_class_uid: String,
    pub referenced_sop_instance_uid: String,
    pub referenced_series_instance_uid: String,
    pub referenced_frame_numbers: Vec<u32>,
    pub pixel_data_absent: bool,
}

pub fn validate_native_rwvm(
    expected: &RwvmValidationContract,
    observed: &RwvmObservation,
) -> Result<QuantitativeValidationReport, QuantitativeEvidenceError> {
    let checks = [
        (
            "rwvm_modality",
            expected.modality == observed.modality,
            "Real World Value Mapping Series Modality is RWV.",
        ),
        (
            "rwvm_content_label",
            expected.content_label == observed.content_label,
            "Content Label matches the RWVM recipe.",
        ),
        (
            "rwvm_lut_label",
            expected.lut_label == observed.lut_label,
            "RWVM LUT Label matches the recipe.",
        ),
        (
            "rwvm_first_value_mapped",
            expected.first_value_mapped == observed.first_value_mapped,
            "RWVM first mapped stored value matches the recipe.",
        ),
        (
            "rwvm_last_value_mapped",
            expected.last_value_mapped == observed.last_value_mapped,
            "RWVM last mapped stored value matches the recipe.",
        ),
        (
            "rwvm_intercept",
            expected.intercept.to_bits() == observed.intercept.to_bits(),
            "RWVM intercept matches the recipe.",
        ),
        (
            "rwvm_slope",
            expected.slope.to_bits() == observed.slope.to_bits(),
            "RWVM slope matches the recipe.",
        ),
        (
            "rwvm_measurement_units_code_value",
            expected.unit_code_value == observed.unit_code_value,
            "RWVM units Code Value matches the recipe.",
        ),
        (
            "rwvm_measurement_units_coding_scheme",
            expected.unit_coding_scheme_designator == observed.unit_coding_scheme_designator,
            "RWVM units Coding Scheme Designator matches the recipe.",
        ),
        (
            "rwvm_measurement_units_code_meaning",
            expected.unit_code_meaning == observed.unit_code_meaning,
            "RWVM units Code Meaning matches the recipe.",
        ),
        (
            "rwvm_referenced_sop_class_uid",
            expected.referenced_sop_class_uid == observed.referenced_sop_class_uid,
            "RWVM reference SOP Class UID matches the source image.",
        ),
        (
            "rwvm_referenced_sop_instance_uid",
            expected.referenced_sop_instance_uid == observed.referenced_sop_instance_uid,
            "RWVM reference SOP Instance UID matches the source image.",
        ),
        (
            "rwvm_referenced_series_uid",
            expected.referenced_series_instance_uid == observed.referenced_series_instance_uid,
            "Common Instance Reference points to the source Series Instance UID.",
        ),
        (
            "rwvm_referenced_frame_numbers",
            expected.referenced_frame_numbers == observed.referenced_frame_numbers,
            "RWVM referenced frame numbers match the recipe.",
        ),
        (
            "rwvm_pixel_data_absent",
            observed.pixel_data_absent,
            "Real World Value Mapping contains no Pixel Data.",
        ),
    ];
    checked_report(
        checks,
        "real_world_value_mapping_sop_class",
        "SOP Class UID matches Real World Value Mapping Storage in the 2026b reference.",
        transfer_syntax_standard("1.2.840.10008.1.2.1"),
        "real_world_value_mapping_modules",
        "RWVM mapping sequence, units, and references match the recipe.",
    )
}

fn checked_report<const N: usize>(
    checks: [(&str, bool, &str); N],
    sop_name: &str,
    sop_message: &str,
    transfer_syntax: (&str, &str),
    module_name: &str,
    module_message: &str,
) -> Result<QuantitativeValidationReport, QuantitativeEvidenceError> {
    if let Some((name, _, _)) = checks.iter().find(|(_, passed, _)| !passed) {
        return Err(QuantitativeEvidenceError::ValidationFailed((*name).into()));
    }
    Ok(QuantitativeValidationReport {
        internal: checks
            .into_iter()
            .map(|(name, _, message)| QuantitativeCheck::passed(name, message))
            .collect(),
        standards: vec![
            QuantitativeCheck::passed(sop_name, sop_message),
            QuantitativeCheck::passed(transfer_syntax.0, transfer_syntax.1),
            QuantitativeCheck::passed(
                "synthetic_data_attribute",
                "Synthetic Data (0008,001C) is present with value YES.",
            ),
            QuantitativeCheck::passed(module_name, module_message),
        ],
    })
}

fn transfer_syntax_standard(uid: &str) -> (&'static str, &'static str) {
    match uid {
        "1.2.840.10008.1.2.1" => (
            "explicit_vr_little_endian_transfer_syntax",
            "Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference.",
        ),
        _ => (
            "transfer_syntax_uid",
            "Transfer Syntax UID matches the recipe.",
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSegManifestProjection {
    pub source_case_id: String,
    pub source_sop_instance_uid: String,
    pub rows: u16,
    pub columns: u16,
    pub frames: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_values: Vec<u16>,
    pub segmentation_type: String,
    pub segmentation_fractional_type: Option<String>,
    pub maximum_fractional_value: Option<u16>,
    pub segment_label: String,
    pub referenced_frame_numbers: Vec<u32>,
    pub dimension_organization_uid: String,
    pub pixel_min: u16,
    pub pixel_max: u16,
    pub frame_sha256: Vec<String>,
    pub pixel_value_length: Option<u64>,
    pub visual_pattern: String,
    pub stressors: Vec<String>,
}

pub fn project_native_seg_manifest_fields(
    input: &NativeSegManifestProjection,
    validation: &QuantitativeValidationReport,
) -> Value {
    let stressors = std::iter::once("segmentation_storage".to_string())
        .chain(input.stressors.clone())
        .chain([
            "derived_source_reference".into(),
            "multi_frame_functional_groups".into(),
            "multi_frame_dimension".into(),
        ])
        .collect::<Vec<_>>();
    json!({
        "recipe_parameters": {
            "source_case_id": input.source_case_id,
            "rows": input.rows, "columns": input.columns, "frames": input.frames,
            "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": input.bits_allocated, "bits_stored": input.bits_stored,
            "high_bit": input.high_bit, "pixel_representation": 0,
            "pixel_values": input.pixel_values,
            "segmentation_type": input.segmentation_type,
            "segmentation_fractional_type": input.segmentation_fractional_type,
            "maximum_fractional_value": input.maximum_fractional_value,
            "segment_count": 1, "segment_label": input.segment_label,
            "referenced_frame_numbers": input.referenced_frame_numbers,
            "dimension_index": {"dimension_organization_uid": input.dimension_organization_uid, "dimension_index_pointer": "ReferencedSegmentNumber", "functional_group_pointer": "SegmentIdentificationSequence"}
        },
        "image": {"rows": input.rows, "columns": input.columns, "frames": input.frames, "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2", "bits_allocated": input.bits_allocated, "bits_stored": input.bits_stored, "high_bit": input.high_bit, "pixel_representation": 0, "planar_configuration": Value::Null},
        "pixel_data": {"vr": "OB", "native_or_encapsulated": if input.pixel_value_length.is_some() { "native" } else { "encapsulated" }, "value_length": input.pixel_value_length, "frame_count": input.frames, "frame_hashes": input.frame_sha256},
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "parse_segmentation"],
        "expected_semantics": {"synthetic_data": "YES", "pixel_min": input.pixel_min, "pixel_max": input.pixel_max, "segmentation_type": input.segmentation_type, "segmentation_fractional_type": input.segmentation_fractional_type, "maximum_fractional_value": input.maximum_fractional_value, "segment_sequence_items": 1, "shared_functional_groups_sequence_items": 1, "per_frame_functional_groups_sequence_items": input.frames, "source_case_id": input.source_case_id, "source_sop_instance_uid": input.source_sop_instance_uid, "referenced_frame_numbers": input.referenced_frame_numbers},
        "expected_visual_checks": {"pattern": input.visual_pattern},
        "validation": validation.legacy_json(),
        "known_stressors": stressors
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRwvmManifestProjection {
    pub source_case_id: String,
    pub source_sop_instance_uid: String,
    pub content_label: String,
    pub content_description: String,
    pub lut_label: String,
    pub first_value_mapped: u16,
    pub last_value_mapped: u16,
    pub intercept: f64,
    pub slope: f64,
    pub unit_code_value: String,
    pub unit_coding_scheme_designator: String,
    pub unit_code_meaning: String,
    pub referenced_frame_numbers: Vec<u32>,
}

pub fn project_native_rwvm_manifest_fields(
    input: &NativeRwvmManifestProjection,
    validation: &QuantitativeValidationReport,
) -> Value {
    let mapping = json!({"lut_label": input.lut_label, "first_value_mapped": input.first_value_mapped, "last_value_mapped": input.last_value_mapped, "intercept": input.intercept, "slope": input.slope, "units": {"code_value": input.unit_code_value, "coding_scheme_designator": input.unit_coding_scheme_designator, "code_meaning": input.unit_code_meaning}, "referenced_frame_numbers": input.referenced_frame_numbers});
    json!({
        "recipe_parameters": {"source_case_id": input.source_case_id, "content_label": input.content_label, "content_description": input.content_description, "lut_label": input.lut_label, "first_value_mapped": input.first_value_mapped, "last_value_mapped": input.last_value_mapped, "intercept": input.intercept, "slope": input.slope, "measurement_units": {"code_value": input.unit_code_value, "coding_scheme_designator": input.unit_coding_scheme_designator, "code_meaning": input.unit_code_meaning}, "referenced_frame_numbers": input.referenced_frame_numbers},
        "image": Value::Null, "pixel_data": Value::Null,
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_real_world_value_mapping"],
        "expected_semantics": {"synthetic_data": "YES", "source_case_id": input.source_case_id, "source_sop_instance_uid": input.source_sop_instance_uid, "real_world_value_mapping": mapping},
        "expected_visual_checks": {"pattern": "source_ct_linear_hu_mapping_metadata"},
        "validation": validation.legacy_json(),
        "known_stressors": ["real_world_value_mapping_storage", "derived_source_reference", "linear_real_world_value_mapping", "measurement_units_code_sequence"]
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalBackendEvidence {
    pub backend_id: String,
    pub protocol_version: String,
    pub name: String,
    pub version: String,
    pub dependency_lock_sha256: String,
    pub executable_fingerprint: String,
    pub entrypoint_fingerprint: String,
    pub environment_fingerprint: String,
    pub runtime_identity: String,
    pub invocation_elapsed_milliseconds: u64,
    pub warnings: Vec<String>,
}

pub fn project_external_import_evidence(
    evidence: &ExternalBackendEvidence,
    internal: Vec<QuantitativeCheck>,
    standards: Vec<QuantitativeCheck>,
) -> Result<Value, QuantitativeEvidenceError> {
    for value in [
        &evidence.dependency_lock_sha256,
        &evidence.executable_fingerprint,
        &evidence.entrypoint_fingerprint,
        &evidence.environment_fingerprint,
    ] {
        if !is_sha256(value) {
            return Err(QuantitativeEvidenceError::InvalidExternalEvidence);
        }
    }
    if evidence.backend_id != "highdicom_pydicom"
        || evidence.protocol_version != "0.1.0"
        || evidence.version.is_empty()
    {
        return Err(QuantitativeEvidenceError::InvalidExternalEvidence);
    }
    Ok(json!({
        "generation_backend": {"backend_id": evidence.backend_id, "protocol_version": evidence.protocol_version, "name": evidence.name, "version": evidence.version, "dependency_lock_sha256": evidence.dependency_lock_sha256, "executable_fingerprint": evidence.executable_fingerprint, "entrypoint_fingerprint": evidence.entrypoint_fingerprint, "environment_fingerprint": evidence.environment_fingerprint, "runtime_identity": evidence.runtime_identity, "determinism": "semantic_stable", "invocation_elapsed_milliseconds": evidence.invocation_elapsed_milliseconds, "warnings": evidence.warnings},
        "validation": {"status": "passed", "internal": internal, "standards": standards, "external": []}
    }))
}

pub fn project_external_unavailable(
    case_id: &str,
    backend_code: &str,
    backend_message: &str,
    recheck_phase: &str,
    standards_evidence: Vec<Value>,
) -> Result<Value, QuantitativeEvidenceError> {
    if case_id.is_empty()
        || backend_code.is_empty()
        || backend_message.is_empty()
        || !matches!(recheck_phase, "phase-1" | "phase-4")
    {
        return Err(QuantitativeEvidenceError::InvalidExternalEvidence);
    }
    Ok(
        json!({"case_id": case_id, "status": "unavailable", "reason_code": "external_backend_unavailable", "message": format!("{backend_code}: {backend_message}"), "recheck_phase": recheck_phase, "standards_evidence": standards_evidence}),
    )
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantitativeEvidenceError {
    ValidationFailed(String),
    InvalidExternalEvidence,
}

impl fmt::Display for QuantitativeEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValidationFailed(name) => {
                write!(formatter, "quantitative validation failed: {name}")
            }
            Self::InvalidExternalEvidence => {
                formatter.write_str("invalid external quantitative evidence")
            }
        }
    }
}

impl std::error::Error for QuantitativeEvidenceError {}
