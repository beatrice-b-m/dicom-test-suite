#![recursion_limit = "256"]

use std::fs;
use std::path::Path;

use serde_json::Value;

const SCHEMAS: &[(&str, &str)] = &[
    (
        "schemas/assembly-request.schema.json",
        "https://dicom-test-suite.local/schemas/assembly-request.schema.json",
    ),
    (
        "schemas/assembly-result.schema.json",
        "https://dicom-test-suite.local/schemas/assembly-result.schema.json",
    ),
    (
        "schemas/corpus-definition-bundle.schema.json",
        "https://synth-dicom-gen.local/schemas/corpus-definition-bundle.schema.json",
    ),
    (
        "schemas/version-result.schema.json",
        "https://dicom-test-suite.local/schemas/version-result.schema.json",
    ),
    (
        "schemas/version-result-v2.schema.json",
        "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json",
    ),
    (
        "schemas/capabilities-result.schema.json",
        "https://dicom-test-suite.local/schemas/capabilities-result.schema.json",
    ),
    (
        "schemas/capabilities-result-v2.schema.json",
        "https://synth-dicom-gen.local/schemas/capabilities-result-v2.schema.json",
    ),
    (
        "schemas/generation-result.schema.json",
        "https://dicom-test-suite.local/schemas/generation-result.schema.json",
    ),
    (
        "schemas/generation-result-v2.schema.json",
        "https://synth-dicom-gen.local/schemas/generation-result-v2.schema.json",
    ),
    (
        "schemas/composition-result.schema.json",
        "https://dicom-test-suite.local/schemas/composition-result.schema.json",
    ),
    (
        "schemas/composition-manifest-v1.schema.json",
        "https://synth-dicom-gen.local/schemas/composition-manifest-v1.schema.json",
    ),
    (
        "schemas/templates-result.schema.json",
        "https://dicom-test-suite.local/schemas/templates-result.schema.json",
    ),
    (
        "schemas/validation-result.schema.json",
        "https://dicom-test-suite.local/schemas/validation-result.schema.json",
    ),
    (
        "schemas/report-result.schema.json",
        "https://dicom-test-suite.local/schemas/report-result.schema.json",
    ),
    (
        "schemas/case-list-result.schema.json",
        "https://dicom-test-suite.local/schemas/case-list-result.schema.json",
    ),
    (
        "schemas/case-recipe.schema.json",
        "https://dicom-test-suite.local/schemas/case-recipe.schema.json",
    ),
    (
        "schemas/standards-result.schema.json",
        "https://dicom-test-suite.local/schemas/standards-result.schema.json",
    ),
    (
        "schemas/conformance-result.schema.json",
        "https://dicom-test-suite.local/schemas/conformance-result.schema.json",
    ),
    (
        "schemas/interoperability-result.schema.json",
        "https://dicom-test-suite.local/schemas/interoperability-result.schema.json",
    ),
    (
        "schemas/cli-success-envelope.schema.json",
        "https://dicom-test-suite.local/schemas/cli-success-envelope.schema.json",
    ),
    (
        "schemas/cli-error-envelope.schema.json",
        "https://dicom-test-suite.local/schemas/cli-error-envelope.schema.json",
    ),
    (
        "schemas/cli-error-code-registry.schema.json",
        "https://dicom-test-suite.local/schemas/cli-error-code-registry.schema.json",
    ),
    (
        "schemas/composition-spec.schema.json",
        "https://dicom-test-suite.local/schemas/composition-spec.schema.json",
    ),
    (
        "schemas/composition-manifest.schema.json",
        "https://dicom-test-suite.local/schemas/composition-manifest.schema.json",
    ),
    (
        "schemas/composition-provider-request.schema.json",
        "https://dicom-test-suite.local/schemas/composition-provider-request.schema.json",
    ),
    (
        "schemas/composition-provider-response.schema.json",
        "https://dicom-test-suite.local/schemas/composition-provider-response.schema.json",
    ),
    (
        "schemas/template-catalog.schema.json",
        "https://dicom-test-suite.local/schemas/template-catalog.schema.json",
    ),
    (
        "schemas/conformance-run.schema.json",
        "https://dicom-test-suite.local/schemas/conformance-run.schema.json",
    ),
    (
        "schemas/conformance-accepted-findings.schema.json",
        "https://dicom-test-suite.local/schemas/conformance-accepted-findings.schema.json",
    ),
    (
        "schemas/manifest.schema.json",
        "https://dicom-test-suite.local/schemas/manifest.schema.json",
    ),
    (
        "schemas/manifest-v1.schema.json",
        "https://synth-dicom-gen.local/schemas/manifest-v1.schema.json",
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
        "schemas/coverage-gap-report.schema.json",
        "https://dicom-test-suite.local/schemas/coverage-gap-report.schema.json",
    ),
    (
        "schemas/generation-backend-request.schema.json",
        "https://dicom-test-suite.local/schemas/generation-backend-request.schema.json",
    ),
    (
        "schemas/generation-backend-response.schema.json",
        "https://dicom-test-suite.local/schemas/generation-backend-response.schema.json",
    ),
    (
        "schemas/generation-backend-lock.schema.json",
        "https://dicom-test-suite.local/schemas/generation-backend-lock.schema.json",
    ),
    (
        "schemas/viewer-report.schema.json",
        "https://dicom-test-suite.local/schemas/viewer-report.schema.json",
    ),
    (
        "schemas/media-report.schema.json",
        "https://dicom-test-suite.local/schemas/media-report.schema.json",
    ),
    (
        "schemas/release-manifest.schema.json",
        "https://dicom-test-suite.invalid/schemas/release-manifest.schema.json",
    ),
    (
        "schemas/structural-assembly-manifest.schema.json",
        "https://dicom-test-suite.local/schemas/structural-assembly-manifest.schema.json",
    ),
    (
        "schemas/structural-assembly-report.schema.json",
        "https://dicom-test-suite.local/schemas/structural-assembly-report.schema.json",
    ),
    (
        "schemas/transaction-report.schema.json",
        "https://dicom-test-suite.local/schemas/transaction-report.schema.json",
    ),
];

#[test]
fn committed_schema_files_are_parseable_json_schema_documents() {
    let mut committed_paths = fs::read_dir("schemas")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>();
    committed_paths.sort();
    let mut inventoried_paths = SCHEMAS
        .iter()
        .map(|(path, _)| (*path).to_string())
        .collect::<Vec<_>>();
    inventoried_paths.sort();
    assert_eq!(
        inventoried_paths, committed_paths,
        "stable schema ID inventory must cover every committed schema"
    );

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

    assert_renamed_product_reader_compatibility();
}

fn assert_renamed_product_reader_compatibility() {
    let cases = [
        (
            "schemas/manifest.schema.json",
            "/$defs/generator/properties/name",
        ),
        (
            "schemas/version-result.schema.json",
            "/properties/product/properties/name",
        ),
        (
            "schemas/release-manifest.schema.json",
            "/properties/product/properties/name",
        ),
        (
            "schemas/structural-assembly-manifest.schema.json",
            "/properties/generator/properties/name",
        ),
    ];

    for (path, pointer) in cases {
        let schema = read_json(path);
        let name_schema = schema
            .pointer(pointer)
            .unwrap_or_else(|| panic!("{path} lacks identity reader schema at {pointer}"));
        let identity_schema = serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {"name": name_schema},
            "additionalProperties": false
        });
        let validator = jsonschema::validator_for(&identity_schema)
            .unwrap_or_else(|error| panic!("{path} identity reader must compile: {error}"));

        let current_document = serde_json::json!({"name": "synth-dicom-gen"});
        validator
            .validate(&current_document)
            .unwrap_or_else(|error| {
                panic!("{path} must accept the new producer identity: {error}")
            });

        let mut historical_document = current_document.clone();
        historical_document["name"] = Value::String("dicom-test-suite".into());
        validator
            .validate(&historical_document)
            .unwrap_or_else(|error| {
                panic!("{path} must retain the supported historical reader identity: {error}")
            });

        let mut unrelated_document = current_document;
        unrelated_document["name"] = Value::String("unrelated-generator".into());
        assert!(
            validator.validate(&unrelated_document).is_err(),
            "{path} must not turn additive rename compatibility into an open product name"
        );
    }
}

#[test]
fn committed_schema_files_compile() {
    let version_v2 =
        jsonschema::Resource::from_contents(read_json("schemas/version-result-v2.schema.json"))
            .expect("version v2 schema resource");
    let legacy_manifest =
        jsonschema::Resource::from_contents(read_json("schemas/manifest.schema.json"))
            .expect("legacy manifest schema resource");
    for (path, _) in SCHEMAS {
        let schema = read_json(path);
        jsonschema::options()
            .with_resource(
                "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json",
                version_v2.clone(),
            )
            .with_resource(
                "https://dicom-test-suite.local/schemas/manifest.schema.json",
                legacy_manifest.clone(),
            )
            .build(&schema)
            .unwrap_or_else(|error| panic!("{path} must compile as JSON Schema: {error}"));
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

    let valid_file_required = schema
        .pointer("/$defs/file/allOf/0/else/required")
        .and_then(Value::as_array)
        .expect("valid manifest files must retain their conditional requirements");
    assert!(
        valid_file_required
            .iter()
            .any(|value| value.as_str() == Some("references")),
        "valid manifest file entries must include references"
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
    let uid_required = schema
        .pointer("/$defs/uids/required")
        .and_then(Value::as_array)
        .expect("manifest schema must define required UID fields");
    assert!(
        !uid_required
            .iter()
            .any(|value| value.as_str() == Some("frame_of_reference_uid")),
        "manifest UID blocks must allow non-image objects without Frame of Reference UID"
    );

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
fn manifest_schema_rejects_unsafe_file_paths() {
    let schema = read_json("schemas/manifest.schema.json");
    let path_schema = schema
        .pointer("/$defs/file/properties/path")
        .expect("manifest file path schema");
    let validator = jsonschema::validator_for(path_schema).expect("file path schema must compile");

    assert!(validator.is_valid(&Value::String(
        "classic/sc/example/instance.dcm".to_string()
    )));
    for unsafe_path in [
        "",
        "/absolute/instance.dcm",
        "../sibling/instance.dcm",
        "classic/../sibling/instance.dcm",
        "classic\\instance.dcm",
        "C:/instance.dcm",
    ] {
        assert!(
            !validator.is_valid(&Value::String(unsafe_path.to_string())),
            "manifest schema must reject {unsafe_path}"
        );
    }
}

#[test]
fn manifest_schema_defines_encapsulated_pixel_data_layout_metadata() {
    let schema = read_json("schemas/manifest.schema.json");

    assert!(
        schema
            .pointer("/$defs/pixel_data/allOf")
            .and_then(Value::as_array)
            .is_some_and(|rules| !rules.is_empty()),
        "encapsulated pixel_data entries must require layout metadata"
    );

    let required = schema
        .pointer("/$defs/encapsulated_pixel_data/required")
        .and_then(Value::as_array)
        .expect("manifest schema must define encapsulated Pixel Data fields");
    for field in [
        "basic_offset_table",
        "fragments_per_frame",
        "extended_offset_table",
        "compressed_frame_hashes",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "encapsulated Pixel Data metadata must require {field}"
        );
    }

    assert_eq!(
        schema
            .pointer("/$defs/encapsulated_pixel_data/properties/basic_offset_table/properties/present/const")
            .and_then(Value::as_bool),
        Some(true),
        "encapsulated Pixel Data always starts with a Basic Offset Table item"
    );
}

#[test]
fn manifest_schema_requires_exact_extended_offset_table_arrays() {
    let schema = read_json("schemas/manifest.schema.json");
    let eot_schema = schema
        .pointer("/$defs/encapsulated_pixel_data/properties/extended_offset_table")
        .expect("manifest schema must define Extended Offset Table metadata");
    let validator = jsonschema::validator_for(eot_schema)
        .expect("Extended Offset Table metadata schema must compile");

    assert!(validator.is_valid(&serde_json::json!({
        "present": true,
        "lengths_present": true,
        "offset_count": 3,
        "length_count": 3,
        "offsets": [0, 78, 152],
        "lengths": [69, 66, 69]
    })));
    assert!(!validator.is_valid(&serde_json::json!({
        "present": true,
        "lengths_present": true,
        "offset_count": 3,
        "length_count": 3
    })));
    assert!(!validator.is_valid(&serde_json::json!({
        "present": false,
        "lengths_present": false,
        "offset_count": 0,
        "length_count": 0,
        "offsets": [],
        "lengths": []
    })));

    let expected_eot = schema
        .pointer("/$defs/expected_eot")
        .expect("manifest schema must define the exact EOT oracle");
    let validator =
        jsonschema::validator_for(expected_eot).expect("expected EOT schema must compile");
    assert!(validator.is_valid(&serde_json::json!({
        "origin": "first_fragment_item_tag",
        "item_header_bytes": 8,
        "frame_encoded_lengths": [69, 66, 69],
        "offsets": [0, 78, 152],
        "lengths": [69, 66, 69]
    })));
}

#[test]
fn manifest_schema_locks_approved_lossy_metric_oracles() {
    let schema = read_json("schemas/manifest.schema.json");
    let metric_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": schema["$defs"].clone(),
        "$ref": "#/$defs/expected_lossy_metrics"
    });
    let validator =
        jsonschema::validator_for(&metric_schema).expect("lossy metric oracle schema must compile");
    let jxl = serde_json::json!({
        "sample_domain": "unsigned_8_bit",
        "sample_order": "interleaved_by_pixel",
        "sample_count": 3072,
        "dimensions": { "rows": 32, "columns": 32, "frames": 1 },
        "channels": [
            { "index": 0, "name": "R", "sample_count": 1024, "max_absolute_error": { "observed": 6, "limit": 8 }, "rmse": { "observed": 1.5, "limit": 3 } },
            { "index": 1, "name": "G", "sample_count": 1024, "max_absolute_error": { "observed": 7, "limit": 8 }, "rmse": { "observed": 1.7, "limit": 3 } },
            { "index": 2, "name": "B", "sample_count": 1024, "max_absolute_error": { "observed": 8, "limit": 8 }, "rmse": { "observed": 1.9, "limit": 3 } }
        ],
        "encoder": {
            "id": "cjxl_jpegxl_lossy_encoder",
            "version": "0.11.2",
            "executable_sha256": "5b7b6cdc09a1bdaef39e30d3660e29861a405fffc1bc1136f3bb91cfe6db658e",
            "options": { "input_format": "binary_ppm_rgb8", "argument_vector": ["--distance=0.05"], "distance": 0.05, "effort": 7, "num_threads": 0, "container": false, "modular": false },
            "options_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        },
        "overall_rmse": { "observed": 1.7, "limit": 3 },
        "uncompressed_bytes": 3072,
        "compressed_bytes": 768,
        "compression_ratio": { "numerator": 3072, "denominator": 768, "computed": 4, "dicom_value": "4.0" },
        "lossy_image_compression": "01",
        "lossy_image_compression_method": "ISO_18181_1",
        "decoder": {
            "id": "dicom_rs_jxl_oxide_decoder",
            "version": "dicom-transfer-syntax-registry 0.9.1 + jxl-oxide 0.10.2",
            "independence": "independent"
        }
    });
    assert!(validator.is_valid(&jxl));

    let mut undersized = jxl.clone();
    undersized["dimensions"]["rows"] = serde_json::json!(31);
    assert!(!validator.is_valid(&undersized));
    let mut same_encoder_decoder = jxl;
    same_encoder_decoder["decoder"]["independence"] = serde_json::json!("same_implementation");
    assert!(!validator.is_valid(&same_encoder_decoder));

    let lossy_rule = schema
        .pointer("/$defs/file/allOf/2")
        .expect("manifest files must case-scope lossy metrics");
    assert_eq!(
        lossy_rule
            .pointer("/then/allOf/0/then/properties/expected_lossy_metrics/properties/overall_rmse/properties/limit/const")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        lossy_rule
            .pointer("/then/allOf/0/else/properties/expected_lossy_metrics/properties/overall_rmse/properties/limit/const")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        schema
            .pointer("/$defs/jxl_red_lossy_channel/allOf/1/properties/max_absolute_error/properties/limit/const")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        schema
            .pointer("/$defs/htj2k_mono_lossy_channel/allOf/1/properties/max_absolute_error/properties/limit/const")
            .and_then(Value::as_u64),
        Some(64)
    );
    assert_eq!(
        schema
            .pointer(
                "/$defs/jxl_lossy_encoder/allOf/1/properties/options/properties/distance/const"
            )
            .and_then(Value::as_f64),
        Some(0.05)
    );
    assert_eq!(
        schema
            .pointer("/$defs/htj2k_lossy_encoder/allOf/1/properties/options/properties/qstep/const")
            .and_then(Value::as_f64),
        Some(0.00025)
    );
    assert!(
        lossy_rule
            .pointer("/else/not/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required == &[serde_json::json!("expected_lossy_metrics")]),
        "unapproved cases must not claim a lossy metric oracle"
    );
}

fn negative_manifest_file_fixture() -> Value {
    serde_json::json!({
        "case_id": "negative/encoding/illegal_vr_bytes",
        "profile_membership": ["negative"],
        "path": "negative/encoding/illegal_vr_bytes/instance.dcm",
        "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "size_bytes": 511,
        "determinism": "byte_stable",
        "validity": "expected_invalid",
        "provider": { "kind": "mutation_layer", "id": "checked_part10_mutation" },
        "recipe": {
            "recipe_id": "negative_encoding_illegal_vr_bytes",
            "recipe_version": "0.1.0",
            "recipe_parameters": {}
        },
        "negative_evidence": {
            "contract_version": "0.1.0",
            "recipe_version": "0.1.0",
            "source": {
                "case_id": "classic/sc/mono2_u8_explicit_le",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "size_bytes": 512
            },
            "source_shape": "Explicit VR Little Endian Part 10 with a located short-VR field",
            "mutation_steps": [{
                "ordinal": 1,
                "mutation_id": "illegal_vr_bytes",
                "parameters": {
                    "vr_field": { "start": 300, "end": 302 },
                    "replacement": [90, 90],
                    "length_field": null
                },
                "changed_byte_ranges": [{
                    "source": { "start": 300, "end": 302 },
                    "output": { "start": 300, "end": 302 }
                }],
                "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "output_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "expected_failure_layer": "dataset_parser",
                "acceptable_outcomes": ["clean_rejection", "parse_failure"]
            }],
            "probe": {
                "kind": "same_project_bounded_parser_classifier",
                "independence": "same_project",
                "outcome": "parse_failure",
                "detail": "The bounded Part 10 locator rejected illegal VR bytes."
            },
            "unacceptable_outcomes": ["timeout", "crash", "hang"],
            "final_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        },
        "standards_evidence": []
    })
}

#[test]
fn manifest_schema_separates_expected_invalid_files_from_valid_dicom_contracts() {
    let schema = read_json("schemas/manifest.schema.json");
    let file_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/file",
        "$defs": schema["$defs"].clone()
    });
    let validator =
        jsonschema::validator_for(&file_schema).expect("manifest file schema should compile");
    let negative = negative_manifest_file_fixture();
    let errors = validator
        .iter_errors(&negative)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "expected-invalid files must validate from mutation evidence without valid-DICOM fields:\n{}",
        errors.join("\n")
    );

    let mut missing_validity = negative.clone();
    missing_validity
        .as_object_mut()
        .expect("fixture object")
        .remove("validity");
    assert!(
        !validator.is_valid(&missing_validity),
        "validity absent must retain the legacy valid-DICOM requirements"
    );

    let mut false_dicom_claim = negative.clone();
    false_dicom_claim["dicom"] = serde_json::json!({});
    assert!(
        !validator.is_valid(&false_dicom_claim),
        "expected-invalid files must not carry valid-DICOM identity claims"
    );

    let mut unsafe_outcomes = negative.clone();
    unsafe_outcomes["negative_evidence"]["unacceptable_outcomes"] =
        serde_json::json!(["timeout", "crash"]);
    assert!(
        !validator.is_valid(&unsafe_outcomes),
        "timeout, crash, and hang must all remain explicitly unacceptable"
    );

    let mut incomplete_ranges = negative;
    incomplete_ranges["negative_evidence"]["mutation_steps"][0]["changed_byte_ranges"][0]["output"] =
        Value::Null;
    assert!(
        !validator.is_valid(&incomplete_ranges),
        "each changed range must preserve exact source and output half-open ranges"
    );

    let mut empty_parameters = negative_manifest_file_fixture();
    empty_parameters["negative_evidence"]["mutation_steps"][0]["parameters"] =
        serde_json::json!({});
    assert!(
        !validator.is_valid(&empty_parameters),
        "mutation parameters must identify the exact bounded operation"
    );
}

fn negative_coverage_row_fixture() -> Value {
    serde_json::json!({
        "case_id": "negative/encoding/illegal_vr_bytes",
        "profile": "negative",
        "status": "generated",
        "validity": "expected_invalid",
        "path": "negative/encoding/illegal_vr_bytes/instance.dcm",
        "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "size_bytes": 511,
        "determinism": "byte_stable",
        "provider_kind": "mutation_layer",
        "provider_id": "checked_part10_mutation",
        "recipe_id": "negative_encoding_illegal_vr_bytes",
        "recipe_version": "0.1.0",
        "contract_version": "0.1.0",
        "source_case_id": "classic/sc/mono2_u8_explicit_le",
        "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "source_transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "source_size_bytes": 512,
        "source_shape": "Explicit VR Little Endian Part 10 with a located short-VR field",
        "mutation_ids": ["illegal_vr_bytes"],
        "mutation_count": 1,
        "expected_failure_layers": ["dataset_parser"],
        "acceptable_outcomes": ["clean_rejection", "parse_failure"],
        "probe_kind": "same_project_bounded_parser_classifier",
        "probe_independence": "same_project",
        "probe_detail": "The bounded Part 10 locator rejected illegal VR bytes.",
        "observed_outcome": "parse_failure",
        "outcome_status": "acceptable",
        "unacceptable_outcomes": ["timeout", "crash", "hang"],
        "final_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    })
}

#[test]
fn coverage_report_schema_projects_negative_outcomes_separately() {
    let schema = read_json("schemas/coverage-report.schema.json");
    let negative_row_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/negative_coverage_row",
        "$defs": schema["$defs"].clone()
    });
    let validator = jsonschema::validator_for(&negative_row_schema)
        .expect("negative coverage row schema should compile");
    let row = negative_coverage_row_fixture();
    assert!(validator.is_valid(&row));

    let mut unsafe_status = row.clone();
    unsafe_status["observed_outcome"] = Value::String("timeout".to_string());
    assert!(
        !validator.is_valid(&unsafe_status),
        "timeouts must never be reported as acceptable outcomes"
    );
    unsafe_status["outcome_status"] = Value::String("unacceptable".to_string());
    assert!(validator.is_valid(&unsafe_status));

    let valid_row_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/coverage_row",
        "$defs": schema["$defs"].clone()
    });
    let valid_row_validator = jsonschema::validator_for(&valid_row_schema)
        .expect("valid coverage row schema should compile");
    assert!(
        !valid_row_validator.is_valid(&row),
        "negative outcomes must not masquerade as valid conformance rows"
    );
}

#[test]
fn manifest_schema_types_payload_free_bounded_fuzz_qualification() {
    let schema = read_json("schemas/manifest.schema.json");
    let fuzz_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/fuzz_qualification",
        "$defs": schema["$defs"].clone()
    });
    let validator = jsonschema::validator_for(&fuzz_schema)
        .expect("bounded fuzz qualification schema should compile");
    let mut qualification = serde_json::json!({
        "case_id": "fuzz/parser/bounded_seed_corpus",
        "kind": "bounded_fuzz_run",
        "contract_version": "0.1.0",
        "profile": "fuzz",
        "run_seed": 7,
        "provider": {
            "kind": "mutation_layer",
            "id": "bounded_deterministic_fuzz"
        },
        "target": {
            "kind": "same_project_bounded_part10_probe",
            "independence": "same_project",
            "operation_unit": "input_byte"
        },
        "budget": {
            "max_iterations": 64,
            "max_candidates": 64,
            "max_mutations_per_candidate": 8,
            "max_total_mutations": 512,
            "max_bytes_per_mutation": 64,
            "max_input_bytes": 8388608,
            "max_output_bytes": 8388608,
            "max_minimization_attempts": 256,
            "max_total_target_operations": 100000000,
            "max_target_operations": 1000000
        },
        "seeds": [{
            "id": "part10-explicit-vr-le-v1",
            "source_case_id": "classic/sc/mono2_u8_explicit_le",
            "source_recipe_id": "sc_mono2_u8",
            "source_recipe_version": "0.1.0",
            "source_generation_seed": 7,
            "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "source_size_bytes": 512,
            "surfaces": ["file_meta", "dataset_headers", "pixel_data"]
        }],
        "counters": {
            "iterations": 64,
            "candidates": 64,
            "mutations": 300,
            "target_operations": 32000
        },
        "outcomes": {
            "accepted": 8,
            "clean_rejection": 56,
            "parse_failure": 0,
            "validation_failure": 0,
            "decode_failure": 0,
            "crash": 0,
            "hang": 0,
            "timeout": 0,
            "resource_limit": 0
        },
        "minimizations": [{
            "seed_description_id": "part10-explicit-vr-le-v1",
            "candidate_iteration": 0,
            "candidate_seed": 42,
            "outcome": "clean_rejection",
            "original_size": 512,
            "minimized_size": 1,
            "attempts": 10,
            "target_operations": 1024,
            "minimized_fingerprint": "fnv1a64:0000000000000000"
        }],
        "unacceptable_outcomes": ["crash", "hang", "timeout", "resource_limit"],
        "payload_policy": "generated_payloads_uncommitted",
        "status": "passed"
    });
    assert!(validator.is_valid(&qualification));

    qualification["outcomes"]["timeout"] = Value::from(1);
    assert!(
        !validator.is_valid(&qualification),
        "a passing fuzz qualification cannot hide a timeout"
    );
    qualification["status"] = Value::String("failed".to_string());
    assert!(validator.is_valid(&qualification));
    assert!(qualification.get("path").is_none());
    assert!(qualification.get("bytes").is_none());
}

