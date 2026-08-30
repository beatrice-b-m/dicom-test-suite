//! Exact compatibility projection for plan-first expected-invalid artifacts.

use serde_json::{Value, json};

use crate::corpus_plan::{PlannedArtifact, PlannedMutationArtifact};
use crate::curated_plan::CuratedArtifactProjectionContext;
use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionCompatibilityInput};
use crate::executor::evidence::{ExecutionStatus, ResultStatus};
use crate::negative_plan::NEGATIVE_PARSER_RULE_ID;

use super::{CuratedManifestError, err, fail};

pub(super) fn project_file_entry(
    context: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    input: &ManifestProjectionCompatibilityInput,
) -> Result<Value, CuratedManifestError> {
    let PlannedArtifact::Mutation(planned) = &pair.planned else {
        return fail("negative projection received a non-mutation artifact");
    };
    validate_identity(context, pair, planned)?;
    let output = pair
        .execution
        .output
        .as_ref()
        .ok_or_else(|| err("negative mutation has no output evidence"))?;
    if pair.execution.status != ExecutionStatus::Succeeded
        || !output.publish
        || output.relative_path != planned.output.relative_path.as_str()
        || output.sha256 != planned.mutation.expected_output_sha256
    {
        return fail("negative output evidence differs from the mutation plan");
    }

    let source_pair = input
        .artifacts
        .iter()
        .find(|candidate| candidate.planned.logical_id() == planned.source_artifact_id)
        .ok_or_else(|| err("negative private source is absent"))?;
    let PlannedArtifact::Dicom(source) = &source_pair.planned else {
        return fail("negative private source is not a planned DICOM artifact");
    };
    let source_output = source_pair
        .execution
        .output
        .as_ref()
        .ok_or_else(|| err("negative private source has no execution evidence"))?;
    let source_binding = source
        .case_binding
        .as_ref()
        .ok_or_else(|| err("negative private source has no versioned case binding"))?;
    if source.output.publish
        || source_output.publish
        || source_pair.execution.status != ExecutionStatus::Succeeded
        || source_binding.case_id != planned.mutation.source_identity.case_id
        || source_binding.recipe_id != planned.mutation.source_identity.recipe_id
        || source_binding.recipe_version != planned.mutation.source_identity.recipe_version
        || source_output.sha256 != planned.mutation.expected_source_sha256
        || source_output.sha256 != planned.mutation.source_identity.expected_sha256
        || source_output.size_bytes != source.resources.output_bytes
    {
        return fail("negative private-source identity or hash differs from the plan");
    }

    let validation = pair
        .execution
        .validation
        .iter()
        .find(|result| result.rule_id == NEGATIVE_PARSER_RULE_ID)
        .ok_or_else(|| err("negative parser-probe validation evidence is absent"))?;
    if validation.status != ResultStatus::Passed
        || validation.details.get("ordinary_valid_dicom_validation") != Some(&Value::Bool(false))
    {
        return fail("negative artifact received ordinary valid-DICOM validation");
    }
    let probe = validation
        .details
        .get("probe")
        .cloned()
        .ok_or_else(|| err("negative parser-probe observation is absent"))?;
    let probe_outcome = probe
        .get("outcome")
        .and_then(Value::as_str)
        .ok_or_else(|| err("negative parser-probe outcome is invalid"))?;
    if !planned
        .mutation
        .acceptable_outcomes
        .iter()
        .any(|outcome| outcome == probe_outcome)
    {
        return fail("negative parser-probe outcome is not accepted by the plan");
    }
    let actual_steps = validation
        .details
        .get("ordered_mutation_steps")
        .ok_or_else(|| err("negative ordered materializer-step evidence is absent"))?;
    let expected_actual_steps = Value::Array(
        planned
            .mutation
            .operations
            .iter()
            .map(|operation| {
                json!({
                    "order": operation.order,
                    "operation_id": operation.operation_id,
                    "source_sha256": operation.expected_source_sha256,
                    "output_sha256": operation.expected_output_sha256,
                    "changed_byte_ranges": operation.changed_byte_ranges,
                    "expected_failure_layer": operation.expected_failure_layer,
                    "acceptable_outcomes": operation.acceptable_outcomes,
                })
            })
            .collect(),
    );
    if actual_steps != &expected_actual_steps {
        return fail("negative materializer-step evidence differs from the immutable plan");
    }

    let mutation_steps = planned
        .mutation
        .operations
        .iter()
        .map(|operation| {
            Ok(json!({
                "ordinal": operation.order + 1,
                "mutation_id": operation.operation_id,
                "parameters": compatibility_parameters(operation)?,
                "source_sha256": operation.expected_source_sha256,
                "output_sha256": operation.expected_output_sha256,
                "changed_byte_ranges": operation.changed_byte_ranges,
                "expected_failure_layer": operation.expected_failure_layer,
                "acceptable_outcomes": operation.acceptable_outcomes,
            }))
        })
        .collect::<Result<Vec<_>, CuratedManifestError>>()?;
    let mutation_operations = planned
        .mutation
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<Vec<_>>();

    Ok(json!({
        "case_id": context.registry_case.case_id,
        "profile_membership": context.registry_case.profiles,
        "path": output.relative_path,
        "sha256": output.sha256,
        "size_bytes": output.size_bytes,
        "determinism": context.registry_case.determinism,
        "recipe": {
            "recipe_id": context.case_recipe.recipe_id,
            "recipe_version": context.case_recipe.recipe_version,
            "recipe_parameters": {
                "source_case_id": source_binding.case_id,
                "mutation_operations": mutation_operations,
            }
        },
        "provider": {"id":"checked_part10_mutation","kind":"mutation_layer"},
        "validity": "expected_invalid",
        "negative_evidence": {
            "contract_version": planned.mutation.contract_version,
            "recipe_version": context.case_recipe.recipe_version,
            "source": {
                "case_id": source_binding.case_id,
                "sha256": source_output.sha256,
                "size_bytes": source_output.size_bytes,
                "transfer_syntax_uid": source.encoding.transfer_syntax_uid,
            },
            "source_shape": source_shape(planned)?,
            "mutation_steps": mutation_steps,
            "final_sha256": planned.mutation.expected_output_sha256,
            "probe": probe,
            "unacceptable_outcomes": ["timeout", "crash", "hang"],
        },
        "standards_evidence": context.registry_case.standards_evidence,
        "references": [],
    }))
}

