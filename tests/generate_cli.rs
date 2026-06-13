use std::fs;
use std::path::PathBuf;
use std::process::Command;

use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::Value;

#[test]
fn generate_command_writes_first_smoke_part10_file_and_manifest() {
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
    assert!(stdout.contains("files_written\t1"));
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
        Some(1)
    );
    let file_entry = manifest
        .pointer("/files/0")
        .and_then(Value::as_object)
        .expect("manifest should describe the generated file");
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
            .is_some_and(|cases| {
                cases.len() == 2
                    && cases.iter().all(|case| {
                        case.get("case_id").and_then(Value::as_str)
                            != Some("classic/sc/mono2_u8_explicit_le")
                    })
            }),
        "manifest should skip only unimplemented smoke cases"
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
