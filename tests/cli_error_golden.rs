use std::fs;
use std::process::{Command, Output};

use synth_dicom_gen::cli_protocol::CliFailure;
use serde_json::Value;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_synth-dicom-gen")
}

fn assert_error(output: Output, exit: i32, command: &str, code: &str) {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/cli-error-envelope.schema.json").unwrap())
            .unwrap();
    assert!(
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&envelope)
    );
    assert_eq!(envelope["command"], command);
    assert_eq!(envelope["error"]["code"], code);
}

#[test]
fn machine_exit_classes_are_stable_end_to_end() {
    let success = Command::new(binary())
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(success.status.code(), Some(0));
    assert!(success.stderr.is_empty());

    assert_error(
        Command::new(binary())
            .args(["capabilities", "--format", "json", "--unknown"])
            .output()
            .unwrap(),
        2,
        "capabilities",
        "command.syntax.invalid",
    );
    assert_error(
        Command::new(binary())
            .args([
                "templates",
                "describe",
                "classic/unknown",
                "--format",
                "json",
            ])
            .output()
            .unwrap(),
        3,
        "templates describe",
        "capability.template.unavailable",
    );

    let existing = unique_root("existing-output");
    fs::create_dir(&existing).unwrap();
    assert_error(
        Command::new(binary())
            .args([
                "generate",
                "--profile",
                "smoke",
                "--out",
                existing.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .unwrap(),
        4,
        "generate",
        "output.destination.exists",
    );
    fs::remove_dir(&existing).unwrap();

    let snapshot = synth_dicom_gen::product_resources::ProductResources::embedded()
        .snapshot()
        .unwrap();
    let registry = snapshot.root().join("cases/registry.json");
    let mut bytes = fs::read(&registry).unwrap();
    bytes.push(b'\n');
    fs::write(registry, bytes).unwrap();
    assert_error(
        Command::new(binary())
            .args([
                "--resource-root",
                snapshot.root().to_str().unwrap(),
                "version",
                "--format",
                "json",
            ])
            .output()
            .unwrap(),
        5,
        "version",
        "evidence.integrity.failed",
    );

    let directory_as_registry = unique_root("directory-registry");
    fs::create_dir(&directory_as_registry).unwrap();
    assert_error(
        Command::new(binary())
            .args([
                "list-cases",
                "--registry",
                directory_as_registry.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .unwrap(),
        6,
        "list-cases",
        "io.read.failed",
    );
    fs::remove_dir(directory_as_registry).unwrap();
}

#[test]
fn workflow_error_classification_matches_the_append_only_registry() {
    let cases = [
        (
            "version",
            "unknown version argument: --bad",
            2,
            "command.syntax.invalid",
        ),
        (
            "capabilities",
            "capabilities requires --format json",
            2,
            "command.argument.missing",
        ),
        (
            "generate",
            "generation output path /tmp/out already exists; choose a new path",
            4,
            "output.destination.exists",
        ),
        (
            "compose",
            "composition request invalid: protected content tag 0028,0010",
            2,
            "request.schema.invalid",
        ),
        (
            "validate",
            "validation failed",
            5,
            "validation.artifact.failed",
        ),
        (
            "templates describe",
            "unknown template classic/unknown",
            3,
            "capability.template.unavailable",
        ),
        (
            "report",
            "failed to read manifest /tmp/missing",
            2,
            "request.read.failed",
        ),
        (
            "standards verify-kb",
            "standards knowledge base unavailable",
            3,
            "capability.runtime.unavailable",
        ),
        (
            "conformance verify",
            "conformance verification failed",
            5,
            "conformance.verification.failed",
        ),
        (
            "interoperate media-dicomdir",
            "interoperability qualification failed",
            5,
            "interoperability.qualification.failed",
        ),
        (
            "list-cases",
            "failed to read case registry",
            6,
            "io.read.failed",
        ),
    ];
    for (command, message, exit, code) in cases {
        let failure = CliFailure::classify(command, message);
        assert_eq!(failure.exit, exit, "{command}: {message}");
        assert_eq!(failure.error.code, code, "{command}: {message}");
    }
}

fn unique_root(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dts-cli-error-{label}-{}-{nonce}",
        std::process::id()
    ))
}
