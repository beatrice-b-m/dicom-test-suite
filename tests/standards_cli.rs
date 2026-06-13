use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::json;

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

#[test]
fn standards_gaps_reports_registry_evidence_gaps_for_profile() {
    let registry_path = unique_temp_file("standards-gaps-registry.json");
    fs::write(
        &registry_path,
        serde_json::to_string_pretty(&json!({
            "case_registry_schema_version": "0.1.0",
            "cases": [
                {
                    "case_id": "classic/sc/no_evidence_explicit_le",
                    "status": "implemented",
                    "profiles": ["core"],
                    "skip": null,
                    "standards_evidence": []
                },
                {
                    "case_id": "classic/sc/blocked_explicit_le",
                    "status": "blocked",
                    "profiles": ["core"],
                    "skip": {
                        "reason_code": "standards_gap",
                        "message": "waiting on standards evidence"
                    },
                    "standards_evidence": [
                        {
                            "source": "dicom-standard-kb",
                            "edition": "2026b",
                            "query": "lookup_uid ExplicitVRLittleEndian",
                            "covered": true
                        }
                    ]
                },
                {
                    "case_id": "classic/sc/source_note_explicit_le",
                    "status": "implemented",
                    "profiles": ["core"],
                    "skip": null,
                    "standards_evidence": [
                        {
                            "source": "local-source-note",
                            "query": "PS3.5 UID root",
                            "covered": false
                        }
                    ]
                },
                {
                    "case_id": "classic/sc/extended_only_explicit_le",
                    "status": "implemented",
                    "profiles": ["extended"],
                    "skip": null,
                    "standards_evidence": []
                }
            ]
        }))
        .expect("temporary registry should serialize"),
    )
    .expect("temporary registry should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "standards",
            "gaps",
            "--profile",
            "core",
            "--registry",
            registry_path
                .to_str()
                .expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("standards gaps command must run");

    assert!(
        output.status.success(),
        "standards gaps should accept registry: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("standards gaps stdout should be UTF-8");
    assert!(stdout.starts_with("case_id\tstatus\tprofiles\tgap_kind\treason\n"));
    assert!(stdout.contains(
        "classic/sc/no_evidence_explicit_le\timplemented\tcore\tmissing_standards_evidence"
    ));
    assert!(
        stdout.contains("classic/sc/blocked_explicit_le\tblocked\tcore\tblocked\tstandards_gap")
    );
    assert!(stdout.contains(
        "classic/sc/source_note_explicit_le\timplemented\tcore\tincomplete_standards_evidence"
    ));
    assert!(stdout.contains(
        "classic/sc/source_note_explicit_le\timplemented\tcore\tuncovered_standards_evidence"
    ));
    assert!(
        stdout
            .contains("classic/sc/source_note_explicit_le\timplemented\tcore\tsource_note_backed")
    );
    assert!(!stdout.contains("classic/sc/extended_only_explicit_le"));

    fs::remove_file(registry_path).expect("temporary registry should be removable");
}

#[test]
fn standards_gaps_requires_profile() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["standards", "gaps"])
        .output()
        .expect("standards gaps command must run");

    assert!(
        !output.status.success(),
        "standards gaps should require a profile"
    );
    let stderr = String::from_utf8(output.stderr).expect("standards gaps stderr should be UTF-8");
    assert!(stderr.contains("standards gaps requires --profile"));
}

#[test]
fn standards_verify_kb_reports_unavailable_without_runtime_mcp() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["standards", "verify-kb", "--edition", "2026b"])
        .output()
        .expect("standards verify-kb command must run");

    assert!(
        output.status.success(),
        "verify-kb should return an intentional unavailable status: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("verify-kb stdout should be UTF-8");
    assert!(stdout.contains("status\tunavailable"));
    assert!(stdout.contains("edition\t2026b"));
    assert!(stdout.contains("standalone CLI cannot access the dicom-standard-kb MCP server"));
}

#[test]
fn standards_verify_kb_rejects_unsupported_edition() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["standards", "verify-kb", "--edition", "2025d"])
        .output()
        .expect("standards verify-kb command must run");

    assert!(
        !output.status.success(),
        "verify-kb should reject unsupported editions"
    );
    let stderr = String::from_utf8(output.stderr).expect("verify-kb stderr should be UTF-8");
    assert!(stderr.contains("unsupported standards edition 2025d"));
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
