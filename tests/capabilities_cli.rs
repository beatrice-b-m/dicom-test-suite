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
fn capabilities_json_is_live_schema_valid_and_conservative_outside_the_checkout() {
    let cwd = std::env::temp_dir().join(format!(
        "dts-capabilities-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&cwd).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .current_dir(&cwd)
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    fs::remove_dir_all(cwd).unwrap();

    assert!(
        output.status.success(),
        "capabilities failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "JSON success must have empty stderr"
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(compile_schema("schemas/cli-success-envelope.schema.json").is_valid(&envelope));
    assert!(
        compile_schema("schemas/capabilities-result.schema.json").is_valid(&envelope["result"])
    );
    assert_eq!(envelope["command"], "capabilities");
    assert_eq!(
        envelope["result"]["supported_versions"]["result_schemas"]["composition"][0],
        "1.0.0"
    );
    assert_eq!(
        envelope["result"]["supported_versions"]["result_schemas"]["validation"][0],
        "1.0.0"
    );
    assert_eq!(
        envelope["result"]["structural_assembly"]["availability"],
        "unavailable"
    );
    assert_eq!(
        envelope["result"]["structural_assembly"]["reason_code"],
        "capability.feature.unavailable"
    );
    assert!(
        envelope["result"]["qualified_templates"]
            .as_array()
            .is_some_and(|templates| !templates.is_empty())
    );
    let syntaxes = envelope["result"]["transfer_syntaxes"].as_array().unwrap();
    assert!(syntaxes.iter().any(|syntax| {
        syntax["uid"] == "1.2.840.10008.1.2.1" && syntax["availability"] == "available"
    }));
    for syntax in syntaxes {
        if syntax["availability"] == "unavailable" {
            assert!(
                syntax["unavailable_reasons"]
                    .as_array()
                    .is_some_and(|reasons| !reasons.is_empty()),
                "unavailable syntax lacks stable reason: {syntax}"
            );
        }
    }
    let runtimes = envelope["result"]["optional_runtimes"].as_array().unwrap();
    assert!(runtimes.iter().any(|runtime| {
        runtime["runtime_id"] == "highdicom_pydicom"
            && runtime["availability"] == "requires_explicit_configuration"
            && runtime["reason_code"] == "capability.runtime.unavailable"
    }));
    assert!(
        runtimes
            .iter()
            .all(|runtime| runtime["availability"] != "available")
    );
}

#[test]
fn capabilities_requires_an_explicit_machine_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .arg("capabilities")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "capabilities requires --format json\n"
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn capabilities_machine_syntax_failure_is_one_schema_valid_stderr_envelope() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["capabilities", "--format", "json", "--unknown"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(compile_schema("schemas/cli-error-envelope.schema.json").is_valid(&error));
    assert_eq!(error["command"], "capabilities");
    assert_eq!(error["error"]["code"], "command.syntax.invalid");
    assert_eq!(error["error"]["retryable"], false);
}
