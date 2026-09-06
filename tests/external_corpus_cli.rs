#[path = "support/generic_cr_bundle.rs"]
mod generic_cr_bundle;
#[path = "support/generic_ct_bundle.rs"]
mod generic_ct_bundle;
#[path = "support/generic_ct_geometry_bundle.rs"]
mod generic_ct_geometry_bundle;
#[path = "support/generic_dx_mg_bundle.rs"]
mod generic_dx_mg_bundle;
#[path = "support/generic_metadata_sc_bundle.rs"]
mod generic_metadata_sc_bundle;
#[path = "support/generic_mr_bundle.rs"]
mod generic_mr_bundle;
#[path = "support/generic_nm_multiframe_bundle.rs"]
mod generic_nm_multiframe_bundle;
#[path = "support/generic_nonsquare_sc_bundle.rs"]
mod generic_nonsquare_sc_bundle;
#[path = "support/generic_pet_bundle.rs"]
mod generic_pet_bundle;
#[path = "support/generic_sc_bundle.rs"]
mod generic_sc_bundle;
#[path = "support/generic_timezone_sc_bundle.rs"]
mod generic_timezone_sc_bundle;
#[path = "support/generic_us_bundle.rs"]
mod generic_us_bundle;
#[path = "support/generic_us_multiframe_bundle.rs"]
mod generic_us_multiframe_bundle;
#[path = "support/generic_vl_photo_bundle.rs"]
mod generic_vl_photo_bundle;
#[path = "support/generic_xa_xrf_bundle.rs"]
mod generic_xa_xrf_bundle;

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

#[test]
fn caller_named_sc_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_sc_bundle::GenericScBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_sc_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    let oracle = generic_sc_bundle::oracle();
    let rows = oracle["cases"].as_array().unwrap();
    let assessment = &assessed["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!(
            rows.iter()
                .map(|r| format!("curated_{}_instance", r["recipe_id"].as_str().unwrap()))
                .collect::<Vec<_>>()
        )
    );
    assert!(!f.root.join("cli-output").exists());
    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 13);
    assert_eq!(generated["result"]["direct_case_count"], 13);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_sc_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_sc_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller SC must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("cli-output").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
        for root in [f.root.join("cli-output"), sdk.output_root().to_path_buf()] {
            generic_sc_bundle::assert_payload(
                &root.join(path),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }
    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 13);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 13);
    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 13);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );
    let original = Fixture::new();
    for profile in ["smoke", "core"] {
        let selected = rows
            .iter()
            .filter(|r| r["profile"] == profile)
            .collect::<Vec<_>>();
        let mut extra = vec!["--seed", "1", "--parallelism", "4"];
        for row in &selected {
            extra.extend(["--case-id", row["source_case"].as_str().unwrap()]);
        }
        let name = format!("sc-original-{profile}");
        original.generate(&name, profile, &extra);
        let root = original.0.join("generated").join(&name);
        let historical: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let files = historical["files"].as_array().unwrap();
        assert_eq!(files.len(), selected.len());
        assert_eq!(historical["run"]["profile"], profile);
        for (file, row) in files.iter().zip(selected) {
            assert_eq!(file["case_id"], row["source_case"]);
            assert_eq!(file["size_bytes"], row["source_size_bytes"]);
            assert_eq!(file["sha256"], row["source_sha256"]);
            generic_sc_bundle::assert_semantics(file, row);
            generic_sc_bundle::assert_payload(
                &root.join(file["path"].as_str().unwrap()),
                &row["source_size_bytes"],
                &row["source_sha256"],
            );
        }
    }
}

#[test]
fn caller_named_cr_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_cr_bundle::GenericCrBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_cr_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    let oracle = generic_cr_bundle::oracle();
    let rows = oracle["cases"].as_array().unwrap();
    let assessment = &assessed["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!(
            rows.iter()
                .map(|r| format!("curated_{}_instance", r["recipe_id"].as_str().unwrap()))
                .collect::<Vec<_>>()
        )
    );
    assert!(!f.root.join("cli-output").exists());
    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 1);
    assert_eq!(generated["result"]["direct_case_count"], 1);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_cr_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_cr_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller CR must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("cli-output").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
        for root in [f.root.join("cli-output"), sdk.output_root().to_path_buf()] {
            generic_cr_bundle::assert_payload(
                &root.join(path),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }
    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 1);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 1);
    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 1);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );
    let original = Fixture::new();
    for profile in ["core"] {
        let selected = rows
            .iter()
            .filter(|r| r["profile"] == profile)
            .collect::<Vec<_>>();
        let mut extra = vec!["--seed", "1", "--parallelism", "4"];
        for row in &selected {
            extra.extend(["--case-id", row["source_case"].as_str().unwrap()]);
        }
        let name = format!("cr-original-{profile}");
        original.generate(&name, profile, &extra);
        let root = original.0.join("generated").join(&name);
        let historical: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let files = historical["files"].as_array().unwrap();
        assert_eq!(files.len(), selected.len());
        assert_eq!(historical["run"]["profile"], profile);
        for (file, row) in files.iter().zip(selected) {
            assert_eq!(file["case_id"], row["source_case"]);
            assert_eq!(file["size_bytes"], row["source_size_bytes"]);
            assert_eq!(file["sha256"], row["source_sha256"]);
            generic_cr_bundle::assert_semantics(file, row);
            generic_cr_bundle::assert_payload(
                &root.join(file["path"].as_str().unwrap()),
                &row["source_size_bytes"],
                &row["source_sha256"],
            );
        }
    }
}

#[test]
fn caller_named_mr_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_mr_bundle::GenericMrBundle::new();
    generic_mr_bundle::assert_identity(&f.identity);
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_mr_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap_or_else(|error| panic!("{}", error.diagnostic()));
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    generic_mr_bundle::assert_assessment(&assessed["result"]);
    assert!(!f.root.join("cli-output").exists());

    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 3);
    assert_eq!(generated["result"]["direct_case_count"], 1);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_mr_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");

    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_mr_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller MR must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        let cli_path = f.root.join("cli-output").join(path);
        let sdk_path = sdk.output_root().join(path);
        assert_eq!(fs::read(&cli_path).unwrap(), fs::read(&sdk_path).unwrap());
        generic_mr_bundle::assert_payload(&cli_path, &file["size_bytes"], &file["sha256"]);
        generic_mr_bundle::assert_payload(&sdk_path, &file["size_bytes"], &file["sha256"]);
    }

    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 3);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 3);

    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 3);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );

    // Preserve the accepted embedded three-file payload oracle independently of the
    // deliberately different caller-owned metadata, pixels, names, and paths above.
    let oracle = generic_mr_bundle::oracle();
    let original = Fixture::new();
    let original_name = "mr-original-core";
    original.generate(
        original_name,
        "core",
        &[
            "--seed",
            "1",
            "--parallelism",
            "4",
            "--case-id",
            oracle["source_case"].as_str().unwrap(),
        ],
    );
    let original_root = original.0.join("generated").join(original_name);
    let historical: Value =
        serde_json::from_slice(&fs::read(original_root.join("manifest.json")).unwrap()).unwrap();
    let historical_files = historical["files"].as_array().unwrap();
    let source_rows = oracle["source_files"].as_array().unwrap();
    assert_eq!(historical_files.len(), 3);
    assert_eq!(historical["run"]["profile"], "core");
    for (file, row) in historical_files.iter().zip(source_rows) {
        assert_eq!(file["case_id"], oracle["source_case"]);
        assert_eq!(file["path"], row["path"]);
        assert_eq!(file["size_bytes"], row["size_bytes"]);
        assert_eq!(file["sha256"], row["sha256"]);
        generic_mr_bundle::assert_payload(
            &original_root.join(file["path"].as_str().unwrap()),
            &row["size_bytes"],
            &row["sha256"],
        );
    }
}

#[test]
fn caller_named_us_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_us_bundle::GenericUsBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_us_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    let oracle = generic_us_bundle::oracle();
    let rows = oracle["cases"].as_array().unwrap();
    let assessment = &assessed["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!(
            rows.iter()
                .map(|r| format!("curated_{}_instance", r["recipe_id"].as_str().unwrap()))
                .collect::<Vec<_>>()
        )
    );
    assert!(!f.root.join("cli-output").exists());
    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 1);
    assert_eq!(generated["result"]["direct_case_count"], 1);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_us_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_us_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller US must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("cli-output").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
        for root in [f.root.join("cli-output"), sdk.output_root().to_path_buf()] {
            generic_us_bundle::assert_payload(
                &root.join(path),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }
    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 1);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 1);
    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 1);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );
    let original = Fixture::new();
    for profile in ["core"] {
        let selected = rows
            .iter()
            .filter(|r| r["profile"] == profile)
            .collect::<Vec<_>>();
        let mut extra = vec!["--seed", "1", "--parallelism", "4"];
        for row in &selected {
            extra.extend(["--case-id", row["source_case"].as_str().unwrap()]);
        }
        let name = format!("us-original-{profile}");
        original.generate(&name, profile, &extra);
        let root = original.0.join("generated").join(&name);
        let historical: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let files = historical["files"].as_array().unwrap();
        assert_eq!(files.len(), selected.len());
        assert_eq!(historical["run"]["profile"], profile);
        for (file, row) in files.iter().zip(selected) {
            assert_eq!(file["case_id"], row["source_case"]);
            assert_eq!(file["size_bytes"], row["source_size_bytes"]);
            assert_eq!(file["sha256"], row["source_sha256"]);
            generic_us_bundle::assert_semantics(file, row);
            generic_us_bundle::assert_payload(
                &root.join(file["path"].as_str().unwrap()),
                &row["source_size_bytes"],
                &row["source_sha256"],
            );
        }
    }
}

