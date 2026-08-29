use std::collections::BTreeSet;
use std::fs;

use dicom_test_suite::recipes::{
    AdvancedPlanProvider, AdvancedPlanProviderRequest, AdvancedProviderFamily,
    AdvancedProviderLimits, EnhancedPlanProvider, EnhancedProviderInput, RecipeCatalog,
    WSI_ADVANCED_PROVIDER_ID, WsiAdvancedPlanProvider,
};
use dicom_test_suite::sha256_hex;
use serde_json::Value;

const ENHANCED_PROVIDER_ID: &str = "native.enhanced_plan";

fn catalog() -> RecipeCatalog {
    RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap()
}

fn limits() -> AdvancedProviderLimits {
    AdvancedProviderLimits {
        max_artifacts: 8,
        max_references: 8,
        max_binding_slots: 8,
        max_total_output_bytes: 128 * 1024 * 1024,
        max_peak_working_bytes: 256 * 1024 * 1024,
        max_parallelism: 4,
    }
}

#[test]
fn implemented_advanced_recipes_are_catalog_owned_and_plan_exactly_once() {
    let catalog = catalog();
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let implemented = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented")
        .map(|case| case["case_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let advanced = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            matches!(
                recipe.plan_provider_id.as_str(),
                ENHANCED_PROVIDER_ID | WSI_ADVANCED_PROVIDER_ID
            )
        })
        .collect::<Vec<_>>();
    assert!(!advanced.is_empty());
    assert!(
        advanced
            .iter()
            .all(|recipe| implemented.contains(recipe.binding.case_id.as_str()))
    );
    assert_eq!(
        advanced
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        advanced.len()
    );

    let lock = sha256_hex(&fs::read("standards.lock.json").unwrap());
    let enhanced = EnhancedPlanProvider::new(lock.clone()).unwrap();
    let wsi = WsiAdvancedPlanProvider::new(lock);
    for recipe in advanced {
        let dicom = recipe.dicom.as_ref().unwrap();
        let (family, output) = if recipe.plan_provider_id == ENHANCED_PROVIDER_ID {
            let input = catalog
                .enhanced_input_for_case(&recipe.binding.case_id)
                .unwrap()
                .expect("enhanced recipe input");
            let common = match &input {
                EnhancedProviderInput::Ct(value) => &value.common,
                EnhancedProviderInput::Mr(value) => &value.common,
                EnhancedProviderInput::Pet(value) => &value.common,
            };
            assert_eq!(common.case_id, recipe.binding.case_id);
            assert_eq!(common.recipe_id, recipe.recipe_id);
            assert_eq!(common.recipe_version, recipe.recipe_version);
            let request = AdvancedPlanProviderRequest {
                provider_id: ENHANCED_PROVIDER_ID.into(),
                family: AdvancedProviderFamily::Enhanced,
                case_id: recipe.binding.case_id.clone(),
                recipe: recipe.identity(),
                seed: 1,
                limits: limits(),
            };
            (
                AdvancedProviderFamily::Enhanced,
                enhanced.plan(&request, &input).unwrap(),
            )
        } else {
            let input = catalog
                .wsi_input_for_case(&recipe.binding.case_id)
                .unwrap()
                .expect("WSI recipe input");
            let request = AdvancedPlanProviderRequest {
                provider_id: WSI_ADVANCED_PROVIDER_ID.into(),
                family: AdvancedProviderFamily::WholeSlide,
                case_id: recipe.binding.case_id.clone(),
                recipe: recipe.identity(),
                seed: 1,
                limits: limits(),
            };
            (
                AdvancedProviderFamily::WholeSlide,
                wsi.plan(&request, &input).unwrap(),
            )
        };
        assert!(matches!(
            family,
            AdvancedProviderFamily::Enhanced | AdvancedProviderFamily::WholeSlide
        ));
        assert_eq!(output.artifacts.len(), dicom.artifacts.len());
        assert_eq!(output.bindings.len(), dicom.artifacts.len());
        for (planned, declared) in output.artifacts.iter().zip(&dicom.artifacts) {
            assert_eq!(planned.planned.logical_id, declared.logical_id);
            assert_eq!(planned.planned.order, u64::from(declared.order));
            assert_eq!(
                planned.planned.output.relative_path.as_str(),
                declared.output.path.as_deref().unwrap()
            );
            assert_eq!(
                planned.planned.instance.template_id.0,
                declared.template.as_ref().unwrap().template_id
            );
        }
        assert_eq!(
            output
                .artifacts
                .iter()
                .map(|artifact| artifact.planned.logical_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            dicom.artifacts.len()
        );
    }
}

#[test]
fn advanced_recipe_sources_have_no_case_fact_switches_or_writer_paths() {
    for source in [
        include_str!("../src/recipes/enhanced.rs"),
        include_str!("../src/recipes/wsi.rs"),
    ] {
        assert!(!source.contains("curated_wsi_recipes"));
        assert!(!source.contains("pub fn owns_recipe"));
        assert!(!source.contains("pub fn owns(case_id"));
        assert!(!source.contains("crate::generator"));
        assert!(!source.contains("std::fs"));
    }
}
