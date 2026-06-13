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
    assert!(stdout.contains("files_written\t3"));

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
        Some(3)
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
        manifest
            .pointer("/skipped_cases")
            .and_then(Value::as_array)
            .is_some_and(|cases| {
                cases.iter().any(|case| {
                    case.get("case_id").and_then(Value::as_str)
                        == Some("classic/ct/mono2_i16_rescale_12bit_explicit_le")
                })
            }),
        "manifest should still report planned core cases without generators"
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
    manifest
        .pointer(pointer)
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
