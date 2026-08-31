use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-assemble-cli-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn request(path: &std::path::Path) {
    fs::write(path, br#"{"assembly_request_schema_version":"1.0.0","instances":[{"instance_id":"primary","sop_class_uid":"1.2.840.10008.5.1.4.1.1.7","elements":[{"address":{"keyword":"PatientName"},"value":{"kind":"string","value":"SYNTHETIC^CLI"}}]}]}"#).unwrap();
}

#[test]
fn assemble_cli_publish_and_dry_run_share_the_typed_machine_shape() {
    let workspace = root("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let request_path = workspace.join("request.json");
    request(&request_path);
    let published_root = root("published");
    let run = |out: &std::path::Path, dry: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"));
        command
            .args(["assemble", "--request"])
            .arg(&request_path)
            .arg("--out")
            .arg(out)
            .args(["--seed", "3", "--format", "json"]);
        if dry {
            command.arg("--dry-run");
        }
        command.output().unwrap()
    };
    let published = run(&published_root, false);
    assert!(
        published.status.success(),
        "{}",
        String::from_utf8_lossy(&published.stderr)
    );
    assert!(published.stderr.is_empty());
    let published: serde_json::Value = serde_json::from_slice(&published.stdout).unwrap();
    let dry_root = root("dry");
    let dry = run(&dry_root, true);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let dry: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    let schema: serde_json::Value =
        serde_json::from_slice(&fs::read("schemas/assembly-result.schema.json").unwrap()).unwrap();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&published["result"]));
    assert!(validator.is_valid(&dry["result"]));
    assert_eq!(published["result"]["published"], true);
    assert_eq!(dry["result"]["published"], false);
    assert_eq!(
        published["result"]["corpus_plan_sha256"],
        dry["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        published["result"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        dry["result"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(!dry_root.exists());
    fs::remove_dir_all(workspace).unwrap();
    fs::remove_dir_all(published_root).unwrap();
}

#[test]
fn assemble_cli_schema_failure_uses_stable_machine_error() {
    let workspace = root("invalid");
    fs::create_dir_all(&workspace).unwrap();
    let request_path = workspace.join("request.json");
    fs::write(&request_path, b"{}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["assemble", "--request"])
        .arg(&request_path)
        .arg("--out")
        .arg(root("never"))
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "assemble");
    assert_eq!(error["error"]["code"], "request.schema.invalid");
    fs::remove_dir_all(workspace).unwrap();
}