#[test]
fn manifest_schema_types_cross_instance_geometry_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    assert_eq!(
        schema.pointer("/$defs/expected_geometry/properties/sort_basis/const"),
        Some(&Value::String(
            "image_position_patient_projected_on_slice_normal".to_string()
        ))
    );
    let required = schema
        .pointer("/$defs/expected_geometry/required")
        .and_then(Value::as_array)
        .expect("expected geometry should have required fields");
    for field in [
        "geometric_order_index",
        "position_along_normal_mm",
        "image_position_patient",
        "image_orientation_patient",
        "adjacent_spacing_mm",
        "spacing_uniform",
        "instance_number_state",
        "instance_number",
        "instance_number_order_index",
        "sorting_conflict_expected",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "expected geometry should require {field}"
        );
    }

    assert_eq!(
        schema.pointer("/$defs/expected_geometry/properties/instance_number_state/enum"),
        Some(&serde_json::json!(["numeric", "empty"]))
    );
    for field in [
        "instance_number",
        "instance_number_order_index",
        "sorting_conflict_expected",
    ] {
        assert!(
            schema
                .pointer(&format!("/$defs/expected_geometry/properties/{field}/type"))
                .and_then(Value::as_array)
                .is_some_and(|types| types.iter().any(|value| value.as_str() == Some("null"))),
            "expected geometry {field} must allow null"
        );
    }
    for (field, length) in [
        ("image_position_patient", 3),
        ("image_orientation_patient", 6),
    ] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/expected_geometry/properties/{field}/minItems"
                ))
                .and_then(Value::as_u64),
            Some(length)
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/expected_geometry/properties/{field}/maxItems"
                ))
                .and_then(Value::as_u64),
            Some(length)
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/expected_geometry/properties/{field}/items/type"
                ))
                .and_then(Value::as_str),
            Some("number")
        );
    }
    assert_eq!(
        schema
            .pointer("/$defs/expected_geometry/properties/adjacent_spacing_mm/minItems")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        schema
            .pointer("/$defs/expected_geometry/properties/spacing_uniform/type")
            .and_then(Value::as_str),
        Some("boolean")
    );
    assert!(
        schema
            .pointer("/$defs/expected_geometry/properties/gantry_detector_tilt_degrees/type")
            .and_then(Value::as_array)
            .is_some_and(|types| types.iter().any(|value| value.as_str() == Some("null"))),
        "gantry detector tilt must allow null"
    );
    assert_eq!(
        schema
            .pointer("/$defs/expected_geometry/additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn manifest_schema_types_cross_series_organization_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let file_required = schema
        .pointer("/$defs/file/required")
        .and_then(Value::as_array)
        .expect("manifest schema must define required file fields");
    assert!(
        !file_required
            .iter()
            .any(|value| value.as_str() == Some("expected_series_organization")),
        "series organization expectations must remain optional per file"
    );
    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_series_organization/anyOf/0/$ref"),
        Some(&Value::String(
            "#/$defs/expected_series_organization".to_string()
        ))
    );

    let required = schema
        .pointer("/$defs/expected_series_organization/required")
        .and_then(Value::as_array)
        .expect("expected series organization should have required fields");
    for field in [
        "group_id",
        "study_series_count",
        "series_ordinal",
        "series_instance_count",
        "shared_study_instance_uid_expected",
        "shared_frame_of_reference_uid_expected",
        "distinct_series_instance_uids_expected",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "expected series organization should require {field}"
        );
    }
    assert_eq!(
        schema
            .pointer("/$defs/expected_series_organization/additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn manifest_schema_accepts_strict_utf8_person_name_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");
    let metadata = utf8_person_name_expectations();

    let errors = validator
        .iter_errors(&metadata)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "valid UTF-8 Person Name expectations should pass:\n{}",
        errors.join("\n")
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_metadata/$ref"),
        Some(&Value::String("#/$defs/expected_metadata".to_string()))
    );
    assert_eq!(
        schema
            .pointer("/$defs/expected_metadata/additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "metadata expectations must reject undeclared fields"
    );
}

#[test]
fn manifest_schema_allows_default_iso2022_repertoire_but_not_all_empty() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");

    let mut iso2022 = utf8_person_name_expectations();
    iso2022["specific_character_sets"] = serde_json::json!(["", "ISO 2022 IR 87"]);
    assert!(
        validator.is_valid(&iso2022),
        "an empty first value denotes the default ISO-IR 6 repertoire"
    );

    iso2022["specific_character_sets"] = serde_json::json!([""]);
    assert!(
        !validator.is_valid(&iso2022),
        "the charset contract must still declare a non-default extension"
    );
}

#[test]
fn manifest_schema_types_timezone_boundary_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");
    let temporal = positive_timezone_expectations();
    let errors = validator
        .iter_errors(&temporal)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "valid timezone boundary expectations should pass: {errors:?}"
    );

    let mut malformed = temporal;
    malformed["temporal"]["timezone_offset_from_utc"]["offset_minutes"] = serde_json::json!(841);
    malformed["temporal"]["date_values"][0]["vr"] = serde_json::json!("TM");
    malformed["temporal"]["date_time_values"][0]["normalized_utc"] =
        serde_json::json!("2024-02-29T09:59:59Z");
    malformed["temporal"]["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&malformed).collect::<Vec<_>>();
    assert!(
        errors.len() >= 4,
        "offset range, typed VR, UTC precision, and unknown fields must be rejected: {errors:?}"
    );
}

#[test]
fn manifest_schema_types_empty_type2_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");
    let mut metadata = empty_type2_expectations();
    assert!(
        validator.is_valid(&metadata),
        "the exact five-attribute zero-length contract should pass"
    );

    metadata["empty_type2_attributes"][0]["value_length"] = serde_json::json!(2);
    metadata["empty_type2_attributes"][1]["vr"] = serde_json::json!("LO");
    metadata["empty_type2_attributes"][2]["unexpected"] = serde_json::json!(true);
    metadata["empty_type2_attributes"]
        .as_array_mut()
        .expect("attributes should be an array")
        .pop();
    let errors = validator.iter_errors(&metadata).collect::<Vec<_>>();
    assert!(
        errors.len() >= 4,
        "nonzero VL, invalid VR, unknown fields, and incomplete sets must be rejected: {errors:?}"
    );
}

#[test]
fn manifest_schema_types_long_multivalue_string_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");
    let mut metadata = string_element_expectations();
    assert!(validator.is_valid(&metadata));

    metadata["string_elements"][0]["vr"] = serde_json::json!("UT");
    metadata["string_elements"][1]["value_multiplicity"] = serde_json::json!(0);
    metadata["string_elements"][2]["raw_value_sha256"] = serde_json::json!("not-a-hash");
    metadata["string_elements"][2]["padding"] = serde_json::json!("null");
    metadata["string_elements"]
        .as_array_mut()
        .expect("string elements should be an array")
        .pop();
    let errors = validator.iter_errors(&metadata).collect::<Vec<_>>();
    assert!(
        errors.len() >= 5,
        "VR, VM, hash, padding, and exact element count must be enforced: {errors:?}"
    );
}

#[test]
fn manifest_schema_types_private_creator_block_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");
    let mut metadata = private_creator_expectations();
    assert!(validator.is_valid(&metadata));

    metadata["private_creator_blocks"][0]["vr"] = serde_json::json!("SH");
    metadata["private_creator_blocks"][1]["block_start_tag"] = serde_json::json!("00111200");
    metadata["private_creator_blocks"][1]["elements"][0]["vr"] = serde_json::json!("UL");
    metadata["private_creator_blocks"]
        .as_array_mut()
        .expect("private blocks should be an array")
        .pop();
    let errors = validator.iter_errors(&metadata).collect::<Vec<_>>();
    assert!(
        errors.len() >= 4,
        "creator VR, block ID, private VR, and exact block count must be enforced: {errors:?}"
    );
}

#[test]
fn manifest_schema_types_sequence_length_encoding_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");
    let defined = sequence_length_expectations("defined");
    let undefined = sequence_length_expectations("undefined");
    assert!(validator.is_valid(&defined));
    assert!(validator.is_valid(&undefined));

    let mut malformed = defined;
    malformed["sequence_length_encoding"]["sequence_value_length"] = serde_json::json!(57);
    malformed["sequence_length_encoding"]["sequence_delimitation_present"] =
        serde_json::json!(true);
    malformed["sequence_length_encoding"]["decoded_items"][0]["coding_scheme_designator"] =
        serde_json::json!("SRT");
    malformed["sequence_length_encoding"]["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&malformed).collect::<Vec<_>>();
    assert!(
        errors.len() >= 4,
        "defined VL, delimiter state, semantic code, and unknown fields must be rejected: {errors:?}"
    );
}

#[test]
fn manifest_schema_types_nonsquare_spacing_variants() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_nonsquare_spacing",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("non-square spacing expectation schema should compile");
    let spacing = nonsquare_spacing_expectation("pixel_spacing");
    let aspect = nonsquare_spacing_expectation("pixel_aspect_ratio");
    assert!(validator.is_valid(&spacing));
    assert!(validator.is_valid(&aspect));

    let mut malformed = Vec::new();

    let mut swapped_spacing = spacing.clone();
    swapped_spacing["pixel_spacing"]["lexical_value"] = serde_json::json!("0.3\\0.6");
    malformed.push(("swapped spacing", swapped_spacing));

    let mut zero_aspect = aspect.clone();
    zero_aspect["pixel_aspect_ratio"]["horizontal_extent"] = serde_json::json!(0);
    malformed.push(("zero aspect component", zero_aspect));

    let mut combined_axes = spacing.clone();
    combined_axes["pixel_aspect_ratio"] = aspect["pixel_aspect_ratio"].clone();
    malformed.push(("combined spacing and aspect axes", combined_axes));

    let mut missing_nominal = spacing.clone();
    missing_nominal["nominal_scanned_pixel_spacing"] = Value::Null;
    malformed.push(("missing nominal scanned spacing", missing_nominal));

    let mut wrong_vr = aspect.clone();
    wrong_vr["pixel_aspect_ratio"]["vr"] = serde_json::json!("DS");
    malformed.push(("wrong aspect VR", wrong_vr));

    let mut wrong_tag = spacing.clone();
    wrong_tag["nominal_scanned_pixel_spacing"]["tag"] = serde_json::json!("0018,1164");
    malformed.push(("wrong nominal spacing tag", wrong_tag));

    let mut wrong_vm = aspect.clone();
    wrong_vm["pixel_aspect_ratio"]["vm"] = serde_json::json!(1);
    malformed.push(("wrong aspect VM", wrong_vm));

    let mut wrong_hash = aspect.clone();
    wrong_hash["pixel_data_sha256"] = serde_json::json!("0".repeat(64));
    malformed.push(("wrong pixel hash", wrong_hash));

    let mut unexpected = spacing;
    unexpected["unexpected"] = serde_json::json!(true);
    malformed.push(("unexpected field", unexpected));

    for (description, value) in malformed {
        assert!(
            !validator.is_valid(&value),
            "schema must reject {description}: {value}"
        );
    }
}

#[test]
fn manifest_schema_locks_nonsquare_case_image_and_pixel_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("classic/sc/nonsquare_pixel_spacing")
        })
        .expect("manifest schema should define the non-square spacing case conditional");

    let required = rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("non-square case conditional should require specialized fields");
    for field in ["image", "pixel_data", "expected_nonsquare_spacing"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "non-square case conditional must require {field}"
        );
    }
    for (pointer, expected) in [
        (
            "/then/properties/image/properties/rows/const",
            serde_json::json!(4),
        ),
        (
            "/then/properties/image/properties/columns/const",
            serde_json::json!(6),
        ),
        (
            "/then/properties/image/properties/frames/const",
            serde_json::json!(1),
        ),
        (
            "/then/properties/image/properties/photometric_interpretation/const",
            serde_json::json!("MONOCHROME2"),
        ),
        (
            "/then/properties/image/properties/bits_allocated/const",
            serde_json::json!(8),
        ),
        (
            "/then/properties/image/properties/bits_stored/const",
            serde_json::json!(8),
        ),
        (
            "/then/properties/image/properties/high_bit/const",
            serde_json::json!(7),
        ),
        (
            "/then/properties/pixel_data/properties/vr/const",
            serde_json::json!("OB"),
        ),
        (
            "/then/properties/pixel_data/properties/native_or_encapsulated/const",
            serde_json::json!("native"),
        ),
        (
            "/then/properties/pixel_data/properties/value_length/const",
            serde_json::json!(24),
        ),
        (
            "/then/properties/pixel_data/properties/frame_count/const",
            serde_json::json!(1),
        ),
    ] {
        assert_eq!(
            rule.pointer(pointer),
            Some(&expected),
            "wrong contract at {pointer}"
        );
    }
    assert_eq!(
        rule.pointer("/then/properties/image/properties/planar_configuration/type")
            .and_then(Value::as_str),
        Some("null")
    );
}

fn nonsquare_spacing_expectation(variant_id: &str) -> Value {
    let pixel_spacing = serde_json::json!({
        "tag": "0028,0030",
        "keyword": "PixelSpacing",
        "vr": "DS",
        "vm": 2,
        "lexical_value": "0.6\\0.3",
        "row_spacing_mm": 0.6,
        "column_spacing_mm": 0.3
    });
    let nominal_scanned_pixel_spacing = serde_json::json!({
        "tag": "0018,2010",
        "keyword": "NominalScannedPixelSpacing",
        "vr": "DS",
        "vm": 2,
        "lexical_value": "0.6\\0.3",
        "row_spacing_mm": 0.6,
        "column_spacing_mm": 0.3
    });
    let pixel_aspect_ratio = serde_json::json!({
        "tag": "0028,0034",
        "keyword": "PixelAspectRatio",
        "vr": "IS",
        "vm": 2,
        "lexical_value": "2\\1",
        "vertical_extent": 2,
        "horizontal_extent": 1
    });
    serde_json::json!({
        "variant_id": variant_id,
        "pixel_spacing": if variant_id == "pixel_spacing" { pixel_spacing } else { Value::Null },
        "nominal_scanned_pixel_spacing": if variant_id == "pixel_spacing" {
            nominal_scanned_pixel_spacing
        } else {
            Value::Null
        },
        "pixel_aspect_ratio": if variant_id == "pixel_aspect_ratio" {
            pixel_aspect_ratio
        } else {
            Value::Null
        },
        "uncalibrated": true,
        "patient_space_geometry_present": false,
        "pixel_data_sha256": "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6"
    })
}

#[test]
fn manifest_schema_types_nuclear_medicine_multiframe_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let nm_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_nm_multiframe",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&nm_schema).expect("NM expectation schema should compile");
    let mut expectations = nuclear_medicine_multiframe_expectations();
    assert!(validator.is_valid(&expectations));

    expectations["frame_increment_pointers"][1] = serde_json::json!("0054,0070");
    expectations["energy_window_vector"][2] = serde_json::json!(1);
    expectations["detectors"][0]["collimator_type"] = serde_json::json!("INVALID");
    expectations["frame_dimensions"][3]["detector_index"] = serde_json::json!(1);
    expectations["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&expectations).collect::<Vec<_>>();
    assert!(
        errors.len() >= 5,
        "pointer order, dimension vectors, detector terms, frame tuples, and unknown fields must be rejected: {errors:?}"
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_nm_multiframe/$ref"),
        Some(&Value::String("#/$defs/expected_nm_multiframe".to_string()))
    );
}

#[test]
fn manifest_schema_types_pet_activity_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let pet_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_pet_activity",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&pet_schema).expect("PET expectation schema should compile");
    let mut expectations = pet_activity_expectations();
    assert!(validator.is_valid(&expectations));

    expectations["units"] = serde_json::json!("GML");
    expectations["rescale_intercept"] = serde_json::json!(1.0);
    expectations["series_type"][0] = serde_json::json!("GATED");
    expectations["activity_values_bqml"][3] = serde_json::json!(999.0);
    expectations["radiopharmaceutical_information_item_count"] = serde_json::json!(1);
    expectations["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&expectations).collect::<Vec<_>>();
    assert!(
        errors.len() >= 6,
        "units, intercept, series type, activity mapping, isotope cardinality, and unknown fields must be rejected: {errors:?}"
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_pet_activity/$ref"),
        Some(&Value::String("#/$defs/expected_pet_activity".to_string()))
    );
}

#[test]
fn manifest_schema_types_enhanced_pet_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let enhanced_pet_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_enhanced_pet",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&enhanced_pet_schema)
        .expect("Enhanced PET expectation schema should compile");
    let mut expectations = enhanced_pet_expectations();
    assert!(validator.is_valid(&expectations));

    expectations["image_type"][0] = serde_json::json!("ORIGINAL");
    expectations["dimension_index_pointer"] = serde_json::json!("0020,0032");
    expectations["stack_ids"][1] = serde_json::json!("2");
    expectations["anatomic_region"]["code_value"] = serde_json::json!("80891009");
    expectations["radiopharmaceutical_information"]["radionuclide"]["code_meaning"] =
        serde_json::json!("Fluorine-18");
    expectations["view_code"]["code_value"] = serde_json::json!("399321004");
    expectations["corrections"]["decay"] = serde_json::json!("YES");
    expectations["real_world_value_mapping"]["slope"] = serde_json::json!(1.0);
    expectations["stored_values_by_frame"][1][3] = serde_json::json!(401);
    expectations["nonclaims"]["suv"] = serde_json::json!(true);
    expectations["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&expectations).collect::<Vec<_>>();
    assert!(
        errors.len() >= 10,
        "Enhanced PET identity, dimensions, codes, correction nonclaims, quantitative mapping, native frames, and unknown fields must be rejected: {errors:?}"
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_enhanced_pet/$ref"),
        Some(&Value::String("#/$defs/expected_enhanced_pet".to_string()))
    );

    let case_rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .and_then(|rules| {
            rules.iter().find(|rule| {
                rule.pointer("/if/properties/case_id/const")
                    .and_then(Value::as_str)
                    == Some("enhanced/pet/multiframe_explicit_le")
            })
        })
        .expect("Enhanced PET case must have a manifest schema conditional");
    let required = case_rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("Enhanced PET case conditional must require manifest fields");
    for field in ["image", "pixel_data", "expected_enhanced_pet"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "Enhanced PET case conditional must require {field}"
        );
    }
    assert_eq!(
        case_rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&Value::from("1.2.840.10008.5.1.4.1.1.130"))
    );
    assert_eq!(
        case_rule.pointer("/then/properties/image/properties/frames/const"),
        Some(&Value::from(2))
    );
}

#[test]
fn manifest_schema_types_ultrasound_multiframe_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let us_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_us_multiframe",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&us_schema).expect("US expectation schema should compile");
    let mut expectations = ultrasound_multiframe_expectations();
    assert!(validator.is_valid(&expectations));

    expectations["frame_increment_pointer"] = serde_json::json!("0018,1065");
    expectations["frame_relative_times_ms"][2] = serde_json::json!(250.0);
    expectations["frames"][1]["frame_number"] = serde_json::json!(3);
    expectations["frames"][2]["frame_sha256"] =
        serde_json::json!("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
    expectations["frames"][3]["pixel_values"][13] = serde_json::json!(80);
    expectations["spatially_related_frames"] = serde_json::json!(true);
    expectations["color_data_present"] = serde_json::json!(true);
    expectations["region_calibrated"] = serde_json::json!(true);
    expectations["lossy_image_compression"] = serde_json::json!("01");
    expectations["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&expectations).collect::<Vec<_>>();
    assert!(
        errors.len() >= 10,
        "pointer, timing, frame order, hashes, pixels, explicit non-claims, loss history, and unknown fields must be rejected: {errors:?}"
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_us_multiframe/$ref"),
        Some(&Value::String("#/$defs/expected_us_multiframe".to_string()))
    );

    let us_case_rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .and_then(|rules| {
            rules.iter().find(|rule| {
                rule.pointer("/if/properties/case_id/const")
                    .and_then(Value::as_str)
                    == Some("classic/us/multiframe_explicit_le")
            })
        })
        .expect("US case must have a manifest schema conditional");
    let required = us_case_rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("US case conditional must require manifest fields");
    for field in ["image", "pixel_data", "expected_us_multiframe"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "US case conditional must require {field}"
        );
    }
    assert_eq!(
        us_case_rule.pointer("/then/properties/image/properties/rows/const"),
        Some(&Value::from(4))
    );
    assert_eq!(
        us_case_rule.pointer("/then/properties/image/properties/columns/const"),
        Some(&Value::from(4))
    );
}

#[test]
fn manifest_schema_types_xa_projection_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let xa_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_xa_projection",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&xa_schema).expect("XA expectation schema should compile");
    let mut expectations = xa_projection_expectations();
    assert!(validator.is_valid(&expectations));

    expectations["image_type"][2] = serde_json::json!("BIPLANE A");
    expectations["body_part_examined"] = serde_json::json!("CHEST");
    expectations["patient_orientation_empty"] = serde_json::json!(false);
    expectations["pixel_intensity_relationship"] = serde_json::json!("LOG");
    expectations["radiation_setting"] = serde_json::json!("SC");
    expectations["exposure_mas"] = serde_json::json!(5);
    expectations["imager_pixel_spacing_mm"][1] = serde_json::json!(0.3);
    expectations["estimated_radiographic_magnification_factor"] = serde_json::json!(1.4);
    expectations["multiframe_cine"] = serde_json::json!(true);
    expectations["biplane_data_present"] = serde_json::json!(true);
    expectations["patient_space_geometry_present"] = serde_json::json!(true);
    expectations["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&expectations).collect::<Vec<_>>();
    assert!(
        errors.len() >= 12,
        "XA plane, anatomy, acquisition, geometry, explicit non-claims, and unknown fields must be rejected: {errors:?}"
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_xa_projection/$ref"),
        Some(&Value::String("#/$defs/expected_xa_projection".to_string()))
    );

    let xa_case_rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .and_then(|rules| {
            rules.iter().find(|rule| {
                rule.pointer("/if/properties/case_id/const")
                    .and_then(Value::as_str)
                    == Some("classic/xa/monoplane_explicit_le")
            })
        })
        .expect("XA case must have a manifest schema conditional");
    let required = xa_case_rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("XA case conditional must require manifest fields");
    for field in ["image", "pixel_data", "expected_xa_projection"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "XA case conditional must require {field}"
        );
    }
    assert_eq!(
        xa_case_rule.pointer("/then/properties/image/properties/frames/const"),
        Some(&Value::from(1))
    );
    assert_eq!(
        xa_case_rule.pointer("/then/properties/image/properties/bits_allocated/const"),
        Some(&Value::from(8))
    );
}

#[test]
fn manifest_schema_types_xrf_projection_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let xrf_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_xrf_projection",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&xrf_schema).expect("XRF expectation schema should compile");
    let mut expectations = xrf_projection_expectations();
    assert!(validator.is_valid(&expectations));

    expectations["image_type"][2] = serde_json::json!("BIPLANE A");
    expectations["frame_count"] = serde_json::json!(2);
    expectations["body_part_examined"] = serde_json::json!("CHEST");
    expectations["patient_orientation_empty"] = serde_json::json!(false);
    expectations["laterality_present"] = serde_json::json!(true);
    expectations["pixel_intensity_relationship"] = serde_json::json!("LOG");
    expectations["radiation_setting"] = serde_json::json!("GR");
    expectations["kvp"] = serde_json::json!(71.0);
    expectations["exposure_mas"] = serde_json::json!(2);
    expectations["imager_pixel_spacing_mm"][1] = serde_json::json!(0.3);
    expectations["distance_source_to_detector_mm"] = serde_json::json!(1201.0);
    expectations["distance_source_to_patient_mm"] = serde_json::json!(801.0);
    expectations["estimated_radiographic_magnification_factor"] = serde_json::json!(1.4);
    expectations["column_angulation_degrees"] = serde_json::json!(11.0);
    expectations["lossy_image_compression"] = serde_json::json!("01");
    expectations["multiframe_cine"] = serde_json::json!(true);
    expectations["biplane_data_present"] = serde_json::json!(true);
    expectations["contrast_used"] = serde_json::json!(true);
    expectations["subtraction_applied"] = serde_json::json!(true);
    expectations["table_position_present"] = serde_json::json!(true);
    expectations["table_motion_present"] = serde_json::json!(true);
    expectations["table_tilt_present"] = serde_json::json!(true);
    expectations["tomography_present"] = serde_json::json!(true);
    expectations["patient_space_geometry_present"] = serde_json::json!(true);
    expectations["pixel_spacing_calibrated"] = serde_json::json!(true);
    expectations["xa_positioner_angles_present"] = serde_json::json!(true);
    expectations["unexpected"] = serde_json::json!(true);
    let errors = validator.iter_errors(&expectations).collect::<Vec<_>>();
    assert!(
        errors.len() >= 27,
        "XRF identity, anatomy, acquisition, positioner geometry, explicit non-claims, and unknown fields must be rejected: {errors:?}"
    );

    assert_eq!(
        schema.pointer("/$defs/file/properties/expected_xrf_projection/$ref"),
        Some(&Value::String(
            "#/$defs/expected_xrf_projection".to_string()
        ))
    );

    let xrf_case_rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .and_then(|rules| {
            rules.iter().find(|rule| {
                rule.pointer("/if/properties/case_id/const")
                    .and_then(Value::as_str)
                    == Some("classic/xrf/monoplane_explicit_le")
            })
        })
        .expect("XRF case must have a manifest schema conditional");
    let required = xrf_case_rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("XRF case conditional must require manifest fields");
    for field in ["image", "pixel_data", "expected_xrf_projection"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "XRF case conditional must require {field}"
        );
    }
    for (pointer, expected) in [
        (
            "/then/properties/dicom/properties/sop_class_uid/const",
            Value::from("1.2.840.10008.5.1.4.1.1.12.2"),
        ),
        (
            "/then/properties/dicom/properties/sop_class_name/const",
            Value::from("X-Ray Radiofluoroscopic Image Storage"),
        ),
        (
            "/then/properties/dicom/properties/iod_name/const",
            Value::from("X-Ray Radiofluoroscopic Image"),
        ),
        (
            "/then/properties/dicom/properties/modality/const",
            Value::from("RF"),
        ),
        (
            "/then/properties/dicom/properties/transfer_syntax_uid/const",
            Value::from("1.2.840.10008.1.2.1"),
        ),
        (
            "/then/properties/dicom/properties/transfer_syntax_name/const",
            Value::from("Explicit VR Little Endian"),
        ),
        (
            "/then/properties/image/properties/frames/const",
            Value::from(1),
        ),
        (
            "/then/properties/image/properties/bits_allocated/const",
            Value::from(8),
        ),
        (
            "/then/properties/pixel_data/properties/vr/const",
            Value::from("OB"),
        ),
        (
            "/then/properties/pixel_data/properties/value_length/const",
            Value::from(16),
        ),
        (
            "/then/properties/recipe/properties/recipe_id/const",
            Value::from("classic_xrf_monoplane_explicit_le"),
        ),
        (
            "/then/properties/recipe/properties/recipe_version/const",
            Value::from("0.1.0"),
        ),
    ] {
        assert_eq!(xrf_case_rule.pointer(pointer), Some(&expected));
    }
    assert!(
        xrf_case_rule
            .pointer("/then/properties/recipe/properties/recipe_parameters/required")
            .and_then(Value::as_array)
            .is_some_and(|required| {
                required
                    .iter()
                    .any(|value| value.as_str() == Some("xrf_projection"))
            }),
        "XRF case conditional must require xrf_projection recipe parameters"
    );
}

#[test]
fn manifest_schema_rejects_malformed_person_name_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let metadata_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_metadata",
        "$defs": schema["$defs"].clone(),
    });
    let validator =
        jsonschema::validator_for(&metadata_schema).expect("metadata schema should compile");

    let mut malformed = utf8_person_name_expectations();
    malformed["person_names"][0]["tag"] = serde_json::json!("00100010");
    malformed["person_names"][0]["vr"] = serde_json::json!("LO");
    malformed["person_names"][0]["raw_value_sha256"] = serde_json::json!("not-a-hash");
    malformed["person_names"][0]["raw_value_hex"] = serde_json::json!("1b2442");
    malformed["person_names"][0]["component_groups"][0]["components"][0]["position"] =
        serde_json::json!(2);
    malformed["unexpected"] = serde_json::json!(true);

    let errors = validator.iter_errors(&malformed).collect::<Vec<_>>();
    assert!(
        errors.len() >= 6,
        "malformed tag, VR, raw hex, hash, component order, and unknown field must each be rejected; got {errors:?}"
    );
}

fn utf8_person_name_expectations() -> Value {
    let components = |values: [&str; 5]| {
        serde_json::json!([
            { "position": 1, "decoded_value": values[0] },
            { "position": 2, "decoded_value": values[1] },
            { "position": 3, "decoded_value": values[2] },
            { "position": 4, "decoded_value": values[3] },
            { "position": 5, "decoded_value": values[4] },
        ])
    };

    serde_json::json!({
        "specific_character_sets": ["ISO_IR 192"],
        "person_names": [{
            "tag": "0010,0010",
            "keyword": "PatientName",
            "vr": "PN",
            "decoded_value": "Müller^Zoë^^Dr.^III=山田^太郎=やまだ^たろう",
            "raw_value_hex": "4D756C6C65725E5A6F65",
            "raw_value_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "raw_value_byte_length": 61,
            "component_groups": [
                {
                    "position": 1,
                    "kind": "alphabetic",
                    "decoded_value": "Müller^Zoë^^Dr.^III",
                    "components": components(["Müller", "Zoë", "", "Dr.", "III"]),
                },
                {
                    "position": 2,
                    "kind": "ideographic",
                    "decoded_value": "山田^太郎",
                    "components": components(["山田", "太郎", "", "", ""]),
                },
                {
                    "position": 3,
                    "kind": "phonetic",
                    "decoded_value": "やまだ^たろう",
                    "components": components(["やまだ", "たろう", "", "", ""]),
                },
            ],
        }],
    })
}

