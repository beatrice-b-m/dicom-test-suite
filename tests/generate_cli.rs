use std::fs;
use std::path::PathBuf;
use std::process::Command;

use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::Value;

#[test]
fn generate_command_writes_smoke_part10_files_and_manifest() {
    let out_dir = unique_temp_dir("generate-command");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "smoke",
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

    assert!(out_dir.is_dir(), "generate must create the output root");
    let manifest_path = out_dir.join("manifest.json");
    assert!(manifest_path.is_file(), "generate must write manifest.json");

    let stdout = String::from_utf8(output.stdout).expect("generate stdout must be utf-8");
    assert!(stdout.contains("profile\tsmoke"));
    assert!(stdout.contains("seed\t7"));
    assert!(stdout.contains("include_stress\tfalse"));
    assert!(stdout.contains(&format!("out\t{}", out_dir.display())));
    assert!(stdout.contains(&format!("manifest\t{}", manifest_path.display())));
    assert!(stdout.contains("files_written\t3"));
    assert!(stdout.contains("manifest_written\ttrue"));

    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    assert_eq!(
        manifest.pointer("/run/profile").and_then(Value::as_str),
        Some("smoke")
    );
    assert_eq!(
        manifest.pointer("/run/seed").and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        manifest
            .pointer("/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    let files = manifest
        .pointer("/files")
        .and_then(Value::as_array)
        .expect("manifest should describe generated files");
    let file_entry = files[0]
        .as_object()
        .expect("manifest should describe the first generated file");
    assert_eq!(
        file_entry.get("case_id").and_then(Value::as_str),
        Some("classic/sc/mono2_u8_explicit_le")
    );
    assert_eq!(
        file_entry.get("path").and_then(Value::as_str),
        Some("classic/sc/mono2_u8_explicit_le/instance.dcm")
    );
    assert_eq!(
        manifest
            .pointer("/files/0/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::SECONDARY_CAPTURE_IMAGE_STORAGE)
    );
    assert_eq!(
        manifest
            .pointer("/files/0/dicom/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some(uids::EXPLICIT_VR_LITTLE_ENDIAN)
    );
    assert_eq!(
        manifest
            .pointer("/files/0/expected_semantics/synthetic_data")
            .and_then(Value::as_str),
        Some("YES")
    );
    assert_eq!(
        manifest
            .pointer("/files/0/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        files[1].get("case_id").and_then(Value::as_str),
        Some("classic/sc/mono1_u8_explicit_le")
    );
    assert_eq!(
        files[1]
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        files[2].get("case_id").and_then(Value::as_str),
        Some("classic/sc/rgb_planar0_explicit_le")
    );
    assert_eq!(
        files[2]
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        files[2]
            .pointer("/image/samples_per_pixel")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        files[2]
            .pointer("/image/planar_configuration")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        manifest
            .pointer("/files/0/validation/status")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"part10_preamble"),
        "manifest should record Part 10 preamble validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"sop_instance_uid_consistency"),
        "manifest should record SOP Instance UID consistency validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"native_pixel_data_length"),
        "manifest should record native Pixel Data length validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"photometric_samples_per_pixel"),
        "manifest should record photometric sample-shape validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/standards")
            .contains(&"synthetic_data_attribute"),
        "manifest should record standards validation for Synthetic Data"
    );

    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    let dcm_bytes = fs::read(&dcm_path).expect("generated DICOM file should be readable");
    assert_eq!(&dcm_bytes[128..132], b"DICM", "file must be Part 10");
    assert_eq!(
        file_entry.get("size_bytes").and_then(Value::as_u64),
        Some(dcm_bytes.len() as u64)
    );
    assert_eq!(
        file_entry.get("sha256").and_then(Value::as_str),
        Some(dicom_test_suite::sha256_hex(&dcm_bytes).as_str())
    );

    let obj = open_file(&dcm_path).expect("generated DICOM file should parse");
    let sop_class_uid = obj
        .element(tags::SOP_CLASS_UID)
        .expect("dataset should contain SOP Class UID")
        .value()
        .to_str()
        .expect("SOP Class UID should be text");
    let sop_instance_uid = obj
        .element(tags::SOP_INSTANCE_UID)
        .expect("dataset should contain SOP Instance UID")
        .value()
        .to_str()
        .expect("SOP Instance UID should be text");
    let synthetic_data = obj
        .element(tags::SYNTHETIC_DATA)
        .expect("dataset should contain Synthetic Data")
        .value()
        .to_str()
        .expect("Synthetic Data should be text");
    assert_eq!(
        sop_class_uid.trim_end_matches('\0'),
        obj.meta()
            .media_storage_sop_class_uid()
            .trim_end_matches('\0')
    );
    assert_eq!(
        sop_instance_uid.trim_end_matches('\0'),
        obj.meta()
            .media_storage_sop_instance_uid()
            .trim_end_matches('\0')
    );
    assert_eq!(synthetic_data.trim(), "YES");
    assert_eq!(
        manifest
            .pointer("/files/0/uids/sop_instance_uid")
            .and_then(Value::as_str),
        Some(sop_instance_uid.trim_end_matches('\0'))
    );
    assert!(
        manifest
            .pointer("/skipped_cases")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "manifest should not skip smoke cases once all smoke recipes are generated"
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn generate_command_writes_core_u16_native_pixel_case() {
    let out_dir = unique_temp_dir("generate-core-command");

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
    let stdout = String::from_utf8(output.stdout).expect("generate stdout must be utf-8");
    assert!(stdout.contains("profile\tcore"));
    assert!(stdout.contains("files_written\t19"));

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    assert_eq!(
        manifest.pointer("/run/profile").and_then(Value::as_str),
        Some("core")
    );
    assert_eq!(
        manifest
            .pointer("/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(19)
    );
    let u16_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_explicit_le");
    assert_eq!(
        u16_file.pointer("/pixel_data/vr").and_then(Value::as_str),
        Some("OW")
    );
    assert_eq!(
        u16_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        u16_file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        u16_file
            .pointer("/image/bits_stored")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        u16_file.pointer("/image/high_bit").and_then(Value::as_u64),
        Some(15)
    );
    assert_eq!(
        u16_file
            .pointer("/expected_semantics/pixel_max")
            .and_then(Value::as_u64),
        Some(65535)
    );
    let i16_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_i16_explicit_le");
    assert_eq!(
        i16_file
            .pointer("/image/pixel_representation")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        i16_file
            .pointer("/expected_semantics/pixel_min")
            .and_then(Value::as_i64),
        Some(-32768)
    );
    assert_eq!(
        i16_file
            .pointer("/expected_semantics/pixel_max")
            .and_then(Value::as_i64),
        Some(32767)
    );
    assert_eq!(
        i16_file.pointer("/pixel_data/vr").and_then(Value::as_str),
        Some("OW")
    );
    let rgb_planar1_file = file_entry_by_case_id(&manifest, "classic/sc/rgb_planar1_explicit_le");
    assert_eq!(
        rgb_planar1_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        rgb_planar1_file
            .pointer("/image/samples_per_pixel")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        rgb_planar1_file
            .pointer("/image/planar_configuration")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rgb_planar1_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(12)
    );
    let palette_file = file_entry_by_case_id(&manifest, "classic/sc/palette_color_u8_explicit_le");
    assert_eq!(
        palette_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("PALETTE COLOR")
    );
    assert_eq!(
        palette_file
            .pointer("/image/samples_per_pixel")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        palette_file
            .pointer("/image/planar_configuration")
            .and_then(Value::as_u64),
        None
    );
    assert_eq!(
        palette_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        palette_file.pointer("/recipe/recipe_parameters/palette/descriptor"),
        Some(&serde_json::json!([4, 0, 16]))
    );
    let ybr_file = file_entry_by_case_id(&manifest, "classic/sc/ybr_full_planar0_explicit_le");
    assert_eq!(
        ybr_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("YBR_FULL")
    );
    assert_eq!(
        ybr_file
            .pointer("/image/samples_per_pixel")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        ybr_file
            .pointer("/image/planar_configuration")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        ybr_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(12)
    );
    let ybr_422_file = file_entry_by_case_id(&manifest, "classic/sc/ybr_full_422_explicit_le");
    assert_eq!(
        ybr_422_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("YBR_FULL_422")
    );
    assert_eq!(
        ybr_422_file
            .pointer("/image/samples_per_pixel")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        ybr_422_file
            .pointer("/image/planar_configuration")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        ybr_422_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert!(
        validation_result_names(ybr_422_file.pointer("/validation/internal"))
            .contains(&"native_ybr_full_422_pixel_data_length"),
        "YBR_FULL_422 manifest should record the special native byte-length validation"
    );
    let odd_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_odd_3x3_explicit_le");
    assert_eq!(
        odd_file.pointer("/image/rows").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        odd_file.pointer("/image/columns").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        odd_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(18)
    );
    let rect_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_rect_2x3_explicit_le");
    assert_eq!(
        rect_file.pointer("/image/rows").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rect_file.pointer("/image/columns").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        rect_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(12)
    );
    let tiny_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_tiny_1x1_explicit_le");
    assert_eq!(
        tiny_file.pointer("/image/rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        tiny_file.pointer("/image/columns").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        tiny_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(2)
    );
    let padding_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_padding_explicit_le");
    assert_eq!(
        padding_file.pointer("/recipe/recipe_parameters/pixel_padding/value"),
        Some(&serde_json::json!(0))
    );
    assert_eq!(
        padding_file.pointer("/recipe/recipe_parameters/pixel_padding/range_limit"),
        Some(&serde_json::json!(0))
    );
    assert!(
        validation_result_names(padding_file.pointer("/validation/internal"))
            .contains(&"pixel_padding_value"),
        "padding manifest should record Pixel Padding Value validation"
    );
    assert!(
        validation_result_names(padding_file.pointer("/validation/internal"))
            .contains(&"pixel_padding_range_limit"),
        "padding manifest should record Pixel Padding Range Limit validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"pixel_data_vr"),
        "manifest should record Pixel Data VR validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"native_pixel_data_length"),
        "manifest should record native Pixel Data length validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"bits_stored_within_bits_allocated"),
        "manifest should record Bits Stored invariant validation"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/internal")
            .contains(&"photometric_planar_configuration_presence"),
        "manifest should record photometric planar configuration validation"
    );
    let ct_file =
        file_entry_by_case_id(&manifest, "classic/ct/mono2_i16_rescale_12bit_explicit_le");
    assert_eq!(
        ct_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::CT_IMAGE_STORAGE)
    );
    assert_eq!(
        ct_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("CT")
    );
    assert_eq!(
        ct_file
            .pointer("/image/bits_stored")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        ct_file.pointer("/image/high_bit").and_then(Value::as_u64),
        Some(11)
    );
    assert_eq!(
        ct_file
            .pointer("/recipe/recipe_parameters/rescale/intercept")
            .and_then(Value::as_str),
        Some("-1024")
    );
    assert_eq!(
        ct_file
            .pointer("/recipe/recipe_parameters/window/width")
            .and_then(Value::as_str),
        Some("400")
    );
    assert!(
        validation_result_names(ct_file.pointer("/validation/internal"))
            .contains(&"ct_rescale_intercept"),
        "CT manifest should record Rescale Intercept validation"
    );
    assert!(
        validation_result_names(ct_file.pointer("/validation/internal"))
            .contains(&"ct_window_width"),
        "CT manifest should record Window Width validation"
    );
    assert!(
        validation_result_names(ct_file.pointer("/validation/standards"))
            .contains(&"ct_image_sop_class"),
        "CT manifest should record standards validation for CT Image Storage"
    );
    let mg_file = file_entry_by_case_id(
        &manifest,
        "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
    );
    assert_eq!(
        mg_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION)
    );
    assert_eq!(
        mg_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("MG")
    );
    assert_eq!(
        mg_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mg_file
            .pointer("/image/bits_stored")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        mg_file
            .pointer("/recipe/recipe_parameters/presentation_intent_type")
            .and_then(Value::as_str),
        Some("FOR PRESENTATION")
    );
    assert_eq!(
        mg_file
            .pointer("/recipe/recipe_parameters/presentation_lut_shape")
            .and_then(Value::as_str),
        Some("INVERSE")
    );
    assert!(
        validation_result_names(mg_file.pointer("/validation/internal"))
            .contains(&"mg_presentation_intent_type"),
        "MG manifest should record Presentation Intent Type validation"
    );
    assert!(
        validation_result_names(mg_file.pointer("/validation/internal"))
            .contains(&"mg_view_code_sequence"),
        "MG manifest should record View Code Sequence validation"
    );
    assert!(
        validation_result_names(mg_file.pointer("/validation/standards"))
            .contains(&"digital_mammography_for_presentation_sop_class"),
        "MG manifest should record standards validation for mammography SOP Class"
    );
    let mg_processing_file = file_entry_by_case_id(
        &manifest,
        "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
    );
    assert_eq!(
        mg_processing_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING)
    );
    assert_eq!(
        mg_processing_file
            .pointer("/dicom/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some(uids::IMPLICIT_VR_LITTLE_ENDIAN)
    );
    assert_eq!(
        mg_processing_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        mg_processing_file
            .pointer("/recipe/recipe_parameters/presentation_intent_type")
            .and_then(Value::as_str),
        Some("FOR PROCESSING")
    );
    assert_eq!(
        mg_processing_file
            .pointer("/recipe/recipe_parameters/presentation_lut_shape")
            .and_then(Value::as_str),
        Some("IDENTITY")
    );
    assert_eq!(
        mg_processing_file.pointer("/recipe/recipe_parameters/window/center"),
        Some(&Value::Null)
    );
    assert!(
        validation_result_names(mg_processing_file.pointer("/validation/internal"))
            .contains(&"mg_window_center_absent"),
        "MG For Processing manifest should record absent Window Center validation"
    );
    assert!(
        validation_result_names(mg_processing_file.pointer("/validation/standards"))
            .contains(&"digital_mammography_for_processing_sop_class"),
        "MG For Processing manifest should record standards validation for its SOP Class"
    );
    assert!(
        validation_result_names(mg_processing_file.pointer("/validation/standards"))
            .contains(&"implicit_vr_little_endian_transfer_syntax"),
        "MG For Processing manifest should record standards validation for Implicit VR LE"
    );
    let cr_file = file_entry_by_case_id(&manifest, "classic/cr/overlay_modality_voi_explicit_le");
    assert_eq!(
        cr_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE)
    );
    assert_eq!(
        cr_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("CR")
    );
    assert_eq!(
        cr_file
            .pointer("/recipe/recipe_parameters/overlay/value_length")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        cr_file
            .pointer("/recipe/recipe_parameters/modality_lut/descriptor")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        cr_file
            .pointer("/recipe/recipe_parameters/voi_lut/data_value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert!(
        validation_result_names(cr_file.pointer("/validation/internal"))
            .contains(&"cr_overlay_data"),
        "CR manifest should record Overlay Data validation"
    );
    assert!(
        validation_result_names(cr_file.pointer("/validation/internal"))
            .contains(&"cr_modality_lut_descriptor"),
        "CR manifest should record Modality LUT validation"
    );
    assert!(
        validation_result_names(cr_file.pointer("/validation/internal"))
            .contains(&"cr_voi_lut_descriptor"),
        "CR manifest should record VOI LUT validation"
    );
    assert!(
        validation_result_names(cr_file.pointer("/validation/standards"))
            .contains(&"computed_radiography_image_sop_class"),
        "CR manifest should record standards validation for Computed Radiography Image Storage"
    );
    let dx_file = file_entry_by_case_id(
        &manifest,
        "classic/dx/display_shutter_mono2_u16_explicit_le",
    );
    assert_eq!(
        dx_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION)
    );
    assert_eq!(
        dx_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("DX")
    );
    assert_eq!(
        dx_file
            .pointer("/recipe/recipe_parameters/presentation_intent_type")
            .and_then(Value::as_str),
        Some("FOR PRESENTATION")
    );
    assert_eq!(
        dx_file
            .pointer("/recipe/recipe_parameters/display_shutter/shape")
            .and_then(Value::as_str),
        Some("RECTANGULAR")
    );
    assert_eq!(
        dx_file
            .pointer("/recipe/recipe_parameters/display_shutter/presentation_value")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert!(
        validation_result_names(dx_file.pointer("/validation/internal"))
            .contains(&"dx_shutter_shape"),
        "DX manifest should record Shutter Shape validation"
    );
    assert!(
        validation_result_names(dx_file.pointer("/validation/internal"))
            .contains(&"dx_shutter_presentation_value"),
        "DX manifest should record Shutter Presentation Value validation"
    );
    assert!(
        validation_result_names(dx_file.pointer("/validation/standards"))
            .contains(&"digital_x_ray_for_presentation_sop_class"),
        "DX manifest should record standards validation for Digital X-Ray Image Storage"
    );
    let us_file = file_entry_by_case_id(&manifest, "classic/us/mono2_u8_explicit_le");
    assert_eq!(
        us_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::ULTRASOUND_IMAGE_STORAGE)
    );
    assert_eq!(
        us_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("US")
    );
    assert_eq!(
        us_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        us_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        us_file
            .pointer("/recipe/recipe_parameters/lossy_image_compression")
            .and_then(Value::as_str),
        Some("00")
    );
    assert_eq!(
        us_file
            .pointer("/recipe/recipe_parameters/ultrasound_color_data_present")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert!(
        validation_result_names(us_file.pointer("/validation/internal"))
            .contains(&"us_ultrasound_color_data_present"),
        "US manifest should record Ultrasound Color Data Present validation"
    );
    assert!(
        validation_result_names(us_file.pointer("/validation/standards"))
            .contains(&"ultrasound_image_sop_class"),
        "US manifest should record standards validation for Ultrasound Image Storage"
    );
    let mr_files = file_entries_by_case_id(&manifest, "classic/mr/multislice_oblique_explicit_le");
    assert_eq!(
        mr_files.len(),
        3,
        "MR case should generate a three-instance series"
    );
    assert!(
        mr_files.iter().all(|file| {
            file.pointer("/dicom/sop_class_uid").and_then(Value::as_str)
                == Some(uids::MR_IMAGE_STORAGE)
        }),
        "all MR files should use MR Image Storage"
    );
    assert!(
        mr_files.iter().all(|file| {
            file.pointer("/uids/study_instance_uid")
                == mr_files[0].pointer("/uids/study_instance_uid")
                && file.pointer("/uids/series_instance_uid")
                    == mr_files[0].pointer("/uids/series_instance_uid")
                && file.pointer("/uids/frame_of_reference_uid")
                    == mr_files[0].pointer("/uids/frame_of_reference_uid")
        }),
        "MR files should share Study, Series, and Frame of Reference UIDs"
    );
    assert_eq!(
        mr_files
            .iter()
            .map(|file| {
                file.pointer("/recipe/recipe_parameters/geometry/slice_order_index")
                    .and_then(Value::as_u64)
                    .expect("MR file should record slice order index")
            })
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        mr_files
            .iter()
            .map(|file| {
                file.pointer("/recipe/recipe_parameters/geometry/position_along_normal")
                    .and_then(Value::as_f64)
                    .expect("MR file should record position along normal")
            })
            .collect::<Vec<_>>(),
        vec![0.0, 5.0, 10.0]
    );
    assert!(
        validation_result_names(mr_files[0].pointer("/validation/internal"))
            .contains(&"mr_position_along_normal"),
        "MR manifest should record computed geometry sorting validation"
    );
    assert!(
        validation_result_names(mr_files[0].pointer("/validation/standards"))
            .contains(&"mr_image_sop_class"),
        "MR manifest should record standards validation for MR Image Storage"
    );
    assert!(
        manifest
            .pointer("/skipped_cases")
            .and_then(Value::as_array)
            .is_some_and(|cases| {
                cases.iter().all(|case| {
                    !matches!(
                        case.get("case_id").and_then(Value::as_str),
                        Some("classic/ct/mono2_i16_rescale_12bit_explicit_le")
                            | Some("classic/mg/for_presentation_mono1_u16_12bit_explicit_le")
                            | Some("classic/mg/for_processing_mono2_u16_12bit_implicit_le")
                            | Some("classic/cr/overlay_modality_voi_explicit_le")
                            | Some("classic/dx/display_shutter_mono2_u16_explicit_le")
                            | Some("classic/us/mono2_u8_explicit_le")
                            | Some("classic/mr/multislice_oblique_explicit_le")
                    )
                })
            }),
        "implemented classic radiology cases should not be reported as skipped"
    );

    let dcm_path = out_dir.join("classic/sc/mono2_u16_explicit_le/instance.dcm");
    let obj = open_file(&dcm_path).expect("generated DICOM file should parse");
    assert_eq!(
        obj.element(tags::PIXEL_DATA)
            .expect("dataset should contain Pixel Data")
            .vr(),
        dicom_core::VR::OW
    );
    assert_eq!(
        obj.element(tags::BITS_ALLOCATED)
            .expect("dataset should contain Bits Allocated")
            .value()
            .to_int::<u16>()
            .expect("Bits Allocated should be numeric"),
        16
    );
    let signed_path = out_dir.join("classic/sc/mono2_i16_explicit_le/instance.dcm");
    let signed = open_file(&signed_path).expect("signed generated DICOM file should parse");
    assert_eq!(
        signed
            .element(tags::PIXEL_REPRESENTATION)
            .expect("dataset should contain Pixel Representation")
            .value()
            .to_int::<u16>()
            .expect("Pixel Representation should be numeric"),
        1
    );
    let planar1_path = out_dir.join("classic/sc/rgb_planar1_explicit_le/instance.dcm");
    let planar1 = open_file(&planar1_path).expect("RGB planar1 generated DICOM file should parse");
    assert_eq!(
        planar1
            .element(tags::PLANAR_CONFIGURATION)
            .expect("dataset should contain Planar Configuration")
            .value()
            .to_int::<u16>()
            .expect("Planar Configuration should be numeric"),
        1
    );
    let palette_path = out_dir.join("classic/sc/palette_color_u8_explicit_le/instance.dcm");
    let palette = open_file(&palette_path).expect("palette generated DICOM file should parse");
    assert_eq!(
        palette
            .element(tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR)
            .expect("dataset should contain Red Palette Color Lookup Table Descriptor")
            .value()
            .to_multi_int::<u16>()
            .expect("Red Palette Color Lookup Table Descriptor should be numeric"),
        vec![4, 0, 16]
    );
    let red_palette_data = palette
        .element(tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA)
        .expect("dataset should contain Red Palette Color Lookup Table Data");
    assert_eq!(red_palette_data.vr(), dicom_core::VR::OW);
    assert_eq!(
        red_palette_data
            .value()
            .to_bytes()
            .expect("Red Palette Color Lookup Table Data should be byte-backed")
            .len(),
        8
    );
    let ybr_path = out_dir.join("classic/sc/ybr_full_planar0_explicit_le/instance.dcm");
    let ybr = open_file(&ybr_path).expect("YBR_FULL generated DICOM file should parse");
    assert_eq!(
        ybr.element(tags::PHOTOMETRIC_INTERPRETATION)
            .expect("dataset should contain Photometric Interpretation")
            .value()
            .to_str()
            .expect("Photometric Interpretation should be text")
            .trim(),
        "YBR_FULL"
    );
    assert_eq!(
        ybr.element(tags::PLANAR_CONFIGURATION)
            .expect("dataset should contain Planar Configuration")
            .value()
            .to_int::<u16>()
            .expect("Planar Configuration should be numeric"),
        0
    );
    let ybr_422_path = out_dir.join("classic/sc/ybr_full_422_explicit_le/instance.dcm");
    let ybr_422 = open_file(&ybr_422_path).expect("YBR_FULL_422 generated DICOM file should parse");
    assert_eq!(
        ybr_422
            .element(tags::PHOTOMETRIC_INTERPRETATION)
            .expect("dataset should contain Photometric Interpretation")
            .value()
            .to_str()
            .expect("Photometric Interpretation should be text")
            .trim(),
        "YBR_FULL_422"
    );
    assert_eq!(
        ybr_422
            .element(tags::PIXEL_DATA)
            .expect("dataset should contain Pixel Data")
            .value()
            .to_bytes()
            .expect("Pixel Data should be byte-backed")
            .len(),
        8
    );
    let odd_path = out_dir.join("classic/sc/mono2_u16_odd_3x3_explicit_le/instance.dcm");
    let odd = open_file(&odd_path).expect("odd-dimension generated DICOM file should parse");
    assert_eq!(
        odd.element(tags::ROWS)
            .expect("dataset should contain Rows")
            .value()
            .to_int::<u16>()
            .expect("Rows should be numeric"),
        3
    );
    assert_eq!(
        odd.element(tags::COLUMNS)
            .expect("dataset should contain Columns")
            .value()
            .to_int::<u16>()
            .expect("Columns should be numeric"),
        3
    );
    let tiny_path = out_dir.join("classic/sc/mono2_u16_tiny_1x1_explicit_le/instance.dcm");
    let tiny = open_file(&tiny_path).expect("tiny generated DICOM file should parse");
    assert_eq!(
        tiny.element(tags::PIXEL_DATA)
            .expect("dataset should contain Pixel Data")
            .value()
            .to_bytes()
            .expect("Pixel Data should be byte-backed")
            .len(),
        2
    );
    let padding_path = out_dir.join("classic/sc/mono2_u16_padding_explicit_le/instance.dcm");
    let padding =
        open_file(&padding_path).expect("pixel-padding generated DICOM file should parse");
    assert_eq!(
        padding
            .element(tags::PIXEL_PADDING_VALUE)
            .expect("dataset should contain Pixel Padding Value")
            .value()
            .to_int::<u16>()
            .expect("Pixel Padding Value should be numeric"),
        0
    );
    assert_eq!(
        padding
            .element(tags::PIXEL_PADDING_RANGE_LIMIT)
            .expect("dataset should contain Pixel Padding Range Limit")
            .value()
            .to_int::<u16>()
            .expect("Pixel Padding Range Limit should be numeric"),
        0
    );
    let ct_path = out_dir.join("classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm");
    let ct = open_file(&ct_path).expect("CT generated DICOM file should parse");
    assert_eq!(
        ct.element(tags::SOP_CLASS_UID)
            .expect("dataset should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::CT_IMAGE_STORAGE
    );
    assert_eq!(
        ct.element(tags::MODALITY)
            .expect("dataset should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "CT"
    );
    assert_eq!(
        ct.element(tags::BITS_STORED)
            .expect("dataset should contain Bits Stored")
            .value()
            .to_int::<u16>()
            .expect("Bits Stored should be numeric"),
        12
    );
    assert_eq!(
        ct.element(tags::RESCALE_INTERCEPT)
            .expect("dataset should contain Rescale Intercept")
            .value()
            .to_str()
            .expect("Rescale Intercept should be text")
            .trim(),
        "-1024"
    );
    assert_eq!(
        ct.element(tags::WINDOW_CENTER)
            .expect("dataset should contain Window Center")
            .value()
            .to_str()
            .expect("Window Center should be text")
            .trim(),
        "40"
    );
    assert_eq!(
        ct.element(tags::FRAME_OF_REFERENCE_UID)
            .expect("dataset should contain Frame of Reference UID")
            .value()
            .to_str()
            .expect("Frame of Reference UID should be text")
            .trim_end_matches('\0'),
        ct_file
            .pointer("/uids/frame_of_reference_uid")
            .and_then(Value::as_str)
            .expect("manifest should record CT Frame of Reference UID")
    );
    let mg_path =
        out_dir.join("classic/mg/for_presentation_mono1_u16_12bit_explicit_le/instance.dcm");
    let mg = open_file(&mg_path).expect("MG generated DICOM file should parse");
    assert_eq!(
        mg.element(tags::SOP_CLASS_UID)
            .expect("dataset should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION
    );
    assert_eq!(
        mg.element(tags::PRESENTATION_INTENT_TYPE)
            .expect("dataset should contain Presentation Intent Type")
            .value()
            .to_str()
            .expect("Presentation Intent Type should be text")
            .trim(),
        "FOR PRESENTATION"
    );
    assert_eq!(
        mg.element(tags::PHOTOMETRIC_INTERPRETATION)
            .expect("dataset should contain Photometric Interpretation")
            .value()
            .to_str()
            .expect("Photometric Interpretation should be text")
            .trim(),
        "MONOCHROME1"
    );
    assert_eq!(
        mg.element(tags::PRESENTATION_LUT_SHAPE)
            .expect("dataset should contain Presentation LUT Shape")
            .value()
            .to_str()
            .expect("Presentation LUT Shape should be text")
            .trim(),
        "INVERSE"
    );
    assert_eq!(
        mg.element(tags::PIXEL_INTENSITY_RELATIONSHIP_SIGN)
            .expect("dataset should contain Pixel Intensity Relationship Sign")
            .value()
            .to_int::<i16>()
            .expect("Pixel Intensity Relationship Sign should be numeric"),
        -1
    );
    assert_eq!(
        mg.element(tags::IMAGER_PIXEL_SPACING)
            .expect("dataset should contain Imager Pixel Spacing")
            .value()
            .to_str()
            .expect("Imager Pixel Spacing should be text")
            .trim(),
        "0.070\\0.070"
    );
    assert_eq!(
        mg.element(tags::VIEW_CODE_SEQUENCE)
            .expect("dataset should contain View Code Sequence")
            .items()
            .expect("View Code Sequence should contain items")
            .len(),
        1
    );
    let mg_processing_path =
        out_dir.join("classic/mg/for_processing_mono2_u16_12bit_implicit_le/instance.dcm");
    let mg_processing = open_file(&mg_processing_path)
        .expect("MG For Processing generated DICOM file should parse");
    assert_eq!(
        mg_processing
            .meta()
            .transfer_syntax()
            .trim_end_matches('\0'),
        uids::IMPLICIT_VR_LITTLE_ENDIAN
    );
    assert_eq!(
        mg_processing
            .element(tags::SOP_CLASS_UID)
            .expect("dataset should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING
    );
    assert_eq!(
        mg_processing
            .element(tags::PRESENTATION_INTENT_TYPE)
            .expect("dataset should contain Presentation Intent Type")
            .value()
            .to_str()
            .expect("Presentation Intent Type should be text")
            .trim(),
        "FOR PROCESSING"
    );
    assert_eq!(
        mg_processing
            .element(tags::PHOTOMETRIC_INTERPRETATION)
            .expect("dataset should contain Photometric Interpretation")
            .value()
            .to_str()
            .expect("Photometric Interpretation should be text")
            .trim(),
        "MONOCHROME2"
    );
    assert_eq!(
        mg_processing
            .element(tags::PRESENTATION_LUT_SHAPE)
            .expect("dataset should contain Presentation LUT Shape")
            .value()
            .to_str()
            .expect("Presentation LUT Shape should be text")
            .trim(),
        "IDENTITY"
    );
    assert!(
        mg_processing
            .element_opt(tags::WINDOW_CENTER)
            .expect("Window Center lookup should succeed")
            .is_none(),
        "MG For Processing should omit Window Center"
    );
    assert!(
        mg_processing
            .element_opt(tags::WINDOW_WIDTH)
            .expect("Window Width lookup should succeed")
            .is_none(),
        "MG For Processing should omit Window Width"
    );
    let cr_path = out_dir.join("classic/cr/overlay_modality_voi_explicit_le/instance.dcm");
    let cr = open_file(&cr_path).expect("CR generated DICOM file should parse");
    assert_eq!(
        cr.element(tags::SOP_CLASS_UID)
            .expect("dataset should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE
    );
    assert_eq!(
        cr.element(tags::OVERLAY_ROWS.inner())
            .expect("dataset should contain Overlay Rows")
            .value()
            .to_int::<u16>()
            .expect("Overlay Rows should be numeric"),
        2
    );
    assert_eq!(
        cr.element(tags::OVERLAY_DATA.inner())
            .expect("dataset should contain Overlay Data")
            .value()
            .to_bytes()
            .expect("Overlay Data should be byte-backed")
            .as_ref(),
        &[0x09, 0x00]
    );
    let modality_lut = cr
        .element(tags::MODALITY_LUT_SEQUENCE)
        .expect("dataset should contain Modality LUT Sequence")
        .items()
        .expect("Modality LUT Sequence should contain items")
        .first()
        .expect("Modality LUT Sequence should contain one item");
    assert_eq!(
        modality_lut
            .element(tags::LUT_DESCRIPTOR)
            .expect("Modality LUT item should contain LUT Descriptor")
            .value()
            .to_multi_int::<u16>()
            .expect("LUT Descriptor should be numeric"),
        vec![4, 0, 16]
    );
    assert_eq!(
        modality_lut
            .element(tags::MODALITY_LUT_TYPE)
            .expect("Modality LUT item should contain Modality LUT Type")
            .value()
            .to_str()
            .expect("Modality LUT Type should be text")
            .trim(),
        "US"
    );
    let voi_lut = cr
        .element(tags::VOILUT_SEQUENCE)
        .expect("dataset should contain VOI LUT Sequence")
        .items()
        .expect("VOI LUT Sequence should contain items")
        .first()
        .expect("VOI LUT Sequence should contain one item");
    assert_eq!(
        voi_lut
            .element(tags::LUT_DATA)
            .expect("VOI LUT item should contain LUT Data")
            .value()
            .to_bytes()
            .expect("VOI LUT Data should be byte-backed")
            .len(),
        8
    );
    let dx_path = out_dir.join("classic/dx/display_shutter_mono2_u16_explicit_le/instance.dcm");
    let dx = open_file(&dx_path).expect("DX generated DICOM file should parse");
    assert_eq!(
        dx.element(tags::SOP_CLASS_UID)
            .expect("DX file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION
    );
    assert_eq!(
        dx.element(tags::MODALITY)
            .expect("DX file should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "DX"
    );
    assert_eq!(
        dx.element(tags::PRESENTATION_INTENT_TYPE)
            .expect("DX file should contain Presentation Intent Type")
            .value()
            .to_str()
            .expect("Presentation Intent Type should be text")
            .trim(),
        "FOR PRESENTATION"
    );
    assert_eq!(
        dx.element(tags::SHUTTER_SHAPE)
            .expect("DX file should contain Shutter Shape")
            .value()
            .to_str()
            .expect("Shutter Shape should be text")
            .trim(),
        "RECTANGULAR"
    );
    assert_eq!(
        dx.element(tags::SHUTTER_LEFT_VERTICAL_EDGE)
            .expect("DX file should contain Shutter Left Vertical Edge")
            .value()
            .to_str()
            .expect("Shutter edge should be text")
            .trim(),
        "1"
    );
    assert_eq!(
        dx.element(tags::SHUTTER_RIGHT_VERTICAL_EDGE)
            .expect("DX file should contain Shutter Right Vertical Edge")
            .value()
            .to_str()
            .expect("Shutter edge should be text")
            .trim(),
        "2"
    );
    assert_eq!(
        dx.element(tags::SHUTTER_UPPER_HORIZONTAL_EDGE)
            .expect("DX file should contain Shutter Upper Horizontal Edge")
            .value()
            .to_str()
            .expect("Shutter edge should be text")
            .trim(),
        "1"
    );
    assert_eq!(
        dx.element(tags::SHUTTER_LOWER_HORIZONTAL_EDGE)
            .expect("DX file should contain Shutter Lower Horizontal Edge")
            .value()
            .to_str()
            .expect("Shutter edge should be text")
            .trim(),
        "2"
    );
    assert_eq!(
        dx.element(tags::SHUTTER_PRESENTATION_VALUE)
            .expect("DX file should contain Shutter Presentation Value")
            .value()
            .to_int::<u16>()
            .expect("Shutter Presentation Value should be numeric"),
        0
    );
    let us_path = out_dir.join("classic/us/mono2_u8_explicit_le/instance.dcm");
    let us = open_file(&us_path).expect("US generated DICOM file should parse");
    assert_eq!(
        us.element(tags::SOP_CLASS_UID)
            .expect("US file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ULTRASOUND_IMAGE_STORAGE
    );
    assert_eq!(
        us.element(tags::MODALITY)
            .expect("US file should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "US"
    );
    assert_eq!(
        us.element(tags::LOSSY_IMAGE_COMPRESSION)
            .expect("US file should contain Lossy Image Compression")
            .value()
            .to_str()
            .expect("Lossy Image Compression should be text")
            .trim(),
        "00"
    );
    assert_eq!(
        us.element(tags::ULTRASOUND_COLOR_DATA_PRESENT)
            .expect("US file should contain Ultrasound Color Data Present")
            .value()
            .to_int::<u16>()
            .expect("Ultrasound Color Data Present should be numeric"),
        0
    );
    let mr_slice_paths = [
        out_dir.join("classic/mr/multislice_oblique_explicit_le/slice-001.dcm"),
        out_dir.join("classic/mr/multislice_oblique_explicit_le/slice-002.dcm"),
        out_dir.join("classic/mr/multislice_oblique_explicit_le/slice-003.dcm"),
    ];
    let mr_slices = mr_slice_paths
        .iter()
        .map(|path| open_file(path).expect("MR generated DICOM file should parse"))
        .collect::<Vec<_>>();
    let mr_study_uid = mr_slices[0]
        .element(tags::STUDY_INSTANCE_UID)
        .expect("MR file should contain Study Instance UID")
        .value()
        .to_str()
        .expect("Study Instance UID should be text")
        .trim_end_matches('\0')
        .to_string();
    let mr_series_uid = mr_slices[0]
        .element(tags::SERIES_INSTANCE_UID)
        .expect("MR file should contain Series Instance UID")
        .value()
        .to_str()
        .expect("Series Instance UID should be text")
        .trim_end_matches('\0')
        .to_string();
    let mr_frame_uid = mr_slices[0]
        .element(tags::FRAME_OF_REFERENCE_UID)
        .expect("MR file should contain Frame of Reference UID")
        .value()
        .to_str()
        .expect("Frame of Reference UID should be text")
        .trim_end_matches('\0')
        .to_string();
    let mut mr_positions = Vec::new();
    for (index, mr) in mr_slices.iter().enumerate() {
        assert_eq!(
            mr.element(tags::SOP_CLASS_UID)
                .expect("MR file should contain SOP Class UID")
                .value()
                .to_str()
                .expect("SOP Class UID should be text")
                .trim_end_matches('\0'),
            uids::MR_IMAGE_STORAGE
        );
        assert_eq!(
            mr.element(tags::STUDY_INSTANCE_UID)
                .expect("MR file should contain Study Instance UID")
                .value()
                .to_str()
                .expect("Study Instance UID should be text")
                .trim_end_matches('\0'),
            mr_study_uid
        );
        assert_eq!(
            mr.element(tags::SERIES_INSTANCE_UID)
                .expect("MR file should contain Series Instance UID")
                .value()
                .to_str()
                .expect("Series Instance UID should be text")
                .trim_end_matches('\0'),
            mr_series_uid
        );
        assert_eq!(
            mr.element(tags::FRAME_OF_REFERENCE_UID)
                .expect("MR file should contain Frame of Reference UID")
                .value()
                .to_str()
                .expect("Frame of Reference UID should be text")
                .trim_end_matches('\0'),
            mr_frame_uid
        );
        assert_eq!(
            mr.element(tags::INSTANCE_NUMBER)
                .expect("MR file should contain Instance Number")
                .value()
                .to_str()
                .expect("Instance Number should be text")
                .trim(),
            (index + 1).to_string()
        );
        assert_eq!(
            mr.element(tags::IMAGE_ORIENTATION_PATIENT)
                .expect("MR file should contain Image Orientation Patient")
                .value()
                .to_str()
                .expect("Image Orientation Patient should be text")
                .trim(),
            "0.70710678\\0.70710678\\0\\0\\0\\1"
        );
        mr_positions.push(
            mr.element(tags::IMAGE_POSITION_PATIENT)
                .expect("MR file should contain Image Position Patient")
                .value()
                .to_str()
                .expect("Image Position Patient should be text")
                .trim()
                .to_string(),
        );
    }
    assert_eq!(
        mr_positions,
        vec![
            "0\\0\\0".to_string(),
            "3.535534\\-3.535534\\0".to_string(),
            "7.071068\\-7.071068\\0".to_string()
        ],
        "MR slices should advance along the oblique slice normal"
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn generate_command_writes_extended_enhanced_ct_multiframe_case() {
    let out_dir = unique_temp_dir("generate-extended-command");

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
    let stdout = String::from_utf8(output.stdout).expect("generate stdout must be utf-8");
    assert!(stdout.contains("profile\textended"));
    assert!(stdout.contains("files_written\t3"));

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    assert_eq!(
        manifest
            .pointer("/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
    let enhanced_ct_file = file_entry_by_case_id(
        &manifest,
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
    );
    assert_eq!(
        enhanced_ct_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::ENHANCED_CT_IMAGE_STORAGE)
    );
    assert_eq!(
        enhanced_ct_file
            .pointer("/image/frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        enhanced_ct_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        enhanced_ct_file
            .pointer(
                "/recipe/recipe_parameters/shared_functional_groups/pixel_measures/pixel_spacing"
            )
            .and_then(Value::as_str),
        Some("0.75\\0.75")
    );
    assert_eq!(
        enhanced_ct_file
            .pointer("/recipe/recipe_parameters/per_frame_functional_groups/image_position_patient")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(
        validation_result_names(enhanced_ct_file.pointer("/validation/internal"))
            .contains(&"enhanced_ct_per_frame_functional_groups_sequence_items"),
        "Enhanced CT manifest should record Per-Frame Functional Groups validation"
    );
    assert!(
        validation_result_names(enhanced_ct_file.pointer("/validation/standards"))
            .contains(&"enhanced_ct_image_sop_class"),
        "Enhanced CT manifest should record standards validation for Enhanced CT Image Storage"
    );
    let enhanced_mr_file = file_entry_by_case_id(
        &manifest,
        "enhanced/mr/multiframe_echo_perframe_explicit_le",
    );
    assert_eq!(
        enhanced_mr_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::ENHANCED_MR_IMAGE_STORAGE)
    );
    assert_eq!(
        enhanced_mr_file
            .pointer("/image/frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        enhanced_mr_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        enhanced_mr_file
            .pointer("/recipe/recipe_parameters/per_frame_functional_groups/effective_echo_time/1")
            .and_then(Value::as_f64),
        Some(24.5)
    );
    assert!(
        validation_result_names(enhanced_mr_file.pointer("/validation/internal"))
            .contains(&"enhanced_mr_per_frame_effective_echo_time"),
        "Enhanced MR manifest should record per-frame MR Echo validation"
    );
    assert!(
        validation_result_names(enhanced_mr_file.pointer("/validation/standards"))
            .contains(&"enhanced_mr_image_sop_class"),
        "Enhanced MR manifest should record standards validation for Enhanced MR Image Storage"
    );
    let enhanced_mr_temporal_file = file_entry_by_case_id(
        &manifest,
        "enhanced/mr/multiframe_temporal_position_explicit_le",
    );
    assert_eq!(
        enhanced_mr_temporal_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::ENHANCED_MR_IMAGE_STORAGE)
    );
    assert_eq!(
        enhanced_mr_temporal_file
            .pointer("/image/frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        enhanced_mr_temporal_file
            .pointer("/recipe/recipe_parameters/frame_type")
            .and_then(Value::as_str),
        Some("DERIVED\\PRIMARY\\DYNAMIC\\NONE")
    );
    assert_eq!(
        enhanced_mr_temporal_file
            .pointer(
                "/recipe/recipe_parameters/per_frame_functional_groups/temporal_position_time_offset/1"
            )
            .and_then(Value::as_f64),
        Some(1.5)
    );
    assert!(
        validation_result_names(enhanced_mr_temporal_file.pointer("/validation/internal"))
            .contains(&"enhanced_mr_temporal_position_time_offset"),
        "Enhanced MR temporal manifest should record Temporal Position validation"
    );
    assert!(
        manifest
            .pointer("/skipped_cases")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "implemented extended cases should not be reported as skipped"
    );

    let enhanced_ct_path =
        out_dir.join("enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm");
    let enhanced_ct = open_file(&enhanced_ct_path).expect("Enhanced CT DICOM file should parse");
    assert_eq!(
        enhanced_ct
            .element(tags::SOP_CLASS_UID)
            .expect("Enhanced CT file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    assert_eq!(
        enhanced_ct
            .element(tags::NUMBER_OF_FRAMES)
            .expect("Enhanced CT file should contain Number of Frames")
            .value()
            .to_str()
            .expect("Number of Frames should be text")
            .trim(),
        "2"
    );
    assert_eq!(
        enhanced_ct
            .element(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)
            .expect("Enhanced CT file should contain Shared Functional Groups Sequence")
            .items()
            .expect("Shared Functional Groups should be a sequence")
            .len(),
        1
    );
    let per_frame_items = enhanced_ct
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .expect("Enhanced CT file should contain Per-Frame Functional Groups Sequence")
        .items()
        .expect("Per-Frame Functional Groups should be a sequence");
    assert_eq!(per_frame_items.len(), 2);
    let second_position_item = per_frame_items[1]
        .element(tags::PLANE_POSITION_SEQUENCE)
        .expect("second frame should contain Plane Position Sequence")
        .items()
        .expect("Plane Position should be a sequence");
    assert_eq!(
        second_position_item[0]
            .element(tags::IMAGE_POSITION_PATIENT)
            .expect("Plane Position should contain Image Position Patient")
            .value()
            .to_str()
            .expect("Image Position Patient should be text")
            .trim(),
        "0\\0\\2.5"
    );

    let enhanced_mr_path =
        out_dir.join("enhanced/mr/multiframe_echo_perframe_explicit_le/instance.dcm");
    let enhanced_mr = open_file(&enhanced_mr_path).expect("Enhanced MR DICOM file should parse");
    assert_eq!(
        enhanced_mr
            .element(tags::SOP_CLASS_UID)
            .expect("Enhanced MR file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_MR_IMAGE_STORAGE
    );
    let mr_per_frame_items = enhanced_mr
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .expect("Enhanced MR file should contain Per-Frame Functional Groups Sequence")
        .items()
        .expect("Per-Frame Functional Groups should be a sequence");
    assert_eq!(mr_per_frame_items.len(), 2);
    let second_echo_item = mr_per_frame_items[1]
        .element(tags::MR_ECHO_SEQUENCE)
        .expect("second frame should contain MR Echo Sequence")
        .items()
        .expect("MR Echo should be a sequence");
    assert_eq!(
        second_echo_item[0]
            .element(tags::EFFECTIVE_ECHO_TIME)
            .expect("MR Echo should contain Effective Echo Time")
            .value()
            .to_float64()
            .expect("Effective Echo Time should be FD"),
        24.5
    );

    let enhanced_mr_temporal_path =
        out_dir.join("enhanced/mr/multiframe_temporal_position_explicit_le/instance.dcm");
    let enhanced_mr_temporal = open_file(&enhanced_mr_temporal_path)
        .expect("Enhanced MR temporal DICOM file should parse");
    let temporal_per_frame_items = enhanced_mr_temporal
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .expect("Enhanced MR temporal file should contain Per-Frame Functional Groups Sequence")
        .items()
        .expect("Per-Frame Functional Groups should be a sequence");
    assert_eq!(temporal_per_frame_items.len(), 2);
    let second_temporal_frame_content = temporal_per_frame_items[1]
        .element(tags::FRAME_CONTENT_SEQUENCE)
        .expect("second temporal frame should contain Frame Content Sequence")
        .items()
        .expect("Frame Content should be a sequence");
    assert_eq!(
        second_temporal_frame_content[0]
            .element(tags::TEMPORAL_POSITION_INDEX)
            .expect("Frame Content should contain Temporal Position Index")
            .value()
            .to_int::<u32>()
            .expect("Temporal Position Index should be UL"),
        2
    );
    let second_temporal_position_item = temporal_per_frame_items[1]
        .element(tags::TEMPORAL_POSITION_SEQUENCE)
        .expect("second frame should contain Temporal Position Sequence")
        .items()
        .expect("Temporal Position should be a sequence");
    assert_eq!(
        second_temporal_position_item[0]
            .element(tags::TEMPORAL_POSITION_TIME_OFFSET)
            .expect("Temporal Position should contain Temporal Position Time Offset")
            .value()
            .to_float64()
            .expect("Temporal Position Time Offset should be FD"),
        1.5
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn generate_command_requires_output_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "smoke"])
        .output()
        .expect("generate command must run");

    assert!(
        !output.status.success(),
        "generate without --out should fail"
    );

    let stderr = String::from_utf8(output.stderr).expect("generate stderr must be utf-8");
    assert!(
        stderr.contains("generate requires --out"),
        "error should explain the missing output path"
    );
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

fn validation_results_named<'a>(manifest: &'a Value, pointer: &str) -> Vec<&'a str> {
    validation_result_names(manifest.pointer(pointer))
}

fn validation_result_names<'a>(value: Option<&'a Value>) -> Vec<&'a str> {
    value
        .and_then(Value::as_array)
        .expect("validation result array should exist")
        .iter()
        .map(|result| {
            assert_eq!(result.get("status").and_then(Value::as_str), Some("passed"));
            result
                .get("name")
                .and_then(Value::as_str)
                .expect("validation result should have a name")
        })
        .collect()
}

fn file_entry_by_case_id<'a>(manifest: &'a Value, case_id: &str) -> &'a Value {
    manifest
        .pointer("/files")
        .and_then(Value::as_array)
        .expect("manifest files should be an array")
        .iter()
        .find(|file| file.get("case_id").and_then(Value::as_str) == Some(case_id))
        .unwrap_or_else(|| panic!("manifest should contain {case_id}"))
}

fn file_entries_by_case_id<'a>(manifest: &'a Value, case_id: &str) -> Vec<&'a Value> {
    manifest
        .pointer("/files")
        .and_then(Value::as_array)
        .expect("manifest files should be an array")
        .iter()
        .filter(|file| file.get("case_id").and_then(Value::as_str) == Some(case_id))
        .collect()
}
