#![recursion_limit = "256"]

use std::fs;
use std::path::Path;

use serde_json::Value;

const SCHEMAS: &[(&str, &str)] = &[
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
    reordered["channels"]
        .as_array_mut()
        .expect("channels")
        .swap(0, 1);
    assert!(!validator.is_valid(&reordered));

    let mut duplicate_lead = expectation.clone();
    duplicate_lead["channels"][11]["source"] = duplicate_lead["channels"][0]["source"].clone();
    assert!(!validator.is_valid(&duplicate_lead));

    let mut wrong_rate = expectation.clone();
    wrong_rate["multiplex_group"]["sampling_frequency_hz"] = serde_json::json!(199);
    assert!(!validator.is_valid(&wrong_rate));

    let mut wrong_interleave = expectation.clone();
    wrong_interleave["storage"]["interleave_order"] = serde_json::json!("sample_then_channel");
    assert!(!validator.is_valid(&wrong_interleave));

    let mut corrupt_payload = expectation.clone();
    corrupt_payload["storage"]["payload_sha256"] = serde_json::json!("0".repeat(64));
    assert!(!validator.is_valid(&corrupt_payload));

    let mut padded = expectation;
    padded["storage"]["value_field_padding_bytes"] = serde_json::json!(2);
    assert!(!validator.is_valid(&padded));
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
        rule.pointer("/else/not/required"),
        Some(&serde_json::json!(["expected_waveform"]))
    );
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
        "multiplex_group": {
            "group_count": 1,
            "originality": "ORIGINAL",
            "label": "RESTING_12_LEAD",
            "channel_count": 12,
            "samples_per_channel": 500,
            "sampling_frequency_hz": 500,
            "duration_seconds": 1,
            "simultaneous_sampling": true
        },
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

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let contents =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse {path:?} as JSON: {err}"))
}
