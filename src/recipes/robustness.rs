//! Strict, frontend-neutral contracts for robustness recipe selection.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use super::super::model::{CaseRecipe, Parameters, RecipeReference};

pub const FUZZ_PLAN_PROVIDER_ID: &str = "qualification.fuzz_plan";
pub const EOT_ARITHMETIC_PLAN_PROVIDER_ID: &str = "qualification.eot_arithmetic_plan";

const FAILURE_LAYERS: &[&str] = &[
    "file_meta",
    "dataset_parser",
    "value_decoding",
    "semantic_validation",
    "pixel_decoding",
    "encapsulation",
    "text_decoding",
];
const OUTCOMES: &[&str] = &[
    "clean_rejection",
    "parse_failure",
    "validation_failure",
    "decode_failure",
    "accepted_with_bounded_warning",
];

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RobustnessProviderParameters {
    pub qualification_provider_id: String,
    pub payload_policy: PayloadPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadPolicy {
    NoPayloadRetained,
    EvidenceOnly,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "qualification_kind", rename_all = "snake_case")]
pub enum QualificationParameters {
    BoundedDeterministicFuzz {
        source_generation_seed: u64,
        candidates_per_source: u64,
        sources: Vec<FuzzSource>,
        budget: FuzzBudgetContract,
    },
    CheckedEotU64Overflow {
        fragment_lengths: Vec<u64>,
        arithmetic_steps: Vec<String>,
        expected_error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuzzSource {
    pub seed_description_id: String,
    pub dependency_role: String,
    pub recipe: RecipeReference,
    pub artifact_logical_id: String,
    pub mutation_surfaces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuzzBudgetContract {
    pub max_iterations: u64,
    pub max_candidates: u64,
    pub max_mutations_per_candidate: u32,
    pub max_total_mutations: u64,
    pub max_bytes_per_mutation: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_minimization_attempts: u64,
    pub max_total_target_operations: u64,
    pub max_target_operations: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct TagParameter {
    tag: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ByteReplacement {
    tag: String,
    byte_offset: u64,
    replacement_byte: u8,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CharsetReplacement {
    tag: String,
    replacement_byte: u8,
    fill_policy: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectorFraction {
    selector: String,
    numerator: u64,
    denominator: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct TagFraction {
    tag: String,
    numerator: u64,
    denominator: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectorOnly {
    selector: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct LengthAdjustment {
    tag: String,
    declared_length_adjustment: i64,
    width_policy: String,
}

fn object(parameters: &Parameters) -> Value {
    Value::Object(parameters.clone())
}

fn typed<T: for<'de> Deserialize<'de>>(parameters: &Parameters) -> Result<T, String> {
    serde_json::from_value(object(parameters)).map_err(|error| error.to_string())
}

pub fn validate_mutation_contract(recipe: &CaseRecipe) -> Result<(), String> {
    let mutation = recipe
        .mutation
        .as_ref()
        .ok_or_else(|| "mutation recipe lacks mutation contract".to_string())?;
    if !recipe.provider_parameters.is_empty() {
        return Err("mutation provider parameters must be empty".into());
    }
    if mutation.output.path.is_none() || mutation.output.provider_derived == Some(true) {
        return Err("mutation output must declare an exact path".into());
    }
    if mutation.retention != "expected_invalid_only" {
        return Err("mutation retention must be expected_invalid_only".into());
    }
    if mutation
        .failure_layers
        .iter()
        .any(|value| !FAILURE_LAYERS.contains(&value.as_str()))
    {
        return Err("mutation declares an unknown failure layer".into());
    }
    if mutation
        .acceptable_outcomes
        .iter()
        .any(|value| !OUTCOMES.contains(&value.as_str()))
    {
        return Err("mutation declares an unknown acceptable outcome".into());
    }
    let mut edit_ids = BTreeSet::new();
    for (index, edit) in mutation.edits.iter().enumerate() {
        if !edit_ids.insert(edit.edit_id.as_str()) {
            return Err("mutation edit IDs must be unique".into());
        }
        if !edit.edit_id.starts_with(&format!("{:02}_", index + 1)) {
            return Err("mutation edit IDs must carry their one-based execution ordinal".into());
        }
        validate_operation(&edit.mutation_id, &edit.parameters)?;
    }
    Ok(())
}

fn validate_operation(id: &str, parameters: &Parameters) -> Result<(), String> {
    match id {
        "mutation.invalid_character_set_declaration" => {
            let value: CharsetReplacement = typed(parameters)?;
            valid_tag(&value.tag)?;
            if value.fill_policy != "value_length" {
                return Err("invalid charset fill policy".into());
            }
        }
        "mutation.malformed_encoded_text" => {
            let value: ByteReplacement = typed(parameters)?;
            valid_tag(&value.tag)?;
        }
        "mutation.invalid_nested_item_length" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                selector: String,
                declared_length_adjustment: i64,
            }
            let value: P = typed(parameters)?;
            if value.selector != "first_sequence_item" || value.declared_length_adjustment == 0 {
                return Err("invalid nested item length parameters".into());
            }
        }
        "mutation.truncate_dataset" => {
            let value: SelectorOnly = typed(parameters)?;
            if value.selector != "pixel_data_header_start" {
                return Err("invalid dataset truncation selector".into());
            }
        }
        "mutation.truncate_sequence_item" | "mutation.truncate_fragment" => {
            let value: SelectorFraction = typed(parameters)?;
            valid_fraction(value.numerator, value.denominator)?;
            let expected = if id == "mutation.truncate_sequence_item" {
                "first_nonempty_sequence_item"
            } else {
                "first_fragment"
            };
            if value.selector != expected {
                return Err("invalid truncation selector".into());
            }
        }
        "mutation.undefined_length_without_delimitation" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                selector: String,
                remove: String,
            }
            let value: P = typed(parameters)?;
            if value.selector != "first_undefined_length_sequence"
                || value.remove != "sequence_delimitation_item"
            {
                return Err("invalid delimitation parameters".into());
            }
        }
        "mutation.broken_extended_offset_table" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                entry_index: u64,
                offset: String,
            }
            let value: P = typed(parameters)?;
            if value.entry_index != 0 || value.offset != "u64_max" {
                return Err("invalid EOT mutation parameters".into());
            }
        }
        "mutation.incorrect_explicit_vr_length" | "mutation.invalid_pixel_byte_length" => {
            let value: LengthAdjustment = typed(parameters)?;
            valid_tag(&value.tag)?;
            if value.declared_length_adjustment == 0 || value.width_policy != "from_vr" {
                return Err("invalid length adjustment parameters".into());
            }
        }
        "mutation.illegal_vr_bytes" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                tag: String,
                replacement_ascii: String,
            }
            let value: P = typed(parameters)?;
            valid_tag(&value.tag)?;
            if value.replacement_ascii.as_bytes().len() != 2 {
                return Err("replacement VR must be two ASCII bytes".into());
            }
        }
        "mutation.transfer_syntax_mismatch" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                replacement_uid: String,
                padding_policy: String,
            }
            let value: P = typed(parameters)?;
            if value.padding_policy != "preserve_value_length"
                || !value
                    .replacement_uid
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'.')
            {
                return Err("invalid transfer syntax mismatch parameters".into());
            }
        }
        "mutation.dataset_uid_mismatch" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                dataset_tag: String,
                replacement_policy: String,
            }
            let value: P = typed(parameters)?;
            valid_tag(&value.dataset_tag)?;
            if value.replacement_policy != "increment_last_decimal_digit_preserve_length" {
                return Err("invalid UID replacement policy".into());
            }
        }
        "mutation.missing_type1_attribute" => {
            let value: TagParameter = typed(parameters)?;
            valid_tag(&value.tag)?;
        }
        "mutation.truncate_file_meta_value" | "mutation.truncate_pixel_value" => {
            let value: TagFraction = typed(parameters)?;
            valid_tag(&value.tag)?;
            valid_fraction(value.numerator, value.denominator)?;
        }
        "mutation.invalid_bits_stored_high_bit" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                bits_stored_tag: String,
                high_bit_tag: String,
                bits_stored: u16,
                high_bit: u16,
            }
            let value: P = typed(parameters)?;
            valid_tag(&value.bits_stored_tag)?;
            valid_tag(&value.high_bit_tag)?;
            if value.bits_stored <= value.high_bit {
                return Err("invalid bits mutation must be internally inconsistent".into());
            }
        }
        _ => return Err(format!("unknown mutation id {id}")),
    }
    Ok(())
}

