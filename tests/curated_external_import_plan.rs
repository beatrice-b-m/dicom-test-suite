use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::corpus_plan::PlannedArtifact;
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_manifest::project_curated_file_entries;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::ExecutionServiceFactory;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::executor::services::SlotExecutionBinding;
use dicom_test_suite::runtime_capabilities::CapabilityInventory;

const EXTERNAL_CASES: [&str; 5] = [
    "derived/parametric-map/float32_ct_derived_explicit_le",
    "derived/parametric-map/float64_ct_derived_explicit_le",
    "derived/seg/wsi_tile_reference",
    "derived/sr/tid1500_ct_measurement_report",
    "derived/sr/comprehensive3d_scoord3d",
];
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempRoot(std::path::PathBuf);
impl TempRoot {
    fn new(label: &str, create: bool) -> Self {
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "dts-curated-external-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
        assert!(!path.exists());
        if create {
            std::fs::create_dir_all(&path).unwrap();
        }
        Self(path)
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

struct Projector;
impl ManifestProjector for Projector {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        Ok(b"{}\n".to_vec())
    }
}

fn provider(available: bool) -> CuratedScCorpusPlanProvider {
    let mut inventory = CapabilityInventory::compiled();
    if available {
        inventory
            .external_providers
            .insert("highdicom_pydicom".into());
    }
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .with_capability_inventory(inventory)
}

fn request() -> CuratedScPlanRequest {
    CuratedScPlanRequest {
        selection: CuratedScSelection::CaseIds(
            EXTERNAL_CASES.iter().map(|case| (*case).into()).collect(),
        ),
        seed: 1,
        max_parallelism: 4,
    }
}

#[test]
fn all_external_recipe_routes_are_imported_dicom_with_closed_sources() {
    let bundle = provider(true).plan(&request()).unwrap();
    let selected = bundle
        .plan
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PlannedArtifact::ImportedDicom(imported) => Some(imported),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected.len(),
        EXTERNAL_CASES.len(),
        "pending={:?} unavailable={:?}",
        bundle.pending,
        bundle.plan.unavailable
    );
    assert!(bundle.pending.is_empty());

    let planned_ids = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| artifact.logical_id())
        .collect::<BTreeSet<_>>();
    for imported in selected {
        assert_eq!(imported.provider.provider_id, "highdicom_pydicom");
        assert_eq!(imported.provider.output_slot, "dicom");
        assert_eq!(imported.provider.media_type, "application/dicom");
        assert!(imported.provider.maximum_size_bytes > 0);
        assert!(!imported.declared_instance.identities.identities.is_empty());
        assert!(!imported.declared_instance.references.is_empty());
        assert!(
            imported
                .provider
                .source_assets
                .values()
                .all(|source| planned_ids.contains(source.as_str()))
        );
        let binding = &bundle.bindings[&imported.logical_id];
        let SlotExecutionBinding::ProviderRequest { request } = &binding.slots["dicom"] else {
            panic!("external import must be driven by a provider request");
        };
        assert_eq!(request.provider_id, imported.provider.provider_id);
        assert_eq!(
            request.input_assets.len(),
            imported.provider.source_assets.len()
        );
    }

    let imported_ids = bundle
        .plan
        .artifacts
        .iter()
        .filter(|artifact| matches!(artifact, PlannedArtifact::ImportedDicom(_)))
        .map(|artifact| artifact.logical_id())
        .collect::<BTreeSet<_>>();
    assert!(bundle.plan.dependencies.iter().all(|edge| {
        !imported_ids.contains(edge.artifact_id.as_str())
            || planned_ids.contains(edge.depends_on.as_str())
    }));
    for target in bundle.projection.artifacts.iter().filter(|context| {
        context.case_recipe.plan_provider_id == "external.highdicom_sr_import_plan"
    }) {
        for declaration in target.case_recipe.provider_parameters["sources"]
            .as_array()
            .unwrap()
        {
            let recipe_id = declaration["recipe"]["recipe_id"].as_str().unwrap();
            let recipe_version = declaration["recipe"]["recipe_version"].as_str().unwrap();
            let artifact_id = declaration["artifact_logical_id"].as_str().unwrap();
            assert_eq!(
                bundle
                    .projection
                    .artifacts
                    .iter()
                    .filter(|source| {
                        source.case_recipe.recipe_id == recipe_id
                            && source.case_recipe.recipe_version == recipe_version
                            && source.artifact_recipe.logical_id == artifact_id
                    })
                    .count(),
                1,
                "semantic source {recipe_id}@{recipe_version}/{artifact_id} must resolve exactly once"
            );
        }
    }

    let staging = TempRoot::new("bind", true);
    CuratedExecutionServiceFactory::new(&bundle)
        .bind(&staging.0)
        .unwrap();
}

#[test]
fn external_recipes_remain_explicitly_unavailable_without_capability() {
    let bundle = provider(false).plan(&request()).unwrap();
    let pending = bundle
        .pending
        .iter()
        .filter(|pending| EXTERNAL_CASES.contains(&pending.case_id.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(pending.len(), EXTERNAL_CASES.len());
    assert!(bundle.plan.artifacts.iter().all(|artifact| {
        !matches!(artifact, PlannedArtifact::ImportedDicom(imported) if EXTERNAL_CASES.iter().any(|case| imported.case_binding.as_ref().is_some_and(|binding| binding.case_id == *case)))
    }));
}

#[test]
fn prepared_external_provider_executes_through_private_staging() {
    if !std::path::Path::new("generation-backends/highdicom-pydicom/.venv/bin/python").is_file() {
        return;
    }
    let bundle = provider(true)
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "derived/parametric-map/float32_ct_derived_explicit_le".into(),
            ]),
            seed: 1,
            max_parallelism: 2,
        })
        .unwrap();
    let destination = TempRoot::new("execution", false);
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 2, &CancellationToken::new())
        .unwrap();
    let imported = bundle
        .plan
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::ImportedDicom(imported) => Some(imported),
            _ => None,
        })
        .unwrap();
    assert!(
        destination
            .0
            .join(imported.output.relative_path.as_str())
            .is_file()
    );
    assert_eq!(result.evidence.artifacts.len(), bundle.plan.artifacts.len());
}

#[test]
fn all_prepared_external_imports_project_from_executor_evidence() {
    if !std::path::Path::new("generation-backends/highdicom-pydicom/.venv/bin/python").is_file() {
        return;
    }
    let bundle = provider(true).plan(&request()).unwrap();
    let staging = TempRoot::new("projection", true);
    let staged = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute_into_staging(&bundle.plan, &staging.0, 4, &CancellationToken::new())
        .unwrap();
    let files = project_curated_file_entries(&bundle.projection, &staged.projection).unwrap();
    let projected = files
        .iter()
        .filter(|file| {
            file["case_id"]
                .as_str()
                .is_some_and(|case| EXTERNAL_CASES.contains(&case))
        })
        .collect::<Vec<_>>();
    assert_eq!(projected.len(), EXTERNAL_CASES.len());
    for file in projected {
        assert_eq!(
            file.pointer("/generation_backend/backend_id")
                .and_then(serde_json::Value::as_str),
            Some("highdicom_pydicom")
        );
        assert_eq!(
            file.pointer("/validation/status")
                .and_then(serde_json::Value::as_str),
            Some("passed")
        );
        assert!(staging.0.join(file["path"].as_str().unwrap()).is_file());
    }
}