#[test]
fn caller_owned_us_multiframe_cli_sdk_and_report_are_identical() {
    let f = generic_us_multiframe_bundle::GenericUsMultiframeBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let cli = command(&f.args("generate", Some("cli-output")));
    valid("generation-result-v3.schema.json", &cli["result"]);
    assert_eq!(cli["result"]["validation_status"], "passed");
    let cli_manifest_bytes = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let cli_manifest: Value = serde_json::from_slice(&cli_manifest_bytes).unwrap();
    valid("manifest-v2.schema.json", &cli_manifest);
    generic_us_multiframe_bundle::assert_manifest(&cli_manifest);
    generic_us_multiframe_bundle::assert_payload(
        &f.root.join("cli-output/independent/caller-cine.dcm"),
    );

    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_us_multiframe_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller-owned US multiframe must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), cli_manifest);
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_manifest_bytes
    );
    assert_eq!(
        fs::read(sdk.output_root().join("independent/caller-cine.dcm")).unwrap(),
        fs::read(f.root.join("cli-output/independent/caller-cine.dcm")).unwrap()
    );
    generic_us_multiframe_bundle::assert_payload(
        &sdk.output_root().join("independent/caller-cine.dcm"),
    );

    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 1);
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());

    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    generic_us_multiframe_bundle::assert_report(&report["result"]["report"]);
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );

    let payload_path = f.root.join("cli-output/independent/caller-cine.dcm");
    let mut padded = fs::read(&payload_path).unwrap();
    assert_eq!(
        padded.last(),
        Some(&0),
        "odd Pixel Data must have a zero pad byte"
    );
    *padded.last_mut().unwrap() = 0x7f;
    fs::write(&payload_path, &padded).unwrap();
    let mut pad_corrupt_manifest = cli_manifest.clone();
    pad_corrupt_manifest["files"][0]["sha256"] = synth_dicom_gen::sha256_hex(&padded).into();
    fs::write(
        f.root.join("cli-output/manifest.json"),
        serde_json::to_vec(&pad_corrupt_manifest).unwrap(),
    )
    .unwrap();
    let pad_validation = product
        .validate(ValidateRequest::new(f.root.join("cli-output")))
        .unwrap();
    assert!(!pad_validation.is_valid());
    assert!(
        pad_validation
            .failures()
            .iter()
            .any(|failure| failure.contains("pixel_padding")),
        "{:?}",
        pad_validation.failures()
    );
}

#[test]
fn caller_owned_nm_multiframe_cli_sdk_and_report_are_identical() {
    let f = generic_nm_multiframe_bundle::GenericNmMultiframeBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let capabilities = command(&f.args("capabilities", None));
    assert_eq!(
        capabilities["result"]["loaded_corpus"]["assessment"]["selector"]["case_ids"],
        json!(["caller/acquisition/rotating-study"])
    );

    let cli = command(&f.args("generate", Some("cli-output")));
    valid("generation-result-v3.schema.json", &cli["result"]);
    assert_eq!(cli["result"]["validation_status"], "passed");
    let cli_manifest_bytes = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let cli_manifest: Value = serde_json::from_slice(&cli_manifest_bytes).unwrap();
    valid("manifest-v2.schema.json", &cli_manifest);
    generic_nm_multiframe_bundle::assert_manifest(&cli_manifest);
    generic_nm_multiframe_bundle::assert_payload(
        &f.root.join("cli-output/caller-results/orbit-counts.dcm"),
    );

    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_nm_multiframe_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller-owned NM multiframe must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), cli_manifest);
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_manifest_bytes
    );
    assert_eq!(
        fs::read(sdk.output_root().join("caller-results/orbit-counts.dcm")).unwrap(),
        fs::read(f.root.join("cli-output/caller-results/orbit-counts.dcm")).unwrap()
    );

    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert!(
        product
            .validate(ValidateRequest::new(sdk.output_root()))
            .unwrap()
            .is_valid()
    );

    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    generic_nm_multiframe_bundle::assert_report(&report["result"]["report"]);
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );

    let mut tampered = cli_manifest;
    tampered["files"][0]["expected_semantics"]["pixel_spacing_mm"] = json!([9.0, 9.0]);
    fs::write(
        f.root.join("cli-output/manifest.json"),
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .unwrap();
    let validation = product
        .validate(ValidateRequest::new(f.root.join("cli-output")))
        .unwrap();
    assert!(!validation.is_valid());
    assert!(
        validation
            .failures()
            .iter()
            .any(|failure| failure.contains("nm_pixel_spacing_type2"))
    );
}

#[test]
fn caller_owned_nonsquare_sc_cli_sdk_strict_and_report_are_identical() {
    let f = generic_nonsquare_sc_bundle::GenericNonsquareScBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let capabilities = command(&f.args("capabilities", None));
    assert_eq!(
        capabilities["result"]["loaded_corpus"]["assessment"]["selector"]["case_ids"],
        json!(["caller/geometry/independent-rectangles"])
    );

    let cli = command(&f.args("generate", Some("cli-output")));
    valid("generation-result-v3.schema.json", &cli["result"]);
    assert_eq!(cli["result"]["validation_status"], "passed");
    assert_eq!(cli["result"]["emitted_file_count"], 2);
    let cli_manifest_bytes = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let cli_manifest: Value = serde_json::from_slice(&cli_manifest_bytes).unwrap();
    valid("manifest-v2.schema.json", &cli_manifest);
    generic_nonsquare_sc_bundle::assert_manifest(&cli_manifest);
    for (file, row) in cli_manifest["files"].as_array().unwrap().iter().zip(
        generic_nonsquare_sc_bundle::oracle()["caller"]["files"]
            .as_array()
            .unwrap(),
    ) {
        generic_nonsquare_sc_bundle::assert_payload(
            &f.root
                .join("cli-output")
                .join(file["path"].as_str().unwrap()),
            row,
        );
    }

    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_nonsquare_sc_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller-owned nonsquare SC must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), cli_manifest);
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_manifest_bytes
    );
    for file in cli_manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(sdk.output_root().join(path)).unwrap(),
            fs::read(f.root.join("cli-output").join(path)).unwrap()
        );
    }

    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 2);
    assert!(
        product
            .validate(ValidateRequest::new(sdk.output_root()))
            .unwrap()
            .is_valid()
    );

    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    generic_nonsquare_sc_bundle::assert_report(&report["result"]["report"]);
    generic_nonsquare_sc_bundle::assert_report_mutations_fail(&report["result"]["report"]);
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );

    let mut tampered = cli_manifest;
    tampered["files"][0]["expected_nonsquare_spacing"]["pixel_spacing"]["lexical_value"] =
        json!("9.0\\4.5");
    fs::write(
        f.root.join("cli-output/manifest.json"),
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .unwrap();
    let validation = product
        .validate(ValidateRequest::new(f.root.join("cli-output")))
        .unwrap();
    assert!(!validation.is_valid());
    assert!(
        validation
            .failures()
            .iter()
            .any(|failure| failure.contains("nonsquare_pixel_spacing")),
        "{:?}",
        validation.failures()
    );
}

#[test]
fn caller_owned_ct_geometry_cli_sdk_strict_and_report_are_identical() {
    let f = generic_ct_geometry_bundle::GenericCtGeometryBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let capabilities = command(&f.args("capabilities", None));
    valid(
        "capabilities-result-v3.schema.json",
        &capabilities["result"],
    );
    let sdk_capabilities = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_ct_geometry_bundle::selector())
                .with_seed(13)
                .with_parallelism(3),
        )
        .unwrap();
    assert_eq!(
        capabilities["result"],
        serde_json::to_value(sdk_capabilities).unwrap()
    );
    let assessment = &capabilities["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(
        assessment["artifact_ids"],
        json!([
            "curated_caller_angled_order_study_diagnostic_origin",
            "curated_caller_angled_order_study_diagnostic_middle",
            "curated_caller_angled_order_study_diagnostic_high",
            "curated_caller_angled_order_study_late_scout_middle",
            "curated_caller_angled_order_study_scout_origin",
            "curated_caller_angled_order_study_scout_high"
        ])
    );
    assert!(!f.root.join("cli-output").exists());

    let cli = command(&f.args("generate", Some("cli-output")));
    valid("generation-result-v3.schema.json", &cli["result"]);
    assert_eq!(cli["result"]["validation_status"], "passed");
    assert_eq!(cli["result"]["emitted_file_count"], 6);
    let cli_manifest_bytes = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let cli_manifest: Value = serde_json::from_slice(&cli_manifest_bytes).unwrap();
    valid("manifest-v2.schema.json", &cli_manifest);
    generic_ct_geometry_bundle::assert_manifest(&cli_manifest);
    generic_ct_geometry_bundle::assert_manifest_rescale_mutations_fail(&cli_manifest);
    generic_ct_geometry_bundle::assert_manifest_geometry_mutations_fail(&cli_manifest);
    let oracle = generic_ct_geometry_bundle::oracle();
    for (file, row) in cli_manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .zip(oracle["caller"]["files"].as_array().unwrap())
    {
        generic_ct_geometry_bundle::assert_payload(
            &f.root
                .join("cli-output")
                .join(file["path"].as_str().unwrap()),
            row,
        );
    }

    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_ct_geometry_bundle::selector(),
            )
            .with_seed(13)
            .with_parallelism(3),
        )
        .unwrap()
    else {
        panic!("caller-owned CT geometry must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), cli_manifest);
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_manifest_bytes
    );
    for file in cli_manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(sdk.output_root().join(path)).unwrap(),
            fs::read(f.root.join("cli-output").join(path)).unwrap()
        );
    }

    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 6);
    assert!(
        product
            .validate(ValidateRequest::new(sdk.output_root()))
            .unwrap()
            .is_valid()
    );

    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    generic_ct_geometry_bundle::assert_report(&report["result"]["report"]);
    generic_ct_geometry_bundle::assert_report_mutations_fail(&report["result"]["report"]);
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );

    let mut tampered = cli_manifest;
    tampered["files"][0]["expected_geometry"]["geometric_order_index"] = json!(99);
    fs::write(
        f.root.join("cli-output/manifest.json"),
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .unwrap();
    let validation = product
        .validate(ValidateRequest::new(f.root.join("cli-output")))
        .unwrap();
    assert!(!validation.is_valid());
    assert!(
        validation
            .failures()
            .iter()
            .any(|failure| failure.contains("geometry_order: actual rank")),
        "{:?}",
        validation.failures()
    );

    let negative = generic_ct_geometry_bundle::GenericCtGeometryBundle::new();
    negative.rewrite_rescale("-2", "10");
    let GenerateCorpusOutcome::Published(negative_run) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &negative.descriptor,
                &negative.members,
                negative.root.join("negative-output"),
                generic_ct_geometry_bundle::selector(),
            )
            .with_seed(13)
            .with_parallelism(3),
        )
        .unwrap()
    else {
        panic!("negative-slope caller CT geometry must publish")
    };
    let negative_manifest = negative_run.manifest().deserialize::<Value>().unwrap();
    assert_eq!(
        negative_manifest["files"][0]["expected_semantics"]["rescale"],
        json!({
            "intercept":"10",
            "slope":"-2",
            "type":"HU",
            "output_min":-3790,
            "output_max":2010
        }),
        "negative slope must transform both endpoints and reorder the output bounds"
    );

    let heterogeneous = generic_ct_geometry_bundle::GenericCtGeometryBundle::new();
    heterogeneous.rewrite_second_series_without_sorting_conflict();
    let GenerateCorpusOutcome::Published(heterogeneous_run) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &heterogeneous.descriptor,
                &heterogeneous.members,
                heterogeneous.root.join("heterogeneous-output"),
                generic_ct_geometry_bundle::selector(),
            )
            .with_seed(13)
            .with_parallelism(3),
        )
        .unwrap()
    else {
        panic!("heterogeneous per-series sorting conflicts must publish")
    };
    let heterogeneous_manifest = heterogeneous_run.manifest().deserialize::<Value>().unwrap();
    let conflicts = heterogeneous_manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| {
            (
                file["expected_series_organization"]["series_ordinal"]
                    .as_u64()
                    .unwrap(),
                file["expected_geometry"]["sorting_conflict_expected"]
                    .as_bool()
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        conflicts,
        vec![
            (1, true),
            (1, true),
            (1, true),
            (2, false),
            (2, false),
            (2, false)
        ]
    );
    let heterogeneous_report = product
        .report(ReportRequest::new(heterogeneous_run.output_root()))
        .unwrap()
        .deserialize::<Value>()
        .unwrap();
    assert_eq!(
        heterogeneous_report["coverage_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (
                row["series_ordinal"].as_u64().unwrap(),
                row["geometry_sorting_conflict_expected"].as_bool().unwrap(),
            ))
            .collect::<Vec<_>>(),
        conflicts,
        "report2 must retain per-series conflict facts rather than flattening the aggregate"
    );
}

