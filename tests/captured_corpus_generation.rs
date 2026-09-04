use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "synth-dicom-gen-captured-run-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn bundle() -> Arc<CorpusDefinitionBundle> {
    static BUNDLE: std::sync::OnceLock<Arc<CorpusDefinitionBundle>> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            let workspace = Workspace::new();
            let root = workspace.0.join("definition");
            assert!(
                std::process::Command::new("python3")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/scripts/build-current-corpus-definition-bundle.py"
                    ))
                    .arg(&root)
                    .status()
                    .unwrap()
                    .success()
            );
            Arc::new(CorpusDefinitionBundle::load(&root).unwrap())
            // Workspace is removed before any planning or execution.
        })
        .clone()
}
fn request(root: PathBuf) -> CapturedCorpusRequest {
    CapturedCorpusRequest {
        selection: CorpusSelection::Profile {
            profile: "smoke".into(),
            include_stress: false,
        },
        destination: root,
        seed: 1,
        parallelism: 3,
        dry_run: false,
        cancellation: CancellationToken::new(),
    }
}
fn published(outcome: CapturedCorpusOutcome) -> CorpusExecutionResult {
    match outcome {
        CapturedCorpusOutcome::Published(result) => result,
        _ => panic!("expected published corpus"),
    }
}

#[test]
fn smoke_publication_is_verified_deterministic_and_reader_bounded() {
    let workspace = Workspace::new();
    let bundle = bundle();
    let resources = EngineResources::embedded();
    let first = published(
        run_captured_corpus(
            bundle.clone(),
            resources.clone(),
            request(workspace.0.join("parallel")),
        )
        .unwrap(),
    );
    let mut sequential = request(workspace.0.join("sequential"));
    sequential.parallelism = 1;
    let second =
        published(run_captured_corpus(bundle.clone(), resources.clone(), sequential).unwrap());
    let manifest: Value = serde_json::from_slice(&first.manifest_bytes).unwrap();
    let other: Value = serde_json::from_slice(&second.manifest_bytes).unwrap();
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"]["identity"],
        serde_json::to_value(bundle.identity()).unwrap()
    );
    assert_eq!(
        manifest["identity_projection"]["external_runtime"],
        json!([])
    );
    assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 3);
    for (case, hash) in [
        (
            "mono1_u8_explicit_le",
            "76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc",
        ),
        (
            "mono2_u8_explicit_le",
            "fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15",
        ),
        (
            "rgb_planar0_explicit_le",
            "33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5",
        ),
    ] {
        let file = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["case_id"].as_str().unwrap().ends_with(case))
            .unwrap();
        assert_eq!(file["sha256"], hash);
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(first.destination.join(path)).unwrap(),
            fs::read(second.destination.join(path)).unwrap()
        );
    }
    assert_eq!(manifest["selection_ledger"], other["selection_ledger"]);
    assert!(
        crate::validate_generated_root_with_resources(&first.destination, &resources)
            .unwrap()
            .failures
            .is_empty()
    );
    assert!(
        crate::build_coverage_report_with_resources(&first.destination, &resources)
            .unwrap_err()
            .to_string()
            .contains("not yet supported")
    );
    let sdk = crate::sdk::DicomTestSuite::embedded().unwrap();
    assert!(
        sdk.validate(crate::sdk::ValidateRequest::new(&first.destination))
            .unwrap_err()
            .to_string()
            .contains("not yet supported")
    );
    assert!(
        sdk.report(crate::sdk::ReportRequest::new(&first.destination))
            .unwrap_err()
            .to_string()
            .contains("not yet supported")
    );
}