fn empty_type2_expectations() -> Value {
    serde_json::json!({
        "empty_type2_attributes": [
            { "tag": "0010,0010", "keyword": "PatientName", "vr": "PN", "value_length": 0 },
            { "tag": "0010,0030", "keyword": "PatientBirthDate", "vr": "DA", "value_length": 0 },
            { "tag": "0010,0040", "keyword": "PatientSex", "vr": "CS", "value_length": 0 },
            { "tag": "0008,0090", "keyword": "ReferringPhysicianName", "vr": "PN", "value_length": 0 },
            { "tag": "0008,0050", "keyword": "AccessionNumber", "vr": "SH", "value_length": 0 }
        ]
    })
}

fn string_element_expectations() -> Value {
    serde_json::json!({
        "string_elements": [
            {
                "tag": "0020,4000", "keyword": "ImageComments", "vr": "LT",
                "decoded_values": ["A"], "value_multiplicity": 1,
                "decoded_value_lengths": [10240], "raw_value_byte_length": 10240,
                "raw_value_sha256": "75497849c172d88a38e271cc6ce82f31adbba1f16b6191d8ddaeb4e9f6268e52",
                "padding": "none"
            },
            {
                "tag": "0018,1020", "keyword": "SoftwareVersions", "vr": "LO",
                "decoded_values": ["A", "B"], "value_multiplicity": 2,
                "decoded_value_lengths": [64, 64], "raw_value_byte_length": 130,
                "raw_value_sha256": "e79f64c5853732dd713d14c3530ef494d800f684653fc5bf0aced3933241a260",
                "padding": "space"
            },
            {
                "tag": "0028,0030", "keyword": "PixelSpacing", "vr": "DS",
                "decoded_values": ["0.12345678901234", "0.98765432109876"],
                "value_multiplicity": 2, "decoded_value_lengths": [16, 16],
                "raw_value_byte_length": 34,
                "raw_value_sha256": "e09885a80758e44eaa4b9b544e7301c852395d3ee14ed7b7588e62a5f3b2db6a",
                "padding": "space"
            },
            {
                "tag": "0020,0012", "keyword": "AcquisitionNumber", "vr": "IS",
                "decoded_values": ["+02147483647"], "value_multiplicity": 1,
                "decoded_value_lengths": [12], "raw_value_byte_length": 12,
                "raw_value_sha256": "f9cf9c74b83f0c66cdb48d3536a5a5d884babc2cfda813d01b3577b473de20cf",
                "padding": "none"
            }
        ]
    })
}

fn private_creator_expectations() -> Value {
    serde_json::json!({
        "private_creator_blocks": [
            {
                "creator_tag": "0011,0010", "creator_id": "DTS_PRIVATE_ALPHA", "vr": "LO",
                "raw_value_hex": "4454535F505249564154455F414C50484120",
                "raw_value_byte_length": 18,
                "raw_value_sha256": "02a7ccdec62f131efea4bb7c0954d15df2b1efd67abec69123ff0afcb197f8c3",
                "block_start_tag": "0011,1000", "block_end_tag": "0011,10FF",
                "elements": [
                    { "tag": "0011,1001", "vr": "LO", "decoded_value": "ALPHA-GROUP-0011", "raw_value_hex": "414C5048412D47524F55502D30303131", "raw_value_byte_length": 16, "raw_value_sha256": "6b95b0cd9835f0ab50173c42a37511a7e8a547af8837f67e0a9bd0d6ff0da1ae" },
                    { "tag": "0011,10F0", "vr": "US", "decoded_value": 4660, "raw_value_hex": "3412", "raw_value_byte_length": 2, "raw_value_sha256": "e74d0e44a658ffcdc0ee7266ebd171413b8fcf182c97a27254d9f48abaea6266" }
                ]
            },
            {
                "creator_tag": "0011,0012", "creator_id": "DTS_PRIVATE_BETA", "vr": "LO",
                "raw_value_hex": "4454535F505249564154455F42455441",
                "raw_value_byte_length": 16,
                "raw_value_sha256": "df2316ffa7d764760e6c7f6174d3b15a2d59687834a90474b7446ff323df073d",
                "block_start_tag": "0011,1200", "block_end_tag": "0011,12FF",
                "elements": [
                    { "tag": "0011,1201", "vr": "LO", "decoded_value": "BETA-BLOCK-12", "raw_value_hex": "424554412D424C4F434B2D313220", "raw_value_byte_length": 14, "raw_value_sha256": "3329e2d8d73e62f294fd73110474122239fd4d75a8a2aefbe16c117f0265b328" }
                ]
            },
            {
                "creator_tag": "0013,0011", "creator_id": "DTS_PRIVATE_ALPHA", "vr": "LO",
                "raw_value_hex": "4454535F505249564154455F414C50484120",
                "raw_value_byte_length": 18,
                "raw_value_sha256": "02a7ccdec62f131efea4bb7c0954d15df2b1efd67abec69123ff0afcb197f8c3",
                "block_start_tag": "0013,1100", "block_end_tag": "0013,11FF",
                "elements": [
                    { "tag": "0013,1101", "vr": "LO", "decoded_value": "ALPHA-GROUP-0013", "raw_value_hex": "414C5048412D47524F55502D30303133", "raw_value_byte_length": 16, "raw_value_sha256": "6374ee55ea117a6d46b516c6ca6f2550d95c849a16221c58bfea5c054b9e6919" }
                ]
            }
        ]
    })
}

fn sequence_length_expectations(variant: &str) -> Value {
    let (value_length, length_hex, sequence_delimiter) = match variant {
        "defined" => (serde_json::json!(56), "38000000", false),
        "undefined" => (Value::Null, "FFFFFFFF", true),
        _ => panic!("unsupported sequence length fixture variant"),
    };
    serde_json::json!({
        "sequence_length_encoding": {
            "variant_id": variant,
            "sequence_tag": "0008,2218",
            "keyword": "AnatomicRegionSequence",
            "vr": "SQ",
            "sequence_value_length": value_length,
            "sequence_length_field_hex": length_hex,
            "sequence_delimitation_present": sequence_delimiter,
            "item_count": 1,
            "item_length_encoding": "undefined",
            "item_length_field_hex": "FFFFFFFF",
            "item_delimitation_present": true,
            "decoded_items": [{
                "code_value": "69536005",
                "coding_scheme_designator": "SCT",
                "code_meaning": "Head"
            }]
        }
    })
}

fn nuclear_medicine_multiframe_expectations() -> Value {
    let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    serde_json::json!({
        "image_type": ["ORIGINAL", "PRIMARY", "STATIC", "EMISSION"],
        "frame_increment_pointers": ["0054,0010", "0054,0020"],
        "energy_window_vector": [1, 1, 2, 2],
        "detector_vector": [1, 2, 1, 2],
        "number_of_energy_windows": 2,
        "number_of_detectors": 2,
        "energy_windows": [
            { "index": 1, "name": "Tc99m Photopeak", "lower_limit_kev": 126.0, "upper_limit_kev": 154.0 },
            { "index": 2, "name": "Tc99m Scatter", "lower_limit_kev": 100.0, "upper_limit_kev": 120.0 }
        ],
        "detectors": [
            {
                "index": 1,
                "collimator_type": "PARA",
                "focal_distance_mm": 0.0,
                "start_angle_degrees": 0.0,
                "image_orientation_patient": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                "image_position_patient": [0.0, 0.0, 0.0]
            },
            {
                "index": 2,
                "collimator_type": "PARA",
                "focal_distance_mm": 0.0,
                "start_angle_degrees": 180.0,
                "image_orientation_patient": [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                "image_position_patient": [0.0, 0.0, 0.0]
            }
        ],
        "actual_frame_duration_ms": 1000,
        "counts_accumulated": 904,
        "frame_dimensions": [
            { "frame_number": 1, "energy_window_index": 1, "detector_index": 1, "frame_sha256": hash },
            { "frame_number": 2, "energy_window_index": 1, "detector_index": 2, "frame_sha256": hash },
            { "frame_number": 3, "energy_window_index": 2, "detector_index": 1, "frame_sha256": hash },
            { "frame_number": 4, "energy_window_index": 2, "detector_index": 2, "frame_sha256": hash }
        ]
    })
}

fn pet_activity_expectations() -> Value {
    serde_json::json!({
        "image_type": ["ORIGINAL", "PRIMARY"],
        "units": "BQML",
        "counts_source": "EMISSION",
        "series_type": ["STATIC", "IMAGE"],
        "number_of_slices": 1,
        "corrected_image": ["DCAL"],
        "decay_correction": "NONE",
        "dose_calibration_factor": 1.0,
        "rescale_intercept": 0.0,
        "rescale_slope": 2.5,
        "stored_values": [0, 100, 200, 400],
        "activity_values_bqml": [0.0, 250.0, 500.0, 1000.0],
        "frame_reference_time_ms": 30000.0,
        "actual_frame_duration_ms": 60000,
        "image_index": 1,
        "radiopharmaceutical_information_item_count": 0
    })
}

fn enhanced_pet_expectations() -> Value {
    let frame_hash = "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784";
    serde_json::json!({
        "image_type": ["DERIVED", "PRIMARY", "STATIC", "MULTIPLICATION"],
        "frame_type": ["DERIVED", "PRIMARY", "STATIC", "MULTIPLICATION"],
        "pixel_presentation": "MONOCHROME",
        "volumetric_properties": "VOLUME",
        "volume_based_calculation_technique": "NONE",
        "content_qualification": "RESEARCH",
        "burned_in_annotation": "NO",
        "lossy_image_compression": "00",
        "presentation_lut_shape": "IDENTITY",
        "frame_count": 2,
        "shared_functional_groups_item_count": 1,
        "per_frame_functional_groups_item_count": 2,
        "dimension_organization_item_count": 1,
        "dimension_index_item_count": 1,
        "dimension_index_pointer": "0020,9057",
        "functional_group_pointer": "0020,9111",
        "stack_ids": ["1", "1"],
        "in_stack_position_numbers": [1, 2],
        "dimension_index_values": [1, 2],
        "temporal_position_indices": [1, 1],
        "image_positions_patient_mm": [[0.0, 0.0, 0.0], [0.0, 0.0, 5.0]],
        "pixel_spacing_mm": [2.0, 2.0],
        "slice_thickness_mm": 5.0,
        "spacing_between_slices_mm": 5.0,
        "image_orientation_patient": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        "frame_laterality": "U",
        "anatomic_region": {
            "code_value": "69536005",
            "coding_scheme_designator": "SCT",
            "code_meaning": "Head"
        },
        "rescale_intercept": 0.0,
        "rescale_slope": 2.5,
        "rescale_type": "US",
        "window_center": 500.0,
        "window_width": 1000.0,
        "real_world_value_mapping": {
            "first_value_mapped": 0,
            "last_value_mapped": 400,
            "intercept": 0.0,
            "slope": 2.5,
            "lut_label": "BQML",
            "lut_explanation": "Activity concentration",
            "measurement_units": {
                "code_value": "Bq/ml",
                "coding_scheme_designator": "UCUM",
                "code_meaning": "Becquerels/milliliter"
            }
        },
        "radiopharmaceutical_information": {
            "item_count": 1,
            "agent_number": 1,
            "radionuclide": {
                "code_value": "77004003",
                "coding_scheme_designator": "SCT",
                "code_meaning": "^18^Fluorine"
            },
            "administration_route": {
                "code_value": "47625008",
                "coding_scheme_designator": "SCT",
                "code_meaning": "Intravenous route"
            },
            "start_datetime": "20260101000000",
            "total_dose_present_empty": true,
            "half_life_seconds": 6586.2,
            "positron_fraction": 0.967,
            "radiopharmaceutical": {
                "code_value": "35321007",
                "coding_scheme_designator": "SCT",
                "code_meaning": "Fluorodeoxyglucose F^18^"
            }
        },
        "radiopharmaceutical_usage_agent_number": 1,
        "table_motion": "STATIC",
        "time_of_flight_information_used": "FALSE",
        "view_code": {
            "code_value": "24422004",
            "coding_scheme_designator": "SCT",
            "code_meaning": "Axial"
        },
        "view_modifier_item_count": 0,
        "slice_progression_direction_present": false,
        "counts_source": "EMISSION",
        "corrections": {
            "decay": "NO",
            "attenuation": "NO",
            "scatter": "NO",
            "dead_time": "NO",
            "gantry_motion": "NO",
            "patient_motion": "NO",
            "count_loss_normalization": "NO",
            "randoms": "NO",
            "non_uniform_radial_sampling": "NO",
            "sensitivity_calibration": "NO",
            "detector_normalization": "NO"
        },
        "derivation_image_item_count": 0,
        "acquisition_context_item_count": 0,
        "stored_values_by_frame": [[0, 100, 200, 400], [0, 100, 200, 400]],
        "activity_values_bqml_by_frame": [
            [0.0, 250.0, 500.0, 1000.0],
            [0.0, 250.0, 500.0, 1000.0]
        ],
        "frame_sha256": [frame_hash, frame_hash],
        "pixel_data_sha256": "3a43b45e2f6d4d04fe4fc357dfc0efaa21caa5415ffc5db96fc19428d34a7bb5",
        "nonclaims": {
            "suv": false,
            "body_weight_normalization": false,
            "body_surface_area_normalization": false,
            "decay_corrected": false,
            "clinically_calibrated": false,
            "acquisition_counts": false,
            "actual_clinical_dose": false,
            "gating": false,
            "detector_motion": false,
            "time_of_flight_processing": false,
            "reconstruction": false
        }
    })
}

fn ultrasound_multiframe_expectations() -> Value {
    serde_json::json!({
        "image_type": ["ORIGINAL", "PRIMARY", "ABDOMINAL", "0001"],
        "frame_increment_pointer": "0018,1063",
        "frame_time_ms": 100.0,
        "frame_relative_times_ms": [0.0, 100.0, 200.0, 300.0],
        "frame_count": 4,
        "frames": [
            {
                "frame_number": 1,
                "frame_sha256": "be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7",
                "pixel_values": [0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 255, 80, 48, 64, 80, 64]
            },
            {
                "frame_number": 2,
                "frame_sha256": "303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee",
                "pixel_values": [0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 255, 48, 64, 80, 80]
            },
            {
                "frame_number": 3,
                "frame_sha256": "7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb",
                "pixel_values": [0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 64, 255, 80]
            },
            {
                "frame_number": 4,
                "frame_sha256": "8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650",
                "pixel_values": [0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 255, 80, 64]
            }
        ],
        "spatially_related_frames": false,
        "color_data_present": false,
        "region_calibrated": false,
        "lossy_image_compression": "00"
    })
}

fn xa_projection_expectations() -> Value {
    serde_json::json!({
        "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"],
        "frame_count": 1,
        "body_part_examined": "HEART",
        "patient_orientation_empty": true,
        "laterality_present": false,
        "pixel_intensity_relationship": "LIN",
        "radiation_setting": "GR",
        "kvp": 80.0,
        "exposure_mas": 4,
        "imager_pixel_spacing_mm": [0.2, 0.2],
        "positioner_primary_angle_degrees": 15.0,
        "positioner_secondary_angle_degrees": -10.0,
        "distance_source_to_detector_mm": 1200.0,
        "distance_source_to_patient_mm": 800.0,
        "estimated_radiographic_magnification_factor": 1.5,
        "lossy_image_compression": "00",
        "multiframe_cine": false,
        "biplane_data_present": false,
        "contrast_used": false,
        "subtraction_applied": false,
        "table_motion_present": false,
        "patient_space_geometry_present": false,
        "pixel_spacing_calibrated": false
    })
}

fn xrf_projection_expectations() -> Value {
    serde_json::json!({
        "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"],
        "frame_count": 1,
        "body_part_examined": "ABDOMEN",
        "patient_orientation_empty": true,
        "laterality_present": false,
        "pixel_intensity_relationship": "LIN",
        "radiation_setting": "SC",
        "kvp": 70.0,
        "exposure_mas": 1,
        "imager_pixel_spacing_mm": [0.2, 0.2],
        "distance_source_to_detector_mm": 1200.0,
        "distance_source_to_patient_mm": 800.0,
        "estimated_radiographic_magnification_factor": 1.5,
        "column_angulation_degrees": 10.0,
        "lossy_image_compression": "00",
        "multiframe_cine": false,
        "biplane_data_present": false,
        "contrast_used": false,
        "subtraction_applied": false,
        "table_position_present": false,
        "table_motion_present": false,
        "table_tilt_present": false,
        "tomography_present": false,
        "patient_space_geometry_present": false,
        "pixel_spacing_calibrated": false,
        "xa_positioner_angles_present": false
    })
}

fn positive_timezone_expectations() -> Value {
    serde_json::json!({
        "temporal": {
            "boundary_id": "positive_max",
            "timezone_offset_from_utc": {
                "tag": "0008,0201",
                "keyword": "TimezoneOffsetFromUTC",
                "vr": "SH",
                "decoded_value": "+1400",
                "raw_value_hex": "2B3134303020",
                "raw_value_sha256": "91a932becb33d781226fc6594e6bcb216db6ea5b3083e3fa61c7d0d8f9ea3385",
                "raw_value_byte_length": 6,
                "offset_minutes": 840
            },
            "date_values": [{
                "tag": "0008,0020",
                "keyword": "StudyDate",
                "vr": "DA",
                "decoded_value": "20240229",
                "raw_value_hex": "3230323430323239",
                "raw_value_sha256": "2f6535964836f84a6109cda2cfd8603a977b64a00c1d6db2a6e3eb754a6f5370",
                "raw_value_byte_length": 8
            }],
            "time_values": [{
                "tag": "0008,0030",
                "keyword": "StudyTime",
                "vr": "TM",
                "decoded_value": "235959.999999",
                "raw_value_hex": "3233353935392E39393939393920",
                "raw_value_sha256": "9a0aa44898ffd1f02e8896c0083f939cf2fcfaa838084947aa8f9035ed094fad",
                "raw_value_byte_length": 14
            }],
            "date_time_values": [{
                "tag": "0008,002A",
                "keyword": "AcquisitionDateTime",
                "vr": "DT",
                "decoded_value": "20240229235959.999999+1400",
                "raw_value_hex": "32303234303232393233353935392E3939393939392B31343030",
                "raw_value_sha256": "81ba5e3ca1486cfdb6d2ca599c135950a3e406cfc0d890aafc06bcdd2a806252",
                "raw_value_byte_length": 26,
                "embedded_offset_minutes": 840,
                "normalized_utc": "2024-02-29T09:59:59.999999Z"
            }],
            "combined_da_tm_utc": "2024-02-29T09:59:59.999999Z"
        }
    })
}

#[test]
fn manifest_schema_types_enhanced_instance_uids() {
    let schema = read_json("schemas/manifest.schema.json");
    let uid_properties = schema
        .pointer("/$defs/uids/properties")
        .and_then(Value::as_object)
        .expect("manifest UID properties should be an object");

    for property in ["dimension_organization_uid", "irradiation_event_uid"] {
        assert_eq!(
            uid_properties
                .get(property)
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("string"),
            "{property} should be an optional typed UID"
        );
    }
}

#[test]
fn manifest_schema_types_external_generation_and_float_pixels() {
    let schema = read_json("schemas/manifest.schema.json");
    let backend_required = schema
        .pointer("/$defs/generation_backend/required")
        .and_then(Value::as_array)
        .expect("manifest schema must type generation backend provenance");
    for field in [
        "backend_id",
        "protocol_version",
        "dependency_lock_sha256",
        "executable_fingerprint",
        "entrypoint_fingerprint",
        "environment_fingerprint",
        "runtime_identity",
        "determinism",
    ] {
        assert!(
            backend_required
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "generation backend provenance must require {field}"
        );
    }

    let sample_types = schema
        .pointer("/$defs/image/properties/sample_type/enum")
        .and_then(Value::as_array)
        .expect("image metadata must enumerate sample types");
    for sample_type in ["float32", "float64"] {
        assert!(
            sample_types
                .iter()
                .any(|value| value.as_str() == Some(sample_type)),
            "image metadata must distinguish {sample_type} samples"
        );
    }
    let image_required = schema
        .pointer("/$defs/image/required")
        .and_then(Value::as_array)
        .expect("image schema must define required fields");
    for integer_only in ["bits_stored", "high_bit", "pixel_representation"] {
        assert!(
            !image_required
                .iter()
                .any(|value| value.as_str() == Some(integer_only)),
            "float image metadata must not globally require {integer_only}"
        );
    }
    assert!(
        schema
            .pointer("/$defs/image/allOf")
            .and_then(Value::as_array)
            .is_some_and(|rules| !rules.is_empty()),
        "image schema must conditionally preserve integer pixel requirements"
    );
}

#[test]
fn manifest_schema_locks_float64_parametric_map_pixel_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let file_rules = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("manifest files must define conditional contracts");
    let rule = file_rules
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/parametric-map/float64_ct_derived_explicit_le")
        })
        .expect("float64 Parametric Map must have a case-specific manifest contract");

    for (pointer, expected) in [
        (
            "/then/properties/image/properties/sample_type/const",
            serde_json::json!("float64"),
        ),
        (
            "/then/properties/image/properties/rows/const",
            serde_json::json!(2),
        ),
        (
            "/then/properties/image/properties/columns/const",
            serde_json::json!(2),
        ),
        (
            "/then/properties/image/properties/frames/const",
            serde_json::json!(3),
        ),
        (
            "/then/properties/image/properties/bits_allocated/const",
            serde_json::json!(64),
        ),
        (
            "/then/properties/pixel_data/properties/vr/const",
            serde_json::json!("OD"),
        ),
        (
            "/then/properties/pixel_data/properties/value_length/const",
            serde_json::json!(96),
        ),
        (
            "/then/properties/pixel_data/properties/frame_count/const",
            serde_json::json!(3),
        ),
    ] {
        assert_eq!(
            rule.pointer(pointer),
            Some(&expected),
            "unexpected contract at {pointer}"
        );
    }

    let image_rules = schema
        .pointer("/$defs/image/allOf")
        .and_then(Value::as_array)
        .expect("image metadata must define conditional contracts");
    for (sample_type, bits_allocated) in [("float32", 32), ("float64", 64)] {
        let rule = image_rules
            .iter()
            .find(|rule| {
                rule.pointer("/if/properties/sample_type/const")
                    .and_then(Value::as_str)
                    == Some(sample_type)
            })
            .unwrap_or_else(|| panic!("missing {sample_type} width rule"));
        assert_eq!(
            rule.pointer("/then/properties/bits_allocated/const")
                .and_then(Value::as_u64),
            Some(bits_allocated)
        );
    }
}

#[test]
fn manifest_schema_types_tid1500_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_tid1500",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("TID 1500 expectation schema should compile");
    let expectation = tid1500_expectation();
    assert!(
        validator.is_valid(&expectation),
        "the locked TID 1500 semantic contract should pass"
    );

    let mut wrong_template = expectation.clone();
    wrong_template["root_template"]["template_identifier"] = serde_json::json!("1501");
    assert!(!validator.is_valid(&wrong_template));

    let mut wrong_measurement = expectation.clone();
    wrong_measurement["measurement_group"]["measurement"]["numeric_value"] =
        serde_json::json!("5.624");
    assert!(!validator.is_valid(&wrong_measurement));

    let mut segment_frames = expectation.clone();
    segment_frames["measurement_group"]["referenced_segment"]["referenced_frame_numbers"] =
        serde_json::json!([1, 2]);
    assert!(
        !validator.is_valid(&segment_frames),
        "the all-frames segment reference must omit Referenced Frame Number"
    );

    let mut reversed_evidence = expectation.clone();
    reversed_evidence["evidence"]
        .as_array_mut()
        .expect("evidence should be an array")
        .reverse();
    assert!(
        !validator.is_valid(&reversed_evidence),
        "evidence order must remain CT then SEG"
    );

    let mut unexpected = expectation;
    unexpected["image_library"] = serde_json::json!(true);
    assert!(
        !validator.is_valid(&unexpected),
        "the deliberately omitted image library must not enter the contract"
    );
}

#[test]
fn manifest_schema_requires_tid1500_contract_for_tid1500_case() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/sr/tid1500_ct_measurement_report")
        })
        .expect("manifest schema should define the TID 1500 case conditional");

    let required = rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("TID 1500 conditional should require specialized fields");
    for field in ["generation_backend", "expected_tid1500"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "TID 1500 conditional must require {field}"
        );
    }
    for (pointer, expected) in [
        (
            "/then/properties/dicom/properties/sop_class_uid/const",
            serde_json::json!("1.2.840.10008.5.1.4.1.1.88.34"),
        ),
        (
            "/then/properties/dicom/properties/iod_name/const",
            serde_json::json!("Comprehensive 3D SR"),
        ),
        (
            "/then/properties/dicom/properties/modality/const",
            serde_json::json!("SR"),
        ),
    ] {
        assert_eq!(rule.pointer(pointer), Some(&expected));
    }
    assert_eq!(
        rule.pointer("/then/properties/image/type")
            .and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type")
            .and_then(Value::as_str),
        Some("null")
    );
}

fn tid1500_expectation() -> Value {
    serde_json::json!({
        "completion_flag": "COMPLETE",
        "preliminary_flag": "FINAL",
        "verification_flag": "UNVERIFIED",
        "root_template": {
            "mapping_resource": "DCMR",
            "template_identifier": "1500"
        },
        "document_title": {
            "code_value": "126000",
            "coding_scheme_designator": "DCM",
            "code_meaning": "Imaging Measurement Report"
        },
        "observation_context": {
            "observer_type": "DEVICE",
            "device_observer_uid": "1.2.826.0.1.3680043.10.543.1"
        },
        "procedure_reported": {
            "code_value": "25045-6",
            "coding_scheme_designator": "LN",
            "code_meaning": "CT unspecified body region"
        },
        "imaging_measurements": {
            "code_value": "126010",
            "coding_scheme_designator": "DCM",
            "code_meaning": "Imaging Measurements"
        },
        "measurement_group": {
            "container": {
                "code_value": "125007",
                "coding_scheme_designator": "DCM",
                "code_meaning": "Measurement Group"
            },
            "tracking_identifier": "DTS-TID1500-ROI-1",
            "tracking_uid": "1.2.826.0.1.3680043.10.543.2",
            "finding": {
                "code_value": "123037004",
                "coding_scheme_designator": "SCT",
                "code_meaning": "Body structure"
            },
            "referenced_segment": {
                "source_case_id": "derived/seg/binary_multiframe_explicit_le",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.66.4",
                "sop_instance_uid": "1.2.826.0.1.3680043.10.543.3",
                "series_instance_uid": "1.2.826.0.1.3680043.10.543.30",
                "segment_number": 1,
                "referenced_frame_numbers": null,
                "source_image": {
                    "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
                    "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
                    "series_instance_uid": "1.2.826.0.1.3680043.10.543.40",
                    "referenced_frame_numbers": [1, 2]
                }
            },
            "measurement": {
                "name": {
                    "code_value": "118565006",
                    "coding_scheme_designator": "SCT",
                    "code_meaning": "Volume"
                },
                "numeric_value": "5.625",
                "units": {
                    "code_value": "mm3",
                    "coding_scheme_designator": "UCUM",
                    "code_meaning": "cubic millimeter"
                }
            }
        },
        "evidence": [
            {
                "role": "source_image",
                "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
                "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
                "series_instance_uid": "1.2.826.0.1.3680043.10.543.40"
            },
            {
                "role": "referenced_segmentation",
                "source_case_id": "derived/seg/binary_multiframe_explicit_le",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.66.4",
                "sop_instance_uid": "1.2.826.0.1.3680043.10.543.3",
                "series_instance_uid": "1.2.826.0.1.3680043.10.543.30"
            }
        ]
    })
}