pub fn qualification_parameters(recipe: &CaseRecipe) -> Result<QualificationParameters, String> {
    let qualification = recipe
        .qualification
        .as_ref()
        .ok_or_else(|| "qualification payload is absent".to_string())?;
    serde_json::from_value(Value::Object(qualification.parameters.clone()))
        .map_err(|error| error.to_string())
}

pub fn validate_qualification_contract(recipe: &CaseRecipe) -> Result<(), String> {
    let provider: RobustnessProviderParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| error.to_string())?;
    let qualification = recipe
        .qualification
        .as_ref()
        .ok_or_else(|| "qualification payload is absent".to_string())?;
    match (
        recipe.plan_provider_id.as_str(),
        qualification_parameters(recipe)?,
    ) {
        (
            FUZZ_PLAN_PROVIDER_ID,
            QualificationParameters::BoundedDeterministicFuzz {
                source_generation_seed,
                candidates_per_source,
                sources,
                budget,
            },
        ) => {
            if provider.qualification_provider_id != "fuzz.bounded_deterministic"
                || provider.payload_policy != PayloadPolicy::NoPayloadRetained
                || qualification.retention != "none"
            {
                return Err("fuzz provider/retention contract does not agree".into());
            }
            if source_generation_seed == 0 || candidates_per_source == 0 || sources.is_empty() {
                return Err("fuzz source selection must be nonempty and bounded".into());
            }
            if budget.max_iterations == 0
                || budget.max_candidates > budget.max_iterations
                || budget.max_mutations_per_candidate == 0
                || budget.max_total_mutations == 0
                || budget.max_bytes_per_mutation == 0
                || budget.max_input_bytes == 0
                || budget.max_output_bytes == 0
                || budget.max_minimization_attempts == 0
                || budget.max_target_operations == 0
                || budget.max_target_operations > budget.max_total_target_operations
            {
                return Err("fuzz budget is inconsistent".into());
            }
            if qualification.resource_policy.max_input_bytes != budget.max_input_bytes
                || qualification.resource_policy.max_output_bytes != budget.max_output_bytes
                || qualification.resource_policy.max_operations
                    != budget.max_total_target_operations
            {
                return Err("fuzz resource policy does not match typed budget".into());
            }
            let dependencies = recipe
                .dependencies
                .iter()
                .map(|dependency| (dependency.role.as_str(), dependency.recipe.identity()))
                .collect::<BTreeMap<_, _>>();
            let mut ids = BTreeSet::new();
            for source in sources {
                if !ids.insert(source.seed_description_id)
                    || dependencies.get(source.dependency_role.as_str())
                        != Some(&source.recipe.identity())
                    || source.artifact_logical_id.is_empty()
                    || source.mutation_surfaces.is_empty()
                {
                    return Err("fuzz source declaration does not match a unique dependency".into());
                }
            }
            if ids.len() != dependencies.len() {
                return Err("fuzz dependencies and source declarations are not one-to-one".into());
            }
        }
        (
            EOT_ARITHMETIC_PLAN_PROVIDER_ID,
            QualificationParameters::CheckedEotU64Overflow {
                fragment_lengths,
                arithmetic_steps,
                expected_error,
            },
        ) => {
            if provider.qualification_provider_id != "encapsulation.checked_eot_arithmetic"
                || provider.payload_policy != PayloadPolicy::EvidenceOnly
                || qualification.retention != "evidence_only"
                || !recipe.dependencies.is_empty()
            {
                return Err("EOT qualification provider/retention contract does not agree".into());
            }
            if fragment_lengths != [u64::MAX]
                || arithmetic_steps
                    != [
                        "pad_fragment_to_even",
                        "add_item_header",
                        "accumulate_frame_offset",
                    ]
                || expected_error != "fragment_padding_overflow"
            {
                return Err(
                    "EOT overflow qualification does not declare the locked arithmetic boundary"
                        .into(),
                );
            }
            if qualification.resource_policy.max_input_bytes != 0
                || qualification.resource_policy.max_output_bytes != 0
                || qualification.resource_policy.max_operations != 3
            {
                return Err(
                    "EOT arithmetic resource policy must be payload-free and bounded".into(),
                );
            }
        }
        _ => return Err("qualification provider and typed parameters disagree".into()),
    }
    Ok(())
}

fn valid_tag(tag: &str) -> Result<(), String> {
    let Some((group, element)) = tag.split_once(',') else {
        return Err("tag must use GGGG,EEEE".into());
    };
    if group.len() != 4
        || element.len() != 4
        || u16::from_str_radix(group, 16).is_err()
        || u16::from_str_radix(element, 16).is_err()
    {
        return Err("tag must use uppercase hexadecimal GGGG,EEEE".into());
    }
    Ok(())
}

fn valid_fraction(numerator: u64, denominator: u64) -> Result<(), String> {
    if numerator == 0 || denominator == 0 || numerator >= denominator {
        Err("truncation fraction must be between zero and one".into())
    } else {
        Ok(())
    }
}
