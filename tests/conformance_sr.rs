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
    let manifest_path = generated.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let comprehensive_sr = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == "derived/sr/comprehensive_measurement_explicit_le")
        .unwrap();
    comprehensive_sr["dicom"]["sop_class_uid"] = json!("1.2.840.10008.5.1.4.1.1.88.34");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
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
                    "id": "pixelmed-sr-validator",
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
                        "1.2.840.10008.5.1.4.1.1.88.34",
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
        .find(|tool| tool["adapter_id"] == "pixelmed-sr-validator")
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
    assert_eq!(sr_results.len(), 4);
    assert!(sr_results.iter().any(|(instance, _)| {
        instance["case_id"] == "derived/sr/comprehensive_measurement_explicit_le"
            && instance["sop_class_uid"] == "1.2.840.10008.5.1.4.1.1.88.34"
    }));
    let (_, tid1500_result) = sr_results
        .iter()
        .find(|(instance, _)| {
            instance["case_id"] == "derived/sr/tid1500_ct_measurement_report"
                && instance["sop_class_uid"] == "1.2.840.10008.5.1.4.1.1.88.34"
        })
        .expect("promoted TID 1500 case must route through PixelMed");
    assert_eq!(tid1500_result["status"], "completed");
    assert_eq!(tid1500_result["findings"], json!([]));
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

    let mut incomplete = evidence.clone();
    for tool in incomplete["tools"].as_array_mut().unwrap() {
        if tool["required"] == true || tool["role"] == "sr_validator" {
            tool["lock_status"] = json!("matched");
        }
    }
    let sr_instance = incomplete["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["case_id"] == "derived/sr/key_object_selection_explicit_le")
        .unwrap();
    sr_instance["results"]
        .as_array_mut()
        .unwrap()
        .retain(|result| result["role"] != "sr_validator");
    fs::write(
        evidence_root.join("conformance-run.json"),
        serde_json::to_vec_pretty(&incomplete).unwrap(),
    )
    .unwrap();
    let allowlist = root.join("accepted-findings.json");
    fs::write(
        &allowlist,
        b"{\"schema_version\":\"0.1.0\",\"findings\":[]}",
    )
    .unwrap();
    let verify = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["conformance", "verify"])
        .arg(&evidence_root)
        .args(["--allowlist"])
        .arg(&allowlist)
        .output()
        .unwrap();
    assert!(!verify.status.success());
    assert!(String::from_utf8_lossy(&verify.stdout).contains("SR validation incomplete"));

    let mut optional = evidence.clone();
    optional["instances"]
        .as_array_mut()
        .unwrap()
        .retain(|instance| instance["case_id"] != "derived/sr/tid1500_ct_measurement_report");
    for instance in optional["instances"].as_array_mut().unwrap() {
        instance["results"]
            .as_array_mut()
            .unwrap()
            .retain(|result| result["role"] != "sr_validator");
    }
    for tool in optional["tools"].as_array_mut().unwrap() {
        if tool["required"] == true {
            tool["lock_status"] = json!("matched");
        }
        if tool["role"] == "sr_validator" {
            tool["status"] = json!("absent");
            tool["lock_status"] = json!("unavailable");
            tool["executable"] = Value::Null;
            tool["sha256"] = Value::Null;
        }
    }
    fs::write(
        evidence_root.join("conformance-run.json"),
        serde_json::to_vec_pretty(&optional).unwrap(),
    )
    .unwrap();
    let verify = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["conformance", "verify"])
        .arg(&evidence_root)
        .args(["--allowlist"])
        .arg(&allowlist)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&verify.stdout);
    assert!(!stdout.contains("SR validation incomplete"));
    assert!(!stdout.contains("required PixelMed SR validator"));

    let mut required = evidence.clone();
    for tool in required["tools"].as_array_mut().unwrap() {
        if tool["required"] == true || tool["role"] == "sr_validator" {
            tool["lock_status"] = json!("matched");
        }
    }
    let required_instance = required["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["case_id"] == "derived/sr/comprehensive_measurement_explicit_le")
        .unwrap();
    required_instance["case_id"] = json!("derived/sr/tid1500_ct_measurement_report");
    required_instance["sop_class_uid"] = json!("1.2.840.10008.5.1.4.1.1.88.34");
    required_instance["results"]
        .as_array_mut()
        .unwrap()
        .retain(|result| result["role"] != "sr_validator");
    let pixelmed_tool = required["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["adapter_id"] == "pixelmed-sr-validator")
        .unwrap();
    pixelmed_tool["status"] = json!("absent");
    pixelmed_tool["lock_status"] = json!("unavailable");
    pixelmed_tool["executable"] = Value::Null;
    pixelmed_tool["sha256"] = Value::Null;
    fs::write(
        evidence_root.join("conformance-run.json"),
        serde_json::to_vec_pretty(&required).unwrap(),
    )
    .unwrap();
    let verify = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["conformance", "verify"])
        .arg(&evidence_root)
        .args(["--allowlist"])
        .arg(&allowlist)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&verify.stdout);
    assert!(!verify.status.success());
    assert!(stdout.contains("required PixelMed SR validator is unavailable"));
    assert!(stdout.contains("required PixelMed SR validator is unlocked"));
    assert!(stdout.contains("required PixelMed SR validation incomplete"));
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