#[test]
fn caller_owned_timezone_sc_cli_sdk_strict_and_report_are_identical() {
    let f = generic_timezone_sc_bundle::GenericTimezoneScBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        serde_json::from_slice::<Value>(&raw.stdout).unwrap()
    };
    let product = DicomTestSuite::embedded().unwrap();
    let capabilities = command(&f.args("capabilities", None));
    assert_eq!(
        capabilities["result"]["loaded_corpus"]["assessment"]["selector"]["case_ids"],
        json!(["caller/temporal/offset-extrema"])
    );
    let cli = command(&f.args("generate", Some("cli-output")));
    assert_eq!(cli["result"]["validation_status"], "passed");
    assert_eq!(cli["result"]["emitted_file_count"], 2);
    let manifest_bytes = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["case_id"], "caller/temporal/offset-extrema");
    let positive = files
        .iter()
        .find(|file| file["expected_metadata"]["temporal"]["boundary_id"] == "positive_max")
        .unwrap();
    let negative = files
        .iter()
        .find(|file| file["expected_metadata"]["temporal"]["boundary_id"] == "negative_min")
        .unwrap();
    assert_eq!(positive["path"], "caller/clocks/east.dcm");
    assert_eq!(negative["path"], "caller/clocks/west.dcm");
    for file in files {
        assert_eq!(file["frame_of_reference_uid"], Value::Null);
        assert_eq!(file["references"], json!([]));
    }

    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_timezone_sc_bundle::GenericTimezoneScBundle::selector(),
            )
            .with_seed(41),
        )
        .unwrap()
    else {
        panic!("caller-owned timezone pair must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    for file in files {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(sdk.output_root().join(path)).unwrap(),
            fs::read(f.root.join("cli-output").join(path)).unwrap()
        );
    }
    assert!(
        product
            .validate(ValidateRequest::new(sdk.output_root()))
            .unwrap()
            .is_valid()
    );

    let report = command(&[
        "report".into(),
        "cli-output".into(),
        "--format".into(),
        "json".into(),
        "--cli-api".into(),
        "1.0.0".into(),
    ]);
    let report = &report["result"]["report"];
    valid("coverage-report-v2.schema.json", report);
    let rows = report["coverage_matrix"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let projected = rows
        .iter()
        .map(|row| {
            json!({
                "boundary": row["metadata_temporal_boundary_id"],
                "offset": row["metadata_timezone_offset_from_utc"],
                "date": row["metadata_da_values"],
                "time": row["metadata_tm_values"],
                "date_time": row["metadata_dt_values"],
                "normalized": row["metadata_temporal_normalized_utc"]
            })
        })
        .collect::<Vec<_>>();
    assert!(projected.contains(&json!({
        "boundary":"positive_max", "offset":"+1400", "date":"20321231",
        "time":"235958.123456", "date_time":"20321231235958.123456+1400",
        "normalized":"2032-12-31T09:59:58.123456Z"
    })));
    assert!(projected.contains(&json!({
        "boundary":"negative_min", "offset":"-1200", "date":"20330101",
        "time":"000001.654321", "date_time":"20330101000001.654321-1200",
        "normalized":"2033-01-01T12:00:01.654321Z"
    })));
    assert_eq!(
        report["grouped_coverage"]["metadata_temporal_boundary_ids"]["positive_max"],
        1
    );
    assert_eq!(
        report["grouped_coverage"]["metadata_temporal_boundary_ids"]["negative_min"],
        1
    );
    assert_eq!(
        report["grouped_coverage"]["metadata_timezone_offsets_from_utc"]["+1400"],
        1
    );
    assert_eq!(
        report["grouped_coverage"]["metadata_timezone_offsets_from_utc"]["-1200"],
        1
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );

    let mut tampered = manifest;
    tampered["files"][0]["expected_metadata"]["temporal"]["combined_da_tm_utc"] =
        json!("2000-01-01T00:00:00.000000Z");
    fs::write(
        f.root.join("cli-output/manifest.json"),
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .unwrap();
    let validation = product
        .validate(ValidateRequest::new(f.root.join("cli-output")))
        .unwrap();
    assert!(!validation.is_valid());
    assert!(
        validation
            .failures()
            .iter()
            .any(|failure| failure.contains("metadata_temporal_combined_utc"))
    );
}

#[test]
fn caller_named_pet_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_pet_bundle::GenericPetBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_pet_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    let oracle = generic_pet_bundle::oracle();
    let rows = oracle["cases"].as_array().unwrap();
    let assessment = &assessed["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!(
            rows.iter()
                .map(|r| format!("curated_{}_instance", r["recipe_id"].as_str().unwrap()))
                .collect::<Vec<_>>()
        )
    );
    assert!(!f.root.join("cli-output").exists());
    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 1);
    assert_eq!(generated["result"]["direct_case_count"], 1);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_pet_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_pet_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller PET must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("cli-output").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
        for root in [f.root.join("cli-output"), sdk.output_root().to_path_buf()] {
            generic_pet_bundle::assert_payload(
                &root.join(path),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }
    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 1);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 1);
    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 1);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );
    let original = Fixture::new();
    for profile in ["core"] {
        let selected = rows
            .iter()
            .filter(|r| r["profile"] == profile)
            .collect::<Vec<_>>();
        let mut extra = vec!["--seed", "1", "--parallelism", "4"];
        for row in &selected {
            extra.extend(["--case-id", row["source_case"].as_str().unwrap()]);
        }
        let name = format!("pet-original-{profile}");
        original.generate(&name, profile, &extra);
        let root = original.0.join("generated").join(&name);
        let historical: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let files = historical["files"].as_array().unwrap();
        assert_eq!(files.len(), selected.len());
        assert_eq!(historical["run"]["profile"], profile);
        for (file, row) in files.iter().zip(selected) {
            assert_eq!(file["case_id"], row["source_case"]);
            assert_eq!(file["size_bytes"], row["source_size_bytes"]);
            assert_eq!(file["sha256"], row["source_sha256"]);
            generic_pet_bundle::assert_semantics(file, row);
            generic_pet_bundle::assert_payload(
                &root.join(file["path"].as_str().unwrap()),
                &row["source_size_bytes"],
                &row["source_sha256"],
            );
        }
    }
}

