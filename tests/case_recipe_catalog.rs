use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;

use dicom_test_suite::recipes::{RecipeCatalog, RecipeCatalogError, RecipeIdentity};

#[test]
fn catalog_exactly_and_uniquely_binds_every_implemented_registry_recipe() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented")
        .map(|case| RecipeIdentity {
            recipe_id: case["recipe_id"].as_str().unwrap().to_string(),
            recipe_version: case["recipe_version"].as_str().unwrap().to_string(),
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        catalog.recipes().keys().cloned().collect::<BTreeSet<_>>(),
        expected
    );
    for case in registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented")
    {
        let case_id = case["case_id"].as_str().unwrap();
        assert_eq!(
            catalog.binding_for_case(case_id),
            Some(&RecipeIdentity {
                recipe_id: case["recipe_id"].as_str().unwrap().to_string(),
                recipe_version: case["recipe_version"].as_str().unwrap().to_string(),
            })
        );
    }
}

#[test]
fn modular_loading_and_dependency_order_are_deterministic() {
    let first = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let second = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    assert_eq!(first.ordered_identities(), second.ordered_identities());
    let positions = first
        .ordered_identities()
        .iter()
        .enumerate()
        .map(|(index, identity)| (identity, index))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (identity, recipe) in first.recipes() {
        for dependency in &recipe.dependencies {
            assert!(positions[&dependency.recipe.identity()] < positions[identity]);
        }
    }
}

#[test]
fn schema_rejects_unknown_fields_before_completeness_checks() {
    let error = RecipeCatalog::load(
        "tests/fixtures/case-recipes/invalid",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap_err();
    assert!(matches!(error, RecipeCatalogError::Schema { .. }));
}

#[test]
fn committed_positive_fixture_is_schema_valid() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let fixture: Value =
        serde_json::from_slice(&fs::read("tests/fixtures/case-recipes/valid/dicom.json").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert_eq!(validator.iter_errors(&fixture).count(), 0);
}

#[test]
fn schema_rejects_parent_traversal_output_path() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let fixture: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/case-recipes/invalid/unsafe-output.json").unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.iter_errors(&fixture).next().is_some());
}

#[test]
fn schema_rejects_kind_payload_mismatch() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let fixture: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/case-recipes/invalid/kind-payload-mismatch.json").unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.iter_errors(&fixture).next().is_some());
}
