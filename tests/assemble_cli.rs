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
        let mut command = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"));
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
        serde_json::from_slice(&fs::read("schemas/assembly-result-v2.schema.json").unwrap())
            .unwrap();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&published["result"]));
    assert!(validator.is_valid(&dry["result"]));
    assert_eq!(published["result"]["published"], true);
    assert_eq!(dry["result"]["published"], false);
    assert_eq!(
        published["result"]["assembly_result_schema_version"],
        "2.0.0"
    );
    assert_eq!(published["result"]["manifest_schema_version"], "2.0.0");
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
fn cli_validate_and_report_read_both_assembly_manifest_versions() {
    let workspace = root("manifest-readers-workspace");
    fs::create_dir_all(&workspace).unwrap();
    let request_path = workspace.join("request.json");
    fs::write(
        &request_path,
        include_bytes!("fixtures/cli/assembly-request-v1-capture.json"),
    )
    .unwrap();
    let output_root = root("manifest-readers");
    let assembled = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["assemble", "--request"])
        .arg(&request_path)
        .arg("--out")
        .arg(&output_root)
        .args(["--seed", "5", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stderr)
    );
    let envelope: serde_json::Value = serde_json::from_slice(&assembled.stdout).unwrap();
    let current_result = envelope["result"].clone();
    let result_v2_schema: serde_json::Value =
        serde_json::from_slice(&fs::read("schemas/assembly-result-v2.schema.json").unwrap())
            .unwrap();
    assert!(
        jsonschema::validator_for(&result_v2_schema)
            .unwrap()
            .is_valid(&current_result)
    );
    let legacy_result: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/cli/assembly-result-v1.json")).unwrap();
    let result_v1_schema: serde_json::Value =
        serde_json::from_slice(&fs::read("schemas/assembly-result.schema.json").unwrap()).unwrap();
    assert!(
        jsonschema::validator_for(&result_v1_schema)
            .unwrap()
            .is_valid(&legacy_result)
    );
    let mut normalized_result = current_result;
    normalized_result["assembly_result_schema_version"] = "1.0.0".into();
    normalized_result["manifest_schema_version"] = "1.0.0".into();
    normalized_result["requested_output_root"] = legacy_result["requested_output_root"].clone();
    normalized_result["manifest_path"] = legacy_result["manifest_path"].clone();
    assert_eq!(normalized_result, legacy_result);
    let manifest_path = output_root.join("manifest.json");
    let current = fs::read(&manifest_path).unwrap();
    for (version, bytes) in [
        ("2.0.0", current.as_slice()),
        (
            "1.0.0",
            include_bytes!("fixtures/cli/assembly-manifest-v1.json").as_slice(),
        ),
    ] {
        fs::write(&manifest_path, bytes).unwrap();
        for args in [
            vec!["validate", output_root.to_str().unwrap()],
            vec!["report", output_root.to_str().unwrap(), "--format", "json"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "assembly manifest {version} reader failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    fs::remove_dir_all(workspace).unwrap();
    fs::remove_dir_all(output_root).unwrap();
}

#[test]
fn cli_validate_and_report_reject_invalid_assembly_identity_contracts() {
    let workspace = root("manifest-rejections-workspace");
    fs::create_dir_all(&workspace).unwrap();
    let request_path = workspace.join("request.json");
    fs::write(
        &request_path,
        include_bytes!("fixtures/cli/assembly-request-v1-capture.json"),
    )
    .unwrap();
    let output_root = root("manifest-rejections");
    let assembled = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["assemble", "--request"])
        .arg(&request_path)
        .arg("--out")
        .arg(&output_root)
        .args(["--seed", "5"])
        .output()
        .unwrap();
    assert!(assembled.status.success());
    let manifest_path = output_root.join("manifest.json");
    let current: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let runtime = serde_json::json!({
        "runtime_id": "provider/primary/fixture",
        "runtime_kind": "generation_provider",
        "executable_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "version": "1.0.0",
        "invocation_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    });
    let mut changed_runtime = runtime.clone();
    changed_runtime["invocation_sha256"] =
        serde_json::json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    for (label, manifest, diagnostic) in [
        (
            "unknown-version",
            {
                let mut value = current.clone();
                value["manifest_schema_version"] = "9.0.0".into();
                value
            },
            "unsupported assembly manifest schema version",
        ),
        (
            "missing-identity",
            {
                let mut value = current.clone();
                value.as_object_mut().unwrap().remove("identity_projection");
                value
            },
            "identity_projection",
        ),
        (
            "malformed-digest",
            {
                let mut value = current.clone();
                value["identity_projection"]["engine"]["engine_sha256"] = "short".into();
                value
            },
            "short",
        ),
        (
            "duplicate-runtime",
            {
                let mut value = current.clone();
                value["identity_projection"]["external_runtime"] =
                    serde_json::json!([runtime, changed_runtime]);
                value
            },
            "duplicate runtime_id",
        ),
    ] {
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        for args in [
            vec!["validate", output_root.to_str().unwrap()],
            vec!["report", output_root.to_str().unwrap(), "--format", "json"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
                .args(args)
                .output()
                .unwrap();
            assert!(!output.status.success(), "{label} unexpectedly succeeded");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                "{label} diagnostic: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    fs::remove_dir_all(workspace).unwrap();
    fs::remove_dir_all(output_root).unwrap();
}

#[test]
fn assemble_cli_schema_failure_uses_stable_machine_error() {
    let workspace = root("invalid");
    fs::create_dir_all(&workspace).unwrap();
    let request_path = workspace.join("request.json");
    fs::write(&request_path, b"{}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
        let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
        if code == "request.version.unsupported" {
            assert_eq!(
                error["error"]["context"]["migration_action"],
                "select a version advertised by capabilities.result.supported_versions"
            );
        }
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
    let existing_result = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
    let limited_result = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
