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

    assert!(
        schema.pointer("/$defs/case/properties/modality").is_some(),
        "case registry schema must allow optional modality metadata"
    );

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
        "enhanced_mr_velocity_encoding_minimum_value",
        "enhanced_mr_velocity_encoding_maximum_value",
        "segmentation_type",
        "segmentation_fractional_type",
        "segmentation_maximum_fractional_value",
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
        "enhanced_mr_velocity_encoding_minimum_values",
        "enhanced_mr_velocity_encoding_maximum_values",
        "segmentation_types",
        "segmentation_fractional_types",
        "segmentation_maximum_fractional_values",
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
