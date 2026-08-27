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

    assert!(
        schema
            .pointer("/$defs/image/properties/sample_type/enum")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("float32"))),
        "image metadata must distinguish float32 samples"
    );
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
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/coverage_row/properties/{field}/type/0"))
                .and_then(Value::as_str),
            Some("boolean"),
            "coverage row {field} must be nullable boolean"
        );
    }
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

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let contents =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse {path:?} as JSON: {err}"))
}
