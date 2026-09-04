use super::*;
use crate::corpus_generation::{
    CapturedCorpusOutcome, CapturedCorpusRequest, CorpusSelection, run_captured_corpus,
};
use crate::engine_resources::EngineResources;
use std::{fs, sync::Arc};

fn fixture(ids: Option<Vec<String>>) -> Value {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "synth-dicom-gen-captured-report-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("definition");
    assert!(
        std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/build-current-corpus-definition-bundle.py"
            ))
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let bundle = Arc::new(crate::corpus_definition::CorpusDefinitionBundle::load(&source).unwrap());
    fs::remove_dir_all(source).unwrap();
    let selection = match ids {
        Some(case_ids) => CorpusSelection::CaseIds {
            profile: "all".into(),
            include_stress: false,
            case_ids,
        },
        None => CorpusSelection::Profile {
            profile: "smoke".into(),
            include_stress: false,
        },
    };
    let CapturedCorpusOutcome::Published(result) = run_captured_corpus(
        bundle,
        EngineResources::embedded(),
        CapturedCorpusRequest {
            selection,
            destination: root.join("output"),
            seed: 1,
            parallelism: 2,
            dry_run: false,
            cancellation: crate::executor::cancellation::CancellationToken::new(),
        },
    )
    .unwrap() else {
        panic!("expected published fixture")
    };
    let manifest = serde_json::from_slice(&result.manifest_bytes).unwrap();
    fs::remove_dir_all(root).unwrap();
    manifest
}

#[test]
fn reports_preserve_exact_source_and_distinguish_case_artifact_counts() {
    let source = fixture(Some(vec![
        "derived/registration/spatial_ct_pair".into(),
        "classic/dx/mono2_u12_jpeg_extended".into(),
        "classic/sc/mono2_u16_jpeg2000_lossless".into(),
    ]));
    let report = project(&source).unwrap();
    validate(&report).unwrap();
    assert_eq!(report["source_manifest"], source);
    assert_eq!(report["identity_projection"], source["identity_projection"]);
    assert_eq!(
        report["summary"]["logical_cases"],
        source["selection_ledger"].as_array().unwrap().len()
    );
    assert_eq!(
        report["summary"]["emitted_files"],
        source["files"].as_array().unwrap().len()
    );
    assert!(report["summary"]["dependency_cases"].as_u64().unwrap() > 0);
    assert_eq!(report["summary"]["outcomes"]["planned"], 1);
    assert_eq!(report["summary"]["outcomes"]["unavailable"], 1);
    assert_eq!(project(&source).unwrap(), report);
    assert!(markdown(&report).contains("No new validation"));
    assert_eq!(report["evidence"]["validation"], "not_assessed");
}

#[test]
fn projection_accepts_unknown_case_names_without_embedded_inference() {
    let mut source = fixture(None);
    let old = source["selection_ledger"][0]["case_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let new = "caller/synthetic_name";
    source["selection_ledger"][0]["case_id"] = json!(new);
    source["selection_ledger"][0]["case_definition"]["case_id"] = json!(new);
    for file in source["files"].as_array_mut().unwrap() {
        if file["case_id"] == old {
            file["case_id"] = json!(new);
        }
    }
    source["selection_ledger"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|r| r["case_id"].as_str().unwrap().to_owned());
    let mut extra = source["files"][0].clone();
    extra["path"] = json!("caller-extra.dcm");
    let case = extra["case_id"].clone();
    source["files"].as_array_mut().unwrap().push(extra);
    let row = source["selection_ledger"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["case_id"] == case)
        .unwrap();
    row["artifact_paths"]
        .as_array_mut()
        .unwrap()
        .push(json!("caller-extra.dcm"));
    row["artifact_paths"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|v| v.as_str().unwrap().to_owned());
    let report = project(&source).unwrap();
    validate(&report).unwrap();
    assert_eq!(report["summary"]["logical_cases"], 3);
    assert_eq!(report["summary"]["emitted_files"], 4);
    assert_eq!(report["source_manifest"], source);
    // Group real captured-definition shapes without executing isolated scopes.
    let registry: Value = serde_json::from_slice(include_bytes!("../cases/registry.json")).unwrap();
    let isolated = ["negative", "fuzz", "stress"].map(|profile| {
        registry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| {
                row["profiles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p == profile)
            })
            .unwrap()
    });
    let grouped = dimensions(
        isolated
            .iter()
            .map(|row| (row["case_id"].as_str().unwrap(), *row)),
        false,
    );
    let profiles = grouped["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| group["value"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(profiles, vec!["fuzz", "negative", "stress"]);
}

#[test]
fn report_semantics_reject_summary_identity_and_source_tampering() {
    let report = project(&fixture(None)).unwrap();
    for (pointer, replacement) in [
        ("/coverage_report_schema_version", json!("99.0.0")),
        ("/summary/logical_cases", json!(900)),
        (
            "/identity_projection/engine/engine_sha256",
            json!("a".repeat(64)),
        ),
        (
            "/source_manifest/selection_ledger/0/case_definition/status",
            json!("planned"),
        ),
        ("/case_dimensions/modalities", json!([])),
        ("/evidence/validation", json!("passed")),
    ] {
        let mut changed = report.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert!(validate(&changed).is_err(), "{pointer}");
    }
    assert!(crate::report_contract::validate_report_contract(&report).is_ok());
    assert_eq!(
        crate::sdk::ReportOutcome::from_report_test_fixture(&report)
            .unwrap()
            .kind(),
        crate::sdk::ReportKind::ExternalCorpus
    );
}
