use std::fs;

use serde_json::{Value, json};

fn read(path: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("fixture should be readable"))
        .expect("fixture should be valid JSON")
}

fn assert_valid(schema_path: &str, instance: &Value) {
    let schema = read(schema_path);
    let validator = jsonschema::validator_for(&schema).expect("schema should compile");
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
}

fn assert_invalid(schema_path: &str, instance: &Value) {
    let schema = read(schema_path);
    let validator = jsonschema::validator_for(&schema).expect("schema should compile");
    assert!(!validator.is_valid(instance));
}

#[test]
fn committed_conformance_examples_match_full_schemas() {
    assert_valid(
        "schemas/conformance-run.schema.json",
        &read("tests/conformance/fixtures/minimal-run.json"),
    );
    assert_valid(
        "schemas/conformance-accepted-findings.schema.json",
        &read("tests/conformance/fixtures/minimal-accepted-findings.json"),
    );
    assert_valid(
        "schemas/conformance-accepted-findings.schema.json",
        &read("conformance/accepted-findings.json"),
    );
}

#[test]
fn run_schema_rejects_unknown_fields_missing_fingerprints_and_severities() {
    let mut run = read("tests/conformance/fixtures/minimal-run.json");
    run["unexpected"] = json!(true);
    assert_invalid("schemas/conformance-run.schema.json", &run);

    let mut run = read("tests/conformance/fixtures/minimal-run.json");
    run["source"]
        .as_object_mut()
        .unwrap()
        .remove("manifest_sha256");
    assert_invalid("schemas/conformance-run.schema.json", &run);

    let mut run = read("tests/conformance/fixtures/minimal-run.json");
    run["entity"]["findings"][0]["severity"] = json!("fatal");
    assert_invalid("schemas/conformance-run.schema.json", &run);
}

#[test]
fn run_schema_locks_the_entity_projection_contract() {
    let mut run = read("tests/conformance/fixtures/minimal-run.json");
    run["entity"]["input_projection"] = json!({
        "method": "terminal_pixel_data_element_redaction_v1",
        "scope": "entity_consistency_only",
        "file_list": { "path": "entity/files.txt", "sha256": "a".repeat(64) },
        "entries": [{
            "source_case_id": "classic/sc/mono2_u32_explicit_le",
            "source_path": "classic/sc/mono2_u32_explicit_le/instance.dcm",
            "source_copy": { "path": "entity/projections/u32.source.dcm", "sha256": "b".repeat(64) },
            "projected_input": { "path": "entity/projections/u32.projected.dcm", "sha256": "c".repeat(64) },
            "transfer_syntax_uid": "1.2.840.10008.1.2.1",
            "removed_element": {
                "tag": "(7FE0,0010)", "vr": "OW", "element_offset": 930,
                "value_offset": 942, "value_length": 16, "value_sha256": "d".repeat(64)
            }
        }]
    });
    assert_valid("schemas/conformance-run.schema.json", &run);

    let mut unknown_method = run.clone();
    unknown_method["entity"]["input_projection"]["method"] = json!("generic_redaction");
    assert_invalid("schemas/conformance-run.schema.json", &unknown_method);

    let mut wrong_role = run.clone();
    wrong_role["entity"]["role"] = json!("primary_iod_validator");
    assert_invalid("schemas/conformance-run.schema.json", &wrong_role);

    run["entity"]["input_projection"]["entries"][0]["unexpected"] = json!(true);
    assert_invalid("schemas/conformance-run.schema.json", &run);
}

#[test]
fn allowlist_schema_rejects_broad_entries_and_incomplete_reviews() {
    let base = json!({
        "schema_version": "0.1.0",
        "findings": [{
            "validator_adapter_id": "dicom3tools-dciodvfy",
            "validator_fingerprint": "a".repeat(64),
            "case_id": "classic/sc/example",
            "path": "classic/sc/example/instance.dcm",
            "rule_id": "Type1AttributeMissing",
            "message_fingerprint": "b".repeat(64),
            "original_severity": "warning",
            "disposition": "validator_limitation",
            "rationale": "The pinned validator lacks this edition definition.",
            "citation": "PS3.3 C.7.1",
            "reviewer": "Conformance reviewer",
            "review_date": "2026-08-26",
            "recheck_condition": "Recheck when validator definitions change"
        }]
    });
    assert_valid("schemas/conformance-accepted-findings.schema.json", &base);

    let mut wildcard = base.clone();
    wildcard["findings"][0]["case_id"] = json!("classic/*");
    assert_invalid(
        "schemas/conformance-accepted-findings.schema.json",
        &wildcard,
    );

    let mut missing_fingerprint = base;
    missing_fingerprint["findings"][0]
        .as_object_mut()
        .unwrap()
        .remove("validator_fingerprint");
    assert_invalid(
        "schemas/conformance-accepted-findings.schema.json",
        &missing_fingerprint,
    );
}
