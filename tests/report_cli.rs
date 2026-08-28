#![recursion_limit = "256"]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use serde_json::json;

#[test]
fn report_command_writes_json_coverage_for_core_root() {
    let out_dir = unique_temp_dir("report-core-json");
    generate_core(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let report_schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json")
            .expect("coverage report schema should be readable"),
    )
    .expect("coverage report schema should be JSON");
    let report_validator =
        jsonschema::validator_for(&report_schema).expect("coverage schema should compile");
    let report_errors = report_validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        report_errors.is_empty(),
        "metadata coverage report must match its schema: {report_errors:?}"
    );
    let mut partial_xa_report = report.clone();
    coverage_row_mut(&mut partial_xa_report, "classic/xa/monoplane_explicit_le")["xa_frame_count"] =
        Value::Null;
    assert!(
        !report_validator.is_valid(&partial_xa_report),
        "coverage schema must reject a partial non-null XA contract"
    );
    let mut hidden_xa_report = report.clone();
    coverage_row_mut(
        &mut hidden_xa_report,
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    )["xa_frame_count"] = Value::from(1);
    assert!(
        !report_validator.is_valid(&hidden_xa_report),
        "coverage schema must reject XA fields hidden behind a null XA image type"
    );
    assert_eq!(
        report
            .get("coverage_report_schema_version")
            .and_then(Value::as_str),
        Some("0.1.0")
    );
    assert_eq!(
        report.pointer("/counts/generated").and_then(Value::as_u64),
        Some(49)
    );
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        report
            .pointer("/coverage_matrix")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(49)
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    let us_multiframe_row = coverage_row(&report, "classic/us/multiframe_explicit_le");
    assert_eq!(us_multiframe_row["status"], "generated");
    assert_eq!(
        us_multiframe_row["us_image_type"],
        "ORIGINAL; PRIMARY; ABDOMINAL; 0001"
    );
    assert_eq!(us_multiframe_row["us_frame_increment_pointer"], "0018,1063");
    assert_eq!(us_multiframe_row["us_frame_time_ms"], 100.0);
    assert_eq!(
        us_multiframe_row["us_frame_relative_times_ms"],
        "0.0; 100.0; 200.0; 300.0"
    );
    assert_eq!(us_multiframe_row["us_frame_count"], 4);
    assert_eq!(us_multiframe_row["us_spatially_related_frames"], false);
    assert_eq!(us_multiframe_row["us_color_data_present"], false);
    assert_eq!(us_multiframe_row["us_region_calibrated"], false);
    assert_eq!(us_multiframe_row["us_lossy_image_compression"], "00");
    for pointer in [
        "/grouped_coverage/us_image_types/ORIGINAL; PRIMARY; ABDOMINAL; 0001",
        "/grouped_coverage/us_frame_increment_pointers/0018,1063",
        "/grouped_coverage/us_frame_times_ms/100.0",
        "/grouped_coverage/us_frame_counts/4",
        "/grouped_coverage/us_spatially_related_frames/false",
        "/grouped_coverage/us_color_data_present/false",
        "/grouped_coverage/us_region_calibrated/false",
        "/grouped_coverage/us_lossy_image_compressions/00",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    let empty_type2_row = coverage_row(&report, "metadata/sc/empty_type2_attributes");
    assert_eq!(
        empty_type2_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        empty_type2_row
            .get("metadata_empty_type2_attribute_count")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert!(
        empty_type2_row
            .get("metadata_empty_type2_attributes")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("0010,0010 PatientName PN VL=0"))
    );
    let native_row = coverage_row(&report, "metadata/sc/utf8_person_name");
    assert_eq!(
        native_row
            .get("metadata_specific_character_sets")
            .and_then(Value::as_str),
        Some("ISO_IR 192")
    );
    assert_eq!(
        native_row
            .get("metadata_person_name")
            .and_then(Value::as_str),
        Some("Wang^XiaoDong=王^小東")
    );
    assert_eq!(
        native_row
            .get("metadata_person_name_component_groups")
            .and_then(Value::as_str),
        Some("alphabetic:Wang^XiaoDong | ideographic:王^小東")
    );
    assert_eq!(
        native_row
            .get("metadata_person_name_component_group_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        native_row
            .get("metadata_person_name_encoded_sha256")
            .and_then(Value::as_str),
        Some("64a9d3d6b55142162489a8679e8643caa94efcff26dd30bf24650ac5186c1382")
    );
    assert_eq!(
        native_row
            .get("metadata_person_name_encoded_length_bytes")
            .and_then(Value::as_u64),
        Some(24)
    );
    assert_eq!(native_row.get("generation_backend_id"), Some(&Value::Null));
    assert_eq!(
        native_row.get("generation_backend_version"),
        Some(&Value::Null)
    );
    assert_eq!(
        native_row.get("generation_backend_determinism"),
        Some(&Value::Null)
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/rgb_planar0_explicit_le")
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/rgb_planar0_explicit_le")
            .get("modality")
            .and_then(Value::as_str),
        Some("XC")
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/rgb_planar0_explicit_le")
            .get("image_type")
            .and_then(Value::as_str),
        Some("ORIGINAL\\PRIMARY")
    );
    assert_eq!(
        coverage_row(&report, "classic/sc/rgb_planar1_explicit_le")
            .get("conversion_type")
            .and_then(Value::as_str),
        Some("SYN")
    );
    assert_eq!(
        coverage_row(
            &report,
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le"
        )
        .get("presentation_lut_shape")
        .and_then(Value::as_str),
        Some("INVERSE")
    );
    assert_eq!(
        coverage_row(
            &report,
            "classic/mg/for_processing_mono2_u16_12bit_implicit_le"
        )
        .get("presentation_lut_shape")
        .and_then(Value::as_str),
        Some("IDENTITY")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("window_center")
            .and_then(Value::as_str),
        Some("40")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("window_width")
            .and_then(Value::as_str),
        Some("400")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("kvp")
            .and_then(Value::as_str),
        Some("120")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("ct_acquisition_number")
            .and_then(Value::as_str),
        Some("1")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("ct_rescale_intercept")
            .and_then(Value::as_str),
        Some("-1024")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("ct_rescale_slope")
            .and_then(Value::as_str),
        Some("1")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("ct_rescale_type")
            .and_then(Value::as_str),
        Some("HU")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_scanning_sequence")
            .and_then(Value::as_str),
        Some("SE")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_sequence_variant")
            .and_then(Value::as_str),
        Some("NONE")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_acquisition_type")
            .and_then(Value::as_str),
        Some("2D")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_repetition_time")
            .and_then(Value::as_str),
        Some("500")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_echo_time")
            .and_then(Value::as_str),
        Some("20")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_echo_train_length")
            .and_then(Value::as_str),
        Some("1")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("mr_magnetic_field_strength")
            .and_then(Value::as_str),
        Some("1.5")
    );
    assert_eq!(
        coverage_row(&report, "classic/dx/display_shutter_mono2_u16_explicit_le")
            .get("display_shutter_shape")
            .and_then(Value::as_str),
        Some("RECTANGULAR")
    );
    assert_eq!(
        coverage_row(&report, "classic/dx/display_shutter_mono2_u16_explicit_le")
            .get("imager_pixel_spacing")
            .and_then(Value::as_str),
        Some("0.150\\0.150")
    );
    assert_eq!(
        coverage_row(&report, "classic/dx/display_shutter_mono2_u16_explicit_le")
            .get("display_shutter_presentation_value")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("body_part_examined")
            .and_then(Value::as_str),
        Some("CHEST")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("view_position")
            .and_then(Value::as_str),
        Some("PA")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("modality_lut_descriptor")
            .and_then(Value::as_str),
        Some("4\\0\\16")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("modality_lut_type")
            .and_then(Value::as_str),
        Some("US")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("modality_lut_data_value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("voi_lut_descriptor")
            .and_then(Value::as_str),
        Some("4\\0\\16")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("voi_lut_data_value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_rows")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_columns")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_type")
            .and_then(Value::as_str),
        Some("G")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_origin")
            .and_then(Value::as_str),
        Some("1\\1")
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_bits_allocated")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_bit_position")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        coverage_row(&report, "classic/cr/overlay_modality_voi_explicit_le")
            .get("overlay_data_value_length")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        coverage_row(&report, "classic/dx/display_shutter_mono2_u16_explicit_le")
            .get("body_part_examined")
            .and_then(Value::as_str),
        Some("CHEST")
    );
    assert_eq!(
        coverage_row(
            &report,
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le"
        )
        .get("body_part_examined")
        .and_then(Value::as_str),
        Some("BREAST")
    );
    assert_eq!(
        coverage_row(
            &report,
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le"
        )
        .get("view_position")
        .and_then(Value::as_str),
        Some("MLO")
    );
    assert_eq!(
        coverage_row(
            &report,
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le"
        )
        .get("imager_pixel_spacing")
        .and_then(Value::as_str),
        Some("0.070\\0.070")
    );
    assert_eq!(
        coverage_row(
            &report,
            "classic/mg/for_processing_mono2_u16_12bit_implicit_le"
        )
        .get("window_center"),
        Some(&Value::Null)
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("study_instance_uid_root")
            .and_then(Value::as_str),
        Some("2.25")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("series_instance_uid_root")
            .and_then(Value::as_str),
        Some("2.25")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("sop_instance_uid_root")
            .and_then(Value::as_str),
        Some("2.25")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("sop_class_name")
            .and_then(Value::as_str),
        Some("CT Image Storage")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .pointer("/geometry/spacing/0")
            .and_then(Value::as_f64),
        Some(0.625)
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("pixel_spacing")
            .and_then(Value::as_str),
        Some("0.625\\0.625")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("image_orientation_patient")
            .and_then(Value::as_str),
        Some("1\\0\\0\\0\\1\\0")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("image_position_patient")
            .and_then(Value::as_str),
        Some("-0.625\\-0.625\\0")
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("slice_thickness")
            .and_then(Value::as_str),
        Some("1")
    );
    let spatial_sort_row = coverage_row(
        &report,
        "geometry/ct/spatial_sort_conflicts_instance_number",
    );
    assert_eq!(
        spatial_sort_row
            .get("geometry_instance_number_state")
            .and_then(Value::as_str),
        Some("numeric")
    );
    assert_eq!(
        spatial_sort_row.get("geometry_adjacent_spacing_mm"),
        Some(&json!([5.0, 5.0]))
    );
    assert_eq!(
        spatial_sort_row
            .get("geometry_spacing_uniform")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        spatial_sort_row.get("geometry_gantry_detector_tilt_degrees"),
        Some(&Value::Null)
    );
    for field in [
        "series_organization_group_id",
        "study_series_count",
        "series_ordinal",
        "series_organization_instance_count",
        "shared_study_instance_uid_expected",
        "shared_frame_of_reference_uid_expected",
        "distinct_series_instance_uids_expected",
        "enhanced_mr_temporal_position_indices",
        "enhanced_mr_dimension_index_values",
        "enhanced_mr_frame_acquisition_numbers",
        "enhanced_mr_dimension_index_pointer",
        "enhanced_mr_functional_group_pointer",
        "enhanced_mr_temporal_position_time_offset_unit",
    ] {
        assert_eq!(
            spatial_sort_row.get(field),
            Some(&Value::Null),
            "single-series Phase 1 geometry should leave {field} unset"
        );
    }
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("spacing_between_slices")
            .and_then(Value::as_str),
        Some("5")
    );
    assert_eq!(
        coverage_row(&report, "classic/mr/multislice_oblique_explicit_le")
            .get("slice_location")
            .and_then(Value::as_str),
        Some("0")
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/palette_color_explicit_le")
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/palette_color_explicit_le")
            .get("modality")
            .and_then(Value::as_str),
        Some("XC")
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/palette_color_explicit_le")
            .get("profile_membership")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/palette_color_explicit_le")
            .get("transfer_syntax_name")
            .and_then(Value::as_str),
        Some("Explicit VR Little Endian")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/profiles/core")
            .and_then(Value::as_u64),
        Some(49)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/profile_memberships/core")
            .and_then(Value::as_u64),
        Some(49)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/modalities/XC")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/transfer_syntax_names/Explicit VR Little Endian")
            .and_then(Value::as_u64),
        Some(48)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/transfer_syntax_names/Implicit VR Little Endian")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_types/ORIGINAL\\PRIMARY")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_types/ORIGINAL\\PRIMARY\\AXIAL")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/conversion_types/SYN")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/presentation_lut_shapes/IDENTITY")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/presentation_lut_shapes/INVERSE")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/window_centers/40")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/window_centers/2048")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/window_widths/400")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/window_widths/4096")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/kvps/120")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_acquisition_numbers/1")
            .and_then(Value::as_u64),
        Some(15)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_rescale_intercepts/-1024")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_rescale_slopes/1")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_rescale_types/HU")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/pixel_spacings/0.625\\0.625")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/pixel_spacings/1.000\\1.000")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/imager_pixel_spacings/0.070\\0.070")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/imager_pixel_spacings/0.150\\0.150")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_orientations_patient/1\\0\\0\\0\\1\\0")
            .and_then(Value::as_u64),
        Some(18)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_positions_patient/0\\0\\0")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/slice_thicknesses/5")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/spacing_between_slices/5")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/slice_locations/10")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_scanning_sequences/SE")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_sequence_variants/NONE")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_acquisition_types/2D")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_repetition_times/500")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_echo_times/20")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_echo_train_lengths/1")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/mr_magnetic_field_strengths/1.5")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/modality_lut_descriptors/4\\0\\16")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/modality_lut_types/US")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/modality_lut_data_value_lengths/8")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/voi_lut_descriptors/4\\0\\16")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/voi_lut_data_value_lengths/8")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/overlay_geometries/2x2")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/overlay_types/G")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/overlay_origins/1\\1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/overlay_bits_allocated/1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/overlay_bit_positions/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/overlay_data_value_lengths/2")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/display_shutter_shapes/RECTANGULAR")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/display_shutter_presentation_values/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/body_parts_examined/CHEST")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/body_parts_examined/BREAST")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/view_positions/PA")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/view_positions/MLO")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/study_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(49)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/series_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(49)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(49)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_class_names/CT Image Storage")
            .and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_class_names/VL Photographic Image Storage")
            .and_then(Value::as_u64),
        Some(2)
    );
    for (pointer, expected) in [
        (
            "/grouped_coverage/metadata_specific_character_sets/ISO_IR 192",
            1,
        ),
        (
            "/grouped_coverage/metadata_person_names/Wang^XiaoDong=王^小東",
            1,
        ),
        (
            "/grouped_coverage/metadata_person_name_component_group_counts/2",
            1,
        ),
        (
            "/grouped_coverage/metadata_person_name_encoded_length_bytes/24",
            1,
        ),
    ] {
        assert_eq!(
            report.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "grouped metadata field {pointer}"
        );
    }

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Geometry Sorting Expectations"));
    assert!(markdown.contains("Instance Number state"));
    assert!(markdown.contains("Adjacent spacing (mm)"));
    assert!(markdown.contains("Uniform spacing"));
    assert!(markdown.contains("Gantry tilt (degrees)"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn markdown_report_renders_cross_series_organization_expectations() {
    let report = json!({
        "coverage_matrix": [{
            "case_id": "geometry/ct/multiseries_shared_frame_of_reference",
            "geometry_sort_basis": null,
            "series_organization_group_id": "shared-study-frame-of-reference",
            "study_series_count": 2,
            "series_ordinal": 1,
            "series_organization_instance_count": 3,
            "shared_study_instance_uid_expected": true,
            "shared_frame_of_reference_uid_expected": true,
            "distinct_series_instance_uids_expected": true
        }],
        "gaps": []
    });

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Cross-Series Organization Expectations"));
    assert!(markdown.contains(
        "| geometry/ct/multiseries_shared_frame_of_reference | shared-study-frame-of-reference | 2 | 1 | 3 | true | true | true |"
    ));
}

#[test]
fn markdown_report_renders_metadata_and_vr_expectations() {
    let report = json!({
        "coverage_matrix": [{
            "case_id": "metadata/sc/utf8_person_name",
            "metadata_specific_character_sets": "ISO_IR 192",
            "metadata_person_name": "Wang^XiaoDong=王^小東",
            "metadata_person_name_component_groups": "alphabetic:Wang^XiaoDong | ideographic:王^小東",
            "metadata_person_name_component_group_count": 2,
            "metadata_person_name_encoded_sha256": "6d3ef01e6f20a77c1457c4561427b2638e3da732e8f52ff7a18202ea004603b5",
            "metadata_person_name_encoded_length_bytes": 29
        }],
        "gaps": []
    });

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Metadata and VR Expectations"));
    assert!(markdown.contains("Specific Character Set"));
    assert!(markdown.contains("alphabetic:Wang^XiaoDong \\| ideographic:王^小東"));
    assert!(markdown.contains("6d3ef01e6f20a77c1457c4561427b2638e3da732e8f52ff7a18202ea004603b5"));
    assert!(markdown.contains("| 2 |"));
    assert!(markdown.contains("| 29 |"));
}

#[test]
fn report_command_writes_enhanced_mr_per_frame_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-enhanced-mr-per-frame-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    assert_eq!(
        coverage_row(&report, "enhanced/mr/multiframe_echo_perframe_explicit_le")
            .get("enhanced_mr_effective_echo_times")
            .and_then(Value::as_str),
        Some("12.5; 24.5")
    );
    let temporal_row = coverage_row(
        &report,
        "enhanced/mr/multiframe_temporal_position_explicit_le",
    );
    for (field, expected) in [
        ("enhanced_mr_temporal_position_time_offsets", "0.0; 1.5"),
        ("enhanced_mr_temporal_position_indices", "1; 2"),
        ("enhanced_mr_dimension_index_values", "1; 2"),
        ("enhanced_mr_frame_acquisition_numbers", "1; 2"),
        (
            "enhanced_mr_dimension_index_pointer",
            "TemporalPositionTimeOffset",
        ),
        (
            "enhanced_mr_functional_group_pointer",
            "TemporalPositionSequence",
        ),
        ("enhanced_mr_temporal_position_time_offset_unit", "seconds"),
    ] {
        assert_eq!(
            temporal_row.get(field).and_then(Value::as_str),
            Some(expected),
            "temporal report field {field}"
        );
    }
    let phase_row = coverage_row(
        &report,
        "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
    );
    assert_eq!(
        phase_row
            .get("enhanced_mr_velocity_encoding_minimum_value")
            .and_then(Value::as_str),
        Some("-150.0")
    );
    assert_eq!(
        phase_row
            .get("enhanced_mr_velocity_encoding_maximum_value")
            .and_then(Value::as_str),
        Some("150.0")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_mr_effective_echo_times/12.5; 24.5")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_mr_temporal_position_time_offsets/0.0; 1.5")
            .and_then(Value::as_u64),
        Some(1)
    );
    for (pointer, expected) in [
        (
            "/grouped_coverage/enhanced_mr_temporal_position_indices/1; 2",
            1,
        ),
        (
            "/grouped_coverage/enhanced_mr_dimension_index_values/1; 2",
            1,
        ),
        (
            "/grouped_coverage/enhanced_mr_frame_acquisition_numbers/1; 2",
            1,
        ),
        (
            "/grouped_coverage/enhanced_mr_dimension_index_pointers/TemporalPositionTimeOffset",
            1,
        ),
        (
            "/grouped_coverage/enhanced_mr_functional_group_pointers/TemporalPositionSequence",
            1,
        ),
        (
            "/grouped_coverage/enhanced_mr_temporal_position_time_offset_units/seconds",
            1,
        ),
    ] {
        assert_eq!(
            report.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "grouped temporal field {pointer}"
        );
    }
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_mr_velocity_encoding_minimum_values/-150.0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_mr_velocity_encoding_maximum_values/150.0")
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### Enhanced MR Effective Echo Times"));
    assert!(markdown.contains("| 12.5; 24.5 | 1 |"));
    assert!(markdown.contains("### Enhanced MR Temporal Position Time Offsets (seconds)"));
    assert!(markdown.contains("| 0.0; 1.5 | 1 |"));
    assert!(markdown.contains("## Enhanced MR Temporal Expectations"));
    assert!(markdown.contains("Time offsets (s)"));
    assert!(markdown.contains("| enhanced/mr/multiframe_temporal_position_explicit_le | 1; 2 | 1; 2 | 1; 2 | 0.0; 1.5 | TemporalPositionTimeOffset | TemporalPositionSequence |"));
    assert!(markdown.contains("### Enhanced MR Velocity Encoding Minimum Values"));
    assert!(markdown.contains("| -150.0 | 1 |"));
    assert!(markdown.contains("### Enhanced MR Velocity Encoding Maximum Values"));
    assert!(markdown.contains("| 150.0 | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_rejects_incomplete_or_inconsistent_enhanced_mr_temporal_contracts() {
    let out_dir = unique_temp_dir("report-enhanced-mr-temporal-malformed");
    generate_extended(&out_dir);
    let manifest_path = out_dir.join("manifest.json");
    let original: Value = serde_json::from_slice(
        &fs::read(&manifest_path).expect("generated manifest should be readable"),
    )
    .expect("generated manifest should be JSON");

    let cases = [
        (
            "/expected_semantics/temporal_position_indices",
            Value::Null,
            "temporal expected semantics must define an integer temporal_position_indices array",
        ),
        (
            "/expected_semantics/dimension_index_values",
            json!([1]),
            "must be non-empty arrays of equal length",
        ),
        (
            "/recipe/recipe_parameters/dimension_index/dimension_index_pointer",
            json!("EffectiveEchoTime"),
            "temporal dimension_index_pointer must be TemporalPositionTimeOffset",
        ),
        (
            "/expected_semantics/temporal_position_time_offset_unit",
            json!("milliseconds"),
            "temporal_position_time_offset_unit must be seconds",
        ),
    ];
    for (pointer, replacement, expected_error) in cases {
        let mut manifest = original.clone();
        let temporal_file = manifest
            .get_mut("files")
            .and_then(Value::as_array_mut)
            .expect("manifest files should be an array")
            .iter_mut()
            .find(|file| {
                file.get("case_id").and_then(Value::as_str)
                    == Some("enhanced/mr/multiframe_temporal_position_explicit_le")
            })
            .expect("temporal Enhanced MR file should be generated");
        if replacement.is_null() {
            temporal_file
                .pointer_mut("/expected_semantics")
                .and_then(Value::as_object_mut)
                .expect("temporal expected semantics should be an object")
                .remove("temporal_position_indices");
        } else {
            *temporal_file
                .pointer_mut(pointer)
                .unwrap_or_else(|| panic!("temporal manifest should contain {pointer}")) =
                replacement;
        }
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
        )
        .expect("malformed manifest fixture should be writable");

        let error = dicom_test_suite::build_coverage_report(&out_dir)
            .expect_err("malformed temporal report contract should be rejected")
            .to_string();
        assert!(
            error.contains(expected_error),
            "unexpected error for {pointer}: {error}"
        );
    }

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_enhanced_ct_concatenation_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-enhanced-ct-concat-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    assert_eq!(
        coverage_row(
            &report,
            "enhanced/ct/multiframe_shared_perframe_explicit_le"
        )
        .get("enhanced_ct_dimension_index_values")
        .and_then(Value::as_str),
        Some("1; 2")
    );
    assert_eq!(
        coverage_row(
            &report,
            "enhanced/ct/multiframe_shared_perframe_explicit_le"
        )
        .get("enhanced_ct_in_concatenation_number"),
        Some(&Value::Null)
    );

    let concat_part_1 = coverage_row_with_u64_field(
        &report,
        "enhanced/ct/concatenation_two_part_explicit_le",
        "enhanced_ct_in_concatenation_number",
        1,
    );
    assert_eq!(
        concat_part_1
            .get("enhanced_ct_dimension_index_values")
            .and_then(Value::as_str),
        Some("1")
    );
    assert_eq!(
        concat_part_1
            .get("enhanced_ct_in_concatenation_number")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        concat_part_1
            .get("enhanced_ct_in_concatenation_total_number")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        concat_part_1
            .get("enhanced_ct_concatenation_frame_offset_number")
            .and_then(Value::as_u64),
        Some(0)
    );

    let concat_part_2 = coverage_row_with_u64_field(
        &report,
        "enhanced/ct/concatenation_two_part_explicit_le",
        "enhanced_ct_in_concatenation_number",
        2,
    );
    assert_eq!(
        concat_part_2
            .get("enhanced_ct_dimension_index_values")
            .and_then(Value::as_str),
        Some("2")
    );
    assert_eq!(
        concat_part_2
            .get("enhanced_ct_in_concatenation_number")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        concat_part_2
            .get("enhanced_ct_in_concatenation_total_number")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        concat_part_2
            .get("enhanced_ct_concatenation_frame_offset_number")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_dimension_index_values/1; 2")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_dimension_index_values/1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_dimension_index_values/2")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_in_concatenation_numbers/1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_in_concatenation_numbers/2")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_in_concatenation_total_numbers/2")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_concatenation_frame_offset_numbers/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/enhanced_ct_concatenation_frame_offset_numbers/1")
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### Enhanced CT Dimension Index Values"));
    assert!(markdown.contains("| 1; 2 | 1 |"));
    assert!(markdown.contains("### Enhanced CT In-concatenation Numbers"));
    assert!(markdown.contains("| 1 | 1 |"));
    assert!(markdown.contains("### Enhanced CT In-concatenation Total Numbers"));
    assert!(markdown.contains("| 2 | 2 |"));
    assert!(markdown.contains("### Enhanced CT Concatenation Frame Offset Numbers"));
    assert!(markdown.contains("| 0 | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_segmentation_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-segmentation-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let wsi_tile_segmentation_generated =
        coverage_row(&report, "derived/seg/wsi_tile_reference")["status"] == "generated";
    assert_eq!(
        coverage_row(&report, "derived/seg/binary_multiframe_explicit_le")
            .get("segmentation_type")
            .and_then(Value::as_str),
        Some("BINARY")
    );
    let fractional_row = coverage_row(
        &report,
        "derived/seg/fractional_probability_multiframe_explicit_le",
    );
    assert_eq!(
        fractional_row
            .get("segmentation_type")
            .and_then(Value::as_str),
        Some("FRACTIONAL")
    );
    assert_eq!(
        fractional_row
            .get("segmentation_fractional_type")
            .and_then(Value::as_str),
        Some("PROBABILITY")
    );
    assert_eq!(
        fractional_row
            .get("segmentation_maximum_fractional_value")
            .and_then(Value::as_u64),
        Some(255)
    );
    assert_eq!(
        coverage_row(&report, "derived/seg/labelmap_multiframe_explicit_le")
            .get("segmentation_type")
            .and_then(Value::as_str),
        Some("LABELMAP")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/segmentation_types/BINARY")
            .and_then(Value::as_u64),
        Some(if cfg!(feature = "deflate") { 2 } else { 1 })
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/segmentation_types/FRACTIONAL")
            .and_then(Value::as_u64),
        Some(1 + u64::from(wsi_tile_segmentation_generated))
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/segmentation_types/LABELMAP")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/segmentation_fractional_types/PROBABILITY")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/segmentation_maximum_fractional_values/255")
            .and_then(Value::as_u64),
        Some(1 + u64::from(wsi_tile_segmentation_generated))
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### Segmentation Types"));
    assert!(markdown.contains(if cfg!(feature = "deflate") {
        "| BINARY | 2 |"
    } else {
        "| BINARY | 1 |"
    }));
    assert!(markdown.contains(&format!(
        "| FRACTIONAL | {} |",
        1 + u64::from(wsi_tile_segmentation_generated)
    )));
    assert!(markdown.contains("| LABELMAP | 1 |"));
    assert!(markdown.contains("### Segmentation Fractional Types"));
    assert_eq!(
        markdown.contains("| OCCUPANCY | 1 |"),
        wsi_tile_segmentation_generated
    );
    assert!(markdown.contains("| PROBABILITY | 1 |"));
    assert!(markdown.contains("### Segmentation Maximum Fractional Values"));
    assert!(markdown.contains(&format!(
        "| 255 | {} |",
        1 + u64::from(wsi_tile_segmentation_generated)
    )));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_gsps_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-gsps-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(
        &report,
        "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
    );
    assert_eq!(
        row.get("gsps_content_label").and_then(Value::as_str),
        Some("DTSGSPS")
    );
    assert_eq!(
        row.get("gsps_content_description").and_then(Value::as_str),
        Some("Synthetic CT window presentation state")
    );
    assert_eq!(
        row.get("gsps_presentation_size_mode")
            .and_then(Value::as_str),
        Some("SCALE TO FIT")
    );
    assert_eq!(
        row.get("gsps_presentation_pixel_aspect_ratio")
            .and_then(Value::as_str),
        Some("1\\1")
    );
    assert_eq!(
        row.get("gsps_window_center").and_then(Value::as_str),
        Some("350")
    );
    assert_eq!(
        row.get("gsps_window_width").and_then(Value::as_str),
        Some("1400")
    );
    assert_eq!(
        row.get("gsps_presentation_lut_shape")
            .and_then(Value::as_str),
        Some("IDENTITY")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/gsps_content_labels/DTSGSPS")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer(
                "/grouped_coverage/gsps_content_descriptions/Synthetic CT window presentation state"
            )
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/gsps_presentation_size_modes/SCALE TO FIT")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/gsps_presentation_pixel_aspect_ratios/1\\1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/gsps_window_centers/350")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/gsps_window_widths/1400")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/gsps_presentation_lut_shapes/IDENTITY")
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### GSPS Content Labels"));
    assert!(markdown.contains("| DTSGSPS | 1 |"));
    assert!(markdown.contains("### GSPS Content Descriptions"));
    assert!(markdown.contains("| Synthetic CT window presentation state | 1 |"));
    assert!(markdown.contains("### GSPS Presentation Size Modes"));
    assert!(markdown.contains("| SCALE TO FIT | 1 |"));
    assert!(markdown.contains("### GSPS Presentation Pixel Aspect Ratios"));
    assert!(markdown.contains("| 1\\1 | 1 |"));
    assert!(markdown.contains("### GSPS Window Centers"));
    assert!(markdown.contains("| 350 | 1 |"));
    assert!(markdown.contains("### GSPS Window Widths"));
    assert!(markdown.contains("| 1400 | 1 |"));
    assert!(markdown.contains("### GSPS Presentation LUT Shapes"));
    assert!(markdown.contains("| IDENTITY | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_color_softcopy_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-color-softcopy-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "derived/presentation-state/color_softcopy");
    assert_eq!(
        row.get("color_softcopy_presentation_state_kind")
            .and_then(Value::as_str),
        Some("Color Softcopy Presentation State")
    );
    assert_eq!(
        row.get("color_softcopy_sop_class_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.11.2")
    );
    assert_eq!(
        row.get("color_softcopy_source_topology")
            .and_then(Value::as_str),
        Some("same_study+different_series; 1 series/1 complete instance")
    );
    assert_eq!(
        row.get("color_softcopy_displayed_area")
            .and_then(Value::as_str),
        Some("global [1,1]-[2,2]; SCALE TO FIT; aspect 1\\1")
    );
    assert_eq!(
        row.get("color_softcopy_icc_profile_sha256")
            .and_then(Value::as_str),
        Some("8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef")
    );
    assert_eq!(
        row.get("color_softcopy_icc_profile_size_bytes")
            .and_then(Value::as_u64),
        Some(736)
    );
    assert_eq!(
        row.get("color_softcopy_icc_color_space")
            .and_then(Value::as_str),
        Some("SRGB")
    );
    assert_eq!(
        row.get("color_softcopy_optional_modules_absent")
            .and_then(Value::as_str),
        Some("shutter+graphic_annotation+graphic_layer+overlay+spatial_transform")
    );
    assert_eq!(
        row.get("color_softcopy_pixel_data_absent")
            .and_then(Value::as_bool),
        Some(true)
    );

    for (field, key) in [
        (
            "color_softcopy_presentation_state_kinds",
            "Color Softcopy Presentation State",
        ),
        (
            "color_softcopy_sop_class_uids",
            "1.2.840.10008.5.1.4.1.1.11.2",
        ),
        (
            "color_softcopy_source_topologies",
            "same_study+different_series; 1 series/1 complete instance",
        ),
        (
            "color_softcopy_displayed_areas",
            "global [1,1]-[2,2]; SCALE TO FIT; aspect 1\\1",
        ),
        (
            "color_softcopy_icc_profile_sha256_values",
            "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
        ),
        ("color_softcopy_icc_profile_size_byte_counts", "736"),
        ("color_softcopy_icc_color_spaces", "SRGB"),
        (
            "color_softcopy_optional_module_absence_sets",
            "shutter+graphic_annotation+graphic_layer+overlay+spatial_transform",
        ),
        ("color_softcopy_pixel_data_absent_states", "true"),
    ] {
        assert_eq!(
            report["grouped_coverage"][field][key].as_u64(),
            Some(1),
            "grouped Color Softcopy coverage must count {field}={key}"
        );
    }

    let report_schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json")
            .expect("coverage report schema should be readable"),
    )
    .expect("coverage report schema should be JSON");
    let report_validator =
        jsonschema::validator_for(&report_schema).expect("coverage schema should compile");
    assert!(
        report_validator.is_valid(&report),
        "Color Softcopy coverage report must match its schema"
    );
    let mut partial_report = report.clone();
    coverage_row_mut(
        &mut partial_report,
        "derived/presentation-state/color_softcopy",
    )["color_softcopy_displayed_area"] = Value::Null;
    assert!(
        !report_validator.is_valid(&partial_report),
        "coverage schema must reject a partial Color Softcopy contract"
    );
    let mut hidden_report = report.clone();
    coverage_row_mut(
        &mut hidden_report,
        "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
    )["color_softcopy_pixel_data_absent"] = Value::from(true);
    assert!(
        !report_validator.is_valid(&hidden_report),
        "coverage schema must reject Color Softcopy fields on other cases"
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");
    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("## Color Softcopy Presentation State Expectations"));
    assert!(markdown.contains("### Color Softcopy Source Topologies"));
    assert!(markdown.contains("### Color Softcopy Displayed Areas"));
    assert!(markdown.contains("### Color Softcopy ICC Profile SHA-256 Values"));
    assert!(markdown.contains("### Color Softcopy Optional Modules Absent"));
    assert!(markdown.contains("same_study+different_series; 1 series/1 complete instance"));
    assert!(markdown.contains("global [1,1]-[2,2]; SCALE TO FIT; aspect 1\\1"));
    assert!(
        markdown.contains("shutter+graphic_annotation+graphic_layer+overlay+spatial_transform")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_rt_dose_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-rt-dose-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "non-image/rt/dose_grid_u16_explicit_le");
    assert_eq!(row.get("rt_dose_units").and_then(Value::as_str), Some("GY"));
    assert_eq!(
        row.get("rt_dose_type").and_then(Value::as_str),
        Some("PHYSICAL")
    );
    assert_eq!(
        row.get("rt_dose_summation_type").and_then(Value::as_str),
        Some("RECORD")
    );
    assert_eq!(
        row.get("rt_dose_grid_scaling").and_then(Value::as_str),
        Some("0.001")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_dose_units/GY")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_dose_types/PHYSICAL")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_dose_summation_types/RECORD")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_dose_grid_scalings/0.001")
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### RT Dose Units"));
    assert!(markdown.contains("| GY | 1 |"));
    assert!(markdown.contains("### RT Dose Types"));
    assert!(markdown.contains("| PHYSICAL | 1 |"));
    assert!(markdown.contains("### RT Dose Summation Types"));
    assert!(markdown.contains("| RECORD | 1 |"));
    assert!(markdown.contains("### RT Dose Grid Scalings"));
    assert!(markdown.contains("| 0.001 | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_rt_structure_set_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-rt-structure-set-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "non-image/rt/structure_set_single_roi_explicit_le");
    assert_eq!(
        row.get("rt_structure_set_label").and_then(Value::as_str),
        Some("DTS_RTSTRUCT")
    );
    assert_eq!(
        row.get("rt_structure_set_roi_name").and_then(Value::as_str),
        Some("DTS_SYNTHETIC_ROI")
    );
    assert_eq!(
        row.get("rt_roi_generation_algorithm")
            .and_then(Value::as_str),
        Some("MANUAL")
    );
    assert_eq!(
        row.get("rt_contour_geometric_type").and_then(Value::as_str),
        Some("CLOSED_PLANAR")
    );
    assert_eq!(
        row.get("rt_contour_points").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        row.get("rt_roi_interpreted_type").and_then(Value::as_str),
        Some("ORGAN")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_structure_set_labels/DTS_RTSTRUCT")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_structure_set_roi_names/DTS_SYNTHETIC_ROI")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_roi_generation_algorithms/MANUAL")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_contour_geometric_types/CLOSED_PLANAR")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_contour_points/4")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rt_roi_interpreted_types/ORGAN")
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### RT Structure Set Labels"));
    assert!(markdown.contains("| DTS_RTSTRUCT | 1 |"));
    assert!(markdown.contains("### RT Structure Set ROI Names"));
    assert!(markdown.contains("| DTS_SYNTHETIC_ROI | 1 |"));
    assert!(markdown.contains("### RT ROI Generation Algorithms"));
    assert!(markdown.contains("| MANUAL | 1 |"));
    assert!(markdown.contains("### RT Contour Geometric Types"));
    assert!(markdown.contains("| CLOSED_PLANAR | 1 |"));
    assert!(markdown.contains("### RT Contour Points"));
    assert!(markdown.contains("| 4 | 1 |"));
    assert!(markdown.contains("### RT ROI Interpreted Types"));
    assert!(markdown.contains("| ORGAN | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_encapsulated_document_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-encapsulated-document-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(
        &report,
        "non-image/encapsulated-document/pdf_minimal_explicit_le",
    );
    assert_eq!(
        row.get("encapsulated_document_burned_in_annotation")
            .and_then(Value::as_str),
        Some("NO")
    );
    assert_eq!(
        row.get("encapsulated_document_recognizable_visual_features")
            .and_then(Value::as_str),
        Some("NO")
    );
    assert_eq!(
        row.get("encapsulated_document_title")
            .and_then(Value::as_str),
        Some("DTS Minimal Synthetic PDF")
    );
    assert_eq!(
        row.get("encapsulated_document_mime_type")
            .and_then(Value::as_str),
        Some("application/pdf")
    );
    let document_length = row
        .get("encapsulated_document_length")
        .and_then(Value::as_u64)
        .expect("Encapsulated PDF row should report document length");
    assert!(document_length > 0);
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_document_burned_in_annotations/NO")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_document_recognizable_visual_features/NO")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_document_titles/DTS Minimal Synthetic PDF")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_document_mime_types/application~1pdf")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer(&format!(
                "/grouped_coverage/encapsulated_document_lengths/{document_length}"
            ))
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### Encapsulated Document Burned In Annotations"));
    assert!(markdown.contains("| NO | 1 |"));
    assert!(markdown.contains("### Encapsulated Document Recognizable Visual Features"));
    assert!(markdown.contains("### Encapsulated Document Titles"));
    assert!(markdown.contains("| DTS Minimal Synthetic PDF | 1 |"));
    assert!(markdown.contains("### Encapsulated Document MIME Types"));
    assert!(markdown.contains("| application/pdf | 1 |"));
    assert!(markdown.contains("### Encapsulated Document Lengths"));
    assert!(markdown.contains(&format!("| {document_length} | 1 |")));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_rwvm_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-rwvm-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "derived/rwvm/linear_ct_mapping_explicit_le");
    assert_eq!(
        row.get("rwvm_content_label").and_then(Value::as_str),
        Some("DTSRWVM")
    );
    assert_eq!(
        row.get("rwvm_lut_label").and_then(Value::as_str),
        Some("DTS_HU")
    );
    assert_eq!(
        row.get("rwvm_first_value_mapped").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        row.get("rwvm_last_value_mapped").and_then(Value::as_u64),
        Some(700)
    );
    assert_eq!(
        row.get("rwvm_intercept").and_then(Value::as_str),
        Some("-1024.0")
    );
    assert_eq!(row.get("rwvm_slope").and_then(Value::as_str), Some("1.0"));
    assert_eq!(
        row.get("rwvm_units_code_value").and_then(Value::as_str),
        Some("HU")
    );
    assert_eq!(
        row.get("rwvm_units_coding_scheme_designator")
            .and_then(Value::as_str),
        Some("UCUM")
    );
    assert_eq!(
        row.get("rwvm_units_code_meaning").and_then(Value::as_str),
        Some("Hounsfield unit")
    );
    assert_eq!(
        row.get("rwvm_referenced_frame_numbers")
            .and_then(Value::as_str),
        Some("1; 2")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_content_labels/DTSRWVM")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        report
            .pointer("/grouped_coverage/rwvm_content_labels/DTSGSPS")
            .is_none(),
        "RWVM content labels must not include GSPS content labels"
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_lut_labels/DTS_HU")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_first_values_mapped/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_last_values_mapped/700")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_intercepts/-1024.0")
            .and_then(Value::as_u64),
        Some(1)
    );
    let parametric_maps_generated = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix")
        .iter()
        .filter(|row| {
            row["status"] == "generated"
                && matches!(
                    row["case_id"].as_str(),
                    Some("derived/parametric-map/float32_ct_derived_explicit_le")
                        | Some("derived/parametric-map/float64_ct_derived_explicit_le")
                )
        })
        .count();
    assert!(matches!(parametric_maps_generated, 0 | 2));
    if parametric_maps_generated == 2 {
        let float64_row = report["coverage_matrix"]
            .as_array()
            .expect("coverage matrix")
            .iter()
            .find(|row| {
                row["case_id"].as_str()
                    == Some("derived/parametric-map/float64_ct_derived_explicit_le")
            })
            .expect("generated float64 Parametric Map coverage row");
        assert_eq!(float64_row["bits_allocated"], 64);
        assert_eq!(float64_row["pixel_data_vr"], "OD");
        assert_eq!(float64_row["generation_backend_id"], "highdicom_pydicom");
        assert_eq!(
            report
                .pointer("/grouped_coverage/bits_allocated/64")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/pixel_data_vrs/OD")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/known_stressors/double_float_pixel_data")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/known_stressors/parametric_map_storage")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/generation_backends/highdicom_pydicom")
                .and_then(Value::as_u64),
            Some(5)
        );
    }
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_slopes/1.0")
            .and_then(Value::as_u64),
        Some(1 + parametric_maps_generated as u64)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_units_code_values/HU")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_units_coding_scheme_designators/UCUM")
            .and_then(Value::as_u64),
        Some(1 + parametric_maps_generated as u64)
    );
    if parametric_maps_generated > 0 {
        assert_eq!(
            report
                .pointer("/grouped_coverage/rwvm_units_code_values/1")
                .and_then(Value::as_u64),
            Some(parametric_maps_generated as u64)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/rwvm_units_code_meanings/no units")
                .and_then(Value::as_u64),
            Some(parametric_maps_generated as u64)
        );
    }
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_units_code_meanings/Hounsfield unit")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_referenced_frame_numbers/1; 2")
            .and_then(Value::as_u64),
        Some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### RWVM Content Labels"));
    assert!(markdown.contains("| DTSRWVM | 1 |"));
    assert!(markdown.contains("### RWVM LUT Labels"));
    assert!(markdown.contains("| DTS_HU | 1 |"));
    assert!(markdown.contains("### RWVM First Values Mapped"));
    assert!(markdown.contains("| 0 | 1 |"));
    assert!(markdown.contains("### RWVM Last Values Mapped"));
    assert!(markdown.contains("| 700 | 1 |"));
    assert!(markdown.contains("### RWVM Intercepts"));
    assert!(markdown.contains("| -1024.0 | 1 |"));
    assert!(markdown.contains("### RWVM Slopes"));
    assert!(markdown.contains(&format!("| 1.0 | {} |", 1 + parametric_maps_generated)));
    assert!(markdown.contains("### RWVM Units Code Values"));
    assert!(markdown.contains("| HU | 1 |"));
    assert!(markdown.contains("### RWVM Units Coding Scheme Designators"));
    assert!(markdown.contains(&format!("| UCUM | {} |", 1 + parametric_maps_generated)));
    assert!(markdown.contains("### RWVM Units Code Meanings"));
    assert!(markdown.contains("| Hounsfield unit | 1 |"));
    assert!(markdown.contains("### RWVM Referenced Frame Numbers"));
    assert!(markdown.contains("| 1; 2 | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_structured_report_content_coverage_for_extended_root() {
    let out_dir = unique_temp_dir("report-structured-report-content-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let basic_text_row = coverage_row(&report, "derived/sr/basic_text_observation_explicit_le");
    assert_eq!(
        basic_text_row
            .get("sr_completion_flag")
            .and_then(Value::as_str),
        Some("COMPLETE")
    );
    assert_eq!(
        basic_text_row
            .get("sr_verification_flag")
            .and_then(Value::as_str),
        Some("UNVERIFIED")
    );
    assert_eq!(
        basic_text_row
            .get("sr_root_value_type")
            .and_then(Value::as_str),
        Some("CONTAINER")
    );
    assert_eq!(
        basic_text_row
            .get("sr_root_continuity_of_content")
            .and_then(Value::as_str),
        Some("SEPARATE")
    );
    assert_eq!(
        basic_text_row
            .get("sr_content_sequence_items")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        basic_text_row
            .get("sr_observation_text")
            .and_then(Value::as_str),
        Some("Synthetic Basic Text SR observation for Enhanced CT source images.")
    );
    assert!(
        basic_text_row
            .get("sr_measurement_numeric_value")
            .is_some_and(Value::is_null)
    );

    let comprehensive_row =
        coverage_row(&report, "derived/sr/comprehensive_measurement_explicit_le");
    assert_eq!(
        comprehensive_row
            .get("sr_content_sequence_items")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        comprehensive_row
            .get("sr_measurement_numeric_value")
            .and_then(Value::as_str),
        Some("12.5")
    );
    assert!(
        comprehensive_row
            .get("sr_observation_text")
            .is_some_and(Value::is_null)
    );

    let kos_row = coverage_row(&report, "derived/sr/key_object_selection_explicit_le");
    assert_eq!(
        kos_row
            .get("sr_content_sequence_items")
            .and_then(Value::as_u64),
        Some(2)
    );
    let tid1500_row = coverage_row(&report, "derived/sr/tid1500_ct_measurement_report");
    let scoord3d_row = coverage_row(&report, "derived/sr/comprehensive3d_scoord3d");
    let tid1500_generated = tid1500_row["status"] == "generated";
    let scoord3d_generated = scoord3d_row["status"] == "generated";
    assert_eq!(tid1500_generated, scoord3d_generated);
    for (row, expected_value) in [(tid1500_row, "5.625"), (scoord3d_row, "2.5")] {
        if tid1500_generated {
            assert_eq!(
                row.get("sr_content_sequence_items").and_then(Value::as_u64),
                Some(8)
            );
            assert_eq!(
                row.get("sr_measurement_numeric_value")
                    .and_then(Value::as_str),
                Some(expected_value)
            );
        } else {
            assert_eq!(row["status"], "unavailable");
            assert!(row["sr_content_sequence_items"].is_null());
            assert!(row["sr_measurement_numeric_value"].is_null());
        }
    }
    let optional_sr_count = if tid1500_generated { 2 } else { 0 };
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_completion_flags/COMPLETE")
            .and_then(Value::as_u64),
        Some(3 + optional_sr_count)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_verification_flags/UNVERIFIED")
            .and_then(Value::as_u64),
        Some(3 + optional_sr_count)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_root_value_types/CONTAINER")
            .and_then(Value::as_u64),
        Some(3 + optional_sr_count)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_root_continuity_of_content/SEPARATE")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_root_continuity_of_content/CONTINUOUS")
            .and_then(Value::as_u64),
        tid1500_generated.then_some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_content_sequence_item_counts/1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_content_sequence_item_counts/2")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_content_sequence_item_counts/8")
            .and_then(Value::as_u64),
        tid1500_generated.then_some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_measurement_numeric_values/2.5")
            .and_then(Value::as_u64),
        scoord3d_generated.then_some(1)
    );
    assert_eq!(
        report
            .pointer(
                "/grouped_coverage/sr_observation_texts/Synthetic Basic Text SR observation for Enhanced CT source images."
            )
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_measurement_numeric_values/12.5")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_measurement_numeric_values/5.625")
            .and_then(Value::as_u64),
        tid1500_generated.then_some(1)
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report markdown command must run");

    assert!(
        markdown_output.status.success(),
        "markdown report should accept generated output: {}",
        String::from_utf8_lossy(&markdown_output.stderr)
    );
    let markdown =
        String::from_utf8(markdown_output.stdout).expect("markdown stdout should be UTF-8");
    assert!(markdown.contains("### SR Completion Flags"));
    assert!(markdown.contains(&format!("| COMPLETE | {} |", 3 + optional_sr_count)));
    assert!(markdown.contains("### SR Verification Flags"));
    assert!(markdown.contains(&format!("| UNVERIFIED | {} |", 3 + optional_sr_count)));
    assert!(markdown.contains("### SR Root Value Types"));
    assert!(markdown.contains(&format!("| CONTAINER | {} |", 3 + optional_sr_count)));
    assert!(markdown.contains("### SR Root Continuity Of Content"));
    assert!(markdown.contains("| SEPARATE | 3 |"));
    assert_eq!(markdown.contains("| CONTINUOUS | 2 |"), tid1500_generated);
    assert!(markdown.contains("### SR Content Sequence Item Counts"));
    assert!(markdown.contains("| 2 | 2 |"));
    if tid1500_generated {
        assert!(markdown.contains("| 8 | 2 |"));
    }
    assert!(markdown.contains("### SR Observation Texts"));
    assert!(
        markdown
            .contains("| Synthetic Basic Text SR observation for Enhanced CT source images. | 1 |")
    );
    assert!(markdown.contains("### SR Measurement Numeric Values"));
    assert!(markdown.contains("| 12.5 | 1 |"));
    assert_eq!(markdown.contains("| 5.625 | 1 |"), tid1500_generated);
    assert_eq!(markdown.contains("| 2.5 | 1 |"), scoord3d_generated);

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_markdown_coverage_for_core_root() {
    let out_dir = unique_temp_dir("report-core-markdown");
    generate_core(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("report stdout should be UTF-8");
    assert!(stdout.starts_with("# DICOM Test Suite Coverage Report"));
    assert!(stdout.contains("| generated | 49 |"));
    assert!(stdout.contains("| planned | 0 |"));
    assert!(stdout.contains("### Profile Memberships"));
    assert!(stdout.contains("| core | 49 |"));
    assert!(stdout.contains("### Transfer Syntax Names"));
    assert!(stdout.contains("| Explicit VR Little Endian | 48 |"));
    assert!(stdout.contains("| Implicit VR Little Endian | 1 |"));
    assert!(stdout.contains("### SOP Class Names"));
    assert!(stdout.contains("| CT Image Storage | 17 |"));
    assert!(stdout.contains("| VL Photographic Image Storage | 2 |"));
    assert!(stdout.contains("### Image Types"));
    assert!(stdout.contains("### Conversion Types"));
    assert!(stdout.contains("### Presentation LUT Shapes"));
    assert!(stdout.contains("| IDENTITY | 2 |"));
    assert!(stdout.contains("| INVERSE | 1 |"));
    assert!(stdout.contains("### Window Centers"));
    assert!(stdout.contains("| 40 | 17 |"));
    assert!(stdout.contains("| 2048 | 2 |"));
    assert!(stdout.contains("### Window Widths"));
    assert!(stdout.contains("| 400 | 17 |"));
    assert!(stdout.contains("| 4096 | 2 |"));
    assert!(stdout.contains("### KVPs"));
    assert!(stdout.contains("| 120 | 17 |"));
    assert!(stdout.contains("### CT Acquisition Numbers"));
    assert!(stdout.contains("| 1 | 15 |"));
    assert!(stdout.contains("| 2 | 2 |"));
    assert!(stdout.contains("### CT Rescale Intercepts"));
    assert!(stdout.contains("| -1024 | 17 |"));
    assert!(stdout.contains("### CT Rescale Slopes"));
    assert!(stdout.contains("### CT Rescale Types"));
    assert!(stdout.contains("| HU | 17 |"));
    assert!(stdout.contains("### Pixel Spacings"));
    assert!(stdout.contains("| 0.625\\0.625 | 17 |"));
    assert!(stdout.contains("| 1.000\\1.000 | 3 |"));
    assert!(stdout.contains("### Imager Pixel Spacings"));
    assert!(stdout.contains("| 0.070\\0.070 | 2 |"));
    assert!(stdout.contains("| 0.150\\0.150 | 1 |"));
    assert!(stdout.contains("### Image Orientations Patient"));
    assert!(stdout.contains("| 1\\0\\0\\0\\1\\0 | 18 |"));
    assert!(stdout.contains("### Image Positions Patient"));
    assert!(stdout.contains("| 0\\0\\0 | 8 |"));
    assert!(stdout.contains("### Slice Thicknesses"));
    assert!(stdout.contains("| 5 | 16 |"));
    assert!(stdout.contains("### Spacing Between Slices"));
    assert!(stdout.contains("| 5 | 16 |"));
    assert!(stdout.contains("### Slice Locations"));
    assert!(stdout.contains("| 10 | 1 |"));
    assert!(stdout.contains("### MR Scanning Sequences"));
    assert!(stdout.contains("| SE | 3 |"));
    assert!(stdout.contains("### MR Sequence Variants"));
    assert!(stdout.contains("| NONE | 3 |"));
    assert!(stdout.contains("### MR Acquisition Types"));
    assert!(stdout.contains("| 2D | 3 |"));
    assert!(stdout.contains("### MR Repetition Times"));
    assert!(stdout.contains("| 500 | 3 |"));
    assert!(stdout.contains("### MR Echo Times"));
    assert!(stdout.contains("| 20 | 3 |"));
    assert!(stdout.contains("### MR Echo Train Lengths"));
    assert!(stdout.contains("| 1 | 3 |"));
    assert!(stdout.contains("### MR Magnetic Field Strengths"));
    assert!(stdout.contains("| 1.5 | 3 |"));
    assert!(stdout.contains("### Modality LUT Descriptors"));
    assert!(stdout.contains("| 4\\0\\16 | 1 |"));
    assert!(stdout.contains("### Modality LUT Types"));
    assert!(stdout.contains("| US | 1 |"));
    assert!(stdout.contains("### Modality LUT Data Value Lengths"));
    assert!(stdout.contains("| 8 | 1 |"));
    assert!(stdout.contains("### VOI LUT Descriptors"));
    assert!(stdout.contains("### VOI LUT Data Value Lengths"));
    assert!(stdout.contains("### Overlay Geometries"));
    assert!(stdout.contains("| 2x2 | 1 |"));
    assert!(stdout.contains("### Overlay Types"));
    assert!(stdout.contains("| G | 1 |"));
    assert!(stdout.contains("### Overlay Origins"));
    assert!(stdout.contains("| 1\\1 | 1 |"));
    assert!(stdout.contains("### Overlay Bits Allocated"));
    assert!(stdout.contains("| 1 | 1 |"));
    assert!(stdout.contains("### Overlay Bit Positions"));
    assert!(stdout.contains("| 0 | 1 |"));
    assert!(stdout.contains("### Overlay Data Value Lengths"));
    assert!(stdout.contains("| 2 | 1 |"));
    assert!(stdout.contains("### Display Shutter Shapes"));
    assert!(stdout.contains("| RECTANGULAR | 1 |"));
    assert!(stdout.contains("### Display Shutter Presentation Values"));
    assert!(stdout.contains("| 0 | 1 |"));
    assert!(stdout.contains("### Body Parts Examined"));
    assert!(stdout.contains("| CHEST | 2 |"));
    assert!(stdout.contains("| BREAST | 2 |"));
    assert!(stdout.contains("### View Positions"));
    assert!(stdout.contains("| PA | 1 |"));
    assert!(stdout.contains("| MLO | 2 |"));
    assert!(stdout.contains("### Study Instance UID Roots"));
    assert!(stdout.contains("### Series Instance UID Roots"));
    assert!(stdout.contains("### SOP Instance UID Roots"));
    assert!(stdout.contains("| 2.25 | 49 |"));
    assert!(stdout.contains("## Ultrasound Multi-frame Expectations"));
    assert!(stdout.contains("classic/us/multiframe_explicit_le"));
    assert!(stdout.contains("0.0; 100.0; 200.0; 300.0"));
    assert!(stdout.contains("### US Frame Increment Pointers"));
    assert!(stdout.contains("### US Lossy Image Compression History"));
    assert!(stdout.contains("### Derived Reference SOP Instance UID Roots"));
    assert!(stdout.contains("## Gaps"));
    assert!(
        stdout.contains("| classic/ct/mono2_i16_rescale_12bit_explicit_le | generated | core |")
    );
    assert!(stdout.contains("| vl/photo/rgb_planar0_explicit_le | generated | core |"));
    assert!(stdout.contains("| vl/photo/palette_color_explicit_le | generated | core |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_counts_generated_rgb_rle_lossless_row() {
    let out_dir = unique_temp_dir("report-rgb-rle-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "classic/sc/rgb_planar0_rle_lossless");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.5")
    );
    assert_eq!(
        row.get("transfer_syntax_name").and_then(Value::as_str),
        Some("RLE Lossless")
    );
    assert_eq!(
        row.get("codec_family").and_then(Value::as_str),
        Some("RLE Lossless")
    );
    assert_eq!(
        row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(row.get("pixel_data_vr").and_then(Value::as_str), Some("OB"));
    assert_eq!(
        row.get("pixel_data_layout").and_then(Value::as_str),
        Some("encapsulated")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_families/RLE Lossless")
            .and_then(Value::as_u64),
        Some(58)
    );
    let mono1_row = coverage_row(&report, "classic/sc/mono1_u8_rle_lossless");
    assert_eq!(
        mono1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    let mono1_odd_row = coverage_row(&report, "classic/sc/mono1_u8_odd_fragment_rle_lossless");
    assert_eq!(
        mono1_odd_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_odd_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_odd_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_odd_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        mono1_odd_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(2)
    );
    let mono1_u16_row = coverage_row(&report, "classic/sc/mono1_u16_rle_lossless");
    assert_eq!(
        mono1_u16_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_u16_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_u16_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(mono1_u16_row.get("bits").and_then(Value::as_u64), Some(16));
    let signed_row = coverage_row(&report, "classic/sc/mono2_i16_rle_lossless");
    assert_eq!(
        signed_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    let mono1_signed_row = coverage_row(&report, "classic/sc/mono1_i16_rle_lossless");
    assert_eq!(
        mono1_signed_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let tiny_row = coverage_row(&report, "classic/sc/mono2_u16_tiny_1x1_rle_lossless");
    assert_eq!(
        tiny_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        tiny_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        tiny_row.pointer("/geometry/rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        tiny_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(1)
    );
    let mono1_tiny_row = coverage_row(&report, "classic/sc/mono1_u16_tiny_1x1_rle_lossless");
    assert_eq!(
        mono1_tiny_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_tiny_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_tiny_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_tiny_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        mono1_tiny_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(1)
    );
    let signed_tiny_row = coverage_row(&report, "classic/sc/mono2_i16_tiny_1x1_rle_lossless");
    assert_eq!(
        signed_tiny_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_tiny_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_tiny_row
            .get("pixel_representation")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        signed_tiny_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        signed_tiny_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(1)
    );
    let mono1_signed_tiny_row = coverage_row(&report, "classic/sc/mono1_i16_tiny_1x1_rle_lossless");
    assert_eq!(
        mono1_signed_tiny_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_tiny_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_tiny_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_tiny_row
            .get("pixel_representation")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        mono1_signed_tiny_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        mono1_signed_tiny_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(1)
    );
    let padding_row = coverage_row(&report, "classic/sc/mono2_u16_padding_rle_lossless");
    assert_eq!(
        padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        padding_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        padding_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(padding_row.get("bits").and_then(Value::as_u64), Some(16));
    let u8_padding_row = coverage_row(&report, "classic/sc/mono2_u8_padding_rle_lossless");
    assert_eq!(
        u8_padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        u8_padding_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        u8_padding_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(u8_padding_row.get("bits").and_then(Value::as_u64), Some(8));
    let mono1_u8_padding_row = coverage_row(&report, "classic/sc/mono1_u8_padding_rle_lossless");
    assert_eq!(
        mono1_u8_padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_u8_padding_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_u8_padding_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_u8_padding_row.get("bits").and_then(Value::as_u64),
        Some(8)
    );
    let u8_padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono2_u8_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        u8_padding_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        u8_padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        u8_padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        u8_padding_multiframe_row
            .get("bits")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        u8_padding_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let mono1_u8_padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono1_u8_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        mono1_u8_padding_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_u8_padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_u8_padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_u8_padding_multiframe_row
            .get("bits")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        mono1_u8_padding_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let mono1_padding_row = coverage_row(&report, "classic/sc/mono1_u16_padding_rle_lossless");
    assert_eq!(
        mono1_padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_padding_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_padding_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_padding_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono2_u16_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        padding_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        padding_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        padding_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let mono1_padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono1_u16_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        mono1_padding_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_padding_multiframe_row
            .get("bits")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_padding_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let signed_padding_row = coverage_row(&report, "classic/sc/mono2_i16_padding_rle_lossless");
    assert_eq!(
        signed_padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_padding_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_padding_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        signed_padding_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let mono1_signed_padding_row =
        coverage_row(&report, "classic/sc/mono1_i16_padding_rle_lossless");
    assert_eq!(
        mono1_signed_padding_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_padding_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_padding_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_padding_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let mono1_signed_padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono1_i16_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        mono1_signed_padding_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_padding_multiframe_row
            .get("bits")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_signed_padding_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let signed_padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono2_i16_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        signed_padding_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        signed_padding_multiframe_row
            .get("bits")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        signed_padding_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let odd_3x3_row = coverage_row(&report, "classic/sc/mono2_u16_odd_3x3_rle_lossless");
    assert_eq!(
        odd_3x3_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        odd_3x3_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        odd_3x3_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        odd_3x3_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let mono1_odd_3x3_row = coverage_row(&report, "classic/sc/mono1_u16_odd_3x3_rle_lossless");
    assert_eq!(
        mono1_odd_3x3_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_odd_3x3_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_odd_3x3_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_odd_3x3_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_odd_3x3_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        mono1_odd_3x3_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let signed_odd_3x3_row = coverage_row(&report, "classic/sc/mono2_i16_odd_3x3_rle_lossless");
    assert_eq!(
        signed_odd_3x3_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_odd_3x3_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_odd_3x3_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        signed_odd_3x3_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        signed_odd_3x3_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        signed_odd_3x3_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let mono1_signed_odd_3x3_row =
        coverage_row(&report, "classic/sc/mono1_i16_odd_3x3_rle_lossless");
    assert_eq!(
        mono1_signed_odd_3x3_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_odd_3x3_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_odd_3x3_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_odd_3x3_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_signed_odd_3x3_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        mono1_signed_odd_3x3_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let rect_row = coverage_row(&report, "classic/sc/mono2_u16_rect_2x3_rle_lossless");
    assert_eq!(
        rect_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        rect_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rect_row.pointer("/geometry/rows").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rect_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let mono1_rect_row = coverage_row(&report, "classic/sc/mono1_u16_rect_2x3_rle_lossless");
    assert_eq!(
        mono1_rect_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_rect_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_rect_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_rect_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        mono1_rect_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let signed_rect_row = coverage_row(&report, "classic/sc/mono2_i16_rect_2x3_rle_lossless");
    assert_eq!(
        signed_rect_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_rect_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_rect_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        signed_rect_row
            .get("pixel_representation")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        signed_rect_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        signed_rect_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let mono1_signed_rect_row = coverage_row(&report, "classic/sc/mono1_i16_rect_2x3_rle_lossless");
    assert_eq!(
        mono1_signed_rect_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_rect_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_rect_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_rect_row
            .get("pixel_representation")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        mono1_signed_rect_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        mono1_signed_rect_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let planar1_row = coverage_row(&report, "classic/sc/rgb_planar1_rle_lossless");
    assert_eq!(
        planar1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        planar1_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let rgb_multiframe_row =
        coverage_row(&report, "classic/sc/rgb_planar0_multiframe_rle_lossless");
    assert_eq!(
        rgb_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        rgb_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rgb_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        rgb_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let rgb_planar1_multiframe_row =
        coverage_row(&report, "classic/sc/rgb_planar1_multiframe_rle_lossless");
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let ybr_row = coverage_row(&report, "classic/sc/ybr_full_planar0_rle_lossless");
    assert_eq!(
        ybr_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        ybr_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL RLE report row should retain YBR_FULL pixel stressor"
    );
    let ybr_planar1_row = coverage_row(&report, "classic/sc/ybr_full_planar1_rle_lossless");
    assert_eq!(
        ybr_planar1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_planar1_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        ybr_planar1_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL planar-1 RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL planar-1 RLE report row should retain YBR_FULL pixel stressor"
    );
    let ybr_multiframe_row = coverage_row(
        &report,
        "classic/sc/ybr_full_planar0_multiframe_rle_lossless",
    );
    assert_eq!(
        ybr_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        ybr_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("YBR_FULL")
    );
    assert_eq!(
        ybr_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        ybr_multiframe_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL multi-frame RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL multi-frame RLE report row should retain YBR_FULL pixel stressor"
    );
    let ybr_planar1_multiframe_row = coverage_row(
        &report,
        "classic/sc/ybr_full_planar1_multiframe_rle_lossless",
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("YBR_FULL")
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        ybr_planar1_multiframe_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL planar-1 multi-frame RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL planar-1 multi-frame RLE report row should retain YBR_FULL pixel stressor"
    );
    let palette_row = coverage_row(&report, "classic/sc/palette_color_u8_rle_lossless");
    assert_eq!(
        palette_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        palette_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        palette_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("PALETTE COLOR RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("palette_color_pixels")),
        "PALETTE COLOR RLE report row should retain palette color stressor"
    );
    let palette_multiframe_row = coverage_row(
        &report,
        "classic/sc/palette_color_u8_multiframe_rle_lossless",
    );
    assert_eq!(
        palette_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        palette_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        palette_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("PALETTE COLOR")
    );
    assert_eq!(
        palette_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        palette_multiframe_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("PALETTE COLOR multi-frame RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("palette_color_pixels")),
        "PALETTE COLOR multi-frame RLE report row should retain palette color stressor"
    );
    let multiframe_row = coverage_row(&report, "classic/sc/mono2_u8_multiframe_rle_lossless");
    assert_eq!(
        multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        multiframe_row
            .get("basic_offset_table")
            .and_then(Value::as_str),
        Some("empty")
    );
    assert_eq!(
        multiframe_row
            .get("encapsulated_fragment_layout")
            .and_then(Value::as_str),
        Some("single_fragment_per_frame")
    );
    let mono1_multiframe_row = coverage_row(&report, "classic/sc/mono1_u8_multiframe_rle_lossless");
    assert_eq!(
        mono1_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let u16_multiframe_row = coverage_row(&report, "classic/sc/mono2_u16_multiframe_rle_lossless");
    assert_eq!(
        u16_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        u16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        u16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let mono1_u16_multiframe_row =
        coverage_row(&report, "classic/sc/mono1_u16_multiframe_rle_lossless");
    assert_eq!(
        mono1_u16_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_u16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_u16_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_u16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_u16_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let i16_multiframe_row = coverage_row(&report, "classic/sc/mono2_i16_multiframe_rle_lossless");
    assert_eq!(
        i16_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        i16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        i16_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        i16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        i16_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let mono1_i16_multiframe_row =
        coverage_row(&report, "classic/sc/mono1_i16_multiframe_rle_lossless");
    assert_eq!(
        mono1_i16_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_i16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_i16_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_i16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_i16_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let odd_fragment_row = coverage_row(&report, "classic/sc/mono2_u8_odd_fragment_rle_lossless");
    assert_eq!(
        odd_fragment_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        odd_fragment_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let ct_row = coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_rle_lossless");
    assert_eq!(
        ct_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ct_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mr_row = coverage_row(&report, "classic/mr/mono2_u16_rle_lossless");
    assert_eq!(
        mr_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mr_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let cr_row = coverage_row(&report, "classic/cr/overlay_modality_voi_rle_lossless");
    assert_eq!(
        cr_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        cr_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let dx_row = coverage_row(&report, "classic/dx/display_shutter_mono2_u16_rle_lossless");
    assert_eq!(
        dx_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        dx_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mg_row = coverage_row(
        &report,
        "classic/mg/for_presentation_mono1_u16_12bit_rle_lossless",
    );
    assert_eq!(
        mg_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mg_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mg_processing_row = coverage_row(
        &report,
        "classic/mg/for_processing_mono2_u16_12bit_rle_lossless",
    );
    assert_eq!(
        mg_processing_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mg_processing_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let us_row = coverage_row(&report, "classic/us/mono2_u8_rle_lossless");
    assert_eq!(
        us_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        us_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        us_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("US RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ultrasound_image_storage")),
        "US RLE report row should retain Ultrasound Image Storage stressor"
    );
    let vl_photo_row = coverage_row(&report, "vl/photo/rgb_planar0_rle_lossless");
    assert_eq!(
        vl_photo_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        vl_photo_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        vl_photo_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("VL Photographic RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("vl_photographic_image_storage")),
        "VL Photographic RLE report row should retain VL Photographic Image Storage stressor"
    );
    let vl_photo_planar1_row = coverage_row(&report, "vl/photo/rgb_planar1_rle_lossless");
    assert_eq!(
        vl_photo_planar1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        vl_photo_planar1_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        vl_photo_planar1_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("VL Photographic planar-1 RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("vl_rgb_pixels")),
        "VL Photographic planar-1 RLE report row should retain VL RGB pixel stressor"
    );
    let vl_photo_palette_row = coverage_row(&report, "vl/photo/palette_color_rle_lossless");
    assert_eq!(
        vl_photo_palette_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        vl_photo_palette_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        vl_photo_palette_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("PALETTE COLOR")
    );
    assert!(
        vl_photo_palette_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("VL Photographic palette RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("vl_palette_color_pixels")),
        "VL Photographic palette RLE report row should retain VL palette stressor"
    );
    let feature_gated_jpeg_row = coverage_row(&report, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
    assert_eq!(
        feature_gated_jpeg_row.get("status").and_then(Value::as_str),
        Some(if cfg!(feature = "jpeg") {
            "generated"
        } else {
            "unavailable"
        })
    );
    assert_eq!(
        feature_gated_jpeg_row
            .get("modality")
            .and_then(Value::as_str),
        Some("OT")
    );
    let feature_gated_deflated_seg_row = coverage_row(
        &report,
        "derived/seg/binary_multiframe_deflated_image_frame",
    );
    assert_eq!(
        feature_gated_deflated_seg_row
            .get("status")
            .and_then(Value::as_str),
        Some(if cfg!(feature = "deflate") {
            "generated"
        } else {
            "unavailable"
        })
    );
    assert_eq!(
        feature_gated_deflated_seg_row
            .get("modality")
            .and_then(Value::as_str),
        Some("SEG")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "jpeg")]
fn report_command_counts_generated_multifragment_jpeg_baseline_row() {
    let out_dir = unique_temp_dir("report-jpeg-multifragment-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("encapsulated_fragment_layout")
            .and_then(Value::as_str),
        Some("multi_fragment_per_frame")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_fragment_layouts/multi_fragment_per_frame")
            .and_then(Value::as_u64),
        Some(1)
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "htj2k_openjph")]
fn report_command_counts_generated_htj2k_lossless_row() {
    let out_dir = unique_temp_dir("report-htj2k-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(&report, "classic/sc/mono2_u16_htj2k_lossless");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.201")
    );
    assert_eq!(
        row.get("validation_status").and_then(Value::as_str),
        Some("passed")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "deflate")]
fn report_command_counts_generated_deflated_image_frame_seg_row() {
    let out_dir = unique_temp_dir("report-deflated-image-frame-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let row = coverage_row(
        &report,
        "derived/seg/binary_multiframe_deflated_image_frame",
    );
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.8.1")
    );
    assert_eq!(
        row.get("validation_status").and_then(Value::as_str),
        Some("passed")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "legacy_jpeg_dcmtk")]
fn report_command_counts_generated_legacy_jpeg_lossless_rows() {
    let out_dir = unique_temp_dir("report-legacy-jpeg-lossless-json");
    generate_extended(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    let process_14_row = coverage_row(&report, "classic/sc/mono2_u16_jpeg_lossless_process_14");
    assert_eq!(
        process_14_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        process_14_row
            .get("transfer_syntax")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.57")
    );
    assert_eq!(
        process_14_row
            .get("validation_status")
            .and_then(Value::as_str),
        Some("passed")
    );

    let row = coverage_row(&report, "classic/sc/mono2_u16_jpeg_lossless_sv1");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.70")
    );
    assert_eq!(
        row.get("validation_status").and_then(Value::as_str),
        Some("passed")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_projects_manifest_references_for_non_image_rows() {
    let out_dir = unique_temp_dir("report-non-image-references");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [
            {
                "case_id": "derived/rwvm/linear_ct_mapping_explicit_le",
                "profile_membership": ["extended"],
                "dicom": {
                    "iod_name": "Real World Value Mapping",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.67",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                    "transfer_syntax_name": "Explicit VR Little Endian"
                },
                "image": null,
                "pixel_data": null,
                "references": [
                    {
                        "relationship": "source_image",
                        "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
                        "source_path": "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
                        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
                        "sop_instance_uid": "2.25.1",
                        "series_instance_uid": "2.25.2",
                        "frame_numbers": [1, 2]
                    }
                ],
                "validation": {
                    "status": "passed"
                },
                "determinism": "byte_stable",
                "known_stressors": ["real_world_value_mapping"]
            }
        ],
        "skipped_cases": []
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept non-image manifest rows");
    let row = coverage_row(&report, "derived/rwvm/linear_ct_mapping_explicit_le");
    assert_eq!(
        row.get("photometric"),
        Some(&Value::Null),
        "non-image rows should not invent image metadata"
    );
    assert_eq!(
        row.pointer("/geometry/rows"),
        Some(&Value::Null),
        "non-image rows should keep geometry empty"
    );
    assert_eq!(
        row.get("derived_refs")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        row.get("derived_reference_relationships")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("source_image")
    );
    assert_eq!(
        row.get("derived_reference_targets")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        row.get("derived_reference_sop_class_uids")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.2.1")
    );
    assert_eq!(
        row.get("derived_reference_sop_instance_uid_roots")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("2.25")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_relationships/source_image")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer(
                "/grouped_coverage/derived_reference_targets/enhanced~1ct~1multiframe_shared_perframe_explicit_le"
            )
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer(
                "/grouped_coverage/derived_reference_sop_class_uids/1.2.840.10008.5.1.4.1.1.2.1"
            )
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_sop_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/object_types/derived")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        row.get("pixel_data_vr"),
        Some(&Value::Null),
        "non-image rows should not invent Pixel Data VR metadata"
    );
    assert_eq!(
        row.get("pixel_data_layout"),
        Some(&Value::Null),
        "non-image rows should not invent Pixel Data layout metadata"
    );
    assert_eq!(
        row.get("basic_offset_table"),
        Some(&Value::Null),
        "non-image rows should not invent Basic Offset Table metadata"
    );
    assert_eq!(
        row.get("encapsulated_fragment_layout"),
        Some(&Value::Null),
        "non-image rows should not invent fragment layout metadata"
    );
    assert_eq!(
        row.get("extended_offset_table"),
        Some(&Value::Null),
        "non-image rows should not invent Extended Offset Table metadata"
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_summarizes_compressed_codec_coverage() {
    let out_dir = unique_temp_dir("report-compressed-codec-summary");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [
            {
                "case_id": "classic/sc/rgb_planar0_rle_lossless",
                "profile_membership": ["extended"],
                "dicom": {
                    "iod_name": "Secondary Capture Image",
                    "modality": "OT",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.5",
                    "transfer_syntax_name": "RLE Lossless"
                },
                "image": {
                    "photometric_interpretation": "RGB",
                    "bits_allocated": 8,
                    "bits_stored": 8,
                    "high_bit": 7,
                    "pixel_representation": 0,
                    "samples_per_pixel": 3,
                    "planar_configuration": 0,
                    "frames": 1,
                    "rows": 2,
                    "columns": 2
                },
                "pixel_data": {
                    "vr": "OB",
                    "native_or_encapsulated": "encapsulated",
                    "encapsulated_pixel_data": {
                        "basic_offset_table": {
                            "present": true,
                            "populated": true,
                            "offset_count": 1,
                            "offsets": [0]
                        },
                        "fragments_per_frame": [1],
                        "fragments": [],
                        "extended_offset_table": {
                            "present": false,
                            "lengths_present": false,
                            "offset_count": 0,
                            "length_count": 0
                        },
                        "compressed_frame_hashes": [
                            "1111111111111111111111111111111111111111111111111111111111111111"
                        ]
                    },
                    "codec": {
                        "backend_id": "native_rle_lossless",
                        "backend_kind": "native",
                        "feature_gate": null
                    }
                },
                "validation": {
                    "status": "passed"
                },
                "determinism": "byte_stable",
                "expected_semantics": {
                    "synthetic_data": "YES",
                    "image_type": "DERIVED\\PRIMARY",
                    "conversion_type": "SYN",
                    "lossy_image_compression": "00"
                },
                "references": [
                    {
                        "relationship": "source_image",
                        "source_case_id": "classic/sc/mono2_u8_explicit_le",
                        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
                        "sop_instance_uid": "2.25.42"
                    }
                ],
                "known_stressors": ["compressed_pixel_data"]
            }
        ],
        "skipped_cases": [
            {
                "case_id": "classic/sc/rgb_planar0_jpeg_baseline_8bit",
                "status": "unavailable",
                "reason_code": "feature_gated_case_unavailable",
                "message": "This implemented registry case requires Cargo feature(s) jpeg.",
                "standards_evidence": []
            }
        ]
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should summarize compressed coverage");
    let generated = coverage_row(&report, "classic/sc/rgb_planar0_rle_lossless");
    assert_eq!(
        generated.get("codec_family").and_then(Value::as_str),
        Some("RLE Lossless")
    );
    assert_eq!(
        generated.get("codec_backend_id").and_then(Value::as_str),
        Some("native_rle_lossless")
    );
    assert_eq!(
        generated.get("modality").and_then(Value::as_str),
        Some("OT")
    );
    assert_eq!(
        generated.get("object_type").and_then(Value::as_str),
        Some("classic")
    );
    assert_eq!(
        generated.get("synthetic_data").and_then(Value::as_str),
        Some("YES")
    );
    assert_eq!(
        generated.get("image_type").and_then(Value::as_str),
        Some("DERIVED\\PRIMARY")
    );
    assert_eq!(
        generated.get("conversion_type").and_then(Value::as_str),
        Some("SYN")
    );
    assert_eq!(
        generated
            .get("lossy_image_compression")
            .and_then(Value::as_str),
        Some("00")
    );
    assert_eq!(
        generated
            .get("pixel_representation")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        generated.get("samples_per_pixel").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        generated.get("bits_allocated").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        generated.get("bits_stored").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(generated.get("high_bit").and_then(Value::as_u64), Some(7));
    assert_eq!(
        generated
            .get("planar_configuration")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        generated.get("pixel_data_vr").and_then(Value::as_str),
        Some("OB")
    );
    assert_eq!(
        generated.get("pixel_data_layout").and_then(Value::as_str),
        Some("encapsulated")
    );
    assert_eq!(
        generated.get("basic_offset_table").and_then(Value::as_str),
        Some("populated")
    );
    assert_eq!(
        generated
            .get("encapsulated_fragment_layout")
            .and_then(Value::as_str),
        Some("single_fragment_per_frame")
    );
    assert_eq!(
        generated
            .get("extended_offset_table")
            .and_then(Value::as_str),
        Some("absent")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_families/RLE Lossless")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/transfer_syntax_names/RLE Lossless")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/transfer_syntax_names/JPEG Baseline (Process 1)")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_families/JPEG Baseline")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/profile_memberships/extended")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_backends/native_rle_lossless")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_classes/1.2.840.10008.5.1.4.1.1.7")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/determinism/byte_stable")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/validation_statuses/passed")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/validation_statuses/unavailable")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/statuses/generated")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/statuses/unavailable")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/frame_counts/1")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/pixel_representations/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/samples_per_pixel/3")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/bits_allocated/8")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/bits_stored/8")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/high_bits/7")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/planar_configurations/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/pixel_data_vrs/OB")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/pixel_data_layouts/encapsulated")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/basic_offset_tables/populated")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_fragment_layouts/single_fragment_per_frame")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/extended_offset_tables/absent")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/geometries/2x2")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/object_types/classic")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_states/with_source_reference")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_states/without_source_reference")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        generated
            .get("derived_reference_relationships")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("source_image")
    );
    assert_eq!(
        generated
            .get("derived_reference_targets")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("classic/sc/mono2_u8_explicit_le")
    );
    assert_eq!(
        generated
            .get("derived_reference_sop_class_uids")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.7")
    );
    assert_eq!(
        generated
            .get("derived_reference_sop_instance_uid_roots")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("2.25")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_relationships/source_image")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer(
                "/grouped_coverage/derived_reference_targets/classic~1sc~1mono2_u8_explicit_le"
            )
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_sop_class_uids/1.2.840.10008.5.1.4.1.1.7")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/derived_reference_sop_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/synthetic_data/YES")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_types/DERIVED\\PRIMARY")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/conversion_types/SYN")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/lossy_image_compression/00")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/modalities/OT")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/unavailable_reasons/feature_gated_case_unavailable")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_feature_gates/jpeg")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/known_stressors/compressed_pixel_data")
            .and_then(Value::as_u64),
        Some(1)
    );
    let unavailable = coverage_row(&report, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
    assert_eq!(
        unavailable
            .get("codec_feature_gate")
            .and_then(Value::as_str),
        Some("jpeg")
    );
    assert_eq!(
        unavailable.get("modality").and_then(Value::as_str),
        Some("OT")
    );
    assert_eq!(
        unavailable
            .get("transfer_syntax_name")
            .and_then(Value::as_str),
        Some("JPEG Baseline (Process 1)")
    );
    assert_eq!(
        unavailable.get("object_type").and_then(Value::as_str),
        Some("classic")
    );
    assert_eq!(unavailable.get("pixel_representation"), Some(&Value::Null));
    assert_eq!(unavailable.get("samples_per_pixel"), Some(&Value::Null));
    assert_eq!(unavailable.get("bits_allocated"), Some(&Value::Null));
    assert_eq!(unavailable.get("bits_stored"), Some(&Value::Null));
    assert_eq!(unavailable.get("high_bit"), Some(&Value::Null));
    assert_eq!(unavailable.get("planar_configuration"), Some(&Value::Null));
    assert_eq!(unavailable.get("pixel_data_vr"), Some(&Value::Null));
    assert_eq!(unavailable.get("pixel_data_layout"), Some(&Value::Null));
    assert_eq!(unavailable.get("basic_offset_table"), Some(&Value::Null));
    assert_eq!(
        unavailable.get("lossy_image_compression"),
        Some(&Value::Null)
    );
    assert_eq!(unavailable.get("image_type"), Some(&Value::Null));
    assert_eq!(unavailable.get("conversion_type"), Some(&Value::Null));
    assert_eq!(
        unavailable.get("encapsulated_fragment_layout"),
        Some(&Value::Null)
    );
    assert_eq!(unavailable.get("extended_offset_table"), Some(&Value::Null));

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("### Codec Families"));
    assert!(markdown.contains("| RLE Lossless | 1 |"));
    assert!(markdown.contains("### SOP Classes"));
    assert!(markdown.contains("| 1.2.840.10008.5.1.4.1.1.7 | 2 |"));
    assert!(markdown.contains("### SOP Class Names"));
    assert!(markdown.contains("| Secondary Capture Image Storage | 1 |"));
    assert!(markdown.contains("### Modalities"));
    assert!(markdown.contains("| OT | 2 |"));
    assert!(markdown.contains("### Statuses"));
    assert!(markdown.contains("| generated | 1 |"));
    assert!(markdown.contains("| unavailable | 1 |"));
    assert!(markdown.contains("### Codec Backends"));
    assert!(markdown.contains("| native_rle_lossless | 1 |"));
    assert!(markdown.contains("### Codec Feature Gates"));
    assert!(markdown.contains("| jpeg | 1 |"));
    assert!(markdown.contains("### Validation Statuses"));
    assert!(markdown.contains("| passed | 1 |"));
    assert!(markdown.contains("| unavailable | 1 |"));
    assert!(markdown.contains("### Unavailable Reasons"));
    assert!(markdown.contains("### Frame Counts"));
    assert!(markdown.contains("| 1 | 1 |"));
    assert!(markdown.contains("### Pixel Representations"));
    assert!(markdown.contains("| 0 | 1 |"));
    assert!(markdown.contains("### Samples Per Pixel"));
    assert!(markdown.contains("| 3 | 1 |"));
    assert!(markdown.contains("### Bits Allocated"));
    assert!(markdown.contains("| 8 | 1 |"));
    assert!(markdown.contains("### Bits Stored"));
    assert!(markdown.contains("| 8 | 1 |"));
    assert!(markdown.contains("### High Bits"));
    assert!(markdown.contains("| 7 | 1 |"));
    assert!(markdown.contains("### Planar Configurations"));
    assert!(markdown.contains("| 0 | 1 |"));
    assert!(markdown.contains("### Pixel Data VRs"));
    assert!(markdown.contains("| OB | 1 |"));
    assert!(markdown.contains("### Pixel Data Layouts"));
    assert!(markdown.contains("| encapsulated | 1 |"));
    assert!(markdown.contains("### Basic Offset Tables"));
    assert!(markdown.contains("| populated | 1 |"));
    assert!(markdown.contains("### Encapsulated Fragment Layouts"));
    assert!(markdown.contains("| single_fragment_per_frame | 1 |"));
    assert!(markdown.contains("### Extended Offset Tables"));
    assert!(markdown.contains("| absent | 1 |"));
    assert!(markdown.contains("### Geometries"));
    assert!(markdown.contains("| 2x2 | 1 |"));
    assert!(markdown.contains("### Object Types"));
    assert!(markdown.contains("| classic | 2 |"));
    assert!(markdown.contains("### Derived Reference States"));
    assert!(markdown.contains("| with_source_reference | 1 |"));
    assert!(markdown.contains("| without_source_reference | 1 |"));
    assert!(markdown.contains("### Derived Reference Relationships"));
    assert!(markdown.contains("| source_image | 1 |"));
    assert!(markdown.contains("### Derived Reference Targets"));
    assert!(markdown.contains("| classic/sc/mono2_u8_explicit_le | 1 |"));
    assert!(markdown.contains("### Derived Reference SOP Class UIDs"));
    assert!(markdown.contains("| 1.2.840.10008.5.1.4.1.1.7 | 1 |"));
    assert!(markdown.contains("### Synthetic Data"));
    assert!(markdown.contains("| YES | 1 |"));
    assert!(markdown.contains("### Image Types"));
    assert!(markdown.contains("| DERIVED\\PRIMARY | 1 |"));
    assert!(markdown.contains("### Conversion Types"));
    assert!(markdown.contains("| SYN | 1 |"));
    assert!(markdown.contains("### Lossy Image Compression"));
    assert!(markdown.contains("| 00 | 1 |"));
    assert!(markdown.contains("### Known Stressors"));
    assert!(markdown.contains("| compressed_pixel_data | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_counts_feature_gated_planned_cases_as_planned() {
    let out_dir = unique_temp_dir("report-feature-gated-planned");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [],
        "skipped_cases": [
            {
                "case_id": "classic/sc/mono2_u8_deflated_explicit_le",
                "status": "unavailable",
                "reason_code": "feature_gated_case_planned",
                "message": "This planned registry case requires Cargo feature(s) deflate.",
                "recheck_phase": "phase-6",
                "standards_evidence": []
            }
        ]
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept feature-gated planned rows");
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report.pointer("/counts/skipped").and_then(Value::as_u64),
        Some(0)
    );
    let row = coverage_row(&report, "classic/sc/mono2_u8_deflated_explicit_le");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("planned"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.1.99")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_exposes_external_generation_backend_provenance() {
    let out_dir = unique_temp_dir("report-generation-backend");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [
            {
                "case_id": "derived/parametric-map/float32_ct_derived_explicit_le",
                "profile_membership": ["extended"],
                "relative_path": "derived/parametric-map/float32_ct_derived_explicit_le/instance.dcm",
                "dicom": {
                    "iod_name": "Parametric Map",
                    "modality": "OT",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.30",
                    "sop_class_name": "Parametric Map Storage",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                    "transfer_syntax_name": "Explicit VR Little Endian"
                },
                "image": {
                    "rows": 2,
                    "columns": 2,
                    "frames": 2,
                    "samples_per_pixel": 1,
                    "photometric_interpretation": "MONOCHROME2",
                    "bits_allocated": 32,
                    "sample_type": "float32"
                },
                "pixel_data": {
                    "vr": "OF",
                    "native_or_encapsulated": "native",
                    "value_length": 32,
                    "frame_count": 2,
                    "frame_hashes": [
                        "0000000000000000000000000000000000000000000000000000000000000000",
                        "1111111111111111111111111111111111111111111111111111111111111111"
                    ]
                },
                "generation_backend": {
                    "backend_id": "highdicom_pydicom",
                    "version": "0.27.0",
                    "determinism": "semantic_stable"
                },
                "validation": {
                    "status": "passed"
                },
                "determinism": "semantic_stable",
                "expected_semantics": {
                    "synthetic_data": "YES"
                },
                "known_stressors": ["float_pixel_data"]
            }
        ],
        "skipped_cases": []
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept external generation backend provenance");
    let row = coverage_row(
        &report,
        "derived/parametric-map/float32_ct_derived_explicit_le",
    );
    assert_eq!(
        row.get("generation_backend_id").and_then(Value::as_str),
        Some("highdicom_pydicom")
    );
    assert_eq!(
        row.get("generation_backend_version")
            .and_then(Value::as_str),
        Some("0.27.0")
    );
    assert_eq!(
        row.get("generation_backend_determinism")
            .and_then(Value::as_str),
        Some("semantic_stable")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/generation_backends/highdicom_pydicom")
            .and_then(Value::as_u64),
        Some(1)
    );
    let report_schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json")
            .expect("coverage schema should be readable"),
    )
    .expect("coverage schema should be JSON");
    let validator =
        jsonschema::validator_for(&report_schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "provenance coverage report must match its schema: {errors:?}"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("### Generation Backends"));
    assert!(markdown.contains("| highdicom_pydicom | 1 |"));
    assert!(markdown.contains("| highdicom_pydicom | 0.27.0 | semantic_stable | passed |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_counts_feature_gated_implemented_cases_as_unavailable() {
    let out_dir = unique_temp_dir("report-feature-gated-implemented");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [],
        "skipped_cases": [
            {
                "case_id": "classic/sc/mono2_u8_deflated_explicit_le",
                "status": "unavailable",
                "reason_code": "feature_gated_case_unavailable",
                "message": "This implemented registry case requires Cargo feature(s) deflate.",
                "recheck_phase": "phase-6",
                "standards_evidence": []
            }
        ]
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept feature-gated unavailable rows");
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        report.pointer("/counts/skipped").and_then(Value::as_u64),
        Some(1)
    );
    let row = coverage_row(&report, "classic/sc/mono2_u8_deflated_explicit_le");
    assert_eq!(
        row.get("status").and_then(Value::as_str),
        Some("unavailable")
    );
    for field in [
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
    ] {
        assert_eq!(
            row.get(field),
            Some(&Value::Null),
            "unavailable coverage rows must keep {field} explicitly null"
        );
    }

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_summarizes_lossy_image_compression_method() {
    let out_dir = unique_temp_dir("report-lossy-method");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [
            {
                "case_id": "classic/sc/rgb_planar0_jpeg_baseline_8bit",
                "profile_membership": ["extended"],
                "relative_path": "classic/sc/rgb_planar0_jpeg_baseline_8bit/instance.dcm",
                "dicom": {
                    "iod_name": "Secondary Capture Image",
                    "modality": "OT",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.4.50",
                    "transfer_syntax_name": "JPEG Baseline (Process 1)"
                },
                "image": {
                    "rows": 2,
                    "columns": 2,
                    "frames": 1,
                    "samples_per_pixel": 3,
                    "photometric_interpretation": "RGB",
                    "bits_allocated": 8,
                    "bits_stored": 8,
                    "high_bit": 7,
                    "pixel_representation": 0,
                    "planar_configuration": 0
                },
                "pixel_data": {
                    "vr": "OB",
                    "native_or_encapsulated": "encapsulated",
                    "value_length": null,
                    "frame_count": 1,
                    "frame_hashes": [
                        "0000000000000000000000000000000000000000000000000000000000000000"
                    ],
                    "encapsulated_pixel_data": {
                        "basic_offset_table": {
                            "present": true,
                            "populated": true,
                            "offset_count": 1,
                            "offsets": [0]
                        },
                        "fragments_per_frame": [1],
                        "fragments": [],
                        "extended_offset_table": {
                            "present": false,
                            "lengths_present": false,
                            "offset_count": 0,
                            "length_count": 0
                        },
                        "compressed_frame_hashes": [
                            "1111111111111111111111111111111111111111111111111111111111111111"
                        ]
                    },
                    "codec": {
                        "backend_id": "dicom_rs_jpeg_baseline_writer",
                        "backend_kind": "dicom-rs-adapter",
                        "feature_gate": "jpeg"
                    }
                },
                "validation": {
                    "status": "passed"
                },
                "determinism": "semantic_stable",
                "expected_semantics": {
                    "synthetic_data": "YES",
                    "lossy_image_compression": "01",
                    "lossy_image_compression_ratio": "1.500000",
                    "lossy_image_compression_method": "ISO_10918_1"
                },
                "known_stressors": ["lossy_image_compression"]
            }
        ],
        "skipped_cases": []
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should summarize lossy compression method coverage");
    let row = coverage_row(&report, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
    assert_eq!(
        row.get("lossy_image_compression_ratio")
            .and_then(Value::as_str),
        Some("1.500000")
    );
    assert_eq!(
        row.get("lossy_image_compression_method")
            .and_then(Value::as_str),
        Some("ISO_10918_1")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/lossy_image_compression_ratios/1.500000")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/lossy_image_compression_methods/ISO_10918_1")
            .and_then(Value::as_u64),
        Some(1)
    );
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("### Lossy Image Compression Ratios"));
    assert!(markdown.contains("| 1.500000 | 1 |"));
    assert!(markdown.contains("### Lossy Image Compression Methods"));
    assert!(markdown.contains("| ISO_10918_1 | 1 |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_rejects_missing_manifest() {
    let out_dir = unique_temp_dir("report-missing-manifest");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        !output.status.success(),
        "report should fail without a manifest"
    );
    let stderr = String::from_utf8(output.stderr).expect("report stderr must be UTF-8");
    assert!(stderr.contains("failed to read report metadata"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_surfaces_complete_unsigned_u32_pixel_contract() {
    let out_dir = unique_temp_dir("report-u32-pixels");
    generate_extended(&out_dir);
    let json_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(json_output.status.success());
    let report: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    let row = coverage_row(&report, "classic/sc/mono2_u32_explicit_le");
    assert_eq!(row["u32_stored_values"], "0; 65535; 2147483648; 4294967295");
    assert_eq!(
        row["u32_pixel_data_sha256"],
        "56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41"
    );
    assert_eq!(row["u32_word_byte_order"], "little_endian");
    assert_eq!(row["u32_full_unsigned_range"], true);
    assert_eq!(
        report.pointer("/grouped_coverage/u32_stored_value_sets/0; 65535; 2147483648; 4294967295"),
        Some(&json!(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/u32_full_unsigned_range_states/true"),
        Some(&json!(1))
    );
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/coverage-report.schema.json").unwrap()).unwrap();
    let report_validator = jsonschema::validator_for(&schema).unwrap();
    assert!(report_validator.is_valid(&report));

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "markdown"])
        .output()
        .unwrap();
    assert!(markdown_output.status.success());
    let markdown = String::from_utf8(markdown_output.stdout).unwrap();
    assert!(markdown.contains("### Unsigned 32-bit Stored Value Sets"));
    assert!(markdown.contains("0; 65535; 2147483648; 4294967295"));
    assert!(markdown.contains("56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41"));

    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == "classic/sc/mono2_u32_explicit_le")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("expected_u32_pixels");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("requires expected_u32_pixels"));

    fs::remove_dir_all(out_dir).unwrap();
}

#[test]
fn report_surfaces_complete_one_bit_pixel_contract() {
    let out_dir = unique_temp_dir("report-u1-pixels");
    generate_extended(&out_dir);
    let json_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(json_output.status.success());
    let report: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    let row = coverage_row(&report, "classic/sc/mono2_u1_native");
    assert_eq!(
        row["u1_stored_values"],
        "1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0"
    );
    assert_eq!(
        row["u1_decoded_frame_sha256"],
        "a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3; c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae"
    );
    assert_eq!(
        row["u1_pixel_data_sha256"],
        "9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b"
    );
    assert_eq!(row["u1_packing_order"], "least_significant_bit_first");
    assert_eq!(
        row["u1_frame_boundary_policy"],
        "continuous_without_per_frame_padding"
    );
    assert_eq!(row["u1_significant_bits"], 18);
    assert_eq!(row["u1_unused_high_bits"], 6);
    assert_eq!(row["u1_value_field_padding_bytes"], 1);
    assert_eq!(
        report.pointer("/grouped_coverage/u1_packing_orders/least_significant_bit_first"),
        Some(&json!(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/u1_value_field_padding_byte_counts/1"),
        Some(&json!(1))
    );
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/coverage-report.schema.json").unwrap()).unwrap();
    let report_validator = jsonschema::validator_for(&schema).unwrap();
    assert!(report_validator.is_valid(&report));

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "markdown"])
        .output()
        .unwrap();
    assert!(markdown_output.status.success());
    let markdown = String::from_utf8(markdown_output.stdout).unwrap();
    assert!(markdown.contains("### One-bit Stored Value Sets"));
    assert!(markdown.contains("continuous_without_per_frame_padding"));
    assert!(markdown.contains("9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b"));

    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == "classic/sc/mono2_u1_native")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("expected_u1_pixels");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("requires expected_u1_pixels"));

    fs::remove_dir_all(out_dir).unwrap();
}

#[test]
fn report_surfaces_complete_icc_profile_contract() {
    let out_dir = unique_temp_dir("report-icc-profile");
    generate_extended(&out_dir);
    let json_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(json_output.status.success());
    let report: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    let row = coverage_row(&report, "vl/photo/rgb_icc_profile_explicit_le");
    assert_eq!(row["icc_profile_tag"], "(0028,2000)");
    assert_eq!(row["icc_profile_vr"], "OB");
    assert_eq!(
        row["icc_profile_sha256"],
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
    );
    assert_eq!(row["icc_profile_size_bytes"], 736);
    assert_eq!(row["icc_declared_profile_size_bytes"], 736);
    assert_eq!(row["icc_profile_version"], "2.1.0");
    assert_eq!(row["icc_device_class"], "scnr");
    assert_eq!(row["icc_data_color_space"], "RGB");
    assert_eq!(row["icc_profile_connection_space"], "XYZ");
    assert_eq!(row["icc_profile_signature"], "acsp");
    assert_eq!(row["icc_rendering_intent"], "perceptual");
    assert_eq!(row["icc_rendering_intent_code"], 0);
    assert_eq!(row["icc_tag_count"], 9);
    assert_eq!(row["icc_color_space"], "SRGB");
    assert_eq!(row["icc_profile_description"], "sRGB");
    assert_eq!(row["icc_copyright"], "CC0");
    assert_eq!(
        row["icc_source_identity"],
        "DCMTK 3.7.0 DCMTK_SRGB_ICC_SAMPLE"
    );
    assert_eq!(
        report.pointer("/grouped_coverage/icc_device_classes/scnr"),
        Some(&json!(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/icc_profile_size_byte_counts/736"),
        Some(&json!(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/icc_color_spaces/SRGB"),
        Some(&json!(1))
    );
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/coverage-report.schema.json").unwrap()).unwrap();
    let report_validator = jsonschema::validator_for(&schema).unwrap();
    assert!(report_validator.is_valid(&report));
    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "markdown"])
        .output()
        .unwrap();
    assert!(markdown_output.status.success());
    let markdown = String::from_utf8(markdown_output.stdout).unwrap();
    assert!(markdown.contains("### ICC Profile SHA-256 Values"));
    assert!(markdown.contains("### ICC Profile Connection Spaces"));
    assert!(markdown.contains("DCMTK 3.7.0 DCMTK_SRGB_ICC_SAMPLE"));

    let manifest_path = out_dir.join("manifest.json");
    let original_manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let mut missing_contract = original_manifest.clone();
    missing_contract["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == "vl/photo/rgb_icc_profile_explicit_le")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("expected_icc_profile");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&missing_contract).unwrap(),
    )
    .unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("requires expected_icc_profile"));

    let mut malformed_contract = original_manifest;
    malformed_contract["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == "vl/photo/rgb_icc_profile_explicit_le")
        .unwrap()["expected_icc_profile"]["profile_connection_space"] = json!("Lab");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&malformed_contract).unwrap(),
    )
    .unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("requires XYZ profile connection space")
    );

    fs::remove_dir_all(out_dir).unwrap();
}

#[test]
fn report_surfaces_both_nonsquare_spatial_variants() {
    let out_dir = unique_temp_dir("report-nonsquare-spacing");
    generate_core(&out_dir);
    let json_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(
        json_output.status.success(),
        "report should accept non-square variants: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let report: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    let rows = coverage_rows(&report, "classic/sc/nonsquare_pixel_spacing");
    assert_eq!(rows.len(), 2);
    let spacing = rows
        .iter()
        .find(|row| row["nonsquare_variant_id"] == "pixel_spacing")
        .expect("report should contain the physical-spacing variant");
    assert_eq!(spacing["nonsquare_pixel_spacing"], "0.6\\0.3");
    assert_eq!(
        spacing["nonsquare_nominal_scanned_pixel_spacing"],
        "0.6\\0.3"
    );
    assert!(spacing["nonsquare_pixel_aspect_ratio"].is_null());
    let aspect = rows
        .iter()
        .find(|row| row["nonsquare_variant_id"] == "pixel_aspect_ratio")
        .expect("report should contain the aspect-ratio variant");
    assert!(aspect["nonsquare_pixel_spacing"].is_null());
    assert!(aspect["nonsquare_nominal_scanned_pixel_spacing"].is_null());
    assert_eq!(aspect["nonsquare_pixel_aspect_ratio"], "2\\1");
    for row in rows {
        assert_eq!(row["nonsquare_uncalibrated"], true);
        assert_eq!(row["nonsquare_patient_space_geometry_present"], false);
        assert_eq!(
            row["nonsquare_pixel_data_sha256"],
            "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6"
        );
    }
    assert_eq!(
        report.pointer("/grouped_coverage/nonsquare_variant_ids/pixel_spacing"),
        Some(&json!(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/nonsquare_variant_ids/pixel_aspect_ratio"),
        Some(&json!(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/nonsquare_uncalibrated_states/true"),
        Some(&json!(2))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/nonsquare_patient_space_geometry_present_states/false"),
        Some(&json!(2))
    );
    assert_eq!(
        report.pointer(&format!(
            "/grouped_coverage/nonsquare_pixel_data_sha256_values/{}",
            "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6"
        )),
        Some(&json!(2))
    );
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/coverage-report.schema.json").unwrap()).unwrap();
    let report_validator = jsonschema::validator_for(&schema).unwrap();
    assert!(report_validator.is_valid(&report));
    let mut crossed_report = report.clone();
    crossed_report["coverage_matrix"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| {
            row["case_id"] == "classic/sc/nonsquare_pixel_spacing"
                && row["nonsquare_variant_id"] == "pixel_aspect_ratio"
        })
        .unwrap()["nonsquare_pixel_spacing"] = json!("0.6\\0.3");
    assert!(
        !report_validator.is_valid(&crossed_report),
        "coverage schema must reject crossed non-square axes"
    );
    let mut hidden_report = report.clone();
    coverage_row_mut(
        &mut hidden_report,
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    )["nonsquare_variant_id"] = json!("pixel_spacing");
    assert!(
        !report_validator.is_valid(&hidden_report),
        "coverage schema must reject non-square fields on another case"
    );

    let markdown_output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "markdown"])
        .output()
        .unwrap();
    assert!(markdown_output.status.success());
    let markdown = String::from_utf8(markdown_output.stdout).unwrap();
    for expected in [
        "### Non-square Spatial Variant IDs",
        "## Non-square Spatial Expectations",
        "pixel_spacing",
        "pixel_aspect_ratio",
        "0.6\\0.3",
        "2\\1",
        "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6",
    ] {
        assert!(
            markdown.contains(expected),
            "markdown should contain {expected}"
        );
    }

    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| {
            file["case_id"] == "classic/sc/nonsquare_pixel_spacing"
                && file["expected_nonsquare_spacing"]["variant_id"] == "pixel_aspect_ratio"
        })
        .unwrap()["expected_nonsquare_spacing"]["pixel_aspect_ratio"]["lexical_value"] =
        json!("1\\2");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report"])
        .arg(&out_dir)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("requires one exact spatial variant")
    );

    fs::remove_dir_all(out_dir).unwrap();
}

#[test]
fn spatial_registration_report_exposes_strict_json_groups_and_compact_markdown() {
    let out_dir = unique_temp_dir("report-spatial-registration");
    generate_extended(&out_dir);

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("Spatial Registration coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "registration report must match schema: {errors:?}"
    );

    let row = coverage_row(&report, "derived/registration/spatial_ct_pair");
    assert_eq!(row["registration_matrix_direction"], "source_to_registered");
    assert_eq!(row["registration_matrix_type"], "RIGID");
    assert_eq!(row["registration_item_count"], 2);
    assert_eq!(
        row["registration_reference_topology"],
        "same_study_target+other_study_source"
    );
    assert_eq!(
        row["registration_reference_relationships"],
        "registered_target; moving_source"
    );
    assert_eq!(row["registration_pixel_data_absent"], true);
    assert_eq!(
        row["registration_landmark_mapping"],
        "[-0.625, -0.625, 0] -> [0, 0, 2.5]"
    );
    for pointer in [
        "/grouped_coverage/registration_matrix_directions/source_to_registered",
        "/grouped_coverage/registration_matrix_types/RIGID",
        "/grouped_coverage/registration_item_counts/2",
        "/grouped_coverage/registration_reference_topologies/same_study_target+other_study_source",
        "/grouped_coverage/registration_reference_relationships/registered_target; moving_source",
        "/grouped_coverage/registration_pixel_data_absent_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    assert_eq!(
        report
            .pointer("/grouped_coverage/registration_landmark_mappings")
            .and_then(Value::as_object)
            .and_then(|mappings| mappings.get("[-0.625, -0.625, 0] -> [0, 0, 2.5]")),
        Some(&Value::from(1))
    );

    let mut incomplete = report.clone();
    coverage_row_mut(&mut incomplete, "derived/registration/spatial_ct_pair")["registration_pixel_data_absent"] =
        Value::Null;
    assert!(
        !validator.is_valid(&incomplete),
        "schema must reject a partial Spatial Registration report contract"
    );
    let mut leaked = report.clone();
    coverage_row_mut(
        &mut leaked,
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    )["registration_matrix_type"] = json!("RIGID");
    assert!(
        !validator.is_valid(&leaked),
        "schema must reject registration fields on unrelated rows"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Spatial Registration Expectations"));
    assert!(markdown.contains("Matrix direction"));
    assert!(markdown.contains("registered_target; moving_source"));
    assert!(markdown.contains("same_study_target+other_study_source"));
    assert!(markdown.contains("[-0.625, -0.625, 0] -> [0, 0, 2.5]"));
    assert!(markdown.contains("### Registration Matrix Directions"));
    assert!(markdown.contains("| source_to_registered | 1 |"));

    fs::remove_dir_all(out_dir).expect("remove report root");
}

#[test]
fn deformable_registration_report_exposes_exact_grid_contract() {
    let out_dir = unique_temp_dir("report-deformable-registration");
    generate_extended(&out_dir);
    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("generated manifest"))
            .expect("manifest JSON");
    let deformable_generated = manifest["files"].as_array().is_some_and(|files| {
        files
            .iter()
            .any(|file| file["case_id"] == Value::from("derived/registration/deformable_ct_pair"))
    });
    if !deformable_generated {
        let mut fixture = manifest["files"]
            .as_array()
            .and_then(|files| {
                files.iter().find(|file| {
                    file["case_id"] == Value::from("derived/registration/spatial_ct_pair")
                })
            })
            .cloned()
            .expect("Spatial Registration fixture source");
        fixture["case_id"] = json!("derived/registration/deformable_ct_pair");
        fixture["expected_deformable_spatial_registration"] = json!({
            "sampling_direction": "registered_to_source",
            "grid": {
                "dimensions": [2, 2, 1],
                "resolution_mm": [0.75, 0.75, 2.5],
                "vector_count": 4,
                "payload_sha256": "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47"
            }
        });
        manifest["files"]
            .as_array_mut()
            .expect("manifest files")
            .push(fixture);
        manifest["skipped_cases"]
            .as_array_mut()
            .expect("manifest skipped cases")
            .retain(|case| {
                case["case_id"] != Value::from("derived/registration/deformable_ct_pair")
            });
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize report fixture manifest"),
        )
        .expect("write report fixture manifest");
    }

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("Deformable Spatial Registration coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "deformable registration report must match schema: {errors:?}"
    );

    let row = coverage_row(&report, "derived/registration/deformable_ct_pair");
    assert_eq!(
        row["deformable_registration_sampling_direction"],
        "registered_to_source"
    );
    assert_eq!(row["deformable_registration_grid_dimensions"], "2x2x1");
    assert_eq!(
        row["deformable_registration_grid_resolution_mm"],
        "0.75\\0.75\\2.5"
    );
    assert_eq!(row["deformable_registration_vector_count"], 4);
    assert_eq!(
        row["deformable_registration_payload_sha256"],
        "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47"
    );
    assert_eq!(
        row["deformable_registration_matrix_types"],
        "pre:RIGID; post:RIGID"
    );
    assert_eq!(
        row["deformable_registration_reference_topology"],
        "same_study_target+other_study_source"
    );
    assert_eq!(
        row["deformable_registration_mapping_summary"],
        "4 registered_to_source point mappings"
    );
    for pointer in [
        "/grouped_coverage/deformable_registration_sampling_directions/registered_to_source",
        "/grouped_coverage/deformable_registration_grid_dimensions/2x2x1",
        "/grouped_coverage/deformable_registration_grid_resolutions/0.75\\0.75\\2.5",
        "/grouped_coverage/deformable_registration_vector_counts/4",
        "/grouped_coverage/deformable_registration_matrix_types/pre:RIGID; post:RIGID",
        "/grouped_coverage/deformable_registration_reference_topologies/same_study_target+other_study_source",
        "/grouped_coverage/deformable_registration_mapping_summaries/4 registered_to_source point mappings",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    assert_eq!(
        report
            .pointer("/grouped_coverage/deformable_registration_payload_sha256_values")
            .and_then(Value::as_object)
            .and_then(|hashes| hashes
                .get("d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47")),
        Some(&Value::from(1))
    );

    let mut incomplete = report.clone();
    coverage_row_mut(&mut incomplete, "derived/registration/deformable_ct_pair")["deformable_registration_vector_count"] =
        Value::Null;
    assert!(
        !validator.is_valid(&incomplete),
        "schema must reject a partial deformable registration report contract"
    );
    let mut leaked = report.clone();
    coverage_row_mut(
        &mut leaked,
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    )["deformable_registration_grid_dimensions"] = json!("2x2x1");
    assert!(
        !validator.is_valid(&leaked),
        "schema must reject deformable registration fields on unrelated rows"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Deformable Spatial Registration Expectations"));
    assert!(markdown.contains("registered_to_source"));
    assert!(markdown.contains("0.75\\0.75\\2.5"));
    assert!(markdown.contains("pre:RIGID; post:RIGID"));
    assert!(markdown.contains("4 registered_to_source point mappings"));
    assert!(markdown.contains("### Deformable Registration Grid Dimensions"));
    assert!(markdown.contains("| 2x2x1 | 1 |"));

    fs::remove_dir_all(out_dir).expect("remove report root");
}

#[test]
fn advanced_blending_report_exposes_exact_topology_and_unresolved_findings() {
    let out_dir = unique_temp_dir("report-advanced-blending");
    generate_extended(&out_dir);
    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("generated manifest"))
            .expect("manifest JSON");
    if !manifest["files"].as_array().is_some_and(|files| {
        files.iter().any(|file| {
            file["case_id"] == Value::from("derived/presentation-state/advanced_blending")
        })
    }) {
        let mut fixture = manifest["files"]
            .as_array()
            .and_then(|files| {
                files.iter().find(|file| {
                    file["case_id"] == Value::from("derived/presentation-state/color_softcopy")
                })
            })
            .cloned()
            .expect("Color Softcopy report fixture source");
        fixture["case_id"] = json!("derived/presentation-state/advanced_blending");
        fixture["dicom"]["iod_name"] = json!("Advanced Blending Presentation State");
        fixture["dicom"]["sop_class_uid"] = json!("1.2.840.10008.5.1.4.1.1.11.8");
        fixture["expected_advanced_blending_presentation_state"] = json!({
            "same_study": true,
            "shared_frame_of_reference": true,
            "different_series": true,
            "sources": [
                {"complete_instance": true, "referenced_frame_numbers": []},
                {"complete_instance": true, "referenced_frame_numbers": []},
                {"complete_instance": true, "referenced_frame_numbers": []},
                {"complete_instance": true, "referenced_frame_numbers": []}
            ],
            "blending_inputs": [
                {"input_number": 1, "time_series_blending": "FALSE", "geometry_for_display": "TRUE"},
                {"input_number": 2, "time_series_blending": "FALSE", "geometry_for_display": "FALSE"}
            ],
            "display_operation": {
                "items": 1,
                "input_numbers": [1, 2],
                "blending_mode": "EQUAL",
                "final_output": true
            },
            "icc_profile": {
                "size_bytes": 736,
                "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
                "dicom_color_space": "SRGB"
            },
            "common_instance_reference": {
                "series": [
                    {"referenced_source_indices": [1, 2]},
                    {"referenced_source_indices": [3, 4]}
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
        });
        manifest["files"]
            .as_array_mut()
            .expect("manifest files")
            .push(fixture);
        manifest["skipped_cases"]
            .as_array_mut()
            .expect("manifest skipped cases")
            .retain(|case| {
                case["case_id"] != Value::from("derived/presentation-state/advanced_blending")
            });
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize report fixture manifest"),
        )
        .expect("write report fixture manifest");
    }

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("Advanced Blending coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "Advanced Blending report schema: {errors:?}"
    );

    let row = coverage_row(&report, "derived/presentation-state/advanced_blending");
    for (field, expected) in [
        (
            "advanced_blending_presentation_state_kind",
            json!("Advanced Blending Presentation State"),
        ),
        (
            "advanced_blending_sop_class_uid",
            json!("1.2.840.10008.5.1.4.1.1.11.8"),
        ),
        ("advanced_blending_source_series_count", json!(2)),
        ("advanced_blending_source_image_count", json!(4)),
        ("advanced_blending_source_closure", json!(true)),
        ("advanced_blending_input_numbers", json!("1; 2")),
        ("advanced_blending_time_series_flags", json!("FALSE; FALSE")),
        (
            "advanced_blending_geometry_for_display_flags",
            json!("TRUE; FALSE"),
        ),
        ("advanced_blending_display_operation_count", json!(1)),
        ("advanced_blending_display_input_order", json!("1; 2")),
        ("advanced_blending_final_output", json!(true)),
        ("advanced_blending_blending_mode", json!("EQUAL")),
        ("advanced_blending_icc_profile_size_bytes", json!(736)),
        ("advanced_blending_icc_color_space", json!("SRGB")),
        ("advanced_blending_common_reference_closure", json!(true)),
        ("advanced_blending_optional_transforms_absent", json!(true)),
        ("advanced_blending_pixel_data_absent", json!(true)),
    ] {
        assert_eq!(row[field], expected, "{field}");
    }
    assert_eq!(
        row["advanced_blending_icc_profile_sha256"],
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
    );
    assert!(
        row["advanced_blending_unresolved_external_validator_findings"]
            .as_str()
            .is_some_and(|finding| finding.contains("dciodvfy"))
    );

    for pointer in [
        "/grouped_coverage/advanced_blending_source_series_counts/2",
        "/grouped_coverage/advanced_blending_source_image_counts/4",
        "/grouped_coverage/advanced_blending_source_closure_states/true",
        "/grouped_coverage/advanced_blending_input_number_orders/1; 2",
        "/grouped_coverage/advanced_blending_time_series_flags/FALSE; FALSE",
        "/grouped_coverage/advanced_blending_geometry_for_display_flags/TRUE; FALSE",
        "/grouped_coverage/advanced_blending_display_operation_counts/1",
        "/grouped_coverage/advanced_blending_display_input_orders/1; 2",
        "/grouped_coverage/advanced_blending_final_output_states/true",
        "/grouped_coverage/advanced_blending_modes/EQUAL",
        "/grouped_coverage/advanced_blending_icc_profile_size_byte_counts/736",
        "/grouped_coverage/advanced_blending_common_reference_closure_states/true",
        "/grouped_coverage/advanced_blending_optional_transforms_absent_states/true",
        "/grouped_coverage/advanced_blending_pixel_data_absent_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&json!(1)), "{pointer}");
    }

    let mut incomplete = report.clone();
    coverage_row_mut(
        &mut incomplete,
        "derived/presentation-state/advanced_blending",
    )["advanced_blending_common_reference_closure"] = Value::Null;
    assert!(!validator.is_valid(&incomplete));
    let mut leaked = report.clone();
    coverage_row_mut(
        &mut leaked,
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    )["advanced_blending_blending_mode"] = json!("EQUAL");
    assert!(!validator.is_valid(&leaked));

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## Advanced Blending Presentation State Expectations",
        "2 series / 4 images",
        "FALSE; FALSE",
        "TRUE; FALSE",
        "dciodvfy",
        "### Advanced Blending Input Number Orders",
    ] {
        assert!(
            markdown.contains(expected),
            "markdown should contain {expected}"
        );
    }
    fs::remove_dir_all(out_dir).expect("remove report root");
}

#[test]
fn blending_report_exposes_palette_rescale_and_source_closure() {
    let out_dir = unique_temp_dir("report-blending");
    generate_extended(&out_dir);
    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("generated manifest"))
            .expect("manifest JSON");
    if !manifest["files"].as_array().is_some_and(|files| {
        files
            .iter()
            .any(|file| file["case_id"] == Value::from("derived/presentation-state/blending"))
    }) {
        let mut fixture = manifest["files"]
            .as_array()
            .and_then(|files| {
                files.iter().find(|file| {
                    file["case_id"] == Value::from("derived/presentation-state/color_softcopy")
                })
            })
            .cloned()
            .expect("Color Softcopy report fixture source");
        fixture["case_id"] = json!("derived/presentation-state/blending");
        fixture["dicom"]["iod_name"] = json!("Blending Softcopy Presentation State");
        fixture["dicom"]["sop_class_uid"] = json!("1.2.840.10008.5.1.4.1.1.11.4");
        fixture["expected_blending_presentation_state"] = json!({
            "same_study": true,
            "shared_frame_of_reference": true,
            "different_series": true,
            "sources": [{}, {}, {}, {}],
            "blending_items": [
                {
                    "blending_position": "UNDERLYING",
                    "rescale_intercept": -1024,
                    "rescale_slope": 1,
                    "rescale_type": "HU",
                    "softcopy_voi_lut_items": 0,
                    "referenced_spatial_registration_items": 0,
                    "referenced_frame_numbers": [],
                    "complete_instances": true
                },
                {
                    "blending_position": "SUPERIMPOSED",
                    "rescale_intercept": -1024,
                    "rescale_slope": 1,
                    "rescale_type": "HU",
                    "softcopy_voi_lut_items": 0,
                    "referenced_spatial_registration_items": 0,
                    "referenced_frame_numbers": [],
                    "complete_instances": true
                }
            ],
            "relative_opacity": 0.5,
            "displayed_area": {
                "items": 1,
                "applies_to_all_references": true,
                "top_left": [1, 1],
                "bottom_right": [2, 2],
                "presentation_size_mode": "SCALE TO FIT",
                "presentation_pixel_aspect_ratio": [1, 1]
            },
            "palette_color_lut": {
                "channels": [
                    {
                        "channel": "red",
                        "data_size_bytes": 512,
                        "data_sha256": "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5",
                        "storage": "identity_u16_little_endian"
                    },
                    {
                        "channel": "green",
                        "data_size_bytes": 512,
                        "data_sha256": "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5",
                        "storage": "identity_u16_little_endian"
                    },
                    {
                        "channel": "blue",
                        "data_size_bytes": 512,
                        "data_sha256": "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5",
                        "storage": "identity_u16_little_endian"
                    }
                ]
            },
            "icc_profile": {
                "size_bytes": 736,
                "sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
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
        });
        manifest["files"]
            .as_array_mut()
            .expect("manifest files")
            .push(fixture);
        manifest["skipped_cases"]
            .as_array_mut()
            .expect("manifest skipped cases")
            .retain(|case| case["case_id"] != Value::from("derived/presentation-state/blending"));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize report fixture manifest"),
        )
        .expect("write report fixture manifest");
    }

    let report =
        dicom_test_suite::build_coverage_report(&out_dir).expect("Blending coverage report");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "Blending report schema: {errors:?}");

    let row = coverage_row(&report, "derived/presentation-state/blending");
    for (field, expected) in [
        (
            "blending_presentation_state_kind",
            json!("Blending Softcopy Presentation State"),
        ),
        (
            "blending_sop_class_uid",
            json!("1.2.840.10008.5.1.4.1.1.11.4"),
        ),
        ("blending_source_series_count", json!(2)),
        ("blending_source_image_count", json!(4)),
        ("blending_source_closure", json!(true)),
        ("blending_positions", json!("UNDERLYING; SUPERIMPOSED")),
        ("blending_relative_opacity", json!("0.5")),
        ("blending_rescale_summary", json!("-1024/1/HU; -1024/1/HU")),
        ("blending_optional_transforms_absent", json!(true)),
        ("blending_palette_data_sizes_bytes", json!("512; 512; 512")),
        ("blending_icc_profile_size_bytes", json!(736)),
        ("blending_icc_color_space", json!("SRGB")),
        ("blending_forbidden_modules_absent", json!(true)),
        ("blending_pixel_data_absent", json!(true)),
        (
            "blending_unresolved_external_validator_findings",
            json!("none"),
        ),
    ] {
        assert_eq!(row[field], expected, "{field}");
    }
    for pointer in [
        "/grouped_coverage/blending_source_series_counts/2",
        "/grouped_coverage/blending_source_image_counts/4",
        "/grouped_coverage/blending_source_closure_states/true",
        "/grouped_coverage/blending_position_orders/UNDERLYING; SUPERIMPOSED",
        "/grouped_coverage/blending_relative_opacities/0.5",
        "/grouped_coverage/blending_optional_transforms_absent_states/true",
        "/grouped_coverage/blending_icc_profile_size_byte_counts/736",
        "/grouped_coverage/blending_forbidden_modules_absent_states/true",
        "/grouped_coverage/blending_pixel_data_absent_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&json!(1)), "{pointer}");
    }

    let mut incomplete = report.clone();
    coverage_row_mut(&mut incomplete, "derived/presentation-state/blending")["blending_palette_descriptors"] =
        Value::Null;
    assert!(!validator.is_valid(&incomplete));
    let mut leaked = report.clone();
    coverage_row_mut(
        &mut leaked,
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    )["blending_relative_opacity"] = json!("0.5");
    assert!(!validator.is_valid(&leaked));

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## Blending Softcopy Presentation State Expectations",
        "UNDERLYING; SUPERIMPOSED",
        "red:[256,0,16]",
        "identity_u16_little_endian",
        "### Blending Position Orders",
    ] {
        assert!(
            markdown.contains(expected),
            "markdown should contain {expected}"
        );
    }
    fs::remove_dir_all(out_dir).expect("remove report root");
}

#[test]
fn report_command_exposes_promoted_ecg_waveform_contracts() {
    let out_dir = unique_temp_dir("report-ecg-waveform-json");
    fs::create_dir_all(&out_dir).expect("create report fixture root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&waveform_report_manifest()).expect("serialize report fixture"),
    )
    .expect("write report fixture manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");
    assert!(
        output.status.success(),
        "report should accept waveform output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("report stdout JSON");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "waveform report schema errors: {errors:?}"
    );

    let row = coverage_row(&report, "non-image/waveform/twelve_lead_ecg");
    for (field, expected) in [
        ("waveform_group_count", 1),
        ("waveform_channel_count", 12),
        ("waveform_samples_per_channel", 500),
        ("waveform_sampling_frequency_hz", 500),
        ("waveform_duration_seconds", 1),
        ("waveform_bits_allocated", 16),
        ("waveform_bits_stored", 16),
        ("waveform_payload_length_bytes", 12_000),
        ("waveform_channel_hash_count", 12),
    ] {
        assert_eq!(row[field], expected, "{field}");
    }
    assert_eq!(row["waveform_iod_kind"], "twelve_lead_ecg");
    assert_eq!(row["waveform_group_shapes"], "RESTING_12_LEAD:12x500@500Hz");
    assert_eq!(
        row["waveform_group_channel_labels"],
        "RESTING_12_LEAD[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]"
    );
    assert_eq!(row["waveform_group_payload_lengths_bytes"], "12000");
    assert_eq!(
        row["waveform_group_payload_sha256_values"],
        "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
    );
    assert_eq!(row["waveform_total_channel_count"], 12);
    assert_eq!(row["waveform_total_payload_length_bytes"], 12_000);
    assert_eq!(
        row["waveform_aggregate_payload_sha256"],
        "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
    );
    assert_eq!(row["waveform_total_channel_hash_count"], 12);
    assert_eq!(row["waveform_all_groups_simultaneous_sampling"], true);
    assert_eq!(row["waveform_common_duration_seconds"], 1);
    assert_eq!(
        row["waveform_channel_labels"],
        "I; II; III; aVR; aVL; aVF; V1; V2; V3; V4; V5; V6"
    );
    assert!(
        row["waveform_channel_source_codes"]
            .as_str()
            .is_some_and(|value| value.starts_with("2:1|MDC|Lead I; 2:2|MDC|Lead II"))
    );
    assert_eq!(row["waveform_sample_interpretation"], "SS");
    assert_eq!(row["waveform_storage_vr"], "OW");
    assert_eq!(row["waveform_interleave_order"], "channel_then_sample");
    assert_eq!(row["waveform_simultaneous_sampling"], true);
    assert_eq!(row["waveform_pixel_data_absent"], true);
    assert_eq!(
        row["waveform_payload_sha256"],
        "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
    );
    assert_eq!(
        row["waveform_external_validator_disposition"],
        "external conformance evidence not embedded; run conformance separately"
    );
    for pointer in [
        "/grouped_coverage/waveform_iod_kinds/twelve_lead_ecg",
        "/grouped_coverage/waveform_group_shape_orders/RESTING_12_LEAD:12x500@500Hz",
        "/grouped_coverage/waveform_total_channel_counts/12",
        "/grouped_coverage/waveform_total_payload_lengths_bytes/12000",
        "/grouped_coverage/waveform_aggregate_payload_sha256_values/98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        "/grouped_coverage/waveform_total_channel_hash_counts/12",
        "/grouped_coverage/waveform_common_durations_seconds/1",
        "/grouped_coverage/waveform_channel_counts/12",
        "/grouped_coverage/waveform_sampling_frequencies_hz/500",
        "/grouped_coverage/waveform_payload_length_bytes/12000",
        "/grouped_coverage/waveform_channel_hash_counts/12",
        "/grouped_coverage/waveform_simultaneous_sampling_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    for pointer in [
        "/grouped_coverage/waveform_all_groups_simultaneous_sampling_states/true",
        "/grouped_coverage/waveform_pixel_data_absent_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(2)), "{pointer}");
    }

    let general = coverage_row(&report, "non-image/waveform/general_ecg");
    assert_general_ecg_waveform_row(general);
    for pointer in [
        "/grouped_coverage/waveform_iod_kinds/general_ecg",
        "/grouped_coverage/waveform_group_counts/2",
        "/grouped_coverage/waveform_group_shape_orders/12x1000@250Hz; 4x4000@1000Hz",
        "/grouped_coverage/waveform_total_channel_counts/16",
        "/grouped_coverage/waveform_total_channel_hash_counts/16",
        "/grouped_coverage/waveform_total_payload_lengths_bytes/56000",
        "/grouped_coverage/waveform_aggregate_payload_sha256_values/c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea",
        "/grouped_coverage/waveform_common_durations_seconds/4",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }

    let planned = coverage_row(&report, "non-image/rt/dose_grid_u16_explicit_le");
    assert_eq!(planned["status"], "planned");
    for field in [
        "waveform_iod_kind",
        "waveform_payload_sha256",
        "waveform_group_shapes",
        "waveform_group_channel_labels",
        "waveform_group_channel_source_codes",
        "waveform_group_payload_lengths_bytes",
        "waveform_group_payload_sha256_values",
        "waveform_total_channel_count",
        "waveform_total_payload_length_bytes",
        "waveform_aggregate_payload_sha256",
        "waveform_total_channel_hash_count",
        "waveform_all_groups_simultaneous_sampling",
        "waveform_common_duration_seconds",
    ] {
        assert!(
            planned[field].is_null(),
            "a truly planned non-waveform case must not leak {field}"
        );
    }

    let mut incomplete = report.clone();
    coverage_row_mut(&mut incomplete, "non-image/waveform/twelve_lead_ecg")["waveform_channel_hash_count"] =
        Value::Null;
    assert!(
        !validator.is_valid(&incomplete),
        "coverage schema must reject a partial Twelve-lead waveform contract"
    );
    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "non-image/waveform/general_ecg")["waveform_iod_kind"] =
        Value::from("twelve_lead_ecg");
    assert!(
        !validator.is_valid(&leaked),
        "coverage schema must reject waveform coverage hidden on another case"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## Waveform Expectations",
        "RESTING_12_LEAD:12x500@500Hz",
        "RESTING_12_LEAD[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]",
        "I; II; III; aVR; aVL; aVF; V1; V2; V3; V4; V5; V6",
        "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        "12x1000@250Hz; 4x4000@1000Hz",
        "STD12_250HZ[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]",
        "AUX4_1000HZ[A1, A2, A3, A4]",
        "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea",
        "### Waveform IOD Kinds",
        "### Waveform External Validator Dispositions",
    ] {
        assert!(
            markdown.contains(expected),
            "markdown should contain {expected}"
        );
    }
    fs::remove_dir_all(out_dir).expect("remove report root");
}

fn assert_general_ecg_waveform_row(row: &Value) {
    assert_eq!(row["status"], "generated");
    assert_eq!(row["waveform_iod_kind"], "general_ecg");
    assert_eq!(row["waveform_group_count"], 2);
    assert_eq!(row["waveform_group_shapes"], "12x1000@250Hz; 4x4000@1000Hz");
    assert_eq!(
        row["waveform_group_channel_labels"],
        "STD12_250HZ[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]; AUX4_1000HZ[A1, A2, A3, A4]"
    );
    assert!(
        row["waveform_group_channel_source_codes"]
            .as_str()
            .is_some_and(|value| {
                value.starts_with("STD12_250HZ[2:1|MDC|Lead I")
                    && value.contains("; AUX4_1000HZ[2:75|MDC|Auxiliary unipolar lead 1")
                    && value.ends_with("2:78|MDC|Auxiliary unipolar lead 4]")
            })
    );
    assert_eq!(row["waveform_group_payload_lengths_bytes"], "24000; 32000");
    assert_eq!(
        row["waveform_group_payload_sha256_values"],
        "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb; 5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe"
    );
    assert_eq!(row["waveform_total_channel_count"], 16);
    assert_eq!(row["waveform_total_channel_hash_count"], 16);
    assert_eq!(row["waveform_total_payload_length_bytes"], 56_000);
    assert_eq!(
        row["waveform_aggregate_payload_sha256"],
        "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea"
    );
    assert_eq!(row["waveform_all_groups_simultaneous_sampling"], true);
    assert_eq!(row["waveform_common_duration_seconds"], 4);
    assert_eq!(row["waveform_pixel_data_absent"], true);
    for field in [
        "waveform_channel_count",
        "waveform_samples_per_channel",
        "waveform_sampling_frequency_hz",
        "waveform_duration_seconds",
        "waveform_channel_labels",
        "waveform_channel_source_codes",
        "waveform_bits_allocated",
        "waveform_bits_stored",
        "waveform_sample_interpretation",
        "waveform_storage_vr",
        "waveform_payload_length_bytes",
        "waveform_payload_sha256",
        "waveform_interleave_order",
        "waveform_channel_hash_count",
        "waveform_simultaneous_sampling",
    ] {
        assert!(
            row[field].is_null(),
            "heterogeneous groups must not expose {field}"
        );
    }
}

#[test]
fn report_locks_general_ecg_group_contract() {
    let out_dir = unique_temp_dir("report-general-ecg-groups");
    fs::create_dir_all(&out_dir).expect("create report fixture root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&general_ecg_report_manifest()).expect("serialize fixture"),
    )
    .expect("write report fixture manifest");

    let report = dicom_test_suite::build_coverage_report(&out_dir).expect("General ECG report");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "General ECG schema errors: {errors:?}");

    let row = coverage_row(&report, "non-image/waveform/general_ecg");
    assert_eq!(row["status"], "generated");
    assert_eq!(row["waveform_iod_kind"], "general_ecg");
    assert_eq!(row["waveform_group_count"], 2);
    assert_eq!(row["waveform_group_shapes"], "12x1000@250Hz; 4x4000@1000Hz");
    assert_eq!(
        row["waveform_group_channel_labels"],
        "STD12_250HZ[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]; AUX4_1000HZ[A1, A2, A3, A4]"
    );
    assert!(
        row["waveform_group_channel_source_codes"]
            .as_str()
            .is_some_and(|value| {
                value.starts_with("STD12_250HZ[2:1|MDC|Lead I")
                    && value.ends_with("2:78|MDC|Auxiliary unipolar lead 4]")
            })
    );
    assert_eq!(row["waveform_group_payload_lengths_bytes"], "24000; 32000");
    assert_eq!(
        row["waveform_group_payload_sha256_values"],
        "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb; 5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe"
    );
    assert_eq!(row["waveform_total_channel_count"], 16);
    assert_eq!(row["waveform_total_channel_hash_count"], 16);
    assert_eq!(row["waveform_total_payload_length_bytes"], 56_000);
    assert_eq!(
        row["waveform_aggregate_payload_sha256"],
        "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea"
    );
    assert_eq!(row["waveform_all_groups_simultaneous_sampling"], true);
    assert_eq!(row["waveform_common_duration_seconds"], 4);
    assert_eq!(row["waveform_pixel_data_absent"], true);
    for field in [
        "waveform_channel_count",
        "waveform_samples_per_channel",
        "waveform_sampling_frequency_hz",
        "waveform_duration_seconds",
        "waveform_channel_labels",
        "waveform_channel_source_codes",
        "waveform_bits_allocated",
        "waveform_bits_stored",
        "waveform_sample_interpretation",
        "waveform_storage_vr",
        "waveform_payload_length_bytes",
        "waveform_payload_sha256",
        "waveform_interleave_order",
        "waveform_channel_hash_count",
        "waveform_simultaneous_sampling",
    ] {
        assert!(
            row[field].is_null(),
            "heterogeneous groups must not expose {field}"
        );
    }

    for pointer in [
        "/grouped_coverage/waveform_iod_kinds/general_ecg",
        "/grouped_coverage/waveform_group_counts/2",
        "/grouped_coverage/waveform_group_shape_orders/12x1000@250Hz; 4x4000@1000Hz",
        "/grouped_coverage/waveform_total_channel_counts/16",
        "/grouped_coverage/waveform_total_channel_hash_counts/16",
        "/grouped_coverage/waveform_total_payload_lengths_bytes/56000",
        "/grouped_coverage/waveform_aggregate_payload_sha256_values/c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea",
        "/grouped_coverage/waveform_all_groups_simultaneous_sampling_states/true",
        "/grouped_coverage/waveform_common_durations_seconds/4",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }

    let mut partial = report.clone();
    coverage_row_mut(&mut partial, "non-image/waveform/general_ecg")["waveform_total_channel_hash_count"] =
        Value::Null;
    assert!(
        !validator.is_valid(&partial),
        "partial General contract must fail"
    );
    let mut tampered = report.clone();
    coverage_row_mut(&mut tampered, "non-image/waveform/general_ecg")["waveform_aggregate_payload_sha256"] =
        Value::from("0".repeat(64));
    assert!(
        !validator.is_valid(&tampered),
        "tampered aggregate hash must fail"
    );
    let mut wrong_iod = report.clone();
    coverage_row_mut(&mut wrong_iod, "non-image/waveform/general_ecg")["waveform_iod_kind"] =
        Value::from("twelve_lead_ecg");
    assert!(
        !validator.is_valid(&wrong_iod),
        "General waveform fields require the General ECG IOD kind"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "12x1000@250Hz; 4x4000@1000Hz",
        "STD12_250HZ[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]",
        "AUX4_1000HZ[A1, A2, A3, A4]",
        "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }
    fs::remove_dir_all(out_dir).expect("remove report root");
}

#[test]
fn report_locks_linked_rt_plan_contract_and_markdown() {
    let out_dir = unique_temp_dir("report-linked-rt-plan");
    fs::create_dir_all(&out_dir).expect("create RT Plan report root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&linked_rt_plan_report_manifest()).expect("serialize fixture"),
    )
    .expect("write RT Plan manifest");

    let report = dicom_test_suite::build_coverage_report(&out_dir).expect("RT Plan report");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "RT Plan schema errors: {errors:?}");

    let row = coverage_row(&report, "non-image/rt/plan_linked");
    for (field, expected) in [
        ("rt_plan_label", "DTS_PLAN"),
        ("rt_plan_geometry", "PATIENT"),
        ("rt_plan_fraction_group_numbers", "1"),
        ("rt_plan_beam_numbers", "1"),
        ("rt_plan_beam_names", "DTS_STATIC_AP"),
        ("rt_plan_beam_types", "STATIC"),
        ("rt_plan_radiation_types", "PHOTON"),
        ("rt_plan_control_point_indices", "0; 1"),
        ("rt_plan_meterset_range", "0..1"),
    ] {
        assert_eq!(row[field], expected, "{field}");
    }
    assert_eq!(row["rt_plan_fraction_group_count"], 1);
    assert_eq!(row["rt_plan_beam_count"], 1);
    assert_eq!(row["rt_plan_control_point_count"], 2);
    assert_eq!(row["rt_plan_reference_closure"], true);
    assert_eq!(row["rt_plan_pixel_data_absent"], true);
    assert_eq!(
        row["rt_plan_external_validator_disposition"],
        "external conformance evidence not embedded; run conformance separately"
    );
    let structure_identity = row["rt_plan_structure_set_reference_identity"]
        .as_str()
        .expect("Structure Set identity");
    assert!(structure_identity.starts_with(
        "non-image/rt/structure_set_single_roi_explicit_le|non-image/rt/structure_set_single_roi_explicit_le/instance.dcm|aaaaaaaa"
    ));
    assert!(structure_identity.contains("class=1.2.840.10008.5.1.4.1.1.481.3"));
    let dose_identity = row["rt_plan_dose_reference_identity"]
        .as_str()
        .expect("Dose identity");
    assert!(dose_identity.starts_with(
        "non-image/rt/dose_grid_u16_explicit_le|non-image/rt/dose_grid_u16_explicit_le/instance.dcm|bbbbbbbb"
    ));
    assert!(dose_identity.contains("class=1.2.840.10008.5.1.4.1.1.481.2"));

    for pointer in [
        "/grouped_coverage/rt_plan_labels/DTS_PLAN",
        "/grouped_coverage/rt_plan_geometries/PATIENT",
        "/grouped_coverage/rt_plan_fraction_group_counts/1",
        "/grouped_coverage/rt_plan_beam_counts/1",
        "/grouped_coverage/rt_plan_beam_name_orders/DTS_STATIC_AP",
        "/grouped_coverage/rt_plan_beam_type_orders/STATIC",
        "/grouped_coverage/rt_plan_radiation_type_orders/PHOTON",
        "/grouped_coverage/rt_plan_control_point_counts/2",
        "/grouped_coverage/rt_plan_control_point_index_orders/0; 1",
        "/grouped_coverage/rt_plan_meterset_ranges/0..1",
        "/grouped_coverage/rt_plan_reference_closure_states/true",
        "/grouped_coverage/rt_plan_pixel_data_absent_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }

    let mut partial = report.clone();
    coverage_row_mut(&mut partial, "non-image/rt/plan_linked")["rt_plan_control_point_count"] =
        Value::Null;
    assert!(
        !validator.is_valid(&partial),
        "partial Plan coverage must fail"
    );
    let mut tampered = report.clone();
    coverage_row_mut(&mut tampered, "non-image/rt/plan_linked")["rt_plan_beam_types"] =
        Value::from("DYNAMIC");
    assert!(
        !validator.is_valid(&tampered),
        "tampered Plan coverage must fail"
    );
    let mut leaked = report.clone();
    let leaked_row = coverage_row_mut(&mut leaked, "non-image/rt/plan_linked");
    leaked_row["case_id"] = Value::from("non-image/rt/dose_grid_u16_explicit_le");
    assert!(!validator.is_valid(&leaked), "Plan coverage must not leak");

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## Linked RT Plan Expectations",
        "DTS_PLAN",
        "PATIENT",
        "DTS_STATIC_AP",
        "STATIC / PHOTON",
        "0; 1 / 0..1",
        "non-image/rt/structure_set_single_roi_explicit_le",
        "non-image/rt/dose_grid_u16_explicit_le",
        "### RT Plan Reference Closure States",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }
    fs::remove_dir_all(out_dir).expect("remove RT Plan report root");
}

#[test]
fn report_keeps_planned_rt_plan_coverage_null() {
    let out_dir = unique_temp_dir("report-planned-rt-plan");
    fs::create_dir_all(&out_dir).expect("create planned RT Plan root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "generated_at": "20260101000000.000000+0000",
            "standards": { "standards_lock_sha256": "0".repeat(64) },
            "run": { "profile": "extended" },
            "files": [],
            "skipped_cases": [{
                "case_id": "non-image/rt/plan_linked",
                "status": "unavailable",
                "reason_code": "case_planned",
                "message": "recipe_unimplemented"
            }]
        }))
        .expect("serialize planned fixture"),
    )
    .expect("write planned fixture");
    let report = dicom_test_suite::build_coverage_report(&out_dir).expect("planned Plan report");
    let row = coverage_row(&report, "non-image/rt/plan_linked");
    assert_eq!(row["status"], "planned");
    for field in RT_PLAN_REPORT_FIELDS {
        assert!(row[*field].is_null(), "planned row leaked {field}");
    }
    fs::remove_dir_all(out_dir).expect("remove planned Plan root");
}

#[test]
fn report_locks_linked_rt_image_contract_and_markdown() {
    let out_dir = unique_temp_dir("report-linked-rt-image");
    fs::create_dir_all(&out_dir).expect("create RT Image report root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&linked_rt_image_report_manifest()).expect("serialize fixture"),
    )
    .expect("write RT Image manifest");

    let report = dicom_test_suite::build_coverage_report(&out_dir).expect("RT Image report");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "RT Image schema errors: {errors:?}");

    let row = coverage_row(&report, "non-image/rt/image_linked");
    for (field, expected) in [
        ("rt_image_type", "DERIVED\\SECONDARY\\DRR"),
        ("rt_image_label", "DTS_DRR"),
        ("rt_image_plane", "NORMAL"),
        ("rt_image_pixel_spacing_mm", "1\\1"),
        ("rt_image_position_mm", "-1.5\\1.5"),
        ("rt_image_dimensions", "4x4x1"),
        ("rt_image_bit_contract", "8/8/7/u0"),
        (
            "rt_image_payload_sha256",
            "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811",
        ),
        (
            "rt_image_pixel_disposition",
            "native OB / 16 bytes / 0 padding bytes",
        ),
        (
            "rt_image_external_validator_disposition",
            "external conformance evidence not embedded; run conformance separately",
        ),
    ] {
        assert_eq!(row[field], expected, "{field}");
    }
    assert_eq!(row["rt_image_referenced_beam_number"], 1);
    assert_eq!(row["rt_image_referenced_fraction_group_number"], 1);
    assert_eq!(row["rt_image_radiation_machine_sad_mm"], 1000);
    assert_eq!(row["rt_image_sid_mm"], 1500);
    assert_eq!(row["rt_image_reference_closure"], true);
    let plan_identity = row["rt_image_plan_reference_identity"]
        .as_str()
        .expect("Plan identity");
    assert!(
        plan_identity
            .starts_with("non-image/rt/plan_linked|non-image/rt/plan_linked/instance.dcm|cccccccc")
    );
    assert!(plan_identity.contains("class=1.2.840.10008.5.1.4.1.1.481.5"));

    for pointer in [
        "/grouped_coverage/rt_image_types/DERIVED\\SECONDARY\\DRR",
        "/grouped_coverage/rt_image_labels/DTS_DRR",
        "/grouped_coverage/rt_image_planes/NORMAL",
        "/grouped_coverage/rt_image_pixel_spacings_mm/1\\1",
        "/grouped_coverage/rt_image_positions_mm/-1.5\\1.5",
        "/grouped_coverage/rt_image_dimensions/4x4x1",
        "/grouped_coverage/rt_image_bit_contracts/8~18~17~1u0",
        "/grouped_coverage/rt_image_referenced_beam_numbers/1",
        "/grouped_coverage/rt_image_referenced_fraction_group_numbers/1",
        "/grouped_coverage/rt_image_radiation_machine_sad_values_mm/1000",
        "/grouped_coverage/rt_image_sid_values_mm/1500",
        "/grouped_coverage/rt_image_reference_closure_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }

    let mut partial = report.clone();
    coverage_row_mut(&mut partial, "non-image/rt/image_linked")["rt_image_payload_sha256"] =
        Value::Null;
    assert!(
        !validator.is_valid(&partial),
        "partial Image coverage must fail"
    );
    let mut tampered = report.clone();
    coverage_row_mut(&mut tampered, "non-image/rt/image_linked")["rt_image_sid_mm"] =
        Value::from(1499);
    assert!(
        !validator.is_valid(&tampered),
        "tampered Image coverage must fail"
    );
    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "non-image/rt/image_linked")["case_id"] =
        Value::from("non-image/rt/plan_linked");
    assert!(!validator.is_valid(&leaked), "Image coverage must not leak");

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## Linked RT Image Expectations",
        "DERIVED\\SECONDARY\\DRR",
        "DTS_DRR / NORMAL",
        "1\\1 / -1.5\\1.5",
        "4x4x1 / 8/8/7/u0",
        "non-image/rt/plan_linked",
        "1000 / 1500",
        "native OB / 16 bytes / 0 padding bytes",
        "### RT Image Reference Closure States",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }
    fs::remove_dir_all(out_dir).expect("remove RT Image report root");
}

#[test]
fn report_keeps_planned_rt_image_coverage_null() {
    let out_dir = unique_temp_dir("report-planned-rt-image");
    fs::create_dir_all(&out_dir).expect("create planned RT Image root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "generated_at": "20260101000000.000000+0000",
            "standards": { "standards_lock_sha256": "0".repeat(64) },
            "run": { "profile": "extended" },
            "files": [],
            "skipped_cases": [{
                "case_id": "non-image/rt/image_linked",
                "status": "unavailable",
                "reason_code": "case_planned",
                "message": "recipe_unimplemented"
            }]
        }))
        .expect("serialize planned fixture"),
    )
    .expect("write planned fixture");
    let report = dicom_test_suite::build_coverage_report(&out_dir).expect("planned Image report");
    let row = coverage_row(&report, "non-image/rt/image_linked");
    assert_eq!(row["status"], "planned");
    for field in RT_IMAGE_REPORT_FIELDS {
        assert!(row[*field].is_null(), "planned row leaked {field}");
    }
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema should compile");
    assert!(
        validator.is_valid(&report),
        "planned Image report must validate"
    );
    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "non-image/rt/image_linked")["rt_image_label"] =
        Value::from("DTS_DRR");
    assert!(
        !validator.is_valid(&leaked),
        "planned Image coverage must reject leaked generated fields"
    );
    fs::remove_dir_all(out_dir).expect("remove planned Image root");
}

#[test]
fn report_exposes_locked_single_frame_vl_rows_and_markdown() {
    let out_dir = unique_temp_dir("report-vl-single-frame");
    fs::create_dir_all(&out_dir).expect("create VL report root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&vl_single_frame_report_manifest())
            .expect("serialize VL report manifest"),
    )
    .expect("write VL report manifest");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("single-frame VL coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema compiles");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "VL report must match schema: {errors:?}");

    for (case_id, sop_uid, sop_name, iod, modality, body_part, stressor) in [
        (
            "vl/endoscopic/rgb_explicit_le",
            "1.2.840.10008.5.1.4.1.1.77.1.1",
            "VL Endoscopic Image Storage",
            "VL Endoscopic Image",
            "ES",
            "LUNG",
            "vl_endoscopic_image_storage",
        ),
        (
            "vl/microscopic/rgb_explicit_le",
            "1.2.840.10008.5.1.4.1.1.77.1.2",
            "VL Microscopic Image Storage",
            "VL Microscopic Image",
            "GM",
            "EYE",
            "vl_microscopic_image_storage",
        ),
    ] {
        let row = coverage_row(&report, case_id);
        assert_eq!(row["status"], "generated");
        assert_eq!(row["sop_class_uid"], sop_uid);
        assert_eq!(row["sop_class_name"], sop_name);
        assert_eq!(row["iod"], iod);
        assert_eq!(row["modality"], modality);
        assert_eq!(row["body_part_examined"], body_part);
        assert_eq!(row["laterality"], "R");
        assert_eq!(row["image_type"], "ORIGINAL\\PRIMARY");
        assert_eq!(row["photometric"], "RGB");
        assert_eq!(row["samples_per_pixel"], 3);
        assert_eq!(row["frames"], 1);
        assert_eq!(row.pointer("/geometry/rows"), Some(&Value::from(2)));
        assert_eq!(row.pointer("/geometry/columns"), Some(&Value::from(2)));
        assert_eq!(
            row["known_stressors"],
            json!([stressor, "vl_rgb_pixels", "native_ob_pixel_data"])
        );
    }
    assert_eq!(
        report.pointer("/grouped_coverage/lateralities/R"),
        Some(&Value::from(2))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/body_parts_examined/LUNG"),
        Some(&Value::from(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/body_parts_examined/EYE"),
        Some(&Value::from(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/image_types/ORIGINAL\\PRIMARY"),
        Some(&Value::from(2))
    );
    let mut missing_laterality = report.clone();
    coverage_row_mut(&mut missing_laterality, "vl/endoscopic/rgb_explicit_le")["laterality"] =
        Value::Null;
    assert!(
        !validator.is_valid(&missing_laterality),
        "VL row must not hide its locked Laterality"
    );
    let mut leaked_laterality = report.clone();
    coverage_row_mut(&mut leaked_laterality, "vl/endoscopic/rgb_explicit_le")["case_id"] =
        Value::from("vl/photo/rgb_planar0_explicit_le");
    assert!(
        !validator.is_valid(&leaked_laterality),
        "non-milestone row must not leak the locked VL Laterality"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "### Lateralities",
        "| R | 2 |",
        "### Body Parts Examined",
        "| LUNG | 1 |",
        "| EYE | 1 |",
        "### Image Types",
        "ORIGINAL\\PRIMARY",
        "vl_endoscopic_image_storage",
        "vl_microscopic_image_storage",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }

    fs::remove_dir_all(out_dir).expect("remove VL report root");
}

#[test]
fn report_exposes_locked_single_frame_vl_planned_rows() {
    let out_dir = unique_temp_dir("report-vl-single-frame-planned");
    fs::create_dir_all(&out_dir).expect("create planned VL report root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "generated_at": "20260101000000.000000+0000",
            "standards": { "standards_lock_sha256": "0".repeat(64) },
            "run": { "profile": "extended" },
            "files": [],
            "skipped_cases": [
                { "case_id": "vl/endoscopic/rgb_explicit_le", "status": "unavailable", "reason_code": "case_planned", "message": "recipe_unimplemented" },
                { "case_id": "vl/microscopic/rgb_explicit_le", "status": "unavailable", "reason_code": "case_planned", "message": "recipe_unimplemented" }
            ]
        }))
        .expect("serialize planned VL manifest"),
    )
    .expect("write planned VL manifest");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("planned VL coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema compiles");
    assert!(
        validator.is_valid(&report),
        "planned VL coverage report must match its schema"
    );
    for (case_id, sop_uid, sop_name, iod, modality, body_part, stressor) in [
        (
            "vl/endoscopic/rgb_explicit_le",
            "1.2.840.10008.5.1.4.1.1.77.1.1",
            "VL Endoscopic Image Storage",
            "VL Endoscopic Image",
            "ES",
            "LUNG",
            "vl_endoscopic_image_storage",
        ),
        (
            "vl/microscopic/rgb_explicit_le",
            "1.2.840.10008.5.1.4.1.1.77.1.2",
            "VL Microscopic Image Storage",
            "VL Microscopic Image",
            "GM",
            "EYE",
            "vl_microscopic_image_storage",
        ),
    ] {
        let row = coverage_row(&report, case_id);
        assert_eq!(row["status"], "planned");
        assert_eq!(row["sop_class_uid"], sop_uid);
        assert_eq!(row["sop_class_name"], sop_name);
        assert_eq!(row["iod"], iod);
        assert_eq!(row["modality"], modality);
        assert_eq!(row["body_part_examined"], body_part);
        assert_eq!(row["laterality"], "R");
        assert_eq!(row["image_type"], "ORIGINAL\\PRIMARY");
        assert_eq!(row["photometric"], "RGB");
        assert_eq!(row.pointer("/geometry/rows"), Some(&Value::from(2)));
        assert_eq!(row.pointer("/geometry/columns"), Some(&Value::from(2)));
        assert_eq!(row["known_stressors"], json!([stressor, "vl_rgb_pixels"]));
    }
    fs::remove_dir_all(out_dir).expect("remove planned VL report root");
}

#[test]
fn report_exposes_locked_tiled_full_wsi_plan_without_claiming_generation() {
    let out_dir = unique_temp_dir("report-wsi-tiled-full-planned");
    fs::create_dir_all(&out_dir).expect("create planned WSI report root");
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "generated_at": "20260101000000.000000+0000",
            "standards": { "standards_lock_sha256": "0".repeat(64) },
            "run": { "profile": "extended" },
            "files": [],
            "skipped_cases": [{
                "case_id": "vl/wsi/tiled_full_small",
                "status": "unavailable",
                "reason_code": "case_planned",
                "message": "recipe_unimplemented"
            }]
        }))
        .expect("serialize planned WSI manifest"),
    )
    .expect("write planned WSI manifest");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("planned WSI coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema compiles");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "planned WSI coverage report must match its schema: {errors:#?}"
    );

    let row = coverage_row(&report, "vl/wsi/tiled_full_small");
    assert_eq!(row["status"], "planned");
    assert_eq!(row["validation_status"], "unavailable");
    assert_eq!(row["determinism"], "byte_stable");
    assert_eq!(row["wsi_iod_kind"], "vl_wsi_tiled_full");
    assert_eq!(row["wsi_dimension_organization_type"], "TILED_FULL");
    assert_eq!(
        row["wsi_tile_geometry"],
        "2x2 tiles; 4x4 total matrix; 2x2 tile grid; 4 frames"
    );
    assert_eq!(
        row["wsi_implicit_frame_order"],
        "1:(1,1); 2:(3,1); 3:(1,3); 4:(3,3)"
    );
    assert_eq!(
        row["wsi_total_pixel_matrix_sha256"],
        "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a"
    );
    assert_eq!(
        row["wsi_specimen_identity"],
        "DTS-SLIDE-001/DTS-SPECIMEN-001"
    );
    assert_eq!(
        row["wsi_optical_path_icc_sha256"],
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
    );
    assert_eq!(row["wsi_implicit_position_reconstruction"], true);
    assert_eq!(row["wsi_sparse_dimension_metadata_absent"], true);
    assert_eq!(row["wsi_reference_free"], true);
    assert_eq!(
        row["known_stressors"],
        json!([
            "vl_whole_slide_microscopy_image_storage",
            "tiled_full_implicit_frame_order",
            "total_pixel_matrix_reconstruction",
            "specimen_and_optical_path_metadata",
            "nested_icc_profile",
            "absent_per_frame_functional_groups"
        ])
    );

    let mut corrupted = report.clone();
    coverage_row_mut(&mut corrupted, "vl/wsi/tiled_full_small")["wsi_total_pixel_matrix_sha256"] =
        Value::from("a".repeat(64));
    assert!(
        !validator.is_valid(&corrupted),
        "schema must reject a corrupted locked WSI reconstruction hash"
    );
    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "vl/wsi/tiled_full_small")["case_id"] =
        Value::from("vl/wsi/tiled_sparse_small");
    assert!(
        !validator.is_valid(&leaked),
        "schema must reject WSI report fields outside the locked case"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## Whole Slide Microscopy Expectations",
        "vl/wsi/tiled_full_small",
        "TILED_FULL",
        "DTS-SLIDE-001/DTS-SPECIMEN-001",
        "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }

    fs::remove_dir_all(out_dir).expect("remove planned WSI report root");
}

#[test]
fn report_exposes_generated_tiled_sparse_wsi_and_rejects_field_leakage() {
    let out_dir = unique_temp_dir("report-wsi-tiled-sparse-generated");
    generate_extended(&out_dir);

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("generated sparse WSI coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema compiles");
    assert!(
        validator.is_valid(&report),
        "generated sparse WSI coverage report must match its schema: {:?}",
        validator
            .iter_errors(&report)
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
    );

    let row = coverage_row(&report, "vl/wsi/tiled_sparse_small");
    assert_eq!(row["status"], "generated");
    assert_eq!(row["validation_status"], "passed");
    assert_eq!(row["frames"], 2);
    assert_eq!(row["wsi_iod_kind"], "vl_wsi_tiled_sparse");
    assert_eq!(row["wsi_dimension_organization_type"], "TILED_SPARSE");
    assert_eq!(row["wsi_explicit_frame_positions"], "1:(1,1); 2:(3,3)");
    assert_eq!(row["wsi_dimension_index_values"], "1:[1,1]; 2:[2,2]");
    assert_eq!(row["wsi_occupancy_mask"], "present,absent,absent,present");
    assert_eq!(row["wsi_absent_tile_positions"], "(3,1); (1,3)");
    assert_eq!(
        row["wsi_pixel_payload_sha256"],
        "94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee"
    );
    assert_eq!(
        row["wsi_sentinel_matrix_sha256"],
        "d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3"
    );
    assert_eq!(row["wsi_implicit_position_reconstruction"], false);
    assert_eq!(row["wsi_explicit_position_reconstruction"], true);
    assert_eq!(row["wsi_sparse_dimension_metadata_absent"], false);
    assert_eq!(row["wsi_reference_free"], true);
    assert_eq!(
        row["wsi_specimen_identity"],
        "DTS-SLIDE-001/DTS-SPECIMEN-001"
    );
    assert_eq!(
        row["wsi_optical_path_icc_sha256"],
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef"
    );

    let mut corrupted = report.clone();
    coverage_row_mut(&mut corrupted, "vl/wsi/tiled_sparse_small")["wsi_sentinel_matrix_sha256"] =
        Value::from("a".repeat(64));
    assert!(
        !validator.is_valid(&corrupted),
        "schema must reject a corrupted sparse sentinel reconstruction hash"
    );
    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "vl/wsi/tiled_sparse_small")["case_id"] =
        Value::from("vl/wsi/tiled_full_small");
    assert!(
        !validator.is_valid(&leaked),
        "schema must reject sparse WSI report fields on the tiled-full case"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "vl/wsi/tiled_sparse_small",
        "TILED_SPARSE",
        "present,absent,absent,present",
        "94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee",
        "d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }

    fs::remove_dir_all(out_dir).expect("remove planned sparse WSI report root");
}

#[test]
fn report_exposes_generated_wsi_tile_segmentation_closure() {
    let out_dir = unique_temp_dir("report-wsi-tile-segmentation-generated");
    generate_extended(&out_dir);

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("generated WSI tile segmentation coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema compiles");
    assert!(
        validator.is_valid(&report),
        "generated WSI tile segmentation report must match its schema: {:?}",
        validator
            .iter_errors(&report)
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
    );

    let row = coverage_row(&report, "derived/seg/wsi_tile_reference");
    if row["status"] == "unavailable" {
        assert_eq!(row["validation_status"], "unavailable");
        assert!(row["wsi_tile_seg_source_frame_mapping"].is_null());
        assert!(row["wsi_tile_seg_reference_closure"].is_null());
        fs::remove_dir_all(out_dir).expect("remove unavailable WSI tile segmentation report root");
        return;
    }
    assert_eq!(row["status"], "generated");
    assert_eq!(row["validation_status"], "passed");
    assert_eq!(
        row["wsi_tile_seg_source_frame_mapping"],
        "SEG1->WSI1; SEG2->WSI4"
    );
    assert_eq!(
        row["wsi_tile_seg_payload_sha256"],
        "74fa7cbb10160e0eb1f16f35fa9ad0e7f2712af56019996e88cf1034be92635e"
    );
    assert_eq!(
        row["wsi_tile_seg_reconstructed_matrix_sha256"],
        "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467"
    );
    assert_eq!(row["wsi_tile_seg_reference_closure"], true);
    assert_eq!(row["wsi_tile_seg_internal_validation_closure"], true);
    assert_eq!(row["wsi_tile_seg_budget_closure"], true);
    assert!(
        row["wsi_tile_seg_actual_dicom_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes <= 16_384)
    );
    assert!(
        row["wsi_tile_seg_observed_generation_milliseconds"]
            .as_u64()
            .is_some_and(|milliseconds| milliseconds <= 5_000)
    );

    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "derived/seg/wsi_tile_reference")["case_id"] =
        Value::from("derived/seg/fractional");
    assert!(
        !validator.is_valid(&leaked),
        "schema must reject WSI tile segmentation fields on another case"
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    for expected in [
        "## WSI Tile Segmentation Expectations",
        "derived/seg/wsi_tile_reference",
        "SEG1->WSI1; SEG2->WSI4",
        "74fa7cbb10160e0eb1f16f35fa9ad0e7f2712af56019996e88cf1034be92635e",
        "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467",
    ] {
        assert!(
            markdown.contains(expected),
            "Markdown must contain {expected}"
        );
    }

    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path).expect("read generated WSI tile segmentation manifest"),
    )
    .expect("parse generated WSI tile segmentation manifest");
    let segmentation = manifest["files"]
        .as_array_mut()
        .expect("manifest files")
        .iter_mut()
        .find(|file| file["case_id"] == "derived/seg/wsi_tile_reference")
        .expect("generated WSI tile segmentation entry");
    segmentation["validation"]["internal"]
        .as_array_mut()
        .expect("internal findings")
        .retain(|finding| finding["name"] != "wsi_tile_seg_per_frame_items");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize mutated manifest"),
    )
    .expect("write mutated manifest");
    let error = dicom_test_suite::build_coverage_report(&out_dir)
        .expect_err("report must not claim closure without strict graph evidence")
        .to_string();
    assert!(
        error.contains("closed internal validation evidence"),
        "{error}"
    );

    fs::remove_dir_all(out_dir).expect("remove generated WSI tile segmentation report root");
}

#[test]
fn report_rejects_partial_and_wrong_case_single_frame_vl_contracts() {
    let out_dir = unique_temp_dir("report-vl-single-frame-malformed");
    fs::create_dir_all(&out_dir).expect("create malformed VL report root");
    let manifest_path = out_dir.join("manifest.json");
    let original = vl_single_frame_report_manifest();
    let reject = |label: &str, manifest: &Value| {
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(manifest).expect("serialize malformed VL manifest"),
        )
        .expect("write malformed VL manifest");
        assert!(
            dicom_test_suite::build_coverage_report(&out_dir).is_err(),
            "report must reject {label}"
        );
    };

    let mut partial = original.clone();
    partial["files"][0]["expected_vl_single_frame"]
        .as_object_mut()
        .expect("VL expectation object")
        .remove("laterality");
    reject("partial expected_vl_single_frame", &partial);

    let mut wrong_case = original;
    wrong_case["files"][0]["case_id"] = Value::from("vl/photo/rgb_planar0_explicit_le");
    reject("expected_vl_single_frame on the wrong case", &wrong_case);

    fs::remove_dir_all(out_dir).expect("remove malformed VL report root");
}

#[test]
fn report_locks_rt_radiation_pair_contracts_and_markdown() {
    let out_dir = unique_temp_dir("report-rt-radiation-pair");
    generate_extended(&out_dir);
    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("RT Radiation pair coverage report should build");
    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("coverage schema compiles");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "RT Radiation coverage must match schema: {errors:?}"
    );

    let radiation = coverage_row(
        &report,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    );
    assert_eq!(
        radiation["rt_radiation_iod_kind"],
        "carm_photon_electron_radiation"
    );
    assert_eq!(radiation["rt_radiation_label"], "DTS_RADIATION");
    assert_eq!(radiation["rt_radiation_content_detail"], "IDENT_ONLY");
    assert_eq!(radiation["rt_radiation_record_flag"], "NO");
    assert_eq!(
        radiation["rt_radiation_treatment_technique"],
        "130102|DCM|Static Beam"
    );
    assert_eq!(radiation["rt_radiation_treatment_position_count"], 1);
    assert_eq!(radiation["rt_radiation_control_point_count"], 2);
    assert_eq!(radiation["rt_radiation_control_point_indices"], "1; 2");
    assert_eq!(radiation["rt_radiation_meterset_range"], "0..100");
    assert_eq!(radiation["rt_radiation_reference_closure"], true);
    assert_eq!(radiation["rt_radiation_pixel_data_absent"], true);

    let set = coverage_row(&report, "non-image/rt/radiation_set_minimal");
    assert_eq!(set["rt_radiation_set_iod_kind"], "rt_radiation_set");
    assert_eq!(set["rt_radiation_set_label"], "DTS_RADSET");
    assert_eq!(set["rt_radiation_set_intent"], "TREATMENT");
    assert_eq!(set["rt_radiation_set_intended_fraction_count"], 1);
    assert_eq!(set["rt_radiation_set_treatment_position_group_count"], 1);
    assert_eq!(
        set["rt_radiation_set_treatment_position_group_labels"],
        "DTS_TPG_1"
    );
    assert_eq!(set["rt_radiation_set_common_instance_reference_count"], 2);
    assert_eq!(set["rt_radiation_set_reference_closure"], true);
    assert_eq!(set["rt_radiation_set_dose_contribution_absent"], true);
    assert_eq!(set["rt_radiation_set_pixel_data_absent"], true);

    for (group, row, field) in [
        ("rt_radiation_iod_kinds", radiation, "rt_radiation_iod_kind"),
        ("rt_radiation_labels", radiation, "rt_radiation_label"),
        (
            "rt_radiation_content_details",
            radiation,
            "rt_radiation_content_detail",
        ),
        (
            "rt_radiation_record_flags",
            radiation,
            "rt_radiation_record_flag",
        ),
        (
            "rt_radiation_treatment_techniques",
            radiation,
            "rt_radiation_treatment_technique",
        ),
        (
            "rt_radiation_device_identities",
            radiation,
            "rt_radiation_device_identity",
        ),
        (
            "rt_radiation_dosimeter_units",
            radiation,
            "rt_radiation_dosimeter_unit",
        ),
        (
            "rt_radiation_treatment_position_index_orders",
            radiation,
            "rt_radiation_treatment_position_indices",
        ),
        (
            "rt_radiation_control_point_index_orders",
            radiation,
            "rt_radiation_control_point_indices",
        ),
        (
            "rt_radiation_meterset_ranges",
            radiation,
            "rt_radiation_meterset_range",
        ),
        (
            "rt_radiation_definition_source_identities",
            radiation,
            "rt_radiation_definition_source_identity",
        ),
        (
            "rt_radiation_external_validator_dispositions",
            radiation,
            "rt_radiation_external_validator_disposition",
        ),
        (
            "rt_radiation_set_iod_kinds",
            set,
            "rt_radiation_set_iod_kind",
        ),
        ("rt_radiation_set_labels", set, "rt_radiation_set_label"),
        ("rt_radiation_set_intents", set, "rt_radiation_set_intent"),
        (
            "rt_radiation_set_device_identities",
            set,
            "rt_radiation_set_device_identity",
        ),
        (
            "rt_radiation_set_definition_source_identities",
            set,
            "rt_radiation_set_definition_source_identity",
        ),
        (
            "rt_radiation_set_radiation_reference_identity_orders",
            set,
            "rt_radiation_set_radiation_reference_identities",
        ),
        (
            "rt_radiation_set_treatment_position_group_label_orders",
            set,
            "rt_radiation_set_treatment_position_group_labels",
        ),
        (
            "rt_radiation_set_external_validator_dispositions",
            set,
            "rt_radiation_set_external_validator_disposition",
        ),
    ] {
        let key = row[field].as_str().expect("grouped string field");
        assert_eq!(report["grouped_coverage"][group][key], 1, "{group}");
    }
    for (group, row, field) in [
        (
            "rt_radiation_treatment_position_counts",
            radiation,
            "rt_radiation_treatment_position_count",
        ),
        (
            "rt_radiation_control_point_counts",
            radiation,
            "rt_radiation_control_point_count",
        ),
        (
            "rt_radiation_reference_closure_states",
            radiation,
            "rt_radiation_reference_closure",
        ),
        (
            "rt_radiation_pixel_data_absent_states",
            radiation,
            "rt_radiation_pixel_data_absent",
        ),
        (
            "rt_radiation_set_intended_fraction_counts",
            set,
            "rt_radiation_set_intended_fraction_count",
        ),
        (
            "rt_radiation_set_treatment_position_group_counts",
            set,
            "rt_radiation_set_treatment_position_group_count",
        ),
        (
            "rt_radiation_set_common_instance_reference_counts",
            set,
            "rt_radiation_set_common_instance_reference_count",
        ),
        (
            "rt_radiation_set_reference_closure_states",
            set,
            "rt_radiation_set_reference_closure",
        ),
        (
            "rt_radiation_set_dose_contribution_absent_states",
            set,
            "rt_radiation_set_dose_contribution_absent",
        ),
        (
            "rt_radiation_set_pixel_data_absent_states",
            set,
            "rt_radiation_set_pixel_data_absent",
        ),
    ] {
        let key = if let Some(value) = row[field].as_u64() {
            value.to_string()
        } else {
            row[field]
                .as_bool()
                .expect("grouped bool field")
                .to_string()
        };
        assert_eq!(report["grouped_coverage"][group][&key], 1, "{group}");
    }

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## RT Radiation Expectations"));
    assert!(markdown.contains("## RT Radiation Set Expectations"));
    assert!(markdown.contains("DTS_RADIATION"));
    assert!(markdown.contains("DTS_RADSET"));
    for title in [
        "RT Radiation IOD Kinds",
        "RT Radiation Labels",
        "RT Radiation Content Details",
        "RT Radiation Record Flags",
        "RT Radiation Treatment Techniques",
        "RT Radiation Device Identities",
        "RT Radiation Dosimeter Units",
        "RT Radiation Treatment Position Counts",
        "RT Radiation Treatment Position Index Orders",
        "RT Radiation Control Point Counts",
        "RT Radiation Control Point Index Orders",
        "RT Radiation Meterset Ranges",
        "RT Radiation Definition Source Identities",
        "RT Radiation Reference Closure States",
        "RT Radiation Pixel Data Absent States",
        "RT Radiation External Validator Dispositions",
        "RT Radiation Set IOD Kinds",
        "RT Radiation Set Labels",
        "RT Radiation Set Intents",
        "RT Radiation Set Fraction Counts",
        "RT Radiation Set Device Identities",
        "RT Radiation Set Definition Source Identities",
        "RT Radiation Set Radiation Reference Identity Orders",
        "RT Radiation Set Position Group Counts",
        "RT Radiation Set Position Group Label Orders",
        "RT Radiation Set Common Instance Reference Counts",
        "RT Radiation Set Reference Closure States",
        "RT Radiation Set Dose Contribution Absent States",
        "RT Radiation Set Pixel Data Absent States",
        "RT Radiation Set External Validator Dispositions",
    ] {
        assert!(
            markdown.contains(&format!("### {title}")),
            "missing Markdown group {title}"
        );
    }

    let mut partial = report.clone();
    coverage_row_mut(
        &mut partial,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )["rt_radiation_control_point_count"] = Value::Null;
    assert!(
        !validator.is_valid(&partial),
        "schema must reject a partial Radiation contract"
    );
    let mut leaked = report.clone();
    coverage_row_mut(&mut leaked, "non-image/rt/plan_linked")["rt_radiation_set_label"] =
        Value::from("DTS_RADSET");
    assert!(
        !validator.is_valid(&leaked),
        "schema must reject leaked Radiation Set fields"
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_rejects_malformed_rt_radiation_pair_manifests() {
    let out_dir = unique_temp_dir("report-rt-radiation-malformed");
    generate_extended(&out_dir);
    let manifest_path = out_dir.join("manifest.json");
    let original: Value = serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
        .expect("manifest JSON");
    fn file_mut<'a>(manifest: &'a mut Value, case_id: &str) -> &'a mut Value {
        manifest["files"]
            .as_array_mut()
            .expect("files array")
            .iter_mut()
            .find(|file| file["case_id"] == case_id)
            .expect("case file")
    }
    let reject = |label: &str, manifest: &Value| {
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(manifest).expect("serialize mutation"),
        )
        .expect("write mutated manifest");
        assert!(
            dicom_test_suite::build_coverage_report(&out_dir).is_err(),
            "report must reject {label}"
        );
    };

    let mut missing = original.clone();
    file_mut(
        &mut missing,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )
    .as_object_mut()
    .expect("Radiation file object")
    .remove("expected_rt_radiation");
    reject("missing Radiation expectation", &missing);

    let mut missing_set = original.clone();
    file_mut(&mut missing_set, "non-image/rt/radiation_set_minimal")
        .as_object_mut()
        .expect("Radiation Set file object")
        .remove("expected_rt_radiation_set");
    reject("missing Radiation Set expectation", &missing_set);

    let mut wrong_case = original.clone();
    let contract = file_mut(
        &mut wrong_case,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )
    .as_object_mut()
    .expect("Radiation file object")
    .remove("expected_rt_radiation")
    .expect("Radiation contract");
    file_mut(&mut wrong_case, "non-image/rt/plan_linked")["expected_rt_radiation"] = contract;
    reject("Radiation expectation on wrong case", &wrong_case);

    let mut wrong_set_case = original.clone();
    let contract = file_mut(&mut wrong_set_case, "non-image/rt/radiation_set_minimal")
        .as_object_mut()
        .expect("Radiation Set file object")
        .remove("expected_rt_radiation_set")
        .expect("Radiation Set contract");
    file_mut(&mut wrong_set_case, "non-image/rt/plan_linked")["expected_rt_radiation_set"] =
        contract;
    reject("Radiation Set expectation on wrong case", &wrong_set_case);

    let mut radiation_device = original.clone();
    file_mut(
        &mut radiation_device,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )["expected_rt_radiation"]["device"]["model_name"] = Value::from("WRONG");
    reject("wrong Radiation device identity", &radiation_device);

    let mut set_device = original.clone();
    file_mut(&mut set_device, "non-image/rt/radiation_set_minimal")["expected_rt_radiation_set"]
        ["linked_radiation_device"]["serial_number"] = Value::from("WRONG");
    reject("wrong Radiation Set device identity", &set_device);

    let mut uppercase_hash = original.clone();
    let hash = file_mut(
        &mut uppercase_hash,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )["expected_rt_radiation"]["definition_source"]["source_sha256"]
        .as_str()
        .expect("source hash")
        .to_ascii_uppercase();
    file_mut(
        &mut uppercase_hash,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )["expected_rt_radiation"]["definition_source"]["source_sha256"] = Value::from(hash);
    reject("non-lowercase Radiation source hash", &uppercase_hash);

    let mut set_uppercase_hash = original.clone();
    let hash = file_mut(
        &mut set_uppercase_hash,
        "non-image/rt/radiation_set_minimal",
    )["expected_rt_radiation_set"]["radiation_references"][0]["source_sha256"]
        .as_str()
        .expect("Set Radiation source hash")
        .to_ascii_uppercase();
    let set = file_mut(
        &mut set_uppercase_hash,
        "non-image/rt/radiation_set_minimal",
    );
    set["expected_rt_radiation_set"]["radiation_references"][0]["source_sha256"] =
        Value::from(hash.clone());
    set["expected_rt_radiation_set"]["treatment_position_groups"][0]["radiation_references"][0]["source_sha256"] =
        Value::from(hash.clone());
    set["expected_rt_radiation_set"]["common_instance_references"][1]["source_sha256"] =
        Value::from(hash);
    reject(
        "consistently mirrored non-lowercase Set source hash",
        &set_uppercase_hash,
    );

    let mut radiation_uid_binding = original.clone();
    file_mut(
        &mut radiation_uid_binding,
        "non-image/rt/carm_photon_electron_radiation_minimal",
    )["expected_rt_radiation"]["definition_source"]["study_instance_uid"] = Value::from("2.25.999");
    reject(
        "Radiation source Study UID outside containing contract",
        &radiation_uid_binding,
    );

    let mut set_plan_uid_binding = original.clone();
    let set = file_mut(
        &mut set_plan_uid_binding,
        "non-image/rt/radiation_set_minimal",
    );
    set["expected_rt_radiation_set"]["definition_source"]["study_instance_uid"] =
        Value::from("2.25.997");
    set["expected_rt_radiation_set"]["common_instance_references"][0]["study_instance_uid"] =
        Value::from("2.25.997");
    reject(
        "consistently mirrored Set Plan source Study UID corruption",
        &set_plan_uid_binding,
    );

    let mut set_uid_binding = original.clone();
    let set = file_mut(&mut set_uid_binding, "non-image/rt/radiation_set_minimal");
    set["expected_rt_radiation_set"]["radiation_references"][0]["frame_of_reference_uid"] =
        Value::from("2.25.999");
    set["expected_rt_radiation_set"]["treatment_position_groups"][0]["radiation_references"][0]["frame_of_reference_uid"] =
        Value::from("2.25.999");
    set["expected_rt_radiation_set"]["common_instance_references"][1]["frame_of_reference_uid"] =
        Value::from("2.25.999");
    reject(
        "consistently mirrored Set source Frame UID corruption",
        &set_uid_binding,
    );

    let mut mirrored = original.clone();
    let set = file_mut(&mut mirrored, "non-image/rt/radiation_set_minimal");
    set["expected_rt_radiation_set"]["treatment_position_groups"][0]["radiation_references"][0]["sop_instance_uid"] =
        Value::from("2.25.998");
    set["expected_rt_radiation_set"]["common_instance_references"][1]["sop_instance_uid"] =
        Value::from("2.25.998");
    reject("consistently corrupted Set reference mirrors", &mirrored);

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

const RT_IMAGE_REPORT_FIELDS: &[&str] = &[
    "rt_image_type",
    "rt_image_label",
    "rt_image_plane",
    "rt_image_pixel_spacing_mm",
    "rt_image_position_mm",
    "rt_image_dimensions",
    "rt_image_bit_contract",
    "rt_image_payload_sha256",
    "rt_image_plan_reference_identity",
    "rt_image_referenced_beam_number",
    "rt_image_referenced_fraction_group_number",
    "rt_image_radiation_machine_sad_mm",
    "rt_image_sid_mm",
    "rt_image_reference_closure",
    "rt_image_pixel_disposition",
    "rt_image_external_validator_disposition",
];

fn linked_rt_image_report_manifest() -> Value {
    let study_uid = "2.25.420000000000000000000000000000000000001";
    let frame_uid = "2.25.420000000000000000000000000000000000002";
    let image_series_uid = "2.25.420000000000000000000000000000000000003";
    let image_sop_uid = "2.25.420000000000000000000000000000000000004";
    let plan_series_uid = "2.25.420000000000000000000000000000000000005";
    let plan_sop_uid = "2.25.420000000000000000000000000000000000006";
    let plan_reference = json!({
        "relationship": "referenced_rt_plan",
        "source_case_id": "non-image/rt/plan_linked",
        "source_path": "non-image/rt/plan_linked/instance.dcm",
        "source_sha256": "c".repeat(64),
        "study_instance_uid": study_uid,
        "series_instance_uid": plan_series_uid,
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
        "sop_instance_uid": plan_sop_uid,
        "frame_of_reference_uid": frame_uid
    });
    json!({
        "generated_at": "20260101000000.000000+0000",
        "standards": { "standards_lock_sha256": "0".repeat(64) },
        "run": { "profile": "extended" },
        "files": [{
            "case_id": "non-image/rt/image_linked",
            "profile_membership": ["extended"],
            "determinism": "byte_stable",
            "dicom": {
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.1",
                "sop_class_name": "RT Image Storage",
                "iod_name": "RT Image",
                "modality": "RTIMAGE",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "image": {
                "rows": 4, "columns": 4, "frames": 1, "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8,
                "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
                "planar_configuration": null
            },
            "pixel_data": {
                "vr": "OB", "native_or_encapsulated": "native", "value_length": 16,
                "frame_count": 1,
                "frame_hashes": ["a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811"]
            },
            "references": [{
                "relationship": plan_reference["relationship"],
                "source_case_id": plan_reference["source_case_id"],
                "source_path": plan_reference["source_path"],
                "series_instance_uid": plan_reference["series_instance_uid"],
                "sop_class_uid": plan_reference["sop_class_uid"],
                "sop_instance_uid": plan_reference["sop_instance_uid"]
            }],
            "expected_semantics": { "synthetic_data": "YES" },
            "expected_rt_image": {
                "iod_kind": "rt_image",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.1",
                "iod_name": "RT Image",
                "modality": "RTIMAGE",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "sop_instance_uid": image_sop_uid,
                "study_instance_uid": study_uid,
                "series_instance_uid": image_series_uid,
                "frame_of_reference_uid": frame_uid,
                "plan_reference": plan_reference,
                "linkage": {
                    "referenced_fraction_group_number": 1,
                    "referenced_beam_number": 1
                },
                "image": {
                    "image_type": ["DERIVED", "SECONDARY", "DRR"],
                    "conversion_type": "WSD", "label": "DTS_DRR", "plane": "NORMAL",
                    "xray_image_receptor_angle_degrees": 0,
                    "image_plane_pixel_spacing_mm": [1, 1], "position_mm": [-1.5, 1.5],
                    "radiation_machine_name": "DTS_LINAC", "radiation_machine_sad_mm": 1000,
                    "rt_image_sid_mm": 1500, "primary_dosimeter_unit": "MU"
                },
                "storage": {
                    "rows": 4, "columns": 4, "frames": 1, "samples_per_pixel": 1,
                    "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8,
                    "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
                    "data_vr": "OB", "encoding": "native", "payload_length_bytes": 16,
                    "value_field_padding_bytes": 0, "pixel_value_formula": "17 * (4 * r + c)",
                    "pixel_values": [0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255],
                    "pixel_min": 0, "pixel_max": 255,
                    "payload_sha256": "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811",
                    "decoded_pixels_sha256": "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811"
                },
                "absent_content": {
                    "patient_study_module": true, "contrast_bolus_module": true,
                    "cine_module": true, "multi_frame_module": true,
                    "modality_lut_module": true, "voi_lut_module": true,
                    "approval_module": true, "clinical_trial_module": true,
                    "frame_extraction_module": true, "common_instance_reference_module": true,
                    "reported_values_origin": true, "rt_image_orientation": true,
                    "isocenter_position": true, "patient_position": true,
                    "fluence_map_sequence": true, "exposure_sequence": true,
                    "overlays": true, "encapsulated_pixel_data": true,
                    "lossy_pixel_attributes": true
                }
            },
            "validation": { "status": "passed" },
            "known_stressors": []
        }],
        "skipped_cases": []
    })
}

fn vl_single_frame_report_manifest() -> Value {
    let file = |case_id: &str,
                iod_kind: &str,
                sop_class_uid: &str,
                sop_class_name: &str,
                iod_name: &str,
                modality: &str,
                body_part_examined: &str,
                storage_stressor: &str| {
        json!({
            "case_id": case_id,
            "profile_membership": ["extended"],
            "determinism": "byte_stable",
            "dicom": {
                "sop_class_uid": sop_class_uid,
                "sop_class_name": sop_class_name,
                "iod_name": iod_name,
                "modality": modality,
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "image": {
                "rows": 2,
                "columns": 2,
                "frames": 1,
                "samples_per_pixel": 3,
                "photometric_interpretation": "RGB",
                "bits_allocated": 8,
                "bits_stored": 8,
                "high_bit": 7,
                "pixel_representation": 0,
                "planar_configuration": 0
            },
            "pixel_data": {
                "vr": "OB",
                "native_or_encapsulated": "native",
                "value_length": 12,
                "frame_count": 1,
                "frame_hashes": ["0".repeat(64)]
            },
            "expected_semantics": {
                "synthetic_data": "YES",
                "body_part_examined": body_part_examined,
                "laterality": "R",
                "image_type": "ORIGINAL\\PRIMARY"
            },
            "expected_vl_single_frame": {
                "iod_kind": iod_kind,
                "sop_class_uid": sop_class_uid,
                "sop_class_name": sop_class_name,
                "iod_name": iod_name,
                "modality": modality,
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "body_part_examined": body_part_examined,
                "laterality": "R",
                "image_type": ["ORIGINAL", "PRIMARY"],
                "acquisition_context_items": 0,
                "image": {
                    "rows": 2,
                    "columns": 2,
                    "samples_per_pixel": 3,
                    "photometric_interpretation": "RGB",
                    "planar_configuration": 0,
                    "bits_allocated": 8,
                    "bits_stored": 8,
                    "high_bit": 7,
                    "pixel_representation": 0
                },
                "absent_content": [
                    "number_of_frames",
                    "frame_of_reference_uid",
                    "specimen_module",
                    "optical_path_module",
                    "icc_profile_module"
                ]
            },
            "validation": { "status": "passed" },
            "known_stressors": [storage_stressor, "vl_rgb_pixels", "native_ob_pixel_data"]
        })
    };
    json!({
        "generated_at": "20260101000000.000000+0000",
        "standards": { "standards_lock_sha256": "0".repeat(64) },
        "run": { "profile": "extended" },
        "files": [
            file(
                "vl/endoscopic/rgb_explicit_le",
                "vl_endoscopic_single_frame",
                "1.2.840.10008.5.1.4.1.1.77.1.1",
                "VL Endoscopic Image Storage",
                "VL Endoscopic Image",
                "ES",
                "LUNG",
                "vl_endoscopic_image_storage",
            ),
            file(
                "vl/microscopic/rgb_explicit_le",
                "vl_microscopic_single_frame",
                "1.2.840.10008.5.1.4.1.1.77.1.2",
                "VL Microscopic Image Storage",
                "VL Microscopic Image",
                "GM",
                "EYE",
                "vl_microscopic_image_storage",
            )
        ],
        "skipped_cases": []
    })
}

const RT_PLAN_REPORT_FIELDS: &[&str] = &[
    "rt_plan_label",
    "rt_plan_geometry",
    "rt_plan_fraction_group_count",
    "rt_plan_fraction_group_numbers",
    "rt_plan_beam_count",
    "rt_plan_beam_numbers",
    "rt_plan_beam_names",
    "rt_plan_beam_types",
    "rt_plan_radiation_types",
    "rt_plan_control_point_count",
    "rt_plan_control_point_indices",
    "rt_plan_meterset_range",
    "rt_plan_structure_set_reference_identity",
    "rt_plan_dose_reference_identity",
    "rt_plan_reference_closure",
    "rt_plan_pixel_data_absent",
    "rt_plan_external_validator_disposition",
];

fn linked_rt_plan_report_manifest() -> Value {
    let study_uid = "2.25.410000000000000000000000000000000000001";
    let frame_uid = "2.25.410000000000000000000000000000000000002";
    let plan_series_uid = "2.25.410000000000000000000000000000000000003";
    let plan_sop_uid = "2.25.410000000000000000000000000000000000004";
    let structure_series_uid = "2.25.410000000000000000000000000000000000005";
    let structure_sop_uid = "2.25.410000000000000000000000000000000000006";
    let dose_series_uid = "2.25.410000000000000000000000000000000000007";
    let dose_sop_uid = "2.25.410000000000000000000000000000000000008";
    let structure_reference = json!({
        "ordinal": 1,
        "relationship": "referenced_structure_set",
        "source_case_id": "non-image/rt/structure_set_single_roi_explicit_le",
        "source_path": "non-image/rt/structure_set_single_roi_explicit_le/instance.dcm",
        "source_sha256": "a".repeat(64),
        "study_instance_uid": study_uid,
        "series_instance_uid": structure_series_uid,
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.3",
        "sop_instance_uid": structure_sop_uid,
        "frame_of_reference_uid": frame_uid
    });
    let dose_reference = json!({
        "ordinal": 2,
        "relationship": "referenced_dose",
        "source_case_id": "non-image/rt/dose_grid_u16_explicit_le",
        "source_path": "non-image/rt/dose_grid_u16_explicit_le/instance.dcm",
        "source_sha256": "b".repeat(64),
        "study_instance_uid": study_uid,
        "series_instance_uid": dose_series_uid,
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.2",
        "sop_instance_uid": dose_sop_uid,
        "frame_of_reference_uid": frame_uid
    });
    let generic_reference = |reference: &Value| {
        json!({
            "relationship": reference["relationship"],
            "source_case_id": reference["source_case_id"],
            "source_path": reference["source_path"],
            "series_instance_uid": reference["series_instance_uid"],
            "sop_class_uid": reference["sop_class_uid"],
            "sop_instance_uid": reference["sop_instance_uid"]
        })
    };
    json!({
        "generated_at": "20260101000000.000000+0000",
        "standards": { "standards_lock_sha256": "0".repeat(64) },
        "run": { "profile": "extended" },
        "files": [{
            "case_id": "non-image/rt/plan_linked",
            "profile_membership": ["extended"],
            "determinism": "byte_stable",
            "dicom": {
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
                "sop_class_name": "RT Plan Storage",
                "iod_name": "RT Plan",
                "modality": "RTPLAN",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "image": null,
            "pixel_data": null,
            "references": [
                generic_reference(&structure_reference),
                generic_reference(&dose_reference)
            ],
            "expected_semantics": { "synthetic_data": "YES" },
            "expected_rt_plan": {
                "iod_kind": "rt_plan",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.5",
                "iod_name": "RT Plan",
                "modality": "RTPLAN",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "sop_instance_uid": plan_sop_uid,
                "study_instance_uid": study_uid,
                "series_instance_uid": plan_series_uid,
                "frame_of_reference_uid": frame_uid,
                "references": [structure_reference, dose_reference],
                "plan": {
                    "label": "DTS_PLAN", "date": "20260101", "time": "000000",
                    "geometry": "PATIENT"
                },
                "fraction_groups": [{
                    "ordinal": 1, "fraction_group_number": 1,
                    "number_of_fractions_planned": 1, "number_of_beams": 1,
                    "number_of_brachy_application_setups": 0,
                    "referenced_beams": [{ "ordinal": 1, "referenced_beam_number": 1 }]
                }],
                "beams": [{
                    "ordinal": 1, "treatment_machine_name": "DTS_LINAC",
                    "primary_dosimeter_unit": "MU", "source_axis_distance_mm": 1000,
                    "beam_number": 1, "beam_name": "DTS_STATIC_AP", "beam_type": "STATIC",
                    "radiation_type": "PHOTON", "treatment_delivery_type": "TREATMENT",
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
                    "control_points": [{
                        "ordinal": 1, "control_point_index": 0, "cumulative_meterset_weight": 0,
                        "geometry": {
                            "nominal_beam_energy_mev": 6,
                            "jaw_positions_mm": [[-50, 50], [-50, 50]],
                            "gantry_angle_degrees": 0, "gantry_rotation_direction": "NONE",
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
                    }, {
                        "ordinal": 2, "control_point_index": 1, "cumulative_meterset_weight": 1,
                        "geometry": null, "inherits_geometry_from_control_point": 0
                    }]
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
            },
            "validation": { "status": "passed" },
            "known_stressors": []
        }],
        "skipped_cases": []
    })
}

fn general_ecg_report_manifest() -> Value {
    let standard_channels = [
        ("I", "2:1", "Lead I"),
        ("II", "2:2", "Lead II"),
        ("III", "2:61", "Lead III"),
        ("aVR", "2:62", "aVR, augmented voltage, right"),
        ("aVL", "2:63", "aVL, augmented voltage, left"),
        ("aVF", "2:64", "aVF, augmented voltage, foot"),
        ("V1", "2:3", "Lead V1"),
        ("V2", "2:4", "Lead V2"),
        ("V3", "2:5", "Lead V3"),
        ("V4", "2:6", "Lead V4"),
        ("V5", "2:7", "Lead V5"),
        ("V6", "2:8", "Lead V6"),
    ];
    let auxiliary_channels = [
        ("A1", "2:75", "Auxiliary unipolar lead 1"),
        ("A2", "2:76", "Auxiliary unipolar lead 2"),
        ("A3", "2:77", "Auxiliary unipolar lead 3"),
        ("A4", "2:78", "Auxiliary unipolar lead 4"),
    ];
    let standard_hashes = [
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
        "9280ad35672b82a7847d3ccabadd4d85a94be3d39d0a836191384571f0a23ab6",
    ];
    let auxiliary_hashes = [
        "5da46776ad84a78eb0c16066cb8ac7d5e05ca6ad87170264b227c71261def284",
        "7bd73425422f4e79504b55932040e481ccdfafecabe1dba613ee36074a51b9e3",
        "e56dad9647dfa50a10b40d244e29eaedbf23d97a558901f46fbccc07ad1a1766",
        "e1b68207c92fe2cc4c6765fc097668f2600eeda152eb5a1d6f0444f4c9e36fbc",
    ];
    let standard = standard_channels
        .into_iter()
        .enumerate()
        .map(|(index, (label, code, meaning))| {
            report_waveform_channel(index + 1, label, code, meaning)
        })
        .collect::<Vec<_>>();
    let auxiliary = auxiliary_channels
        .into_iter()
        .enumerate()
        .map(|(index, (label, code, meaning))| {
            report_waveform_channel(index + 1, label, code, meaning)
        })
        .collect::<Vec<_>>();

    json!({
        "generated_at": "20260101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": { "profile": "extended" },
        "files": [{
            "case_id": "non-image/waveform/general_ecg",
            "profile_membership": ["extended"],
            "determinism": "byte_stable",
            "dicom": {
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.2",
                "sop_class_name": "General ECG Waveform Storage",
                "iod_name": "General ECG Waveform",
                "modality": "ECG",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "pixel_data": null,
            "references": [],
            "expected_semantics": { "synthetic_data": "YES" },
            "expected_waveform": {
                "iod_kind": "general_ecg",
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.2",
                "iod_name": "General ECG Waveform",
                "modality": "ECG",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "acquisition_context_items": 0,
                "multiplex_groups": [{
                    "ordinal": 1,
                    "originality": "ORIGINAL",
                    "label": "STD12_250HZ",
                    "channel_count": 12,
                    "samples_per_channel": 1000,
                    "sampling_frequency_hz": 250,
                    "duration_seconds": 4,
                    "simultaneous_sampling": true,
                    "channels": standard,
                    "storage": {
                        "bits_allocated": 16,
                        "sample_interpretation": "SS",
                        "data_vr": "OW",
                        "byte_order": "little_endian",
                        "interleave_order": "channel_then_sample",
                        "payload_length_bytes": 24000,
                        "payload_sha256": "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
                        "channel_sha256": standard_hashes,
                        "sample_value_formula": "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000",
                        "sample_min": -1000,
                        "sample_max": 1000,
                        "waveform_padding_value_absent": true,
                        "value_field_padding_bytes": 0
                    }
                }, {
                    "ordinal": 2,
                    "originality": "ORIGINAL",
                    "label": "AUX4_1000HZ",
                    "channel_count": 4,
                    "samples_per_channel": 4000,
                    "sampling_frequency_hz": 1000,
                    "duration_seconds": 4,
                    "simultaneous_sampling": true,
                    "channels": auxiliary,
                    "storage": {
                        "bits_allocated": 16,
                        "sample_interpretation": "SS",
                        "data_vr": "OW",
                        "byte_order": "little_endian",
                        "interleave_order": "channel_then_sample",
                        "payload_length_bytes": 32000,
                        "payload_sha256": "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
                        "channel_sha256": auxiliary_hashes,
                        "sample_value_formula": "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000",
                        "sample_min": -1000,
                        "sample_max": 1000,
                        "waveform_padding_value_absent": true,
                        "value_field_padding_bytes": 0
                    }
                }],
                "aggregate": {
                    "group_count": 2,
                    "total_channel_count": 16,
                    "common_duration_seconds": 4,
                    "total_payload_length_bytes": 56000,
                    "group_payload_sha256": [
                        "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
                        "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe"
                    ],
                    "aggregate_payload_sha256": "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea"
                },
                "absent_content": {
                    "annotation_module": true,
                    "synchronization_module": true,
                    "references": true,
                    "image": true,
                    "pixel_data": true
                }
            },
            "validation": { "status": "passed" },
            "known_stressors": []
        }],
        "skipped_cases": []
    })
}

fn report_waveform_channel(ordinal: usize, label: &str, code: &str, meaning: &str) -> Value {
    json!({
        "ordinal": ordinal,
        "label": label,
        "source": {
            "code_value": code,
            "coding_scheme_designator": "MDC",
            "code_meaning": meaning
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
}

fn waveform_report_manifest() -> Value {
    let labels = [
        "I", "II", "III", "aVR", "aVL", "aVF", "V1", "V2", "V3", "V4", "V5", "V6",
    ];
    let codes = [
        "2:1", "2:2", "2:61", "2:62", "2:63", "2:64", "2:3", "2:4", "2:5", "2:6", "2:7", "2:8",
    ];
    let meanings = [
        "Lead I",
        "Lead II",
        "Lead III",
        "aVR, augmented voltage, right",
        "aVL, augmented voltage, left",
        "aVF, augmented voltage, foot",
        "Lead V1",
        "Lead V2",
        "Lead V3",
        "Lead V4",
        "Lead V5",
        "Lead V6",
    ];
    let channels = labels
        .iter()
        .zip(codes)
        .zip(meanings)
        .enumerate()
        .map(|(index, ((label, code), meaning))| {
            json!({
                "ordinal": index + 1,
                "label": label,
                "source": {
                    "code_value": code,
                    "coding_scheme_designator": "MDC",
                    "code_meaning": meaning
                },
                "bits_stored": 16
            })
        })
        .collect::<Vec<_>>();
    let mut twelve_manifest = json!({
        "generated_at": "20260101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": { "profile": "extended" },
        "files": [{
            "case_id": "non-image/waveform/twelve_lead_ecg",
            "profile_membership": ["extended"],
            "determinism": "byte_stable",
            "dicom": {
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.1",
                "sop_class_name": "12-lead ECG Waveform Storage",
                "iod_name": "12-lead ECG Waveform",
                "modality": "ECG",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1",
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "pixel_data": null,
            "references": [],
            "expected_semantics": { "synthetic_data": "YES" },
            "expected_waveform": {
                "iod_kind": "twelve_lead_ecg",
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
                        "interleave_order": "channel_then_sample",
                        "payload_length_bytes": 12000,
                        "payload_sha256": "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
                        "channel_sha256": vec!["unused"; 12]
                    }
                }],
                "aggregate": {
                    "group_count": 1,
                    "total_channel_count": 12,
                    "common_duration_seconds": 1,
                    "total_payload_length_bytes": 12000,
                    "group_payload_sha256": ["98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"],
                    "aggregate_payload_sha256": "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
                },
                "absent_content": { "pixel_data": true }
            },
            "validation": { "status": "passed" },
            "known_stressors": []
        }],
        "skipped_cases": [{
            "case_id": "non-image/rt/dose_grid_u16_explicit_le",
            "status": "unavailable",
            "reason_code": "case_planned",
            "message": "recipe_unimplemented"
        }]
    });
    let twelve = twelve_manifest["files"]
        .as_array_mut()
        .expect("Twelve-lead fixture files")
        .pop()
        .expect("Twelve-lead fixture file");
    let skipped = twelve_manifest["skipped_cases"].clone();
    let mut manifest = general_ecg_report_manifest();
    manifest["files"]
        .as_array_mut()
        .expect("General ECG fixture files")
        .insert(0, twelve);
    manifest["skipped_cases"] = skipped;
    manifest
}

fn generate_core(out_dir: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "core",
            "--out",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("generate command must run");

    assert!(
        output.status.success(),
        "generate should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn generate_extended(out_dir: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("generate command must run");

    assert!(
        output.status.success(),
        "generate should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn coverage_row<'a>(report: &'a Value, case_id: &str) -> &'a Value {
    report
        .pointer("/coverage_matrix")
        .and_then(Value::as_array)
        .expect("coverage matrix should be an array")
        .iter()
        .find(|row| row.get("case_id").and_then(Value::as_str) == Some(case_id))
        .unwrap_or_else(|| panic!("coverage matrix should contain {case_id}"))
}

fn coverage_rows<'a>(report: &'a Value, case_id: &str) -> Vec<&'a Value> {
    report
        .pointer("/coverage_matrix")
        .and_then(Value::as_array)
        .expect("coverage matrix should be an array")
        .iter()
        .filter(|row| row.get("case_id").and_then(Value::as_str) == Some(case_id))
        .collect()
}

fn coverage_row_mut<'a>(report: &'a mut Value, case_id: &str) -> &'a mut Value {
    report
        .get_mut("coverage_matrix")
        .and_then(Value::as_array_mut)
        .expect("coverage matrix should be an array")
        .iter_mut()
        .find(|row| row.get("case_id").and_then(Value::as_str) == Some(case_id))
        .unwrap_or_else(|| panic!("coverage matrix should contain {case_id}"))
}

fn coverage_row_with_u64_field<'a>(
    report: &'a Value,
    case_id: &str,
    field: &str,
    expected: u64,
) -> &'a Value {
    report
        .pointer("/coverage_matrix")
        .and_then(Value::as_array)
        .expect("coverage matrix should be an array")
        .iter()
        .find(|row| {
            row.get("case_id").and_then(Value::as_str) == Some(case_id)
                && row.get(field).and_then(Value::as_u64) == Some(expected)
        })
        .unwrap_or_else(|| {
            panic!("coverage matrix should contain {case_id} with {field}={expected}")
        })
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{name}-{}-{nonce}",
        std::process::id()
    ))
}