#[test]
fn selectors_dry_run_and_nonexecutable_outcomes_never_publish() {
    let workspace = Workspace::new();
    let bundle = bundle();
    let mut dry = request(workspace.0.join("dry"));
    dry.dry_run = true;
    let CapturedCorpusOutcome::Planned(preview) =
        run_captured_corpus(bundle.clone(), EngineResources::embedded(), dry).unwrap()
    else {
        panic!("expected plan")
    };
    assert_eq!(preview.validation, "not_run");
    assert_eq!(preview.publication, "not_run");
    assert!(preview.identities.corpus_definition.identity.is_some());
    let legacy = crate::curated_plan::CuratedScCorpusPlanProvider::load(
        crate::curated_plan::CuratedCatalogPaths::from_repository_root(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap()
    .plan(&CuratedScPlanRequest {
        selection: CuratedScSelection::Profile {
            profile: "smoke".into(),
            include_stress: false,
        },
        seed: 1,
        max_parallelism: 3,
    })
    .unwrap();
    assert_eq!(
        serde_json::to_value(&preview.plan).unwrap(),
        serde_json::to_value(legacy.plan).unwrap()
    );
    assert!(!workspace.0.join("dry").exists());
    for ids in [
        vec![],
        vec!["classic/sc/mono1_u8_explicit_le".into(); 2],
        vec!["unknown/case".into()],
    ] {
        let mut invalid = request(workspace.0.join("invalid"));
        invalid.selection = CorpusSelection::CaseIds {
            profile: "smoke".into(),
            include_stress: false,
            case_ids: ids,
        };
        assert!(run_captured_corpus(bundle.clone(), EngineResources::embedded(), invalid).is_err());
    }
    let registry: Value =
        serde_json::from_slice(bundle.bytes(&bundle.manifest().registry.path).unwrap()).unwrap();
    let planned = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["status"] == "planned")
        .unwrap();
    let id = planned["case_id"].as_str().unwrap();
    let profile = planned["profiles"][0].as_str().unwrap();
    let mut no = request(workspace.0.join("no-executable"));
    no.selection = CorpusSelection::CaseIds {
        profile: profile.into(),
        include_stress: false,
        case_ids: vec![id.into()],
    };
    let CapturedCorpusOutcome::NoExecutableCases(result) =
        run_captured_corpus(bundle, EngineResources::embedded(), no).unwrap()
    else {
        panic!("planned-only must not publish")
    };
    assert_eq!(result.validation, "not_run");
    assert_eq!(result.publication, "not_run");
    assert!(
        result
            .selection_ledger
            .iter()
            .any(|row| row["case_id"] == id && row["outcome"] == "planned")
    );
    assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
}

#[test]
fn captured_runner_is_independent_of_working_directory() {
    let workspace = Workspace::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "corpus_generation::captured_runner_tests::smoke_publication_is_verified_deterministic_and_reader_bounded"])
        .current_dir(&workspace.0).output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn explicit_selection_preserves_dependencies_and_mixed_statuses() {
    let workspace = Workspace::new();
    let bundle = bundle();
    let mut mixed = request(workspace.0.join("mixed"));
    mixed.selection = CorpusSelection::CaseIds {
        profile: "all".into(),
        include_stress: false,
        case_ids: vec![
            "classic/sc/mono1_u8_explicit_le".into(),
            "classic/dx/mono2_u12_jpeg_extended".into(),
        ],
    };
    let result =
        published(run_captured_corpus(bundle.clone(), EngineResources::embedded(), mixed).unwrap());
    let manifest: Value = serde_json::from_slice(&result.manifest_bytes).unwrap();
    assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 2);
    assert_eq!(manifest["files"].as_array().unwrap().len(), 1);
    assert!(
        manifest["selection_ledger"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["outcome"] == "planned")
    );
    let mut dependency = request(workspace.0.join("dependency"));
    dependency.selection = CorpusSelection::CaseIds {
        profile: "all".into(),
        include_stress: false,
        case_ids: vec!["derived/registration/spatial_ct_pair".into()],
    };
    let result = published(
        run_captured_corpus(bundle.clone(), EngineResources::embedded(), dependency).unwrap(),
    );
    let manifest: Value = serde_json::from_slice(&result.manifest_bytes).unwrap();
    assert!(
        manifest["selection_ledger"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["selection"] == "dependency")
    );
    for row in manifest["selection_ledger"].as_array().unwrap() {
        let mut expected = bundle
            .manifest()
            .cases
            .iter()
            .find(|c| c.case_id == row["case_id"].as_str().unwrap())
            .unwrap()
            .dependencies
            .clone();
        expected.sort();
        assert_eq!(row["dependency_case_ids"], json!(expected));
    }
    let mut unavailable = request(workspace.0.join("unavailable"));
    unavailable.selection = CorpusSelection::CaseIds {
        profile: "all".into(),
        include_stress: false,
        case_ids: vec!["classic/sc/mono2_u16_jpeg2000_lossless".into()],
    };
    let CapturedCorpusOutcome::NoExecutableCases(result) =
        run_captured_corpus(bundle.clone(), EngineResources::embedded(), unavailable).unwrap()
    else {
        panic!("uncompiled codec must remain unavailable")
    };
    assert!(
        result
            .selection_ledger
            .iter()
            .all(|r| r["outcome"] == "unavailable")
    );
    assert!(!workspace.0.join("unavailable").exists());
    for (profile, include_stress, ids) in [
        (
            "smoke",
            false,
            vec!["derived/registration/spatial_ct_pair".into()],
        ),
        ("unknown", false, vec![]),
        ("smoke", true, vec![]),
    ] {
        assert!(
            selection(
                &bundle,
                &CorpusSelection::CaseIds {
                    profile: profile.into(),
                    include_stress,
                    case_ids: ids
                }
            )
            .is_err()
        );
    }
    let mut invalid = request(workspace.0.join("zero"));
    invalid.parallelism = 0;
    assert!(run_captured_corpus(bundle, EngineResources::embedded(), invalid).is_err());
}

#[test]
fn transaction_failure_cancellation_and_existing_destination_are_atomic() {
    let workspace = Workspace::new();
    let bundle = bundle();
    let cancelled = request(workspace.0.join("cancelled"));
    cancelled.cancellation.cancel();
    assert!(matches!(
        run_captured_corpus(bundle.clone(), EngineResources::embedded(), cancelled),
        Err(CapturedCorpusError::Cancelled)
    ));
    let failure = run_with_publication_check(
        bundle.clone(),
        EngineResources::embedded(),
        request(workspace.0.join("failed")),
        Arc::new(|| {
            Err(ManifestProjectionError(
                "injected publication refusal".into(),
            ))
        }),
    );
    assert!(failure.is_err());
    assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
    let final_cancel = request(workspace.0.join("final-cancel"));
    let token = final_cancel.cancellation.clone();
    assert!(
        run_with_publication_check(
            bundle.clone(),
            EngineResources::embedded(),
            final_cancel,
            Arc::new(move || {
                token.cancel();
                Ok(())
            })
        )
        .is_err()
    );
    assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
    fs::create_dir(workspace.0.join("existing")).unwrap();
    fs::write(workspace.0.join("existing/sentinel"), b"retained").unwrap();
    assert!(
        run_captured_corpus(
            bundle,
            EngineResources::embedded(),
            request(workspace.0.join("existing"))
        )
        .is_err()
    );
    assert_eq!(
        fs::read(workspace.0.join("existing/sentinel")).unwrap(),
        b"retained"
    );
}
