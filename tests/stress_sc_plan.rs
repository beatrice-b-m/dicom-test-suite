use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;
use synth_dicom_gen::recipes::{
    RecipeCatalog, StressScContentRequest, StressScPlanError, plan_stress_sc_recipe,
};
use synth_dicom_gen::sha256_hex;

fn load() -> (RecipeCatalog, String) {
    (
        RecipeCatalog::load(
            "cases/recipes",
            "cases/registry.json",
            "templates/catalog.json",
        )
        .unwrap(),
        sha256_hex(&fs::read("standards.lock.json").unwrap()),
    )
}

#[test]
fn stress_sc_ownership_is_catalog_derived_and_complete() {
    let (catalog, lock) = load();
    let owned = catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.stress_sc_plan")
        .collect::<Vec<_>>();
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|case| {
            let id = case["case_id"].as_str()?;
            (case["status"] == "implemented" && id.starts_with("stress/sc/")).then(|| id.to_owned())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        owned
            .iter()
            .map(|recipe| recipe.binding.case_id.clone())
            .collect::<BTreeSet<_>>(),
        expected
    );
    let mut orders = BTreeSet::new();
    for recipe in owned {
        assert!(orders.insert(recipe.planning_order.unwrap()));
        let plan = plan_stress_sc_recipe(recipe, &lock, 1).unwrap().unwrap();
        assert_eq!(plan.order, 0);
        assert!(plan.dependencies.is_empty());
        assert!(plan.resources.output_bytes > 0);
        assert!(plan.resources.peak_working_bytes >= plan.resources.output_bytes / 2);
        assert_eq!(
            plan.output_relative_path.as_str(),
            format!("{}/instance.dcm", recipe.binding.case_id)
        );
        assert!(!plan.parameters.policy().full_scale_available);
    }
}

#[test]
fn reduced_scale_parameters_and_content_algorithms_are_exact() {
    let (catalog, lock) = load();
    let mut kinds = BTreeSet::new();
    for recipe in catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.stress_sc_plan")
    {
        let plan = plan_stress_sc_recipe(recipe, &lock, 1).unwrap().unwrap();
        match plan.content {
            StressScContentRequest::RepeatedNativeBytes {
                byte: 0,
                length: 67_108_864,
            } => {
                kinds.insert("bulk");
            }
            StressScContentRequest::NestedPrivateBulk {
                sequence_depth: 32,
                byte: 90,
                length: 16_777_216,
                ..
            } => {
                kinds.insert("nested");
            }
            StressScContentRequest::RepeatedPrivateText {
                creator_blocks: 4,
                values_per_block: 256,
                value_bytes: 1024,
                fill_character: 'M',
            } => {
                kinds.insert("metadata");
            }
            StressScContentRequest::DeterministicRleFrames {
                rows: 512,
                columns: 512,
                frames: 256,
                fragments_per_frame: 64,
                extended_offset_table: true,
                ..
            } => {
                kinds.insert("encapsulated");
            }
            other => panic!("unexpected stress request: {other:?}"),
        }
    }
    assert_eq!(
        kinds,
        BTreeSet::from(["bulk", "encapsulated", "metadata", "nested"])
    );
}

#[test]
fn planning_is_filesystem_free_and_malformed_parameters_fail_closed() {
    let (catalog, lock) = load();
    let recipe = catalog
        .recipes()
        .values()
        .find(|recipe| recipe.binding.case_id == "stress/sc/large_bulk_data")
        .unwrap();
    let absent = std::env::temp_dir().join(format!("dts-stress-plan-{}", std::process::id()));
    assert!(!absent.exists());
    plan_stress_sc_recipe(recipe, &lock, 1).unwrap().unwrap();
    assert!(!absent.exists());

    let mut corrupt = recipe.clone();
    corrupt
        .provider_parameters
        .insert("unbounded".into(), Value::Bool(true));
    assert!(matches!(
        plan_stress_sc_recipe(&corrupt, &lock, 1),
        Err(StressScPlanError::Parameters(_))
    ));
}
