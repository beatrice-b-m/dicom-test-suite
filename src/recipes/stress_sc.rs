//! Typed, filesystem-free plans for reduced-scale Secondary Capture stressors.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::corpus_plan::{ArtifactResourceEstimate, OutputRelativePath};
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

use super::CaseRecipe;

pub const STRESS_SC_PLAN_PROVIDER_ID: &str = "native.stress_sc_plan";
pub const STRESS_SC_CONTENT_PROVIDER_ID: &str = "content.stress.synthetic";
pub const STRESS_SC_ALGORITHM_PROVIDER_ID: &str = "algorithm.stress_sc";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReducedStressPolicy {
    pub qualification_scale: String,
    pub full_scale_available: bool,
    pub full_scale_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StressScParameters {
    LargeBulk {
        rows: u32,
        columns: u32,
        payload_bytes: u64,
        fill_byte: u8,
        policy: ReducedStressPolicy,
    },
    DeepNestedSequences {
        sequence_depth: u32,
        payload_bytes: u64,
        fill_byte: u8,
        private_creator: String,
        policy: ReducedStressPolicy,
    },
    LongValueMetadata {
        creator_blocks: u32,
        values_per_block: u32,
        metadata_value_bytes: u32,
        fill_character: String,
        policy: ReducedStressPolicy,
    },
    LargeEncapsulatedMultifragment {
        rows: u32,
        columns: u32,
        frames: u32,
        fragments_per_frame: u32,
        native_algorithm: String,
        offset_table_policy: String,
        policy: ReducedStressPolicy,
    },
}

impl StressScParameters {
    pub fn policy(&self) -> &ReducedStressPolicy {
        match self {
            Self::LargeBulk { policy, .. }
            | Self::DeepNestedSequences { policy, .. }
            | Self::LongValueMetadata { policy, .. }
            | Self::LargeEncapsulatedMultifragment { policy, .. } => policy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StressScContentRequest {
    RepeatedNativeBytes {
        byte: u8,
        length: u64,
    },
    NestedPrivateBulk {
        sequence_depth: u32,
        creator: String,
        byte: u8,
        length: u64,
    },
    RepeatedPrivateText {
        creator_blocks: u32,
        values_per_block: u32,
        value_bytes: u32,
        fill_character: char,
    },
    DeterministicRleFrames {
        rows: u32,
        columns: u32,
        frames: u32,
        fragments_per_frame: u32,
        algorithm: String,
        extended_offset_table: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressScIdentityPlan {
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub sop_instance_uid: String,
    pub implementation_class_uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressScCommonPlan {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_date: String,
    pub study_time: String,
    pub study_id: String,
    pub modality: String,
    pub series_number: String,
    pub instance_number: String,
    pub conversion_type: String,
    pub manufacturer: String,
    pub manufacturer_model_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StressScPixelRequest {
    RepeatedU16 {
        rows: u32,
        columns: u32,
        value: u16,
    },
    LiteralU8 {
        rows: u32,
        columns: u32,
        values: Vec<u8>,
    },
    AlgorithmicU8Multiframe {
        rows: u32,
        columns: u32,
        frames: u32,
        algorithm: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressScArtifactPlan {
    pub logical_id: String,
    pub order: u64,
    pub output_relative_path: OutputRelativePath,
    pub dependencies: Vec<String>,
    pub template_id: String,
    pub template_version: String,
    pub sop_class_uid: String,
    pub transfer_syntax_uid: String,
    pub identities: StressScIdentityPlan,
    pub common: StressScCommonPlan,
    pub pixels: StressScPixelRequest,
    pub parameters: StressScParameters,
    pub content: StressScContentRequest,
    pub resources: ArtifactResourceEstimate,
    pub validation_rule_ids: Vec<String>,
    pub projection_rule_ids: Vec<String>,
}

pub fn plan_stress_sc_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<StressScArtifactPlan>, StressScPlanError> {
    if !recipe.binding.case_id.starts_with("stress/sc/") {
        return Ok(None);
    }
    if recipe.plan_provider_id != STRESS_SC_PLAN_PROVIDER_ID {
        return Err(contract("stress SC recipe has the wrong plan provider"));
    }
    let order = recipe
        .planning_order
        .ok_or_else(|| contract("stress SC recipe requires planning_order"))?;
    if !(1401..=1404).contains(&order) {
        return Err(contract("stress SC planning_order is outside 1401..=1404"));
    }
    let parameters: StressScParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| StressScPlanError::Parameters(error.to_string()))?;
    validate_parameters(&parameters)?;
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| contract("stress SC recipe requires DICOM artifacts"))?;
    if dicom.artifacts.len() != 1 {
        return Err(contract("stress SC recipe requires exactly one artifact"));
    }
    let artifact = &dicom.artifacts[0];
    if artifact.logical_id != "instance"
        || artifact.order != 0
        || artifact.content.provider_id != STRESS_SC_CONTENT_PROVIDER_ID
        || artifact.algorithm_provider_id.as_deref() != Some(STRESS_SC_ALGORITHM_PROVIDER_ID)
        || !artifact.parameters.is_empty()
        || !artifact.attribute_operations.is_empty()
    {
        return Err(contract("stress SC artifact binding is not explicit"));
    }
    let template = artifact
        .template
        .as_ref()
        .ok_or_else(|| contract("stress SC artifact requires a template"))?;
    let path = artifact
        .output
        .path
        .as_ref()
        .ok_or_else(|| contract("stress SC output path must be explicit"))?;
    let (content, pixels, resources, expected_template, expected_ts) = content_plan(&parameters)?;
    if template.template_id != expected_template
        || artifact.encoding.transfer_syntax_uid != expected_ts
    {
        return Err(contract("stress SC template or transfer syntax mismatch"));
    }
    let identities = StressScIdentityPlan {
        study_instance_uid: uid(recipe, standards_lock_sha256, seed, UidRole::StudyInstance),
        series_instance_uid: uid(recipe, standards_lock_sha256, seed, UidRole::SeriesInstance),
        sop_instance_uid: uid(recipe, standards_lock_sha256, seed, UidRole::SopInstance),
        implementation_class_uid: deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: "dicom-test-suite/implementation",
            recipe_version: crate::PACKAGE_VERSION,
            run_seed: 0,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role: UidRole::ImplementationClass,
        }),
    };
    Ok(Some(StressScArtifactPlan {
        logical_id: artifact.logical_id.clone(),
        order: artifact.order.into(),
        output_relative_path: OutputRelativePath::new(path.clone())
            .map_err(|error| contract(error.to_string()))?,
        dependencies: Vec::new(),
        template_id: template.template_id.clone(),
        template_version: template.template_version.clone(),
        sop_class_uid: if expected_template.ends_with("multiframe-grayscale-byte") {
            "1.2.840.10008.5.1.4.1.1.7.2"
        } else {
            "1.2.840.10008.5.1.4.1.1.7"
        }
        .into(),
        transfer_syntax_uid: artifact.encoding.transfer_syntax_uid.clone(),
        identities,
        common: StressScCommonPlan {
            patient_name: "DICOMTEST^STRESS".into(),
            patient_id: "DICOMTEST-STRESS-001".into(),
            patient_birth_date: "19700101".into(),
            patient_sex: "O".into(),
            study_date: "20260101".into(),
            study_time: "000000".into(),
            study_id: "DTS-STRESS".into(),
            modality: "OT".into(),
            series_number: "1".into(),
            instance_number: "1".into(),
            conversion_type: "SYN".into(),
            manufacturer: "dicom-test-suite".into(),
            manufacturer_model_name: recipe.recipe_id.clone(),
        },
        pixels,
        parameters,
        content,
        resources,
        validation_rule_ids: artifact.validation_rule_ids.clone(),
        projection_rule_ids: artifact.projection_rule_ids.clone(),
    }))
}

fn validate_parameters(parameters: &StressScParameters) -> Result<(), StressScPlanError> {
    let policy = match parameters {
        StressScParameters::LargeBulk {
            rows,
            columns,
            payload_bytes,
            fill_byte,
            policy,
        } if (*rows, *columns, *payload_bytes, *fill_byte) == (8192, 4096, 64 * 1024 * 1024, 0) => {
            policy
        }
        StressScParameters::DeepNestedSequences {
            sequence_depth,
            payload_bytes,
            fill_byte,
            private_creator,
            policy,
        } if (
            *sequence_depth,
            *payload_bytes,
            *fill_byte,
            private_creator.as_str(),
        ) == (32, 16 * 1024 * 1024, 0x5a, "DTS_STRESS_NESTED") =>
        {
            policy
        }
        StressScParameters::LongValueMetadata {
            creator_blocks,
            values_per_block,
            metadata_value_bytes,
            fill_character,
            policy,
        } if (
            *creator_blocks,
            *values_per_block,
            *metadata_value_bytes,
            fill_character.as_str(),
        ) == (4, 256, 1024, "M") =>
        {
            policy
        }
        StressScParameters::LargeEncapsulatedMultifragment {
            rows,
            columns,
            frames,
            fragments_per_frame,
            native_algorithm,
            offset_table_policy,
            policy,
        } if (
            *rows,
            *columns,
            *frames,
            *fragments_per_frame,
            native_algorithm.as_str(),
            offset_table_policy.as_str(),
        ) == (
            512,
            512,
            256,
            64,
            "index_mul_37_frame_mul_17_xor_index_shift_8",
            "empty_basic_with_extended",
        ) =>
        {
            policy
        }
        _ => {
            return Err(contract(
                "stress SC parameters differ from the reduced qualification",
            ));
        }
    };
    if policy.qualification_scale != "reduced"
        || policy.full_scale_available
        || policy.full_scale_reason.is_empty()
    {
        return Err(contract(
            "full-scale stress unavailability must remain explicit",
        ));
    }
    Ok(())
}

fn content_plan(
    parameters: &StressScParameters,
) -> Result<
    (
        StressScContentRequest,
        StressScPixelRequest,
        ArtifactResourceEstimate,
        &'static str,
        &'static str,
    ),
    StressScPlanError,
> {
    const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";
    const RLE: &str = "1.2.840.10008.1.2.5";
    const PART10_STRUCTURAL_BUDGET: u64 = 1024 * 1024;
    let result = match parameters {
        StressScParameters::LargeBulk {
            payload_bytes,
            fill_byte,
            ..
        } => (
            StressScContentRequest::RepeatedNativeBytes {
                byte: *fill_byte,
                length: *payload_bytes,
            },
            StressScPixelRequest::RepeatedU16 {
                rows: 8192,
                columns: 4096,
                value: 0,
            },
            estimate(
                payload_bytes
                    .checked_add(PART10_STRUCTURAL_BUDGET)
                    .ok_or(StressScPlanError::ResourceOverflow)?,
                payload_bytes
                    .checked_mul(2)
                    .ok_or(StressScPlanError::ResourceOverflow)?,
            )?,
            "classic/secondary-capture/monochrome",
            EXPLICIT_LE,
        ),
        StressScParameters::DeepNestedSequences {
            sequence_depth,
            payload_bytes,
            fill_byte,
            private_creator,
            ..
        } => (
            StressScContentRequest::NestedPrivateBulk {
                sequence_depth: *sequence_depth,
                creator: private_creator.clone(),
                byte: *fill_byte,
                length: *payload_bytes,
            },
            StressScPixelRequest::LiteralU8 {
                rows: 2,
                columns: 2,
                values: vec![0, 85, 170, 255],
            },
            estimate(
                payload_bytes
                    .checked_add(PART10_STRUCTURAL_BUDGET)
                    .ok_or(StressScPlanError::ResourceOverflow)?,
                *payload_bytes + 1_048_576,
            )?,
            "classic/secondary-capture/monochrome",
            EXPLICIT_LE,
        ),
        StressScParameters::LongValueMetadata {
            creator_blocks,
            values_per_block,
            metadata_value_bytes,
            fill_character,
            ..
        } => {
            let payload = u64::from(*creator_blocks)
                .checked_mul(u64::from(*values_per_block))
                .and_then(|value| value.checked_mul(u64::from(*metadata_value_bytes)))
                .ok_or(StressScPlanError::ResourceOverflow)?;
            (
                StressScContentRequest::RepeatedPrivateText {
                    creator_blocks: *creator_blocks,
                    values_per_block: *values_per_block,
                    value_bytes: *metadata_value_bytes,
                    fill_character: fill_character
                        .chars()
                        .next()
                        .ok_or_else(|| contract("empty fill character"))?,
                },
                StressScPixelRequest::LiteralU8 {
                    rows: 2,
                    columns: 2,
                    values: vec![0, 85, 170, 255],
                },
                estimate(
                    payload
                        .checked_add(PART10_STRUCTURAL_BUDGET)
                        .ok_or(StressScPlanError::ResourceOverflow)?,
                    payload + 1_048_576,
                )?,
                "classic/secondary-capture/monochrome",
                EXPLICIT_LE,
            )
        }
        StressScParameters::LargeEncapsulatedMultifragment {
            rows,
            columns,
            frames,
            fragments_per_frame,
            native_algorithm,
            ..
        } => {
            let native = u64::from(*rows)
                .checked_mul(u64::from(*columns))
                .and_then(|value| value.checked_mul(u64::from(*frames)))
                .ok_or(StressScPlanError::ResourceOverflow)?;
            let fragment_headers = u64::from(*frames)
                .checked_mul(u64::from(*fragments_per_frame))
                .and_then(|value| value.checked_mul(8))
                .ok_or(StressScPlanError::ResourceOverflow)?;
            (
                StressScContentRequest::DeterministicRleFrames {
                    rows: *rows,
                    columns: *columns,
                    frames: *frames,
                    fragments_per_frame: *fragments_per_frame,
                    algorithm: native_algorithm.clone(),
                    extended_offset_table: true,
                },
                StressScPixelRequest::AlgorithmicU8Multiframe {
                    rows: *rows,
                    columns: *columns,
                    frames: *frames,
                    algorithm: native_algorithm.clone(),
                },
                estimate(
                    native
                        .checked_add(fragment_headers)
                        .and_then(|value| value.checked_add(PART10_STRUCTURAL_BUDGET))
                        .ok_or(StressScPlanError::ResourceOverflow)?,
                    native
                        .checked_mul(4)
                        .and_then(|value| value.checked_add(4 * 1024 * 1024))
                        .ok_or(StressScPlanError::ResourceOverflow)?,
                )?,
                "classic/secondary-capture/multiframe-grayscale-byte",
                RLE,
            )
        }
    };
    Ok(result)
}

fn estimate(
    output_bytes: u64,
    peak_working_bytes: u64,
) -> Result<ArtifactResourceEstimate, StressScPlanError> {
    if output_bytes == 0 || peak_working_bytes == 0 {
        return Err(StressScPlanError::ResourceOverflow);
    }
    Ok(ArtifactResourceEstimate {
        output_bytes,
        peak_working_bytes,
    })
}

fn uid(recipe: &CaseRecipe, lock: &str, seed: u64, role: UidRole) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: &recipe.binding.case_id,
        recipe_version: &recipe.recipe_version,
        run_seed: seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn contract(message: impl Into<String>) -> StressScPlanError {
    StressScPlanError::Contract(message.into())
}

#[derive(Debug)]
pub enum StressScPlanError {
    Contract(String),
    Parameters(String),
    ResourceOverflow,
}

impl fmt::Display for StressScPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(message) | Self::Parameters(message) => formatter.write_str(message),
            Self::ResourceOverflow => formatter.write_str("stress resource estimate overflow"),
        }
    }
}

impl Error for StressScPlanError {}