fn validate_identity(
    context: &CuratedArtifactProjectionContext,
    pair: &ManifestProjectionArtifact,
    planned: &PlannedMutationArtifact,
) -> Result<(), CuratedManifestError> {
    if planned.logical_id != context.artifact_id
        || planned.order != context.plan_order
        || pair.execution.logical_id != context.artifact_id
        || planned.case_binding.case_id != context.case_recipe.binding.case_id
        || planned.case_binding.recipe_id != context.case_recipe.recipe_id
        || planned.case_binding.recipe_version != context.case_recipe.recipe_version
    {
        return fail("negative projection identity differs from the planned recipe");
    }
    Ok(())
}

fn source_shape(planned: &PlannedMutationArtifact) -> Result<&'static str, CuratedManifestError> {
    let operations = planned
        .mutation
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<Vec<_>>();
    match operations.as_slice() {
        [
            "invalid_character_set_declaration",
            "malformed_encoded_text",
        ] => Ok("Explicit VR LE SC with Specific Character Set and non-empty Person Name"),
        ["invalid_nested_item_length"] => {
            Ok("Explicit VR LE SC with at least one nested Sequence Item")
        }
        ["truncate_dataset"] => Ok("native Explicit VR LE SC with top-level Pixel Data"),
        ["truncate_item"] => Ok("Explicit VR LE SC with a non-empty nested Sequence Item"),
        ["undefined_length_without_delimitation"] => {
            Ok("Explicit VR LE SC with a delimited undefined-length Sequence")
        }
        ["broken_extended_offset_table"] => Ok("RLE SC with a non-empty Extended Offset Table"),
        ["truncate_fragment"] => Ok("RLE SC with at least one non-empty Pixel Data fragment"),
        ["incorrect_explicit_vr_length"] | ["illegal_vr_bytes"] => {
            Ok("Explicit VR LE SC with Person Name")
        }
        ["transfer_syntax_mismatch"] => {
            Ok("native Explicit VR LE SC whose TS UID value can hold the RLE UID")
        }
        [
            "file_meta_dataset_uid_mismatch",
            "file_meta_dataset_uid_mismatch",
        ] => Ok("Explicit VR LE SC with dataset SOP Class and Instance UIDs"),
        ["missing_type_1_element"] => Ok("Explicit VR LE SC with top-level Type 1 Modality"),
        ["truncate_file_meta"] => {
            Ok("Explicit VR LE SC with complete Transfer Syntax UID file meta")
        }
        ["invalid_bits_stored_high_bit", "invalid_pixel_byte_length"] => {
            Ok("native Explicit VR LE SC with Bits Stored, High Bit, and defined Pixel Data")
        }
        ["truncate_pixel_value"] => {
            Ok("native Explicit VR LE SC with non-empty defined Pixel Data")
        }
        _ => fail("negative mutation operation set has no typed source-shape projection"),
    }
}

