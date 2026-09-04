use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{
    CorpusSelector, DicomTestSuite, GenerateCorpusOutcome, GenerateCorpusRequest,
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "synth-dicom-gen-external-cli-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        assert!(
            Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/build-current-corpus-definition-bundle.py"
                ))
                .arg(root.join("corpus-members"))
                .output()
                .unwrap()
                .status
                .success()
        );
        fs::rename(
            root.join("corpus-members/corpus-definition.json"),
            root.join("definition.json"),
        )
        .unwrap();
        Self(root)
    }
    fn args(&self, name: &str, profile: &str) -> Vec<String> {
        [
            "generate",
            "--corpus",
            "./definition.json",
            "--asset-root",
            "corpus-members",
            "--profile",
            profile,
            "--out",
            &format!("generated/{name}"),
            "--format",
            "json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }
    fn command(&self, args: &[String]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn generate(&self, name: &str, profile: &str, extra: &[&str]) -> Value {
        let mut args = self.args(name, profile);
        args.extend(extra.iter().map(|s| s.to_string()));
        let output = self.command(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &envelope);
        assert_eq!(envelope["command"], "generate");
        valid("generation-result-v3.schema.json", &envelope["result"]);
        envelope["result"].clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn validator(name: &str) -> jsonschema::Validator {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let resources = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            let v: Value = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
            (
                v["$id"].as_str().unwrap().to_string(),
                jsonschema::Resource::from_contents(v).unwrap(),
            )
        });
    jsonschema::options()
        .with_resources(resources)
        .build(&serde_json::from_slice::<Value>(&fs::read(root.join(name)).unwrap()).unwrap())
        .unwrap()
}
fn valid(name: &str, value: &Value) {
    let validator = validator(name);
    let errors = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}
fn error(output: Output, code: &str, exit: i32) {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    valid("cli-error-envelope.schema.json", &v);
    assert_eq!(v["command"], "generate");
    assert_eq!(v["error"]["code"], code);
    let registry: Value =
        serde_json::from_slice(include_bytes!("../product/cli-error-codes.json")).unwrap();
    let row = registry["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == code)
        .unwrap();
    assert_eq!(v["error"]["retryable"], row["retryable_default"]);
    assert_eq!(v["error"]["message"], row["meaning"]);
}

#[test]
fn external_cli_profile_is_sdk_identical_and_reports_version_two() {
    let f = Fixture::new();
    let result = f.generate("cli", "smoke", &["--seed", "1", "--parallelism", "2"]);
    assert_eq!(result["outcome"], "published");
    assert_eq!(result["emitted_file_count"], 3);
    assert_eq!(result["output_bytes"], 2790);
    let product = DicomTestSuite::embedded().unwrap();
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                f.0.join("definition.json"),
                f.0.join("corpus-members"),
                f.0.join("sdk"),
                CorpusSelector::Profile {
                    profile: "smoke".into(),
                    include_stress: false,
                },
            )
            .with_seed(1)
            .with_parallelism(2),
        )
        .unwrap()
    else {
        panic!("publication")
    };
    let manifest: Value =
        serde_json::from_slice(&fs::read(f.0.join("generated/cli/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        result["identity_projection"],
        manifest["identity_projection"]
    );
    assert_eq!(result["selection_ledger"], manifest["selection_ledger"]);
    assert_eq!(result["selector"], manifest["run"]["selector"]);
    assert_eq!(result["corpus_plan_sha256"], sdk.corpus_plan_sha256());
    assert_eq!(
        result["selected_case_count"],
        manifest["selection_ledger"].as_array().unwrap().len()
    );
    assert_eq!(
        fs::canonicalize(result["manifest_path"].as_str().unwrap()).unwrap(),
        f.0.join("generated/cli/manifest.json")
    );
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.0.join("generated/cli").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
    }
    fs::remove_dir_all(f.0.join("corpus-members")).unwrap();
    fs::remove_file(f.0.join("definition.json")).unwrap();
    let raw = f.command(&["report", "generated/cli", "--format", "json"].map(String::from));
    assert!(raw.status.success());
    let report: Value = serde_json::from_slice(&raw.stdout).unwrap();
    assert_eq!(report["source_manifest"], manifest);
    let envelope = f.command(
        &[
            "report",
            "generated/cli",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(String::from),
    );
    assert!(envelope.status.success());
    let envelope: Value = serde_json::from_slice(&envelope.stdout).unwrap();
    valid("report-result-v2.schema.json", &envelope["result"]);
    assert_eq!(envelope["result"]["report"], report);
    let markdown =
        f.command(&["report", "generated/cli", "--format", "markdown"].map(String::from));
    assert!(markdown.status.success());
    assert!(
        String::from_utf8(markdown.stdout)
            .unwrap()
            .contains("No new validation")
    );
    let mut tampered = envelope["result"].clone();
    tampered["report_schema_version"] = json!("1.0.0");
    assert!(!validator("report-result-v2.schema.json").is_valid(&tampered));
}

#[test]
fn external_cli_planned_and_noexecution_preserve_scope_and_evidence() {
    let f = Fixture::new();
    for (name, extra, state, ready) in [
        ("dry", vec!["--dry-run"], "planned", true),
        (
            "empty-dry",
            vec![
                "--case-id",
                "classic/dx/mono2_u12_jpeg_extended",
                "--dry-run",
            ],
            "planned",
            false,
        ),
        (
            "planned",
            vec!["--case-id", "classic/dx/mono2_u12_jpeg_extended"],
            "no_executable_cases",
            false,
        ),
        (
            "unavailable",
            vec!["--case-id", "classic/sc/mono2_u16_jpeg2000_lossless"],
            "no_executable_cases",
            false,
        ),
    ] {
        let profile = if name == "dry" { "smoke" } else { "all" };
        let r = f.generate(name, profile, &extra);
        assert_eq!(r["outcome"], state);
        assert_eq!(r["manifest_path"], Value::Null);
        assert_eq!(r["publication_status"], "not_run");
        assert_eq!(r["validation_status"], "not_run");
        assert_eq!(r["emitted_file_count"], 0);
        assert_eq!(r["output_bytes"], 0);
        assert!(!f.0.join("generated").exists());
        assert_eq!(
            r["selection_ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["outcome"] == "ready"),
            ready
        );
        assert!(
            r["selection_ledger"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["outcome"] != "generated"
                    && row["artifact_paths"].as_array().unwrap().is_empty())
        );
        let mut bad = r.clone();
        bad["manifest_path"] = json!("fake");
        assert!(!validator("generation-result-v3.schema.json").is_valid(&bad));
        let mut bad = r.clone();
        bad["selection_ledger"][0]["outcome"] = json!("generated");
        assert!(!validator("generation-result-v3.schema.json").is_valid(&bad));
        if state == "no_executable_cases" {
            assert!(r["preview_artifact_ids"].as_array().unwrap().is_empty());
            let mut bad = r.clone();
            bad["selection_ledger"][0]["outcome"] = json!("ready");
            assert!(!validator("generation-result-v3.schema.json").is_valid(&bad));
        }
    }
    let r = f.generate(
        "ids",
        "all",
        &["--case-id", "derived/registration/spatial_ct_pair"],
    );
    assert!(r["dependency_case_count"].as_u64().unwrap() > 0);
    assert_eq!(r["direct_case_count"], 1);
    assert_eq!(
        r["selected_case_count"].as_u64().unwrap(),
        r["direct_case_count"].as_u64().unwrap() + r["dependency_case_count"].as_u64().unwrap()
    );
}

#[test]
fn external_cli_errors_are_structural_and_prepublication() {
    let f = Fixture::new();
    let mut invalid_format = f.args("bad-format", "smoke");
    let format_index = invalid_format
        .iter()
        .position(|arg| arg == "--format")
        .unwrap();
    invalid_format[format_index + 1] = "yaml".into();
    invalid_format.extend(["--cli-api".into(), "1.0.0".into()]);
    error(f.command(&invalid_format), "command.syntax.invalid", 2);
    assert!(!f.0.join("generated").exists());
    for (extra, code, exit) in [
        (
            vec!["--case-id", "unknown/case"],
            "request.schema.invalid",
            2,
        ),
        (
            vec![
                "--case-id",
                "classic/sc/mono1_u8_explicit_le",
                "--case-id",
                "classic/sc/mono1_u8_explicit_le",
            ],
            "request.schema.invalid",
            2,
        ),
        (vec!["--parallelism", "0"], "request.schema.invalid", 2),
        (vec!["--parallelism", "bad"], "command.syntax.invalid", 2),
        (vec!["--seed", "bad"], "command.syntax.invalid", 2),
        (vec!["--case-id", ""], "request.schema.invalid", 2),
        (
            vec!["--case-id", "negative/isolated/not_in_smoke"],
            "request.schema.invalid",
            2,
        ),
        (vec!["--unrecognized"], "command.syntax.invalid", 2),
        (
            vec!["--cli-api", "99.0.0"],
            "request.version.unsupported",
            2,
        ),
        (vec!["--include-stress"], "request.schema.invalid", 2),
    ] {
        let mut args = f.args("invalid", "smoke");
        args.extend(extra.into_iter().map(String::from));
        error(f.command(&args), code, exit);
        assert!(!f.0.join("generated").exists());
    }
    error(
        f.command(&f.args("invalid", "unknown")),
        "request.schema.invalid",
        2,
    );
    fs::create_dir(f.0.join("existing")).unwrap();
    fs::write(f.0.join("existing/sentinel"), b"keep").unwrap();
    let mut args = f.args("unused", "smoke");
    let i = args.iter().position(|v| v == "--out").unwrap();
    args[i + 1] = "existing".into();
    error(f.command(&args), "output.destination.exists", 4);
    args.extend(["--format".into(), "invalid".into()]);
    error(f.command(&args), "command.syntax.invalid", 2);
    assert_eq!(fs::read(f.0.join("existing/sentinel")).unwrap(), b"keep");
    let mut args = f.args("invalid", "smoke");
    args[2] = "definition.json".into();
    error(f.command(&args), "resource.document.invalid", 2);
    args[2] = "./missing.json".into();
    error(f.command(&args), "io.read.failed", 6);
    args[2] = "./definition.json".into();
    fs::write(f.0.join("corpus-members/cases/registry.json"), b"tampered").unwrap();
    error(f.command(&args), "evidence.integrity.failed", 5);
}

#[test]
fn external_cli_descriptor_limits_versions_and_missing_options_fail_closed() {
    let f = Fixture::new();
    let args = f.args("invalid", "smoke");
    let mut missing = args.clone();
    missing.truncate(2);
    missing.extend(["--format".into(), "json".into()]);
    error(f.command(&missing), "command.argument.missing", 2);
    let mut missing = args.clone();
    missing.drain(3..5);
    error(f.command(&missing), "command.argument.missing", 2);
    let bytes = fs::read(f.0.join("definition.json")).unwrap();
    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    value["corpus_definition_bundle_schema_version"] = json!("99.0.0");
    fs::write(
        f.0.join("definition.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    error(f.command(&args), "request.version.unsupported", 2);
    fs::write(f.0.join("definition.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    error(f.command(&args), "resource.limit.exceeded", 4);
    assert!(!f.0.join("generated").exists());
}

#[test]
fn corpus_option_looking_value_does_not_select_external_dispatch() {
    let f = Fixture::new();
    // Missing profile fails in the unchanged embedded parser; --corpus is an
    // out value, not an external flag. No output or corpus capture is attempted.
    let output =
        f.command(&["generate", "--out", "--corpus", "--format", "json"].map(String::from));
    assert_eq!(output.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(v["error"]["code"], "command.argument.missing");
    assert!(!f.0.join("--corpus").exists());
}
