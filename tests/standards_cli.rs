use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn standards_check_lock_accepts_committed_lock_with_documented_warnings() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["standards", "check-lock"])
        .output()
        .expect("standards check-lock command must run");

    assert!(
        output.status.success(),
        "check-lock should accept committed lock: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("check-lock stdout should be UTF-8");
    assert!(stdout.contains("status\tok"));
    assert!(stdout.contains("dicom_base_edition\t2026b"));
    assert!(stdout.contains("include_final_text_after_base\tfalse"));
    assert!(stdout.contains("kb_db_edition\t2026b"));
    assert!(stdout.contains(
        "kb_source_manifest_sha256\t9959bee76fd293c7eda3fc81ce2ced7528612faa1b2df28cccd01504a83f54b0"
    ));
    assert!(stdout.contains("warning\tdicom_standard_kb.commit unavailable"));
    assert!(stdout.contains("warning\tdicom_standard_kb.db_sha256 unavailable"));
    assert!(stdout.contains("source_artifacts\t5"));
    assert!(stdout.contains("verification_queries\t3"));
}

#[test]
fn standards_check_lock_rejects_malformed_lock() {
    let lock_path = unique_temp_file("malformed-standards-lock.json");
    fs::write(&lock_path, "{}").expect("temporary lock should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "standards",
            "check-lock",
            "--lock",
            lock_path.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("standards check-lock command must run");

    assert!(
        !output.status.success(),
        "check-lock should reject malformed lock"
    );
    let stderr = String::from_utf8(output.stderr).expect("check-lock stderr should be UTF-8");
    assert!(stderr.contains("invalid standards lock metadata"));
    assert!(stderr.contains("/schema_version must be a string"));

    fs::remove_file(lock_path).expect("temporary lock should be removable");
}

fn unique_temp_file(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{name}-{}-{nonce}",
        std::process::id()
    ))
}