#[test]
fn manifest_schema_types_spatial_registration_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_spatial_registration",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("Spatial Registration expectation schema should compile");
    let expectation = spatial_registration_expectation();
    assert!(validator.is_valid(&expectation));

    let mut reversed = expectation.clone();
    reversed["matrix_direction"] = serde_json::json!("registered_to_source");
    assert!(!validator.is_valid(&reversed));

    let mut wrong_translation = expectation.clone();
    wrong_translation["registration_items"][1]["matrix"]["values"][11] = serde_json::json!(-2.5);
    assert!(!validator.is_valid(&wrong_translation));

    let mut wrong_order = expectation.clone();
    wrong_order["registration_items"]
        .as_array_mut()
        .expect("registration items")
        .swap(0, 1);
    assert!(!validator.is_valid(&wrong_order));

    let mut extra_other_study = expectation;
    let duplicate = extra_other_study["common_instance_reference"]["other_studies"][0].clone();
    extra_other_study["common_instance_reference"]["other_studies"]
        .as_array_mut()
        .expect("other studies")
        .push(duplicate);
    assert!(!validator.is_valid(&extra_other_study));
}

#[test]
fn manifest_schema_types_deformable_registration_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_deformable_spatial_registration",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("Deformable Spatial Registration expectation schema should compile");
    let expectation = deformable_registration_expectation();
    assert!(validator.is_valid(&expectation));

    let mut reversed = expectation.clone();
    reversed["sampling_direction"] = serde_json::json!("source_to_registered");
    assert!(!validator.is_valid(&reversed));

    let mut swapped = expectation.clone();
    swapped["grid"]["vectors_mm"]
        .as_array_mut()
        .expect("vectors")
        .swap(1, 2);
    assert!(!validator.is_valid(&swapped));

    let mut truncated = expectation.clone();
    truncated["grid"]["byte_length"] = serde_json::json!(44);
    assert!(!validator.is_valid(&truncated));

    let mut non_identity = expectation;
    non_identity["pre_deformation_matrix"]["values"][3] = serde_json::json!(1);
    assert!(!validator.is_valid(&non_identity));
}

#[test]
fn manifest_schema_types_color_softcopy_presentation_state_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_color_softcopy_presentation_state",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("Color Softcopy Presentation State expectation schema should compile");
    let expectation = color_softcopy_presentation_state_expectation();
    assert!(validator.is_valid(&expectation));

    let mut wrong_presentation_identity = expectation.clone();
    wrong_presentation_identity["presentation_state"]["laterality"] = serde_json::json!("L");
    assert!(!validator.is_valid(&wrong_presentation_identity));

    let mut wrong_source_shape = expectation.clone();
    wrong_source_shape["source"]["columns"] = serde_json::json!(3);
    assert!(!validator.is_valid(&wrong_source_shape));

    let mut frame_scoped = expectation.clone();
    frame_scoped["relationship"]["referenced_frame_numbers"] = serde_json::json!([1]);
    assert!(!validator.is_valid(&frame_scoped));

    let mut incomplete_displayed_area = expectation.clone();
    incomplete_displayed_area["displayed_area"]["bottom_right"] = serde_json::json!([1, 2]);
    assert!(!validator.is_valid(&incomplete_displayed_area));

    let mut wrong_icc_header = expectation.clone();
    wrong_icc_header["icc_profile"]["data_color_space"] = serde_json::json!("RGB");
    assert!(!validator.is_valid(&wrong_icc_header));

    let mut wrong_icc_hash = expectation.clone();
    wrong_icc_hash["icc_profile"]["sha256"] = serde_json::json!(format!("{:064x}", 1));
    assert!(!validator.is_valid(&wrong_icc_hash));

    let mut unexpected_graphics = expectation.clone();
    unexpected_graphics["graphic_layer_items"] = serde_json::json!(1);
    assert!(!validator.is_valid(&unexpected_graphics));

    let mut unexpected_pixels = expectation;
    unexpected_pixels["pixel_data_absent"] = serde_json::json!(false);
    assert!(!validator.is_valid(&unexpected_pixels));
}

#[test]
fn manifest_schema_requires_color_softcopy_presentation_state_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/presentation-state/color_softcopy")
        })
        .expect("manifest schema should define the Color Softcopy PR conditional");
    assert!(
        rule.pointer("/then/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required
                .iter()
                .any(|field| field == "expected_color_softcopy_presentation_state"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&serde_json::json!("1.2.840.10008.5.1.4.1.1.11.2"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/modality/const"),
        Some(&serde_json::json!("PR"))
    );
    assert_eq!(
        rule.pointer("/then/properties/image/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type"),
        Some(&serde_json::json!("null"))
    );
}

fn color_softcopy_presentation_state_expectation() -> Value {
    serde_json::json!({
        "presentation_state": {
            "modality": "PR",
            "body_part_examined": "HAND",
            "laterality": "R",
            "content_label": "DTSCOLORPR",
            "content_description": "Synthetic RGB color presentation state",
            "presentation_creation_date": "20260101",
            "presentation_creation_time": "000000",
            "instance_number": 1,
            "series_number": 62
        },
        "source": {
            "source_case_id": "classic/sc/rgb_planar0_explicit_le",
            "source_path": "classic/sc/rgb_planar0_explicit_le/instance.dcm",
            "source_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "study_instance_uid": "1.2.826.0.1.3680043.10.543.11",
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.12",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.13",
            "rows": 2,
            "columns": 2,
            "photometric_interpretation": "RGB",
            "samples_per_pixel": 3,
            "planar_configuration": 0,
            "complete_instance": true
        },
        "same_study": true,
        "different_series": true,
        "relationship": {
            "referenced_series_items": 1,
            "referenced_image_items": 1,
            "referenced_frame_numbers": [],
            "applies_to_complete_instance": true
        },
        "displayed_area": {
            "items": 1,
            "applies_to_all_references": true,
            "top_left": [1, 1],
            "bottom_right": [2, 2],
            "presentation_size_mode": "SCALE TO FIT",
            "presentation_pixel_aspect_ratio": [1, 1],
            "presentation_pixel_spacing": null,
            "presentation_pixel_magnification_ratio": null
        },
        "icc_profile": {
            "vr": "OB",
            "size_bytes": 736,
            "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
            "device_class": "scnr",
            "data_color_space": "RGB ",
            "profile_connection_space": "XYZ ",
            "signature": "acsp",
            "dicom_color_space": "SRGB"
        },
        "shutter_items": 0,
        "graphic_annotation_items": 0,
        "graphic_layer_items": 0,
        "overlay_items": 0,
        "spatial_transform_present": false,
        "pixel_data_absent": true
    })
}

#[test]
fn manifest_schema_types_blending_presentation_state_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_blending_presentation_state",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("Blending Softcopy Presentation State expectation schema should compile");
    let expectation = blending_presentation_state_expectation();
    assert!(validator.is_valid(&expectation));

    let mut reordered_sources = expectation.clone();
    reordered_sources["sources"]
        .as_array_mut()
        .expect("sources")
        .swap(0, 1);
    assert!(!validator.is_valid(&reordered_sources));

    let mut duplicate_position = expectation.clone();
    duplicate_position["blending_items"][1]["blending_position"] = serde_json::json!("UNDERLYING");
    assert!(!validator.is_valid(&duplicate_position));

    let mut reordered_images = expectation.clone();
    reordered_images["blending_items"][0]["referenced_source_indices"] = serde_json::json!([2, 1]);
    assert!(!validator.is_valid(&reordered_images));

    let mut wrong_rescale = expectation.clone();
    wrong_rescale["blending_items"][0]["rescale_intercept"] = serde_json::json!(0);
    assert!(!validator.is_valid(&wrong_rescale));

    let mut unexpected_voi = expectation.clone();
    unexpected_voi["blending_items"][1]["softcopy_voi_lut_items"] = serde_json::json!(1);
    assert!(!validator.is_valid(&unexpected_voi));

    let mut wrong_opacity = expectation.clone();
    wrong_opacity["relative_opacity"] = serde_json::json!(0.75);
    assert!(!validator.is_valid(&wrong_opacity));

    let mut scoped_displayed_area = expectation.clone();
    scoped_displayed_area["displayed_area"]["referenced_image_items"] = serde_json::json!(4);
    assert!(!validator.is_valid(&scoped_displayed_area));

    let mut corrupt_palette = expectation.clone();
    corrupt_palette["palette_color_lut"]["channels"][2]["data_sha256"] =
        serde_json::json!(format!("{:064x}", 1));
    assert!(!validator.is_valid(&corrupt_palette));

    let mut corrupt_icc = expectation.clone();
    corrupt_icc["icc_profile"]["sha256"] = serde_json::json!(format!("{:064x}", 1));
    assert!(!validator.is_valid(&corrupt_icc));

    let mut forbidden_module = expectation.clone();
    forbidden_module["absent_modules"]["frame_of_reference"] = serde_json::json!(false);
    assert!(!validator.is_valid(&forbidden_module));

    let mut unexpected_pixels = expectation;
    unexpected_pixels["pixel_data_absent"] = serde_json::json!(false);
    assert!(!validator.is_valid(&unexpected_pixels));
}

#[test]
fn manifest_schema_requires_exclusive_blending_presentation_state_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/presentation-state/blending")
        })
        .expect("manifest schema should define the Blending Softcopy PR conditional");
    assert!(
        rule.pointer("/then/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required
                .iter()
                .any(|field| field == "expected_blending_presentation_state"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&serde_json::json!("1.2.840.10008.5.1.4.1.1.11.4"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/modality/const"),
        Some(&serde_json::json!("PR"))
    );
    assert_eq!(
        rule.pointer("/then/properties/image/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_blending_presentation_state"]))
    );
}

fn blending_presentation_state_expectation() -> Value {
    let study_uid = "1.2.826.0.1.3680043.10.543.10";
    let series_uids = [
        "1.2.826.0.1.3680043.10.543.31",
        "1.2.826.0.1.3680043.10.543.32",
    ];
    let sop_uids = [
        "1.2.826.0.1.3680043.10.543.41",
        "1.2.826.0.1.3680043.10.543.42",
        "1.2.826.0.1.3680043.10.543.43",
        "1.2.826.0.1.3680043.10.543.44",
    ];
    let source = |path: &str, series_order: usize, image_order: usize, z: f64, index: usize| {
        serde_json::json!({
            "source_case_id": "geometry/ct/multiseries_shared_frame_of_reference",
            "source_path": path,
            "source_sha256": format!("{:064x}", index + 1),
            "study_instance_uid": study_uid,
            "series_instance_uid": series_uids[series_order - 1],
            "frame_of_reference_uid": "1.2.826.0.1.3680043.10.543.20",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
            "sop_instance_uid": sop_uids[index],
            "series_order": series_order,
            "image_order": image_order,
            "rows": 2,
            "columns": 2,
            "image_orientation_patient": [1, 0, 0, 0, 1, 0],
            "image_position_patient_mm": [0, 0, z],
            "referenced_frame_numbers": [],
            "complete_instance": true
        })
    };
    let blending_item = |position: &str, series_order: usize, indices: [usize; 2]| {
        serde_json::json!({
            "blending_position": position,
            "source_series_order": series_order,
            "study_instance_uid": study_uid,
            "series_instance_uid": series_uids[series_order - 1],
            "referenced_source_indices": indices,
            "referenced_frame_numbers": [],
            "rescale_intercept": -1024,
            "rescale_slope": 1,
            "rescale_type": "HU",
            "softcopy_voi_lut_items": 0,
            "referenced_spatial_registration_items": 0,
            "complete_instances": true
        })
    };
    let palette_channel = |channel: &str| {
        serde_json::json!({
            "channel": channel,
            "descriptor": [256, 0, 16],
            "data_vr": "OW",
            "data_size_bytes": 512,
            "data_sha256": "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5",
            "storage": "identity_u16_little_endian"
        })
    };

    serde_json::json!({
        "presentation_state": {
            "study_instance_uid": study_uid,
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.50",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.51",
            "modality": "PR",
            "laterality": "R",
            "content_label": "DTSBLEND",
            "content_description": "Synthetic DTSBLEND presentation state",
            "content_creator_name": "DTS^Generator",
            "presentation_creation_date": "20260101",
            "presentation_creation_time": "000000",
            "instance_number": 1,
            "series_number": 81
        },
        "sources": [
            source("geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm", 1, 1, 0.0, 0),
            source("geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm", 1, 2, 5.0, 1),
            source("geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm", 2, 1, 0.0, 2),
            source("geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm", 2, 2, 5.0, 3)
        ],
        "same_study": true,
        "shared_frame_of_reference": true,
        "different_series": true,
        "blending_items": [
            blending_item("UNDERLYING", 1, [1, 2]),
            blending_item("SUPERIMPOSED", 2, [3, 4])
        ],
        "relative_opacity": 0.5,
        "displayed_area": {
            "items": 1,
            "applies_to_all_references": true,
            "referenced_image_items": 0,
            "top_left": [1, 1],
            "bottom_right": [2, 2],
            "presentation_size_mode": "SCALE TO FIT",
            "presentation_pixel_aspect_ratio": [1, 1],
            "presentation_pixel_spacing": null,
            "presentation_pixel_magnification_ratio": null
        },
        "palette_color_lut": {
            "channels": [palette_channel("red"), palette_channel("green"), palette_channel("blue")],
            "segmented_data_present": false,
            "palette_uid_present": false
        },
        "icc_profile": {
            "vr": "OB",
            "size_bytes": 736,
            "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
            "device_class": "scnr",
            "data_color_space": "RGB ",
            "profile_connection_space": "XYZ ",
            "signature": "acsp",
            "dicom_color_space": "SRGB"
        },
        "absent_modules": {
            "clinical_trial_subject": true,
            "clinical_trial_study": true,
            "clinical_trial_series": true,
            "clinical_trial_equipment": true,
            "patient_study": true,
            "specimen": true,
            "graphic_annotation": true,
            "graphic_layer": true,
            "graphic_group": true,
            "spatial_transformation": true,
            "frame_of_reference": true,
            "common_instance_reference": true,
            "softcopy_presentation_lut": true,
            "voi_lut": true,
            "softcopy_voi_lut": true,
            "overlay_plane": true,
            "overlay_activation": true,
            "display_shutter": true,
            "bitmap_display_shutter": true
        },
        "pixel_data_absent": true
    })
}

#[test]
fn manifest_schema_types_advanced_blending_presentation_state_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_advanced_blending_presentation_state",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("Advanced Blending Presentation State expectation schema should compile");
    let expectation = advanced_blending_presentation_state_expectation();
    assert!(validator.is_valid(&expectation));

    let mut reordered_sources = expectation.clone();
    reordered_sources["sources"]
        .as_array_mut()
        .expect("sources")
        .swap(0, 1);
    assert!(!validator.is_valid(&reordered_sources));

    let mut duplicate_input_number = expectation.clone();
    duplicate_input_number["blending_inputs"][1]["input_number"] = serde_json::json!(1);
    assert!(!validator.is_valid(&duplicate_input_number));

    let mut wrong_time_series_flag = expectation.clone();
    wrong_time_series_flag["blending_inputs"][0]["time_series_blending"] =
        serde_json::json!("TRUE");
    assert!(!validator.is_valid(&wrong_time_series_flag));

    let mut second_geometry_source = expectation.clone();
    second_geometry_source["blending_inputs"][1]["geometry_for_display"] =
        serde_json::json!("TRUE");
    assert!(!validator.is_valid(&second_geometry_source));

    let mut dangling_display_input = expectation.clone();
    dangling_display_input["display_operation"]["input_numbers"] = serde_json::json!([1, 3]);
    assert!(!validator.is_valid(&dangling_display_input));

    let mut foreground_blend = expectation.clone();
    foreground_blend["display_operation"]["blending_mode"] = serde_json::json!("FOREGROUND");
    foreground_blend["display_operation"]["relative_opacity"] = serde_json::json!(0.5);
    assert!(!validator.is_valid(&foreground_blend));

    let mut intermediate_output = expectation.clone();
    intermediate_output["display_operation"]["output_blending_input_number"] = serde_json::json!(3);
    assert!(!validator.is_valid(&intermediate_output));

    let mut incomplete_common_reference = expectation.clone();
    incomplete_common_reference["common_instance_reference"]["series"][1]["referenced_source_indices"] =
        serde_json::json!([3]);
    assert!(!validator.is_valid(&incomplete_common_reference));

    let mut corrupt_icc = expectation.clone();
    corrupt_icc["icc_profile"]["sha256"] = serde_json::json!(format!("{:064x}", 1));
    assert!(!validator.is_valid(&corrupt_icc));

    let mut optional_transform = expectation.clone();
    optional_transform["optional_transforms"]["softcopy_voi_lut_items"] = serde_json::json!(1);
    assert!(!validator.is_valid(&optional_transform));

    let mut unexpected_pixels = expectation;
    unexpected_pixels["pixel_data_absent"] = serde_json::json!(false);
    assert!(!validator.is_valid(&unexpected_pixels));
}

#[test]
fn manifest_schema_requires_advanced_blending_presentation_state_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/presentation-state/advanced_blending")
        })
        .expect("manifest schema should define the Advanced Blending PR conditional");
    assert!(
        rule.pointer("/then/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required
                .iter()
                .any(|field| field == "expected_advanced_blending_presentation_state"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&serde_json::json!("1.2.840.10008.5.1.4.1.1.11.8"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/modality/const"),
        Some(&serde_json::json!("PR"))
    );
    assert_eq!(
        rule.pointer("/then/properties/image/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type"),
        Some(&serde_json::json!("null"))
    );
}

fn advanced_blending_presentation_state_expectation() -> Value {
    let study_uid = "1.2.826.0.1.3680043.10.543.10";
    let frame_of_reference_uid = "1.2.826.0.1.3680043.10.543.20";
    let series_uids = [
        "1.2.826.0.1.3680043.10.543.31",
        "1.2.826.0.1.3680043.10.543.32",
    ];
    let sop_uids = [
        "1.2.826.0.1.3680043.10.543.41",
        "1.2.826.0.1.3680043.10.543.42",
        "1.2.826.0.1.3680043.10.543.43",
        "1.2.826.0.1.3680043.10.543.44",
    ];
    let source = |path: &str, series_order: usize, image_order: usize, z: f64, index: usize| {
        serde_json::json!({
            "source_case_id": "geometry/ct/multiseries_shared_frame_of_reference",
            "source_path": path,
            "source_sha256": format!("{:064x}", index + 1),
            "study_instance_uid": study_uid,
            "series_instance_uid": series_uids[series_order - 1],
            "frame_of_reference_uid": frame_of_reference_uid,
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
            "sop_instance_uid": sop_uids[index],
            "series_order": series_order,
            "image_order": image_order,
            "rows": 2,
            "columns": 2,
            "image_orientation_patient": [1, 0, 0, 0, 1, 0],
            "image_position_patient_mm": [0, 0, z],
            "referenced_frame_numbers": [],
            "complete_instance": true
        })
    };

    serde_json::json!({
        "presentation_state": {
            "study_instance_uid": study_uid,
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.50",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.51",
            "frame_of_reference_uid": frame_of_reference_uid,
            "position_reference_indicator": "",
            "modality": "PR",
            "laterality": "R",
            "content_label": "DTSADVBLEND",
            "content_description": "Synthetic DTSADVBLEND presentation state",
            "content_creator_name": "DTS^Generator",
            "presentation_creation_date": "20260101",
            "presentation_creation_time": "000000",
            "instance_number": 1,
            "series_number": 80
        },
        "sources": [
            source("geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm", 1, 1, 0.0, 0),
            source("geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm", 1, 2, 5.0, 1),
            source("geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm", 2, 1, 0.0, 2),
            source("geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm", 2, 2, 5.0, 3)
        ],
        "same_study": true,
        "shared_frame_of_reference": true,
        "different_series": true,
        "blending_inputs": [
            {
                "input_number": 1,
                "source_series_order": 1,
                "study_instance_uid": study_uid,
                "series_instance_uid": series_uids[0],
                "referenced_source_indices": [1, 2],
                "time_series_blending": "FALSE",
                "geometry_for_display": "TRUE",
                "complete_instances": true
            },
            {
                "input_number": 2,
                "source_series_order": 2,
                "study_instance_uid": study_uid,
                "series_instance_uid": series_uids[1],
                "referenced_source_indices": [3, 4],
                "time_series_blending": "FALSE",
                "geometry_for_display": "FALSE",
                "complete_instances": true
            }
        ],
        "pixel_presentation": "TRUE_COLOR",
        "display_operation": {
            "items": 1,
            "input_numbers": [1, 2],
            "blending_mode": "EQUAL",
            "relative_opacity": null,
            "output_blending_input_number": null,
            "final_output": true
        },
        "icc_profile": {
            "vr": "OB",
            "size_bytes": 736,
            "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
            "device_class": "scnr",
            "data_color_space": "RGB ",
            "profile_connection_space": "XYZ ",
            "signature": "acsp",
            "dicom_color_space": "SRGB"
        },
        "common_instance_reference": {
            "series": [
                {
                    "series_order": 1,
                    "series_instance_uid": series_uids[0],
                    "referenced_source_indices": [1, 2]
                },
                {
                    "series_order": 2,
                    "series_instance_uid": series_uids[1],
                    "referenced_source_indices": [3, 4]
                }
            ],
            "other_study_items": 0,
            "mirrors_blending_inputs": true
        },
        "optional_transforms": {
            "referenced_spatial_registration_items": 0,
            "optical_path_selection_items": 0,
            "softcopy_voi_lut_items": 0,
            "palette_color_lut_items": 0,
            "threshold_items": 0,
            "displayed_area_items": 0,
            "graphic_annotation_items": 0,
            "graphic_group_items": 0,
            "specimen_items": 0,
            "spatial_transform_present": false,
            "graphic_layer_items": 0
        },
        "pixel_data_absent": true
    })
}

#[test]
fn manifest_schema_requires_deformable_registration_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/registration/deformable_ct_pair")
        })
        .expect("manifest schema should define the Deformable Registration conditional");
    assert!(
        rule.pointer("/then/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required
                .iter()
                .any(|field| field == "expected_deformable_spatial_registration"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&serde_json::json!("1.2.840.10008.5.1.4.1.1.66.3"))
    );
    assert_eq!(
        rule.pointer("/then/properties/image/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type"),
        Some(&serde_json::json!("null"))
    );
}

fn deformable_registration_expectation() -> Value {
    let identity = |case_id: &str, path: &str, sop_class_uid: &str, suffix: u8| {
        serde_json::json!({
            "source_case_id": case_id,
            "source_path": path,
            "source_sha256": format!("{:064x}", suffix),
            "study_instance_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}1"),
            "series_instance_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}2"),
            "sop_class_uid": sop_class_uid,
            "sop_instance_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}3"),
            "frame_of_reference_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}4")
        })
    };
    let target = identity(
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
        "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
        "1.2.840.10008.5.1.4.1.1.2.1",
        1,
    );
    let source = identity(
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
        "classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm",
        "1.2.840.10008.5.1.4.1.1.2",
        2,
    );
    let identity_matrix = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    serde_json::json!({
        "registered_frame_of_reference_uid": target["frame_of_reference_uid"],
        "sampling_direction": "registered_to_source",
        "source": source,
        "complete_instance": true,
        "deformable_registration_items": 1,
        "registration_type_code_items": 0,
        "pre_deformation_matrix": {"items": 1, "type": "RIGID", "values": identity_matrix},
        "post_deformation_matrix": {"items": 1, "type": "RIGID", "values": identity_matrix},
        "grid": {
            "items": 1,
            "image_position_patient_mm": [0,0,2.5],
            "image_orientation_patient": [1,0,0,0,1,0],
            "dimensions": [2,2,1],
            "resolution_mm": [0.75,0.75,2.5],
            "vector_data_vr": "OF",
            "vector_data_vm": 1,
            "vector_count": 4,
            "component_count": 12,
            "byte_length": 48,
            "payload_sha256": "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47",
            "byte_order": "little_endian_ieee754_binary32",
            "index_order": "i_fastest_then_j_then_k",
            "vectors_mm": [
                [-0.625,-0.625,-2.5],
                [-0.75,-0.625,-2.5],
                [-0.625,-0.75,-2.5],
                [-0.75,-0.75,-2.5]
            ]
        },
        "point_mappings": [
            {"registered_point_mm":[0,0,2.5], "source_point_mm":[-0.625,-0.625,0], "tolerance_mm":0.000001},
            {"registered_point_mm":[0.75,0,2.5], "source_point_mm":[0,-0.625,0], "tolerance_mm":0.000001},
            {"registered_point_mm":[0,0.75,2.5], "source_point_mm":[-0.625,0,0], "tolerance_mm":0.000001},
            {"registered_point_mm":[0.75,0.75,2.5], "source_point_mm":[0,0,0], "tolerance_mm":0.000001}
        ],
        "common_instance_reference": {"same_study": target, "other_studies": [source]},
        "pixel_data_absent": true
    })
}

#[test]
fn manifest_schema_requires_spatial_registration_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/registration/spatial_ct_pair")
        })
        .expect("manifest schema should define the Spatial Registration conditional");
    assert!(
        rule.pointer("/then/required")
            .and_then(Value::as_array)
            .is_some_and(|required| required
                .iter()
                .any(|field| field == "expected_spatial_registration"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&serde_json::json!("1.2.840.10008.5.1.4.1.1.66.1"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/modality/const"),
        Some(&serde_json::json!("REG"))
    );
    assert_eq!(
        rule.pointer("/then/properties/image/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type"),
        Some(&serde_json::json!("null"))
    );
}

fn spatial_registration_expectation() -> Value {
    let identity = |case_id: &str, path: &str, sop_class_uid: &str, suffix: u8| {
        serde_json::json!({
            "source_case_id": case_id,
            "source_path": path,
            "source_sha256": format!("{:064x}", suffix),
            "study_instance_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}1"),
            "series_instance_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}2"),
            "sop_class_uid": sop_class_uid,
            "sop_instance_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}3"),
            "frame_of_reference_uid": format!("1.2.826.0.1.3680043.10.543.{suffix}4")
        })
    };
    let target = identity(
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
        "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
        "1.2.840.10008.5.1.4.1.1.2.1",
        1,
    );
    let source = identity(
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
        "classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm",
        "1.2.840.10008.5.1.4.1.1.2",
        2,
    );
    serde_json::json!({
        "registered_frame_of_reference_uid": target["frame_of_reference_uid"],
        "matrix_direction": "source_to_registered",
        "registration_items": [
            {
                "role": "registered_target",
                "source": target,
                "complete_instance": true,
                "matrix_registration_items": 1,
                "registration_type_code_items": 0,
                "matrix_items": 1,
                "matrix": {
                    "type": "RIGID",
                    "values": [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]
                }
            },
            {
                "role": "moving_source",
                "source": source,
                "complete_instance": true,
                "matrix_registration_items": 1,
                "registration_type_code_items": 0,
                "matrix_items": 1,
                "matrix": {
                    "type": "RIGID",
                    "values": [1,0,0,0.625, 0,1,0,0.625, 0,0,1,2.5, 0,0,0,1]
                }
            }
        ],
        "rigid_tolerances": {
            "orthonormal_abs": 0.000001,
            "determinant_abs": 0.000001,
            "homogeneous_abs": 0.000001
        },
        "landmark": {
            "source_point_mm": [-0.625,-0.625,0],
            "registered_point_mm": [0,0,2.5],
            "tolerance_mm": 0.000001
        },
        "common_instance_reference": {
            "same_study": target,
            "other_studies": [source]
        },
        "pixel_data_absent": true
    })
}

