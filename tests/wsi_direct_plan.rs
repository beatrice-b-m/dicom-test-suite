use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::corpus_plan::{
    OutputRelativePath, PlannedArtifact, PublicationPlan, PublicationTransaction,
};
use dicom_test_suite::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use dicom_test_suite::executor::services::{
    ArtifactExecutionBindings, MaterializationRequest, StagedAssetRegistry,
};
use dicom_test_suite::recipes::{
    AdvancedPlanProvider, AdvancedPlanProviderRequest, AdvancedProviderFamily,
    AdvancedProviderLimits, RecipeCatalog, WSI_ADVANCED_PROVIDER_ID, WsiAdvancedPlanProvider,
    WsiPlanRecipe, curated_wsi_recipes,
};
use dicom_test_suite::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};
use serde_json::{Value, json};

const SEED: u64 = 1;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "dicom-test-suite-wsi-direct-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

struct NoAuxiliary;

impl AuxiliaryMaterializationHandler for NoAuxiliary {
    fn render(
        &self,
        _: &dicom_test_suite::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        panic!("WSI providers contain no auxiliary artifacts")
    }
}

fn lock_hash() -> String {
    sha256_hex(&fs::read("standards.lock.json").unwrap())
}

fn request(recipe: &WsiPlanRecipe) -> AdvancedPlanProviderRequest {
    AdvancedPlanProviderRequest {
        provider_id: WSI_ADVANCED_PROVIDER_ID.into(),
        family: AdvancedProviderFamily::WholeSlide,
        case_id: recipe.case_id.clone(),
        recipe: recipe.recipe.clone(),
        seed: SEED,
        limits: AdvancedProviderLimits {
            max_artifacts: 4,
            max_references: 1,
            max_binding_slots: 4,
            max_total_output_bytes: 64 * 1024 * 1024,
            max_peak_working_bytes: 64 * 1024 * 1024,
            max_parallelism: 2,
        },
    }
}

fn publication() -> PublicationPlan {
    PublicationPlan {
        manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
        transaction: PublicationTransaction::AtomicNoReplace,
        private_staging: true,
        no_overwrite: true,
    }
}

fn catalog_wsi_identities() -> BTreeSet<(String, String, String)> {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.dicom.as_ref().is_some_and(|dicom| {
                !dicom.artifacts.is_empty()
                    && dicom.artifacts.iter().all(|artifact| {
                        artifact
                            .template
                            .as_ref()
                            .is_some_and(|template| template.template_id.starts_with("vl/wsi/"))
                    })
            }) && !recipe.binding.case_id.starts_with("derived/")
        })
        .map(|recipe| {
            (
                recipe.binding.case_id.clone(),
                recipe.recipe_id.clone(),
                recipe.recipe_version.clone(),
            )
        })
        .collect()
}

fn materialize(
    provider: &WsiAdvancedPlanProvider,
    recipe: &WsiPlanRecipe,
    root: &PathBuf,
) -> Vec<(String, Vec<u8>)> {
    let request = request(recipe);
    let output = provider.plan(&request, recipe).unwrap();
    output.validate(&request).unwrap();
    let dispatcher = MaterializationDispatcher::new(root, Arc::new(NoAuxiliary)).unwrap();
    let assets = StagedAssetRegistry::default();
    output
        .artifacts
        .iter()
        .zip(output.bindings)
        .map(|(artifact, binding)| {
            let path = artifact.planned.output.relative_path.as_str().to_owned();
            dispatcher
                .dispatch(
                    &MaterializationRequest {
                        artifact: PlannedArtifact::Dicom(artifact.planned.clone()),
                        bindings: binding,
                    },
                    &assets,
                )
                .unwrap();
            (path.clone(), fs::read(root.join(path)).unwrap())
        })
        .collect()
}

