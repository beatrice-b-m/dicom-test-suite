use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn generate_smoke(out_dir: &Path) {
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