#[test]
fn manifest_schema_types_comprehensive3d_scoord3d_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_scoord3d",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("SCOORD3D expectation schema should compile");
    let expectation = scoord3d_expectation();
    assert!(
        validator.is_valid(&expectation),
        "the locked Comprehensive 3D SR semantic contract should pass"
    );

    let mut wrong_group_template = expectation.clone();
    wrong_group_template["measurement_group"]["template"]["template_identifier"] =
        serde_json::json!("1411");
    assert!(!validator.is_valid(&wrong_group_template));

    let mut wrong_distance = expectation.clone();
    wrong_distance["measurement_group"]["measurement"]["numeric_value"] = serde_json::json!("2.4");
    assert!(!validator.is_valid(&wrong_distance));

    let mut wrong_coordinate = expectation.clone();
    wrong_coordinate["measurement_group"]["measurement"]["spatial_coordinates"]["graphic_data_mm"]
        [5] = serde_json::json!(2.4);
    assert!(!validator.is_valid(&wrong_coordinate));

    let mut missing_fiducial = expectation.clone();
    missing_fiducial["measurement_group"]["measurement"]["spatial_coordinates"]
        .as_object_mut()
        .expect("spatial coordinates should be an object")
        .remove("fiducial_uid");
    assert!(!validator.is_valid(&missing_fiducial));

    let mut wrong_frames = expectation.clone();
    wrong_frames["measurement_group"]["source_image"]["referenced_frame_numbers"] =
        serde_json::json!([2, 1]);
    assert!(!validator.is_valid(&wrong_frames));

    let mut extra_evidence = expectation;
    extra_evidence["evidence"]
        .as_array_mut()
        .expect("evidence should be an array")
        .push(serde_json::json!({
            "role": "source_image",
            "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.40"
        }));
    assert!(
        !validator.is_valid(&extra_evidence),
        "the evidence closure must contain exactly the one referenced CT instance"
    );
}

#[test]
fn manifest_schema_requires_scoord3d_contract_for_comprehensive3d_case() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/sr/comprehensive3d_scoord3d")
        })
        .expect("manifest schema should define the Comprehensive 3D SCOORD3D conditional");

    let required = rule
        .pointer("/then/required")
        .and_then(Value::as_array)
        .expect("SCOORD3D conditional should require specialized fields");
    for field in ["generation_backend", "expected_scoord3d"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "SCOORD3D conditional must require {field}"
        );
    }
    for (pointer, expected) in [
        (
            "/then/properties/dicom/properties/sop_class_uid/const",
            serde_json::json!("1.2.840.10008.5.1.4.1.1.88.34"),
        ),
        (
            "/then/properties/dicom/properties/iod_name/const",
            serde_json::json!("Comprehensive 3D SR"),
        ),
        (
            "/then/properties/dicom/properties/modality/const",
            serde_json::json!("SR"),
        ),
    ] {
        assert_eq!(rule.pointer(pointer), Some(&expected));
    }
    assert_eq!(
        rule.pointer("/then/properties/image/type")
            .and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type")
            .and_then(Value::as_str),
        Some("null")
    );
}

fn scoord3d_expectation() -> Value {
    serde_json::json!({
        "completion_flag": "COMPLETE",
        "preliminary_flag": "FINAL",
        "verification_flag": "UNVERIFIED",
        "root_template": {
            "mapping_resource": "DCMR",
            "template_identifier": "1500"
        },
        "document_title": {
            "code_value": "126000",
            "coding_scheme_designator": "DCM",
            "code_meaning": "Imaging Measurement Report"
        },
        "observation_context": {
            "observer_type": "DEVICE",
            "device_observer_uid": "1.2.826.0.1.3680043.10.543.1"
        },
        "procedure_reported": {
            "code_value": "25045-6",
            "coding_scheme_designator": "LN",
            "code_meaning": "CT unspecified body region"
        },
        "imaging_measurements": {
            "code_value": "126010",
            "coding_scheme_designator": "DCM",
            "code_meaning": "Imaging Measurements"
        },
        "measurement_group": {
            "template": {
                "mapping_resource": "DCMR",
                "template_identifier": "1501"
            },
            "container": {
                "code_value": "125007",
                "coding_scheme_designator": "DCM",
                "code_meaning": "Measurement Group"
            },
            "tracking_identifier": "DTS-SCOORD3D-ROI-1",
            "tracking_uid": "1.2.826.0.1.3680043.10.543.2",
            "finding": {
                "code_value": "123037004",
                "coding_scheme_designator": "SCT",
                "code_meaning": "Body structure"
            },
            "measurement": {
                "name": {
                    "code_value": "121206",
                    "coding_scheme_designator": "DCM",
                    "code_meaning": "Distance"
                },
                "numeric_value": "2.5",
                "units": {
                    "code_value": "mm",
                    "coding_scheme_designator": "UCUM",
                    "code_meaning": "millimeter"
                },
                "spatial_coordinates": {
                    "relationship": "INFERRED FROM",
                    "value_type": "SCOORD3D",
                    "concept_name": {
                        "code_value": "260753009",
                        "coding_scheme_designator": "SCT",
                        "code_meaning": "Source"
                    },
                    "graphic_type": "POLYLINE",
                    "graphic_data_mm": [0.0, 0.0, 0.0, 0.0, 0.0, 2.5],
                    "frame_of_reference_uid": "1.2.826.0.1.3680043.10.543.5",
                    "fiducial_uid": "1.2.826.0.1.3680043.10.543.6"
                }
            },
            "source_image": {
                "relationship": "CONTAINS",
                "value_type": "IMAGE",
                "concept_name": {
                    "code_value": "121112",
                    "coding_scheme_designator": "DCM",
                    "code_meaning": "Source of Measurement"
                },
                "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
                "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
                "series_instance_uid": "1.2.826.0.1.3680043.10.543.40",
                "referenced_frame_numbers": [1, 2]
            }
        },
        "image_library_present": false,
        "evidence": [
            {
                "role": "source_image",
                "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
                "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
                "series_instance_uid": "1.2.826.0.1.3680043.10.543.40"
            }
        ]
    })
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
        "artifact_kind",
        "status",
        "provider",
        "object_family",
        "compatibility_axes",
        "roadmap",
        "blockers",
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

    assert!(
        schema.pointer("/$defs/case/properties/modality").is_some(),
        "case registry schema must allow optional modality metadata"
    );

    assert_eq!(
        schema
            .pointer("/properties/case_registry_schema_version/const")
            .and_then(Value::as_str),
        Some("0.2.0"),
        "roadmap metadata requires an explicit registry schema version"
    );

    let blocker_codes = schema
        .pointer("/$defs/blocker_code/enum")
        .and_then(Value::as_array)
        .expect("case registry schema must define controlled blocker codes");
    for code in [
        "recipe_unimplemented",
        "standards_verification_pending",
        "backend_contract_unimplemented",
        "independent_iod_validator_unavailable",
        "independent_payload_validator_unavailable",
        "numeric_tolerance_policy_pending",
        "stress_budget_policy_pending",
        "mutation_contract_unimplemented",
        "protocol_harness_unimplemented",
        "security_toolchain_unselected",
        "decision_checkpoint",
    ] {
        assert!(
            blocker_codes
                .iter()
                .any(|value| value.as_str() == Some(code)),
            "case registry schema must control blocker code {code}"
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
fn committed_case_registry_validates_against_its_schema() {
    let schema = read_json("schemas/case-registry.schema.json");
    let registry = read_json("cases/registry.json");
    let validator =
        jsonschema::validator_for(&schema).expect("case registry schema should compile");
    let errors = validator
        .iter_errors(&registry)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert!(
        errors.is_empty(),
        "cases/registry.json must validate against its schema:\n{}",
        errors.join("\n")
    );
}

#[test]
fn case_registry_schema_isolates_mutation_profiles() {
    let schema = read_json("schemas/case-registry.schema.json");
    let registry = read_json("cases/registry.json");
    let validator =
        jsonschema::validator_for(&schema).expect("case registry schema should compile");
    let mut case = registry["cases"][0].clone();

    for profiles in [
        serde_json::json!(["all"]),
        serde_json::json!(["negative", "core"]),
        serde_json::json!(["fuzz", "stress"]),
    ] {
        case["profiles"] = profiles;
        assert!(
            !validator.is_valid(&case),
            "invalid profile membership must be rejected: {}",
            case["profiles"]
        );
    }
}

#[test]
fn case_registry_schema_models_non_file_qualification_fixtures_honestly() {
    let schema = read_json("schemas/case-registry.schema.json");
    let validator =
        jsonschema::validator_for(&schema).expect("case registry schema should compile");
    let qualification = serde_json::json!({
        "case_registry_schema_version": "0.2.0",
        "cases": [{
            "case_id": "qualification/encapsulation/eot_u64_overflow",
            "artifact_kind": "qualification_fixture",
            "status": "implemented",
            "provider": {
                "kind": "rust_native",
                "id": "checked_eot_arithmetic"
            },
            "object_family": "robustness",
            "compatibility_axes": ["encapsulation", "robustness"],
            "roadmap": null,
            "blockers": [],
            "profiles": [],
            "recipe_id": "qualification_eot_u64_overflow",
            "recipe_version": "0.1.0",
            "iod_name": null,
            "sop_class_name": null,
            "sop_class_uid": null,
            "modality": null,
            "transfer_syntax_uid": null,
            "determinism": "byte_stable",
            "requirements": {
                "features": [],
                "external_codecs": [],
                "external_validators": []
            },
            "skip": null,
            "standards_evidence": [{
                "source": "local-source-note",
                "edition": "2026b",
                "query": "standards/source-notes/phase-5-extended-offset-table.md",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }]
        }]
    });
    assert!(
        validator.is_valid(&qualification),
        "an executable non-file qualification must be representable without DICOM identity"
    );

    let mut invalid = qualification.clone();
    invalid["cases"][0]["profiles"] = serde_json::json!(["stress"]);
    assert!(
        !validator.is_valid(&invalid),
        "qualification fixtures must not claim generated profile coverage"
    );

    for field in [
        "iod_name",
        "sop_class_name",
        "sop_class_uid",
        "modality",
        "transfer_syntax_uid",
    ] {
        let mut invalid = qualification.clone();
        invalid["cases"][0][field] = serde_json::json!("DICOM claim");
        assert!(
            !validator.is_valid(&invalid),
            "qualification fixtures must reject DICOM identity field {field}"
        );
    }

    let mut invalid = qualification;
    invalid["cases"][0]["standards_evidence"] = serde_json::json!([]);
    assert!(
        !validator.is_valid(&invalid),
        "qualification fixtures must retain standards evidence"
    );
}

#[test]
fn case_registry_schema_models_profile_runtime_qualifications_honestly() {
    let schema = read_json("schemas/case-registry.schema.json");
    let validator =
        jsonschema::validator_for(&schema).expect("case registry schema should compile");
    let registry = serde_json::json!({
        "case_registry_schema_version": "0.2.0",
        "cases": [{
            "case_id": "fuzz/parser/bounded_seed_corpus",
            "artifact_kind": "runtime_qualification",
            "status": "implemented",
            "provider": {"kind": "mutation_layer", "id": "bounded_deterministic_fuzz"},
            "object_family": "robustness",
            "compatibility_axes": ["robustness"],
            "roadmap": null,
            "blockers": [],
            "profiles": ["fuzz"],
            "recipe_id": "fuzz_parser_bounded_seed_corpus",
            "recipe_version": "0.1.0",
            "iod_name": null,
            "sop_class_name": null,
            "sop_class_uid": null,
            "modality": null,
            "transfer_syntax_uid": null,
            "determinism": "semantic_stable",
            "requirements": {
                "features": [],
                "external_codecs": [],
                "external_validators": []
            },
            "skip": null,
            "standards_evidence": [{
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "dicom-kb lookup uid SecondaryCaptureImageStorage --edition 2026b",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }]
        }]
    });
    assert!(validator.is_valid(&registry));

    let mut false_file_identity = registry.clone();
    false_file_identity["cases"][0]["sop_class_uid"] =
        Value::String("1.2.840.10008.5.1.4.1.1.7".to_string());
    assert!(
        !validator.is_valid(&false_file_identity),
        "runtime qualifications must not masquerade as generated DICOM instances"
    );
}

#[test]
fn coverage_report_schema_projects_approved_lossy_metrics_separately() {
    let schema = read_json("schemas/coverage-report.schema.json");
    let required = schema
        .pointer("/$defs/coverage_row/required")
        .and_then(Value::as_array)
        .expect("coverage rows must define required fields");
    assert!(required.iter().any(|field| field == "lossy_metrics"));

    let projection_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": schema["$defs"].clone(),
        "$ref": "#/$defs/lossy_metrics_projection"
    });
    let validator = jsonschema::validator_for(&projection_schema)
        .expect("lossy report projection schema must compile");
    let htj2k = serde_json::json!({
        "sample_domain": "unsigned_16_bit_little_endian",
        "sample_order": "monochrome",
        "sample_count": 1024,
        "dimensions": { "rows": 32, "columns": 32, "frames": 1 },
        "channels": [{
            "index": 0,
            "name": "MONOCHROME2",
            "sample_count": 1024,
            "max_absolute_error": { "observed": 19, "limit": 64 },
            "rmse": { "observed": 4.3548643779, "limit": 16 }
        }],
        "encoder": {
            "id": "openjph_htj2k_lossy_command_encoder",
            "version": "OpenJPH 0.27.3",
            "executable_sha256": "d21a8ea98ffce347928c34a2c51c61e424a068ca4eb746a6867a29d6c30b1627",
            "options": {
                "input_format": "binary_pgm_u16_big_endian",
                "argument_vector": ["-qstep", "0.00025"],
                "qstep": 0.00025,
                "reversible": false,
                "num_decompositions": 2,
                "colour_transform": false,
                "progression": "LRCP"
            },
            "options_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        },
        "overall_rmse": { "observed": 4.3548643779, "limit": 16 },
        "uncompressed_bytes": 2048,
        "compressed_bytes": 1476,
        "compression_ratio": { "numerator": 2048, "denominator": 1476, "computed": 1.3875338753387534, "dicom_value": "1.38753387533875" },
        "lossy_image_compression": "01",
        "lossy_image_compression_method": "ISO_15444_15",
        "decoder": {
            "id": "dicom_rs_openjpeg_htj2k_decoder",
            "version": "dicom-transfer-syntax-registry 0.9.1 + jpeg2k 0.10.1 + openjp2 0.6.1",
            "independence": "independent"
        }
    });
    assert!(validator.is_valid(&htj2k));
    let mut dependent = htj2k;
    dependent["decoder"]["independence"] = serde_json::json!("same_implementation");
    assert!(!validator.is_valid(&dependent));

    let lossy_rule = schema
        .pointer("/$defs/coverage_row/allOf/0")
        .expect("coverage rows must scope lossy metrics to approved cases");
    assert_eq!(
        lossy_rule
            .pointer("/if/properties/status/const")
            .and_then(Value::as_str),
        Some("generated"),
        "unavailable lossy rows must retain null metrics"
    );
    assert_eq!(
        lossy_rule
            .pointer("/then/allOf/0/then/properties/lossy_metrics/properties/overall_rmse/properties/limit/const")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        lossy_rule
            .pointer("/then/allOf/0/else/properties/lossy_metrics/properties/overall_rmse/properties/limit/const")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        lossy_rule
            .pointer("/else/properties/lossy_metrics/type")
            .and_then(Value::as_str),
        Some("null"),
        "all other valid coverage rows must project null lossy metrics"
    );
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
        "profile_membership",
        "status",
        "iod",
        "modality",
        "sop_class_uid",
        "sop_class_name",
        "transfer_syntax",
        "codec_family",
        "codec_backend_id",
        "codec_backend_kind",
        "codec_feature_gate",
        "reason_code",
        "photometric",
        "bits",
        "bits_allocated",
        "bits_stored",
        "high_bit",
        "pixel_representation",
        "samples_per_pixel",
        "planar_configuration",
        "pixel_data_vr",
        "pixel_data_layout",
        "basic_offset_table",
        "encapsulated_fragment_layout",
        "extended_offset_table",
        "frames",
        "geometry",
        "pixel_spacing",
        "imager_pixel_spacing",
        "image_orientation_patient",
        "image_position_patient",
        "slice_thickness",
        "spacing_between_slices",
        "slice_location",
        "geometry_instance_number_state",
        "geometry_adjacent_spacing_mm",
        "geometry_spacing_uniform",
        "geometry_gantry_detector_tilt_degrees",
        "series_organization_group_id",
        "study_series_count",
        "series_ordinal",
        "series_organization_instance_count",
        "shared_study_instance_uid_expected",
        "shared_frame_of_reference_uid_expected",
        "distinct_series_instance_uids_expected",
        "derived_refs",
        "derived_reference_targets",
        "derived_reference_relationships",
        "derived_reference_sop_class_uids",
        "derived_reference_sop_instance_uid_roots",
        "validation_status",
        "determinism",
        "object_type",
        "synthetic_data",
        "image_type",
        "laterality",
        "conversion_type",
        "presentation_lut_shape",
        "window_center",
        "window_width",
        "kvp",
        "ct_acquisition_number",
        "ct_rescale_intercept",
        "ct_rescale_slope",
        "ct_rescale_type",
        "enhanced_ct_dimension_index_values",
        "enhanced_ct_in_concatenation_number",
        "enhanced_ct_in_concatenation_total_number",
        "enhanced_ct_concatenation_frame_offset_number",
        "us_image_type",
        "us_frame_increment_pointer",
        "us_frame_time_ms",
        "us_frame_relative_times_ms",
        "us_frame_count",
        "us_ordered_frame_hashes",
        "us_spatially_related_frames",
        "us_color_data_present",
        "us_region_calibrated",
        "us_lossy_image_compression",
        "mr_scanning_sequence",
        "mr_sequence_variant",
        "mr_acquisition_type",
        "mr_repetition_time",
        "mr_echo_time",
        "mr_echo_train_length",
        "mr_magnetic_field_strength",
        "enhanced_mr_effective_echo_times",
        "enhanced_mr_temporal_position_time_offsets",
        "enhanced_mr_temporal_position_indices",
        "enhanced_mr_dimension_index_values",
        "enhanced_mr_frame_acquisition_numbers",
        "enhanced_mr_dimension_index_pointer",
        "enhanced_mr_functional_group_pointer",
        "enhanced_mr_temporal_position_time_offset_unit",
        "enhanced_mr_velocity_encoding_minimum_value",
        "enhanced_mr_velocity_encoding_maximum_value",
        "segmentation_type",
        "segmentation_fractional_type",
        "segmentation_maximum_fractional_value",
        "gsps_content_label",
        "gsps_content_description",
        "gsps_presentation_size_mode",
        "gsps_presentation_pixel_aspect_ratio",
        "gsps_window_center",
        "gsps_window_width",
        "gsps_presentation_lut_shape",
        "rwvm_content_label",
        "rwvm_lut_label",
        "rwvm_first_value_mapped",
        "rwvm_last_value_mapped",
        "rwvm_intercept",
        "rwvm_slope",
        "rwvm_units_code_value",
        "rwvm_units_coding_scheme_designator",
        "rwvm_units_code_meaning",
        "rwvm_referenced_frame_numbers",
        "rt_dose_units",
        "rt_dose_type",
        "rt_dose_summation_type",
        "rt_dose_grid_scaling",
        "rt_structure_set_label",
        "rt_structure_set_roi_name",
        "rt_roi_generation_algorithm",
        "rt_contour_geometric_type",
        "rt_contour_points",
        "rt_roi_interpreted_type",
        "encapsulated_document_burned_in_annotation",
        "encapsulated_document_recognizable_visual_features",
        "encapsulated_document_title",
        "encapsulated_document_mime_type",
        "encapsulated_document_length",
        "modality_lut_descriptor",
        "modality_lut_type",
        "modality_lut_data_value_length",
        "voi_lut_descriptor",
        "voi_lut_data_value_length",
        "overlay_rows",
        "overlay_columns",
        "overlay_type",
        "overlay_origin",
        "overlay_bits_allocated",
        "overlay_bit_position",
        "overlay_data_value_length",
        "display_shutter_shape",
        "display_shutter_presentation_value",
        "body_part_examined",
        "view_position",
        "study_instance_uid_root",
        "series_instance_uid_root",
        "sop_instance_uid_root",
        "lossy_image_compression",
        "lossy_image_compression_ratio",
        "lossy_image_compression_method",
        "wsi_iod_kind",
        "wsi_dimension_organization_type",
        "wsi_tile_geometry",
        "wsi_implicit_frame_order",
        "wsi_total_pixel_matrix_sha256",
        "wsi_specimen_identity",
        "wsi_slide_label_identity",
        "wsi_optical_path_identity",
        "wsi_optical_path_icc_sha256",
        "wsi_pixel_spacing_mm",
        "wsi_image_orientation_slide",
        "wsi_implicit_position_reconstruction",
        "wsi_sparse_dimension_metadata_absent",
        "wsi_explicit_frame_positions",
        "wsi_dimension_index_values",
        "wsi_occupancy_mask",
        "wsi_absent_tile_positions",
        "wsi_pixel_payload_sha256",
        "wsi_sentinel_matrix_sha256",
        "wsi_explicit_position_reconstruction",
        "wsi_reference_free",
        "wsi_multiple_optical_paths_expectation_present",
        "wsi_multiple_optical_paths_count",
        "wsi_multiple_optical_paths_ordered_identifiers",
        "wsi_multiple_optical_paths_total_frame_count",
        "wsi_multiple_optical_paths_frame_ranges",
        "wsi_multiple_optical_paths_aggregate_payload_sha256",
        "wsi_multiple_optical_paths_per_path_payload_sha256_values",
        "wsi_multiple_optical_paths_per_path_matrix_sha256_values",
        "wsi_multiple_optical_paths_per_path_matrix_shapes",
        "wsi_multiple_optical_paths_per_path_icc_sha256_values",
        "wsi_pyramid_role",
        "wsi_pyramid_ordinal",
        "wsi_pyramid_member_count",
        "wsi_pyramid_ordered_roles",
        "wsi_pyramid_apex_role",
        "wsi_pyramid_pyramid_member",
        "wsi_pyramid_group_closure",
        "wsi_pyramid_member_binding_verified",
        "wsi_pyramid_shared_identity_closure",
        "wsi_pyramid_total_frame_count",
        "wsi_pyramid_total_dicom_bytes",
        "wsi_pyramid_member_matrix_sha256",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "coverage report schema must require {field}"
        );
    }

    assert_eq!(
        schema.pointer("/$defs/coverage_row/properties/geometry_instance_number_state/enum"),
        Some(&serde_json::json!(["numeric", "empty", null]))
    );
    assert_eq!(
        schema
            .pointer("/$defs/coverage_row/properties/geometry_adjacent_spacing_mm/items/type")
            .and_then(Value::as_str),
        Some("number")
    );
    for field in [
        "geometry_spacing_uniform",
        "shared_study_instance_uid_expected",
        "shared_frame_of_reference_uid_expected",
        "distinct_series_instance_uids_expected",
        "us_spatially_related_frames",
        "us_color_data_present",
        "us_region_calibrated",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/coverage_row/properties/{field}/type/0"))
                .and_then(Value::as_str),
            Some("boolean"),
            "coverage row {field} must be nullable boolean"
        );
    }
    assert_eq!(
        schema.pointer("/$defs/coverage_row/properties/us_lossy_image_compression/enum"),
        Some(&serde_json::json!(["00", "01", null])),
        "US lossy history must use the DICOM nullable vocabulary"
    );
    assert_eq!(
        schema.pointer("/$defs/coverage_row/properties/wsi_dimension_organization_type/enum"),
        Some(&serde_json::json!(["TILED_FULL", "TILED_SPARSE", null])),
        "WSI dimension organization must use the locked nullable vocabulary"
    );
    for field in [
        "wsi_implicit_position_reconstruction",
        "wsi_sparse_dimension_metadata_absent",
        "wsi_explicit_position_reconstruction",
        "wsi_reference_free",
        "wsi_multiple_optical_paths_expectation_present",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/coverage_row/properties/{field}/type/0"))
                .and_then(Value::as_str),
            Some("boolean"),
            "coverage row {field} must be nullable boolean"
        );
    }
    let multiple_path_rule = schema
        .pointer("/$defs/coverage_row/allOf")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                == Some(&serde_json::json!("vl/wsi/multiple_optical_paths"))
        })
        .expect("multiple optical paths report rule");
    assert_eq!(
        multiple_path_rule
            .pointer("/then/properties/wsi_multiple_optical_paths_ordered_identifiers/const"),
        Some(&serde_json::json!("BRIGHTFIELD; ALTERNATE"))
    );
    assert_eq!(
        multiple_path_rule
            .pointer("/else/properties/wsi_multiple_optical_paths_expectation_present/type"),
        Some(&serde_json::json!("null"))
    );
    for (field, values) in [
        (
            "enhanced_mr_dimension_index_pointer",
            serde_json::json!(["TemporalPositionTimeOffset", null]),
        ),
        (
            "enhanced_mr_functional_group_pointer",
            serde_json::json!(["TemporalPositionSequence", null]),
        ),
        (
            "enhanced_mr_temporal_position_time_offset_unit",
            serde_json::json!(["seconds", null]),
        ),
    ] {
        assert_eq!(
            schema.pointer(&format!("/$defs/coverage_row/properties/{field}/enum")),
            Some(&values),
            "coverage row {field} must use its strict nullable vocabulary"
        );
    }
    for field in [
        "study_series_count",
        "series_ordinal",
        "series_organization_instance_count",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/coverage_row/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(1),
            "coverage row {field} must be positive"
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

    for grouped_field in [
        "profiles",
        "profile_memberships",
        "statuses",
        "iods",
        "sop_classes",
        "sop_class_names",
        "modalities",
        "transfer_syntaxes",
        "codec_families",
        "codec_backends",
        "codec_backend_kinds",
        "codec_feature_gates",
        "determinism",
        "validation_statuses",
        "unavailable_reasons",
        "photometric_interpretations",
        "bit_depths",
        "bits_allocated",
        "bits_stored",
        "high_bits",
        "pixel_representations",
        "samples_per_pixel",
        "planar_configurations",
        "pixel_data_vrs",
        "pixel_data_layouts",
        "basic_offset_tables",
        "encapsulated_fragment_layouts",
        "extended_offset_tables",
        "known_stressors",
        "frame_counts",
        "geometries",
        "pixel_spacings",
        "imager_pixel_spacings",
        "image_orientations_patient",
        "image_positions_patient",
        "slice_thicknesses",
        "spacing_between_slices",
        "slice_locations",
        "object_types",
        "derived_reference_states",
        "derived_reference_relationships",
        "derived_reference_targets",
        "derived_reference_sop_class_uids",
        "derived_reference_sop_instance_uid_roots",
        "synthetic_data",
        "image_types",
        "lateralities",
        "conversion_types",
        "presentation_lut_shapes",
        "window_centers",
        "window_widths",
        "kvps",
        "ct_acquisition_numbers",
        "ct_rescale_intercepts",
        "ct_rescale_slopes",
        "ct_rescale_types",
        "enhanced_ct_dimension_index_values",
        "enhanced_ct_in_concatenation_numbers",
        "enhanced_ct_in_concatenation_total_numbers",
        "enhanced_ct_concatenation_frame_offset_numbers",
        "us_image_types",
        "us_frame_increment_pointers",
        "us_frame_times_ms",
        "us_frame_counts",
        "us_spatially_related_frames",
        "us_color_data_present",
        "us_region_calibrated",
        "us_lossy_image_compressions",
        "mr_scanning_sequences",
        "mr_sequence_variants",
        "mr_acquisition_types",
        "mr_repetition_times",
        "mr_echo_times",
        "mr_echo_train_lengths",
        "mr_magnetic_field_strengths",
        "enhanced_mr_effective_echo_times",
        "enhanced_mr_temporal_position_time_offsets",
        "enhanced_mr_temporal_position_indices",
        "enhanced_mr_dimension_index_values",
        "enhanced_mr_frame_acquisition_numbers",
        "enhanced_mr_dimension_index_pointers",
        "enhanced_mr_functional_group_pointers",
        "enhanced_mr_temporal_position_time_offset_units",
        "enhanced_mr_velocity_encoding_minimum_values",
        "enhanced_mr_velocity_encoding_maximum_values",
        "segmentation_types",
        "segmentation_fractional_types",
        "segmentation_maximum_fractional_values",
        "rwvm_content_labels",
        "rwvm_lut_labels",
        "rwvm_first_values_mapped",
        "rwvm_last_values_mapped",
        "rwvm_intercepts",
        "rwvm_slopes",
        "rwvm_units_code_values",
        "rwvm_units_coding_scheme_designators",
        "rwvm_units_code_meanings",
        "rwvm_referenced_frame_numbers",
        "rt_dose_units",
        "rt_dose_types",
        "rt_dose_summation_types",
        "rt_dose_grid_scalings",
        "rt_structure_set_labels",
        "rt_structure_set_roi_names",
        "rt_roi_generation_algorithms",
        "rt_contour_geometric_types",
        "rt_contour_points",
        "rt_roi_interpreted_types",
        "encapsulated_document_burned_in_annotations",
        "encapsulated_document_recognizable_visual_features",
        "encapsulated_document_titles",
        "encapsulated_document_mime_types",
        "encapsulated_document_lengths",
        "sr_completion_flags",
        "sr_verification_flags",
        "sr_root_value_types",
        "sr_root_continuity_of_content",
        "sr_content_sequence_item_counts",
        "sr_observation_texts",
        "sr_measurement_numeric_values",
        "modality_lut_descriptors",
        "modality_lut_types",
        "modality_lut_data_value_lengths",
        "voi_lut_descriptors",
        "voi_lut_data_value_lengths",
        "overlay_geometries",
        "overlay_types",
        "overlay_origins",
        "overlay_bits_allocated",
        "overlay_bit_positions",
        "overlay_data_value_lengths",
        "display_shutter_shapes",
        "display_shutter_presentation_values",
        "body_parts_examined",
        "view_positions",
        "study_instance_uid_roots",
        "series_instance_uid_roots",
        "sop_instance_uid_roots",
        "lossy_image_compression",
        "lossy_image_compression_ratios",
        "lossy_image_compression_methods",
        "wsi_pyramid_roles",
        "wsi_pyramid_ordinals",
        "wsi_pyramid_membership_states",
        "wsi_pyramid_group_closure_states",
        "wsi_pyramid_member_binding_states",
        "wsi_pyramid_shared_identity_closure_states",
    ] {
        assert!(
            schema
                .pointer(&format!(
                    "/properties/grouped_coverage/properties/{grouped_field}"
                ))
                .is_some(),
            "coverage report schema must define grouped {grouped_field} coverage"
        );
    }

    for field in [
        "wsi_pyramid_pyramid_member",
        "wsi_pyramid_group_closure",
        "wsi_pyramid_member_binding_verified",
        "wsi_pyramid_shared_identity_closure",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/coverage_row/properties/{field}/type/0"))
                .and_then(Value::as_str),
            Some("boolean"),
            "WSI pyramid report field {field} must be nullable boolean"
        );
    }
}

