use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

#[test]
fn validate_command_accepts_generated_smoke_root() {
    let out_dir = unique_temp_dir("validate-smoke");
    generate_smoke(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        output.status.success(),
        "validate should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("files_checked\t3"));
    assert!(stdout.contains("validation_failures\t0"));
    assert!(stdout.contains(&format!(
        "manifest\t{}",
        out_dir.join("manifest.json").display()
    )));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_accepts_generated_extended_root() {
    let out_dir = unique_temp_dir("validate-extended");
    generate_profile(&out_dir, "extended");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        output.status.success(),
        "validate should accept generated extended output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    let expected_files = 30
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
    assert!(stdout.contains(&format!("files_checked\t{expected_files}")));
    assert!(stdout.contains("validation_failures\t0"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_accepts_references_to_same_run_sources() {
    let out_dir = unique_temp_dir("validate-reference-source");
    generate_smoke(&out_dir);
    append_reference_fixture(&out_dir, false);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        output.status.success(),
        "validate should accept resolved same-run references: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("files_checked\t4"));
    assert!(stdout.contains("validation_failures\t0"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_unresolved_reference_identity() {
    let out_dir = unique_temp_dir("validate-bad-reference");
    generate_smoke(&out_dir);
    append_reference_fixture(&out_dir, true);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject references whose manifest identity does not match the source"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("reference_sop_instance_uid"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_extended_offset_table_for_native_pixel_data() {
    let out_dir = unique_temp_dir("validate-native-extended-offset-table");
    generate_smoke(&out_dir);
    mutate_first_file_pixel_data(&out_dir, |pixel_data| {
        pixel_data.insert(
            "encapsulated_pixel_data".to_string(),
            json!({
                "basic_offset_table": {
                    "present": true,
                    "populated": false,
                    "offset_count": 0
                },
                "fragments_per_frame": [1],
                "extended_offset_table": {
                    "present": true,
                    "lengths_present": true,
                    "offset_count": 1,
                    "length_count": 1
                },
                "compressed_frame_hashes": [
                    "0000000000000000000000000000000000000000000000000000000000000000"
                ]
            }),
        );
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject Extended Offset Table metadata for native Pixel Data"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("extended_offset_table_native_pixel_data"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_invalid_encapsulated_offset_table_combination() {
    let out_dir = unique_temp_dir("validate-invalid-encapsulated-offsets");
    generate_smoke(&out_dir);
    mutate_first_file_pixel_data(&out_dir, |pixel_data| {
        pixel_data.insert(
            "native_or_encapsulated".to_string(),
            Value::String("encapsulated".to_string()),
        );
        pixel_data.insert("value_length".to_string(), Value::Null);
        pixel_data.insert(
            "encapsulated_pixel_data".to_string(),
            json!({
                "basic_offset_table": {
                    "present": true,
                    "populated": true,
                    "offset_count": 1
                },
                "fragments_per_frame": [2],
                "extended_offset_table": {
                    "present": true,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": [
                    "0000000000000000000000000000000000000000000000000000000000000000"
                ]
            }),
        );
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject invalid encapsulated Pixel Data offset-table combinations"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("extended_offset_table_with_populated_basic_offset_table"));
    assert!(stdout.contains("extended_offset_table_multiple_fragments"));
    assert!(stdout.contains("extended_offset_table_without_lengths"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_rle_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-rle-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(&out_dir, "classic/sc/mono2_u8_rle_lossless", |pixel_data| {
        pixel_data.insert(
            "frame_hashes".to_string(),
            json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
        );
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject RLE files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("rle_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "deflate")]
fn validate_command_reports_deflated_image_frame_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-deflated-image-frame-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "derived/seg/binary_multiframe_deflated_image_frame",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!([
                    "0000000000000000000000000000000000000000000000000000000000000000",
                    "0000000000000000000000000000000000000000000000000000000000000000"
                ]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject Deflated Image Frame files whose decoded native hashes do not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("deflated_image_frame_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "charls")]
fn validate_command_reports_jpeg_ls_lossless_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-jpeg-ls-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "classic/sc/mono2_u8_jpeg_ls_lossless",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject JPEG-LS files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("jpeg_ls_lossless_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "jpegxl")]
fn validate_command_reports_jpeg_xl_lossless_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-jpeg-xl-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "classic/sc/rgb_planar0_jpegxl_lossless",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject JPEG XL files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("jpeg_xl_lossless_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "jpeg2000")]
fn validate_command_reports_jpeg_2000_lossless_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-jpeg-2000-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "classic/sc/mono2_u16_jpeg2000_lossless",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject JPEG 2000 files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("jpeg_2000_lossless_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "htj2k_openjph")]
fn validate_command_reports_htj2k_lossless_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-htj2k-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "classic/sc/mono2_u16_htj2k_lossless",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject HTJ2K files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("htj2k_lossless_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "legacy_jpeg_dcmtk")]
fn validate_command_reports_jpeg_lossless_process_14_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-jpeg-lossless-process-14-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "classic/sc/mono2_u16_jpeg_lossless_process_14",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject JPEG Lossless Process 14 files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("jpeg_lossless_process_14_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "legacy_jpeg_dcmtk")]
fn validate_command_reports_jpeg_lossless_sv1_decoded_frame_hash_mismatch() {
    let out_dir = unique_temp_dir("validate-jpeg-lossless-sv1-decoded-hash");
    generate_profile(&out_dir, "extended");
    mutate_case_pixel_data(
        &out_dir,
        "classic/sc/mono2_u16_jpeg_lossless_sv1",
        |pixel_data| {
            pixel_data.insert(
                "frame_hashes".to_string(),
                json!(["0000000000000000000000000000000000000000000000000000000000000000"]),
            );
        },
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should reject JPEG Lossless SV1 files whose decoded native hash does not match the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("jpeg_lossless_sv1_decoded_frame_hashes"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_rejects_missing_manifest() {
    let out_dir = unique_temp_dir("validate-missing-manifest");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail without a manifest"
    );
    let stderr = String::from_utf8(output.stderr).expect("validate stderr must be UTF-8");
    assert!(stderr.contains("failed to read manifest"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_corrupted_generated_file() {
    let out_dir = unique_temp_dir("validate-corrupt");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    let mut bytes = fs::read(&dcm_path).expect("generated DICOM should be readable");
    bytes.push(0);
    fs::write(&dcm_path, bytes).expect("generated DICOM should be corruptible");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when generated files drift from the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("files_checked\t3"));
    assert!(stdout.contains("failure\tclassic/sc/mono2_u8_explicit_le/instance.dcm: sha256"));
    let stderr = String::from_utf8(output.stderr).expect("validate stderr must be UTF-8");
    assert!(stderr.contains("validation failed"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_nonzero_part10_preamble() {
    let out_dir = unique_temp_dir("validate-nonzero-preamble");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        bytes[0] = 1;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when the normal Part 10 preamble is non-zero"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("part10_zero_preamble"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_file_meta_information_version() {
    let out_dir = unique_temp_dir("validate-missing-file-meta-version");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0002, 0x0001)
            .expect("generated DICOM should contain File Meta Information Version");
        bytes[offset] = 0x03;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when File Meta Information Version is missing"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("file_meta_information_version"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_dataset_group_0002_after_file_meta() {
    let out_dir = unique_temp_dir("validate-dataset-group-0002");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0008, 0x0016)
            .expect("generated DICOM should start dataset with SOP Class UID");
        bytes[offset] = 0x02;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when a group 0002 element appears in the dataset"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("file_meta_allowed_element"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_inconsistent_high_bit() {
    let out_dir = unique_temp_dir("validate-high-bit");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0028, 0x0102).expect("generated DICOM should contain High Bit");
        let value_offset = offset + 8;
        bytes[value_offset] = 8;
        bytes[value_offset + 1] = 0;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when High Bit does not equal Bits Stored - 1"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("high_bit_consistency"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_standard_type2_attribute() {
    let out_dir = unique_temp_dir("validate-missing-type2");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0010, 0x0010).expect("generated DICOM should contain Patient's Name");
        bytes[offset] = 0x11;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when a standard Type 2 attribute is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("patient_name_type2"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_sc_conversion_type() {
    let out_dir = unique_temp_dir("validate-missing-sc-conversion-type");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0008, 0x0064)
            .expect("generated DICOM should contain Conversion Type");
        bytes[offset] = 0x09;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when SC Conversion Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("sc_conversion_type_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_ct_image_type() {
    let out_dir = unique_temp_dir("validate-missing-ct-image-type");
    generate_profile(&out_dir, "core");
    let dcm_path = out_dir.join("classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0008, 0x0008).expect("generated CT should contain Image Type");
        bytes[offset] = 0x09;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when CT Image Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("ct_image_type_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_mg_positioner_type() {
    let out_dir = unique_temp_dir("validate-missing-mg-positioner-type");
    generate_profile(&out_dir, "core");
    let dcm_path =
        out_dir.join("classic/mg/for_presentation_mono1_u16_12bit_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0018, 0x1508).expect("generated MG should contain Positioner Type");
        bytes[offset] = 0x19;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when MG Positioner Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("mg_positioner_type_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_dx_presentation_lut_shape() {
    let out_dir = unique_temp_dir("validate-missing-dx-presentation-lut-shape");
    generate_profile(&out_dir, "core");
    let dcm_path = out_dir.join("classic/dx/display_shutter_mono2_u16_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x2050, 0x0020)
            .expect("generated DX should contain Presentation LUT Shape");
        bytes[offset] = 0x51;
        bytes[offset + 1] = 0x20;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when DX Presentation LUT Shape is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("dx_presentation_lut_shape_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_us_image_type() {
    let out_dir = unique_temp_dir("validate-missing-us-image-type");
    generate_profile(&out_dir, "core");
    let dcm_path = out_dir.join("classic/us/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0008, 0x0008).expect("generated US should contain Image Type");
        bytes[offset] = 0x09;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when US Image Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("us_image_type_type2"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_cr_body_part_examined() {
    let out_dir = unique_temp_dir("validate-missing-cr-body-part-examined");
    generate_profile(&out_dir, "core");
    let dcm_path = out_dir.join("classic/cr/overlay_modality_voi_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0018, 0x0015)
            .expect("generated CR should contain Body Part Examined");
        bytes[offset] = 0x19;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when CR Body Part Examined is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("cr_body_part_examined_type2"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_mr_scanning_sequence() {
    let out_dir = unique_temp_dir("validate-missing-mr-scanning-sequence");
    generate_profile(&out_dir, "core");
    let dcm_path = out_dir.join("classic/mr/multislice_oblique_explicit_le/slice-001.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0018, 0x0020).expect("generated MR should contain Scanning Sequence");
        bytes[offset] = 0x19;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when MR Scanning Sequence is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("mr_scanning_sequence_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_enhanced_ct_shared_functional_groups() {
    let out_dir = unique_temp_dir("validate-missing-enhanced-ct-shared-fg");
    generate_profile(&out_dir, "extended");
    let dcm_path = out_dir.join("enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x5200, 0x9229)
            .expect("generated Enhanced CT should contain Shared Functional Groups Sequence");
        bytes[offset] = 0x01;
        bytes[offset + 1] = 0x52;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when Enhanced CT Shared Functional Groups Sequence is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("enhanced_ct_shared_functional_groups_sequence_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_enhanced_mr_dimension_organization() {
    let out_dir = unique_temp_dir("validate-missing-enhanced-mr-dimension-organization");
    generate_profile(&out_dir, "extended");
    let dcm_path = out_dir.join("enhanced/mr/multiframe_echo_perframe_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0020, 0x9221)
            .expect("generated Enhanced MR should contain Dimension Organization Sequence");
        bytes[offset + 2] = 0x23;
        bytes[offset + 3] = 0x92;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when Enhanced MR Dimension Organization Sequence is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("enhanced_mr_dimension_organization_sequence_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_rt_structure_set_roi_sequence() {
    let out_dir = unique_temp_dir("validate-missing-rt-structure-set-roi");
    generate_profile(&out_dir, "extended");
    let dcm_path = out_dir.join("non-image/rt/structure_set_single_roi_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x3006, 0x0020)
            .expect("generated RT Structure Set should contain Structure Set ROI Sequence");
        bytes[offset] = 0x07;
        bytes[offset + 1] = 0x30;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when RT Structure Set ROI Sequence is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("rt_structure_set_roi_sequence_type3"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_rt_dose_grid_scaling() {
    let out_dir = unique_temp_dir("validate-missing-rt-dose-grid-scaling");
    generate_profile(&out_dir, "extended");
    let dcm_path = out_dir.join("non-image/rt/dose_grid_u16_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x3004, 0x000E)
            .expect("generated RT Dose should contain Dose Grid Scaling");
        bytes[offset] = 0x05;
        bytes[offset + 1] = 0x30;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when RT Dose Grid Scaling is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("rt_dose_grid_scaling_type1c"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_encapsulated_pdf_mime_type() {
    let out_dir = unique_temp_dir("validate-missing-encapsulated-pdf-mime");
    generate_profile(&out_dir, "extended");
    let dcm_path =
        out_dir.join("non-image/encapsulated-document/pdf_minimal_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0042, 0x0012)
            .expect("generated Encapsulated PDF should contain MIME Type");
        bytes[offset] = 0x43;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when Encapsulated PDF MIME Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("encapsulated_document_mime_type_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

fn generate_smoke(out_dir: &Path) {
    generate_profile(out_dir, "smoke");
}

fn generate_profile(out_dir: &Path, profile: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            profile,
            "--out",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("generate command must run");

    assert!(
        output.status.success(),
        "generate {profile} should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn append_reference_fixture(out_dir: &Path, corrupt_reference: bool) {
    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    let files = manifest
        .get_mut("files")
        .and_then(Value::as_array_mut)
        .expect("manifest files should be an array");
    let source = files
        .first()
        .cloned()
        .expect("smoke manifest should contain a source file");
    let source_path = source
        .get("path")
        .and_then(Value::as_str)
        .expect("source path should be a string")
        .to_string();
    let source_case_id = source
        .get("case_id")
        .and_then(Value::as_str)
        .expect("source case_id should be a string")
        .to_string();
    let source_sop_class_uid = source
        .pointer("/dicom/sop_class_uid")
        .and_then(Value::as_str)
        .expect("source SOP Class UID should be a string")
        .to_string();
    let source_sop_instance_uid = source
        .pointer("/uids/sop_instance_uid")
        .and_then(Value::as_str)
        .expect("source SOP Instance UID should be a string")
        .to_string();
    let source_series_instance_uid = source
        .pointer("/uids/series_instance_uid")
        .and_then(Value::as_str)
        .expect("source Series Instance UID should be a string")
        .to_string();

    let derived_path = "derived/test/non_image_reference_explicit_le/instance.dcm";
    let derived_file_path = out_dir.join(derived_path);
    fs::create_dir_all(
        derived_file_path
            .parent()
            .expect("derived file should have a parent directory"),
    )
    .expect("derived fixture directory should be creatable");
    fs::copy(out_dir.join(&source_path), &derived_file_path)
        .expect("derived fixture DICOM should be copied from the generated source");

    let mut derived = source;
    let derived_object = derived
        .as_object_mut()
        .expect("manifest file entries should be objects");
    derived_object.insert(
        "case_id".to_string(),
        Value::String("derived/test/non_image_reference_explicit_le".to_string()),
    );
    derived_object.insert("path".to_string(), Value::String(derived_path.to_string()));
    derived_object.insert("image".to_string(), Value::Null);
    derived_object.insert("pixel_data".to_string(), Value::Null);
    derived_object.insert(
        "references".to_string(),
        json!([
            {
                "relationship": "source_image",
                "source_case_id": source_case_id,
                "source_path": source_path,
                "sop_class_uid": source_sop_class_uid,
                "sop_instance_uid": if corrupt_reference {
                    "2.25.999999999"
                } else {
                    source_sop_instance_uid.as_str()
                },
                "series_instance_uid": source_series_instance_uid,
                "frame_numbers": [1]
            }
        ]),
    );
    files.push(derived);

    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");
}

fn mutate_first_file_pixel_data(
    out_dir: &Path,
    mutate: impl FnOnce(&mut serde_json::Map<String, Value>),
) {
    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    let files = manifest
        .get_mut("files")
        .and_then(Value::as_array_mut)
        .expect("manifest files should be an array");
    let pixel_data = files
        .first_mut()
        .and_then(|file| file.get_mut("pixel_data"))
        .and_then(Value::as_object_mut)
        .expect("first generated file should have pixel_data metadata");
    mutate(pixel_data);
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");
}

fn mutate_case_pixel_data(
    out_dir: &Path,
    case_id: &str,
    mutate: impl FnOnce(&mut serde_json::Map<String, Value>),
) {
    let manifest_path = out_dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    let files = manifest
        .get_mut("files")
        .and_then(Value::as_array_mut)
        .expect("manifest files should be an array");
    let pixel_data = files
        .iter_mut()
        .find(|file| file.get("case_id").and_then(Value::as_str) == Some(case_id))
        .and_then(|file| file.get_mut("pixel_data"))
        .and_then(Value::as_object_mut)
        .expect("case should have pixel_data metadata");
    mutate(pixel_data);
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");
}

fn mutate_dicom(path: &Path, mutate: impl FnOnce(&mut Vec<u8>)) {
    let mut bytes = fs::read(path).expect("generated DICOM should be readable");
    mutate(&mut bytes);
    fs::write(path, bytes).expect("generated DICOM should be writable");
}

fn find_tag(bytes: &[u8], group: u16, element: u16) -> Option<usize> {
    let group = group.to_le_bytes();
    let element = element.to_le_bytes();
    bytes
        .windows(4)
        .position(|window| window == [group[0], group[1], element[0], element[1]])
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
