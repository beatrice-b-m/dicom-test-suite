use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use dicom_test_suite::recipes::RecipeCatalog;
use serde_json::Value;

fn catalog() -> RecipeCatalog {
    RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap()
}

fn registry() -> Value {
    serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap()
}

#[test]
fn implemented_robustness_inventory_is_explicit_and_placeholder_free() {
    let catalog = catalog();
    let registry = registry();
    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["status"] == "implemented"
                && (case["profiles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|profile| profile == "negative" || profile == "fuzz")
                    || case["case_id"] == "qualification/encapsulation/eot_u64_overflow")
        })
        .map(|case| case["case_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let actual = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.binding.case_id.starts_with("negative/")
                || recipe.binding.case_id.starts_with("fuzz/")
                || recipe.binding.case_id == "qualification/encapsulation/eot_u64_overflow"
        })
        .map(|recipe| recipe.binding.case_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    let serialized = expected
        .iter()
        .map(|case_id| {
            let identity = catalog.binding_for_case(case_id).unwrap();
            serde_json::to_string(&catalog.recipes()[identity]).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!serialized.contains("mutation.registry_named"));
    assert!(!serialized.contains("provider_defined"));
}

#[test]
fn negative_sources_and_ordered_operations_are_fully_bound() {
    let catalog = catalog();
    for recipe in catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.binding.case_id.starts_with("negative/"))
    {
        let mutation = recipe.mutation.as_ref().unwrap();
        assert_eq!(recipe.dependencies.len(), 1);
        assert_eq!(recipe.dependencies[0].recipe, mutation.source);
        let source = &catalog.recipes()[&mutation.source.identity()];
        assert!(
            source
                .dicom
                .as_ref()
                .unwrap()
                .artifacts
                .iter()
                .any(|artifact| { artifact.logical_id == mutation.source_logical_role })
        );
        assert_eq!(mutation.output.role, "expected_invalid");
        let expected_path = format!("{}/instance.dcm", recipe.binding.case_id);
        assert_eq!(
            mutation.output.path.as_deref(),
            Some(expected_path.as_str())
        );
        let edit_ids = mutation
            .edits
            .iter()
            .map(|edit| edit.edit_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            edit_ids.len(),
            edit_ids.iter().collect::<BTreeSet<_>>().len()
        );
        for (index, edit) in mutation.edits.iter().enumerate() {
            assert!(edit.edit_id.starts_with(&format!("{:02}_", index + 1)));
            assert!(edit.mutation_id.starts_with("mutation."));
            assert!(!edit.parameters.is_empty());
        }
    }
}

#[test]
fn qualification_sources_budgets_and_payload_policy_are_exact() {
    let catalog = catalog();
    let fuzz = &catalog.recipes()[catalog
        .binding_for_case("fuzz/parser/bounded_seed_corpus")
        .unwrap()];
    assert_eq!(fuzz.plan_provider_id, "qualification.fuzz_plan");
    assert_eq!(fuzz.dependencies.len(), 2);
    let parameters = &fuzz.qualification.as_ref().unwrap().parameters;
    assert_eq!(
        parameters["qualification_kind"],
        "bounded_deterministic_fuzz"
    );
    assert_eq!(parameters["source_generation_seed"], 7);
    assert_eq!(parameters["candidates_per_source"], 32);
    assert_eq!(
        parameters["sources"].as_array().unwrap().len(),
        fuzz.dependencies.len()
    );
    assert_eq!(parameters["budget"]["max_iterations"], 64);
    assert_eq!(
        parameters["budget"]["max_total_target_operations"],
        100_000_000
    );
    assert_eq!(
        fuzz.provider_parameters["payload_policy"],
        "no_payload_retained"
    );

    let eot = &catalog.recipes()[catalog
        .binding_for_case("qualification/encapsulation/eot_u64_overflow")
        .unwrap()];
    assert_eq!(eot.plan_provider_id, "qualification.eot_arithmetic_plan");
    assert!(eot.dependencies.is_empty());
    assert_eq!(eot.provider_parameters["payload_policy"], "evidence_only");
    assert_eq!(
        eot.qualification.as_ref().unwrap().parameters["expected_error"],
        "fragment_padding_overflow"
    );
}

#[test]
fn strict_robustness_schema_rejects_placeholders_and_unknown_budget_fields() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for path in [
        "tests/fixtures/case-recipes/invalid/robustness-placeholder.json",
        "tests/fixtures/case-recipes/invalid/robustness-unknown-budget.json",
    ] {
        let fixture: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert!(validator.iter_errors(&fixture).next().is_some(), "{path}");
    }
    let valid: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/case-recipes/valid/robustness-mutation.json").unwrap(),
    )
    .unwrap();
    assert_eq!(validator.iter_errors(&valid).count(), 0);
}

#[test]
fn dependency_order_keeps_every_robustness_source_before_its_consumer() {
    let catalog = catalog();
    let positions = catalog
        .ordered_identities()
        .iter()
        .enumerate()
        .map(|(index, identity)| (identity, index))
        .collect::<BTreeMap<_, _>>();
    for (identity, recipe) in catalog.recipes().iter().filter(|(_, recipe)| {
        recipe.binding.case_id.starts_with("negative/")
            || recipe.binding.case_id.starts_with("fuzz/")
    }) {
        for dependency in &recipe.dependencies {
            assert!(positions[&dependency.recipe.identity()] < positions[identity]);
        }
    }
}