fn generated(profile: &str) -> (TempRoot, Value) {
    let root = TempRoot::absent(profile);
    let run = prepare_generation_run(GenerateOptions {
        profile: profile.into(),
        out_dir: root.0.clone(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    let manifest =
        serde_json::from_slice(&fs::read(root.0.join("manifest.json")).unwrap()).unwrap();
    (root, manifest)
}

#[test]
fn catalog_derived_wsi_ownership_is_complete_and_planning_is_output_free() {
    let expected = catalog_wsi_identities();
    let recipes = curated_wsi_recipes();
    let actual = recipes
        .iter()
        .map(|recipe| {
            (
                recipe.case_id.clone(),
                recipe.recipe.recipe_id.clone(),
                recipe.recipe.recipe_version.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    let absent = TempRoot::absent("planning");
    let provider = WsiAdvancedPlanProvider::new(lock_hash());
    for recipe in recipes {
        let request = request(&recipe);
        let output = provider.plan(&request, &recipe).unwrap();
        assert_eq!(output.artifacts.len(), recipe.artifacts.len());
        assert!(output.artifacts.windows(2).all(|pair| {
            pair[0].planned.order < pair[1].planned.order
                && pair[0].planned.output.relative_path != pair[1].planned.output.relative_path
        }));
    }
    assert!(!absent.0.exists(), "planning created an output root");

    let recipe = curated_wsi_recipes().into_iter().next().unwrap();
    let mut mismatched = request(&recipe);
    mismatched.case_id = "vl/wsi/not_the_recipe_case".into();
    assert!(provider.plan(&mismatched, &recipe).is_err());
    let mut bounded = request(&recipe);
    bounded.limits.max_total_output_bytes = 1;
    assert!(provider.plan(&bounded, &recipe).is_err());
}

#[test]
fn ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts() {
    let (all_root, all_manifest) = generated("all");
    let (stress_root, stress_manifest) = generated("stress");
    let direct = TempRoot::absent("ordinary-direct");
    fs::create_dir(&direct.0).unwrap();
    let provider = WsiAdvancedPlanProvider::new(lock_hash());
    let files = all_manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .chain(stress_manifest["files"].as_array().unwrap())
        .map(|file| (file["path"].as_str().unwrap(), file))
        .collect::<BTreeMap<_, _>>();

    for recipe in curated_wsi_recipes()
        .into_iter()
        .filter(|recipe| recipe.case_id != "stress/wsi/large_pyramid")
    {
        for (path, bytes) in materialize(&provider, &recipe, &direct.0) {
            let legacy_root = if path.starts_with("vl/wsi/pyramid_multiresolution/") {
                &stress_root.0
            } else {
                &all_root.0
            };
            assert_eq!(bytes, fs::read(legacy_root.join(&path)).unwrap(), "{path}");
            let manifest = files[path.as_str()];
            assert_eq!(manifest["case_id"], recipe.case_id);
            assert_eq!(manifest["sha256"], sha256_hex(&bytes));
            assert_eq!(manifest["size_bytes"], bytes.len());
            assert_eq!(manifest["recipe"]["recipe_id"], recipe.recipe.recipe_id);
            assert_eq!(
                manifest["recipe"]["recipe_version"],
                recipe.recipe.recipe_version
            );
            assert_eq!(
                manifest["dicom"]["sop_class_uid"],
                "1.2.840.10008.5.1.4.1.1.77.1.6"
            );
            assert_eq!(
                manifest["dicom"]["transfer_syntax_uid"],
                "1.2.840.10008.1.2.1"
            );
            assert_eq!(manifest["image"]["photometric_interpretation"], "RGB");
            assert_eq!(manifest["pixel_data"]["native_or_encapsulated"], "native");
        }
    }
}

#[test]
fn reduced_stress_wsi_is_bounded_and_deterministic() {
    let recipe = curated_wsi_recipes()
        .into_iter()
        .find(|recipe| recipe.case_id == "stress/wsi/large_pyramid")
        .unwrap();
    let provider = WsiAdvancedPlanProvider::new(lock_hash());
    let request = request(&recipe);
    let first = provider.plan(&request, &recipe).unwrap();
    let second = provider.plan(&request, &recipe).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.artifacts.len(), 3);
    let payload = first
        .artifacts
        .iter()
        .map(|artifact| artifact.planned.instance.content[0].size_bytes)
        .sum::<u64>();
    assert_eq!(payload, 4_128_768);
    assert!(payload < request.limits.max_total_output_bytes);
    assert_eq!(
        first
            .artifacts
            .iter()
            .map(|artifact| artifact.planned.order)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(first.references, Vec::new());
    assert_eq!(
        first
            .dependencies
            .iter()
            .map(|edge| (edge.artifact_id.as_str(), edge.depends_on.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("stress_wsi_level_002", "stress_wsi_level_001"),
            ("stress_wsi_level_003", "stress_wsi_level_002"),
        ]
    );
    assert_eq!(
        first
            .to_corpus_plan(&request, publication())
            .unwrap()
            .topological_order()
            .unwrap(),
        vec![
            "stress_wsi_level_001",
            "stress_wsi_level_002",
            "stress_wsi_level_003",
        ]
    );
    assert_eq!(
        first.artifacts[0].planned.instance.content[0].properties["frames"],
        "16"
    );
    assert_eq!(
        first.artifacts[2].planned.instance.content[0].properties["frames"],
        "1"
    );
    assert_eq!(json!(request.seed), json!(SEED));
}

#[test]
fn wsi_dag_has_volume_root_closure_and_no_singleton_edges() {
    let provider = WsiAdvancedPlanProvider::new(lock_hash());
    for recipe in curated_wsi_recipes() {
        let request = request(&recipe);
        let output = provider.plan(&request, &recipe).unwrap();
        if recipe.artifacts.len() == 1 {
            assert!(
                output.dependencies.is_empty(),
                "{} singleton acquired a dependency",
                recipe.case_id
            );
            assert_eq!(
                output
                    .to_corpus_plan(&request, publication())
                    .unwrap()
                    .topological_order()
                    .unwrap(),
                vec![output.artifacts[0].planned.logical_id.clone()]
            );
        } else if recipe.case_id == "vl/wsi/pyramid_multiresolution" {
            assert_eq!(
                output
                    .dependencies
                    .iter()
                    .map(|edge| {
                        (
                            edge.artifact_id.as_str(),
                            edge.depends_on.as_str(),
                            edge.relationship.as_str(),
                        )
                    })
                    .collect::<Vec<_>>(),
                vec![
                    (
                        "wsi_pyramid_thumbnail",
                        "wsi_pyramid_volume",
                        "whole_slide_pyramid_volume_root",
                    ),
                    (
                        "wsi_pyramid_label",
                        "wsi_pyramid_volume",
                        "whole_slide_pyramid_volume_root",
                    ),
                ]
            );
            assert_eq!(
                output
                    .to_corpus_plan(&request, publication())
                    .unwrap()
                    .topological_order()
                    .unwrap(),
                vec![
                    "wsi_pyramid_volume",
                    "wsi_pyramid_thumbnail",
                    "wsi_pyramid_label",
                ]
            );
        }
    }
}
