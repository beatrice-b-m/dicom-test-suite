use std::fs;

use serde_json::{Value, json};

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}")))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn assert_valid(schema_path: &str, fixture_path: &str) {
    let schema = read_json(schema_path);
    let fixture = read_json(fixture_path);
    let validator = jsonschema::validator_for(&schema).expect("schema must compile");
    let errors = validator
        .iter_errors(&fixture)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{fixture_path}:\n{}", errors.join("\n"));
}

fn assert_invalid(schema_path: &str, fixture_path: &str) {
    let schema = read_json(schema_path);
    let fixture = read_json(fixture_path);
    let validator = jsonschema::validator_for(&schema).expect("schema must compile");
    assert!(
        !validator.is_valid(&fixture),
        "{fixture_path} must be rejected"
    );
}

#[test]
fn composition_spec_positive_fixtures_validate() {
    for fixture in [
        "tests/fixtures/composition/valid/template-only.json",
        "tests/fixtures/composition/valid/typed-local-content.json",
    ] {
        assert_valid("schemas/composition-spec.schema.json", fixture);
    }
}

#[test]
fn composition_spec_negative_fixtures_are_rejected() {
    for fixture in [
        "tests/fixtures/composition/invalid/unknown-field.json",
        "tests/fixtures/composition/invalid/unsafe-path.json",
        "tests/fixtures/composition/invalid/conflicting-identity.json",
        "tests/fixtures/composition/invalid/malformed-tag-vr.json",
    ] {
        assert_invalid("schemas/composition-spec.schema.json", fixture);
    }
}

#[test]
fn composition_paths_reject_every_unsafe_form() {
    let schema = read_json("schemas/composition-spec.schema.json");
    let path_schema = schema
        .pointer("/$defs/safe_relative_path")
        .expect("path schema");
    let validator = jsonschema::validator_for(path_schema).expect("path schema compiles");

    for valid in ["assets/frame.raw", "frame.raw", "a/b/c.bin"] {
        assert!(validator.is_valid(&json!(valid)), "{valid} should be safe");
    }
    for invalid in [
        "",
        "/absolute.raw",
        "../outside.raw",
        "a/../outside.raw",
        "./frame.raw",
        "a/./frame.raw",
        "a\\frame.raw",
        "C:/frame.raw",
        "nul\0byte.raw",
    ] {
        assert!(
            !validator.is_valid(&json!(invalid)),
            "{invalid:?} must be unsafe"
        );
    }
}

#[test]
fn composition_spec_rejects_inline_large_bulk_data() {
    let schema = read_json("schemas/composition-spec.schema.json");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let fixture = json!({
        "composition_spec_schema_version": "0.1.0",
        "instances": [{
            "instance_id": "primary",
            "template": { "id": "classic/secondary-capture/monochrome" },
            "content": [{
                "slot": "pixels",
                "source": {
                    "kind": "inline_small_fixture",
                    "base64": "A".repeat(87385)
                }
            }]
        }]
    });
    assert!(!validator.is_valid(&fixture));
}

#[test]
fn template_catalog_positive_fixture_validates() {
    assert_valid(
        "schemas/template-catalog.schema.json",
        "tests/fixtures/composition/catalog/valid.json",
    );
}

#[test]
fn template_catalog_rejects_unknown_fields_and_missing_evidence() {
    let schema = read_json("schemas/template-catalog.schema.json");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let fixture = read_json("tests/fixtures/composition/catalog/valid.json");

    let mut unknown = fixture.clone();
    unknown["templates"][0]["unknown_policy"] = json!(true);
    assert!(!validator.is_valid(&unknown));

    let mut no_evidence = fixture.clone();
    no_evidence["templates"][0]["standards_evidence"] = json!([]);
    assert!(!validator.is_valid(&no_evidence));

    let mut unconditioned_type_1c = fixture;
    unconditioned_type_1c["templates"][0]["attributes"][0]["requirement"] = json!("1C");
    assert!(!validator.is_valid(&unconditioned_type_1c));
}