#[test]
fn coverage_gap_report_schema_separates_logical_cases_and_dimensions() {
    let schema = read_json("schemas/coverage-gap-report.schema.json");
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("coverage gap report schema must require top-level fields");
    for field in [
        "coverage_gap_report_schema_version",
        "registry_sha256",
        "standards_lock_sha256",
        "counts",
        "dimensions",
        "gaps",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "coverage gap report schema must require {field}"
        );
    }

    let dimensions = schema
        .pointer("/properties/dimensions/required")
        .and_then(Value::as_array)
        .expect("coverage gap report schema must require dimensions");
    for dimension in [
        "sop_classes",
        "modalities",
        "object_families",
        "compatibility_axes",
    ] {
        assert!(
            dimensions
                .iter()
                .any(|value| value.as_str() == Some(dimension)),
            "coverage gap report schema must require {dimension}"
        );
    }
}

#[test]
fn generation_backend_schemas_lock_protocol_version_and_security_fields() {
    for path in [
        "schemas/generation-backend-request.schema.json",
        "schemas/generation-backend-response.schema.json",
        "schemas/generation-backend-lock.schema.json",
    ] {
        let schema = read_json(path);
        jsonschema::validator_for(&schema)
            .unwrap_or_else(|error| panic!("{path} must compile: {error}"));
    }

    let request = read_json("schemas/generation-backend-request.schema.json");
    assert_eq!(
        request
            .pointer("/properties/protocol_version/const")
            .and_then(Value::as_str),
        Some("0.1.0")
    );
    for field in ["staging", "identities", "controlled_metadata", "sources"] {
        assert!(
            request
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|item| item.as_str() == Some(field))),
            "backend request must require {field}"
        );
    }

    let response = read_json("schemas/generation-backend-response.schema.json");
    for field in [
        "dependency_lock_sha256",
        "executable_fingerprint",
        "environment_fingerprint",
    ] {
        assert!(
            response
                .pointer(&format!("/$defs/backend/properties/{field}"))
                .is_some(),
            "backend response provenance must include {field}"
        );
    }

    let lock = read_json("schemas/generation-backend-lock.schema.json");
    for field in [
        "resource_limits",
        "independent_validation",
        "license",
        "blockers",
    ] {
        assert!(
            lock.pointer(&format!("/$defs/backend/properties/{field}"))
                .is_some(),
            "backend lock must model {field}"
        );
    }
}

#[test]
fn backend_discovery_schema_requires_portable_runtime_identity_inputs() {
    let schema = read_json("schemas/generation-backend-lock.schema.json");
    let required = schema
        .pointer("/$defs/discovery/required")
        .and_then(Value::as_array)
        .expect("backend discovery must define required fields");
    for field in [
        "default_relative_executables",
        "fixed_arguments",
        "version_arguments",
        "runtime_identity_arguments",
        "entrypoint_paths",
        "environment_override",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "backend discovery must require {field}"
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

#[test]
fn manifest_schema_types_twelve_lead_ecg_waveform_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_waveform",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("Twelve-lead ECG expectation schema should compile");
    let expectation = twelve_lead_ecg_waveform_expectation();
    assert!(validator.is_valid(&expectation));

    let mut reordered = expectation.clone();
    reordered["multiplex_groups"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .swap(0, 1);
    assert!(!validator.is_valid(&reordered));

    let mut duplicate_lead = expectation.clone();
    duplicate_lead["multiplex_groups"][0]["channels"][11]["source"] =
        duplicate_lead["multiplex_groups"][0]["channels"][0]["source"].clone();
    assert!(!validator.is_valid(&duplicate_lead));

    let mut wrong_rate = expectation.clone();
    wrong_rate["multiplex_groups"][0]["sampling_frequency_hz"] = serde_json::json!(199);
    assert!(!validator.is_valid(&wrong_rate));

    let mut wrong_interleave = expectation.clone();
    wrong_interleave["multiplex_groups"][0]["storage"]["interleave_order"] =
        serde_json::json!("sample_then_channel");
    assert!(!validator.is_valid(&wrong_interleave));

    let mut corrupt_payload = expectation.clone();
    corrupt_payload["multiplex_groups"][0]["storage"]["payload_sha256"] =
        serde_json::json!("0".repeat(64));
    assert!(!validator.is_valid(&corrupt_payload));

    let mut padded = expectation.clone();
    padded["multiplex_groups"][0]["storage"]["value_field_padding_bytes"] = serde_json::json!(2);
    assert!(!validator.is_valid(&padded));

    let mut wrong_group_ordinal = expectation.clone();
    wrong_group_ordinal["multiplex_groups"][0]["ordinal"] = serde_json::json!(2);
    assert!(!validator.is_valid(&wrong_group_ordinal));

    let mut extra_group = expectation.clone();
    let duplicate_group = extra_group["multiplex_groups"][0].clone();
    extra_group["multiplex_groups"]
        .as_array_mut()
        .expect("multiplex groups")
        .push(duplicate_group);
    assert!(!validator.is_valid(&extra_group));

    let mut missing_channel = expectation.clone();
    missing_channel["multiplex_groups"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .pop();
    assert!(!validator.is_valid(&missing_channel));

    let mut missing_channel_hash = expectation.clone();
    missing_channel_hash["multiplex_groups"][0]["storage"]["channel_sha256"]
        .as_array_mut()
        .expect("channel hashes")
        .pop();
    assert!(!validator.is_valid(&missing_channel_hash));

    for field in [
        "group_count",
        "total_channel_count",
        "common_duration_seconds",
        "total_payload_length_bytes",
    ] {
        let mut wrong_aggregate = expectation.clone();
        wrong_aggregate["aggregate"][field] = serde_json::json!(99);
        assert!(
            !validator.is_valid(&wrong_aggregate),
            "aggregate {field} must remain locked"
        );
    }

    let mut wrong_group_hashes = expectation.clone();
    wrong_group_hashes["aggregate"]["group_payload_sha256"] = serde_json::json!(["0".repeat(64)]);
    assert!(!validator.is_valid(&wrong_group_hashes));

    let mut wrong_aggregate_hash = expectation;
    wrong_aggregate_hash["aggregate"]["aggregate_payload_sha256"] =
        serde_json::json!("0".repeat(64));
    assert!(!validator.is_valid(&wrong_aggregate_hash));
}

#[test]
fn manifest_schema_locks_phase4_single_frame_vl_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_vl_endoscopic_single_frame",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("single-frame VL expectation schema should compile");
    let expectation = serde_json::json!({
        "iod_kind": "vl_endoscopic_single_frame",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.1",
        "sop_class_name": "VL Endoscopic Image Storage",
        "iod_name": "VL Endoscopic Image",
        "modality": "ES",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "body_part_examined": "LUNG",
        "laterality": "R",
        "image_type": ["ORIGINAL", "PRIMARY"],
        "acquisition_context_items": 0,
        "image": { "rows": 2, "columns": 2, "samples_per_pixel": 3, "photometric_interpretation": "RGB", "planar_configuration": 0, "bits_allocated": 8, "bits_stored": 8, "high_bit": 7, "pixel_representation": 0 },
        "absent_content": ["number_of_frames", "frame_of_reference_uid", "specimen_module", "optical_path_module", "icc_profile_module"]
    });
    assert!(validator.is_valid(&expectation));
    let mut microscopic = expectation.clone();
    for (field, value) in [
        ("iod_kind", serde_json::json!("vl_microscopic_single_frame")),
        (
            "sop_class_uid",
            serde_json::json!("1.2.840.10008.5.1.4.1.1.77.1.2"),
        ),
        (
            "sop_class_name",
            serde_json::json!("VL Microscopic Image Storage"),
        ),
        ("iod_name", serde_json::json!("VL Microscopic Image")),
        ("modality", serde_json::json!("GM")),
        ("body_part_examined", serde_json::json!("EYE")),
    ] {
        microscopic[field] = value;
    }
    let microscopic_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_vl_microscopic_single_frame",
        "$defs": schema["$defs"].clone(),
    });
    assert!(
        jsonschema::validator_for(&microscopic_schema)
            .expect("microscopic VL schema")
            .is_valid(&microscopic)
    );
    for (pointer, bad) in [
        (
            "/sop_class_name",
            serde_json::json!("VL Microscopic Image Storage"),
        ),
        ("/laterality", serde_json::json!("")),
        ("/image_type", serde_json::json!(["DERIVED", "PRIMARY"])),
        ("/acquisition_context_items", serde_json::json!(1)),
        ("/image/rows", serde_json::json!(3)),
        ("/image/planar_configuration", serde_json::json!(1)),
        ("/image/bits_stored", serde_json::json!(7)),
        (
            "/absent_content/3",
            serde_json::json!("specimen_description"),
        ),
    ] {
        let mut malformed = expectation.clone();
        *malformed.pointer_mut(pointer).expect("mutation pointer") = bad;
        assert!(
            !validator.is_valid(&malformed),
            "schema must reject {pointer}"
        );
    }
    let mut missing = expectation;
    missing
        .as_object_mut()
        .expect("expectation object")
        .remove("sop_class_name");
    assert!(!validator.is_valid(&missing));

    let rules = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file rules");
    let common = rules
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/enum")
                .and_then(Value::as_array)
                .is_some_and(|ids| ids.iter().any(|id| id == "vl/endoscopic/rgb_explicit_le"))
        })
        .expect("VL common exact-case rule");
    assert_eq!(
        common.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "expected_vl_single_frame"
        ]))
    );
    assert_eq!(
        common.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_vl_single_frame"]))
    );
    assert_eq!(
        common.pointer("/then/properties/pixel_data/properties/vr/const"),
        Some(&serde_json::json!("OB"))
    );
    assert_eq!(
        common.pointer("/then/properties/image/properties/frames/const"),
        Some(&serde_json::json!(1))
    );
}

#[test]
fn manifest_schema_locks_phase4_tiled_full_wsi_expectation() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_wsi_tiled_full",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("TILED_FULL WSI expectation schema should compile");
    let expectation = serde_json::json!({
        "iod_kind": "vl_wsi_tiled_full",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.6",
        "sop_class_name": "VL Whole Slide Microscopy Image Storage",
        "iod_name": "VL Whole Slide Microscopy Image",
        "modality": "SM",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "frame_of_reference_uid": "1.2.826.0.1.3680043.10.543.1",
        "image_type": ["ORIGINAL", "PRIMARY", "VOLUME", "NONE"],
        "dimension_organization_type": "TILED_FULL",
        "position_reference_indicator": "SLIDE_CORNER",
        "acquisition_context_items": 0,
        "volumetric_properties": "VOLUME",
        "specimen_label_in_image": "NO",
        "burned_in_annotation": "NO",
        "focus_method": "AUTO",
        "extended_depth_of_field": "NO",
        "lossy_image_compression": "00",
        "image": { "rows": 2, "columns": 2, "frames": 4, "samples_per_pixel": 3, "photometric_interpretation": "RGB", "planar_configuration": 0, "bits_allocated": 8, "bits_stored": 8, "high_bit": 7, "pixel_representation": 0 },
        "pixel_data": {
            "vr": "OB", "native_or_encapsulated": "native", "value_length": 48, "frame_count": 4,
            "frame_hashes": [
                "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
                "6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65",
                "7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f",
                "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21"
            ]
        },
        "tiling": {
            "total_pixel_matrix_rows": 4, "total_pixel_matrix_columns": 4,
            "tiles_per_row": 2, "tiles_per_column": 2,
            "number_of_optical_paths": 1, "total_pixel_matrix_focal_planes": 1,
            "total_pixel_matrix_origin_mm": [0.0, 0.0, 0.0],
            "image_orientation_slide": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "pixel_spacing_mm": [0.5, 0.5], "slice_thickness_mm": 0.001,
            "imaged_volume": { "width_mm": 2.0, "height_mm": 2.0, "depth_micrometers": 1.0 },
            "implicit_frame_positions": [
                { "frame_number": 1, "optical_path_identifier": "RGB", "focal_plane": 1, "column_position": 1, "row_position": 1, "x_mm": 0.0, "y_mm": 0.0, "z_mm": 0.0 },
                { "frame_number": 2, "optical_path_identifier": "RGB", "focal_plane": 1, "column_position": 3, "row_position": 1, "x_mm": 1.0, "y_mm": 0.0, "z_mm": 0.0 },
                { "frame_number": 3, "optical_path_identifier": "RGB", "focal_plane": 1, "column_position": 1, "row_position": 3, "x_mm": 0.0, "y_mm": 1.0, "z_mm": 0.0 },
                { "frame_number": 4, "optical_path_identifier": "RGB", "focal_plane": 1, "column_position": 3, "row_position": 3, "x_mm": 1.0, "y_mm": 1.0, "z_mm": 0.0 }
            ],
            "total_pixel_matrix_sha256": "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a"
        },
        "specimen": {
            "container_identifier": "DTS-SLIDE-001", "container_issuer_items": 0,
            "container_type_code_items": 0, "description_items": 1,
            "specimen_identifier": "DTS-SPECIMEN-001", "specimen_uid": "1.2.826.0.1.3680043.10.543.2",
            "specimen_issuer_items": 0, "specimen_preparation_items": 0
        },
        "slide_label": { "barcode_value": "DTS-SLIDE-001", "label_text": "DTS SYNTHETIC SLIDE 001" },
        "optical_path": {
            "items": 1, "identifier": "RGB", "illumination_wavelength_nm": 550,
            "illumination_type": { "code_value": "111744", "coding_scheme_designator": "DCM", "code_meaning": "Brightfield illumination" },
            "icc_profile": { "size_bytes": 736, "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef", "dicom_color_space": "SRGB", "device_class": "scnr", "data_color_space": "RGB ", "profile_connection_space": "XYZ ", "signature": "acsp" }
        },
        "presence": { "shared_functional_groups_sequence": true, "per_frame_functional_groups_sequence": false, "dimension_index_sequence": false, "references": false, "concatenation": false, "multi_resolution_pyramid": false },
        "absent_content": ["per_frame_functional_groups_sequence", "dimension_index_sequence", "referenced_series_sequence", "concatenation_attributes", "multi_resolution_pyramid", "extended_depth_of_field_number_of_focal_planes", "extended_depth_of_field_distance_between_focal_planes", "lossy_image_compression_ratio", "lossy_image_compression_method", "specimen_reference_sequence"]
    });
    assert!(validator.is_valid(&expectation));
    for (pointer, malformed_value) in [
        (
            "/dimension_organization_type",
            serde_json::json!("TILED_SPARSE"),
        ),
        (
            "/pixel_data/frame_hashes/1",
            serde_json::json!("0".repeat(64)),
        ),
        (
            "/tiling/implicit_frame_positions/1/column_position",
            serde_json::json!(1),
        ),
        ("/tiling/slice_thickness_mm", serde_json::json!(0.1)),
        ("/specimen/description_items", serde_json::json!(0)),
        ("/optical_path/identifier", serde_json::json!("1")),
        (
            "/presence/per_frame_functional_groups_sequence",
            serde_json::json!(true),
        ),
        (
            "/absent_content/6",
            serde_json::json!("number_of_focal_planes"),
        ),
    ] {
        let mut malformed = expectation.clone();
        *malformed.pointer_mut(pointer).expect("mutation pointer") = malformed_value;
        assert!(
            !validator.is_valid(&malformed),
            "schema must reject {pointer}"
        );
    }
    let mut missing = expectation;
    missing.as_object_mut().unwrap().remove("optical_path");
    assert!(!validator.is_valid(&missing));

    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("vl/wsi/tiled_full_small")
        })
        .expect("exact WSI case rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "expected_wsi_tiled_full"
        ]))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_wsi_tiled_full"]))
    );
}

#[test]
fn manifest_schema_locks_phase4_wsi_tile_segmentation_expectation() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_wsi_tile_segmentation",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("WSI tile SEG expectation schema should compile");
    let expectation = serde_json::json!({
        "iod_kind": "wsi_tile_segmentation",
        "profile": "extended",
        "dimension_organization_uid": "1.2.826.0.1.3680043.10.543.6",
        "source": {
            "case_id": "vl/wsi/tiled_full_small",
            "path": "vl/wsi/tiled_full_small/instance.dcm",
            "sha256": "0".repeat(64),
            "study_instance_uid": "1.2.826.0.1.3680043.10.543.1",
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.2",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.6",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.3",
            "frame_numbers": [1, 4],
            "frame_hashes": [
                "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
                "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21"
            ],
            "frame_of_reference_uid": "1.2.826.0.1.3680043.10.543.4",
            "specimen_uid": "1.2.826.0.1.3680043.10.543.5",
            "container_identifier": "DTS-SLIDE-001"
        },
        "segmentation": {
            "type": "FRACTIONAL", "fractional_type": "OCCUPANCY",
            "maximum_fractional_value": 255, "segments_overlap": "NO",
            "segment_number": 1, "segment_label": "DTS_SYNTHETIC_REGION",
            "algorithm_type": "MANUAL",
            "category": { "code_value": "85756007", "coding_scheme_designator": "SCT", "code_meaning": "Tissue" },
            "property_type": { "code_value": "113343", "coding_scheme_designator": "DCM", "code_meaning": "Organ" }
        },
        "image": { "rows": 2, "columns": 2, "frames": 2, "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8, "bits_stored": 8, "high_bit": 7, "pixel_representation": 0 },
        "pixel_data": {
            "vr": "OB", "native_or_encapsulated": "native", "value_length": 8, "frame_count": 2,
            "frame_values": [[255, 0, 0, 255], [0, 255, 255, 0]],
            "frame_hashes": [
                "34aaa746c25a0f105c4316bbb1f009aa359f49582656ee97d73c58132d563423",
                "10db5223d19bd1d58c2b8eb3c723b0ba104cf17564f9434e53e1b9e642fb3b37"
            ],
            "payload_sha256": "74fa7cbb10160e0eb1f16f35fa9ad0e7f2712af56019996e88cf1034be92635e"
        },
        "tiling": {
            "dimension_organization_type": "TILED_SPARSE",
            "total_pixel_matrix_rows": 4, "total_pixel_matrix_columns": 4,
            "total_pixel_matrix_focal_planes": 1, "tile_rows": 2, "tile_columns": 2,
            "total_pixel_matrix_origin_mm": [0.0, 0.0, 0.0],
            "image_orientation_slide": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "pixel_spacing_mm": [0.5, 0.5], "reconstructed_shape": [4, 4],
            "reconstructed_total_pixel_matrix_sha256": "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467"
        },
        "dimension_indices": [
            { "ordinal": 1, "pointer": "ReferencedSegmentNumber", "functional_group_pointer": "SegmentIdentificationSequence" },
            { "ordinal": 2, "pointer": "RowPositionInTotalImagePixelMatrix", "functional_group_pointer": "PlanePositionSlideSequence" },
            { "ordinal": 3, "pointer": "ColumnPositionInTotalImagePixelMatrix", "functional_group_pointer": "PlanePositionSlideSequence" }
        ],
        "frames": [
            { "frame_number": 1, "source_frame_number": 1, "dimension_index_values": [1, 1, 1], "row_position": 1, "column_position": 1, "x_mm": 0.0, "y_mm": 0.0, "z_mm": 0.0 },
            { "frame_number": 2, "source_frame_number": 4, "dimension_index_values": [1, 2, 2], "row_position": 3, "column_position": 3, "x_mm": 1.0, "y_mm": 1.0, "z_mm": 0.0 }
        ],
        "references": {
            "common_instance_reference": true, "per_frame_derivation": true,
            "purpose": { "code_value": "121322", "coding_scheme_designator": "DCM", "code_meaning": "Source Image for Image Processing Operation" },
            "derivation": { "code_value": "113076", "coding_scheme_designator": "DCM", "code_meaning": "Segmentation" },
            "spatial_locations_preserved": "YES"
        },
        "presence": { "shared_functional_groups_sequence": true, "per_frame_functional_groups_sequence": true, "dimension_index_sequence": true, "referenced_series_sequence": true },
        "absent_content": ["tiled_full", "source_frames_2_and_3", "patient_coordinate_functional_groups", "palette_color_lut", "icc_profile", "pixel_padding", "lossy_image_compression_ratio", "lossy_image_compression_method", "tracking_identifiers", "algorithm_identification", "concatenation", "multi_resolution_pyramid"],
        "budget": { "instance_count": 1, "total_frame_count": 2, "max_total_dicom_bytes": 16384, "max_generation_wall_time_seconds": 5 }
    });
    assert!(validator.is_valid(&expectation));

    for (pointer, malformed_value) in [
        ("/source/frame_numbers/1", serde_json::json!(3)),
        ("/segmentation/type", serde_json::json!("BINARY")),
        ("/pixel_data/frame_values/0/0", serde_json::json!(0)),
        (
            "/tiling/dimension_organization_type",
            serde_json::json!("TILED_FULL"),
        ),
        (
            "/dimension_indices/1/pointer",
            serde_json::json!("ColumnPositionInTotalImagePixelMatrix"),
        ),
        ("/frames/1/source_frame_number", serde_json::json!(3)),
        (
            "/references/spatial_locations_preserved",
            serde_json::json!("NO"),
        ),
        ("/budget/max_total_dicom_bytes", serde_json::json!(32768)),
    ] {
        let mut malformed = expectation.clone();
        *malformed.pointer_mut(pointer).expect("mutation pointer") = malformed_value;
        assert!(
            !validator.is_valid(&malformed),
            "schema must reject {pointer}"
        );
    }

    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("derived/seg/wsi_tile_reference")
        })
        .expect("exact WSI tile SEG case rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "generation_backend",
            "expected_wsi_tile_segmentation"
        ]))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_wsi_tile_segmentation"]))
    );
}

