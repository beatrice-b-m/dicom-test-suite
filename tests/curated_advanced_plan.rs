use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::corpus_plan::{ArtifactProvenance, PlannedArtifact};
use synth_dicom_gen::curated_execution::CuratedExecutionServiceFactory;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::executor::adapters::ManifestProjectionInput;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use synth_dicom_gen::executor::evidence::ResultStatus;
use synth_dicom_gen::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use synth_dicom_gen::executor::services::{
    ArtifactExecutionBindings, MaterializationRequest, StagedAssetRegistry,
};
use synth_dicom_gen::recipes::RecipeCatalog;
use synth_dicom_gen::sha256_hex;
use serde_json::Value;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn create() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dts-curated-advanced-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct NoAuxiliary;

impl AuxiliaryMaterializationHandler for NoAuxiliary {
    fn render(
        &self,
        _: &synth_dicom_gen::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        panic!("advanced image providers contain no auxiliary artifacts")
    }
}

struct EvidenceProjector;

impl ManifestProjector for EvidenceProjector {
    fn project(&self, input: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        serde_json::to_vec(&serde_json::json!({
            "corpus_plan_sha256": input.corpus_plan_sha256,
        }))
        .map_err(|error| ManifestProjectionError(error.to_string()))
    }
}

fn provider() -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap()
}

fn catalog() -> RecipeCatalog {
    RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap()
}

fn registry_cases_with_profile(profile: &str) -> BTreeSet<String> {
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["profiles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == profile)
        })
        .map(|case| case["case_id"].as_str().unwrap().to_owned())
        .collect()
}

fn frozen_seed_one_hashes() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            "vl/wsi/tiled_full_small/instance.dcm",
            "0d97e1211e2f15994d29593996202a2096efe2f5a619cac6fddc2f28c98a1d62",
        ),
        (
            "vl/wsi/tiled_sparse_small/instance.dcm",
            "74f3ba05bd879122839f1c5de86ffb11f79fd7d9716fda31bf3e76070894d3a0",
        ),
        (
            "vl/wsi/multiple_optical_paths/instance.dcm",
            "e40f0b1cb494318a22f78311cbc9572b5e8f712b3e039953930debffa5f57535",
        ),
        (
            "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
            "7ad8de623f589ac6f63f27631dadc9e7ab3d01e05bea1fd89a872ea08c9ef919",
        ),
        (
            "enhanced/ct/concatenation_two_part_explicit_le/part-001.dcm",
            "80080befc5ae4e8ea6c11e889c08ac391ec46fe7b55aac14f8ff11c854f73d50",
        ),
        (
            "enhanced/ct/concatenation_two_part_explicit_le/part-002.dcm",
            "4d717c7b1b476d9544ba6886ed3a7537689aa503883e5294a0ea8d2146b167c9",
        ),
        (
            "enhanced/mr/multiframe_echo_perframe_explicit_le/instance.dcm",
            "ae42d05cfba40706f6fe6856192b104238e82777c16c5520b780f32bf657264a",
        ),
        (
            "enhanced/mr/multiframe_temporal_position_explicit_le/instance.dcm",
            "7c87eb000fa46b1b772023f2f4c27d5351d24dfa7b967e71718cdd241b64a9a1",
        ),
        (
            "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le/instance.dcm",
            "00bbee122bdfdaa844fad5a8919a1c984ffe3e9c4a41eecebd4e84f302386fd8",
        ),
        (
            "enhanced/pet/multiframe_explicit_le/instance.dcm",
            "f40d03339b2344d0f415c3be9ed5194b3657dcf68a06680f131f1dfe0607125f",
        ),
    ])
}

