use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

static NONCE: AtomicU64 = AtomicU64::new(0);

fn compile_schema(path: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap()
}

#[test]
fn protocol_baseline_cli_emits_separate_explicit_unavailable_report() {
    let root = fixture_root();
    write_manifest(&root);
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("security/fixtures/fixtures.lock.json");
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "interoperate",
            "protocol-baseline",
            root.to_str().unwrap(),
            "--format",
            "json",
            "--seed",
            "7",
            "--fixtures",
            fixtures.to_str().unwrap(),
        ])
        .output()
        .expect("protocol baseline command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("report should parse");
    assert!(compile_schema("schemas/cli-success-envelope.schema.json").is_valid(&envelope));
    assert!(
        compile_schema("schemas/interoperability-result.schema.json").is_valid(&envelope["result"])
    );
    assert_eq!(envelope["command"], "interoperate protocol-baseline");
    let report = &envelope["result"]["evidence"];
    assert_eq!(
        report.pointer("/summary/total/unavailable"),
        Some(&json!(3))
    );
    assert_eq!(report.pointer("/summary/total/passed"), Some(&json!(0)));
    assert_eq!(
        report
            .pointer("/transactions/0/steps/0/outcome/blocker_code")
            .and_then(Value::as_str),
        Some("independent_dcm4che_peer_unavailable")
    );
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("private_key"));
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn media_cli_requires_explicit_optional_tool_paths() {
    let root = fixture_root();
    write_manifest(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "interoperate",
            "media-dicomdir",
            root.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("media command should run");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires --dcmmkdir"));
    fs::remove_dir_all(root).expect("remove fixture");
}

fn write_manifest(root: &Path) {
    fs::create_dir_all(root).expect("create fixture root");
    let image_uid = "1.2.3.1";
    let rows = [
        json!({
            "case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
            "path": "enhanced/ct/source.dcm",
            "sha256": "a".repeat(64),
            "dicom": {"sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1"},
            "uids": {"sop_instance_uid": image_uid},
            "references": []
        }),
        json!({
            "case_id": "derived/seg/binary_multiframe_explicit_le",
            "path": "derived/seg/instance.dcm",
            "sha256": "b".repeat(64),
            "dicom": {"sop_class_uid": "1.2.840.10008.5.1.4.1.1.66.4"},
            "uids": {"sop_instance_uid": "1.2.3.2"},
            "references": [{"sop_instance_uid": image_uid}]
        }),
        json!({
            "case_id": "non-image/waveform/general_ecg",
            "path": "non-image/waveform/instance.dcm",
            "sha256": "c".repeat(64),
            "dicom": {"sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.2"},
            "uids": {"sop_instance_uid": "1.2.3.3"},
            "references": []
        }),
    ];
    for row in &rows {
        let path = root.join(row["path"].as_str().unwrap());
        fs::create_dir_all(path.parent().unwrap()).expect("create payload parent");
        fs::write(path, b"not-read-by-protocol-baseline").expect("write placeholder");
    }
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&json!({"files": rows})).unwrap(),
    )
    .expect("write manifest");
}

fn fixture_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-interoperate-cli-{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ))
}
