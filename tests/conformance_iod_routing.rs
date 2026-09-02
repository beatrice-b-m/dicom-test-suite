#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const ROUTED_CASE: &str = "classic/sc/mono2_u8_explicit_le";

#[test]
fn case_specific_primary_validator_uses_locked_environment_runtime_and_artifacts() {
    let root = temp_dir("routing");
    let generated = root.join("generated");
    generate_smoke(&generated);
    let default = fake_tool(&root, "default", "echo 'Info - default primary'");
    let routed = fake_tool(&root, "routed", "echo 'Info - routed primary'");
    let quiet = fake_tool(&root, "quiet", "exit 0");
    let artifact_root = root.join("artifacts");
    fs::create_dir_all(&artifact_root).unwrap();
    fs::write(artifact_root.join("definition.lock"), b"locked-definition").unwrap();
    let config = root.join("validators.json");
    let mut routed_adapter = adapter(
        "routed-primary",
        "primary_iod_validator",
        Path::new("configured-through-environment"),
    );
    routed_adapter["executable_env"] = json!("DTS_TEST_IOD_EXECUTABLE");
    routed_adapter["supported_case_ids"] = json!([ROUTED_CASE]);
    routed_adapter["artifacts"] = json!([{
        "root_env": "DTS_TEST_IOD_ARTIFACT_ROOT",
        "path": "definition.lock"
    }]);
    let mut secondary_adapter =
        adapter("registration-secondary", "secondary_iod_validator", &routed);
    secondary_adapter["supported_case_ids"] = json!([ROUTED_CASE]);
    fs::write(
        &config,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("default-primary", "primary_iod_validator", &default),
                routed_adapter,
                secondary_adapter,
                adapter("entity", "entity_validator", &quiet),
                adapter("parser", "independent_parser", &quiet)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let evidence_root = root.join("evidence");
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(&evidence_root)
        .args(["--config"])
        .arg(&config)
        .env("DTS_TEST_IOD_EXECUTABLE", &routed)
        .env("DTS_TEST_IOD_ARTIFACT_ROOT", &artifact_root)
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
    for instance in evidence["instances"].as_array().unwrap() {
        let expected = if instance["case_id"] == ROUTED_CASE {
            "routed-primary"
        } else {
            "default-primary"
        };
        assert_eq!(instance["results"][0]["adapter_id"], expected);
        let secondary = instance["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|result| result["role"] == "secondary_iod_validator");
        if instance["case_id"] == ROUTED_CASE {
            assert_eq!(secondary.unwrap()["adapter_id"], "registration-secondary");
        } else {
            assert!(secondary.is_none());
        }
    }
    let routed_tool = evidence["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "routed-primary")
        .unwrap();
    assert_eq!(routed_tool["status"], "available");
    assert_eq!(routed_tool["executable"], routed.display().to_string());
    assert_eq!(routed_tool["artifacts"].as_array().unwrap().len(), 1);
    assert_ne!(routed_tool["sha256"], routed_tool["executable_sha256"]);

    let mut duplicate: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    let mut second = duplicate["adapters"][1].clone();
    second["id"] = json!("second-routed-primary");
    duplicate["adapters"].as_array_mut().unwrap().push(second);
    let duplicate_config = root.join("duplicate-validators.json");
    fs::write(&duplicate_config, serde_json::to_vec(&duplicate).unwrap()).unwrap();
    let duplicate_output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(root.join("duplicate-evidence"))
        .args(["--config"])
        .arg(&duplicate_config)
        .env("DTS_TEST_IOD_EXECUTABLE", &routed)
        .env("DTS_TEST_IOD_ARTIFACT_ROOT", &artifact_root)
        .output()
        .unwrap();
    assert!(!duplicate_output.status.success());
    assert!(
        String::from_utf8_lossy(&duplicate_output.stderr)
            .contains("multiple primary IOD validators are configured")
    );
}

fn adapter(id: &str, role: &str, executable: &Path) -> Value {
    json!({
        "id": id,
        "role": role,
        "executable": executable,
        "arguments": ["{input}"],
        "version_arguments": ["--version"],
        "timeout_seconds": 2,
        "required": true,
        "platforms": ["macos"],
        "capabilities": ["iod_validation"]
    })
}

fn generate_smoke(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "smoke", "--out"])
        .arg(root)
        .args(["--seed", "1"])
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
    let path = std::env::temp_dir().join(format!(
        "dts-conformance-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
