#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

fn compile_schema(path: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap()
}

#[test]
fn check_tools_machine_result_is_clean_and_schema_bound() {
    let root = temp_dir("machine");
    let available = fake_tool(&root, "available", "echo tool-1.2");
    let config = config(
        &root,
        vec![adapter("available", available.to_str().unwrap(), 1)],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "check-tools", "--config"])
        .arg(config)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(compile_schema("schemas/cli-success-envelope.schema.json").is_valid(&envelope));
    assert!(compile_schema("schemas/conformance-result.schema.json").is_valid(&envelope["result"]));
    assert_eq!(envelope["command"], "conformance check-tools");
    assert_eq!(
        envelope["result"]["outcome"]["tools"][0]["status"],
        "available"
    );
}

#[test]
fn check_tools_distinguishes_available_absent_and_misconfigured() {
    let root = temp_dir("states");
    let available = fake_tool(&root, "available", "echo tool-1.2");
    let config = config(
        &root,
        vec![
            adapter("available", available.to_str().unwrap(), 1),
            adapter("absent", "definitely-not-a-real-dicom-tool", 1),
            adapter("bad-path", "/not/a/real/tool", 1),
        ],
    );
    let report = run_check(&config);
    assert_eq!(report["tools"][0]["status"], "available");
    assert_eq!(report["tools"][0]["version_output"], "tool-1.2");
    assert_eq!(report["tools"][1]["status"], "absent");
    assert_eq!(report["tools"][2]["status"], "misconfigured");
}

#[test]
fn check_tools_captures_nonzero_version_and_versionless_commands() {
    let root = temp_dir("version-probes");
    let nonzero = fake_tool(&root, "nonzero", "echo version-on-stderr >&2\nexit 9");
    let versionless = fake_tool(&root, "versionless", "exit 0");
    let config = config(
        &root,
        vec![
            adapter("nonzero", nonzero.to_str().unwrap(), 1),
            adapter("versionless", versionless.to_str().unwrap(), 1),
        ],
    );
    let report = run_check(&config);
    assert_eq!(report["tools"][0]["status"], "available");
    assert_eq!(report["tools"][0]["version_exit_code"], 9);
    assert_eq!(report["tools"][0]["version_output"], "version-on-stderr");
    assert_eq!(report["tools"][1]["status"], "available");
    assert!(report["tools"][1]["version_output"].is_null());
}

#[test]
fn check_tools_reports_timeout_and_fingerprint_mismatch() {
    let root = temp_dir("timeout");
    let slow = fake_tool(&root, "slow", "sleep 2");
    let config = config(
        &root,
        vec![adapter("dcmtk-dcmdump", slow.to_str().unwrap(), 0)],
    );
    let report = run_check(&config);
    assert_eq!(report["tools"][0]["status"], "timeout");
    assert_eq!(report["tools"][0]["lock_status"], "mismatched");
    assert_eq!(
        report["tools"][0]["sha256"],
        synth_dicom_gen::sha256_hex(&fs::read(slow).unwrap())
    );
}

fn adapter(id: &str, executable: &str, timeout_seconds: u64) -> Value {
    json!({
        "id": id,
        "role": "independent_parser",
        "executable": executable,
        "arguments": ["{input}"],
        "version_arguments": ["--version"],
        "timeout_seconds": timeout_seconds,
        "required": true,
        "platforms": ["macos"],
        "capabilities": ["part10_parse"]
    })
}

fn config(root: &Path, adapters: Vec<Value>) -> PathBuf {
    let path = root.join("validators.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({"schema_version": "0.1.0", "adapters": adapters})).unwrap(),
    )
    .unwrap();
    path
}

fn fake_tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn run_check(config: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "check-tools", "--config"])
        .arg(config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dts-conformance-{label}-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}
