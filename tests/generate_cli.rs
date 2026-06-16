use std::fs;
use std::path::PathBuf;
use std::process::Command;

use dicom_core::Tag;
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::Value;

const SEGMENTATION_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.66.4";
const LABEL_MAP_SEGMENTATION_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.66.7";
const GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.11.1";
const REAL_WORLD_VALUE_MAPPING_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.67";
const BASIC_TEXT_SR_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.88.11";
const COMPREHENSIVE_SR_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.88.33";
const KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.88.59";
const RT_STRUCTURE_SET_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.3";
const RT_DOSE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.2";
const ENCAPSULATED_PDF_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.104.1";
const TAG_SEGMENTATION_TYPE: Tag = Tag(0x0062, 0x0001);
const TAG_SEGMENT_SEQUENCE: Tag = Tag(0x0062, 0x0002);
const TAG_MAXIMUM_FRACTIONAL_VALUE: Tag = Tag(0x0062, 0x000E);
const TAG_SEGMENTATION_FRACTIONAL_TYPE: Tag = Tag(0x0062, 0x0010);
const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
const TAG_REFERENCED_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x1140);
const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);
const TAG_REFERENCED_FRAME_NUMBER: Tag = Tag(0x0008, 0x1160);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: Tag = Tag(0x300C, 0x0060);
const TAG_DISPLAYED_AREA_TOP_LEFT_HAND_CORNER: Tag = Tag(0x0070, 0x0052);
const TAG_DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER: Tag = Tag(0x0070, 0x0053);
const TAG_DISPLAYED_AREA_SELECTION_SEQUENCE: Tag = Tag(0x0070, 0x005A);
const TAG_PRESENTATION_SIZE_MODE: Tag = Tag(0x0070, 0x0100);
const TAG_PRESENTATION_PIXEL_ASPECT_RATIO: Tag = Tag(0x0070, 0x0102);
const TAG_SOFTCOPY_VOI_LUT_SEQUENCE: Tag = Tag(0x0028, 0x3110);
const TAG_PRESENTATION_LUT_SHAPE: Tag = Tag(0x2050, 0x0020);

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
    assert_manifest_matches_committed_schema(&manifest);
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
    assert_manifest_matches_committed_schema(&manifest);
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
    let skipped_cases = manifest
        .pointer("/skipped_cases")
        .and_then(Value::as_array)
        .expect("manifest should contain skipped cases");
    assert_eq!(
        skipped_cases.len(),
        2,
        "core generation should report the two planned VL cases as unavailable"
    );
    let planned_vl_rgb = skipped_case_by_id(&manifest, "vl/photo/rgb_planar0_explicit_le");
    assert_eq!(
        planned_vl_rgb.get("status").and_then(Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        planned_vl_rgb.get("reason_code").and_then(Value::as_str),
        Some("case_planned")
    );
    assert_eq!(
        planned_vl_rgb.get("recheck_phase").and_then(Value::as_str),
        Some("phase-7")
    );
    assert!(
        !planned_vl_rgb
            .get("message")
            .and_then(Value::as_str)
            .expect("planned skipped row should have a message")
            .contains("Phase 1"),
        "planned cases should no longer use the old hard-coded Phase 1 skip text"
    );
    let planned_vl_palette = skipped_case_by_id(&manifest, "vl/photo/palette_color_explicit_le");
    assert_eq!(
        planned_vl_palette.get("status").and_then(Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        planned_vl_palette
            .get("reason_code")
            .and_then(Value::as_str),
        Some("case_planned")
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
    let expected_extended_files = 22
        + if cfg!(feature = "deflate") { 2 } else { 0 }
        + if cfg!(feature = "jpeg") { 1 } else { 0 }
        + if cfg!(feature = "charls") { 1 } else { 0 }
        + if cfg!(feature = "jpegxl") { 1 } else { 0 }
        + if cfg!(feature = "jpeg2000") { 1 } else { 0 }
        + if cfg!(feature = "htj2k_openjph") {
            1
        } else {
            0
        }
        + if cfg!(feature = "legacy_jpeg_dcmtk") {
            2
        } else {
            0
        };
    assert!(stdout.contains(&format!("files_written\t{expected_extended_files}")));

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    assert_manifest_matches_committed_schema(&manifest);
    assert_eq!(
        manifest
            .pointer("/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(expected_extended_files)
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
    let rle_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_rle_lossless");
    assert_eq!(
        rle_file
            .pointer("/dicom/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.5")
    );
    assert_eq!(
        rle_file
            .pointer("/pixel_data/native_or_encapsulated")
            .and_then(Value::as_str),
        Some("encapsulated")
    );
    assert!(
        rle_file
            .pointer("/pixel_data/value_length")
            .is_some_and(Value::is_null),
        "encapsulated Pixel Data should record an undefined value length"
    );
    assert_eq!(
        rle_file
            .pointer("/pixel_data/codec/backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rle_file
            .pointer("/pixel_data/encapsulated_pixel_data/basic_offset_table/present")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        rle_file
            .pointer("/pixel_data/encapsulated_pixel_data/basic_offset_table/populated")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        rle_file
            .pointer("/pixel_data/encapsulated_pixel_data/basic_offset_table/offset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rle_file
            .pointer("/pixel_data/encapsulated_pixel_data/fragments_per_frame/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        validation_result_names(rle_file.pointer("/validation/internal"))
            .contains(&"encapsulated_fragment_count"),
        "RLE manifest should record encapsulated fragment validation"
    );
    assert!(
        validation_result_names(rle_file.pointer("/validation/internal"))
            .contains(&"rle_decoded_frame_hashes"),
        "RLE manifest should record decoded native frame hash validation"
    );
    let rle_u16_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_rle_lossless");
    assert_eq!(
        rle_u16_file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        rle_u16_file
            .pointer("/pixel_data/native_or_encapsulated")
            .and_then(Value::as_str),
        Some("encapsulated")
    );
    assert_eq!(
        rle_u16_file
            .pointer("/pixel_data/codec/backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rle_u16_file
            .pointer("/pixel_data/encapsulated_pixel_data/fragments_per_frame/0")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        validation_result_names(rle_u16_file.pointer("/validation/internal"))
            .contains(&"rle_decoded_frame_hashes"),
        "16-bit RLE manifest should record decoded native frame hash validation"
    );
    let rle_rgb_file = file_entry_by_case_id(&manifest, "classic/sc/rgb_planar0_rle_lossless");
    assert_eq!(
        rle_rgb_file
            .pointer("/image/photometric_interpretation")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        rle_rgb_file
            .pointer("/image/samples_per_pixel")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        rle_rgb_file
            .pointer("/image/planar_configuration")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        rle_rgb_file
            .pointer("/pixel_data/native_or_encapsulated")
            .and_then(Value::as_str),
        Some("encapsulated")
    );
    assert_eq!(
        rle_rgb_file
            .pointer("/pixel_data/codec/backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        validation_result_names(rle_rgb_file.pointer("/validation/internal"))
            .contains(&"rle_decoded_frame_hashes"),
        "RGB RLE manifest should record decoded native frame hash validation"
    );
    let rle_multiframe_file =
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_multiframe_rle_lossless");
    assert_eq!(
        rle_multiframe_file
            .pointer("/image/frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rle_multiframe_file
            .pointer("/pixel_data/frame_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rle_multiframe_file
            .pointer("/pixel_data/frame_hashes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        rle_multiframe_file
            .pointer("/pixel_data/encapsulated_pixel_data/basic_offset_table/offset_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rle_multiframe_file
            .pointer("/pixel_data/encapsulated_pixel_data/fragments_per_frame")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(
        validation_result_names(rle_multiframe_file.pointer("/validation/internal"))
            .contains(&"number_of_frames"),
        "multi-frame RLE manifest should validate Number of Frames"
    );
    assert!(
        validation_result_names(rle_multiframe_file.pointer("/validation/internal"))
            .contains(&"rle_decoded_frame_hashes"),
        "multi-frame RLE manifest should record decoded native frame hash validation"
    );
    let rle_odd_file =
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_odd_fragment_rle_lossless");
    assert_eq!(
        rle_odd_file.pointer("/image/rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rle_odd_file
            .pointer("/image/columns")
            .and_then(Value::as_u64),
        Some(2)
    );
    let odd_fragment = rle_odd_file
        .pointer("/pixel_data/encapsulated_pixel_data/fragments/0")
        .expect("odd RLE case should record first fragment metadata");
    let compressed_length = odd_fragment
        .get("compressed_length")
        .and_then(Value::as_u64)
        .expect("odd RLE fragment should record compressed length");
    let padded_length = odd_fragment
        .get("padded_length")
        .and_then(Value::as_u64)
        .expect("odd RLE fragment should record padded length");
    assert_eq!(
        compressed_length % 2,
        1,
        "RLE fragment should exercise odd compressed length"
    );
    assert_eq!(
        padded_length,
        compressed_length + 1,
        "encapsulated item padding should round the odd RLE fragment length up by one byte"
    );
    assert!(
        rle_odd_file
            .pointer("/known_stressors")
            .and_then(Value::as_array)
            .is_some_and(|stressors| stressors
                .iter()
                .any(|stress| stress.as_str() == Some("encapsulated_item_padding"))),
        "odd RLE case should label encapsulated item padding as a stressor"
    );
    if cfg!(feature = "jpeg") {
        let jpeg_file =
            file_entry_by_case_id(&manifest, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
        assert_eq!(
            jpeg_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.50")
        );
        assert_eq!(
            jpeg_file.pointer("/determinism").and_then(Value::as_str),
            Some("semantic_stable")
        );
        assert_eq!(
            jpeg_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            jpeg_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("dicom_rs_jpeg_baseline_writer")
        );
        assert_eq!(
            jpeg_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("jpeg")
        );
        assert_eq!(
            jpeg_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("01")
        );
        assert!(
            validation_result_names(jpeg_file.pointer("/validation/internal"))
                .contains(&"jpeg_baseline_decoded_frame_tolerance"),
            "JPEG manifest should record decoded sample tolerance validation"
        );
    }
    if cfg!(feature = "charls") {
        let jpeg_ls_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_jpeg_ls_lossless");
        assert_eq!(
            jpeg_ls_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.80")
        );
        assert_eq!(
            jpeg_ls_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            jpeg_ls_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("dicom_rs_charls_jpeg_ls_lossless_writer")
        );
        assert_eq!(
            jpeg_ls_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("charls")
        );
        assert_eq!(
            jpeg_ls_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("00")
        );
        assert!(
            validation_result_names(jpeg_ls_file.pointer("/validation/internal"))
                .contains(&"jpeg_ls_lossless_decoded_frame_hashes"),
            "JPEG-LS manifest should record exact decoded frame hash validation"
        );
    }
    if cfg!(feature = "jpegxl") {
        let jpeg_xl_file =
            file_entry_by_case_id(&manifest, "classic/sc/rgb_planar0_jpegxl_lossless");
        assert_eq!(
            jpeg_xl_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.110")
        );
        assert_eq!(
            jpeg_xl_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            jpeg_xl_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("dicom_rs_jpegxl_lossless_writer")
        );
        assert_eq!(
            jpeg_xl_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("jpegxl")
        );
        assert_eq!(
            jpeg_xl_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("00")
        );
        assert!(
            validation_result_names(jpeg_xl_file.pointer("/validation/internal"))
                .contains(&"jpeg_xl_lossless_decoded_frame_hashes"),
            "JPEG XL manifest should record exact decoded frame hash validation"
        );
    }
    if cfg!(feature = "jpeg2000") {
        let jpeg_2000_file =
            file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_jpeg2000_lossless");
        assert_eq!(
            jpeg_2000_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.90")
        );
        assert_eq!(
            jpeg_2000_file
                .pointer("/image/bits_allocated")
                .and_then(Value::as_u64),
            Some(16)
        );
        assert_eq!(
            jpeg_2000_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            jpeg_2000_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("project_openjp2_jpeg2000_lossless_writer")
        );
        assert_eq!(
            jpeg_2000_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("jpeg2000")
        );
        assert_eq!(
            jpeg_2000_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("00")
        );
        assert!(
            validation_result_names(jpeg_2000_file.pointer("/validation/internal"))
                .contains(&"jpeg_2000_lossless_decoded_frame_hashes"),
            "JPEG 2000 manifest should record exact decoded frame hash validation"
        );
    }
    if cfg!(feature = "htj2k_openjph") {
        let htj2k_file = file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_htj2k_lossless");
        assert_eq!(
            htj2k_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.201")
        );
        assert_eq!(
            htj2k_file
                .pointer("/image/bits_allocated")
                .and_then(Value::as_u64),
            Some(16)
        );
        assert_eq!(
            htj2k_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            htj2k_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("openjph_htj2k_lossless_command_writer")
        );
        assert_eq!(
            htj2k_file
                .pointer("/pixel_data/codec/backend_kind")
                .and_then(Value::as_str),
            Some("external_command")
        );
        assert_eq!(
            htj2k_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("htj2k_openjph")
        );
        assert_eq!(
            htj2k_file
                .pointer("/pixel_data/codec/runtime_identity/command")
                .and_then(Value::as_str),
            Some("ojph_compress")
        );
        assert!(
            htj2k_file
                .pointer("/pixel_data/codec/runtime_identity/executable_sha256")
                .and_then(Value::as_str)
                .is_some_and(|hash| hash.len() == 64),
            "HTJ2K manifest should record the OpenJPH executable SHA-256 fingerprint"
        );
        assert_eq!(
            htj2k_file
                .pointer("/pixel_data/codec/runtime_identity/encoder_options/num_decomps")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            htj2k_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("00")
        );
        assert!(
            validation_result_names(htj2k_file.pointer("/validation/internal"))
                .contains(&"htj2k_lossless_decoded_frame_hashes"),
            "HTJ2K manifest should record exact decoded frame hash validation"
        );
    }
    if cfg!(feature = "legacy_jpeg_dcmtk") {
        let legacy_jpeg_process_14_file =
            file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_process_14");
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.57")
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/image/bits_allocated")
                .and_then(Value::as_u64),
            Some(16)
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer")
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/codec/backend_kind")
                .and_then(Value::as_str),
            Some("external_command")
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("legacy_jpeg_dcmtk")
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/codec/runtime_identity/command")
                .and_then(Value::as_str),
            Some("dcmcjpeg")
        );
        assert!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/codec/runtime_identity/executable_sha256")
                .and_then(Value::as_str)
                .is_some_and(|hash| hash.len() == 64),
            "legacy JPEG Process 14 manifest should record the dcmcjpeg executable SHA-256 fingerprint"
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/pixel_data/codec/runtime_identity/encoder_options/mode")
                .and_then(Value::as_str),
            Some("lossless_process_14")
        );
        assert_eq!(
            legacy_jpeg_process_14_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("00")
        );
        assert!(
            validation_result_names(legacy_jpeg_process_14_file.pointer("/validation/internal"))
                .contains(&"jpeg_lossless_process_14_decoded_frame_hashes"),
            "legacy JPEG Process 14 manifest should record exact decoded frame hash validation"
        );

        let legacy_jpeg_file =
            file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_sv1");
        assert_eq!(
            legacy_jpeg_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.4.70")
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/image/bits_allocated")
                .and_then(Value::as_u64),
            Some(16)
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer")
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/pixel_data/codec/backend_kind")
                .and_then(Value::as_str),
            Some("external_command")
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("legacy_jpeg_dcmtk")
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/pixel_data/codec/runtime_identity/command")
                .and_then(Value::as_str),
            Some("dcmcjpeg")
        );
        assert!(
            legacy_jpeg_file
                .pointer("/pixel_data/codec/runtime_identity/executable_sha256")
                .and_then(Value::as_str)
                .is_some_and(|hash| hash.len() == 64),
            "legacy JPEG manifest should record the dcmcjpeg executable SHA-256 fingerprint"
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/pixel_data/codec/runtime_identity/encoder_options/mode")
                .and_then(Value::as_str),
            Some("lossless_sv1")
        );
        assert_eq!(
            legacy_jpeg_file
                .pointer("/expected_semantics/lossy_image_compression")
                .and_then(Value::as_str),
            Some("00")
        );
        assert!(
            validation_result_names(legacy_jpeg_file.pointer("/validation/internal"))
                .contains(&"jpeg_lossless_sv1_decoded_frame_hashes"),
            "legacy JPEG manifest should record exact decoded frame hash validation"
        );
    }
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
    let enhanced_ct_concat_files =
        file_entries_by_case_id(&manifest, "enhanced/ct/concatenation_two_part_explicit_le");
    assert_eq!(enhanced_ct_concat_files.len(), 2);
    assert_eq!(
        enhanced_ct_concat_files[0]
            .pointer("/path")
            .and_then(Value::as_str),
        Some("enhanced/ct/concatenation_two_part_explicit_le/part-001.dcm")
    );
    assert_eq!(
        enhanced_ct_concat_files[1]
            .pointer("/path")
            .and_then(Value::as_str),
        Some("enhanced/ct/concatenation_two_part_explicit_le/part-002.dcm")
    );
    assert_eq!(
        enhanced_ct_concat_files[0]
            .pointer("/image/frames")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        enhanced_ct_concat_files[1]
            .pointer("/expected_semantics/dimension_index_values/0")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        enhanced_ct_concat_files[0]
            .pointer("/expected_semantics/concatenation/in_concatenation_number")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        enhanced_ct_concat_files[1]
            .pointer("/expected_semantics/concatenation/concatenation_frame_offset_number")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        enhanced_ct_concat_files[0]
            .pointer("/expected_semantics/concatenation/concatenation_uid")
            .and_then(Value::as_str),
        enhanced_ct_concat_files[1]
            .pointer("/expected_semantics/concatenation/concatenation_uid")
            .and_then(Value::as_str)
    );
    assert_eq!(
        enhanced_ct_concat_files[0]
            .pointer("/expected_semantics/concatenation/sop_instance_uid_of_concatenation_source")
            .and_then(Value::as_str),
        enhanced_ct_concat_files[1]
            .pointer("/expected_semantics/concatenation/sop_instance_uid_of_concatenation_source")
            .and_then(Value::as_str)
    );
    assert_ne!(
        enhanced_ct_concat_files[0]
            .pointer("/uids/sop_instance_uid")
            .and_then(Value::as_str),
        enhanced_ct_concat_files[1]
            .pointer("/uids/sop_instance_uid")
            .and_then(Value::as_str)
    );
    assert!(
        validation_result_names(enhanced_ct_concat_files[1].pointer("/validation/internal"))
            .contains(&"enhanced_ct_concatenation_frame_offset_number"),
        "Enhanced CT concatenation manifest should record Concatenation Frame Offset validation"
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
    let enhanced_mr_phase_file = file_entry_by_case_id(
        &manifest,
        "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
    );
    assert_eq!(
        enhanced_mr_phase_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(uids::ENHANCED_MR_IMAGE_STORAGE)
    );
    assert_eq!(
        enhanced_mr_phase_file
            .pointer("/recipe/recipe_parameters/dimension_index/dimension_index_pointer")
            .and_then(Value::as_str),
        Some("VelocityEncodingDirection")
    );
    assert_eq!(
        enhanced_mr_phase_file
            .pointer(
                "/recipe/recipe_parameters/per_frame_functional_groups/velocity_encoding_direction/1/1"
            )
            .and_then(Value::as_f64),
        Some(1.0)
    );
    assert!(
        validation_result_names(enhanced_mr_phase_file.pointer("/validation/internal"))
            .contains(&"enhanced_mr_velocity_encoding_direction"),
        "Enhanced MR phase manifest should record velocity encoding validation"
    );
    assert!(
        manifest
            .pointer("/skipped_cases")
            .and_then(Value::as_array)
            .is_some_and(|cases| {
                cases.iter().all(|case| {
                    !matches!(
                        case.get("case_id").and_then(Value::as_str),
                        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
                            | Some("enhanced/ct/concatenation_two_part_explicit_le")
                            | Some("enhanced/mr/multiframe_echo_perframe_explicit_le")
                            | Some("enhanced/mr/multiframe_temporal_position_explicit_le")
                            | Some("enhanced/mr/multiframe_phase_velocity_encoding_explicit_le")
                    )
                })
            }),
        "implemented extended cases should not be reported as skipped"
    );
    let segmentation_file =
        file_entry_by_case_id(&manifest, "derived/seg/binary_multiframe_explicit_le");
    assert_eq!(
        segmentation_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(SEGMENTATION_STORAGE_UID)
    );
    assert_eq!(
        segmentation_file
            .pointer("/recipe/recipe_parameters/segmentation_type")
            .and_then(Value::as_str),
        Some("BINARY")
    );
    assert_eq!(
        segmentation_file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        segmentation_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        segmentation_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        segmentation_file
            .pointer("/references/0/frame_numbers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(
        validation_result_names(segmentation_file.pointer("/validation/internal"))
            .contains(&"segmentation_type"),
        "SEG manifest should record Segmentation Type validation"
    );
    let fractional_segmentation_file = file_entry_by_case_id(
        &manifest,
        "derived/seg/fractional_probability_multiframe_explicit_le",
    );
    assert_eq!(
        fractional_segmentation_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(SEGMENTATION_STORAGE_UID)
    );
    assert_eq!(
        fractional_segmentation_file
            .pointer("/recipe/recipe_parameters/segmentation_type")
            .and_then(Value::as_str),
        Some("FRACTIONAL")
    );
    assert_eq!(
        fractional_segmentation_file
            .pointer("/recipe/recipe_parameters/segmentation_fractional_type")
            .and_then(Value::as_str),
        Some("PROBABILITY")
    );
    assert_eq!(
        fractional_segmentation_file
            .pointer("/recipe/recipe_parameters/maximum_fractional_value")
            .and_then(Value::as_u64),
        Some(255)
    );
    assert_eq!(
        fractional_segmentation_file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        fractional_segmentation_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert!(
        validation_result_names(fractional_segmentation_file.pointer("/validation/internal"))
            .contains(&"segmentation_fractional_type"),
        "fractional SEG manifest should record fractional type validation"
    );
    let labelmap_segmentation_file =
        file_entry_by_case_id(&manifest, "derived/seg/labelmap_multiframe_explicit_le");
    assert_eq!(
        labelmap_segmentation_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(LABEL_MAP_SEGMENTATION_STORAGE_UID)
    );
    assert_eq!(
        labelmap_segmentation_file
            .pointer("/dicom/sop_class_name")
            .and_then(Value::as_str),
        Some("Label Map Segmentation Storage")
    );
    assert_eq!(
        labelmap_segmentation_file
            .pointer("/recipe/recipe_parameters/segmentation_type")
            .and_then(Value::as_str),
        Some("LABELMAP")
    );
    assert_eq!(
        labelmap_segmentation_file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        labelmap_segmentation_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert!(
        validation_result_names(labelmap_segmentation_file.pointer("/validation/internal"))
            .contains(&"segmentation_type"),
        "LABELMAP SEG manifest should record Segmentation Type validation"
    );
    let presentation_state_file = file_entry_by_case_id(
        &manifest,
        "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
    );
    assert_eq!(
        presentation_state_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID)
    );
    assert_eq!(
        presentation_state_file
            .pointer("/dicom/modality")
            .and_then(Value::as_str),
        Some("PR")
    );
    assert!(
        presentation_state_file
            .pointer("/image")
            .is_some_and(Value::is_null),
        "GSPS manifest should explicitly omit image metadata"
    );
    assert!(
        presentation_state_file
            .pointer("/pixel_data")
            .is_some_and(Value::is_null),
        "GSPS manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        presentation_state_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        presentation_state_file
            .pointer("/recipe/recipe_parameters/window_center")
            .and_then(Value::as_str),
        Some("350")
    );
    assert_eq!(
        presentation_state_file
            .pointer("/recipe/recipe_parameters/window_width")
            .and_then(Value::as_str),
        Some("1400")
    );
    assert!(
        validation_result_names(presentation_state_file.pointer("/validation/internal"))
            .contains(&"presentation_state_referenced_sop_instance_uid"),
        "GSPS manifest should record source-reference validation"
    );
    assert!(
        validation_result_names(presentation_state_file.pointer("/validation/internal"))
            .contains(&"presentation_state_lut_shape"),
        "GSPS manifest should record Presentation LUT Shape validation"
    );
    let rwvm_file = file_entry_by_case_id(&manifest, "derived/rwvm/linear_ct_mapping_explicit_le");
    assert_eq!(
        rwvm_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(REAL_WORLD_VALUE_MAPPING_STORAGE_UID)
    );
    assert_eq!(
        rwvm_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("RWV")
    );
    assert!(
        rwvm_file.pointer("/image").is_some_and(Value::is_null),
        "RWVM manifest should explicitly omit image metadata"
    );
    assert!(
        rwvm_file.pointer("/pixel_data").is_some_and(Value::is_null),
        "RWVM manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        rwvm_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        rwvm_file
            .pointer("/recipe/recipe_parameters/lut_label")
            .and_then(Value::as_str),
        Some("DTS_HU")
    );
    assert_eq!(
        rwvm_file
            .pointer("/recipe/recipe_parameters/intercept")
            .and_then(Value::as_f64),
        Some(-1024.0)
    );
    assert_eq!(
        rwvm_file
            .pointer("/recipe/recipe_parameters/slope")
            .and_then(Value::as_f64),
        Some(1.0)
    );
    assert!(
        validation_result_names(rwvm_file.pointer("/validation/internal"))
            .contains(&"rwvm_referenced_sop_instance_uid"),
        "RWVM manifest should record source-reference validation"
    );
    assert!(
        validation_result_names(rwvm_file.pointer("/validation/internal")).contains(&"rwvm_slope"),
        "RWVM manifest should record linear mapping validation"
    );
    let basic_text_sr_file =
        file_entry_by_case_id(&manifest, "derived/sr/basic_text_observation_explicit_le");
    assert_eq!(
        basic_text_sr_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(BASIC_TEXT_SR_STORAGE_UID)
    );
    assert_eq!(
        basic_text_sr_file
            .pointer("/dicom/modality")
            .and_then(Value::as_str),
        Some("SR")
    );
    assert!(
        basic_text_sr_file
            .pointer("/image")
            .is_some_and(Value::is_null),
        "Basic Text SR manifest should explicitly omit image metadata"
    );
    assert!(
        basic_text_sr_file
            .pointer("/pixel_data")
            .is_some_and(Value::is_null),
        "Basic Text SR manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        basic_text_sr_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        basic_text_sr_file
            .pointer("/recipe/recipe_parameters/completion_flag")
            .and_then(Value::as_str),
        Some("COMPLETE")
    );
    assert_eq!(
        basic_text_sr_file
            .pointer("/recipe/recipe_parameters/verification_flag")
            .and_then(Value::as_str),
        Some("UNVERIFIED")
    );
    assert_eq!(
        basic_text_sr_file
            .pointer("/recipe/recipe_parameters/observation/text")
            .and_then(Value::as_str),
        Some("Synthetic Basic Text SR observation for Enhanced CT source images.")
    );
    assert!(
        validation_result_names(basic_text_sr_file.pointer("/validation/internal"))
            .contains(&"sr_evidence_sop_instance_uid"),
        "Basic Text SR manifest should record evidence-reference validation"
    );
    assert!(
        validation_result_names(basic_text_sr_file.pointer("/validation/internal"))
            .contains(&"sr_observation_text"),
        "Basic Text SR manifest should record text content validation"
    );
    let comprehensive_sr_file = file_entry_by_case_id(
        &manifest,
        "derived/sr/comprehensive_measurement_explicit_le",
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(COMPREHENSIVE_SR_STORAGE_UID)
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/dicom/modality")
            .and_then(Value::as_str),
        Some("SR")
    );
    assert!(
        comprehensive_sr_file
            .pointer("/image")
            .is_some_and(Value::is_null),
        "Comprehensive SR manifest should explicitly omit image metadata"
    );
    assert!(
        comprehensive_sr_file
            .pointer("/pixel_data")
            .is_some_and(Value::is_null),
        "Comprehensive SR manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/recipe/recipe_parameters/measurement/value_type")
            .and_then(Value::as_str),
        Some("NUM")
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/recipe/recipe_parameters/measurement/numeric_value")
            .and_then(Value::as_str),
        Some("12.5")
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/recipe/recipe_parameters/measurement/units/code_value")
            .and_then(Value::as_str),
        Some("mm")
    );
    assert_eq!(
        comprehensive_sr_file
            .pointer("/recipe/recipe_parameters/image_reference/value_type")
            .and_then(Value::as_str),
        Some("IMAGE")
    );
    assert!(
        validation_result_names(comprehensive_sr_file.pointer("/validation/internal"))
            .contains(&"sr_measurement_numeric_value"),
        "Comprehensive SR manifest should record numeric measurement validation"
    );
    assert!(
        validation_result_names(comprehensive_sr_file.pointer("/validation/internal"))
            .contains(&"sr_image_sop_instance_uid"),
        "Comprehensive SR manifest should record image reference validation"
    );
    let kos_file = file_entry_by_case_id(&manifest, "derived/sr/key_object_selection_explicit_le");
    assert_eq!(
        kos_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID)
    );
    assert_eq!(
        kos_file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("KO")
    );
    assert!(
        kos_file.pointer("/image").is_some_and(Value::is_null),
        "KOS manifest should explicitly omit image metadata"
    );
    assert!(
        kos_file.pointer("/pixel_data").is_some_and(Value::is_null),
        "KOS manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        kos_file
            .pointer("/references")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "KOS manifest should reference the source CT and binary SEG objects"
    );
    assert_eq!(
        kos_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        kos_file
            .pointer("/references/1/source_case_id")
            .and_then(Value::as_str),
        Some("derived/seg/binary_multiframe_explicit_le")
    );
    assert_eq!(
        kos_file
            .pointer("/recipe/recipe_parameters/key_object_items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(
        validation_result_names(kos_file.pointer("/validation/internal"))
            .contains(&"kos_image_sop_instance_uid"),
        "KOS manifest should record key object reference validation"
    );
    let rt_structure_set_file = file_entry_by_case_id(
        &manifest,
        "non-image/rt/structure_set_single_roi_explicit_le",
    );
    assert_eq!(
        rt_structure_set_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(RT_STRUCTURE_SET_STORAGE_UID)
    );
    assert_eq!(
        rt_structure_set_file
            .pointer("/dicom/modality")
            .and_then(Value::as_str),
        Some("RTSTRUCT")
    );
    assert!(
        rt_structure_set_file
            .pointer("/image")
            .is_some_and(Value::is_null),
        "RT Structure Set manifest should explicitly omit image metadata"
    );
    assert!(
        rt_structure_set_file
            .pointer("/pixel_data")
            .is_some_and(Value::is_null),
        "RT Structure Set manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        rt_structure_set_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        rt_structure_set_file
            .pointer("/expected_semantics/rt_structure_set/contour_geometric_type")
            .and_then(Value::as_str),
        Some("CLOSED_PLANAR")
    );
    assert!(
        validation_result_names(rt_structure_set_file.pointer("/validation/internal"))
            .contains(&"rt_contour_image_sop_instance_uid"),
        "RT Structure Set manifest should record contour image reference validation"
    );
    let rt_dose_file = file_entry_by_case_id(&manifest, "non-image/rt/dose_grid_u16_explicit_le");
    assert_eq!(
        rt_dose_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(RT_DOSE_STORAGE_UID)
    );
    assert_eq!(
        rt_dose_file
            .pointer("/dicom/modality")
            .and_then(Value::as_str),
        Some("RTDOSE")
    );
    assert_eq!(
        rt_dose_file
            .pointer("/image/frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rt_dose_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        rt_dose_file
            .pointer("/references")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "RT Dose manifest should reference the source CT and RT Structure Set"
    );
    assert_eq!(
        rt_dose_file
            .pointer("/references/0/source_case_id")
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        rt_dose_file
            .pointer("/references/1/source_case_id")
            .and_then(Value::as_str),
        Some("non-image/rt/structure_set_single_roi_explicit_le")
    );
    assert_eq!(
        rt_dose_file
            .pointer("/expected_semantics/rt_dose/dose_grid_scaling")
            .and_then(Value::as_str),
        Some("0.001")
    );
    assert!(
        validation_result_names(rt_dose_file.pointer("/validation/internal"))
            .contains(&"rt_dose_referenced_structure_set_sop_instance_uid"),
        "RT Dose manifest should record structure set reference validation"
    );
    let encapsulated_pdf_file = file_entry_by_case_id(
        &manifest,
        "non-image/encapsulated-document/pdf_minimal_explicit_le",
    );
    assert_eq!(
        encapsulated_pdf_file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str),
        Some(ENCAPSULATED_PDF_STORAGE_UID)
    );
    assert_eq!(
        encapsulated_pdf_file
            .pointer("/dicom/modality")
            .and_then(Value::as_str),
        Some("DOC")
    );
    assert!(
        encapsulated_pdf_file
            .pointer("/image")
            .is_some_and(Value::is_null),
        "Encapsulated PDF manifest should explicitly omit image metadata"
    );
    assert!(
        encapsulated_pdf_file
            .pointer("/pixel_data")
            .is_some_and(Value::is_null),
        "Encapsulated PDF manifest should explicitly omit Pixel Data metadata"
    );
    assert_eq!(
        encapsulated_pdf_file
            .pointer("/references")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "minimal Encapsulated PDF manifest should not reference source objects"
    );
    assert_eq!(
        encapsulated_pdf_file
            .pointer("/expected_semantics/encapsulated_document/mime_type")
            .and_then(Value::as_str),
        Some("application/pdf")
    );
    assert_eq!(
        encapsulated_pdf_file
            .pointer("/expected_semantics/encapsulated_document/document_title")
            .and_then(Value::as_str),
        Some("DTS Minimal Synthetic PDF")
    );
    assert_eq!(
        encapsulated_pdf_file
            .pointer("/expected_semantics/encapsulated_document/burned_in_annotation")
            .and_then(Value::as_str),
        Some("NO")
    );
    assert!(
        validation_result_names(encapsulated_pdf_file.pointer("/validation/internal"))
            .contains(&"encapsulated_pdf_document_payload"),
        "Encapsulated PDF manifest should record document payload validation"
    );
    let skipped_cases = manifest
        .pointer("/skipped_cases")
        .and_then(Value::as_array)
        .expect("manifest should contain skipped cases");
    assert_eq!(
        skipped_cases.len(),
        9 - if cfg!(feature = "deflate") { 2 } else { 0 }
            - if cfg!(feature = "jpeg") { 1 } else { 0 }
            - if cfg!(feature = "charls") { 1 } else { 0 }
            - if cfg!(feature = "jpegxl") { 1 } else { 0 }
            - if cfg!(feature = "jpeg2000") { 1 } else { 0 }
            - if cfg!(feature = "htj2k_openjph") {
                1
            } else {
                0
            }
            - if cfg!(feature = "legacy_jpeg_dcmtk") {
                2
            } else {
                0
            },
        "extended generation should report only unavailable compressed transfer syntax rows plus the no-feature deflated row"
    );
    if !cfg!(feature = "htj2k_openjph") {
        let htj2k = skipped_case_by_id(&manifest, "classic/sc/mono2_u16_htj2k_lossless");
        assert_eq!(
            htj2k.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            htj2k.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            htj2k
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated HTJ2K row should have a message")
                .contains("Cargo feature(s) htj2k_openjph"),
            "feature-gated HTJ2K unavailable row should name the required feature"
        );
    }
    if !cfg!(feature = "jpeg") {
        let jpeg = skipped_case_by_id(&manifest, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
        assert_eq!(
            jpeg.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            jpeg.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            jpeg.get("message")
                .and_then(Value::as_str)
                .expect("feature-gated JPEG row should have a message")
                .contains("Cargo feature(s) jpeg"),
            "feature-gated JPEG unavailable row should name the required feature"
        );
    }
    if !cfg!(feature = "charls") {
        let jpeg_ls = skipped_case_by_id(&manifest, "classic/sc/mono2_u8_jpeg_ls_lossless");
        assert_eq!(
            jpeg_ls.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            jpeg_ls.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            jpeg_ls
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated JPEG-LS row should have a message")
                .contains("Cargo feature(s) charls"),
            "feature-gated JPEG-LS unavailable row should name the required feature"
        );
    }
    if !cfg!(feature = "jpegxl") {
        let jpeg_xl = skipped_case_by_id(&manifest, "classic/sc/rgb_planar0_jpegxl_lossless");
        assert_eq!(
            jpeg_xl.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            jpeg_xl.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            jpeg_xl
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated JPEG XL row should have a message")
                .contains("Cargo feature(s) jpegxl"),
            "feature-gated JPEG XL unavailable row should name the required feature"
        );
    }
    if !cfg!(feature = "jpeg2000") {
        let jpeg_2000 = skipped_case_by_id(&manifest, "classic/sc/mono2_u16_jpeg2000_lossless");
        assert_eq!(
            jpeg_2000.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            jpeg_2000.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            jpeg_2000
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated JPEG 2000 row should have a message")
                .contains("Cargo feature(s) jpeg2000"),
            "feature-gated JPEG 2000 unavailable row should name the required feature"
        );
    }
    if !cfg!(feature = "legacy_jpeg_dcmtk") {
        let legacy_jpeg_process_14 =
            skipped_case_by_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_process_14");
        assert_eq!(
            legacy_jpeg_process_14.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            legacy_jpeg_process_14
                .get("reason_code")
                .and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            legacy_jpeg_process_14
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated legacy JPEG Process 14 row should have a message")
                .contains("Cargo feature(s) legacy_jpeg_dcmtk"),
            "feature-gated legacy JPEG Process 14 unavailable row should name the required feature"
        );

        let legacy_jpeg = skipped_case_by_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_sv1");
        assert_eq!(
            legacy_jpeg.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            legacy_jpeg.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            legacy_jpeg
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated legacy JPEG row should have a message")
                .contains("Cargo feature(s) legacy_jpeg_dcmtk"),
            "feature-gated legacy JPEG unavailable row should name the required feature"
        );
    }
    if cfg!(feature = "deflate") {
        let deflated_file =
            file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_deflated_explicit_le");
        assert_eq!(
            deflated_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.1.99")
        );
        assert!(
            validation_result_names(deflated_file.pointer("/validation/standards"))
                .contains(&"deflated_explicit_vr_little_endian_transfer_syntax"),
            "deflated manifest should record the named transfer syntax validation"
        );
        let deflated_seg_file = file_entry_by_case_id(
            &manifest,
            "derived/seg/binary_multiframe_deflated_image_frame",
        );
        assert_eq!(
            deflated_seg_file
                .pointer("/dicom/transfer_syntax_uid")
                .and_then(Value::as_str),
            Some("1.2.840.10008.1.2.8.1")
        );
        assert_eq!(
            deflated_seg_file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str),
            Some("encapsulated")
        );
        assert_eq!(
            deflated_seg_file
                .pointer("/pixel_data/frame_count")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            deflated_seg_file
                .pointer("/pixel_data/codec/backend_id")
                .and_then(Value::as_str),
            Some("dicom_rs_deflated_image_frame_writer")
        );
        assert_eq!(
            deflated_seg_file
                .pointer("/pixel_data/codec/feature_gate")
                .and_then(Value::as_str),
            Some("deflate")
        );
        assert_eq!(
            deflated_seg_file
                .pointer("/pixel_data/encapsulated_pixel_data/fragments_per_frame")
                .and_then(Value::as_array)
                .map(|counts| counts.iter().filter_map(Value::as_u64).collect::<Vec<_>>()),
            Some(vec![1, 1])
        );
        assert!(
            validation_result_names(deflated_seg_file.pointer("/validation/internal"))
                .contains(&"deflated_image_frame_decoded_frame_hashes"),
            "Deflated Image Frame SEG manifest should record exact decoded frame hash validation"
        );
    } else {
        let deflated = skipped_case_by_id(&manifest, "classic/sc/mono2_u8_deflated_explicit_le");
        assert_eq!(
            deflated.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            deflated.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert_eq!(
            deflated.get("recheck_phase").and_then(Value::as_str),
            Some("phase-6")
        );
        assert!(
            deflated
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated deflated row should have a message")
                .contains("Cargo feature(s) deflate"),
            "feature-gated deflated unavailable row should name the required feature"
        );
        let deflated_seg = skipped_case_by_id(
            &manifest,
            "derived/seg/binary_multiframe_deflated_image_frame",
        );
        assert_eq!(
            deflated_seg.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            deflated_seg.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        assert!(
            deflated_seg
                .get("message")
                .and_then(Value::as_str)
                .expect("feature-gated Deflated Image Frame row should have a message")
                .contains("Cargo feature(s) deflate"),
            "feature-gated Deflated Image Frame unavailable row should name the required feature"
        );
    }

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

    let segmentation_path = out_dir.join("derived/seg/binary_multiframe_explicit_le/instance.dcm");
    let segmentation = open_file(&segmentation_path).expect("SEG DICOM file should parse");
    assert_eq!(
        segmentation
            .element(tags::SOP_CLASS_UID)
            .expect("SEG file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        SEGMENTATION_STORAGE_UID
    );
    assert_eq!(
        segmentation
            .element(TAG_SEGMENTATION_TYPE)
            .expect("SEG file should contain Segmentation Type")
            .value()
            .to_str()
            .expect("Segmentation Type should be text")
            .trim(),
        "BINARY"
    );
    assert_eq!(
        segmentation
            .element(TAG_SEGMENT_SEQUENCE)
            .expect("SEG file should contain Segment Sequence")
            .items()
            .expect("Segment Sequence should be a sequence")
            .len(),
        1
    );
    assert_eq!(
        segmentation
            .element(tags::PIXEL_DATA)
            .expect("SEG file should contain Pixel Data")
            .value()
            .to_bytes()
            .expect("Pixel Data should be byte-backed")
            .len(),
        2
    );
    let fractional_segmentation_path =
        out_dir.join("derived/seg/fractional_probability_multiframe_explicit_le/instance.dcm");
    let fractional_segmentation =
        open_file(&fractional_segmentation_path).expect("fractional SEG DICOM file should parse");
    assert_eq!(
        fractional_segmentation
            .element(TAG_SEGMENTATION_TYPE)
            .expect("fractional SEG file should contain Segmentation Type")
            .value()
            .to_str()
            .expect("Segmentation Type should be text")
            .trim(),
        "FRACTIONAL"
    );
    assert_eq!(
        fractional_segmentation
            .element(TAG_SEGMENTATION_FRACTIONAL_TYPE)
            .expect("fractional SEG file should contain Segmentation Fractional Type")
            .value()
            .to_str()
            .expect("Segmentation Fractional Type should be text")
            .trim(),
        "PROBABILITY"
    );
    assert_eq!(
        fractional_segmentation
            .element(TAG_MAXIMUM_FRACTIONAL_VALUE)
            .expect("fractional SEG file should contain Maximum Fractional Value")
            .value()
            .to_int::<u16>()
            .expect("Maximum Fractional Value should be u16"),
        255
    );
    assert_eq!(
        fractional_segmentation
            .element(tags::BITS_ALLOCATED)
            .expect("fractional SEG file should contain Bits Allocated")
            .value()
            .to_int::<u16>()
            .expect("Bits Allocated should be u16"),
        8
    );
    assert_eq!(
        fractional_segmentation
            .element(tags::PIXEL_DATA)
            .expect("fractional SEG file should contain Pixel Data")
            .value()
            .to_bytes()
            .expect("Pixel Data should be byte-backed")
            .len(),
        8
    );
    let labelmap_segmentation_path =
        out_dir.join("derived/seg/labelmap_multiframe_explicit_le/instance.dcm");
    let labelmap_segmentation =
        open_file(&labelmap_segmentation_path).expect("LABELMAP SEG DICOM file should parse");
    assert_eq!(
        labelmap_segmentation
            .element(tags::SOP_CLASS_UID)
            .expect("LABELMAP SEG file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        LABEL_MAP_SEGMENTATION_STORAGE_UID
    );
    assert_eq!(
        labelmap_segmentation
            .element(TAG_SEGMENTATION_TYPE)
            .expect("LABELMAP SEG file should contain Segmentation Type")
            .value()
            .to_str()
            .expect("Segmentation Type should be text")
            .trim(),
        "LABELMAP"
    );
    assert_eq!(
        labelmap_segmentation
            .element(tags::BITS_ALLOCATED)
            .expect("LABELMAP SEG file should contain Bits Allocated")
            .value()
            .to_int::<u16>()
            .expect("Bits Allocated should be u16"),
        8
    );
    assert_eq!(
        labelmap_segmentation
            .element(tags::PIXEL_DATA)
            .expect("LABELMAP SEG file should contain Pixel Data")
            .value()
            .to_bytes()
            .expect("Pixel Data should be byte-backed")
            .len(),
        8
    );

    let presentation_state_path = out_dir
        .join("derived/presentation-state/grayscale_softcopy_ct_window_explicit_le/instance.dcm");
    let presentation_state =
        open_file(&presentation_state_path).expect("GSPS DICOM file should parse");
    assert_eq!(
        presentation_state
            .element(tags::SOP_CLASS_UID)
            .expect("GSPS file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID
    );
    assert_eq!(
        presentation_state
            .element(tags::MODALITY)
            .expect("GSPS file should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "PR"
    );
    assert!(
        presentation_state
            .element_opt(tags::PIXEL_DATA)
            .expect("GSPS Pixel Data lookup should not fail")
            .is_none(),
        "GSPS must not contain Pixel Data"
    );
    let referenced_series = presentation_state
        .element(TAG_REFERENCED_SERIES_SEQUENCE)
        .expect("GSPS should contain Referenced Series Sequence")
        .items()
        .expect("Referenced Series Sequence should be SQ")
        .first()
        .expect("Referenced Series Sequence should contain one item");
    let referenced_image = referenced_series
        .element(TAG_REFERENCED_IMAGE_SEQUENCE)
        .expect("GSPS referenced series should contain Referenced Image Sequence")
        .items()
        .expect("Referenced Image Sequence should be SQ")
        .first()
        .expect("Referenced Image Sequence should contain one item");
    assert_eq!(
        referenced_image
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("GSPS should reference a source SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    assert!(
        !referenced_image
            .element(TAG_REFERENCED_SOP_INSTANCE_UID)
            .expect("GSPS should reference a source SOP Instance")
            .value()
            .to_str()
            .expect("Referenced SOP Instance UID should be text")
            .trim_end_matches('\0')
            .is_empty(),
        "GSPS source SOP Instance UID reference should not be empty"
    );
    let displayed_area = presentation_state
        .element(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
        .expect("GSPS should contain Displayed Area Selection Sequence")
        .items()
        .expect("Displayed Area Selection Sequence should be SQ")
        .first()
        .expect("Displayed Area Selection Sequence should contain one item");
    assert_eq!(
        displayed_area
            .element(TAG_DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
            .expect("GSPS should contain displayed area TLHC")
            .value()
            .to_multi_int::<i32>()
            .expect("displayed area TLHC should be numeric"),
        vec![1, 1]
    );
    assert_eq!(
        displayed_area
            .element(TAG_DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)
            .expect("GSPS should contain displayed area BRHC")
            .value()
            .to_multi_int::<i32>()
            .expect("displayed area BRHC should be numeric"),
        vec![2, 2]
    );
    assert_eq!(
        displayed_area
            .element(TAG_PRESENTATION_SIZE_MODE)
            .expect("GSPS should contain Presentation Size Mode")
            .value()
            .to_str()
            .expect("Presentation Size Mode should be text")
            .trim(),
        "SCALE TO FIT"
    );
    assert_eq!(
        displayed_area
            .element(TAG_PRESENTATION_PIXEL_ASPECT_RATIO)
            .expect("GSPS should contain Presentation Pixel Aspect Ratio")
            .value()
            .to_multi_int::<i32>()
            .expect("Presentation Pixel Aspect Ratio should be numeric"),
        vec![1, 1]
    );
    let voi_lut = presentation_state
        .element(TAG_SOFTCOPY_VOI_LUT_SEQUENCE)
        .expect("GSPS should contain Softcopy VOI LUT Sequence")
        .items()
        .expect("Softcopy VOI LUT Sequence should be SQ")
        .first()
        .expect("Softcopy VOI LUT Sequence should contain one item");
    assert_eq!(
        voi_lut
            .element(tags::WINDOW_CENTER)
            .expect("GSPS should contain Window Center")
            .value()
            .to_str()
            .expect("Window Center should be text")
            .trim(),
        "350"
    );
    assert_eq!(
        voi_lut
            .element(tags::WINDOW_WIDTH)
            .expect("GSPS should contain Window Width")
            .value()
            .to_str()
            .expect("Window Width should be text")
            .trim(),
        "1400"
    );
    assert_eq!(
        presentation_state
            .element(TAG_PRESENTATION_LUT_SHAPE)
            .expect("GSPS should contain Presentation LUT Shape")
            .value()
            .to_str()
            .expect("Presentation LUT Shape should be text")
            .trim(),
        "IDENTITY"
    );

    let rwvm_path = out_dir.join("derived/rwvm/linear_ct_mapping_explicit_le/instance.dcm");
    let rwvm = open_file(&rwvm_path).expect("RWVM DICOM file should parse");
    assert_eq!(
        rwvm.element(tags::SOP_CLASS_UID)
            .expect("RWVM file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        REAL_WORLD_VALUE_MAPPING_STORAGE_UID
    );
    assert_eq!(
        rwvm.element(tags::MODALITY)
            .expect("RWVM file should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "RWV"
    );
    assert!(
        rwvm.element_opt(tags::PIXEL_DATA)
            .expect("RWVM Pixel Data lookup should not fail")
            .is_none(),
        "RWVM must not contain Pixel Data"
    );
    let mapping = rwvm
        .element(tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE)
        .expect("RWVM should contain Real World Value Mapping Sequence")
        .items()
        .expect("Real World Value Mapping Sequence should be SQ")
        .first()
        .expect("Real World Value Mapping Sequence should contain one item");
    assert_eq!(
        mapping
            .element(tags::LUT_LABEL)
            .expect("RWVM should contain LUT Label")
            .value()
            .to_str()
            .expect("LUT Label should be text")
            .trim(),
        "DTS_HU"
    );
    assert_eq!(
        mapping
            .element(tags::REAL_WORLD_VALUE_FIRST_VALUE_MAPPED)
            .expect("RWVM should contain first mapped value")
            .value()
            .to_int::<u16>()
            .expect("first mapped value should be US"),
        0
    );
    assert_eq!(
        mapping
            .element(tags::REAL_WORLD_VALUE_LAST_VALUE_MAPPED)
            .expect("RWVM should contain last mapped value")
            .value()
            .to_int::<u16>()
            .expect("last mapped value should be US"),
        700
    );
    assert_eq!(
        mapping
            .element(tags::REAL_WORLD_VALUE_INTERCEPT)
            .expect("RWVM should contain intercept")
            .value()
            .to_float64()
            .expect("intercept should be FD"),
        -1024.0
    );
    assert_eq!(
        mapping
            .element(tags::REAL_WORLD_VALUE_SLOPE)
            .expect("RWVM should contain slope")
            .value()
            .to_float64()
            .expect("slope should be FD"),
        1.0
    );
    let units = mapping
        .element(tags::MEASUREMENT_UNITS_CODE_SEQUENCE)
        .expect("RWVM should contain Measurement Units Code Sequence")
        .items()
        .expect("Measurement Units Code Sequence should be SQ")
        .first()
        .expect("Measurement Units Code Sequence should contain one item");
    assert_eq!(
        units
            .element(tags::CODE_VALUE)
            .expect("RWVM units should contain Code Value")
            .value()
            .to_str()
            .expect("Code Value should be text")
            .trim(),
        "HU"
    );
    assert_eq!(
        units
            .element(tags::CODING_SCHEME_DESIGNATOR)
            .expect("RWVM units should contain Coding Scheme Designator")
            .value()
            .to_str()
            .expect("Coding Scheme Designator should be text")
            .trim(),
        "UCUM"
    );
    assert_eq!(
        units
            .element(tags::CODE_MEANING)
            .expect("RWVM units should contain Code Meaning")
            .value()
            .to_str()
            .expect("Code Meaning should be text")
            .trim(),
        "Hounsfield unit"
    );
    let rwvm_referenced_image = mapping
        .element(TAG_REFERENCED_IMAGE_SEQUENCE)
        .expect("RWVM should contain Referenced Image Sequence")
        .items()
        .expect("Referenced Image Sequence should be SQ")
        .first()
        .expect("Referenced Image Sequence should contain one item");
    assert_eq!(
        rwvm_referenced_image
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("RWVM should reference a source SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    assert!(
        !rwvm_referenced_image
            .element(TAG_REFERENCED_SOP_INSTANCE_UID)
            .expect("RWVM should reference a source SOP Instance")
            .value()
            .to_str()
            .expect("Referenced SOP Instance UID should be text")
            .trim_end_matches('\0')
            .is_empty(),
        "RWVM source SOP Instance UID reference should not be empty"
    );
    assert_eq!(
        rwvm_referenced_image
            .element(TAG_REFERENCED_FRAME_NUMBER)
            .expect("RWVM should reference source frame numbers")
            .value()
            .to_multi_int::<i32>()
            .expect("Referenced Frame Number should be multi-value IS"),
        vec![1, 2]
    );

    let basic_text_sr_path =
        out_dir.join("derived/sr/basic_text_observation_explicit_le/instance.dcm");
    let basic_text_sr =
        open_file(&basic_text_sr_path).expect("Basic Text SR DICOM file should parse");
    assert_eq!(
        basic_text_sr
            .element(tags::SOP_CLASS_UID)
            .expect("Basic Text SR file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        BASIC_TEXT_SR_STORAGE_UID
    );
    assert_eq!(
        basic_text_sr
            .element(tags::MODALITY)
            .expect("Basic Text SR file should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "SR"
    );
    assert_eq!(
        basic_text_sr
            .element(tags::COMPLETION_FLAG)
            .expect("Basic Text SR should contain Completion Flag")
            .value()
            .to_str()
            .expect("Completion Flag should be text")
            .trim(),
        "COMPLETE"
    );
    assert_eq!(
        basic_text_sr
            .element(tags::VERIFICATION_FLAG)
            .expect("Basic Text SR should contain Verification Flag")
            .value()
            .to_str()
            .expect("Verification Flag should be text")
            .trim(),
        "UNVERIFIED"
    );
    assert!(
        basic_text_sr
            .element_opt(tags::PIXEL_DATA)
            .expect("Basic Text SR Pixel Data lookup should not fail")
            .is_none(),
        "Basic Text SR must not contain Pixel Data"
    );
    let evidence = basic_text_sr
        .element(tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE)
        .expect("Basic Text SR should contain Current Requested Procedure Evidence Sequence")
        .items()
        .expect("Current Requested Procedure Evidence Sequence should be SQ")
        .first()
        .expect("Current Requested Procedure Evidence Sequence should contain one item");
    let evidence_series = evidence
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("SR evidence should contain Referenced Series Sequence")
        .items()
        .expect("Referenced Series Sequence should be SQ")
        .first()
        .expect("Referenced Series Sequence should contain one item");
    let evidence_sop = evidence_series
        .element(tags::REFERENCED_SOP_SEQUENCE)
        .expect("SR evidence should contain Referenced SOP Sequence")
        .items()
        .expect("Referenced SOP Sequence should be SQ")
        .first()
        .expect("Referenced SOP Sequence should contain one item");
    assert_eq!(
        evidence_sop
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("SR evidence should reference a source SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    assert!(
        !evidence_sop
            .element(TAG_REFERENCED_SOP_INSTANCE_UID)
            .expect("SR evidence should reference a source SOP Instance")
            .value()
            .to_str()
            .expect("Referenced SOP Instance UID should be text")
            .trim_end_matches('\0')
            .is_empty(),
        "Basic Text SR evidence SOP Instance UID reference should not be empty"
    );
    assert_eq!(
        basic_text_sr
            .element(tags::VALUE_TYPE)
            .expect("Basic Text SR should contain root Value Type")
            .value()
            .to_str()
            .expect("Value Type should be text")
            .trim(),
        "CONTAINER"
    );
    let content = basic_text_sr
        .element(tags::CONTENT_SEQUENCE)
        .expect("Basic Text SR should contain Content Sequence")
        .items()
        .expect("Content Sequence should be SQ")
        .first()
        .expect("Content Sequence should contain one item");
    assert_eq!(
        content
            .element(tags::RELATIONSHIP_TYPE)
            .expect("SR content should contain Relationship Type")
            .value()
            .to_str()
            .expect("Relationship Type should be text")
            .trim(),
        "CONTAINS"
    );
    assert_eq!(
        content
            .element(tags::VALUE_TYPE)
            .expect("SR content should contain Value Type")
            .value()
            .to_str()
            .expect("Value Type should be text")
            .trim(),
        "TEXT"
    );
    assert_eq!(
        content
            .element(tags::TEXT_VALUE)
            .expect("SR content should contain Text Value")
            .value()
            .to_str()
            .expect("Text Value should be text")
            .trim(),
        "Synthetic Basic Text SR observation for Enhanced CT source images."
    );

    let comprehensive_sr_path =
        out_dir.join("derived/sr/comprehensive_measurement_explicit_le/instance.dcm");
    let comprehensive_sr =
        open_file(&comprehensive_sr_path).expect("Comprehensive SR DICOM file should parse");
    assert_eq!(
        comprehensive_sr
            .element(tags::SOP_CLASS_UID)
            .expect("Comprehensive SR file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        COMPREHENSIVE_SR_STORAGE_UID
    );
    assert!(
        comprehensive_sr
            .element_opt(tags::PIXEL_DATA)
            .expect("Comprehensive SR Pixel Data lookup should not fail")
            .is_none(),
        "Comprehensive SR must not contain Pixel Data"
    );
    let comprehensive_content = comprehensive_sr
        .element(tags::CONTENT_SEQUENCE)
        .expect("Comprehensive SR should contain Content Sequence")
        .items()
        .expect("Content Sequence should be SQ");
    assert_eq!(
        comprehensive_content.len(),
        2,
        "Comprehensive SR should contain measurement and image-reference items"
    );
    let measurement = &comprehensive_content[0];
    assert_eq!(
        measurement
            .element(tags::VALUE_TYPE)
            .expect("SR measurement should contain Value Type")
            .value()
            .to_str()
            .expect("Value Type should be text")
            .trim(),
        "NUM"
    );
    let measured_value = measurement
        .element(tags::MEASURED_VALUE_SEQUENCE)
        .expect("SR measurement should contain Measured Value Sequence")
        .items()
        .expect("Measured Value Sequence should be SQ")
        .first()
        .expect("Measured Value Sequence should contain one item");
    assert_eq!(
        measured_value
            .element(tags::NUMERIC_VALUE)
            .expect("Measured Value should contain Numeric Value")
            .value()
            .to_str()
            .expect("Numeric Value should be text")
            .trim(),
        "12.5"
    );
    let units = measured_value
        .element(tags::MEASUREMENT_UNITS_CODE_SEQUENCE)
        .expect("Measured Value should contain Measurement Units Code Sequence")
        .items()
        .expect("Measurement Units Code Sequence should be SQ")
        .first()
        .expect("Measurement Units Code Sequence should contain one item");
    assert_eq!(
        units
            .element(tags::CODE_VALUE)
            .expect("Units should contain Code Value")
            .value()
            .to_str()
            .expect("Code Value should be text")
            .trim(),
        "mm"
    );
    let image_reference = &comprehensive_content[1];
    assert_eq!(
        image_reference
            .element(tags::VALUE_TYPE)
            .expect("SR image reference should contain Value Type")
            .value()
            .to_str()
            .expect("Value Type should be text")
            .trim(),
        "IMAGE"
    );
    let image_sop = image_reference
        .element(tags::REFERENCED_SOP_SEQUENCE)
        .expect("SR image reference should contain Referenced SOP Sequence")
        .items()
        .expect("Referenced SOP Sequence should be SQ")
        .first()
        .expect("Referenced SOP Sequence should contain one item");
    assert_eq!(
        image_sop
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("SR image reference should contain source SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    assert_eq!(
        image_sop
            .element(TAG_REFERENCED_FRAME_NUMBER)
            .expect("SR image reference should contain frame numbers")
            .value()
            .to_multi_int::<i32>()
            .expect("Referenced Frame Number should be multi-value IS"),
        vec![1, 2]
    );

    let kos_path = out_dir.join("derived/sr/key_object_selection_explicit_le/instance.dcm");
    let kos = open_file(&kos_path).expect("KOS DICOM file should parse");
    assert_eq!(
        kos.element(tags::SOP_CLASS_UID)
            .expect("KOS file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID
    );
    assert_eq!(
        kos.element(tags::MODALITY)
            .expect("KOS file should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "KO"
    );
    assert!(
        kos.element_opt(tags::PIXEL_DATA)
            .expect("KOS Pixel Data lookup should not fail")
            .is_none(),
        "KOS must not contain Pixel Data"
    );
    let kos_evidence = kos
        .element(tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE)
        .expect("KOS should contain evidence sequence")
        .items()
        .expect("KOS evidence sequence should be SQ")
        .first()
        .expect("KOS evidence sequence should contain one study item");
    let kos_evidence_series = kos_evidence
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("KOS evidence should contain Referenced Series Sequence")
        .items()
        .expect("Referenced Series Sequence should be SQ");
    assert_eq!(
        kos_evidence_series.len(),
        2,
        "KOS evidence should reference the source CT and SEG series"
    );
    let kos_content = kos
        .element(tags::CONTENT_SEQUENCE)
        .expect("KOS should contain Content Sequence")
        .items()
        .expect("Content Sequence should be SQ");
    assert_eq!(
        kos_content.len(),
        2,
        "KOS should contain two IMAGE key object items"
    );
    assert_eq!(
        kos_content[0]
            .element(tags::VALUE_TYPE)
            .expect("first KOS item should contain Value Type")
            .value()
            .to_str()
            .expect("Value Type should be text")
            .trim(),
        "IMAGE"
    );
    let first_kos_sop = kos_content[0]
        .element(tags::REFERENCED_SOP_SEQUENCE)
        .expect("first KOS item should contain Referenced SOP Sequence")
        .items()
        .expect("Referenced SOP Sequence should be SQ")
        .first()
        .expect("Referenced SOP Sequence should contain one item");
    assert_eq!(
        first_kos_sop
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("first KOS item should reference source SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    assert_eq!(
        first_kos_sop
            .element(TAG_REFERENCED_FRAME_NUMBER)
            .expect("first KOS item should reference source frames")
            .value()
            .to_multi_int::<i32>()
            .expect("Referenced Frame Number should be multi-value IS"),
        vec![1, 2]
    );
    let second_kos_sop = kos_content[1]
        .element(tags::REFERENCED_SOP_SEQUENCE)
        .expect("second KOS item should contain Referenced SOP Sequence")
        .items()
        .expect("Referenced SOP Sequence should be SQ")
        .first()
        .expect("Referenced SOP Sequence should contain one item");
    assert_eq!(
        second_kos_sop
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("second KOS item should reference SEG SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        SEGMENTATION_STORAGE_UID
    );

    let rt_structure_set_path =
        out_dir.join("non-image/rt/structure_set_single_roi_explicit_le/instance.dcm");
    let rt_structure_set =
        open_file(&rt_structure_set_path).expect("RT Structure Set DICOM file should parse");
    assert_eq!(
        rt_structure_set
            .element(tags::SOP_CLASS_UID)
            .expect("RT Structure Set file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        RT_STRUCTURE_SET_STORAGE_UID
    );
    assert_eq!(
        rt_structure_set
            .element(tags::MODALITY)
            .expect("RT Structure Set should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "RTSTRUCT"
    );
    assert!(
        rt_structure_set
            .element_opt(tags::PIXEL_DATA)
            .expect("RT Structure Set Pixel Data lookup should not fail")
            .is_none(),
        "RT Structure Set must not contain Pixel Data"
    );
    let structure_set_roi = rt_structure_set
        .element(tags::STRUCTURE_SET_ROI_SEQUENCE)
        .expect("RT Structure Set should contain Structure Set ROI Sequence")
        .items()
        .expect("Structure Set ROI Sequence should be SQ")
        .first()
        .expect("Structure Set ROI Sequence should contain one item");
    assert_eq!(
        structure_set_roi
            .element(tags::ROI_NAME)
            .expect("Structure Set ROI item should contain ROI Name")
            .value()
            .to_str()
            .expect("ROI Name should be text")
            .trim(),
        "DTS_SYNTHETIC_ROI"
    );
    let roi_contour = rt_structure_set
        .element(tags::ROI_CONTOUR_SEQUENCE)
        .expect("RT Structure Set should contain ROI Contour Sequence")
        .items()
        .expect("ROI Contour Sequence should be SQ")
        .first()
        .expect("ROI Contour Sequence should contain one item");
    let contour = roi_contour
        .element(tags::CONTOUR_SEQUENCE)
        .expect("ROI Contour should contain Contour Sequence")
        .items()
        .expect("Contour Sequence should be SQ")
        .first()
        .expect("Contour Sequence should contain one item");
    assert_eq!(
        contour
            .element(tags::CONTOUR_GEOMETRIC_TYPE)
            .expect("Contour should contain Contour Geometric Type")
            .value()
            .to_str()
            .expect("Contour Geometric Type should be text")
            .trim(),
        "CLOSED_PLANAR"
    );
    let contour_image = contour
        .element(tags::CONTOUR_IMAGE_SEQUENCE)
        .expect("Contour should contain Contour Image Sequence")
        .items()
        .expect("Contour Image Sequence should be SQ")
        .first()
        .expect("Contour Image Sequence should contain one item");
    assert_eq!(
        contour_image
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("Contour Image should reference source SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::ENHANCED_CT_IMAGE_STORAGE
    );
    let rt_observation = rt_structure_set
        .element(tags::RTROI_OBSERVATIONS_SEQUENCE)
        .expect("RT Structure Set should contain RT ROI Observations Sequence")
        .items()
        .expect("RT ROI Observations Sequence should be SQ")
        .first()
        .expect("RT ROI Observations Sequence should contain one item");
    assert_eq!(
        rt_observation
            .element(tags::RTROI_INTERPRETED_TYPE)
            .expect("RT ROI Observation should contain interpreted type")
            .value()
            .to_str()
            .expect("RT ROI Interpreted Type should be text")
            .trim(),
        "ORGAN"
    );

    let rt_dose_path = out_dir.join("non-image/rt/dose_grid_u16_explicit_le/instance.dcm");
    let rt_dose = open_file(&rt_dose_path).expect("RT Dose DICOM file should parse");
    assert_eq!(
        rt_dose
            .element(tags::SOP_CLASS_UID)
            .expect("RT Dose file should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        RT_DOSE_STORAGE_UID
    );
    assert_eq!(
        rt_dose
            .element(tags::MODALITY)
            .expect("RT Dose should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "RTDOSE"
    );
    assert_eq!(
        rt_dose
            .element(tags::DOSE_UNITS)
            .expect("RT Dose should contain Dose Units")
            .value()
            .to_str()
            .expect("Dose Units should be text")
            .trim(),
        "GY"
    );
    assert_eq!(
        rt_dose
            .element(tags::DOSE_TYPE)
            .expect("RT Dose should contain Dose Type")
            .value()
            .to_str()
            .expect("Dose Type should be text")
            .trim(),
        "PHYSICAL"
    );
    assert_eq!(
        rt_dose
            .element(tags::DOSE_SUMMATION_TYPE)
            .expect("RT Dose should contain Dose Summation Type")
            .value()
            .to_str()
            .expect("Dose Summation Type should be text")
            .trim(),
        "RECORD"
    );
    assert_eq!(
        rt_dose
            .element(tags::DOSE_GRID_SCALING)
            .expect("RT Dose should contain Dose Grid Scaling")
            .value()
            .to_str()
            .expect("Dose Grid Scaling should be text")
            .trim(),
        "0.001"
    );
    assert_eq!(
        rt_dose
            .element(tags::FRAME_INCREMENT_POINTER)
            .expect("RT Dose should contain Frame Increment Pointer")
            .value()
            .tags()
            .expect("Frame Increment Pointer should be AT"),
        &[tags::GRID_FRAME_OFFSET_VECTOR]
    );
    assert_eq!(
        rt_dose
            .element(tags::PIXEL_DATA)
            .expect("RT Dose should contain Pixel Data")
            .value()
            .to_bytes()
            .expect("RT Dose Pixel Data should be bytes")
            .len(),
        16
    );
    let referenced_structure_set = rt_dose
        .element(TAG_REFERENCED_STRUCTURE_SET_SEQUENCE)
        .expect("RT Dose should contain Referenced Structure Set Sequence")
        .items()
        .expect("Referenced Structure Set Sequence should be SQ")
        .first()
        .expect("Referenced Structure Set Sequence should contain one item");
    assert_eq!(
        referenced_structure_set
            .element(TAG_REFERENCED_SOP_CLASS_UID)
            .expect("RT Dose should reference RT Structure Set SOP Class")
            .value()
            .to_str()
            .expect("Referenced SOP Class UID should be text")
            .trim_end_matches('\0'),
        RT_STRUCTURE_SET_STORAGE_UID
    );

    let encapsulated_pdf_path =
        out_dir.join("non-image/encapsulated-document/pdf_minimal_explicit_le/instance.dcm");
    let encapsulated_pdf =
        open_file(&encapsulated_pdf_path).expect("Encapsulated PDF DICOM file should parse");
    assert_eq!(
        encapsulated_pdf
            .element(tags::SOP_CLASS_UID)
            .expect("Encapsulated PDF should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        ENCAPSULATED_PDF_STORAGE_UID
    );
    assert_eq!(
        encapsulated_pdf
            .element(tags::MODALITY)
            .expect("Encapsulated PDF should contain Modality")
            .value()
            .to_str()
            .expect("Modality should be text")
            .trim(),
        "DOC"
    );
    assert_eq!(
        encapsulated_pdf
            .element(tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT)
            .expect("Encapsulated PDF should contain MIME Type")
            .value()
            .to_str()
            .expect("MIME Type should be text")
            .trim(),
        "application/pdf"
    );
    assert_eq!(
        encapsulated_pdf
            .element(tags::DOCUMENT_TITLE)
            .expect("Encapsulated PDF should contain Document Title")
            .value()
            .to_str()
            .expect("Document Title should be text")
            .trim(),
        "DTS Minimal Synthetic PDF"
    );
    let document_bytes = encapsulated_pdf
        .element(tags::ENCAPSULATED_DOCUMENT)
        .expect("Encapsulated PDF should contain Encapsulated Document")
        .value()
        .to_bytes()
        .expect("Encapsulated Document should be bytes");
    assert!(
        document_bytes.as_ref().starts_with(b"%PDF-1.4\n"),
        "Encapsulated Document should contain a deterministic PDF payload"
    );

    let enhanced_ct_concat_part_1_path =
        out_dir.join("enhanced/ct/concatenation_two_part_explicit_le/part-001.dcm");
    let enhanced_ct_concat_part_2_path =
        out_dir.join("enhanced/ct/concatenation_two_part_explicit_le/part-002.dcm");
    let enhanced_ct_concat_part_1 =
        open_file(&enhanced_ct_concat_part_1_path).expect("first concatenation part should parse");
    let enhanced_ct_concat_part_2 =
        open_file(&enhanced_ct_concat_part_2_path).expect("second concatenation part should parse");
    let concatenation_uid = enhanced_ct_concat_part_1
        .element(tags::CONCATENATION_UID)
        .expect("first part should contain Concatenation UID")
        .value()
        .to_str()
        .expect("Concatenation UID should be text")
        .trim_end_matches('\0')
        .to_string();
    assert_eq!(
        enhanced_ct_concat_part_2
            .element(tags::CONCATENATION_UID)
            .expect("second part should contain Concatenation UID")
            .value()
            .to_str()
            .expect("Concatenation UID should be text")
            .trim_end_matches('\0'),
        concatenation_uid
    );
    assert_eq!(
        enhanced_ct_concat_part_1
            .element(tags::IN_CONCATENATION_NUMBER)
            .expect("first part should contain In-concatenation Number")
            .value()
            .to_int::<u16>()
            .expect("In-concatenation Number should be US"),
        1
    );
    assert_eq!(
        enhanced_ct_concat_part_2
            .element(tags::IN_CONCATENATION_NUMBER)
            .expect("second part should contain In-concatenation Number")
            .value()
            .to_int::<u16>()
            .expect("In-concatenation Number should be US"),
        2
    );
    assert_eq!(
        enhanced_ct_concat_part_1
            .element(tags::CONCATENATION_FRAME_OFFSET_NUMBER)
            .expect("first part should contain Concatenation Frame Offset Number")
            .value()
            .to_int::<u32>()
            .expect("Concatenation Frame Offset Number should be UL"),
        0
    );
    assert_eq!(
        enhanced_ct_concat_part_2
            .element(tags::CONCATENATION_FRAME_OFFSET_NUMBER)
            .expect("second part should contain Concatenation Frame Offset Number")
            .value()
            .to_int::<u32>()
            .expect("Concatenation Frame Offset Number should be UL"),
        1
    );
    let second_concat_per_frame_items = enhanced_ct_concat_part_2
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .expect("second part should contain Per-Frame Functional Groups Sequence")
        .items()
        .expect("Per-Frame Functional Groups should be a sequence");
    assert_eq!(second_concat_per_frame_items.len(), 1);
    let second_concat_frame_content = second_concat_per_frame_items[0]
        .element(tags::FRAME_CONTENT_SEQUENCE)
        .expect("second part frame should contain Frame Content Sequence")
        .items()
        .expect("Frame Content should be a sequence");
    assert_eq!(
        second_concat_frame_content[0]
            .element(tags::DIMENSION_INDEX_VALUES)
            .expect("second part frame should contain Dimension Index Values")
            .value()
            .to_int::<u32>()
            .expect("Dimension Index Values should be UL"),
        2
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

    let enhanced_mr_phase_path =
        out_dir.join("enhanced/mr/multiframe_phase_velocity_encoding_explicit_le/instance.dcm");
    let enhanced_mr_phase =
        open_file(&enhanced_mr_phase_path).expect("Enhanced MR phase DICOM file should parse");
    let phase_per_frame_items = enhanced_mr_phase
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .expect("Enhanced MR phase file should contain Per-Frame Functional Groups Sequence")
        .items()
        .expect("Per-Frame Functional Groups should be a sequence");
    assert_eq!(phase_per_frame_items.len(), 2);
    let second_velocity_encoding_item = phase_per_frame_items[1]
        .element(tags::MR_VELOCITY_ENCODING_SEQUENCE)
        .expect("second frame should contain MR Velocity Encoding Sequence")
        .items()
        .expect("MR Velocity Encoding should be a sequence");
    assert_eq!(
        second_velocity_encoding_item[0]
            .element(tags::VELOCITY_ENCODING_DIRECTION)
            .expect("MR Velocity Encoding should contain Velocity Encoding Direction")
            .value()
            .to_multi_float64()
            .expect("Velocity Encoding Direction should be FD VM 3"),
        vec![0.0, 1.0, 0.0]
    );
    assert_eq!(
        second_velocity_encoding_item[0]
            .element(tags::VELOCITY_ENCODING_MAXIMUM_VALUE)
            .expect("MR Velocity Encoding should contain Velocity Encoding Maximum Value")
            .value()
            .to_float64()
            .expect("Velocity Encoding Maximum Value should be FD"),
        150.0
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn generate_command_writes_all_profile_union_and_skips_planned_cases() {
    let out_dir = unique_temp_dir("generate-all-command");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "all",
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
    assert!(stdout.contains("profile\tall"));
    let expected_all_files = 44
        + if cfg!(feature = "deflate") { 2 } else { 0 }
        + if cfg!(feature = "jpeg") { 1 } else { 0 }
        + if cfg!(feature = "charls") { 1 } else { 0 }
        + if cfg!(feature = "jpegxl") { 1 } else { 0 }
        + if cfg!(feature = "jpeg2000") { 1 } else { 0 }
        + if cfg!(feature = "htj2k_openjph") {
            1
        } else {
            0
        }
        + if cfg!(feature = "legacy_jpeg_dcmtk") {
            2
        } else {
            0
        };
    assert!(stdout.contains(&format!("files_written\t{expected_all_files}")));

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    assert_manifest_matches_committed_schema(&manifest);
    assert_eq!(
        manifest.pointer("/run/profile").and_then(Value::as_str),
        Some("all")
    );
    assert_eq!(
        manifest
            .pointer("/files")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(expected_all_files)
    );

    file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_explicit_le");
    file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_rle_lossless");
    file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_rle_lossless");
    file_entry_by_case_id(&manifest, "classic/sc/rgb_planar0_rle_lossless");
    file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_multiframe_rle_lossless");
    file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_odd_fragment_rle_lossless");
    if cfg!(feature = "jpeg") {
        file_entry_by_case_id(&manifest, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
    }
    if cfg!(feature = "charls") {
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_jpeg_ls_lossless");
    }
    if cfg!(feature = "jpegxl") {
        file_entry_by_case_id(&manifest, "classic/sc/rgb_planar0_jpegxl_lossless");
    }
    if cfg!(feature = "jpeg2000") {
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_jpeg2000_lossless");
    }
    if cfg!(feature = "htj2k_openjph") {
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_htj2k_lossless");
    }
    if cfg!(feature = "legacy_jpeg_dcmtk") {
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_process_14");
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_sv1");
    }
    file_entry_by_case_id(&manifest, "classic/ct/mono2_i16_rescale_12bit_explicit_le");
    file_entry_by_case_id(
        &manifest,
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
    );
    assert_eq!(
        file_entries_by_case_id(&manifest, "classic/mr/multislice_oblique_explicit_le").len(),
        3,
        "all profile should include every file in the multi-instance MR case"
    );
    assert_eq!(
        file_entries_by_case_id(&manifest, "enhanced/ct/concatenation_two_part_explicit_le").len(),
        2,
        "all profile should include both Enhanced CT concatenation members"
    );

    let skipped_cases = manifest
        .pointer("/skipped_cases")
        .and_then(Value::as_array)
        .expect("manifest should contain skipped cases");
    assert_eq!(
        skipped_cases.len(),
        11 - if cfg!(feature = "deflate") { 2 } else { 0 }
            - if cfg!(feature = "jpeg") { 1 } else { 0 }
            - if cfg!(feature = "charls") { 1 } else { 0 }
            - if cfg!(feature = "jpegxl") { 1 } else { 0 }
            - if cfg!(feature = "jpeg2000") { 1 } else { 0 }
            - if cfg!(feature = "htj2k_openjph") {
                1
            } else {
                0
            }
            - if cfg!(feature = "legacy_jpeg_dcmtk") {
                2
            } else {
                0
            },
        "all generation should report unavailable cases according to active features"
    );
    for case_id in [
        "vl/photo/rgb_planar0_explicit_le",
        "vl/photo/palette_color_explicit_le",
    ] {
        let skipped = skipped_case_by_id(&manifest, case_id);
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("case_planned")
        );
    }
    if !cfg!(feature = "htj2k_openjph") {
        let skipped = skipped_case_by_id(&manifest, "classic/sc/mono2_u16_htj2k_lossless");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    if !cfg!(feature = "jpeg") {
        let skipped = skipped_case_by_id(&manifest, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    if !cfg!(feature = "jpegxl") {
        let skipped = skipped_case_by_id(&manifest, "classic/sc/rgb_planar0_jpegxl_lossless");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    if !cfg!(feature = "jpeg2000") {
        let skipped = skipped_case_by_id(&manifest, "classic/sc/mono2_u16_jpeg2000_lossless");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    if !cfg!(feature = "charls") {
        let skipped = skipped_case_by_id(&manifest, "classic/sc/mono2_u8_jpeg_ls_lossless");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    if !cfg!(feature = "legacy_jpeg_dcmtk") {
        let skipped =
            skipped_case_by_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_process_14");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );

        let skipped = skipped_case_by_id(&manifest, "classic/sc/mono2_u16_jpeg_lossless_sv1");
        assert_eq!(
            skipped.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            skipped.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    if cfg!(feature = "deflate") {
        file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_deflated_explicit_le");
        file_entry_by_case_id(
            &manifest,
            "derived/seg/binary_multiframe_deflated_image_frame",
        );
    } else {
        let deflated = skipped_case_by_id(&manifest, "classic/sc/mono2_u8_deflated_explicit_le");
        assert_eq!(
            deflated.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            deflated.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
        let deflated_seg = skipped_case_by_id(
            &manifest,
            "derived/seg/binary_multiframe_deflated_image_frame",
        );
        assert_eq!(
            deflated_seg.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            deflated_seg.get("reason_code").and_then(Value::as_str),
            Some("feature_gated_case_unavailable")
        );
    }
    assert!(
        skipped_cases.iter().all(|case| {
            !matches!(
                case.get("case_id").and_then(Value::as_str),
                Some("classic/sc/mono2_u8_explicit_le")
                    | Some("classic/ct/mono2_i16_rescale_12bit_explicit_le")
                    | Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
            )
        }),
        "all generation should not report implemented union cases as skipped"
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn generate_command_writes_legacy_explicit_big_endian_secondary_capture_case() {
    let out_dir = unique_temp_dir("generate-legacy-big-endian-command");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "legacy",
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
    assert!(stdout.contains("profile\tlegacy"));
    assert!(stdout.contains("files_written\t1"));

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    assert_manifest_matches_committed_schema(&manifest);

    let file_entry = file_entry_by_case_id(&manifest, "classic/sc/mono2_u8_explicit_be");
    assert_eq!(
        file_entry
            .pointer("/profile_membership/0")
            .and_then(Value::as_str),
        Some("legacy")
    );
    assert_eq!(
        file_entry
            .pointer("/dicom/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.2")
    );
    assert_eq!(
        file_entry
            .pointer("/dicom/transfer_syntax_name")
            .and_then(Value::as_str),
        Some("Explicit VR Big Endian")
    );
    assert!(
        file_entry
            .pointer("/known_stressors")
            .and_then(Value::as_array)
            .is_some_and(|stressors| stressors
                .iter()
                .any(|stress| { stress.as_str() == Some("explicit_vr_big_endian_dataset") })),
        "manifest should label the retired Big Endian dataset stressor"
    );
    assert!(
        validation_results_named(&manifest, "/files/0/validation/standards")
            .contains(&"explicit_vr_big_endian_transfer_syntax"),
        "manifest should record standards validation for Explicit VR Big Endian"
    );
    assert!(
        manifest
            .pointer("/skipped_cases")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "legacy generation should not skip the implemented Big Endian case"
    );

    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_be/instance.dcm");
    let dcm_bytes = fs::read(&dcm_path).expect("generated DICOM file should be readable");
    assert_eq!(&dcm_bytes[128..132], b"DICM", "file must be Part 10");
    assert_eq!(
        &dcm_bytes[132..140],
        &[0x02, 0x00, 0x00, 0x00, b'U', b'L', 0x04, 0x00],
        "File Meta Information must remain encoded as Explicit VR Little Endian"
    );
    assert!(
        dcm_bytes
            .windows(6)
            .any(|window| window == [0x00, 0x08, 0x00, 0x16, b'U', b'I']),
        "dataset SOP Class UID tag should be encoded as Explicit VR Big Endian"
    );

    let obj = open_file(&dcm_path).expect("generated Big Endian DICOM file should parse");
    assert_eq!(
        obj.meta().transfer_syntax().trim_end_matches('\0'),
        "1.2.840.10008.1.2.2"
    );
    assert_eq!(
        obj.element(tags::SOP_CLASS_UID)
            .expect("dataset should contain SOP Class UID")
            .value()
            .to_str()
            .expect("SOP Class UID should be text")
            .trim_end_matches('\0'),
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE
    );
    assert_eq!(
        obj.element(tags::ROWS)
            .expect("dataset should contain Rows")
            .value()
            .to_int::<u16>()
            .expect("Rows should be a u16"),
        2
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

fn skipped_case_by_id<'a>(manifest: &'a Value, case_id: &str) -> &'a Value {
    manifest
        .pointer("/skipped_cases")
        .and_then(Value::as_array)
        .expect("manifest skipped cases should be an array")
        .iter()
        .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
        .unwrap_or_else(|| panic!("manifest skipped cases should contain {case_id}"))
}

fn assert_manifest_matches_committed_schema(manifest: &Value) {
    let schema = read_json("schemas/manifest.schema.json");
    assert_required_fields(manifest, &schema, "/required", "manifest");
    assert_allowed_properties(manifest, &schema, "/properties", "manifest");
    assert_eq!(
        manifest
            .get("manifest_schema_version")
            .and_then(Value::as_str),
        schema
            .pointer("/properties/manifest_schema_version/const")
            .and_then(Value::as_str),
        "manifest schema version must match committed schema"
    );

    assert_required_fields(
        manifest
            .get("generator")
            .expect("manifest generator should exist"),
        &schema,
        "/$defs/generator/required",
        "generator",
    );
    assert_allowed_properties(
        manifest
            .get("generator")
            .expect("manifest generator should exist"),
        &schema,
        "/$defs/generator/properties",
        "generator",
    );
    assert_required_fields(
        manifest
            .get("standards")
            .expect("manifest standards should exist"),
        &schema,
        "/$defs/standards/required",
        "standards",
    );
    assert_required_fields(
        manifest
            .get("dependencies")
            .expect("manifest dependencies should exist"),
        &schema,
        "/$defs/dependencies/required",
        "dependencies",
    );
    assert_required_fields(
        manifest.get("run").expect("manifest run should exist"),
        &schema,
        "/$defs/run/required",
        "run",
    );
    assert_allowed_properties(
        manifest.get("run").expect("manifest run should exist"),
        &schema,
        "/$defs/run/properties",
        "run",
    );

    for (index, file) in manifest
        .get("files")
        .and_then(Value::as_array)
        .expect("manifest files should be an array")
        .iter()
        .enumerate()
    {
        assert_required_fields(
            file,
            &schema,
            "/$defs/file/required",
            &format!("files[{index}]"),
        );
        assert_allowed_properties(
            file,
            &schema,
            "/$defs/file/properties",
            &format!("files[{index}]"),
        );
        assert_required_fields(
            file.get("recipe").expect("file recipe should exist"),
            &schema,
            "/$defs/recipe/required",
            &format!("files[{index}].recipe"),
        );
        assert_required_fields(
            file.get("dicom").expect("file dicom should exist"),
            &schema,
            "/$defs/dicom/required",
            &format!("files[{index}].dicom"),
        );
        assert_required_fields(
            file.get("uids").expect("file uids should exist"),
            &schema,
            "/$defs/uids/required",
            &format!("files[{index}].uids"),
        );
        if let Some(image) = file.get("image").filter(|value| value.is_object()) {
            assert_required_fields(
                image,
                &schema,
                "/$defs/image/required",
                &format!("files[{index}].image"),
            );
        }
        if let Some(pixel_data) = file.get("pixel_data").filter(|value| value.is_object()) {
            assert_required_fields(
                pixel_data,
                &schema,
                "/$defs/pixel_data/required",
                &format!("files[{index}].pixel_data"),
            );
        }
        assert_required_fields(
            file.get("validation")
                .expect("file validation should exist"),
            &schema,
            "/$defs/validation/required",
            &format!("files[{index}].validation"),
        );
    }

    for (index, skipped) in manifest
        .get("skipped_cases")
        .and_then(Value::as_array)
        .expect("manifest skipped_cases should be an array")
        .iter()
        .enumerate()
    {
        assert_required_fields(
            skipped,
            &schema,
            "/$defs/skipped_case/required",
            &format!("skipped_cases[{index}]"),
        );
        assert_allowed_properties(
            skipped,
            &schema,
            "/$defs/skipped_case/properties",
            &format!("skipped_cases[{index}]"),
        );
    }
}

fn assert_required_fields(value: &Value, schema: &Value, pointer: &str, label: &str) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{label} should be an object"));
    for required in schema
        .pointer(pointer)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("schema {pointer} should be an array"))
    {
        let field = required
            .as_str()
            .unwrap_or_else(|| panic!("schema {pointer} entries should be strings"));
        assert!(object.contains_key(field), "{label} should contain {field}");
    }
}

fn assert_allowed_properties(value: &Value, schema: &Value, pointer: &str, label: &str) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{label} should be an object"));
    let allowed = schema
        .pointer(pointer)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("schema {pointer} should be an object"));
    for field in object.keys() {
        assert!(
            allowed.contains_key(field),
            "{label} has property {field} not allowed by schema"
        );
    }
}

fn read_json(path: &str) -> Value {
    let contents = fs::read_to_string(path).unwrap_or_else(|err| panic!("{path} readable: {err}"));
    serde_json::from_str(&contents).unwrap_or_else(|err| panic!("{path} should parse: {err}"))
}
