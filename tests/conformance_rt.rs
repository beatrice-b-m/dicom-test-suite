#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const PLAN_CASE_ID: &str = "non-image/rt/plan_linked";
const IMAGE_CASE_ID: &str = "non-image/rt/image_linked";
const ADAPTER_ID: &str = "pydicom-dicom-validator-rt";

#[test]
fn linked_rt_cases_require_clean_locked_additive_secondary_iod_results() {
    for (label, case_id) in [("plan", PLAN_CASE_ID), ("image", IMAGE_CASE_ID)] {
        let fixture = Fixture::new(label, case_id);
        assert!(
            fixture.verify().status.success(),
            "clean locked additive result must pass for {case_id}"
        );

        let mut absent = fixture.baseline.clone();
        absent["tools"]
            .as_array_mut()
            .unwrap()
            .retain(|tool| tool["adapter_id"] != ADAPTER_ID);
        fixture.write_evidence(&absent);
        fixture.assert_failure("required linked RT secondary IOD validator is unavailable");

        let mut unavailable = fixture.baseline.clone();
        secondary_tool_mut(&mut unavailable)["status"] = json!("absent");
        fixture.write_evidence(&unavailable);
        fixture.assert_failure("required linked RT secondary IOD validator is unavailable");

        let mut unlocked = fixture.baseline.clone();
        secondary_tool_mut(&mut unlocked)["lock_status"] = json!("mismatched");
        fixture.write_evidence(&unlocked);
        fixture.assert_failure("required linked RT secondary IOD validator is unlocked");

        let mut incomplete = fixture.baseline.clone();
        incomplete["instances"][0]["results"]
            .as_array_mut()
            .unwrap()
            .retain(|result| result["adapter_id"] != ADAPTER_ID);
        fixture.write_evidence(&incomplete);
        fixture.assert_failure("required linked RT secondary IOD validation incomplete");

        let mut nonzero = fixture.baseline.clone();
        secondary_result_mut(&mut nonzero)["exit_code"] = json!(1);
        fixture.write_evidence(&nonzero);
        fixture.assert_failure(
            "required linked RT secondary IOD validation did not exit successfully",
        );

        let mut error = fixture.baseline.clone();
        let message = "Error: qualified RT secondary rejected the instance";
        secondary_result_mut(&mut error)["findings"] = json!([{
            "severity": "error",
            "rule_id": null,
            "message": message,
            "message_fingerprint": dicom_test_suite::sha256_hex(message.as_bytes()),
            "dicom_path": null,
            "disposition": "unresolved"
        }]);
        fixture.write_evidence(&error);
        fixture
            .assert_failure("required linked RT secondary IOD validation reported error findings");

        fixture.write_evidence(&fixture.baseline);
        assert!(
            fixture.verify().status.success(),
            "restore failed for {case_id}"
        );
    }
}

fn secondary_tool_mut(evidence: &mut Value) -> &mut Value {
    evidence["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["adapter_id"] == ADAPTER_ID)
        .unwrap()
}

fn secondary_result_mut(evidence: &mut Value) -> &mut Value {
    evidence["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|result| result["adapter_id"] == ADAPTER_ID)
        .unwrap()
}

struct Fixture {
    evidence: PathBuf,
    allowlist: PathBuf,
    baseline: Value,
}

impl Fixture {
    fn new(label: &str, case_id: &str) -> Self {
        let root = temp_dir(label);
        let generated = root.join("generated");
        generate_smoke(&generated);
        let quiet = fake_tool(&root, "quiet", "exit 0");
        let config = root.join("validators.json");
        fs::write(
            &config,
            serde_json::to_vec(&json!({
                "schema_version": "0.1.0",
                "adapters": [
                    adapter("primary", "primary_iod_validator", &quiet),
                    adapter("entity", "entity_validator", &quiet),
                    adapter("parser", "independent_parser", &quiet)
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let evidence = root.join("evidence");
        let run = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["conformance", "run"])
            .arg(&generated)
            .args(["--out"])
            .arg(&evidence)
            .args(["--config"])
            .arg(&config)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );

        let evidence_path = evidence.join("conformance-run.json");
        let mut baseline = read_json(&evidence_path);
        for tool in baseline["tools"].as_array_mut().unwrap() {
            tool["lock_status"] = json!("matched");
        }
        baseline["instances"][0]["case_id"] = json!(case_id);

        let mut secondary_tool = baseline["tools"][0].clone();
        secondary_tool["adapter_id"] = json!(ADAPTER_ID);
        secondary_tool["role"] = json!("secondary_iod_validator");
        secondary_tool["required"] = json!(false);
        baseline["tools"]
            .as_array_mut()
            .unwrap()
            .push(secondary_tool);

        let mut secondary_result = baseline["instances"][0]["results"][0].clone();
        secondary_result["adapter_id"] = json!(ADAPTER_ID);
        secondary_result["role"] = json!("secondary_iod_validator");
        secondary_result["status"] = json!("completed");
        secondary_result["exit_code"] = json!(0);
        secondary_result["findings"] = json!([]);
        baseline["instances"][0]["results"]
            .as_array_mut()
            .unwrap()
            .push(secondary_result);

        let manifest_path = evidence.join("source/manifest.json");
        let mut manifest = read_json(&manifest_path);
        let instance_path = baseline["instances"][0]["path"].clone();
        manifest["files"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|file| file["path"] == instance_path)
            .unwrap()["case_id"] = json!(case_id);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        fs::write(&manifest_path, &manifest_bytes).unwrap();
        baseline["source"]["manifest_sha256"] =
            json!(dicom_test_suite::sha256_hex(&manifest_bytes));
        write_json(&evidence_path, &baseline);

        let allowlist = root.join("allowlist.json");
        write_json(
            &allowlist,
            &json!({"schema_version": "0.1.0", "findings": []}),
        );
        Self {
            evidence,
            allowlist,
            baseline,
        }
    }

    fn write_evidence(&self, evidence: &Value) {
        write_json(&self.evidence.join("conformance-run.json"), evidence);
    }

    fn verify(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["conformance", "verify"])
            .arg(&self.evidence)
            .args(["--allowlist"])
            .arg(&self.allowlist)
            .output()
            .unwrap()
    }

    fn assert_failure(&self, needle: &str) {
        let output = self.verify();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(needle),
            "expected {needle:?} in {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
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

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write_json(path: impl AsRef<Path>, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dts-conformance-rt-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