#[test]
fn caller_named_xa_xrf_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_xa_xrf_bundle::GenericXaXrfBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_xa_xrf_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    let oracle = generic_xa_xrf_bundle::oracle();
    let rows = oracle["cases"].as_array().unwrap();
    let assessment = &assessed["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!(
            rows.iter()
                .map(|r| format!("curated_{}_instance", r["recipe_id"].as_str().unwrap()))
                .collect::<Vec<_>>()
        )
    );
    assert!(!f.root.join("cli-output").exists());
    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 2);
    assert_eq!(generated["result"]["direct_case_count"], 2);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_xa_xrf_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_xa_xrf_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller XA/XRF must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("cli-output").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
        for root in [f.root.join("cli-output"), sdk.output_root().to_path_buf()] {
            generic_xa_xrf_bundle::assert_payload(
                &root.join(path),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }
    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 2);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 2);
    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 2);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );
    let original = Fixture::new();
    for profile in ["core"] {
        let selected = rows
            .iter()
            .filter(|r| r["profile"] == profile)
            .collect::<Vec<_>>();
        let mut extra = vec!["--seed", "1", "--parallelism", "4"];
        for row in &selected {
            extra.extend(["--case-id", row["source_case"].as_str().unwrap()]);
        }
        let name = format!("xa-xrf-original-{profile}");
        original.generate(&name, profile, &extra);
        let root = original.0.join("generated").join(&name);
        let historical: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let files = historical["files"].as_array().unwrap();
        assert_eq!(files.len(), selected.len());
        assert_eq!(historical["run"]["profile"], profile);
        for (file, row) in files.iter().zip(selected) {
            assert_eq!(file["case_id"], row["source_case"]);
            assert_eq!(file["size_bytes"], row["source_size_bytes"]);
            assert_eq!(file["sha256"], row["source_sha256"]);
            generic_xa_xrf_bundle::assert_semantics(file, row);
            generic_xa_xrf_bundle::assert_payload(
                &root.join(file["path"].as_str().unwrap()),
                &row["source_size_bytes"],
                &row["source_sha256"],
            );
        }
    }
}

