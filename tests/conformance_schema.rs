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
