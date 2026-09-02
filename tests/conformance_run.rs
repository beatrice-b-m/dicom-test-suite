#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn run_collects_deterministic_manifest_driven_instance_evidence() {
    let root = temp_dir("run");
    let generated = root.join("generated");
    generate_smoke(&generated);
    let manifest_path = generated.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"][1]["case_id"] = manifest["files"][0]["case_id"].clone();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let validator = fake_tool(
        &root,
        "validator",
        r#"case "$2" in
  *mono2*) echo 'Error - </PatientName(0010,0010)> - missing value';;
  *mono1*) echo 'Warning - </StudyDate(0008,0020)> - dubious value' >&2;;
esac"#,
    );
    let config = config(&root, &validator, 2);
    let evidence_root = root.join("evidence");
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
    assert_eq!(evidence["instances"].as_array().unwrap().len(), 3);
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/conformance-run.schema.json").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&evidence));
    let paths = evidence["instances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|instance| instance["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    assert_ne!(
        evidence["instances"][0]["stable_instance_key"],
        evidence["instances"][1]["stable_instance_key"]
    );
    assert_eq!(
        evidence["instances"][0]["case_id"],
        evidence["instances"][1]["case_id"]
    );
    let severities = evidence["instances"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|instance| instance["results"][0]["findings"].as_array().unwrap())
        .map(|finding| finding["severity"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(severities.contains(&"error"));
    assert!(severities.contains(&"warning"));

    for instance in evidence["instances"].as_array().unwrap() {
        let result = &instance["results"][0];
        for stream in ["stdout", "stderr"] {
            let relative = result[stream]["path"].as_str().unwrap();
            let bytes = fs::read(evidence_root.join(relative)).unwrap();
            assert_eq!(
                result[stream]["sha256"],
                synth_dicom_gen::sha256_hex(&bytes)
            );
        }
    }
}

#[test]
fn run_records_timeout_and_malformed_nonzero_output() {
    let root = temp_dir("failures");
    let generated = root.join("generated");
    generate_smoke(&generated);

    let malformed = fake_tool(&root, "malformed", "echo not-normalized\nexit 7");
    let malformed_evidence = root.join("malformed-evidence");
    run(
        &generated,
        &malformed_evidence,
        &config(&root, &malformed, 2),
    );
    let evidence: Value =
        serde_json::from_slice(&fs::read(malformed_evidence.join("conformance-run.json")).unwrap())
            .unwrap();
    assert_eq!(
        evidence["instances"][0]["results"][0]["findings"][0]["severity"],
        "unparsed_output"
    );

    let slow = fake_tool(
        &root,
        "slow",
        "if [ \"$1\" = \"--version\" ]; then exit 0; fi\nsleep 2",
    );
    let timeout_evidence = root.join("timeout-evidence");
    run(&generated, &timeout_evidence, &config(&root, &slow, 1));
    let evidence: Value =
        serde_json::from_slice(&fs::read(timeout_evidence.join("conformance-run.json")).unwrap())
            .unwrap();
    assert_eq!(evidence["instances"][0]["results"][0]["status"], "timeout");
    assert_eq!(
        evidence["instances"][0]["results"][0]["findings"][0]["severity"],
        "timeout"
    );
}

fn run(generated: &Path, evidence: &Path, config: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(generated)
        .args(["--out"])
        .arg(evidence)
        .args(["--config"])
        .arg(config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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

fn config(root: &Path, validator: &Path, timeout: u64) -> PathBuf {
    let path = root.join(format!(
        "validators-{}.json",
        validator.file_name().unwrap().to_string_lossy()
    ));
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [{
                "id": "dicom3tools-dciodvfy",
                "role": "primary_iod_validator",
                "executable": validator,
                "arguments": ["-new", "{input}"],
                "version_arguments": ["--version"],
                "timeout_seconds": timeout,
                "required": true,
                "platforms": ["macos"],
                "capabilities": ["iod_validation"]
            }]
        }))
        .unwrap(),
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

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dts-conformance-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
