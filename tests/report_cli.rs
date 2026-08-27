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
    assert_eq!(
        report
            .get("coverage_report_schema_version")
            .and_then(Value::as_str),
        Some("0.1.0")
    );
    assert_eq!(
        report.pointer("/counts/generated").and_then(Value::as_u64),
        Some(30)
    );
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(11)
    );
    assert_eq!(
        report
            .pointer("/coverage_matrix")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(41)
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    let native_row = coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le");
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
        Some(41)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/profile_memberships/core")
            .and_then(Value::as_u64),
        Some(41)
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
        Some(40)
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
        Some(7)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_types/ORIGINAL\\PRIMARY\\AXIAL")
            .and_then(Value::as_u64),
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/conversion_types/SYN")
            .and_then(Value::as_u64),
        Some(10)
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
        Some(10)
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
        Some(10)
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
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_acquisition_numbers/1")
            .and_then(Value::as_u64),
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_rescale_intercepts/-1024")
            .and_then(Value::as_u64),
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_rescale_slopes/1")
            .and_then(Value::as_u64),
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/ct_rescale_types/HU")
            .and_then(Value::as_u64),
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/pixel_spacings/0.625\\0.625")
            .and_then(Value::as_u64),
        Some(10)
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
        Some(10)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/image_positions_patient/0\\0\\0")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/slice_thicknesses/5")
            .and_then(Value::as_u64),
        Some(9)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/spacing_between_slices/5")
            .and_then(Value::as_u64),
        Some(9)
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
        Some(30)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/series_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(30)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_instance_uid_roots/2.25")
            .and_then(Value::as_u64),
        Some(30)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_class_names/CT Image Storage")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sop_class_names/VL Photographic Image Storage")
            .and_then(Value::as_u64),
        Some(2)
    );

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
    assert_eq!(
        coverage_row(
            &report,
            "enhanced/mr/multiframe_temporal_position_explicit_le"
        )
        .get("enhanced_mr_temporal_position_time_offsets")
        .and_then(Value::as_str),
        Some("0.0; 1.5")
    );
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
    assert!(markdown.contains("### Enhanced MR Temporal Position Time Offsets"));
    assert!(markdown.contains("| 0.0; 1.5 | 1 |"));
    assert!(markdown.contains("### Enhanced MR Velocity Encoding Minimum Values"));
    assert!(markdown.contains("| -150.0 | 1 |"));
    assert!(markdown.contains("### Enhanced MR Velocity Encoding Maximum Values"));
    assert!(markdown.contains("| 150.0 | 1 |"));

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
        Some(1)
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
    assert!(markdown.contains("### Segmentation Types"));
    assert!(markdown.contains(if cfg!(feature = "deflate") {
        "| BINARY | 2 |"
    } else {
        "| BINARY | 1 |"
    }));
    assert!(markdown.contains("| FRACTIONAL | 1 |"));
    assert!(markdown.contains("| LABELMAP | 1 |"));
    assert!(markdown.contains("### Segmentation Fractional Types"));
    assert!(markdown.contains("| PROBABILITY | 1 |"));
    assert!(markdown.contains("### Segmentation Maximum Fractional Values"));
    assert!(markdown.contains("| 255 | 1 |"));

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
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/encapsulated_document_recognizable_visual_features/NO")
            .and_then(Value::as_u64),
        Some(1)
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
    let parametric_map_generated = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix")
        .iter()
        .any(|row| {
            row["case_id"].as_str() == Some("derived/parametric-map/float32_ct_derived_explicit_le")
        });
    assert_eq!(
        report
            .pointer("/grouped_coverage/rwvm_slopes/1.0")
            .and_then(Value::as_u64),
        Some(1 + u64::from(parametric_map_generated))
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
        Some(1 + u64::from(parametric_map_generated))
    );
    if parametric_map_generated {
        assert_eq!(
            report
                .pointer("/grouped_coverage/rwvm_units_code_values/1")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/rwvm_units_code_meanings/no units")
                .and_then(Value::as_u64),
            Some(1)
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
    assert!(markdown.contains(&format!(
        "| 1.0 | {} |",
        1 + usize::from(parametric_map_generated)
    )));
    assert!(markdown.contains("### RWVM Units Code Values"));
    assert!(markdown.contains("| HU | 1 |"));
    assert!(markdown.contains("### RWVM Units Coding Scheme Designators"));
    assert!(markdown.contains(&format!(
        "| UCUM | {} |",
        1 + usize::from(parametric_map_generated)
    )));
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
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_completion_flags/COMPLETE")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_verification_flags/UNVERIFIED")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_root_value_types/CONTAINER")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/sr_root_continuity_of_content/SEPARATE")
            .and_then(Value::as_u64),
        Some(3)
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
    assert!(markdown.contains("| COMPLETE | 3 |"));
    assert!(markdown.contains("### SR Verification Flags"));
    assert!(markdown.contains("| UNVERIFIED | 3 |"));
    assert!(markdown.contains("### SR Root Value Types"));
    assert!(markdown.contains("| CONTAINER | 3 |"));
    assert!(markdown.contains("### SR Root Continuity Of Content"));
    assert!(markdown.contains("| SEPARATE | 3 |"));
    assert!(markdown.contains("### SR Content Sequence Item Counts"));
    assert!(markdown.contains("| 2 | 2 |"));
    assert!(markdown.contains("### SR Observation Texts"));
    assert!(
        markdown
            .contains("| Synthetic Basic Text SR observation for Enhanced CT source images. | 1 |")
    );
    assert!(markdown.contains("### SR Measurement Numeric Values"));
    assert!(markdown.contains("| 12.5 | 1 |"));

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
    assert!(stdout.contains("| generated | 24 |"));
    assert!(stdout.contains("| planned | 13 |"));
    assert!(stdout.contains("### Profile Memberships"));
    assert!(stdout.contains("| core | 37 |"));
    assert!(stdout.contains("### Transfer Syntax Names"));
    assert!(stdout.contains("| Explicit VR Little Endian | 36 |"));
    assert!(stdout.contains("| Implicit VR Little Endian | 1 |"));
    assert!(stdout.contains("### SOP Class Names"));
    assert!(stdout.contains("| CT Image Storage | 8 |"));
    assert!(stdout.contains("| VL Photographic Image Storage | 2 |"));
    assert!(stdout.contains("### Image Types"));
    assert!(stdout.contains("### Conversion Types"));
    assert!(stdout.contains("### Presentation LUT Shapes"));
    assert!(stdout.contains("| IDENTITY | 2 |"));
    assert!(stdout.contains("| INVERSE | 1 |"));
    assert!(stdout.contains("### Window Centers"));
    assert!(stdout.contains("| 40 | 4 |"));
    assert!(stdout.contains("| 2048 | 2 |"));
    assert!(stdout.contains("### Window Widths"));
    assert!(stdout.contains("| 400 | 4 |"));
    assert!(stdout.contains("| 4096 | 2 |"));
    assert!(stdout.contains("### KVPs"));
    assert!(stdout.contains("| 120 | 4 |"));
    assert!(stdout.contains("### CT Acquisition Numbers"));
    assert!(stdout.contains("| 1 | 4 |"));
    assert!(stdout.contains("### CT Rescale Intercepts"));
    assert!(stdout.contains("| -1024 | 4 |"));
    assert!(stdout.contains("### CT Rescale Slopes"));
    assert!(stdout.contains("### CT Rescale Types"));
    assert!(stdout.contains("| HU | 4 |"));
    assert!(stdout.contains("### Pixel Spacings"));
    assert!(stdout.contains("| 0.625\\0.625 | 4 |"));
    assert!(stdout.contains("| 1.000\\1.000 | 3 |"));
    assert!(stdout.contains("### Imager Pixel Spacings"));
    assert!(stdout.contains("| 0.070\\0.070 | 2 |"));
    assert!(stdout.contains("| 0.150\\0.150 | 1 |"));
    assert!(stdout.contains("### Image Orientations Patient"));
    assert!(stdout.contains("| 1\\0\\0\\0\\1\\0 | 4 |"));
    assert!(stdout.contains("### Image Positions Patient"));
    assert!(stdout.contains("| 0\\0\\0 | 2 |"));
    assert!(stdout.contains("### Slice Thicknesses"));
    assert!(stdout.contains("| 5 | 6 |"));
    assert!(stdout.contains("### Spacing Between Slices"));
    assert!(stdout.contains("| 5 | 6 |"));
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
    assert!(stdout.contains("| 2.25 | 24 |"));
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