fn ordinary_cases_and_paths() -> (Vec<String>, Vec<String>) {
    let hashes = frozen_seed_one_hashes();
    let catalog = catalog();
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            matches!(
                recipe.plan_provider_id.as_str(),
                "native.enhanced_plan" | "native.wsi_plan"
            ) && recipe.dicom.as_ref().is_some_and(|dicom| {
                !dicom.artifacts.is_empty()
                    && dicom.artifacts.iter().all(|artifact| {
                        artifact
                            .output
                            .path
                            .as_deref()
                            .is_some_and(|path| hashes.contains_key(path))
                    })
            })
        })
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    let cases = recipes
        .iter()
        .map(|recipe| recipe.binding.case_id.clone())
        .collect();
    let paths = recipes
        .into_iter()
        .flat_map(|recipe| {
            let mut artifacts = recipe
                .dicom
                .as_ref()
                .unwrap()
                .artifacts
                .iter()
                .collect::<Vec<_>>();
            artifacts.sort_by_key(|artifact| artifact.order);
            artifacts
                .into_iter()
                .map(|artifact| artifact.output.path.clone().unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    (cases, paths)
}

#[test]
fn curated_advanced_slice_matches_frozen_seed_one_bytes_paths_and_order() {
    let (case_ids, expected_paths) = ordinary_cases_and_paths();
    assert!(!case_ids.is_empty());
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(case_ids),
            seed: 1,
            max_parallelism: 2,
        })
        .unwrap();
    let actual_paths = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| {
            artifact
                .output()
                .expect("advanced image artifact has output")
                .relative_path
                .as_str()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_paths, expected_paths);

    let staging = TempRoot::create();
    let dispatcher = MaterializationDispatcher::new(&staging.0, Arc::new(NoAuxiliary)).unwrap();
    let assets = StagedAssetRegistry::default();
    let hashes = frozen_seed_one_hashes();
    for artifact in &bundle.plan.artifacts {
        let id = artifact.logical_id();
        dispatcher
            .dispatch(
                &MaterializationRequest {
                    artifact: artifact.clone(),
                    bindings: bundle.bindings[id].clone(),
                },
                &assets,
            )
            .unwrap();
        let path = artifact
            .output()
            .expect("advanced image artifact has output")
            .relative_path
            .as_str();
        let bytes = fs::read(staging.0.join(path)).unwrap();
        assert_eq!(sha256_hex(&bytes), hashes[path], "{path}");
    }
}

#[test]
fn stress_advanced_selection_preserves_requested_provenance_and_bounded_resources() {
    let catalog = catalog();
    let stress_cases = registry_cases_with_profile("stress")
        .into_iter()
        .filter(|case_id| {
            let Some(identity) = catalog.binding_for_case(case_id) else {
                return false;
            };
            matches!(
                catalog.recipes()[identity].plan_provider_id.as_str(),
                "native.enhanced_plan" | "native.wsi_plan"
            )
        })
        .collect::<Vec<_>>();
    assert!(!stress_cases.is_empty());
    let absent = std::env::temp_dir().join(format!(
        "dts-curated-advanced-plan-only-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(!absent.exists());
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(stress_cases),
            seed: 1,
            max_parallelism: 3,
        })
        .unwrap();
    assert!(!absent.exists());
    assert!(bundle.plan.artifacts.iter().all(|artifact| matches!(
        artifact,
        PlannedArtifact::Dicom(value) if value.provenance == ArtifactProvenance::Requested
    )));
    let total = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().output_bytes)
        .sum::<u64>();
    let mut working_sets = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().peak_working_bytes)
        .collect::<Vec<_>>();
    working_sets.sort_unstable_by(|left, right| right.cmp(left));
    let peak = working_sets.into_iter().take(3).sum::<u64>();
    assert_eq!(
        bundle.plan.resources.max_total_output_bytes,
        total + synth_dicom_gen::curated_plan::MAX_CURATED_MANIFEST_BYTES
    );
    assert_eq!(bundle.plan.resources.max_peak_working_bytes, peak);
    assert_eq!(bundle.plan.resources.max_parallelism, 3);
    let ids = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| artifact.logical_id())
        .collect::<BTreeSet<_>>();
    assert!(bundle.plan.dependencies.iter().all(|dependency| {
        ids.contains(dependency.artifact_id.as_str())
            && ids.contains(dependency.depends_on.as_str())
    }));
    bundle.plan.topological_order().unwrap();
}

#[test]
fn curated_executor_records_typed_enhanced_and_wsi_validation_evidence() {
    let catalog = catalog();
    let stress = registry_cases_with_profile("stress");
    let mut selected = BTreeMap::new();
    for recipe in catalog.recipes().values() {
        if matches!(
            recipe.plan_provider_id.as_str(),
            "native.enhanced_plan" | "native.wsi_plan"
        ) && !stress.contains(&recipe.binding.case_id)
        {
            selected
                .entry(recipe.plan_provider_id.as_str())
                .or_insert_with(|| recipe.binding.case_id.clone());
        }
    }
    assert_eq!(
        selected.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from(["native.enhanced_plan", "native.wsi_plan"])
    );
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(selected.values().cloned().collect()),
            seed: 1,
            max_parallelism: 2,
        })
        .unwrap();
    let destination = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "dts-curated-advanced-execution-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(!destination.exists());
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination, 2, &CancellationToken::new())
    .unwrap();
    for artifact in &result.evidence.artifacts {
        assert!(
            artifact
                .validation
                .iter()
                .all(|validation| validation.status == ResultStatus::Passed)
        );
        let check_names = artifact
            .validation
            .iter()
            .flat_map(|validation| {
                validation.details["checks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|check| check["name"].as_str())
            })
            .collect::<BTreeSet<_>>();
        assert!(check_names.iter().any(|name| {
            matches!(
                *name,
                "enhanced_plan_materialization_round_trip" | "wsi_plan_materialization_round_trip"
            )
        }));
    }
    fs::remove_dir_all(destination).unwrap();
}
