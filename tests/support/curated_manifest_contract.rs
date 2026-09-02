use std::fs;

use serde_json::Value;

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}")))
        .unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn validation_errors(manifest: &Value) -> Vec<String> {
    let version = manifest
        .get("manifest_schema_version")
        .and_then(Value::as_str)
        .expect("curated manifest version must be a string");
    let validator = match version {
        "0.2.0" | "0.3.0" => jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&read_json("schemas/manifest.schema.json"))
            .expect("frozen curated manifest schema must compile"),
        "1.0.0" => jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .with_resource(
                "https://dicom-test-suite.local/schemas/manifest.schema.json",
                jsonschema::Resource::from_contents(read_json("schemas/manifest.schema.json"))
                    .expect("frozen manifest schema resource"),
            )
            .with_resource(
                "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json",
                jsonschema::Resource::from_contents(read_json(
                    "schemas/version-result-v2.schema.json",
                ))
                .expect("identity schema resource"),
            )
            .build(&read_json("schemas/manifest-v1.schema.json"))
            .expect("curated manifest v1 schema must compile"),
        other => panic!("unsupported curated manifest fixture version {other}"),
    };
    validator
        .iter_errors(manifest)
        .map(|error| error.to_string())
        .collect()
}

pub fn assert_curated_manifest_schema_valid(manifest: &Value) {
    let errors = validation_errors(manifest);
    assert!(errors.is_empty(), "manifest schema errors: {errors:?}");
}

pub fn curated_manifest_schema_is_valid(manifest: &Value) -> bool {
    validation_errors(manifest).is_empty()
}

pub fn assert_curated_manifest_schema_rejected(manifest: &Value) {
    assert!(
        !validation_errors(manifest).is_empty(),
        "tampered manifest unexpectedly satisfied its versioned schema"
    );
}
