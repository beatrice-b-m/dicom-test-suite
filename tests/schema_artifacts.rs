use std::fs;
use std::path::Path;

use serde_json::Value;

const SCHEMAS: &[(&str, &str)] = &[
    (
        "schemas/manifest.schema.json",
        "https://dicom-test-suite.local/schemas/manifest.schema.json",
    ),
    (
        "schemas/case-registry.schema.json",
        "https://dicom-test-suite.local/schemas/case-registry.schema.json",
    ),
    (
        "schemas/coverage-report.schema.json",
        "https://dicom-test-suite.local/schemas/coverage-report.schema.json",
    ),
    (
        "schemas/viewer-report.schema.json",
        "https://dicom-test-suite.local/schemas/viewer-report.schema.json",
    ),
];

#[test]
fn committed_schema_files_are_parseable_json_schema_documents() {
    for (path, id) in SCHEMAS {
        let schema = read_json(path);

        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "{path} must declare the JSON Schema draft"
        );
        assert_eq!(
            schema.get("$id").and_then(Value::as_str),
            Some(*id),
            "{path} must have a stable schema id"
        );
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("object"),
            "{path} must describe an object at the root"
        );
    }
}

#[test]
fn manifest_schema_requires_the_specified_top_level_sections() {
    let schema = read_json("schemas/manifest.schema.json");
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("manifest schema must have required fields");

    for field in [
        "manifest_schema_version",
        "generated_at",
        "generator",
        "standards",
        "dependencies",
        "run",
        "files",
        "skipped_cases",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "manifest schema must require {field}"
        );
    }

    assert_eq!(
        schema
            .pointer("/$defs/standards/properties/dicom_base_edition/const")
            .and_then(Value::as_str),
        Some("2026b"),
        "manifest standards metadata must stay aligned with standards.lock.json"
    );
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let contents =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse {path:?} as JSON: {err}"))
}