#[test]
fn manifest_schema_locks_phase4_tiled_sparse_wsi_expectation() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_wsi_tiled_sparse",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("TILED_SPARSE WSI expectation schema should compile");
    let dimension_organization_uid = "1.2.826.0.1.3680043.10.543.13";
    let expectation = serde_json::json!({
        "iod_kind": "vl_wsi_tiled_sparse",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.6",
        "sop_class_name": "VL Whole Slide Microscopy Image Storage",
        "iod_name": "VL Whole Slide Microscopy Image", "modality": "SM",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "frame_of_reference_uid": "1.2.826.0.1.3680043.10.543.11",
        "dimension_organization_uid": dimension_organization_uid,
        "image_type": ["ORIGINAL", "PRIMARY", "VOLUME", "NONE"],
        "dimension_organization_type": "TILED_SPARSE",
        "position_reference_indicator": "SLIDE_CORNER", "acquisition_context_items": 0,
        "volumetric_properties": "VOLUME", "specimen_label_in_image": "NO",
        "burned_in_annotation": "NO", "focus_method": "AUTO",
        "extended_depth_of_field": "NO", "lossy_image_compression": "00",
        "tiles_overlap": "NONE",
        "image": {
            "rows": 2, "columns": 2, "frames": 2, "samples_per_pixel": 3,
            "photometric_interpretation": "RGB", "planar_configuration": 0,
            "bits_allocated": 8, "bits_stored": 8, "high_bit": 7, "pixel_representation": 0
        },
        "pixel_data": {
            "vr": "OB", "native_or_encapsulated": "native", "value_length": 24, "frame_count": 2,
            "frame_hashes": [
                "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
                "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21"
            ],
            "payload_sha256": "94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee"
        },
        "tiling": {
            "total_pixel_matrix_rows": 4, "total_pixel_matrix_columns": 4,
            "tiles_per_row": 2, "tiles_per_column": 2,
            "number_of_optical_paths": 1, "total_pixel_matrix_focal_planes": 1,
            "total_pixel_matrix_origin_mm": [0.0, 0.0, 0.0],
            "image_orientation_slide": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "pixel_spacing_mm": [0.5, 0.5], "slice_thickness_mm": 0.001,
            "imaged_volume": { "width_mm": 2.0, "height_mm": 2.0, "depth_micrometers": 1.0 },
            "occupancy_mask": ["present", "absent", "absent", "present"],
            "absent_tile_positions": [
                { "column_position": 3, "row_position": 1 },
                { "column_position": 1, "row_position": 3 }
            ],
            "sentinel_fill_rgb": [0, 0, 0],
            "sentinel_matrix_sha256": "d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3"
        },
        "dimension_indices": [
            { "ordinal": 1, "dimension_index_pointer": "(0048,021E)", "functional_group_pointer": "(0048,021A)", "dimension_organization_uid": dimension_organization_uid, "dimension_description_label": "Column Position" },
            { "ordinal": 2, "dimension_index_pointer": "(0048,021F)", "functional_group_pointer": "(0048,021A)", "dimension_organization_uid": dimension_organization_uid, "dimension_description_label": "Row Position" }
        ],
        "shared_functional_group_macros": ["pixel_measures", "whole_slide_microscopy_image_frame_type"],
        "per_frame_functional_groups": [
            { "frame_number": 1, "macros": ["frame_content", "plane_position_slide", "optical_path_identification"], "dimension_index_values": [1, 1], "optical_path_identifier": "RGB", "column_position": 1, "row_position": 1, "x_mm": 0.0, "y_mm": 0.0, "z_mm": 0.0 },
            { "frame_number": 2, "macros": ["frame_content", "plane_position_slide", "optical_path_identification"], "dimension_index_values": [2, 2], "optical_path_identifier": "RGB", "column_position": 3, "row_position": 3, "x_mm": 1.0, "y_mm": 1.0, "z_mm": 0.0 }
        ],
        "specimen": {
            "container_identifier": "DTS-SLIDE-001", "container_issuer_items": 0,
            "container_type_code_items": 0, "description_items": 1,
            "specimen_identifier": "DTS-SPECIMEN-001",
            "specimen_uid": "1.2.826.0.1.3680043.10.543.12",
            "specimen_issuer_items": 0, "specimen_preparation_items": 0
        },
        "slide_label": { "barcode_value": "DTS-SLIDE-001", "label_text": "DTS SYNTHETIC SLIDE 001" },
        "optical_path": {
            "items": 1, "identifier": "RGB", "illumination_wavelength_nm": 550,
            "illumination_type": { "code_value": "111744", "coding_scheme_designator": "DCM", "code_meaning": "Brightfield illumination" },
            "icc_profile": { "size_bytes": 736, "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef", "dicom_color_space": "SRGB", "device_class": "scnr", "data_color_space": "RGB ", "profile_connection_space": "XYZ ", "signature": "acsp" }
        },
        "presence": { "shared_functional_groups_sequence": true, "per_frame_functional_groups_sequence": true, "dimension_index_sequence": true, "references": false, "concatenation": false, "multi_resolution_pyramid": false },
        "absent_content": ["referenced_series_sequence", "concatenation_attributes", "multi_resolution_pyramid", "extended_depth_of_field_number_of_focal_planes", "extended_depth_of_field_distance_between_focal_planes", "lossy_image_compression_ratio", "lossy_image_compression_method", "top_level_image_pixel_description_icc_profile", "specimen_reference_sequence"]
    });
    assert!(validator.is_valid(&expectation));

    for (pointer, malformed_value) in [
        (
            "/dimension_organization_type",
            serde_json::json!("TILED_FULL"),
        ),
        (
            "/pixel_data/payload_sha256",
            serde_json::json!("0".repeat(64)),
        ),
        ("/tiling/occupancy_mask/1", serde_json::json!("present")),
        (
            "/dimension_indices/0/dimension_index_pointer",
            serde_json::json!("(0048,021F)"),
        ),
        (
            "/per_frame_functional_groups/1/dimension_index_values/0",
            serde_json::json!(1),
        ),
        (
            "/presence/per_frame_functional_groups_sequence",
            serde_json::json!(false),
        ),
    ] {
        let mut malformed = expectation.clone();
        *malformed.pointer_mut(pointer).expect("mutation pointer") = malformed_value;
        assert!(
            !validator.is_valid(&malformed),
            "schema must reject {pointer}"
        );
    }

    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("vl/wsi/tiled_sparse_small")
        })
        .expect("exact sparse WSI case rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "expected_wsi_tiled_sparse"
        ]))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_wsi_tiled_sparse"]))
    );
}

#[test]
fn manifest_schema_locks_phase4_multiresolution_wsi_group() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_wsi_pyramid",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("multi-resolution WSI group schema should compile");
    let uid = |suffix| format!("1.2.826.0.1.3680043.10.543.{suffix}");
    let common = |ordinal,
                  role,
                  image_type: Value,
                  pyramid_uid: Value,
                  frames,
                  matrix_rows,
                  matrix_columns,
                  spacing: Value,
                  width,
                  height,
                  frame_hashes: Value,
                  payload_hash,
                  matrix_hash,
                  label_in_image| {
        serde_json::json!({
            "ordinal": ordinal, "role": role, "path": format!("vl/wsi/pyramid_multiresolution/{role}.dcm"),
            "sha256": format!("{:064x}", ordinal), "size_bytes": 2900 + ordinal,
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.6",
            "sop_instance_uid": uid(20 + ordinal),
            "image_type": image_type, "frame_type": image_type,
            "pyramid_uid": pyramid_uid, "rows": 2, "columns": 2, "frames": frames,
            "total_pixel_matrix_rows": matrix_rows, "total_pixel_matrix_columns": matrix_columns,
            "pixel_spacing_mm": spacing, "imaged_volume_width_mm": width,
            "imaged_volume_height_mm": height, "frame_hashes": frame_hashes,
            "payload_sha256": payload_hash, "matrix_sha256": matrix_hash,
            "specimen_label_in_image": label_in_image
        })
    };
    let pyramid_uid = uid(15);
    let expectation = serde_json::json!({
        "iod_kind": "vl_wsi_pyramid_multiresolution",
        "member_count": 3, "ordered_roles": ["volume", "thumbnail", "label"],
        "apex_role": "thumbnail",
        "shared_identity": {
            "patient_id": "DTS-PATIENT-001", "study_instance_uid": uid(11),
            "series_instance_uid": uid(12), "frame_of_reference_uid": uid(13),
            "container_identifier": "DTS-SLIDE-001", "specimen_identifier": "DTS-SPECIMEN-001",
            "specimen_uid": uid(14), "optical_path_identifier": "RGB",
            "icc_profile_sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
        },
        "pyramid_membership": {
            "pyramid_uid": pyramid_uid, "member_roles": ["volume", "thumbnail"],
            "non_member_roles": ["label"]
        },
        "members": [
            common(1, "volume", serde_json::json!(["ORIGINAL", "PRIMARY", "VOLUME", "NONE"]), serde_json::json!(pyramid_uid), 4, 4, 4, serde_json::json!([0.5, 0.5]), 2.0, 2.0,
                serde_json::json!(["fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8", "6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65", "7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f", "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21"]), "b40b0afc9b180d5ebfb54a7db428e13fe09a33dcc9a8f76220f395ba2c68d2db", "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a", "NO"),
            common(2, "thumbnail", serde_json::json!(["DERIVED", "PRIMARY", "THUMBNAIL", "RESAMPLED"]), serde_json::json!(pyramid_uid), 1, 2, 2, serde_json::json!([1.0, 1.0]), 2.0, 2.0,
                serde_json::json!(["6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc"]), "6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc", "6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc", "NO"),
            common(3, "label", serde_json::json!(["ORIGINAL", "PRIMARY", "LABEL", "NONE"]), Value::Null, 1, 2, 2, serde_json::json!([0.5, 0.5]), 1.0, 1.0,
                serde_json::json!(["ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02"]), "ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02", "ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02", "YES")
        ],
        "budget": { "instance_count": 3, "total_frame_count": 6, "max_total_dicom_bytes": 65536, "max_generation_wall_time_seconds": 5 }
    });
    assert!(validator.is_valid(&expectation));
    for (pointer, bad) in [
        ("/ordered_roles/1", serde_json::json!("label")),
        (
            "/pyramid_membership/member_roles/1",
            serde_json::json!("label"),
        ),
        ("/members/0/image_type/2", serde_json::json!("THUMBNAIL")),
        ("/members/1/pixel_spacing_mm/0", serde_json::json!(0.5)),
        ("/members/2/pyramid_uid", serde_json::json!(uid(15))),
        (
            "/members/2/payload_sha256",
            serde_json::json!("0".repeat(64)),
        ),
        ("/budget/max_total_dicom_bytes", serde_json::json!(65535)),
    ] {
        let mut malformed = expectation.clone();
        *malformed.pointer_mut(pointer).expect("mutation pointer") = bad;
        assert!(
            !validator.is_valid(&malformed),
            "schema must reject {pointer}"
        );
    }

    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("vl/wsi/pyramid_multiresolution")
        })
        .expect("exact pyramid WSI case rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "wsi_pyramid_role",
            "wsi_pyramid_ordinal",
            "expected_wsi_pyramid"
        ]))
    );
    assert_eq!(
        rule.pointer("/else/not/anyOf/2/required"),
        Some(&serde_json::json!(["expected_wsi_pyramid"]))
    );
    assert_eq!(
        rule.pointer("/then/allOf/0/then/properties/wsi_pyramid_ordinal/const"),
        Some(&serde_json::json!(1))
    );
    assert_eq!(
        rule.pointer("/then/allOf/1/then/properties/pixel_data/properties/frame_hashes/const/0"),
        Some(&serde_json::json!(
            "6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc"
        ))
    );
    assert_eq!(
        rule.pointer("/then/allOf/2/then/properties/wsi_pyramid_ordinal/const"),
        Some(&serde_json::json!(3))
    );
}

#[test]
fn manifest_schema_locks_phase4_multiple_optical_paths_expectation() {
    let schema = read_json("schemas/manifest.schema.json");
    let definition = &schema["$defs"]["expected_wsi_multiple_optical_paths"];
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_wsi_multiple_optical_paths",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("multiple optical paths WSI expectation schema should compile");
    let mut expectation = serde_json::Map::new();
    for (name, property) in definition["properties"].as_object().unwrap() {
        if let Some(value) = property.get("const") {
            expectation.insert(name.clone(), value.clone());
        }
    }
    expectation.insert(
        "frame_of_reference_uid".into(),
        serde_json::json!("1.2.826.0.1.3680043.10.543.21"),
    );
    expectation.insert(
        "dimension_organization_uid".into(),
        serde_json::json!("1.2.826.0.1.3680043.10.543.23"),
    );
    expectation.insert(
        "specimen".into(),
        serde_json::json!({
            "container_identifier": "DTS-SLIDE-001", "container_issuer_items": 0,
            "container_type_code_items": 0, "description_items": 1,
            "specimen_identifier": "DTS-SPECIMEN-001",
            "specimen_uid": "1.2.826.0.1.3680043.10.543.22",
            "specimen_issuer_items": 0, "specimen_preparation_items": 0
        }),
    );
    let expectation = Value::Object(expectation);
    assert!(validator.is_valid(&expectation));
    assert_eq!(expectation["profile"], serde_json::json!("extended"));
    assert_eq!(
        expectation["dimension_organization_type"],
        serde_json::json!("TILED_FULL")
    );
    assert_eq!(expectation["image"]["frames"], serde_json::json!(8));
    assert_eq!(
        expectation["tiling"]["number_of_optical_paths"],
        serde_json::json!(2)
    );
    assert_eq!(
        expectation["tiling"]["total_pixel_matrix_focal_planes"],
        serde_json::json!(1)
    );
    assert_eq!(
        expectation["optical_paths"][0]["identifier"],
        serde_json::json!("BRIGHTFIELD")
    );
    assert_eq!(
        expectation["optical_paths"][1]["identifier"],
        serde_json::json!("ALTERNATE")
    );
    assert_eq!(
        expectation["optical_paths"][0]["frame_ordinal_range"],
        serde_json::json!([1, 4])
    );
    assert_eq!(
        expectation["optical_paths"][1]["frame_ordinal_range"],
        serde_json::json!([5, 8])
    );
    assert_eq!(
        expectation["optical_paths"][0]["matrix_shape"],
        serde_json::json!([4, 4, 3])
    );

    for (pointer, bad) in [
        ("/profile", serde_json::json!("stress")),
        ("/image/frames", serde_json::json!(7)),
        (
            "/pixel_data/payload_sha256",
            serde_json::json!("0".repeat(64)),
        ),
        (
            "/optical_paths/0/identifier",
            serde_json::json!("ALTERNATE"),
        ),
        (
            "/optical_paths/1/illumination_wavelength_nm",
            serde_json::json!(550.0),
        ),
        (
            "/optical_paths/1/frame_hashes/0",
            serde_json::json!("0".repeat(64)),
        ),
        (
            "/optical_paths/0/matrix_sha256",
            serde_json::json!("0".repeat(64)),
        ),
        ("/optical_paths/1/matrix_shape/2", serde_json::json!(1)),
        (
            "/tiling/implicit_frame_positions/4/optical_path_ordinal",
            serde_json::json!(1),
        ),
        (
            "/tiling/total_pixel_matrix_focal_planes",
            serde_json::json!(2),
        ),
        ("/budget/max_total_dicom_bytes", serde_json::json!(16383)),
    ] {
        let mut malformed = expectation.clone();
        *malformed.pointer_mut(pointer).expect("mutation pointer") = bad;
        assert!(
            !validator.is_valid(&malformed),
            "schema must reject {pointer}"
        );
    }
    let mut missing = expectation;
    missing.as_object_mut().unwrap().remove("optical_paths");
    assert!(!validator.is_valid(&missing));

    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("vl/wsi/multiple_optical_paths")
        })
        .expect("exact multiple optical paths WSI case rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "expected_wsi_multiple_optical_paths"
        ]))
    );
    assert_eq!(
        rule.pointer("/then/properties/profile_membership/const"),
        Some(&serde_json::json!(["extended"]))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_wsi_multiple_optical_paths"]))
    );
}

#[test]
fn manifest_schema_requires_exclusive_twelve_lead_ecg_waveform_contract() {
    let schema = read_json("schemas/manifest.schema.json");
    let rule = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file schema should define case conditionals")
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                == Some("non-image/waveform/twelve_lead_ecg")
        })
        .expect("manifest schema should define the Twelve-lead ECG conditional");

    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!(["expected_waveform"]))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/sop_class_uid/const"),
        Some(&serde_json::json!("1.2.840.10008.5.1.4.1.1.9.1.1"))
    );
    assert_eq!(
        rule.pointer("/then/properties/dicom/properties/modality/const"),
        Some(&serde_json::json!("ECG"))
    );
    assert_eq!(
        rule.pointer("/then/properties/references/maxItems"),
        Some(&serde_json::json!(0))
    );
    assert_eq!(
        rule.pointer("/then/properties/image/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/then/properties/pixel_data/type"),
        Some(&serde_json::json!("null"))
    );
    assert_eq!(
        rule.pointer("/else/else/not/required"),
        Some(&serde_json::json!(["expected_waveform"]))
    );
    assert_eq!(
        rule.pointer("/else/if/properties/case_id/const"),
        Some(&serde_json::json!("non-image/waveform/general_ecg"))
    );
    assert_eq!(
        rule.pointer("/else/then/required"),
        Some(&serde_json::json!(["expected_waveform"]))
    );
}

#[test]
fn manifest_schema_types_exact_general_ecg_waveform_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_general_ecg_waveform",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("General ECG expectation schema should compile");
    let expectation = general_ecg_waveform_expectation();
    assert!(validator.is_valid(&expectation));

    let mut reordered = expectation.clone();
    reordered["multiplex_groups"]
        .as_array_mut()
        .expect("groups")
        .swap(0, 1);
    assert!(!validator.is_valid(&reordered));

    for (pointer, value) in [
        ("/multiplex_groups/0/ordinal", serde_json::json!(2)),
        (
            "/multiplex_groups/0/samples_per_channel",
            serde_json::json!(4000),
        ),
        (
            "/multiplex_groups/0/sampling_frequency_hz",
            serde_json::json!(1000),
        ),
        (
            "/multiplex_groups/1/samples_per_channel",
            serde_json::json!(1000),
        ),
        (
            "/multiplex_groups/1/sampling_frequency_hz",
            serde_json::json!(250),
        ),
        (
            "/multiplex_groups/0/storage/payload_length_bytes",
            serde_json::json!(32000),
        ),
        (
            "/multiplex_groups/1/storage/payload_sha256",
            serde_json::json!("0".repeat(64)),
        ),
        (
            "/multiplex_groups/1/channels/0/source/code_value",
            serde_json::json!("2:1"),
        ),
        (
            "/multiplex_groups/1/channels/3/source/code_value",
            serde_json::json!("2:75"),
        ),
        ("/aggregate/group_count", serde_json::json!(1)),
        ("/aggregate/total_channel_count", serde_json::json!(12)),
        (
            "/aggregate/total_payload_length_bytes",
            serde_json::json!(24000),
        ),
        (
            "/aggregate/aggregate_payload_sha256",
            serde_json::json!("0".repeat(64)),
        ),
    ] {
        let mut mutated = expectation.clone();
        *mutated.pointer_mut(pointer).expect("mutation pointer") = value;
        assert!(!validator.is_valid(&mutated), "must reject {pointer}");
    }

    let mut missing_group = expectation.clone();
    missing_group["multiplex_groups"]
        .as_array_mut()
        .expect("groups")
        .pop();
    assert!(!validator.is_valid(&missing_group));

    let mut missing_aux_channel = expectation.clone();
    missing_aux_channel["multiplex_groups"][1]["channels"]
        .as_array_mut()
        .expect("aux channels")
        .pop();
    assert!(!validator.is_valid(&missing_aux_channel));

    let mut missing_channel_hash = expectation.clone();
    missing_channel_hash["multiplex_groups"][1]["storage"]["channel_sha256"]
        .as_array_mut()
        .expect("aux channel hashes")
        .pop();
    assert!(!validator.is_valid(&missing_channel_hash));

    let mut missing_standard_channel_hash = expectation.clone();
    missing_standard_channel_hash["multiplex_groups"][0]["storage"]["channel_sha256"]
        .as_array_mut()
        .expect("standard channel hashes")
        .pop();
    assert!(!validator.is_valid(&missing_standard_channel_hash));

    let mut missing_group_hash = expectation.clone();
    missing_group_hash["aggregate"]["group_payload_sha256"]
        .as_array_mut()
        .expect("group hashes")
        .pop();
    assert!(!validator.is_valid(&missing_group_hash));

    let mut reversed_group_hashes = expectation;
    reversed_group_hashes["aggregate"]["group_payload_sha256"]
        .as_array_mut()
        .expect("group hashes")
        .swap(0, 1);
    assert!(!validator.is_valid(&reversed_group_hashes));
}

#[test]
fn manifest_schema_types_exact_linked_rt_plan_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_rt_plan",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("linked RT Plan expectation schema should compile");
    let expectation = linked_rt_plan_expectation();
    assert!(validator.is_valid(&expectation));

    for (pointer, value) in [
        ("/references/0/ordinal", serde_json::json!(2)),
        (
            "/references/0/source_path",
            serde_json::json!("non-image/rt/dose_grid_u16_explicit_le/instance.dcm"),
        ),
        ("/references/1/source_sha256", serde_json::json!("bad-hash")),
        ("/plan/geometry", serde_json::json!("TREATMENT_DEVICE")),
        ("/fraction_groups/0/number_of_beams", serde_json::json!(0)),
        (
            "/fraction_groups/0/referenced_beams/0/referenced_beam_number",
            serde_json::json!(2),
        ),
        ("/beams/0/beam_type", serde_json::json!("DYNAMIC")),
        (
            "/beams/0/accessories/number_of_wedges",
            serde_json::json!(1),
        ),
        (
            "/beams/0/accessories/wedge_sequence_absent",
            serde_json::json!(false),
        ),
        (
            "/beams/0/beam_limiting_devices/0/device_type",
            serde_json::json!("Y"),
        ),
        (
            "/beams/0/control_points/0/geometry/jaw_positions_mm/0/0",
            serde_json::json!(-49),
        ),
        (
            "/beams/0/control_points/0/cumulative_meterset_weight",
            serde_json::json!(1),
        ),
        (
            "/beams/0/control_points/1/control_point_index",
            serde_json::json!(0),
        ),
        ("/beams/0/control_points/1/geometry", serde_json::json!({})),
        (
            "/beams/0/final_cumulative_meterset_weight",
            serde_json::json!(0),
        ),
        (
            "/absent_content/common_instance_reference_module",
            serde_json::json!(false),
        ),
    ] {
        let mut mutated = expectation.clone();
        *mutated.pointer_mut(pointer).expect("mutation pointer") = value;
        assert!(!validator.is_valid(&mutated), "must reject {pointer}");
    }

    for pointer in [
        "/references",
        "/fraction_groups",
        "/fraction_groups/0/referenced_beams",
        "/beams",
        "/beams/0/beam_limiting_devices",
        "/beams/0/control_points",
    ] {
        let mut missing = expectation.clone();
        missing
            .pointer_mut(pointer)
            .expect("array pointer")
            .as_array_mut()
            .expect("array")
            .pop();
        assert!(
            !validator.is_valid(&missing),
            "must reject cardinality {pointer}"
        );
    }

    for pointer in [
        "/references",
        "/beams/0/beam_limiting_devices",
        "/beams/0/control_points",
    ] {
        let mut reordered = expectation.clone();
        reordered
            .pointer_mut(pointer)
            .expect("array pointer")
            .as_array_mut()
            .expect("array")
            .swap(0, 1);
        assert!(
            !validator.is_valid(&reordered),
            "must reject order {pointer}"
        );
    }
}

#[test]
fn manifest_schema_scopes_linked_rt_plan_expectation_to_its_case() {
    let schema = read_json("schemas/manifest.schema.json");
    let rules = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file conditions");
    let rule = rules
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                == Some(&serde_json::json!("non-image/rt/plan_linked"))
        })
        .expect("linked RT Plan rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!(["expected_rt_plan"]))
    );
    assert_eq!(
        rule.pointer("/then/properties/references/minItems"),
        Some(&serde_json::json!(2))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_rt_plan"]))
    );
}