#[test]
fn caller_named_vl_photo_cli_is_sdk_identical_strictly_valid_and_reported() {
    let f = generic_vl_photo_bundle::GenericVlPhotoBundle::new();
    let command = |args: &[String]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&f.root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        assert!(raw.stderr.is_empty());
        let value: Value = serde_json::from_slice(&raw.stdout).unwrap();
        valid("cli-success-envelope.schema.json", &value);
        value
    };
    let product = DicomTestSuite::embedded().unwrap();
    let assessed = command(&f.selection_args("capabilities"));
    valid("capabilities-result-v3.schema.json", &assessed["result"]);
    let sdk_assessment = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&f.descriptor, &f.members)
                .with_selection(generic_vl_photo_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    assert_eq!(
        assessed["result"],
        serde_json::to_value(sdk_assessment).unwrap()
    );
    let oracle = generic_vl_photo_bundle::oracle();
    let rows = oracle["cases"].as_array().unwrap();
    let assessment = &assessed["result"]["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!(
            rows.iter()
                .map(|r| format!("curated_{}_instance", r["recipe_id"].as_str().unwrap()))
                .collect::<Vec<_>>()
        )
    );
    assert!(!f.root.join("cli-output").exists());
    let mut args = f.selection_args("generate");
    args.extend(["--out", "cli-output", "--cli-api", "1.0.0"].map(str::to_owned));
    let generated = command(&args);
    valid("generation-result-v3.schema.json", &generated["result"]);
    assert_eq!(generated["result"]["outcome"], "published");
    assert_eq!(generated["result"]["emitted_file_count"], 2);
    assert_eq!(generated["result"]["direct_case_count"], 2);
    assert_eq!(generated["result"]["dependency_case_count"], 0);
    assert_eq!(generated["result"]["validation_status"], "passed");
    let cli_raw = fs::read(f.root.join("cli-output/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&cli_raw).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    generic_vl_photo_bundle::assert_manifest(&manifest, &f.identity);
    f.assert_closure("cli-output");
    let GenerateCorpusOutcome::Published(sdk) = product
        .generate_corpus(
            GenerateCorpusRequest::from_file(
                &f.descriptor,
                &f.members,
                f.root.join("sdk-output"),
                generic_vl_photo_bundle::selector(),
            )
            .with_seed(1)
            .with_parallelism(4),
        )
        .unwrap()
    else {
        panic!("caller VL photographic must publish")
    };
    assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
    assert_eq!(
        sdk.corpus_plan_sha256(),
        generated["result"]["corpus_plan_sha256"]
    );
    assert_eq!(
        fs::read(sdk.output_root().join("manifest.json")).unwrap(),
        cli_raw
    );
    f.assert_closure("sdk-output");
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("cli-output").join(path)).unwrap(),
            fs::read(sdk.output_root().join(path)).unwrap()
        );
        for root in [f.root.join("cli-output"), sdk.output_root().to_path_buf()] {
            generic_vl_photo_bundle::assert_payload(
                &root.join(path),
                &file["size_bytes"],
                &file["sha256"],
            );
        }
    }
    let validation = command(&["validate", "cli-output", "--format", "json"].map(str::to_owned));
    valid("validation-result.schema.json", &validation["result"]);
    assert_eq!(validation["result"]["valid"], true);
    assert_eq!(validation["result"]["files_checked"], 2);
    assert_eq!(validation["result"]["failures"], json!([]));
    let sdk_validation = product
        .validate(ValidateRequest::new(sdk.output_root()))
        .unwrap();
    assert!(sdk_validation.is_valid());
    assert_eq!(sdk_validation.files_checked(), 2);
    let report = command(
        &[
            "report",
            "cli-output",
            "--format",
            "json",
            "--cli-api",
            "1.0.0",
        ]
        .map(str::to_owned),
    );
    valid("report-result-v2.schema.json", &report["result"]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    let report = &report["result"]["report"];
    assert_eq!(report["source_manifest"], manifest);
    assert_eq!(report["summary"]["emitted_files"], 2);
    assert_eq!(
        report["evidence"],
        json!({"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false})
    );
    assert_eq!(
        product
            .report(ReportRequest::new(sdk.output_root()))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        *report
    );
    let original = Fixture::new();
    for profile in ["core"] {
        let selected = rows
            .iter()
            .filter(|r| r["profile"] == profile)
            .collect::<Vec<_>>();
        let mut extra = vec!["--seed", "1", "--parallelism", "4"];
        for row in &selected {
            extra.extend(["--case-id", row["source_case"].as_str().unwrap()]);
        }
        let name = format!("vl-photo-original-{profile}");
        original.generate(&name, profile, &extra);
        let root = original.0.join("generated").join(&name);
        let historical: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let files = historical["files"].as_array().unwrap();
        assert_eq!(files.len(), selected.len());
        assert_eq!(historical["run"]["profile"], profile);
        for (file, row) in files.iter().zip(selected) {
            assert_eq!(file["case_id"], row["source_case"]);
            assert_eq!(file["size_bytes"], row["source_size_bytes"]);
            assert_eq!(file["sha256"], row["source_sha256"]);
            generic_vl_photo_bundle::assert_semantics(file, row);
            generic_vl_photo_bundle::assert_payload(
                &root.join(file["path"].as_str().unwrap()),
                &row["source_size_bytes"],
                &row["source_sha256"],
            );
        }
    }
}

#[test]
fn caller_owned_enhanced_cli_sdk_strict_report_and_repeat_are_identical() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "generic-enhanced-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&root).unwrap();
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-enhanced-corpus");
    let descriptor = root.join("definition.json");
    fs::copy(source.join("definition.json"), &descriptor).unwrap();
    let definition: Value = serde_json::from_slice(&fs::read(&descriptor).unwrap()).unwrap();
    let members = root.join("members");
    for reference in std::iter::once(&definition["registry"])
        .chain(
            definition["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| &row["recipe"]),
        )
        .chain(definition["evidence"].as_array().unwrap().iter())
    {
        let relative = reference["path"].as_str().unwrap();
        let destination = members.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(source.join("members").join(relative), destination).unwrap();
    }
    let command = |args: &[&str]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&root)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        serde_json::from_slice::<Value>(&raw.stdout).unwrap()
    };
    DicomTestSuite::embedded()
        .unwrap()
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&descriptor, &members).with_selection(
                CorpusSelector::Profile {
                    profile: "core".into(),
                    include_stress: false,
                },
            ),
        )
        .unwrap();
    let capabilities = command(&[
        "capabilities",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--format",
        "json",
    ]);
    assert_eq!(
        capabilities["result"]["loaded_corpus"]["assessment"]["publication"],
        "not_run"
    );
    let cli = command(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--seed",
        "19",
        "--parallelism",
        "3",
        "--out",
        "cli-output",
        "--format",
        "json",
    ]);
    assert_eq!(cli["result"]["emitted_file_count"], 7);
    assert_eq!(cli["result"]["validation_status"], "passed");
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("cli-output/manifest.json")).unwrap()).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    let misleading = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == "stress/enhanced-ct/many_frames")
        .unwrap();
    assert_eq!(misleading["profile_membership"], json!(["core"]));
    assert!(
        manifest["qualifications"].is_null()
            || manifest["qualifications"]
                .as_array()
                .is_some_and(Vec::is_empty)
    );
    assert!(
        !serde_json::to_string(&manifest)
            .unwrap()
            .contains("Full-scale enhanced CT resource behavior")
    );
    assert!(
        !serde_json::to_string(&manifest)
            .unwrap()
            .contains("\"qualification_scale\":\"reduced\"")
    );

    for file in manifest["files"].as_array().unwrap() {
        let case = definition["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["case_id"] == file["case_id"])
            .unwrap();
        let recipe: Value = serde_json::from_slice(
            &fs::read(members.join(case["recipe"]["path"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let declared = recipe["dicom"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["output"]["path"] == file["path"])
            .unwrap();
        let obj =
            dicom_object::open_file(root.join("cli-output").join(file["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(
            obj.element_by_name("PatientName")
                .unwrap()
                .to_str()
                .unwrap(),
            "Sample^Caller"
        );
        assert_eq!(
            obj.element_by_name("PatientID").unwrap().to_str().unwrap(),
            "CALLER-TEST"
        );
        assert_eq!(
            obj.element_by_name("Manufacturer")
                .unwrap()
                .to_str()
                .unwrap(),
            "Fixture Laboratory"
        );
        assert_eq!(
            obj.element_by_name("Columns")
                .unwrap()
                .to_int::<u16>()
                .unwrap(),
            3
        );
        let expected: Vec<u16> = declared["parameters"]["pixels"]["stored_values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u16)
            .collect();
        assert_eq!(
            obj.element_by_name("PixelData")
                .unwrap()
                .to_multi_int::<u16>()
                .unwrap(),
            expected
        );
        assert_eq!(
            file["recipe"]["recipe_parameters"]["patient_study"],
            recipe["provider_parameters"]["common"]["patient_study"]
        );
    }
    let product = DicomTestSuite::embedded().unwrap();
    for output in ["sdk-output", "repeat-output"] {
        let GenerateCorpusOutcome::Published(sdk) = product
            .generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join(output),
                    CorpusSelector::Profile {
                        profile: "core".into(),
                        include_stress: false,
                    },
                )
                .with_seed(19)
                .with_parallelism(3),
            )
            .unwrap()
        else {
            panic!("must publish")
        };
        assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
        for file in manifest["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            assert!(path.starts_with("caller-payload/"));
            assert_eq!(
                fs::read(root.join("cli-output").join(path)).unwrap(),
                fs::read(sdk.output_root().join(path)).unwrap()
            );
        }
        assert!(
            product
                .validate(ValidateRequest::new(sdk.output_root()))
                .unwrap()
                .is_valid()
        );
    }
    let validation = command(&["validate", "cli-output", "--format", "json"]);
    assert_eq!(validation["result"]["valid"], true);
    let report = command(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    assert_eq!(
        product
            .report(ReportRequest::new(root.join("sdk-output")))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );
    for (pointer, replacement) in [
        (
            "/recipe/recipe_parameters/enhanced_capability_version",
            json!("9.0.0"),
        ),
        (
            "/recipe/recipe_parameters/patient_study/patient_id",
            json!("WRONG-PATIENT"),
        ),
        ("/image/frames", json!(999)),
    ] {
        let mut changed = manifest.clone();
        *changed["files"][0].pointer_mut(pointer).unwrap() = replacement;
        fs::write(
            root.join("cli-output/manifest.json"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        let reopened = product.validate(ValidateRequest::new(root.join("cli-output")));
        assert!(
            reopened.is_err() || !reopened.unwrap().is_valid(),
            "must reject altered {pointer}"
        );
    }
    let pet_index = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .position(|file| file.get("expected_enhanced_pet").is_some())
        .unwrap();
    let mut changed = manifest.clone();
    changed["files"][pet_index]["expected_enhanced_pet"]["nonclaims"]["suv"] = json!(true);
    fs::write(
        root.join("cli-output/manifest.json"),
        serde_json::to_vec(&changed).unwrap(),
    )
    .unwrap();
    let rejected = product.validate(ValidateRequest::new(root.join("cli-output")));
    assert!(rejected.is_err() || !rejected.unwrap().is_valid());
    fs::write(
        root.join("cli-output/manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let payload = root
        .join("cli-output")
        .join(manifest["files"][0]["path"].as_str().unwrap());
    let original_bytes = fs::read(&payload).unwrap();
    let mut altered_bytes = original_bytes.clone();
    *altered_bytes.last_mut().unwrap() ^= 1;
    fs::write(&payload, altered_bytes).unwrap();
    assert!(
        !product
            .validate(ValidateRequest::new(root.join("cli-output")))
            .unwrap()
            .is_valid()
    );
    fs::write(&payload, original_bytes).unwrap();
    // Coordinate the declared contract and the actual RWVM field, including
    // the file digest, so only semantic pixel-range coverage can reject this.
    let pet_payload = root
        .join("cli-output")
        .join(manifest["files"][pet_index]["path"].as_str().unwrap());
    let pet_original = fs::read(&pet_payload).unwrap();
    let mut pet_altered = pet_original.clone();
    let tag = dicom_dictionary_std::tags::REAL_WORLD_VALUE_LAST_VALUE_MAPPED;
    let marker = [
        tag.0.to_le_bytes().as_slice(),
        tag.1.to_le_bytes().as_slice(),
        b"US",
        &[2, 0],
    ]
    .concat();
    let matches = pet_altered
        .windows(marker.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == marker).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    let value_offset = matches[0] + marker.len();
    assert_eq!(
        &pet_altered[value_offset..value_offset + 2],
        &511_u16.to_le_bytes()
    );
    pet_altered[value_offset..value_offset + 2].copy_from_slice(&10_u16.to_le_bytes());
    let mut coordinated = manifest.clone();
    for contract in [
        "expected_enhanced_pet",
        "recipe/recipe_parameters/enhanced_pet",
    ] {
        *coordinated["files"][pet_index]
            .pointer_mut(&format!(
                "/{contract}/real_world_value_mapping/last_value_mapped"
            ))
            .unwrap() = json!(10);
    }
    coordinated["files"][pet_index]["sha256"] = json!(synth_dicom_gen::sha256_hex(&pet_altered));
    fs::write(&pet_payload, pet_altered).unwrap();
    fs::write(
        root.join("cli-output/manifest.json"),
        serde_json::to_vec(&coordinated).unwrap(),
    )
    .unwrap();
    let reopened = product
        .validate(ValidateRequest::new(root.join("cli-output")))
        .unwrap();
    assert!(!reopened.is_valid());
    assert!(
        reopened
            .failures()
            .iter()
            .any(|failure| failure.contains("enhanced_pet_rwvm_pixel_coverage")),
        "{:?}",
        reopened.failures()
    );
    fs::write(&pet_payload, pet_original).unwrap();
    fs::write(
        root.join("cli-output/manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let recipe: Value = serde_json::from_slice(
        &fs::read(source.join("members/cases/recipes/acquisition-2.json")).unwrap(),
    )
    .unwrap();
    let mut oversized_common = recipe["provider_parameters"]["common"].clone();
    oversized_common["rows"] = json!(65535);
    oversized_common["columns"] = json!(65535);
    for (case_index, pointer, replacement) in [
        (
            1,
            "/dicom/artifacts/0/parameters",
            json!({"frames":{"source":"axial_linear","frame_count":4294967295_u64,"start_z":0.0,"spacing":1.0,"first_dimension_index":1},"pixels":{"source":"modulo_ramp","modulus":32}}),
        ),
        (
            1,
            "/dicom/artifacts/0/parameters",
            json!({"frames":{"source":"axial_linear","frame_count":2,"start_z":0.0,"spacing":1.0,"first_dimension_index":4294967295_u64},"pixels":{"source":"modulo_ramp","modulus":32}}),
        ),
        (1, "/provider_parameters/common", oversized_common),
        (1, "/provider_parameters/stress", json!(true)),
        (0, "/case_recipe_schema_version", json!("0.1.0")),
        (
            0,
            "/provider_parameters/common/patient_study",
            json!({"patient_id":"incomplete"}),
        ),
        (
            0,
            "/dicom/artifacts/0/parameters/concatenation_frame_offset_number",
            json!(99),
        ),
        (1, "/provider_parameters/pixel_spacing", json!("0\\1")),
        (
            2,
            "/dicom/artifacts/0/parameters/frames/values/1/dimension_index_value",
            json!(1),
        ),
        (
            2,
            "/dicom/artifacts/0/parameters/in_concatenation_number",
            json!(1),
        ),
        (
            5,
            "/provider_parameters/quantitation",
            json!({"first_value_mapped":0}),
        ),
        (
            5,
            "/provider_parameters/quantitation/last_value_mapped",
            json!(1),
        ),
        (
            5,
            "/dicom/artifacts/0/parameters/temporal_position_indices",
            json!([1]),
        ),
    ] {
        let mut changed_definition = definition.clone();
        let relative = definition["cases"][case_index]["recipe"]["path"]
            .as_str()
            .unwrap();
        let original = fs::read(source.join("members").join(relative)).unwrap();
        let mut recipe: Value = serde_json::from_slice(&original).unwrap();
        if let Some(value) = recipe.pointer_mut(pointer) {
            *value = replacement;
        } else {
            recipe["dicom"]["artifacts"][0]["parameters"]["in_concatenation_number"] = replacement;
        }
        if pointer == "/provider_parameters/common" {
            recipe["dicom"]["artifacts"][0]["parameters"]["pixels"] =
                json!({"source":"modulo_ramp","modulus":32});
        }
        let bytes = serde_json::to_vec(&recipe).unwrap();
        fs::write(members.join(relative), &bytes).unwrap();
        changed_definition["cases"][case_index]["recipe"]["size_bytes"] = json!(bytes.len());
        changed_definition["cases"][case_index]["recipe"]["sha256"] =
            json!(synth_dicom_gen::sha256_hex(&bytes));
        fs::write(
            &descriptor,
            serde_json::to_vec(&changed_definition).unwrap(),
        )
        .unwrap();
        assert!(
            product
                .capabilities_with_corpus(
                    InspectCorpusRequest::from_file(&descriptor, &members).with_selection(
                        CorpusSelector::Profile {
                            profile: "core".into(),
                            include_stress: false
                        }
                    )
                )
                .is_err(),
            "must reject {pointer}"
        );
        fs::write(members.join(relative), original).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn caller_owned_sc_integer_cli_sdk_strict_report_and_repeat_are_identical() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "generic-sc-integer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&root).unwrap();
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-sc-integer-corpus");
    let descriptor = root.join("definition.json");
    fs::copy(source.join("definition.json"), &descriptor).unwrap();
    let definition: Value = serde_json::from_slice(&fs::read(&descriptor).unwrap()).unwrap();
    let members = root.join("members");
    for reference in std::iter::once(&definition["registry"])
        .chain(
            definition["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| &row["recipe"]),
        )
        .chain(definition["evidence"].as_array().unwrap().iter())
    {
        let relative = reference["path"].as_str().unwrap();
        let destination = members.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(source.join("members").join(relative), destination).unwrap();
    }
    let command = |args: &[&str]| {
        let raw = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&root)
            .env("PATH", "")
            .output()
            .unwrap();
        if !raw.status.success() && args.first() == Some(&"generate") {
            let diagnostic = DicomTestSuite::embedded().unwrap().generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join("sdk-failure-diagnostic"),
                    CorpusSelector::Profile {
                        profile: "core".into(),
                        include_stress: false,
                    },
                )
                .with_seed(23)
                .with_parallelism(2),
            );
            panic!(
                "CLI generation failed: {}; SDK diagnostic: {diagnostic:?}",
                String::from_utf8_lossy(&raw.stderr)
            );
        }
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        serde_json::from_slice::<Value>(&raw.stdout).unwrap()
    };
    let selector = || CorpusSelector::Profile {
        profile: "core".into(),
        include_stress: false,
    };
    let product = DicomTestSuite::embedded().unwrap();
    let sdk_capabilities = product
        .capabilities_with_corpus(
            InspectCorpusRequest::from_file(&descriptor, &members)
                .with_selection(selector())
                .with_seed(23)
                .with_parallelism(2),
        )
        .unwrap();
    let capabilities = command(&[
        "capabilities",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--seed",
        "23",
        "--parallelism",
        "2",
        "--format",
        "json",
    ]);
    assert_eq!(
        capabilities["result"],
        serde_json::to_value(sdk_capabilities).unwrap()
    );
    assert_eq!(
        capabilities["result"]["loaded_corpus"]["assessment"]["publication"],
        "not_run"
    );
    let cli = command(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--seed",
        "23",
        "--parallelism",
        "2",
        "--out",
        "cli-output",
        "--format",
        "json",
    ]);
    assert_eq!(cli["result"]["emitted_file_count"], 2);
    assert_eq!(cli["result"]["validation_status"], "passed");
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("cli-output/manifest.json")).unwrap()).unwrap();
    valid("manifest-v2.schema.json", &manifest);
    for file in manifest["files"].as_array().unwrap() {
        let case = definition["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["case_id"] == file["case_id"])
            .unwrap();
        let recipe: Value = serde_json::from_slice(
            &fs::read(members.join(case["recipe"]["path"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let pixels = &recipe["dicom"]["artifacts"][0]["secondary_capture"];
        let expected: Vec<u32> = pixels["stored_values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        let obj =
            dicom_object::open_file(root.join("cli-output").join(file["path"].as_str().unwrap()))
                .unwrap();
        for (keyword, value) in [
            ("PatientName", "Integer^Fixture"),
            ("PatientID", "CALLER-INTEGER"),
            ("Manufacturer", "Caller Imaging"),
            ("StudyDate", "20260304"),
        ] {
            assert_eq!(
                obj.element_by_name(keyword).unwrap().to_str().unwrap(),
                value
            );
        }
        let raw = obj
            .element_by_name("PixelData")
            .unwrap()
            .to_bytes()
            .unwrap();
        if pixels["stored_value_type"] == "u1" {
            let actual = (0..expected.len())
                .map(|index| u32::from((raw[index / 8] >> (index % 8)) & 1))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert_eq!(raw.len(), 6);
            assert_eq!(raw[5] & 0b1110_0000, 0);
            assert_eq!(
                obj.element_by_name("PageNumberVector")
                    .unwrap()
                    .to_multi_int::<u32>()
                    .unwrap(),
                vec![1, 2, 3]
            );
            assert_eq!(
                obj.element_by_name("AcquisitionDateTime")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "20260304121314"
            );
            assert_eq!(
                obj.element_by_name("BodyPartExamined")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "HEAD"
            );
        } else {
            let actual = raw
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert!(actual.iter().any(|value| *value > i32::MAX as u32));
            assert!(!actual.contains(&0) && !actual.contains(&u32::MAX));
        }
    }
    for output in ["sdk-output", "repeat-output"] {
        let GenerateCorpusOutcome::Published(sdk) = product
            .generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join(output),
                    selector(),
                )
                .with_seed(23)
                .with_parallelism(2),
            )
            .unwrap()
        else {
            panic!("integer caller must publish")
        };
        assert_eq!(sdk.manifest().deserialize::<Value>().unwrap(), manifest);
        for file in manifest["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            assert!(path.starts_with("caller-integer/"));
            assert_eq!(
                fs::read(root.join("cli-output").join(path)).unwrap(),
                fs::read(sdk.output_root().join(path)).unwrap()
            );
        }
        assert!(
            product
                .validate(ValidateRequest::new(sdk.output_root()))
                .unwrap()
                .is_valid()
        );
    }
    let validation = command(&["validate", "cli-output", "--format", "json"]);
    assert_eq!(validation["result"]["valid"], true);
    let report = command(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    valid(
        "coverage-report-v2.schema.json",
        &report["result"]["report"],
    );
    assert_eq!(
        product
            .report(ReportRequest::new(root.join("sdk-output")))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );

    for file_index in 0..2 {
        for mutation in 0..3 {
            let mut changed = manifest.clone();
            let parameters = changed["files"][file_index]["recipe"]["recipe_parameters"]
                .as_object_mut()
                .unwrap();
            match mutation {
                0 => {
                    parameters.remove("metadata_overrides");
                }
                1 => {
                    parameters.remove("metadata_overrides");
                    parameters.remove("integer_capability_version");
                }
                _ => {
                    parameters.insert("integer_capability_version".into(), json!("99.0.0"));
                }
            }
            fs::write(
                root.join("cli-output/manifest.json"),
                serde_json::to_vec(&changed).unwrap(),
            )
            .unwrap();
            let rejected = product.validate(ValidateRequest::new(root.join("cli-output")));
            assert!(
                rejected.is_err() || !rejected.unwrap().is_valid(),
                "integer file {file_index} must reject metadata/marker mutation {mutation}"
            );
        }
    }
    fs::write(
        root.join("cli-output/manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    for (case_index, pointer, replacement) in [
        (0, "/case_recipe_schema_version", json!("0.1.0")),
        (1, "/case_recipe_schema_version", json!("0.1.0")),
        (0, "/dicom/artifacts/0/attribute_operations", json!([])),
        (1, "/dicom/artifacts/0/attribute_operations", json!([])),
        (
            0,
            "/dicom/artifacts/0/template/template_id",
            json!("classic/secondary-capture/monochrome"),
        ),
        (
            0,
            "/dicom/artifacts/0/secondary_capture/bit_packing",
            Value::Null,
        ),
        (
            0,
            "/dicom/artifacts/0/secondary_capture/bit_packing/frame_start_bit_offsets",
            json!([0, 16, 32]),
        ),
        (
            0,
            "/dicom/artifacts/0/secondary_capture/bit_packing/value_field_padding_bytes",
            json!(1),
        ),
        (
            0,
            "/dicom/artifacts/0/secondary_capture/stored_values/0",
            json!(2),
        ),
        (
            0,
            "/dicom/artifacts/0/secondary_capture/frame_sha256/0",
            json!("0000000000000000000000000000000000000000000000000000000000000000"),
        ),
        (
            1,
            "/dicom/artifacts/0/secondary_capture/integer_word",
            Value::Null,
        ),
        (
            1,
            "/dicom/artifacts/0/secondary_capture/integer_word/covers_full_unsigned_range",
            json!(true),
        ),
        (
            1,
            "/dicom/artifacts/0/secondary_capture/integer_word/byte_order",
            json!("big_endian"),
        ),
        (
            1,
            "/dicom/artifacts/0/secondary_capture/pixel_data_vr",
            json!("OL"),
        ),
        (
            1,
            "/dicom/artifacts/0/secondary_capture/stored_values/0",
            json!(4294967296_u64),
        ),
        (
            1,
            "/dicom/artifacts/0/attribute_operations/0/tag",
            json!("0028,0100"),
        ),
    ] {
        let mut changed = definition.clone();
        let relative = definition["cases"][case_index]["recipe"]["path"]
            .as_str()
            .unwrap();
        let original = fs::read(source.join("members").join(relative)).unwrap();
        let mut recipe: Value = serde_json::from_slice(&original).unwrap();
        *recipe.pointer_mut(pointer).unwrap() = replacement;
        let bytes = serde_json::to_vec(&recipe).unwrap();
        fs::write(members.join(relative), &bytes).unwrap();
        changed["cases"][case_index]["recipe"]["size_bytes"] = json!(bytes.len());
        changed["cases"][case_index]["recipe"]["sha256"] =
            json!(synth_dicom_gen::sha256_hex(&bytes));
        fs::write(&descriptor, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            product
                .capabilities_with_corpus(
                    InspectCorpusRequest::from_file(&descriptor, &members)
                        .with_selection(selector())
                )
                .is_err(),
            "must reject {pointer}"
        );
        fs::write(members.join(relative), original).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn caller_encoded_metadata_cli_sdk_strict_report_and_repeat_are_identical() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "caller-encoded-metadata-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&root).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/generic-encoded-metadata-corpus");
    let descriptor = root.join("definition.json");
    let definition: Value =
        serde_json::from_slice(&fs::read(source.join("definition.json")).unwrap()).unwrap();
    fs::copy(source.join("definition.json"), &descriptor).unwrap();
    let members = root.join("members");
    for reference in std::iter::once(&definition["registry"])
        .chain(
            definition["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|case| &case["recipe"]),
        )
        .chain(definition["evidence"].as_array().unwrap().iter())
    {
        let path = reference["path"].as_str().unwrap();
        let dest = members.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(source.join("members").join(path), dest).unwrap();
    }
    let invoke = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        if !output.status.success() && args.first() == Some(&"generate") {
            let diagnostic = DicomTestSuite::embedded().unwrap().generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join("sdk-diagnostic"),
                    CorpusSelector::Profile {
                        profile: "core".into(),
                        include_stress: false,
                    },
                )
                .with_seed(19)
                .with_parallelism(2),
            );
            panic!(
                "caller metadata generation failed: SDK error {:?}; CLI {}",
                diagnostic.err(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            output.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let selector = || CorpusSelector::Profile {
        profile: "core".into(),
        include_stress: false,
    };
    let product = DicomTestSuite::embedded().unwrap();
    product
        .inspect_corpus(
            InspectCorpusRequest::from_file(&descriptor, &members)
                .with_selection(selector())
                .with_seed(19)
                .with_parallelism(2),
        )
        .unwrap();
    invoke(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--seed",
        "19",
        "--parallelism",
        "2",
        "--out",
        "cli-output",
        "--format",
        "json",
    ]);
    let manifest_path = root.join("cli-output/manifest.json");
    let original_manifest = fs::read(&manifest_path).unwrap();
    let manifest: Value = serde_json::from_slice(&original_manifest).unwrap();
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(manifest["files"].as_array().unwrap().len(), 4);
    for file in manifest["files"].as_array().unwrap() {
        assert!(
            file["case_id"]
                .as_str()
                .unwrap()
                .starts_with("caller/encoded-metadata/")
        );
        assert_eq!(
            file["recipe"]["recipe_parameters"]["metadata_capability_version"],
            "1.0.0"
        );
        let object =
            dicom_object::open_file(root.join("cli-output").join(file["path"].as_str().unwrap()))
                .unwrap();
        assert_eq!(
            object
                .element_by_name("PatientID")
                .unwrap()
                .to_str()
                .unwrap(),
            "CALLER-META-19"
        );
        assert_eq!(
            object
                .element_by_name("Manufacturer")
                .unwrap()
                .to_str()
                .unwrap(),
            "Caller Metadata Lab"
        );
        let metadata = &file["expected_metadata"];
        if let Some(names) = metadata.get("person_names") {
            assert_eq!(
                names[0]["decoded_value"],
                "Suzuki^Hanako=鈴木^花子=すずき^はなこ"
            );
        } else if let Some(strings) = metadata.get("string_elements") {
            assert_eq!(strings.as_array().unwrap().len(), 3);
            assert_eq!(
                strings[1]["decoded_values"],
                json!(["ALPHA", "BETA-2", "GAMMA-3"])
            );
        } else {
            let sequence = &metadata["sequence_length_encoding"];
            assert_eq!(
                sequence["decoded_items"][0]["code_meaning"],
                "Caller abdomen"
            );
            if sequence["variant_id"] == "defined" {
                assert_eq!(sequence["sequence_value_length"], 68);
                assert_eq!(sequence["sequence_length_field_hex"], "44000000");
            }
        }
    }
    for name in ["sdk-output", "repeat-output"] {
        let GenerateCorpusOutcome::Published(out) = product
            .generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join(name),
                    selector(),
                )
                .with_seed(19)
                .with_parallelism(2),
            )
            .unwrap()
        else {
            panic!("caller metadata must publish")
        };
        assert_eq!(out.manifest().deserialize::<Value>().unwrap(), manifest);
        assert!(
            product
                .validate(ValidateRequest::new(out.output_root()))
                .unwrap()
                .is_valid()
        );
        for file in manifest["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            assert_eq!(
                fs::read(root.join("cli-output").join(path)).unwrap(),
                fs::read(out.output_root().join(path)).unwrap()
            );
        }
    }
    assert_eq!(
        invoke(&["validate", "cli-output", "--format", "json"])["result"]["valid"],
        true
    );
    let report = invoke(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert_eq!(
        product
            .report(ReportRequest::new(root.join("sdk-output")))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );
    for index in 0..4 {
        let mut changed = manifest.clone();
        let file = &mut changed["files"][index];
        let path = root.join("cli-output").join(file["path"].as_str().unwrap());
        let original = fs::read(&path).unwrap();
        let mut bytes = original.clone();
        let offset = bytes
            .windows(b"CALLER-META-19".len())
            .position(|part| part == b"CALLER-META-19")
            .unwrap();
        bytes[offset + 12] = b'8';
        fs::write(&path, &bytes).unwrap();
        file["sha256"] = json!(synth_dicom_gen::sha256_hex(&bytes));
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let invalid = product.validate(ValidateRequest::new(root.join("cli-output")));
        assert!(
            invalid.is_err() || !invalid.unwrap().is_valid(),
            "encoded caller metadata must be reopened"
        );
        fs::write(&path, original).unwrap();
    }
    // A coordinated file/manifest edit must still satisfy the declared VR.
    for index in 0..4 {
        let mut changed = manifest.clone();
        let file = &mut changed["files"][index];
        let overrides = file["recipe"]["recipe_parameters"]["metadata_overrides"]
            .as_array_mut()
            .unwrap();
        let operation = overrides
            .iter_mut()
            .find(|op| op["tag"] == "0008,0020")
            .unwrap();
        let old_date = operation["value"].as_str().unwrap().as_bytes().to_vec();
        operation["value"] = json!("abcdefgh");
        let path = root.join("cli-output").join(file["path"].as_str().unwrap());
        let original = fs::read(&path).unwrap();
        let mut bytes = original.clone();
        let offset = bytes
            .windows(old_date.len())
            .position(|part| part == old_date)
            .unwrap();
        bytes[offset..offset + 8].copy_from_slice(b"abcdefgh");
        fs::write(&path, &bytes).unwrap();
        file["sha256"] = json!(synth_dicom_gen::sha256_hex(&bytes));
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let invalid = product.validate(ValidateRequest::new(root.join("cli-output")));
        assert!(invalid.is_err() || !invalid.unwrap().is_valid());
        assert!(
            product
                .report(ReportRequest::new(root.join("cli-output")))
                .is_err()
        );
        fs::write(&path, original).unwrap();
    }
    for index in 0..4 {
        for mutation in 0..5 {
            let mut changed = manifest.clone();
            let file = &mut changed["files"][index];
            match mutation {
                0 => {
                    file.as_object_mut().unwrap().remove("expected_metadata");
                }
                1 => {
                    file["recipe"]["recipe_parameters"]
                        .as_object_mut()
                        .unwrap()
                        .remove("metadata_contract");
                }
                2 => {
                    file["recipe"]["recipe_parameters"]["metadata_capability_version"] =
                        json!("9.0.0");
                }
                3 => {
                    file["recipe"]["recipe_parameters"]["metadata_overrides"] = json!([]);
                }
                _ => {
                    file.as_object_mut().unwrap().remove("expected_metadata");
                    let params = file["recipe"]["recipe_parameters"].as_object_mut().unwrap();
                    params.remove("metadata_contract");
                    params.remove("metadata_capability_version");
                }
            }
            fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
            let invalid = product.validate(ValidateRequest::new(root.join("cli-output")));
            assert!(
                invalid.is_err() || !invalid.unwrap().is_valid(),
                "metadata {index}/{mutation} must fail"
            );
            assert!(
                product
                    .report(ReportRequest::new(root.join("cli-output")))
                    .is_err(),
                "report {index}/{mutation} must fail"
            );
        }
    }
    fs::write(&manifest_path, original_manifest).unwrap();
    for (index, pointer, value) in [
        (
            0,
            "/dicom/artifacts/0/metadata_sc/patient_name_decoded",
            json!("Other^Name"),
        ),
        (
            1,
            "/dicom/artifacts/0/metadata_sc/elements/2/source/values/0",
            json!("NOT_AN_INTEGER"),
        ),
        (
            2,
            "/dicom/artifacts/0/metadata_sc/item_dataset_encoded_length",
            json!(40),
        ),
        (
            2,
            "/dicom/artifacts/1/metadata_sc/code_meaning",
            json!("Conflicting meaning"),
        ),
    ] {
        let mut changed = definition.clone();
        let path = definition["cases"][index]["recipe"]["path"]
            .as_str()
            .unwrap();
        let original = fs::read(members.join(path)).unwrap();
        let mut recipe: Value = serde_json::from_slice(&original).unwrap();
        *recipe.pointer_mut(pointer).unwrap() = value;
        let raw = serde_json::to_vec(&recipe).unwrap();
        fs::write(members.join(path), &raw).unwrap();
        changed["cases"][index]["recipe"]["sha256"] = json!(synth_dicom_gen::sha256_hex(&raw));
        changed["cases"][index]["recipe"]["size_bytes"] = json!(raw.len());
        fs::write(&descriptor, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            product
                .capabilities_with_corpus(
                    InspectCorpusRequest::from_file(&descriptor, &members)
                        .with_selection(selector())
                )
                .is_err(),
            "must reject {pointer}"
        );
        fs::write(members.join(path), original).unwrap();
    }
    let path = definition["cases"][1]["recipe"]["path"].as_str().unwrap();
    let original = fs::read(members.join(path)).unwrap();
    for (rows, spacing, accepted) in [
        (2, "-1", false),
        (2, "0", false),
        (1, "0", true),
        (2, "0.5", true),
    ] {
        let mut recipe: Value = serde_json::from_slice(&original).unwrap();
        let raw_spacing = format!("{spacing}\\1");
        let mut encoded = raw_spacing.as_bytes().to_vec();
        let padding = if encoded.len() % 2 == 1 {
            encoded.push(b' ');
            "space"
        } else {
            "none"
        };
        recipe["dicom"]["artifacts"][0]["metadata_sc"]["elements"].as_array_mut().unwrap().push(json!({
            "tag":"0028,0030", "keyword":"PixelSpacing", "vr":"DS",
            "source":{"source_kind":"literal","values":[spacing,"1"]},
            "padding":padding,"raw_value_byte_length":encoded.len(),"raw_value_sha256":synth_dicom_gen::sha256_hex(&encoded)
        }));
        if rows == 1 {
            recipe["dicom"]["artifacts"][0]["secondary_capture"]["rows"] = json!(1);
            recipe["dicom"]["artifacts"][0]["secondary_capture"]["columns"] = json!(6);
        }
        let raw = serde_json::to_vec(&recipe).unwrap();
        fs::write(members.join(path), &raw).unwrap();
        let mut changed = definition.clone();
        changed["cases"][1]["recipe"]["sha256"] = json!(synth_dicom_gen::sha256_hex(&raw));
        changed["cases"][1]["recipe"]["size_bytes"] = json!(raw.len());
        fs::write(&descriptor, serde_json::to_vec(&changed).unwrap()).unwrap();
        let result = product.capabilities_with_corpus(
            InspectCorpusRequest::from_file(&descriptor, &members).with_selection(selector()),
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "spacing {spacing} rows {rows}: {:?}",
            result.err()
        );
    }
    fs::write(members.join(path), original).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn caller_vl_images_and_icc_are_cli_sdk_identical_and_structurally_valid() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "caller-vl-images-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&root).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/generic-vl-single-frame-corpus");
    let descriptor = root.join("definition.json");
    let definition: Value =
        serde_json::from_slice(&fs::read(source.join("definition.json")).unwrap()).unwrap();
    fs::copy(source.join("definition.json"), &descriptor).unwrap();
    let members = root.join("members");
    for reference in std::iter::once(&definition["registry"])
        .chain(
            definition["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|case| &case["recipe"]),
        )
        .chain(definition["evidence"].as_array().unwrap().iter())
    {
        let path = reference["path"].as_str().unwrap();
        let dest = members.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(source.join("members").join(path), dest).unwrap();
    }
    let invoke = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        if !output.status.success() && args.first() == Some(&"generate") {
            let diagnostic = DicomTestSuite::embedded().unwrap().generate_corpus(
                GenerateCorpusRequest::from_file(
                    &descriptor,
                    &members,
                    root.join("sdk-diagnostic"),
                    CorpusSelector::Profile {
                        profile: "core".into(),
                        include_stress: false,
                    },
                )
                .with_seed(19)
                .with_parallelism(2),
            );
            panic!(
                "caller VL generation failed: SDK error {:?}; CLI {}",
                diagnostic.err(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            output.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let selector = || CorpusSelector::Profile {
        profile: "core".into(),
        include_stress: false,
    };
    let product = DicomTestSuite::embedded().unwrap();
    product
        .inspect_corpus(
            InspectCorpusRequest::from_file(&descriptor, &members)
                .with_selection(selector())
                .with_seed(19)
                .with_parallelism(2),
        )
        .unwrap();
    invoke(&[
        "generate",
        "--corpus",
        "./definition.json",
        "--asset-root",
        "members",
        "--profile",
        "core",
        "--seed",
        "19",
        "--parallelism",
        "2",
        "--out",
        "cli-output",
        "--format",
        "json",
    ]);
    let manifest_path = root.join("cli-output/manifest.json");
    let original_manifest = fs::read(&manifest_path).unwrap();
    let manifest: Value = serde_json::from_slice(&original_manifest).unwrap();
    assert_eq!(manifest["files"].as_array().unwrap().len(), 3);
    for out in ["sdk-output", "repeat-output"] {
        let GenerateCorpusOutcome::Published(output) = product
            .generate_corpus(
                GenerateCorpusRequest::from_file(&descriptor, &members, root.join(out), selector())
                    .with_seed(19)
                    .with_parallelism(2),
            )
            .unwrap()
        else {
            panic!("VL must publish")
        };
        for file in manifest["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            assert_eq!(
                fs::read(root.join("cli-output").join(path)).unwrap(),
                fs::read(output.output_root().join(path)).unwrap()
            );
        }
        assert!(
            product
                .validate(ValidateRequest::new(output.output_root()))
                .unwrap()
                .is_valid()
        );
    }
    let report = invoke(&[
        "report",
        "cli-output",
        "--format",
        "json",
        "--cli-api",
        "1.0.0",
    ]);
    assert_eq!(
        product
            .report(ReportRequest::new(root.join("sdk-output")))
            .unwrap()
            .deserialize::<Value>()
            .unwrap(),
        report["result"]["report"]
    );
    assert_eq!(
        invoke(&["validate", "cli-output", "--format", "json"])["result"]["valid"],
        true
    );
    for file in manifest["files"].as_array().unwrap() {
        assert_eq!(file["image"]["columns"], 3);
        assert_eq!(file["expected_vl_single_frame"]["laterality"], "L");
        assert_eq!(
            file["recipe"]["recipe_parameters"]["vl_provider"]["patient_id"],
            "CALLER-VL-21"
        );
    }
    let icc_index = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .position(|f| f.get("expected_icc_profile").is_some())
        .unwrap();
    assert_eq!(
        manifest["files"][icc_index]["expected_icc_profile"]["profile_description"],
        "RGBx"
    );
    assert!(manifest["files"][icc_index]["expected_icc_profile"]["color_space"].is_null());
    for index in 0..3 {
        for field in [
            "vl_capability_version",
            "vl_provider",
            "vl_artifact",
            "metadata_overrides",
        ] {
            let mut changed = manifest.clone();
            changed["files"][index]["recipe"]["recipe_parameters"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
            let invalid = product.validate(ValidateRequest::new(root.join("cli-output")));
            assert!(
                invalid.is_err() || !invalid.unwrap().is_valid(),
                "missing {field}"
            );
            assert!(
                product
                    .report(ReportRequest::new(root.join("cli-output")))
                    .is_err(),
                "report missing {field}"
            );
        }
        let mut changed = manifest.clone();
        let file = &mut changed["files"][index];
        let path = root.join("cli-output").join(file["path"].as_str().unwrap());
        let original = fs::read(&path).unwrap();
        let mut bytes = original.clone();
        let offset = bytes
            .windows(b"CALLER-VL-21".len())
            .position(|v| v == b"CALLER-VL-21")
            .unwrap();
        bytes[offset + b"CALLER-VL-21".len() - 1] = b'2';
        fs::write(&path, &bytes).unwrap();
        file["sha256"] = json!(synth_dicom_gen::sha256_hex(&bytes));
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let invalid = product.validate(ValidateRequest::new(root.join("cli-output")));
        assert!(invalid.is_err() || !invalid.unwrap().is_valid());
        fs::write(&path, original).unwrap();
    }
    for pointer in [
        "/expected_icc_profile/profile_sha256",
        "/expected_icc_profile/profile_version",
        "/expected_vl_single_frame/iod_kind",
    ] {
        let mut changed = manifest.clone();
        *changed["files"][icc_index].pointer_mut(pointer).unwrap() = json!("invalid");
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            product
                .report(ReportRequest::new(root.join("cli-output")))
                .is_err()
        );
    }
    for index in 0..3 {
        let mut changed = manifest.clone();
        let file = &mut changed["files"][index];
        for field in [
            "vl_capability_version",
            "vl_provider",
            "vl_artifact",
            "icc_projection",
            "metadata_overrides",
        ] {
            file["recipe"]["recipe_parameters"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        file.as_object_mut()
            .unwrap()
            .remove("expected_vl_single_frame");
        file.as_object_mut().unwrap().remove("expected_icc_profile");
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let invalid = product.validate(ValidateRequest::new(root.join("cli-output")));
        assert!(
            invalid.is_err() || !invalid.unwrap().is_valid(),
            "stripped VL declaration"
        );
        assert!(
            product
                .report(ReportRequest::new(root.join("cli-output")))
                .is_err()
        );
    }
    fs::write(&manifest_path, original_manifest).unwrap();
    for (index, pointer, value) in [
        (
            0,
            "/dicom/artifacts/0/parameters/planar_configuration",
            json!(1),
        ),
        (
            1,
            "/dicom/artifacts/0/parameters/sop_class_uid",
            json!("1.2.3"),
        ),
        (
            2,
            "/dicom/artifacts/0/parameters/color_space",
            json!("SRGB"),
        ),
    ] {
        let mut changed = definition.clone();
        let path = definition["cases"][index]["recipe"]["path"]
            .as_str()
            .unwrap();
        let original = fs::read(members.join(path)).unwrap();
        let mut recipe: Value = serde_json::from_slice(&original).unwrap();
        if pointer.ends_with("color_space") {
            recipe["dicom"]["artifacts"][0]["parameters"]["color_space"] = value;
        } else {
            *recipe.pointer_mut(pointer).unwrap() = value;
        }
        let raw = serde_json::to_vec(&recipe).unwrap();
        fs::write(members.join(path), &raw).unwrap();
        changed["cases"][index]["recipe"]["sha256"] = json!(synth_dicom_gen::sha256_hex(&raw));
        changed["cases"][index]["recipe"]["size_bytes"] = json!(raw.len());
        fs::write(&descriptor, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            product
                .capabilities_with_corpus(
                    InspectCorpusRequest::from_file(&descriptor, &members)
                        .with_selection(selector())
                )
                .is_err(),
            "{pointer}"
        );
        fs::write(members.join(path), original).unwrap();
    }
    let path = definition["cases"][2]["recipe"]["path"].as_str().unwrap();
    let original = fs::read(members.join(path)).unwrap();
    for mutation in 0..4 {
        let mut recipe: Value = serde_json::from_slice(&original).unwrap();
        let params = &mut recipe["dicom"]["artifacts"][0]["parameters"];
        let hex = params["icc_profile_hex"].as_str().unwrap();
        let mut bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        match mutation {
            0 => bytes[36] = b'x',
            1 => {
                let first = bytes[136..140].to_vec();
                bytes[148..152].copy_from_slice(&first);
            }
            2 => {
                let offset = u32::from_be_bytes(bytes[136..140].try_into().unwrap()) as usize;
                bytes[offset + 24] = 1;
            }
            _ => bytes[128..132].copy_from_slice(&4096u32.to_be_bytes()),
        }
        params["icc_profile_hex"] =
            json!(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        params["icc_profile_sha256"] = json!(synth_dicom_gen::sha256_hex(&bytes));
        let raw = serde_json::to_vec(&recipe).unwrap();
        fs::write(members.join(path), &raw).unwrap();
        let mut changed = definition.clone();
        changed["cases"][2]["recipe"]["sha256"] = json!(synth_dicom_gen::sha256_hex(&raw));
        changed["cases"][2]["recipe"]["size_bytes"] = json!(raw.len());
        fs::write(&descriptor, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            product
                .capabilities_with_corpus(
                    InspectCorpusRequest::from_file(&descriptor, &members)
                        .with_selection(selector())
                )
                .is_err(),
            "ICC mutation {mutation}"
        );
    }
    fs::write(members.join(path), original).unwrap();
    fs::remove_dir_all(root).unwrap();
}
