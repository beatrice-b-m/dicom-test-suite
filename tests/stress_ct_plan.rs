use std::collections::BTreeSet;
use std::fs;

use dicom_test_suite::recipes::{
    OrderedSeriesProvider, RecipeCatalog, StressCtPlanError, plan_stress_ct_recipe,
};
use dicom_test_suite::sha256_hex;
use serde_json::Value;

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
fn high_instance_ct_plan_is_complete_ordered_and_dag_closed() {
    let (catalog, lock) = load();
    let recipe = catalog
        .recipes()
        .values()
        .find(|recipe| recipe.plan_provider_id == "native.stress_ct_plan")
        .unwrap();
    let output = plan_stress_ct_recipe(recipe, &lock, 1).unwrap().unwrap();
    let instances = recipe.provider_parameters["instances"].as_u64().unwrap() as usize;
    assert_eq!(output.requests.len(), instances);
    assert_eq!(output.resources.len(), output.requests.len());
    let ids = output
        .requests
        .iter()
        .map(|request| request.logical_id.as_str())
        .collect::<BTreeSet<_>>();
    let sop_uids = output
        .requests
        .iter()
        .map(|request| request.sop_instance_uid.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), output.requests.len());
    assert_eq!(sop_uids.len(), output.requests.len());
    for (index, request) in output.requests.iter().enumerate() {
        assert_eq!(request.order as usize, index);
        assert_eq!(
            request.output_relative_path.as_str(),
            format!(
                "stress/study/high_instance_count_ct/slice-{:03}.dcm",
                index + 1
            )
        );
        assert!(request.dependencies.is_empty());
        assert_eq!(request.pixels.pixels.shape.rows, 64);
        assert_eq!(request.pixels.pixels.shape.columns, 64);
        assert_eq!(request.pixels.pixels.stored_values.len(), 64 * 64);
        assert_eq!(request.pixels.pixels.declared_pixel_min, -1024);
        assert_eq!(request.pixels.pixels.declared_pixel_max, 2047);
        assert_eq!(
            request.common.equipment.manufacturer_model_name,
            dicom_test_suite::recipes::ElementPresence::Value(
                "stress_high_instance_count_ct".into()
            )
        );
        assert_eq!(
            request.pixels.pixels.expected_frame_sha256,
            ["e8b46d597c2c40be1ee400f37a882b9513860456f8e1cadb53f44b0b3ffe986d"]
        );
    }
    let mut reversed = output.requests;
    reversed.reverse();
    let planned = OrderedSeriesProvider.plan(reversed).unwrap();
    assert_eq!(planned.len(), instances);
    assert!(planned.iter().enumerate().all(|(index, instance)| {
        instance.order as usize == index && instance.dependencies.is_empty()
    }));
    assert!(!output.policy.full_scale_available);
}

#[test]
fn stress_ct_resources_are_bounded_and_overflow_safe() {
    let (catalog, lock) = load();
    let recipe = catalog
        .recipes()
        .values()
        .find(|recipe| recipe.plan_provider_id == "native.stress_ct_plan")
        .unwrap();
    let output = plan_stress_ct_recipe(recipe, &lock, 1).unwrap().unwrap();
    let total_output = output.resources.iter().try_fold(0_u64, |total, resource| {
        total.checked_add(resource.output_bytes)
    });
    let total_peak = output.resources.iter().try_fold(0_u64, |total, resource| {
        total.checked_add(resource.peak_working_bytes)
    });
    assert!(total_output.is_some_and(|value| value < 2 * 1024 * 1024));
    assert!(total_peak.is_some_and(|value| value < 4 * 1024 * 1024));
}

#[test]
fn stress_ct_planning_is_filesystem_free_and_rejects_corruption() {
    let (catalog, lock) = load();
    let recipe = catalog
        .recipes()
        .values()
        .find(|recipe| recipe.plan_provider_id == "native.stress_ct_plan")
        .unwrap();
    let absent = std::env::temp_dir().join(format!("dts-stress-ct-plan-{}", std::process::id()));
    assert!(!absent.exists());
    plan_stress_ct_recipe(recipe, &lock, 1).unwrap().unwrap();
    assert!(!absent.exists());

    let mut unknown = recipe.clone();
    unknown
        .provider_parameters
        .insert("unbounded".into(), Value::Bool(true));
    assert!(matches!(
        plan_stress_ct_recipe(&unknown, &lock, 1),
        Err(StressCtPlanError::Parameters(_))
    ));

    let mut reordered = recipe.clone();
    reordered.dicom.as_mut().unwrap().artifacts.swap(0, 1);
    assert!(matches!(
        plan_stress_ct_recipe(&reordered, &lock, 1),
        Err(StressCtPlanError::Contract(_))
    ));
}

#[test]
fn stress_ct_registry_preserves_frozen_public_standards_evidence() {
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let case = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_id"] == "stress/study/high_instance_count_ct")
        .unwrap();
    let baseline: Value = serde_json::from_slice(
        &fs::read("/tmp/dts-unified-baseline-20260829-52e1d20/stress/manifest.json").unwrap(),
    )
    .unwrap();
    let file = baseline["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == case["case_id"])
        .unwrap();
    assert_eq!(case["standards_evidence"], file["standards_evidence"]);
}