#[test]
fn manifest_schema_types_exact_linked_rt_image_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_rt_image",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("linked RT Image expectation schema should compile");
    let expectation = linked_rt_image_expectation();
    assert!(validator.is_valid(&expectation));

    for (pointer, value) in [
        (
            "/plan_reference/relationship",
            serde_json::json!("referenced_dose"),
        ),
        (
            "/plan_reference/source_path",
            serde_json::json!("non-image/rt/plan_linked/wrong.dcm"),
        ),
        (
            "/plan_reference/source_sha256",
            serde_json::json!("bad-hash"),
        ),
        (
            "/plan_reference/sop_class_uid",
            serde_json::json!("1.2.840.10008.5.1.4.1.1.481.2"),
        ),
        (
            "/linkage/referenced_fraction_group_number",
            serde_json::json!(2),
        ),
        ("/linkage/referenced_beam_number", serde_json::json!(2)),
        ("/image/image_type/2", serde_json::json!("PORTAL")),
        ("/image/label", serde_json::json!("WRONG")),
        ("/image/plane", serde_json::json!("NON_NORMAL")),
        (
            "/image/image_plane_pixel_spacing_mm/0",
            serde_json::json!(2),
        ),
        ("/image/position_mm/0", serde_json::json!(-1.0)),
        ("/image/radiation_machine_sad_mm", serde_json::json!(999)),
        ("/image/rt_image_sid_mm", serde_json::json!(1499)),
        ("/storage/rows", serde_json::json!(5)),
        ("/storage/payload_length_bytes", serde_json::json!(15)),
        ("/storage/bits_stored", serde_json::json!(7)),
        ("/storage/high_bit", serde_json::json!(6)),
        ("/storage/pixel_representation", serde_json::json!(1)),
        ("/storage/pixel_values/7", serde_json::json!(118)),
        (
            "/storage/payload_sha256",
            serde_json::json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        (
            "/storage/decoded_pixels_sha256",
            serde_json::json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        (
            "/absent_content/reported_values_origin",
            serde_json::json!(false),
        ),
        (
            "/absent_content/rt_image_orientation",
            serde_json::json!(false),
        ),
        (
            "/absent_content/encapsulated_pixel_data",
            serde_json::json!(false),
        ),
    ] {
        let mut mutated = expectation.clone();
        *mutated.pointer_mut(pointer).expect("mutation pointer") = value;
        assert!(!validator.is_valid(&mutated), "must reject {pointer}");
    }

    let mut short_type = expectation.clone();
    short_type["image"]["image_type"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(!validator.is_valid(&short_type));

    let mut short_pixels = expectation.clone();
    short_pixels["storage"]["pixel_values"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(!validator.is_valid(&short_pixels));
}

#[test]
fn manifest_schema_scopes_linked_rt_image_expectation_to_its_case() {
    let schema = read_json("schemas/manifest.schema.json");
    let rules = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file conditions");
    let rule = rules
        .iter()
        .find(|rule| {
            rule.pointer("/if/properties/case_id/const")
                == Some(&serde_json::json!("non-image/rt/image_linked"))
        })
        .expect("linked RT Image rule");
    assert_eq!(
        rule.pointer("/then/required"),
        Some(&serde_json::json!([
            "image",
            "pixel_data",
            "expected_rt_image"
        ]))
    );
    assert_eq!(
        rule.pointer("/then/properties/references/minItems"),
        Some(&serde_json::json!(1))
    );
    assert_eq!(
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_rt_image"]))
    );
}

#[test]
fn manifest_schema_types_exact_carm_rt_radiation_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_rt_radiation",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("C-Arm RT Radiation expectation schema should compile");
    let expectation = minimal_carm_rt_radiation_expectation();
    assert!(validator.is_valid(&expectation));

    for (pointer, value) in [
        ("/iod_kind", serde_json::json!("rt_radiation")),
        (
            "/definition_source/relationship",
            serde_json::json!("referenced_rt_plan"),
        ),
        (
            "/definition_source/referenced_beam_number",
            serde_json::json!(2),
        ),
        ("/content/rt_record_flag", serde_json::json!("YES")),
        (
            "/content/treatment_technique/code_value",
            serde_json::json!("wrong"),
        ),
        ("/device/serial_number", serde_json::json!("OTHER")),
        (
            "/dosimeter_unit/coding_scheme_designator",
            serde_json::json!("DCM"),
        ),
        (
            "/equipment_frame_of_reference_uid",
            serde_json::json!("2.25.999"),
        ),
        (
            "/treatment_positions/0/image_to_equipment_mapping_matrix/1",
            serde_json::json!(1),
        ),
        (
            "/control_points/0/cumulative_meterset",
            serde_json::json!(1),
        ),
        ("/control_points/1/geometry", serde_json::json!({})),
        (
            "/control_points/1/inherits_geometry_from_control_point",
            serde_json::json!(2),
        ),
        (
            "/absent_content/recorded_control_point_attributes",
            serde_json::json!(false),
        ),
    ] {
        let mut mutated = expectation.clone();
        *mutated.pointer_mut(pointer).expect("mutation pointer") = value;
        assert!(!validator.is_valid(&mutated), "must reject {pointer}");
    }

    for pointer in ["/treatment_positions", "/control_points"] {
        let mut missing = expectation.clone();
        missing
            .pointer_mut(pointer)
            .expect("array pointer")
            .as_array_mut()
            .expect("array")
            .pop();
        assert!(
            !validator.is_valid(&missing),
            "must reject cardinality {pointer}"
        );
    }

    let mut reversed = expectation;
    reversed["control_points"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(!validator.is_valid(&reversed));
}

#[test]
fn manifest_schema_types_exact_rt_radiation_set_expectations() {
    let schema = read_json("schemas/manifest.schema.json");
    let expectation_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/expected_rt_radiation_set",
        "$defs": schema["$defs"].clone(),
    });
    let validator = jsonschema::validator_for(&expectation_schema)
        .expect("RT Radiation Set expectation schema should compile");
    let expectation = minimal_rt_radiation_set_expectation();
    assert!(validator.is_valid(&expectation));

    for (pointer, value) in [
        ("/content/intent", serde_json::json!("SIMULATION")),
        (
            "/content/intended_number_of_fractions",
            serde_json::json!(2),
        ),
        (
            "/definition_source/source_case_id",
            serde_json::json!("wrong/case"),
        ),
        ("/radiation_references/0/ordinal", serde_json::json!(2)),
        (
            "/radiation_references/0/sop_class_uid",
            serde_json::json!("1.2.840.10008.5.1.4.1.1.481.5"),
        ),
        (
            "/treatment_position_groups/0/label",
            serde_json::json!("OTHER"),
        ),
        (
            "/treatment_position_groups/0/radiation_references/0/relationship",
            serde_json::json!("definition_source"),
        ),
        (
            "/common_instance_references/1/ordinal",
            serde_json::json!(1),
        ),
        (
            "/absent_content/rt_dose_contribution_module",
            serde_json::json!(false),
        ),
    ] {
        let mut mutated = expectation.clone();
        *mutated.pointer_mut(pointer).expect("mutation pointer") = value;
        assert!(!validator.is_valid(&mutated), "must reject {pointer}");
    }

    for pointer in [
        "/radiation_references",
        "/treatment_position_groups",
        "/treatment_position_groups/0/radiation_references",
        "/common_instance_references",
    ] {
        let mut missing = expectation.clone();
        missing
            .pointer_mut(pointer)
            .expect("array pointer")
            .as_array_mut()
            .expect("array")
            .pop();
        assert!(
            !validator.is_valid(&missing),
            "must reject cardinality {pointer}"
        );
    }

    let mut reversed = expectation;
    reversed["common_instance_references"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(!validator.is_valid(&reversed));
}

#[test]
fn manifest_schema_scopes_second_generation_rt_expectations_to_their_cases() {
    let schema = read_json("schemas/manifest.schema.json");
    let rules = schema
        .pointer("/$defs/file/allOf")
        .and_then(Value::as_array)
        .expect("file conditions");

    for (case_id, field, reference_count) in [
        (
            "non-image/rt/carm_photon_electron_radiation_minimal",
            "expected_rt_radiation",
            1,
        ),
        (
            "non-image/rt/radiation_set_minimal",
            "expected_rt_radiation_set",
            2,
        ),
    ] {
        let rule = rules
            .iter()
            .find(|rule| {
                rule.pointer("/if/properties/case_id/const") == Some(&serde_json::json!(case_id))
            })
            .unwrap_or_else(|| panic!("missing rule for {case_id}"));
        assert_eq!(
            rule.pointer("/then/required"),
            Some(&serde_json::json!([field]))
        );
        assert_eq!(
            rule.pointer("/then/properties/references/minItems"),
            Some(&serde_json::json!(reference_count))
        );
        assert_eq!(
            rule.pointer("/then/properties/references/maxItems"),
            Some(&serde_json::json!(reference_count))
        );
        assert_eq!(
            rule.pointer("/else/not/required"),
            Some(&serde_json::json!([field]))
        );
    }
}

fn twelve_lead_ecg_waveform_expectation() -> Value {
    let leads = [
        (1, "I", "2:1", "Lead I"),
        (2, "II", "2:2", "Lead II"),
        (3, "III", "2:61", "Lead III"),
        (4, "aVR", "2:62", "aVR, augmented voltage, right"),
        (5, "aVL", "2:63", "aVL, augmented voltage, left"),
        (6, "aVF", "2:64", "aVF, augmented voltage, foot"),
        (7, "V1", "2:3", "Lead V1"),
        (8, "V2", "2:4", "Lead V2"),
        (9, "V3", "2:5", "Lead V3"),
        (10, "V4", "2:6", "Lead V4"),
        (11, "V5", "2:7", "Lead V5"),
        (12, "V6", "2:8", "Lead V6"),
    ];
    let channels = leads.map(|(ordinal, label, code_value, code_meaning)| {
        serde_json::json!({
            "ordinal": ordinal,
            "label": label,
            "source": {
                "code_value": code_value,
                "coding_scheme_designator": "MDC",
                "code_meaning": code_meaning
            },
            "sensitivity": 1,
            "sensitivity_units": {
                "code_value": "uV",
                "coding_scheme_designator": "UCUM",
                "code_meaning": "microvolt"
            },
            "sensitivity_correction_factor": 1,
            "baseline": 0,
            "bits_stored": 16,
            "time_skew_seconds": 0,
            "sample_skew_absent": true
        })
    });

    serde_json::json!({
        "iod_kind": "twelve_lead_ecg",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.1",
        "iod_name": "12-lead ECG Waveform",
        "modality": "ECG",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "acquisition_context_items": 0,
        "multiplex_groups": [{
            "ordinal": 1,
            "originality": "ORIGINAL",
            "label": "RESTING_12_LEAD",
            "channel_count": 12,
            "samples_per_channel": 500,
            "sampling_frequency_hz": 500,
            "duration_seconds": 1,
            "simultaneous_sampling": true,
            "channels": channels,
            "storage": {
                "bits_allocated": 16,
                "sample_interpretation": "SS",
                "data_vr": "OW",
                "byte_order": "little_endian",
                "interleave_order": "channel_then_sample",
                "payload_length_bytes": 12000,
                "payload_sha256": "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
                "channel_sha256": [
                "7b4aee068e05c2bdff3896937c78a4c7a32f9ed2bde64d91b1d925913bf29476",
                "bd775dc70f76ea153a25832ad622b0cc26fbe6a37cf3ec6548a30965c4d17fba",
                "19d26b694df281209aa1296abbfa8f7d360e24a03a091422aba6f67663e2f3b1",
                "bb4c99d7857dbfcee5ee620bcff09b7060b61c5f2432427affc6139cb8d3cf9b",
                "230f52ed2ac57624a9a35214d7867711008dd56014f4176ce258623e5b596d3a",
                "60e167db3c081ba5bca957aba820afb519b790d048b660634d49566df88105f2",
                "cf8c73bebf746b799b1fe8aa2c908ca69bc7acc72311c64cbf4131fc8976609f",
                "0f11e5fb5105dac699fa4bcfc01c79fbe696a81db04606f39a719de57b4c7c30",
                "a41d5962abceb6dbe25f8421091ce3df6a69202c45b24ab6b0736159d15e253b",
                "d655e2cbb23d70e229ed52fedba9c45573e22729fed0a794ab690df8d7f33804",
                "005c539f9f4256a86d9e0a212b3bfe73741f99942b0677fb483c0c48db9583cd",
                "f448df95acb226c5c992363e27707a42efc3ffb974ebeff38e2a81522b57d82c"
                ],
                "sample_value_formula": "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000",
                "sample_min": -1000,
                "sample_max": 1000,
                "waveform_padding_value_absent": true,
                "value_field_padding_bytes": 0
            }
        }],
        "aggregate": {
            "group_count": 1,
            "total_channel_count": 12,
            "common_duration_seconds": 1,
            "total_payload_length_bytes": 12000,
            "group_payload_sha256": [
                "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
            ],
            "aggregate_payload_sha256": "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
        },
        "absent_content": {
            "annotation_module": true,
            "synchronization_module": true,
            "references": true,
            "image": true,
            "pixel_data": true
        }
    })
}

fn general_ecg_waveform_expectation() -> Value {
    let mut expectation = twelve_lead_ecg_waveform_expectation();
    expectation["iod_kind"] = serde_json::json!("general_ecg");
    expectation["sop_class_uid"] = serde_json::json!("1.2.840.10008.5.1.4.1.1.9.1.2");
    expectation["iod_name"] = serde_json::json!("General ECG Waveform");

    let formula = "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000";
    let mut standard = expectation["multiplex_groups"][0].clone();
    standard["label"] = serde_json::json!("STD12_250HZ");
    standard["samples_per_channel"] = serde_json::json!(1000);
    standard["sampling_frequency_hz"] = serde_json::json!(250);
    standard["duration_seconds"] = serde_json::json!(4);
    standard["storage"]["payload_length_bytes"] = serde_json::json!(24000);
    standard["storage"]["payload_sha256"] =
        serde_json::json!("e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb");
    standard["storage"]["channel_sha256"] = serde_json::json!([
        "3211bada5580e8bd9c5a2934deb231122706b00aa92f8cdc78480c03b2352197",
        "8f66471e35940851acdd9ea55b422c738bf50ea7971822deed0edca1980e1ea2",
        "9652eb91f4f73f2654c922048a1a8c8731a08062eecd6f5b373256831d0e82b0",
        "97fb26e75907437a705e4e28eb6492d51020570a23265bdf765aca3c4e7b2708",
        "c9776b85b3bda6adef798d33d3c7c95d64a1a7d5bf525866ccf7b0cf5fc3209e",
        "95871f48d729a001eeb1543b36a27059916df360e04838fd322d006661bafb44",
        "04513ee1f1d5803f3f53093f016a606a7fa874c5af8d2651749b909b93392366",
        "c12790f5b1f233662a0a1c3f266cd2abb15af5a75b39258ff961e9b4afaf7913",
        "750913ccad5eb7ec8d8199451e6eb9aa41357eb21d2a0dac6ba75dce4e5708bd",
        "218d5f967ef253722359fee1846485331c63de9330af1f9fad183d779a196cca",
        "9027ec7a0fc7fea3d8236a16a5aa6f265ff20e18a2575f99e61807e102fb3d81",
        "9280ad35672b82a7847d3ccabadd4d85a94be3d39d0a836191384571f0a23ab6"
    ]);
    standard["storage"]["sample_value_formula"] = serde_json::json!(formula);

    let auxiliary_channels = [
        (1, "A1", "2:75", "Auxiliary unipolar lead 1"),
        (2, "A2", "2:76", "Auxiliary unipolar lead 2"),
        (3, "A3", "2:77", "Auxiliary unipolar lead 3"),
        (4, "A4", "2:78", "Auxiliary unipolar lead 4"),
    ]
    .map(|(ordinal, label, code_value, code_meaning)| {
        serde_json::json!({
            "ordinal": ordinal,
            "label": label,
            "source": {
                "code_value": code_value,
                "coding_scheme_designator": "MDC",
                "code_meaning": code_meaning
            },
            "sensitivity": 1,
            "sensitivity_units": {
                "code_value": "uV",
                "coding_scheme_designator": "UCUM",
                "code_meaning": "microvolt"
            },
            "sensitivity_correction_factor": 1,
            "baseline": 0,
            "bits_stored": 16,
            "time_skew_seconds": 0,
            "sample_skew_absent": true
        })
    });
    let auxiliary = serde_json::json!({
        "ordinal": 2,
        "originality": "ORIGINAL",
        "label": "AUX4_1000HZ",
        "channel_count": 4,
        "samples_per_channel": 4000,
        "sampling_frequency_hz": 1000,
        "duration_seconds": 4,
        "simultaneous_sampling": true,
        "channels": auxiliary_channels,
        "storage": {
            "bits_allocated": 16,
            "sample_interpretation": "SS",
            "data_vr": "OW",
            "byte_order": "little_endian",
            "interleave_order": "channel_then_sample",
            "payload_length_bytes": 32000,
            "payload_sha256": "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
            "channel_sha256": [
                "5da46776ad84a78eb0c16066cb8ac7d5e05ca6ad87170264b227c71261def284",
                "7bd73425422f4e79504b55932040e481ccdfafecabe1dba613ee36074a51b9e3",
                "e56dad9647dfa50a10b40d244e29eaedbf23d97a558901f46fbccc07ad1a1766",
                "e1b68207c92fe2cc4c6765fc097668f2600eeda152eb5a1d6f0444f4c9e36fbc"
            ],
            "sample_value_formula": formula,
            "sample_min": -1000,
            "sample_max": 1000,
            "waveform_padding_value_absent": true,
            "value_field_padding_bytes": 0
        }
    });
    expectation["multiplex_groups"] = serde_json::json!([standard, auxiliary]);
    expectation["aggregate"] = serde_json::json!({
        "group_count": 2,
        "total_channel_count": 16,
        "common_duration_seconds": 4,
        "total_payload_length_bytes": 56000,
        "group_payload_sha256": [
            "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
            "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe"
        ],
        "aggregate_payload_sha256": "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea"
    });
    expectation
}

fn minimal_carm_rt_radiation_expectation() -> Value {
    serde_json::json!({
        "iod_kind": "carm_photon_electron_radiation",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.13",
        "iod_name": "C-Arm Photon-Electron Radiation",
        "modality": "RTRAD",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "sop_instance_uid": "2.25.301",
        "study_instance_uid": "2.25.302",
        "series_instance_uid": "2.25.303",
        "frame_of_reference_uid": "2.25.304",
        "instance": rt_radiation_instance(74, "Native C-Arm Photon-Electron Radiation"),
        "definition_source": {
            "relationship": "definition_source",
            "source_case_id": "non-image/rt/plan_linked",
            "source_path": "non-image/rt/plan_linked/instance.dcm",
            "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "study_instance_uid": "2.25.302",
            "series_instance_uid": "2.25.305",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
            "sop_instance_uid": "2.25.306",
            "frame_of_reference_uid": "2.25.304",
            "referenced_beam_number": 1,
            "common_instance_reference_ordinal": 1
        },
        "content": {
            "user_content_label": "DTS_RADIATION",
            "content_description": "",
            "physical_and_geometric_content_detail_flag": "IDENT_ONLY",
            "rt_record_flag": "NO",
            "treatment_technique": rt_code("130102", "DCM", "Static Beam"),
            "number_of_rt_control_points": 2
        },
        "device": rt_treatment_device(),
        "dosimeter_unit": rt_code("{MU}", "UCUM", "Monitor Units"),
        "distance_reference_location": rt_code("130358", "DCM", "Nominal Radiation Source Location"),
        "equipment_frame_of_reference_uid": "1.2.840.10008.1.4.3.1",
        "rt_beam_modifier_definition_distance_mm": 500,
        "equipment_reference_point_coordinates_sequence_present_empty": true,
        "number_of_patient_support_devices": 0,
        "radiation_source_axis_distance_mm": 1000,
        "patient_orientation": rt_code("102538003", "SCT", "recumbent"),
        "patient_orientation_modifier": rt_code("40199007", "SCT", "supine"),
        "patient_equipment_relationship": rt_code("102540008", "SCT", "headfirst"),
        "treatment_positions": [{
            "ordinal": 1,
            "treatment_position_index": 1,
            "image_to_equipment_mapping_matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
            "patient_location_coordinates_present_empty": true,
            "patient_support_position_sequence_present_empty": true
        }],
        "control_points": [
            {
                "ordinal": 1,
                "rt_control_point_index": 1,
                "cumulative_meterset": 0,
                "geometry": {
                    "referenced_treatment_position_index": 1,
                    "source_roll_angle_degrees": 0,
                    "rt_beam_limiting_device_angle_degrees": 0,
                    "delivery_rate_present_empty": true,
                    "source_to_patient_surface_distance_present_empty": true,
                    "source_to_external_contour_distance_present_empty": true,
                    "delivery_rate_unit_sequence_absent": true
                },
                "inherits_geometry_from_control_point": null
            },
            {
                "ordinal": 2,
                "rt_control_point_index": 2,
                "cumulative_meterset": 100,
                "geometry": null,
                "inherits_geometry_from_control_point": 1
            }
        ],
        "absent_content": {
            "patient_study_module": true,
            "clinical_trial_modules": true,
            "referenced_performed_procedure_step_sequences": true,
            "treatment_session_uid": true,
            "treatment_machine_special_mode": true,
            "rt_tolerance_set": true,
            "treatment_time_limit": true,
            "device_alternate_identifier_type": true,
            "device_alternate_identifier_format": true,
            "unique_device_identifier_sequence": true,
            "device_manufacture_date": true,
            "device_expiration_date": true,
            "device_institution_content": true,
            "long_device_description": true,
            "patient_support_devices_sequence": true,
            "radiation_generation_mode": true,
            "beam_limiting_device_definition_and_opening": true,
            "wedge": true,
            "compensator": true,
            "block": true,
            "accessory_holder": true,
            "general_accessory": true,
            "bolus": true,
            "beam_area_limit": true,
            "recorded_control_point_attributes": true,
            "image": true,
            "pixel_data": true,
            "synchronization": true
        }
    })
}

fn minimal_rt_radiation_set_expectation() -> Value {
    let plan_reference = serde_json::json!({
        "ordinal": 1,
        "relationship": "definition_source",
        "source_case_id": "non-image/rt/plan_linked",
        "source_path": "non-image/rt/plan_linked/instance.dcm",
        "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "study_instance_uid": "2.25.302",
        "series_instance_uid": "2.25.305",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
        "sop_instance_uid": "2.25.306",
        "frame_of_reference_uid": "2.25.304"
    });
    let radiation_reference = serde_json::json!({
        "ordinal": 1,
        "relationship": "referenced_rt_radiation",
        "source_case_id": "non-image/rt/carm_photon_electron_radiation_minimal",
        "source_path": "non-image/rt/carm_photon_electron_radiation_minimal/instance.dcm",
        "source_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "study_instance_uid": "2.25.302",
        "series_instance_uid": "2.25.303",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.13",
        "sop_instance_uid": "2.25.301",
        "frame_of_reference_uid": "2.25.304"
    });
    let mut common_radiation_reference = radiation_reference.clone();
    common_radiation_reference["ordinal"] = serde_json::json!(2);

    serde_json::json!({
        "iod_kind": "rt_radiation_set",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.12",
        "iod_name": "RT Radiation Set",
        "modality": "RTRAD",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "sop_instance_uid": "2.25.307",
        "study_instance_uid": "2.25.302",
        "series_instance_uid": "2.25.308",
        "frame_of_reference_uid": "2.25.304",
        "instance": rt_radiation_instance(75, "Native RT Radiation Set"),
        "content": {
            "user_content_label": "DTS_RADSET",
            "content_description": "",
            "intent": "TREATMENT",
            "intended_number_of_fractions": 1,
            "referenced_rt_physician_intent_sequence_present_empty": true,
            "author_identification_sequence_present_empty": true
        },
        "linked_radiation_device": rt_treatment_device(),
        "definition_source": plan_reference.clone(),
        "radiation_references": [radiation_reference.clone()],
        "treatment_position_groups": [{
            "ordinal": 1,
            "treatment_position_group_uid": "2.25.309",
            "label": "DTS_TPG_1",
            "radiation_references": [radiation_reference]
        }],
        "common_instance_references": [plan_reference, common_radiation_reference],
        "absent_content": {
            "patient_study_module": true,
            "clinical_trial_modules": true,
            "referenced_performed_procedure_step_sequences": true,
            "treatment_session_uid": true,
            "synchronization": true,
            "rt_dose_contribution_module": true,
            "fraction_pattern_sequence": true,
            "image": true,
            "pixel_data": true
        }
    })
}

fn rt_radiation_instance(series_number: u8, equipment_model_name: &str) -> Value {
    serde_json::json!({
        "series_number": series_number,
        "instance_number": 1,
        "series_date": "20260101",
        "series_time": "000000",
        "instance_creation_date": "20260101",
        "instance_creation_time": "000000",
        "content_date": "20260101",
        "content_time": "000000",
        "patient_name": "DTS^Synthetic^Patient001",
        "patient_id": "DTS-PATIENT-001",
        "patient_birth_date": "19700101",
        "patient_sex": "O",
        "study_id": "DTS-RTSTRUCT",
        "referring_physician_name": "",
        "accession_number": "",
        "position_reference_indicator": "",
        "equipment_manufacturer": "dicom-test-suite",
        "equipment_model_name": equipment_model_name,
        "equipment_serial_number": "DTS-LINAC-001",
        "software_versions": "0.1.0",
        "author_identification_sequence_present_empty": true
    })
}

fn rt_treatment_device() -> Value {
    serde_json::json!({
        "manufacturer": "dicom-test-suite",
        "model_name": "DTS C-Arm LINAC",
        "model_version": "1",
        "device_label": "DTS_LINAC",
        "serial_number": "DTS-LINAC-001",
        "software_versions": "0.1.0",
        "manufacturer_device_identifier": "DTS-LINAC-001",
        "manufacturer_device_class_uid": "",
        "device_alternate_identifier": "",
        "device_type": rt_code("130361", "DCM", "Radiotherapy Treatment Device")
    })
}

fn rt_code(value: &str, scheme: &str, meaning: &str) -> Value {
    serde_json::json!({
        "code_value": value,
        "coding_scheme_designator": scheme,
        "code_meaning": meaning
    })
}

fn linked_rt_plan_expectation() -> Value {
    serde_json::json!({
        "iod_kind": "rt_plan",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
        "iod_name": "RT Plan",
        "modality": "RTPLAN",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "sop_instance_uid": "2.25.101",
        "study_instance_uid": "2.25.102",
        "series_instance_uid": "2.25.103",
        "frame_of_reference_uid": "2.25.104",
        "references": [
            {
                "ordinal": 1,
                "relationship": "referenced_structure_set",
                "source_case_id": "non-image/rt/structure_set_single_roi_explicit_le",
                "source_path": "non-image/rt/structure_set_single_roi_explicit_le/instance.dcm",
                "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "study_instance_uid": "2.25.102",
                "series_instance_uid": "2.25.105",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.3",
                "sop_instance_uid": "2.25.106",
                "frame_of_reference_uid": "2.25.104"
            },
            {
                "ordinal": 2,
                "relationship": "referenced_dose",
                "source_case_id": "non-image/rt/dose_grid_u16_explicit_le",
                "source_path": "non-image/rt/dose_grid_u16_explicit_le/instance.dcm",
                "source_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "study_instance_uid": "2.25.102",
                "series_instance_uid": "2.25.107",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.2",
                "sop_instance_uid": "2.25.108",
                "frame_of_reference_uid": "2.25.104"
            }
        ],
        "plan": { "label": "DTS_PLAN", "date": "20260101", "time": "000000", "geometry": "PATIENT" },
        "fraction_groups": [{
            "ordinal": 1,
            "fraction_group_number": 1,
            "number_of_fractions_planned": 1,
            "number_of_beams": 1,
            "number_of_brachy_application_setups": 0,
            "referenced_beams": [{ "ordinal": 1, "referenced_beam_number": 1 }]
        }],
        "beams": [{
            "ordinal": 1,
            "treatment_machine_name": "DTS_LINAC",
            "primary_dosimeter_unit": "MU",
            "source_axis_distance_mm": 1000,
            "beam_number": 1,
            "beam_name": "DTS_STATIC_AP",
            "beam_type": "STATIC",
            "radiation_type": "PHOTON",
            "treatment_delivery_type": "TREATMENT",
            "accessories": {
                "number_of_wedges": 0, "wedge_sequence_absent": true,
                "number_of_compensators": 0, "compensator_sequence_absent": true,
                "number_of_boli": 0, "bolus_sequence_absent": true,
                "number_of_blocks": 0, "block_sequence_absent": true
            },
            "beam_limiting_devices": [
                { "ordinal": 1, "device_type": "X", "number_of_leaf_jaw_pairs": 1, "source_to_device_distance_mm": 500 },
                { "ordinal": 2, "device_type": "Y", "number_of_leaf_jaw_pairs": 1, "source_to_device_distance_mm": 500 }
            ],
            "number_of_control_points": 2,
            "final_cumulative_meterset_weight": 1,
            "control_points": [
                {
                    "ordinal": 1,
                    "control_point_index": 0,
                    "cumulative_meterset_weight": 0,
                    "geometry": {
                        "nominal_beam_energy_mev": 6,
                        "jaw_positions_mm": [[-50, 50], [-50, 50]],
                        "gantry_angle_degrees": 0,
                        "gantry_rotation_direction": "NONE",
                        "beam_limiting_device_angle_degrees": 0,
                        "beam_limiting_device_rotation_direction": "NONE",
                        "patient_support_angle_degrees": 0,
                        "patient_support_rotation_direction": "NONE",
                        "table_top_vertical_position_mm": 0,
                        "table_top_longitudinal_position_mm": 0,
                        "table_top_lateral_position_mm": 0,
                        "table_top_pitch_angle_degrees": 0,
                        "table_top_pitch_rotation_direction": "NONE",
                        "table_top_roll_angle_degrees": 0,
                        "table_top_roll_rotation_direction": "NONE",
                        "isocenter_position_mm": [0, 0, 0]
                    },
                    "inherits_geometry_from_control_point": null
                },
                {
                    "ordinal": 2,
                    "control_point_index": 1,
                    "cumulative_meterset_weight": 1,
                    "geometry": null,
                    "inherits_geometry_from_control_point": 0
                }
            ]
        }],
        "absent_content": {
            "referenced_rt_plan_sequence": true,
            "rt_prescription_module": true,
            "rt_tolerance_tables_module": true,
            "rt_patient_setup_module": true,
            "rt_brachy_application_setups_module": true,
            "approval_module": true,
            "clinical_trial_module": true,
            "common_instance_reference_module": true,
            "image": true,
            "pixel_data": true
        }
    })
}

fn linked_rt_image_expectation() -> Value {
    serde_json::json!({
        "iod_kind": "rt_image",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.1",
        "iod_name": "RT Image",
        "modality": "RTIMAGE",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "sop_instance_uid": "2.25.201",
        "study_instance_uid": "2.25.202",
        "series_instance_uid": "2.25.203",
        "frame_of_reference_uid": "2.25.204",
        "plan_reference": {
            "relationship": "referenced_rt_plan",
            "source_case_id": "non-image/rt/plan_linked",
            "source_path": "non-image/rt/plan_linked/instance.dcm",
            "source_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "study_instance_uid": "2.25.202",
            "series_instance_uid": "2.25.205",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
            "sop_instance_uid": "2.25.206",
            "frame_of_reference_uid": "2.25.204"
        },
        "linkage": {
            "referenced_fraction_group_number": 1,
            "referenced_beam_number": 1
        },
        "image": {
            "image_type": ["DERIVED", "SECONDARY", "DRR"],
            "conversion_type": "WSD",
            "label": "DTS_DRR",
            "plane": "NORMAL",
            "xray_image_receptor_angle_degrees": 0,
            "image_plane_pixel_spacing_mm": [1, 1],
            "position_mm": [-1.5, 1.5],
            "radiation_machine_name": "DTS_LINAC",
            "radiation_machine_sad_mm": 1000,
            "rt_image_sid_mm": 1500,
            "primary_dosimeter_unit": "MU"
        },
        "storage": {
            "rows": 4,
            "columns": 4,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": 8,
            "bits_stored": 8,
            "high_bit": 7,
            "pixel_representation": 0,
            "data_vr": "OB",
            "encoding": "native",
            "payload_length_bytes": 16,
            "value_field_padding_bytes": 0,
            "pixel_value_formula": "17 * (4 * r + c)",
            "pixel_values": [0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255],
            "pixel_min": 0,
            "pixel_max": 255,
            "payload_sha256": "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811",
            "decoded_pixels_sha256": "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811"
        },
        "absent_content": {
            "patient_study_module": true,
            "contrast_bolus_module": true,
            "cine_module": true,
            "multi_frame_module": true,
            "modality_lut_module": true,
            "voi_lut_module": true,
            "approval_module": true,
            "clinical_trial_module": true,
            "frame_extraction_module": true,
            "common_instance_reference_module": true,
            "reported_values_origin": true,
            "rt_image_orientation": true,
            "isocenter_position": true,
            "patient_position": true,
            "fluence_map_sequence": true,
            "exposure_sequence": true,
            "overlays": true,
            "encapsulated_pixel_data": true,
            "lossy_pixel_attributes": true
        }
    })
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let contents =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse {path:?} as JSON: {err}"))
}
