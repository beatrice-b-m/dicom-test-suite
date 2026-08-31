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

#[test]
fn assemble_cli_adversarial_inputs_use_stable_exit_classes() {
    let fixtures = [
        (
            "protected",
            br#"{"assembly_request_schema_version":"1.0.0","instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","elements":[{"address":{"keyword":"SOPInstanceUID"},"value":{"kind":"string","value":"1.2.3.4"}}]}]}"#.as_slice(),
            2,
            "request.schema.invalid",
        ),
        (
            "unsafe",
            br#"{"assembly_request_schema_version":"1.0.0","instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","output_path":"../escape.dcm","elements":[]}]}"#.as_slice(),
            4,
            "output.path.unsafe",
        ),
        (
            "transfer",
            br#"{"assembly_request_schema_version":"1.0.0","instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","transfer_syntax_uid":"1.2.840.10008.1.2.4.50","elements":[]}]}"#.as_slice(),
            3,
            "capability.transfer_syntax.unavailable",
        ),
        (
            "version",
            br#"{"assembly_request_schema_version":"2.0.0","instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","elements":[]}]}"#.as_slice(),
            2,
            "request.version.unsupported",
        ),
    ];
    let workspace = root("adversarial");
    fs::create_dir_all(&workspace).unwrap();
    for (label, request, exit, code) in fixtures {
        let request_path = workspace.join(format!("{label}.json"));
        fs::write(&request_path, request).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["assemble", "--request"])
            .arg(&request_path)
            .arg("--out")
            .arg(root(label))
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(exit), "{label}");
        assert!(output.stdout.is_empty(), "{label}");
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code, "{label}: {error}");
    }
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn assemble_cli_destination_and_resource_failures_publish_nothing() {
    let workspace = root("transaction");
    fs::create_dir_all(&workspace).unwrap();
    let request_path = workspace.join("request.json");
    request(&request_path);
    let existing = root("existing");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("sentinel"), b"preserve").unwrap();
    let existing_result = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["assemble", "--request"])
        .arg(&request_path)
        .arg("--out")
        .arg(&existing)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(existing_result.status.code(), Some(4));
    let error: serde_json::Value = serde_json::from_slice(&existing_result.stderr).unwrap();
    assert_eq!(error["error"]["code"], "output.destination.exists");
    assert_eq!(fs::read(existing.join("sentinel")).unwrap(), b"preserve");

    let limited_path = workspace.join("limited.json");
    fs::write(
        &limited_path,
        br#"{"assembly_request_schema_version":"1.0.0","limits":{"max_output_bytes":1},"instances":[{"instance_id":"limited","sop_class_uid":"1.2.3","elements":[]}]}"#,
    )
    .unwrap();
    let limited = root("limited");
    let limited_result = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["assemble", "--request"])
        .arg(&limited_path)
        .arg("--out")
        .arg(&limited)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        limited_result.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&limited_result.stderr)
    );
    let error: serde_json::Value = serde_json::from_slice(&limited_result.stderr).unwrap();
    assert_eq!(error["error"]["code"], "resource.limit.exceeded");
    assert!(!limited.exists());

    fs::remove_dir_all(workspace).unwrap();
    fs::remove_dir_all(existing).unwrap();
}
