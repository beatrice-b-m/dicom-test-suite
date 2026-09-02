#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn parser_records_success_failure_timeout_and_unsupported_syntax() {
    let root = temp_dir("parser");
    let generated = root.join("generated");
    generate_smoke(&generated);
    let primary = fake_tool(&root, "primary", "exit 0");
    let entity = fake_tool(&root, "entity", "exit 0");

    let clean = fake_tool(&root, "clean-parser", "echo parsed");
    let clean_evidence = run(&root, &generated, &primary, &entity, &clean, 2, "clean");
    for instance in clean_evidence["instances"].as_array().unwrap() {
        let parser = parser_result(instance);
        assert_eq!(parser["status"], "completed");
        assert_eq!(parser["role"], "independent_parser");
    }

    let failed = fake_tool(&root, "failed-parser", "echo parser-crashed >&2\nexit 4");
    let failed_evidence = run(&root, &generated, &primary, &entity, &failed, 2, "failed");
    assert_eq!(
        parser_result(&failed_evidence["instances"][0])["status"],
        "tool_failure"
    );

    let unsupported = fake_tool(
        &root,
        "unsupported-parser",
        "echo 'Unsupported transfer syntax' >&2\nexit 2",
    );
    let unsupported_evidence = run(
        &root,
        &generated,
        &primary,
        &entity,
        &unsupported,
        2,
        "unsupported",
    );
    assert_eq!(
        parser_result(&unsupported_evidence["instances"][0])["status"],
        "unsupported"
    );

    let slow = fake_tool(
        &root,
        "slow-parser",
        "if [ \"$1\" = \"--version\" ]; then exit 0; fi\nsleep 2",
    );
    let timeout_evidence = run(&root, &generated, &primary, &entity, &slow, 1, "timeout");
    assert_eq!(
        parser_result(&timeout_evidence["instances"][0])["status"],
        "timeout"
    );
}

fn parser_result(instance: &Value) -> &Value {
    instance["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["role"] == "independent_parser")
        .unwrap()
}

fn run(
    root: &Path,
    generated: &Path,
    primary: &Path,
    entity: &Path,
    parser: &Path,
    timeout: u64,
    label: &str,
) -> Value {
    let config = root.join(format!("{label}-validators.json"));
    fs::write(
        &config,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("primary", "primary_iod_validator", primary, 2),
                adapter("entity", "entity_validator", entity, 2),
                adapter("parser", "independent_parser", parser, timeout)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let evidence = root.join(format!("{label}-evidence"));
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(generated)
        .args(["--out"])
        .arg(&evidence)
        .args(["--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(evidence.join("conformance-run.json")).unwrap()).unwrap()
}

fn adapter(id: &str, role: &str, path: &Path, timeout: u64) -> Value {
    json!({
        "id": id, "role": role, "executable": path,
        "arguments": ["{input}"], "version_arguments": ["--version"],
        "timeout_seconds": timeout, "required": true,
        "platforms": ["macos"], "capabilities": ["test"]
    })
}

fn generate_smoke(root: &Path) {
    assert!(
        Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(["generate", "--profile", "smoke", "--out"])
            .arg(root)
            .status()
            .unwrap()
            .success()
    );
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
