use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde_json::Value;

const CURRENT_COMMANDS: &[&str] = &[
    "version",
    "capabilities",
    "generate",
    "compose",
    "assemble",
    "templates list",
    "templates describe",
    "templates reference",
    "list-cases",
    "validate",
    "report",
    "report gaps",
    "standards check-lock",
    "standards gaps",
    "standards verify-kb",
    "conformance check-tools",
    "conformance run",
    "conformance verify",
    "interoperate media-dicomdir",
    "interoperate protocol-baseline",
];

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}")))
        .unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn validator(schema_path: &str) -> jsonschema::Validator {
    jsonschema::validator_for(&read_json(schema_path))
        .unwrap_or_else(|error| panic!("compile {schema_path}: {error}"))
}

#[test]
fn positive_cli_envelope_fixtures_validate() {
    let success = validator("schemas/cli-success-envelope.schema.json");
    let error = validator("schemas/cli-error-envelope.schema.json");

    assert!(success.is_valid(&read_json("tests/fixtures/cli/valid/success-envelope.json")));
    assert!(error.is_valid(&read_json("tests/fixtures/cli/valid/error-envelope.json")));
}

#[test]
fn adversarial_cli_envelope_fixtures_are_rejected() {
    let success = validator("schemas/cli-success-envelope.schema.json");
    let error = validator("schemas/cli-error-envelope.schema.json");

    for path in [
        "tests/fixtures/cli/invalid/success-wrong-version.json",
        "tests/fixtures/cli/invalid/success-extra-field.json",
    ] {
        assert!(
            !success.is_valid(&read_json(path)),
            "{path} must be rejected"
        );
    }
    for path in [
        "tests/fixtures/cli/invalid/error-invalid-code.json",
        "tests/fixtures/cli/invalid/error-nested-context.json",
    ] {
        assert!(!error.is_valid(&read_json(path)), "{path} must be rejected");
    }
}

#[test]
fn error_registry_is_valid_unique_and_covers_current_commands() {
    let registry = read_json("product/cli-error-codes.json");
    let registry_validator = validator("schemas/cli-error-code-registry.schema.json");
    let validation_errors = registry_validator
        .iter_errors(&registry)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        validation_errors.is_empty(),
        "error registry must satisfy its schema: {validation_errors:#?}"
    );

    let mut codes = BTreeMap::new();
    for error in registry["errors"].as_array().expect("errors") {
        let code = error["code"].as_str().expect("error code");
        let exit = error["exit"].as_u64().expect("error exit");
        assert!(codes.insert(code, exit).is_none(), "duplicate code {code}");
    }

    let mut mapped_commands = BTreeSet::new();
    for mapping in registry["current_failure_mappings"]
        .as_array()
        .expect("failure mappings")
    {
        let command = mapping["command"].as_str().expect("mapped command");
        assert!(
            mapped_commands.insert(command),
            "duplicate mapping for {command}"
        );
        let mut stages = BTreeSet::new();
        for stage in mapping["failure_stages"].as_array().expect("stages") {
            assert!(
                stages.insert(stage["stage"].as_str().expect("stage name")),
                "duplicate stage for {command}"
            );
            for code in stage["codes"].as_array().expect("stage codes") {
                let code = code.as_str().expect("mapped code");
                assert!(
                    codes.contains_key(code),
                    "{command} maps unknown code {code}"
                );
            }
        }
    }

    assert_eq!(
        mapped_commands,
        CURRENT_COMMANDS.iter().copied().collect(),
        "every current public command/subcommand must have a failure mapping"
    );
}

#[test]
fn error_registry_exit_classes_match_the_machine_contract() {
    let registry = read_json("product/cli-error-codes.json");
    let classes = registry["exit_classes"]
        .as_array()
        .expect("exit classes")
        .iter()
        .map(|entry| entry["exit"].as_u64().expect("exit"))
        .collect::<BTreeSet<_>>();
    assert_eq!(classes, BTreeSet::from([0, 2, 3, 4, 5, 6]));

    for error in registry["errors"].as_array().expect("errors") {
        assert!(
            classes.contains(&error["exit"].as_u64().expect("error exit")),
            "every error must use a documented exit class"
        );
    }
}
