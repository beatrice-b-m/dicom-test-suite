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

#[test]
fn manifest_schema_allows_non_image_files_and_requires_references() {
    let schema = read_json("schemas/manifest.schema.json");
    let file_required = schema
        .pointer("/$defs/file/required")
        .and_then(Value::as_array)
        .expect("manifest schema must define required file fields");

    assert!(
        file_required
            .iter()
            .any(|value| value.as_str() == Some("references")),
        "manifest file entries must include references"
    );
    for optional_field in ["image", "pixel_data"] {
        assert!(
            !file_required
                .iter()
                .any(|value| value.as_str() == Some(optional_field)),
            "manifest file entries must allow {optional_field} to be absent for non-image objects"
        );
        assert!(
            schema
                .pointer(&format!("/$defs/file/properties/{optional_field}/anyOf"))
                .and_then(Value::as_array)
                .is_some_and(|variants| variants
                    .iter()
                    .any(|variant| variant.get("type").and_then(Value::as_str) == Some("null"))),
            "manifest file entries must allow {optional_field} to be null"
        );
    }

    let reference_required = schema
        .pointer("/$defs/reference/required")
        .and_then(Value::as_array)
        .expect("manifest schema must define required reference fields");
    for field in [
        "relationship",
        "source_case_id",
        "source_path",
        "sop_class_uid",
        "sop_instance_uid",
    ] {
        assert!(
            reference_required
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "manifest references must require {field}"
        );
    }
}

#[test]
fn case_registry_schema_requires_the_specified_case_fields() {
    let schema = read_json("schemas/case-registry.schema.json");
    let required = schema
        .pointer("/$defs/case/required")
        .and_then(Value::as_array)
        .expect("case registry schema must define required case fields");

    for field in [
        "case_id",
        "status",
        "profiles",
        "recipe_id",
        "recipe_version",
        "iod_name",
        "sop_class_name",
        "sop_class_uid",
        "transfer_syntax_uid",
        "determinism",
        "requirements",
        "skip",
        "standards_evidence",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "case registry schema must require {field}"
        );
    }

    let statuses = schema
        .pointer("/$defs/status/enum")
        .and_then(Value::as_array)
        .expect("case registry schema must enumerate case statuses");

    for status in ["planned", "implemented", "skipped", "blocked", "deprecated"] {
        assert!(
            statuses.iter().any(|value| value.as_str() == Some(status)),
            "case registry schema must allow {status}"
        );
    }
}

#[test]
fn coverage_report_schema_requires_the_specified_matrix_fields() {
    let schema = read_json("schemas/coverage-report.schema.json");
    let required = schema
        .pointer("/$defs/coverage_row/required")
        .and_then(Value::as_array)
        .expect("coverage report schema must define required coverage row fields");

    for field in [
        "case_id",
        "profile",
        "status",
        "iod",
        "sop_class_uid",
        "transfer_syntax",
        "photometric",
        "bits",
        "frames",
        "geometry",
        "derived_refs",
        "validation_status",
        "determinism",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "coverage report schema must require {field}"
        );
    }

    let counts = schema
        .pointer("/$defs/counts/required")
        .and_then(Value::as_array)
        .expect("coverage report schema must define count fields");

    for field in ["generated", "skipped", "blocked", "planned", "deprecated"] {
        assert!(
            counts.iter().any(|value| value.as_str() == Some(field)),
            "coverage report schema must count {field} cases"
        );
    }
}

#[test]
fn viewer_report_schema_requires_viewer_compatibility_result_fields() {
    let schema = read_json("schemas/viewer-report.schema.json");
    let required = schema
        .pointer("/$defs/result/required")
        .and_then(Value::as_array)
        .expect("viewer report schema must define required result fields");

    for field in [
        "case_id",
        "path",
        "status",
        "file_open",
        "object_recognition",
        "metadata",
        "pixel_rendering",
        "timing",
        "errors",
        "warnings",
        "artifacts",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "viewer report schema must require {field}"
        );
    }
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let contents =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse {path:?} as JSON: {err}"))
}