fn compatibility_parameters(
    operation: &crate::corpus_plan::PlannedMutationOperation,
) -> Result<Value, CuratedManifestError> {
    let range = |index: usize| {
        operation
            .changed_byte_ranges
            .get(index)
            .map(|range| json!(range.source))
            .ok_or_else(|| err("negative compatibility parameter lacks a changed range"))
    };
    let offset = || {
        operation
            .changed_byte_ranges
            .first()
            .map(|range| range.source.start)
            .ok_or_else(|| err("negative truncation lacks a changed range"))
    };
    let mut parameters = serde_json::Map::from_iter(operation.parameters.clone());
    match operation.operation_id.as_str() {
        "invalid_character_set_declaration" | "malformed_encoded_text" => {
            parameters.insert("value".into(), range(0)?);
        }
        "invalid_nested_item_length"
        | "incorrect_explicit_vr_length"
        | "invalid_pixel_byte_length" => {
            parameters.insert("length_field".into(), range(0)?);
        }
        "truncate_dataset" => {
            parameters.insert("target".into(), json!("dataset"));
            parameters.insert("offset".into(), json!(offset()?));
        }
        "truncate_item" => {
            parameters.insert("target".into(), json!("item"));
            parameters.insert("offset".into(), json!(offset()?));
        }
        "undefined_length_without_delimitation" => {
            parameters.insert("length_field".into(), Value::Null);
            parameters.insert("delimitation_item".into(), range(0)?);
        }
        "broken_extended_offset_table" => {
            parameters.insert("entry".into(), range(0)?);
        }
        "truncate_fragment" => {
            parameters.insert("target".into(), json!("fragment"));
            parameters.insert("offset".into(), json!(offset()?));
        }
        "illegal_vr_bytes" => {
            parameters.insert("vr_field".into(), range(0)?);
        }
        "transfer_syntax_mismatch" => {
            parameters.insert("file_meta_uid_value".into(), range(0)?);
        }
        "file_meta_dataset_uid_mismatch" => {
            parameters.insert("dataset_uid_value".into(), range(0)?);
        }
        "missing_type_1_element" => {
            parameters.insert("element".into(), range(0)?);
        }
        "truncate_file_meta" => {
            parameters.insert("target".into(), json!("file_meta"));
            parameters.insert("offset".into(), json!(offset()?));
        }
        "invalid_bits_stored_high_bit" => {
            parameters.insert("bits_stored_value".into(), range(0)?);
            parameters.insert("high_bit_value".into(), range(1)?);
        }
        "truncate_pixel_value" => {
            parameters.insert("target".into(), json!("pixel_value"));
            parameters.insert("offset".into(), json!(offset()?));
        }
        other => {
            return fail(format!(
                "unsupported negative compatibility operation {other}"
            ));
        }
    }
    Ok(Value::Object(parameters))
}
