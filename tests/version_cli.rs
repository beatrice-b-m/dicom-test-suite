use std::fs;
use std::process::Command;

use serde_json::Value;

fn compile_schema(path: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap()
}

#[test]
fn version_json_is_clean_schema_valid_and_resource_bound_outside_the_checkout() {
    let cwd = std::env::temp_dir().join(format!(
        "dts-version-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&cwd).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .current_dir(&cwd)
        .args(["version", "--format", "json"])
        .output()
        .unwrap();
    fs::remove_dir_all(cwd).unwrap();

    assert!(
        output.status.success(),
        "version failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "JSON success must have empty stderr"
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(compile_schema("schemas/cli-success-envelope.schema.json").is_valid(&envelope));
    assert!(compile_schema("schemas/version-result.schema.json").is_valid(&envelope["result"]));
    assert_eq!(envelope["command"], "version");
    assert_eq!(
        envelope["result"]["product"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        envelope["result"]["target"],
        env!("DICOM_TEST_SUITE_TARGET")
    );
    assert_eq!(
        envelope["result"]["product_resources"]["origin"],
        "embedded"
    );
    assert!(
        envelope["result"]["product_resources"]["resources"]
            .as_array()
            .is_some_and(|resources| !resources.is_empty())
    );
}

#[test]
fn version_human_output_preserves_the_existing_banner() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .arg("version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("dicom-test-suite {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn version_machine_resource_drift_is_a_stable_integrity_error() {
    let snapshot = dicom_test_suite::product_resources::ProductResources::embedded()
        .snapshot()
        .unwrap();
    let registry = snapshot.root().join("cases/registry.json");
    let mut bytes = fs::read(&registry).unwrap();
    bytes.push(b'\n');
    fs::write(registry, bytes).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "--resource-root",
            snapshot.root().to_str().unwrap(),
            "version",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(compile_schema("schemas/cli-error-envelope.schema.json").is_valid(&error));
    assert_eq!(error["command"], "version");
    assert_eq!(error["error"]["code"], "evidence.integrity.failed");
    assert_eq!(error["error"]["retryable"], false);
}
