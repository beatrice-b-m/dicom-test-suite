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
    assert!(stdout.contains("files_written\t12"));

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
        Some(12)
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
                    )
                })
            }),
        "implemented CT and MG cases should not be reported as skipped"
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
