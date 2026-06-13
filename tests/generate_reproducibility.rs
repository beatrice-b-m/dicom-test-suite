use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const FIRST_SMOKE_PATH: &str = "classic/sc/mono2_u8_explicit_le/instance.dcm";

#[test]
fn smoke_generation_is_byte_stable_across_two_output_roots() {
    let first_out = unique_temp_dir("generate-reproducibility-a");
    let second_out = unique_temp_dir("generate-reproducibility-b");

    let first_manifest = run_smoke_generate(&first_out);
    let second_manifest = run_smoke_generate(&second_out);

    let first_dcm = fs::read(first_out.join(FIRST_SMOKE_PATH))
        .expect("first generated DICOM file should be readable");
    let second_dcm = fs::read(second_out.join(FIRST_SMOKE_PATH))
        .expect("second generated DICOM file should be readable");
    assert_eq!(
        first_dcm, second_dcm,
        "generated DICOM bytes should be stable for the same seed"
    );

    assert_eq!(
        first_manifest.pointer("/files"),
        second_manifest.pointer("/files"),
        "manifest file metadata should be stable across output roots"
    );
    assert_eq!(
        first_manifest.pointer("/skipped_cases"),
        second_manifest.pointer("/skipped_cases"),
        "skipped-case metadata should be stable across output roots"
    );
    assert_eq!(
        first_manifest.pointer("/files/0/sha256"),
        Some(&Value::String(dicom_test_suite::sha256_hex(&first_dcm))),
        "manifest hash should match generated DICOM bytes"
    );
    assert_eq!(
        first_manifest.pointer("/files/0/uids"),
        second_manifest.pointer("/files/0/uids"),
        "deterministic UID metadata should match across runs"
    );
    assert_eq!(
        first_manifest, second_manifest,
        "entire manifest should be stable because it only stores relative paths and deterministic metadata"
    );

    fs::remove_dir_all(first_out).expect("first temporary output root should be removable");
    fs::remove_dir_all(second_out).expect("second temporary output root should be removable");
}

fn run_smoke_generate(out_dir: &Path) -> Value {
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

    serde_json::from_str(
        &fs::read_to_string(out_dir.join("manifest.json")).expect("manifest should be readable"),
    )
    .expect("manifest should parse")
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
