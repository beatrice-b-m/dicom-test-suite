#[path = "support/generic_ct_bundle.rs"]
mod generic_ct_bundle;
#[path = "support/generic_dx_mg_bundle.rs"]
mod generic_dx_mg_bundle;
#[path = "support/generic_metadata_sc_bundle.rs"]
mod generic_metadata_sc_bundle;

use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{
    CorpusSelector, DicomTestSuite, GenerateCorpusOutcome, GenerateCorpusRequest,
    InspectCorpusRequest, ReportRequest, ValidateRequest,
};

#[test]
fn loaded_capabilities_are_verified_destination_free_and_sdk_consistent() {
    let f = Fixture::new();
    let base = [
        "capabilities",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "corpus-members",
        "--format",
        "json",
    ]
    .map(str::to_owned)
    .to_vec();
    let inspect = |args: &[String]| {
        let output = f.command(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        valid("capabilities-result-v3.schema.json", &value["result"]);
        value["result"].clone()
    };
    let metadata = inspect(&base);
    assert_eq!(
        metadata["loaded_corpus"]["assessment_state"],
        "not_assessed"
    );
    assert!(metadata["loaded_corpus"]["assessment"].is_null());
    assert_eq!(
        metadata["identity_domains"]["corpus_definition"],
        metadata["loaded_corpus"]["corpus_definition_identity"]
    );
    assert_eq!(metadata["identity_domains"]["external_runtime"], json!([]));
    assert!(
        metadata["provider_support"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["provider_id"] == "rust_native" && p["availability"] == "compiled")
    );
    for p in metadata["provider_support"].as_array().unwrap() {
        assert_eq!(p["runtime_assessment"], "not_performed");
    }
    let mut selected = base.clone();
    selected.extend(["--profile", "smoke", "--seed", "1", "--parallelism", "2"].map(str::to_owned));
    let result = inspect(&selected);
    // Inspection must not search PATH or invoke a provider/version executable.
    let without_tools = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(&selected)
        .current_dir(&f.0)
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(without_tools.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&without_tools.stdout).unwrap()["result"],
        result
    );
    let product = DicomTestSuite::embedded().unwrap();
    let sdk = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(
                f.0.join("definition.json"),
                f.0.join("corpus-members"),
            )
            .with_selection(CorpusSelector::Profile {
                profile: "smoke".into(),
                include_stress: false,
            })
            .with_parallelism(2),
        )
        .unwrap();
    assert_eq!(result, serde_json::to_value(sdk).unwrap());
    assert_eq!(
        result["loaded_corpus"]["assessment"]["validation"],
        "not_run"
    );
    assert_eq!(
        result["loaded_corpus"]["assessment"]["publication"],
        "not_run"
    );
    assert_eq!(
        result["loaded_corpus"]["assessment"]["artifact_ids"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(!f.0.join("generated").exists());
    let schema = validator("capabilities-result-v3.schema.json");
    for key in ["engine_sha256", "schema_set_sha256"] {
        let mut bad = result.clone();
        let domain = if key == "engine_sha256" {
            "engine"
        } else {
            "schema_set"
        };
        bad["identity_domains"][domain][key] = json!("bad");
        assert!(!schema.is_valid(&bad));
    }
    let mut bad = result.clone();
    bad["capabilities_result_schema_version"] = json!("99.0.0");
    assert!(!schema.is_valid(&bad));
    for options in [
        vec!["--seed", "1"],
        vec!["--parallelism", "2"],
        vec!["--include-stress"],
        vec!["--case-id", "unknown"],
    ] {
        let mut args = base.clone();
        args.extend(options.into_iter().map(str::to_owned));
        let output = f.command(&args);
        assert_eq!(output.status.code(), Some(2));
        let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(envelope["command"], "capabilities");
        assert_eq!(envelope["error"]["code"], "command.argument.missing");
    }
    let mut absent = base.clone();
    absent[2] = "./missing.json".into();
    absent.extend(["--parallelism".into(), "0".into()]);
    let output = f.command(&absent);
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], "request.schema.invalid");
    assert!(!f.0.join("generated").exists());
}

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
fn caller_named_ct_cli_is_sdk_identical_strictly_valid_and_reported() {
    let fixture = generic_ct_bundle::GenericCtBundle::new("cli-sdk");
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&fixture.root)
            .output()
            .unwrap()
    };
    let assessed = command(&[
        "capabilities",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--case-id",
        generic_ct_bundle::CASE_ID,
        "--seed",
        "1",
        "--parallelism",
        "4",
        "--format",
        "json",
    ]);
    assert!(
        assessed.status.success(),
        "{}",
        String::from_utf8_lossy(&assessed.stderr)
    );
    assert!(assessed.stderr.is_empty());
    let assessed: Value = serde_json::from_slice(&assessed.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &assessed);
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["parallelism"],
        4
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["artifact_ids"],
        json!(["curated_caller_signed_ct_caller_instance"])
    );
    let generated = command(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--case-id",
        generic_ct_bundle::CASE_ID,
        "--seed",
        "1",
        "--parallelism",
        "4",
        "--out",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(generated.stderr.is_empty());
    let generated: Value = serde_json::from_slice(&generated.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &generated);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["command"], "generate");
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 1);
    assert_eq!(generated["result"]["selected_case_count"], 1);
    assert_eq!(generated["result"]["direct_case_count"], 1);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    assert_eq!(generated["result"]["publication_status"], "published");

    let cli_manifest: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("cli-output/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        generated["result"]["identity_projection"],
        cli_manifest["identity_projection"]
    );
    generic_ct_bundle::assert_manifest(&cli_manifest, &fixture.identity);
    generic_ct_bundle::assert_output_closure(&fixture, "cli-output");

    let product = DicomTestSuite::embedded().unwrap();
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &fixture.descriptor,
                &fixture.members,
                fixture.root.join("sdk-output"),
                generic_ct_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("public SDK must publish the same caller CT capability")
    };
    let sdk_manifest: Value = sdk.manifest().deserialize().unwrap();
    assert_eq!(sdk_manifest, cli_manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(
            fixture
                .root
                .join("cli-output")
                .join(generic_ct_bundle::DICOM_PATH),
        )
        .unwrap(),
        fs::read(sdk.output_root().join(generic_ct_bundle::DICOM_PATH)).unwrap()
    );

    let validated = command(&["validate", "cli-output", "--format", "json"]);
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    assert!(validated.stderr.is_empty());
    let validated: Value = serde_json::from_slice(&validated.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &validated);
    valid("validation-result.schema.json", &validated["result"]);
    assert_eq!(validated["command"], "validate");
    assert_eq!(validated["result"]["valid"], true);
    assert_eq!(validated["result"]["files_checked"], 1);
    assert_eq!(validated["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 1);

    let reported = command(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert!(
        reported.status.success(),
        "{}",
        String::from_utf8_lossy(&reported.stderr)
    );
    assert!(reported.stderr.is_empty());
    let reported: Value = serde_json::from_slice(&reported.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &reported);
    valid("report-result-v2.schema.json", &reported["result"]);
    assert_eq!(reported["command"], "report");
    let cli_report = &reported["result"]["report"];
    generic_ct_bundle::assert_report(cli_report, &cli_manifest);
    let sdk_report = product
        .report(ReportRequest::new(sdk.output_root()))
        .unwrap();
    assert_eq!(
        sdk_report.kind(),
        synth_dicom_gen::sdk::ReportKind::ExternalCorpus
    );
    assert_eq!(sdk_report.deserialize::<Value>().unwrap(), *cli_report);
}

#[test]
fn caller_named_dx_mg_cli_is_sdk_identical_strictly_valid_and_reported() {
    let fixture = generic_dx_mg_bundle::GenericDxMgBundle::new();
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .env("PATH", "")
            .current_dir(&fixture.root)
            .output()
            .unwrap()
    };
    let assessed = command(&[
        "capabilities",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--case-id",
        generic_dx_mg_bundle::CASE_IDS[0],
        "--case-id",
        generic_dx_mg_bundle::CASE_IDS[1],
        "--case-id",
        generic_dx_mg_bundle::CASE_IDS[2],
        "--seed",
        "1",
        "--parallelism",
        "4",
        "--format",
        "json",
    ]);
    assert!(
        assessed.status.success(),
        "{}",
        String::from_utf8_lossy(&assessed.stderr)
    );
    assert!(assessed.stderr.is_empty());
    let assessed: Value = serde_json::from_slice(&assessed.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &assessed);
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["parallelism"],
        4
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["artifact_ids"],
        json!([
            "curated_caller_presentation_instance",
            "curated_caller_processing_instance",
            "curated_caller_digital_instance"
        ])
    );
    let sdk_capabilities = DicomTestSuite::embedded()
        .unwrap()
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&fixture.descriptor, &fixture.members)
                .with_selection(generic_dx_mg_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_capabilities).unwrap()
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["publication"],
        "not_run"
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["validation"],
        "not_run"
    );
    let generated = command(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--case-id",
        generic_dx_mg_bundle::CASE_IDS[0],
        "--case-id",
        generic_dx_mg_bundle::CASE_IDS[1],
        "--case-id",
        generic_dx_mg_bundle::CASE_IDS[2],
        "--seed",
        "1",
        "--parallelism",
        "4",
        "--out",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(generated.stderr.is_empty());
    let generated: Value = serde_json::from_slice(&generated.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &generated);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["command"], "generate");
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 3);
    assert_eq!(generated["result"]["selected_case_count"], 3);
    assert_eq!(generated["result"]["direct_case_count"], 3);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    assert_eq!(generated["result"]["publication_status"], "published");

    let cli_manifest: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("cli-output/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        generated["result"]["identity_projection"],
        cli_manifest["identity_projection"]
    );
    valid("manifest-v2.schema.json", &cli_manifest);
    generic_dx_mg_bundle::assert_manifest(&cli_manifest, &fixture.identity);
    fixture.assert_closure("cli-output");

    let product = DicomTestSuite::embedded().unwrap();
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &fixture.descriptor,
                &fixture.members,
                fixture.root.join("sdk-output"),
                generic_dx_mg_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("public SDK must publish the same caller DX/MG capability")
    };
    let sdk_manifest: Value = sdk.manifest().deserialize().unwrap();
    assert_eq!(sdk_manifest, cli_manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    fixture.assert_closure("sdk-output");
    for path in generic_dx_mg_bundle::DICOM_PATHS {
        assert_eq!(
            fs::read(fixture.root.join("cli-output").join(path),).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
    }

    let validated = command(&["validate", "cli-output", "--format", "json"]);
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    assert!(validated.stderr.is_empty());
    let validated: Value = serde_json::from_slice(&validated.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &validated);
    valid("validation-result.schema.json", &validated["result"]);
    assert_eq!(validated["command"], "validate");
    assert_eq!(validated["result"]["valid"], true);
    assert_eq!(validated["result"]["files_checked"], 3);
    assert_eq!(validated["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 3);

    let reported = command(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert!(
        reported.status.success(),
        "{}",
        String::from_utf8_lossy(&reported.stderr)
    );
    assert!(reported.stderr.is_empty());
    let reported: Value = serde_json::from_slice(&reported.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &reported);
    valid("report-result-v2.schema.json", &reported["result"]);
    assert_eq!(reported["command"], "report");
    let cli_report = &reported["result"]["report"];
    valid("coverage-report-v2.schema.json", cli_report);
    assert_eq!(cli_report["source_manifest"], cli_manifest);
    assert_eq!(
        cli_report["evidence"],
        json!({"class":"manifest_projection", "validation":"not_assessed", "independent_conformance":"not_assessed", "payloads_reopened":false})
    );
    assert_eq!(cli_report["summary"]["emitted_files"], 3);
    let sdk_report = product
        .report(ReportRequest::new(sdk.output_root()))
        .unwrap();
    assert_eq!(
        sdk_report.kind(),
        synth_dicom_gen::sdk::ReportKind::ExternalCorpus
    );
    assert_eq!(sdk_report.deserialize::<Value>().unwrap(), *cli_report);
    // Preserve the accepted three historical payloads through an exact external selection.
    let original = Fixture::new();
    let ids = generic_dx_mg_bundle::ORIGINAL_IDS;
    original.generate(
        "dx-mg-original",
        "core",
        &[
            "--seed",
            "1",
            "--parallelism",
            "4",
            "--case-id",
            ids[0],
            "--case-id",
            ids[1],
            "--case-id",
            ids[2],
        ],
    );
    let old: Value = serde_json::from_slice(
        &fs::read(original.0.join("generated/dx-mg-original/manifest.json")).unwrap(),
    )
    .unwrap();
    let files = old["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
    let expected = [
        (
            ids[1],
            1586,
            "5a379a14abb40d2a0b6741e0bb87f77c48278853ebe7f953650b15c20cb9e2e5",
        ),
        (
            ids[2],
            1546,
            "0b975741103e16dffe60ad9a3e01431dc39d24e16802605ddb6c27801ac6afa1",
        ),
        (
            ids[0],
            1496,
            "2b2155795d10c4c76fea2793f0d9a5b73fcdefe6931f3813291be1f10c4ad8c4",
        ),
    ];
    for (file, (case_id, size, hash)) in files.iter().zip(expected) {
        assert_eq!(file["case_id"], case_id);
        assert_eq!(file["size_bytes"], size);
        assert_eq!(file["sha256"], hash);
        let raw = fs::read(
            original
                .0
                .join("generated/dx-mg-original")
                .join(file["path"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(raw.len(), size);
        let digest = Command::new("python3").args(["-c", "import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
            .arg(original.0.join("generated/dx-mg-original").join(file["path"].as_str().unwrap()))
            .output().unwrap();
        assert!(digest.status.success());
        assert!(digest.stderr.is_empty());
        assert_eq!(String::from_utf8(digest.stdout).unwrap().trim(), hash);
    }
}

#[test]
fn caller_named_metadata_sc_cli_is_sdk_identical_strictly_valid_and_reported() {
    let fixture = generic_metadata_sc_bundle::GenericMetadataScBundle::new();
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .env("PATH", "")
            .current_dir(&fixture.root)
            .output()
            .unwrap()
    };
    let assessed = command(&[
        "capabilities",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--case-id",
        generic_metadata_sc_bundle::CASE_IDS[0],
        "--case-id",
        generic_metadata_sc_bundle::CASE_IDS[1],
        "--case-id",
        generic_metadata_sc_bundle::CASE_IDS[2],
        "--seed",
        "1",
        "--parallelism",
        "4",
        "--format",
        "json",
    ]);
    assert!(
        assessed.status.success(),
        "{}",
        String::from_utf8_lossy(&assessed.stderr)
    );
    assert!(assessed.stderr.is_empty());
    let assessed: Value = serde_json::from_slice(&assessed.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &assessed);
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["parallelism"],
        4
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["artifact_ids"],
        json!([
            "curated_caller_name_instance",
            "curated_caller_empty_instance",
            "curated_caller_private_instance"
        ])
    );
    let sdk_capabilities = DicomTestSuite::embedded()
        .unwrap()
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&fixture.descriptor, &fixture.members)
                .with_selection(generic_metadata_sc_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_capabilities).unwrap()
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["publication"],
        "not_run"
    );
    assert_eq!(
        assessed["result"]["loaded_corpus"]["assessment"]["validation"],
        "not_run"
    );
    let generated = command(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--case-id",
        generic_metadata_sc_bundle::CASE_IDS[0],
        "--case-id",
        generic_metadata_sc_bundle::CASE_IDS[1],
        "--case-id",
        generic_metadata_sc_bundle::CASE_IDS[2],
        "--seed",
        "1",
        "--parallelism",
        "4",
        "--out",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(generated.stderr.is_empty());
    let generated: Value = serde_json::from_slice(&generated.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &generated);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["command"], "generate");
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 3);
    assert_eq!(generated["result"]["selected_case_count"], 3);
    assert_eq!(generated["result"]["direct_case_count"], 3);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    assert_eq!(generated["result"]["publication_status"], "published");

    let cli_manifest: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("cli-output/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        generated["result"]["identity_projection"],
        cli_manifest["identity_projection"]
    );
    valid("manifest-v2.schema.json", &cli_manifest);
    generic_metadata_sc_bundle::assert_manifest(&cli_manifest, &fixture.identity);
    fixture.assert_closure("cli-output");

    let product = DicomTestSuite::embedded().unwrap();
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &fixture.descriptor,
                &fixture.members,
                fixture.root.join("sdk-output"),
                generic_metadata_sc_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("public SDK must publish the same caller metadata SC capability")
    };
    let sdk_manifest: Value = sdk.manifest().deserialize().unwrap();
    assert_eq!(sdk_manifest, cli_manifest);
    assert_eq!(
        fs::read(fixture.root.join("cli-output/manifest.json")).unwrap(),
        fs::read(sdk.output_root().join("manifest.json")).unwrap()
    );
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    fixture.assert_closure("sdk-output");
    for path in generic_metadata_sc_bundle::DICOM_PATHS {
        assert_eq!(
            fs::read(fixture.root.join("cli-output").join(path),).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
    }

    for file in cli_manifest["files"].as_array().unwrap() {
        for output in [
            fixture.root.join("cli-output"),
            sdk.output_root().to_path_buf(),
        ] {
            generic_metadata_sc_bundle::assert_payload_hash(
                &output.join(file["path"].as_str().unwrap()),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }

    let validated = command(&["validate", "cli-output", "--format", "json"]);
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    assert!(validated.stderr.is_empty());
    let validated: Value = serde_json::from_slice(&validated.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &validated);
    valid("validation-result.schema.json", &validated["result"]);
    assert_eq!(validated["command"], "validate");
    assert_eq!(validated["result"]["valid"], true);
    assert_eq!(validated["result"]["files_checked"], 3);
    assert_eq!(validated["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 3);

    let reported = command(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert!(
        reported.status.success(),
        "{}",
        String::from_utf8_lossy(&reported.stderr)
    );
    assert!(reported.stderr.is_empty());
    let reported: Value = serde_json::from_slice(&reported.stdout).unwrap();
    valid("cli-success-envelope.schema.json", &reported);
    valid("report-result-v2.schema.json", &reported["result"]);
    assert_eq!(reported["command"], "report");
    let cli_report = &reported["result"]["report"];
    valid("coverage-report-v2.schema.json", cli_report);
    assert_eq!(cli_report["source_manifest"], cli_manifest);
    assert_eq!(
        cli_report["evidence"],
        json!({"class":"manifest_projection", "validation":"not_assessed", "independent_conformance":"not_assessed", "payloads_reopened":false})
    );
    assert_eq!(cli_report["summary"]["emitted_files"], 3);
    let sdk_report = product
        .report(ReportRequest::new(sdk.output_root()))
        .unwrap();
    assert_eq!(
        sdk_report.kind(),
        synth_dicom_gen::sdk::ReportKind::ExternalCorpus
    );
    assert_eq!(sdk_report.deserialize::<Value>().unwrap(), *cli_report);
    // Preserve the accepted three historical payloads through an exact external selection.
    let original = Fixture::new();
    let ids = generic_metadata_sc_bundle::ORIGINAL_IDS;
    original.generate(
        "metadata-original",
        "core",
        &[
            "--seed",
            "1",
            "--parallelism",
            "4",
            "--case-id",
            ids[0],
            "--case-id",
            ids[1],
            "--case-id",
            ids[2],
        ],
    );
    let old: Value = serde_json::from_slice(
        &fs::read(original.0.join("generated/metadata-original/manifest.json")).unwrap(),
    )
    .unwrap();
    let files = old["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
    let expected = [
        (
            ids[0],
            978,
            "b1334cff9865e0a8f4e6d9af50f15fd043beea971c98be596fbaa9d200936ac9",
        ),
        (
            ids[1],
            932,
            "7f457e4f9593a8d41dff970d32de86c8b5493841546dd6d60b219f311a7abc7c",
        ),
        (
            ids[2],
            1114,
            "5a0726a68554bb55a6dc5f7a74f639138dc365e8a46f444013303261705141e9",
        ),
    ];
    for (original_file, caller_file) in files.iter().zip(cli_manifest["files"].as_array().unwrap())
    {
        for key in [
            "expected_metadata",
            "expected_semantics",
            "image",
            "pixel_data",
            "standards_evidence",
        ] {
            assert_eq!(
                original_file[key], caller_file[key],
                "preserved metadata semantic field {key}"
            );
        }
    }
    for (file, (case_id, size, hash)) in files.iter().zip(expected) {
        assert_eq!(file["case_id"], case_id);
        assert_eq!(file["size_bytes"], size);
        assert_eq!(file["sha256"], hash);
        let raw = fs::read(
            original
                .0
                .join("generated/metadata-original")
                .join(file["path"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(raw.len(), size);
        let digest = Command::new("python3").args(["-c", "import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
            .arg(original.0.join("generated/metadata-original").join(file["path"].as_str().unwrap()))
            .output().unwrap();
        assert!(digest.status.success());
        assert!(digest.stderr.is_empty());
        assert_eq!(String::from_utf8(digest.stdout).unwrap().trim(), hash);
    }
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
    let validation =
        f.command(&["validate", "generated/cli", "--format", "json"].map(String::from));
    assert!(
        validation.status.success(),
        "{}",
        String::from_utf8_lossy(&validation.stderr)
    );
    let validation: Value = serde_json::from_slice(&validation.stdout).unwrap();
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
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
    let mut args = f.args("api-preview", "smoke");
    args.truncate(args.len() - 2);
    args.extend(["--cli-api".into(), "1.0.0".into(), "--dry-run".into()]);
    let output = f.command(&args);
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    valid("generation-result-v3.schema.json", &envelope["result"]);
    assert_eq!(envelope["result"]["outcome"], "planned");
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
