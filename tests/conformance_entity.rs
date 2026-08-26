#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn entity_adapter_uses_file_list_and_normalizes_consistency_findings() {
    let root = temp_dir("entity with spaces");
    let generated = root.join("generated root");
    generate_smoke(&generated);
    let primary = fake_tool(&root, "primary", "exit 0");
    let entity = fake_tool(
        &root,
        "entity",
        "if [ \"$1\" = \"--version\" ]; then echo entity-1; exit 0; fi\n\
         test \"$1\" = \"-f\" || exit 8\n\
         test \"$(wc -l < \"$2\" | tr -d ' ')\" = \"3\" || exit 9\n\
         grep -q 'generated root' \"$2\" || exit 10\n\
         echo 'Error - SeriesInstanceUID reused for different StudyInstanceUID'\n\
         echo 'Warning - PatientName inconsistent for PatientID' >&2",
    );
    let config = root.join("validators.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("primary", "primary_iod_validator", &primary),
                adapter("entity", "entity_validator", &entity)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let evidence_root = root.join("evidence");
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(&evidence_root)
        .args(["--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: Value =
        serde_json::from_slice(&fs::read(evidence_root.join("conformance-run.json")).unwrap())
            .unwrap();
    assert_eq!(evidence["entity"]["status"], "completed");
    assert_eq!(evidence["entity"]["findings"][0]["severity"], "error");
    assert_eq!(evidence["entity"]["findings"][1]["severity"], "warning");
    assert!(evidence_root.join("entity/files.txt").is_file());
    assert_eq!(
        evidence["entity"]["stdout"]["sha256"],
        dicom_test_suite::sha256_hex(
            &fs::read(evidence_root.join("entity/dcentvfy.stdout")).unwrap()
        )
    );
}

fn adapter(id: &str, role: &str, path: &Path) -> Value {
    json!({
        "id": id,
        "role": role,
        "executable": path,
        "arguments": [],
        "version_arguments": ["--version"],
        "timeout_seconds": 2,
        "required": true,
        "platforms": ["macos"],
        "capabilities": ["test"]
    })
}

fn generate_smoke(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "smoke", "--out"])
        .arg(root)
        .output()
        .unwrap();
    assert!(output.status.success());
}

fn fake_tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dts-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
