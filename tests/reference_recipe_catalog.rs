use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use synth_dicom_gen::recipes::{
    PRESENTATION_ADVANCED_PROVIDER_ID, REGISTRATION_PLAN_PROVIDER_ID, RecipeCatalog,
};
use serde_json::Value;

fn load_catalog() -> RecipeCatalog {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RecipeCatalog::load(
        root.join("cases/recipes"),
        root.join("cases/registry.json"),
        root.join("templates/catalog.json"),
    )
    .expect("committed recipe catalog")
}

#[test]
fn reference_recipe_documents_are_complete_and_uniquely_ordered() {
    let catalog = load_catalog();
    let recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            matches!(
                recipe.plan_provider_id.as_str(),
                REGISTRATION_PLAN_PROVIDER_ID | PRESENTATION_ADVANCED_PROVIDER_ID
            )
        })
        .collect::<Vec<_>>();
    assert!(!recipes.is_empty(), "reference recipe inventory is empty");

    let mut planning_orders = BTreeSet::new();
    let mut observed_kinds = BTreeSet::new();
    for recipe in recipes {
        assert!(
            planning_orders.insert(recipe.planning_order.expect("planning_order")),
            "duplicate reference planning order"
        );
        let dicom = recipe.dicom.as_ref().expect("DICOM recipe");
        let [artifact] = dicom.artifacts.as_slice() else {
            panic!("{} must declare one target", recipe.identity());
        };
        assert_eq!(artifact.logical_id, "artifact_1");
        assert!(artifact.output.path.as_deref().is_some_and(|path| {
            !path.is_empty() && !path.starts_with('/') && !path.split('/').any(|part| part == "..")
        }));
        assert_eq!(artifact.content.provider_id, "content.empty_dataset");
        assert!(artifact.parameters.is_empty());

        let provider = Value::Object(recipe.provider_parameters.clone());
        let kind = provider
            .pointer(
                if recipe.plan_provider_id == REGISTRATION_PLAN_PROVIDER_ID {
                    "/registration/kind"
                } else {
                    "/presentation/kind"
                },
            )
            .and_then(Value::as_str)
            .expect("typed reference kind");
        observed_kinds.insert(kind.to_string());
        let sources = provider["sources"].as_array().expect("typed sources");
        assert!(!sources.is_empty());

        let dependency_roles = recipe
            .dependencies
            .iter()
            .map(|dependency| {
                (
                    dependency.recipe.identity().to_string(),
                    dependency.role.as_str(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let source_recipes = sources
            .iter()
            .map(|source| {
                format!(
                    "{}@{}",
                    source["recipe"]["recipe_id"].as_str().unwrap(),
                    source["recipe"]["recipe_version"].as_str().unwrap()
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            dependency_roles.keys().cloned().collect::<BTreeSet<_>>(),
            source_recipes,
            "{} dependency closure",
            recipe.identity()
        );
    }

    assert_eq!(
        observed_kinds,
        BTreeSet::from([
            "advanced_blending".to_string(),
            "blending".to_string(),
            "color".to_string(),
            "deformable".to_string(),
            "grayscale".to_string(),
            "spatial".to_string(),
        ])
    );
}

#[test]
fn catalog_accessors_fail_closed_without_declared_runtime_sources() {
    let catalog = load_catalog();
    for recipe in catalog.recipes().values() {
        let result = match recipe.plan_provider_id.as_str() {
            REGISTRATION_PLAN_PROVIDER_ID => catalog
                .registration_input_for_case(&recipe.binding.case_id, Vec::new())
                .map(|_| ()),
            PRESENTATION_ADVANCED_PROVIDER_ID => catalog
                .presentation_input_for_case(&recipe.binding.case_id, Vec::new())
                .map(|_| ()),
            _ => continue,
        };
        let error = result.expect_err("missing runtime sources must fail closed");
        assert!(error.to_string().contains(&recipe.binding.case_id));
    }
}
