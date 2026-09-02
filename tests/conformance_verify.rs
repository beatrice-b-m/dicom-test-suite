#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn verify_accepts_complete_clean_hash_linked_evidence() {
    let fixture = Fixture::new("clean");
    let output = fixture.verify(&fixture.allowlist);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("verification_failures\t0"));
}

#[test]
fn verify_rejects_hash_corruption_tool_gaps_and_incomplete_results() {
    let fixture = Fixture::new("integrity");
    let baseline = fixture.evidence_json();

    fs::write(fixture.evidence.join("source/manifest.json"), b"altered").unwrap();
    fixture.assert_failure("source manifest hash mismatch");
    fixture.restore(&baseline);

    let raw_path = baseline["instances"][0]["results"][0]["stdout"]["path"]
        .as_str()
        .unwrap();
    fs::write(fixture.evidence.join(raw_path), b"altered").unwrap();
    fixture.assert_failure("raw artifact hash mismatch");
    fixture.restore(&baseline);

    let mut evidence = baseline.clone();
    evidence["tools"][0]["status"] = json!("absent");
    fixture.write_evidence(&evidence);
    fixture.assert_failure("required tool");
    fixture.restore(&baseline);

    let mut evidence = baseline.clone();
    let mut optional_primary = evidence["tools"][0].clone();
    optional_primary["adapter_id"] = json!("optional-primary");
    optional_primary["required"] = json!(false);
    optional_primary["lock_status"] = json!("mismatched");
    evidence["tools"]
        .as_array_mut()
        .unwrap()
        .push(optional_primary);
    let primary_result = evidence["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|result| result["role"] == "primary_iod_validator")
        .unwrap();
    primary_result["adapter_id"] = json!("optional-primary");
    fixture.write_evidence(&evidence);
    fixture.assert_failure("primary validator optional-primary is unavailable or unlocked");
    evidence["tools"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["lock_status"] = json!("matched");
    fixture.write_evidence(&evidence);
    assert!(fixture.verify(&fixture.allowlist).status.success());
    fixture.restore(&baseline);

    let mut evidence = baseline.clone();
    evidence["instances"].as_array_mut().unwrap().pop();
    fixture.write_evidence(&evidence);
    fixture.assert_failure("instance evidence is incomplete");
}

#[test]
fn verify_requires_locked_registration_secondary_iod_evidence() {
    let fixture = Fixture::new("registration-secondary");
    let mut evidence = fixture.evidence_json();
    let path = evidence["instances"][0]["path"]
        .as_str()
        .unwrap()
        .to_string();
    evidence["instances"][0]["case_id"] = json!("derived/registration/spatial_ct_pair");

    let mut secondary_tool = evidence["tools"][0].clone();
    secondary_tool["adapter_id"] = json!("pydicom-dicom-validator-registration");
    secondary_tool["role"] = json!("secondary_iod_validator");
    secondary_tool["required"] = json!(false);
    evidence["tools"]
        .as_array_mut()
        .unwrap()
        .push(secondary_tool);

    let mut secondary_result = evidence["instances"][0]["results"][0].clone();
    secondary_result["adapter_id"] = json!("pydicom-dicom-validator-registration");
    secondary_result["role"] = json!("secondary_iod_validator");
    evidence["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .push(secondary_result);

    let mut manifest: Value = serde_json::from_slice(&fixture.source_manifest).unwrap();
    let file = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["path"] == path)
        .unwrap();
    file["case_id"] = json!("derived/registration/spatial_ct_pair");
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    fs::write(
        fixture.evidence.join("source/manifest.json"),
        &manifest_bytes,
    )
    .unwrap();
    evidence["source"]["manifest_sha256"] = json!(synth_dicom_gen::sha256_hex(&manifest_bytes));
    fixture.write_evidence(&evidence);
    assert!(fixture.verify(&fixture.allowlist).status.success());

    let mut missing = evidence.clone();
    missing["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .retain(|result| result["role"] != "secondary_iod_validator");
    fixture.write_evidence(&missing);
    fixture.assert_failure("required registration secondary IOD validation incomplete");

    let mut unlocked = evidence;
    unlocked["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-registration")
        .unwrap()["lock_status"] = json!("mismatched");
    fixture.write_evidence(&unlocked);
    fixture.assert_failure("required registration secondary IOD validator is unlocked");
}

#[test]
fn verify_requires_locked_presentation_state_secondary_iod_evidence() {
    let fixture = Fixture::new("presentation-state-secondary");
    let mut evidence = fixture.evidence_json();
    let path = evidence["instances"][0]["path"]
        .as_str()
        .unwrap()
        .to_string();
    evidence["instances"][0]["case_id"] = json!("derived/presentation-state/color_softcopy");

    let mut secondary_tool = evidence["tools"][0].clone();
    secondary_tool["adapter_id"] = json!("pydicom-dicom-validator-presentation-state");
    secondary_tool["role"] = json!("secondary_iod_validator");
    secondary_tool["required"] = json!(false);
    evidence["tools"]
        .as_array_mut()
        .unwrap()
        .push(secondary_tool);

    let mut secondary_result = evidence["instances"][0]["results"][0].clone();
    secondary_result["adapter_id"] = json!("pydicom-dicom-validator-presentation-state");
    secondary_result["role"] = json!("secondary_iod_validator");
    evidence["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .push(secondary_result);

    let mut manifest: Value = serde_json::from_slice(&fixture.source_manifest).unwrap();
    let file = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["path"] == path)
        .unwrap();
    file["case_id"] = json!("derived/presentation-state/color_softcopy");
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    fs::write(
        fixture.evidence.join("source/manifest.json"),
        &manifest_bytes,
    )
    .unwrap();
    evidence["source"]["manifest_sha256"] = json!(synth_dicom_gen::sha256_hex(&manifest_bytes));
    fixture.write_evidence(&evidence);
    assert!(fixture.verify(&fixture.allowlist).status.success());

    let mut missing = evidence.clone();
    missing["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .retain(|result| result["role"] != "secondary_iod_validator");
    fixture.write_evidence(&missing);
    fixture.assert_failure("required presentation-state secondary IOD validation incomplete");

    let mut unlocked = evidence;
    unlocked["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-presentation-state")
        .unwrap()["lock_status"] = json!("mismatched");
    fixture.write_evidence(&unlocked);
    fixture.assert_failure("required presentation-state secondary IOD validator is unlocked");
}

#[test]
fn verify_rejects_unknown_warnings_stale_dispositions_and_wildcards() {
    let fixture = Fixture::new("findings");
    let mut evidence = fixture.evidence_json();
    let message = "Warning - test warning without disposition";
    evidence["instances"][0]["results"][0]["findings"] = json!([{
        "severity": "warning",
        "rule_id": null,
        "message": message,
        "message_fingerprint": synth_dicom_gen::sha256_hex(message.as_bytes()),
        "dicom_path": null,
        "disposition": "unresolved"
    }]);
    fixture.write_evidence(&evidence);
    fixture.assert_failure("unresolved warning");

    let tool_hash = evidence["tools"][0]["sha256"].as_str().unwrap();
    let case_id = evidence["instances"][0]["case_id"].as_str().unwrap();
    let mut entry = json!({
        "validator_adapter_id": "primary",
        "validator_fingerprint": tool_hash,
        "case_id": case_id,
        "message_fingerprint": "f".repeat(64),
        "original_severity": "warning",
        "disposition": "validator_limitation",
        "rationale": "This deliberately stale entry exercises strict matching.",
        "citation": "PS3.3 test citation",
        "reviewer": "test reviewer",
        "review_date": "2026-08-26",
        "recheck_condition": "Recheck whenever the fixture changes"
    });
    fs::write(
        &fixture.allowlist,
        serde_json::to_vec_pretty(&json!({"schema_version": "0.1.0", "findings": [entry.clone()]}))
            .unwrap(),
    )
    .unwrap();
    fixture.assert_failure("stale disposition");

    entry["expires_on"] = json!("2020-01-01");
    fs::write(
        &fixture.allowlist,
        serde_json::to_vec_pretty(&json!({"schema_version": "0.1.0", "findings": [entry.clone()]}))
            .unwrap(),
    )
    .unwrap();
    fixture.assert_failure("expired disposition");

    entry.as_object_mut().unwrap().remove("expires_on");
    entry["case_id"] = json!("classic/*");
    fs::write(
        &fixture.allowlist,
        serde_json::to_vec_pretty(&json!({"schema_version": "0.1.0", "findings": [entry]}))
            .unwrap(),
    )
    .unwrap();
    fixture.assert_failure("allowlist schema");
}

struct Fixture {
    evidence: PathBuf,
    allowlist: PathBuf,
    source_manifest: Vec<u8>,
    raw_files: Vec<(PathBuf, Vec<u8>)>,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = temp_dir(label);
        let generated = root.join("generated");
        generate_smoke(&generated);
        let primary = fake_tool(&root, "primary", "exit 0");
        let entity = fake_tool(&root, "entity", "exit 0");
        let parser = fake_tool(&root, "parser", "exit 0");
        let config = root.join("validators.json");
        fs::write(
            &config,
            serde_json::to_vec(&json!({
                "schema_version": "0.1.0",
                "adapters": [
                    adapter("primary", "primary_iod_validator", &primary),
                    adapter("entity", "entity_validator", &entity),
                    adapter("parser", "independent_parser", &parser)
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let evidence = root.join("evidence");
        let run = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(["conformance", "run"])
            .arg(&generated)
            .args(["--out"])
            .arg(&evidence)
            .args(["--config"])
            .arg(&config)
            .output()
            .unwrap();
        assert!(run.status.success());
        let allowlist = root.join("allowlist.json");
        fs::write(
            &allowlist,
            b"{\"schema_version\":\"0.1.0\",\"findings\":[]}",
        )
        .unwrap();
        let mut value: Value =
            serde_json::from_slice(&fs::read(evidence.join("conformance-run.json")).unwrap())
                .unwrap();
        for tool in value["tools"].as_array_mut().unwrap() {
            tool["lock_status"] = json!("matched");
        }
        fs::write(
            evidence.join("conformance-run.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        let source_manifest = fs::read(evidence.join("source/manifest.json")).unwrap();
        let mut raw_files = Vec::new();
        for result in results(&value) {
            for stream in ["stdout", "stderr"] {
                let path = PathBuf::from(result[stream]["path"].as_str().unwrap());
                raw_files.push((path.clone(), fs::read(evidence.join(path)).unwrap()));
            }
        }
        Self {
            evidence,
            allowlist,
            source_manifest,
            raw_files,
        }
    }

    fn evidence_json(&self) -> Value {
        serde_json::from_slice(&fs::read(self.evidence.join("conformance-run.json")).unwrap())
            .unwrap()
    }

    fn write_evidence(&self, value: &Value) {
        fs::write(
            self.evidence.join("conformance-run.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }

    fn restore(&self, value: &Value) {
        self.write_evidence(value);
        fs::write(
            self.evidence.join("source/manifest.json"),
            &self.source_manifest,
        )
        .unwrap();
        for (path, bytes) in &self.raw_files {
            fs::write(self.evidence.join(path), bytes).unwrap();
        }
    }

    fn verify(&self, allowlist: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(["conformance", "verify"])
            .arg(&self.evidence)
            .args(["--allowlist"])
            .arg(allowlist)
            .output()
            .unwrap()
    }

    fn assert_failure(&self, needle: &str) {
        let output = self.verify(&self.allowlist);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(needle),
            "expected {needle:?} in {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

fn results(evidence: &Value) -> Vec<&Value> {
    let mut results = evidence["instances"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|instance| instance["results"].as_array().unwrap())
        .collect::<Vec<_>>();
    results.push(&evidence["entity"]);
    results
}

fn adapter(id: &str, role: &str, path: &Path) -> Value {
    json!({
        "id": id, "role": role, "executable": path, "arguments": [],
        "version_arguments": ["--version"], "timeout_seconds": 2,
        "required": true, "platforms": ["macos"], "capabilities": ["test"]
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
    let root = std::env::temp_dir().join(format!("dts-verify-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
