//! Pure typed per-file manifest projection for SR and RT artifacts.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::sr_rt_validation::SpecializedValidationEvidence;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestOutputFacts {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestIdentityFacts {
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub sop_instance_uid: String,
    pub frame_of_reference_uid: Option<String>,
    pub implementation_class_uid: String,
    pub implementation_version_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestReferenceFacts {
    pub case_id: String,
    pub path: String,
    pub sha256: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub role: String,
    pub referenced_frames: Option<Vec<u32>>,
}

impl ManifestReferenceFacts {
    fn legacy_json(&self) -> Value {
        json!({
            "case_id": self.case_id, "path": self.path, "sha256": self.sha256,
            "sop_class_uid": self.sop_class_uid, "sop_instance_uid": self.sop_instance_uid,
            "role": self.role, "referenced_frames": self.referenced_frames,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticManifestSpec {
    pub case_id: String,
    pub profile_membership: Vec<String>,
    pub recipe_id: String,
    pub recipe_version: String,
    pub recipe_parameters: Value,
    pub output: ManifestOutputFacts,
    pub determinism: String,
    pub sop_class_uid: String,
    pub sop_class_name: String,
    pub iod_name: String,
    pub modality: String,
    pub transfer_syntax_uid: String,
    pub transfer_syntax_name: String,
    pub identities: ManifestIdentityFacts,
    pub references: Vec<ManifestReferenceFacts>,
    pub expected_capabilities: Vec<String>,
    pub expected_semantics: Value,
    pub expected_visual_pattern: String,
    pub known_stressors: Vec<String>,
    pub standards_evidence: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SrManifestKind {
    BasicText,
    Comprehensive,
    KeyObjectSelection,
    Comprehensive3d,
    Tid1500,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SrManifestProjection {
    pub kind: SrManifestKind,
    pub common: SemanticManifestSpec,
    pub external_import: Option<Value>,
}

pub fn project_sr_manifest_entry(
    input: &SrManifestProjection,
    validation: &SpecializedValidationEvidence,
) -> Result<Value, SemanticManifestError> {
    validate_common(&input.common)?;
    let external = matches!(
        input.kind,
        SrManifestKind::Comprehensive3d | SrManifestKind::Tid1500
    );
    if external != input.external_import.is_some()
        || external != (input.common.determinism == "semantic_stable")
        || (!external && input.common.modality != "SR")
    {
        return Err(SemanticManifestError::KindMismatch);
    }
    let mut value = common_json(&input.common, validation);
    if let Some(evidence) = &input.external_import {
        value["generation_backend"] = evidence.clone();
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RtManifestKind {
    StructureSet,
    Dose,
    Plan,
    Image,
    CarmRadiation,
    RadiationSet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RtManifestProjection {
    pub kind: RtManifestKind,
    pub common: SemanticManifestSpec,
    /// Exact historical specialized RT expectation object, obtained from the
    /// typed validator observation rather than by reopening the output.
    pub expected_rt_object: Value,
    pub image: Option<Value>,
    pub pixel_data: Option<Value>,
}

pub fn project_rt_manifest_entry(
    input: &RtManifestProjection,
    validation: &SpecializedValidationEvidence,
) -> Result<Value, SemanticManifestError> {
    validate_common(&input.common)?;
    let pixel_bearing = matches!(input.kind, RtManifestKind::Dose | RtManifestKind::Image);
    if input.common.determinism != "byte_stable"
        || input.expected_rt_object.is_null()
        || pixel_bearing != (input.image.is_some() && input.pixel_data.is_some())
        || (!pixel_bearing && (input.image.is_some() || input.pixel_data.is_some()))
    {
        return Err(SemanticManifestError::KindMismatch);
    }
    let mut value = common_json(&input.common, validation);
    value["image"] = input.image.clone().unwrap_or(Value::Null);
    value["pixel_data"] = input.pixel_data.clone().unwrap_or(Value::Null);
    let key = match input.kind {
        RtManifestKind::StructureSet => "expected_rt_structure_set",
        RtManifestKind::Dose => "expected_rt_dose",
        RtManifestKind::Plan => "expected_rt_plan",
        RtManifestKind::Image => "expected_rt_image",
        RtManifestKind::CarmRadiation => "expected_rt_radiation",
        RtManifestKind::RadiationSet => "expected_rt_radiation_set",
    };
    value[key] = input.expected_rt_object.clone();
    Ok(value)
}

fn common_json(spec: &SemanticManifestSpec, validation: &SpecializedValidationEvidence) -> Value {
    let references = spec
        .references
        .iter()
        .map(ManifestReferenceFacts::legacy_json)
        .collect::<Vec<_>>();
    json!({
        "case_id": spec.case_id,
        "profile_membership": spec.profile_membership,
        "path": spec.output.relative_path,
        "sha256": spec.output.sha256,
        "size_bytes": spec.output.size_bytes,
        "determinism": spec.determinism,
        "recipe": {"recipe_id": spec.recipe_id, "recipe_version": spec.recipe_version, "recipe_parameters": spec.recipe_parameters},
        "dicom": {"sop_class_uid": spec.sop_class_uid, "sop_class_name": spec.sop_class_name, "iod_name": spec.iod_name, "modality": spec.modality, "transfer_syntax_uid": spec.transfer_syntax_uid, "transfer_syntax_name": spec.transfer_syntax_name},
        "uids": {"study_instance_uid": spec.identities.study_instance_uid, "series_instance_uid": spec.identities.series_instance_uid, "sop_instance_uid": spec.identities.sop_instance_uid, "frame_of_reference_uid": spec.identities.frame_of_reference_uid, "implementation_class_uid": spec.identities.implementation_class_uid, "implementation_version_name": spec.identities.implementation_version_name},
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": references,
        "expected_capabilities": spec.expected_capabilities,
        "expected_semantics": spec.expected_semantics,
        "expected_visual_checks": {"pattern": spec.expected_visual_pattern},
        "validation": validation.legacy_json(),
        "known_stressors": spec.known_stressors,
        "standards_evidence": spec.standards_evidence,
    })
}

fn validate_common(spec: &SemanticManifestSpec) -> Result<(), SemanticManifestError> {
    if spec.case_id.is_empty()
        || spec.recipe_id.is_empty()
        || spec.recipe_version.is_empty()
        || spec.output.relative_path.starts_with('/')
        || spec.output.relative_path.contains("..")
        || spec.output.size_bytes == 0
        || !is_sha256(&spec.output.sha256)
        || spec.profile_membership.is_empty()
        || spec.sop_class_uid.is_empty()
        || spec.identities.study_instance_uid.is_empty()
        || spec.identities.series_instance_uid.is_empty()
        || spec.identities.sop_instance_uid.is_empty()
        || spec.identities.implementation_class_uid.is_empty()
        || spec.expected_capabilities.is_empty()
        || spec.known_stressors.is_empty()
    {
        return Err(SemanticManifestError::InvalidCommonFacts);
    }
    for reference in &spec.references {
        if reference.case_id.is_empty()
            || reference.path.is_empty()
            || reference.role.is_empty()
            || !is_sha256(&reference.sha256)
            || reference.sop_class_uid.is_empty()
            || reference.sop_instance_uid.is_empty()
        {
            return Err(SemanticManifestError::InvalidReference);
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticManifestError {
    InvalidCommonFacts,
    InvalidReference,
    KindMismatch,
}

impl fmt::Display for SemanticManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommonFacts => f.write_str("semantic manifest common facts are invalid"),
            Self::InvalidReference => f.write_str("semantic manifest reference facts are invalid"),
            Self::KindMismatch => {
                f.write_str("semantic manifest kind and specialized facts disagree")
            }
        }
    }
}

impl std::error::Error for SemanticManifestError {}
