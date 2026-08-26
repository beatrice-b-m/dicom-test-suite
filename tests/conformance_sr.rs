#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn sr_adapter_runs_only_for_supported_sop_classes_and_hashes_its_classpath() {
    let root = temp_dir("pixelmed");
    let generated = root.join("generated");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["generate", "--profile", "extended", "--out"])
            .arg(&generated)
            .args(["--seed", "1"])
            .status()
            .unwrap()
            .success()
    );
    let quiet = fake_tool(&root, "quiet", "exit 0");
    let pixelmed = fake_tool(
        &root,
        "pixelmed",
        r#"case "$2" in
  *basic_text*) echo 'Warning: Root Content Item has no Template Identifier';;
  *key_object*) echo 'Error: Template 2010 forbids Concept Name';;
esac"#,
    );
    let artifacts = root.join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(artifacts.join("pixelmed.jar"), b"fake-pixelmed-jar").unwrap();
    let config = root.join("validators.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("primary", "primary_iod_validator", &quiet),
                adapter("entity", "entity_validator", &quiet),
                adapter("parser", "independent_parser", &quiet),
                {
                    "id": "fake-pixelmed",
                    "role": "sr_validator",
                    "executable": pixelmed,
                    "arguments": ["{classpath}", "{input}"],
                    "version_arguments": ["--version"],
                    "timeout_seconds": 2,
                    "required": false,
                    "platforms": ["macos"],
                    "artifact_root_env": "DTS_TEST_PIXELMED_HOME",
                    "classpath": ["pixelmed.jar"],
                    "supported_sop_class_uids": [
                        "1.2.840.10008.5.1.4.1.1.88.11",
                        "1.2.840.10008.5.1.4.1.1.88.33",
                        "1.2.840.10008.5.1.4.1.1.88.59"
                    ],
                    "capabilities": ["sr_validation"]
                }
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
        .env("DTS_TEST_PIXELMED_HOME", &artifacts)
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
    let tool = evidence["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "fake-pixelmed")
        .unwrap();
    assert_eq!(tool["status"], "available");
    assert_eq!(tool["artifacts"].as_array().unwrap().len(), 1);
    assert_ne!(tool["sha256"], tool["executable_sha256"]);

    let sr_results = evidence["instances"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|instance| {
            instance["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|result| result["role"] == "sr_validator")
                .map(|result| (instance, result))
        })
        .collect::<Vec<_>>();
    assert_eq!(sr_results.len(), 3);
    assert!(
        sr_results
            .iter()
            .any(|(_, result)| result["findings"][0]["severity"] == "warning")
    );
    assert!(
        sr_results
            .iter()
            .any(|(_, result)| result["findings"][0]["severity"] == "error")
    );
    for (_, result) in sr_results {
        for stream in ["stdout", "stderr"] {
            let relative = result[stream]["path"].as_str().unwrap();
            let bytes = fs::read(evidence_root.join(relative)).unwrap();
            assert_eq!(
                result[stream]["sha256"],
                dicom_test_suite::sha256_hex(&bytes)
            );
        }
    }
}

fn adapter(id: &str, role: &str, path: &Path) -> Value {
    json!({
        "id": id, "role": role, "executable": path, "arguments": [],
        "version_arguments": ["--version"], "timeout_seconds": 2,
        "required": true, "platforms": ["macos"], "capabilities": ["test"]
    })
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
    let root = std::env::temp_dir().join(format!("dts-conformance-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
