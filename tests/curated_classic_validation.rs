use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::curated_execution::CuratedExecutionServiceFactory;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::executor::adapters::ManifestProjectionInput;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use synth_dicom_gen::executor::evidence::{ExecutionStatus, ResultStatus};
use synth_dicom_gen::recipes::RecipeCatalog;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempOutput(PathBuf);

impl TempOutput {
    fn absent() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dicom-test-suite-curated-classic-validation-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct EvidenceProjector;

impl ManifestProjector for EvidenceProjector {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        Ok(b"{}\n".to_vec())
    }
}

#[test]
fn every_classic_artifact_runs_generic_and_specialized_plan_first_validation() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let mut case_ids = catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.classic_plan")
        .map(|recipe| recipe.binding.case_id.clone())
        .collect::<Vec<_>>();
    case_ids.sort();
    assert!(!case_ids.is_empty());

    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(case_ids),
            seed: 7,
            max_parallelism: 4,
        })
        .unwrap();
    let expected = bundle
        .projection
        .artifacts
        .iter()
        .filter(|context| context.case_recipe.plan_provider_id == "native.classic_plan")
        .map(|context| {
            (
                context.artifact_id.clone(),
                context.case_recipe.binding.case_id.clone(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!(!expected.is_empty());

    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 4, &CancellationToken::new())
    .unwrap();

    let mut observed = std::collections::BTreeSet::new();
    for artifact in &result.evidence.artifacts {
        let Some(case_id) = expected.get(&artifact.logical_id) else {
            continue;
        };
        observed.insert(artifact.logical_id.clone());
        assert_eq!(artifact.status, ExecutionStatus::Succeeded, "{case_id}");
        let validation = artifact.validation.first().expect("validation evidence");
        assert_eq!(validation.status, ResultStatus::Passed, "{case_id}");
        let checks = validation
            .details
            .get("checks")
            .and_then(serde_json::Value::as_array)
            .expect("ordered typed checks");
        let names = checks
            .iter()
            .filter_map(|check| check.get("name").and_then(serde_json::Value::as_str))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(names.contains("part10_preamble"), "{case_id}: {names:?}");
        assert!(
            names.contains("curated_composition_plan"),
            "{case_id}: {names:?}"
        );
        let family_prefix =
            if case_id.starts_with("classic/ct/") || case_id.starts_with("geometry/ct/") {
                assert!(names.contains("classic_ct_group_topology"), "{case_id}");
                "ct_"
            } else if case_id.starts_with("classic/mr/") {
                "mr_"
            } else if case_id.starts_with("classic/cr/") {
                "cr_"
            } else if case_id.starts_with("classic/dx/") {
                "dx_"
            } else if case_id.starts_with("classic/mg/") {
                "mg_"
            } else if case_id.starts_with("classic/us/") {
                "us_"
            } else if case_id.starts_with("classic/nm/") {
                "nm_"
            } else if case_id.starts_with("classic/pet/") {
                "pet_"
            } else if case_id.starts_with("classic/xa/") {
                "xa_"
            } else if case_id.starts_with("classic/xrf/") {
                "xrf_"
            } else {
                ""
            };
        if !family_prefix.is_empty() {
            assert!(
                names.iter().any(|name| name.starts_with(family_prefix)),
                "{case_id}: {names:?}"
            );
        }
        if case_id.contains("icc_profile") {
            assert!(names.contains("icc_profile_round_trip"), "{case_id}");
        }
        if case_id.starts_with("vl/endoscopic/") || case_id.starts_with("vl/microscopic/") {
            assert!(
                names.contains("vl_single_frame_expected_contract"),
                "{case_id}: {names:?}"
            );
        }
        assert!(
            validation
                .details
                .get("generic_plan_checks")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|checks| !checks.is_empty()),
            "{case_id}"
        );
    }
    assert_eq!(observed, expected.keys().cloned().collect());
}
