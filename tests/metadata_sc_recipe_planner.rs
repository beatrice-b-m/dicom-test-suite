use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::composition::{CompositionUidRole, Part10Materializer, TemplateCatalog};
use dicom_test_suite::corpus_plan::ImplementationIdentityPlan;
use dicom_test_suite::recipes::{
    MetadataScPlanInput, RecipeCatalog, encoding_plan_from_recipe, resolved_metadata_sc_plan,
};
use dicom_test_suite::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const IMPLEMENTATION_VERSION_NAME: &str = "DICOMTS010";

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-metadata-sc-planner-{label}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn every_typed_metadata_sc_artifact_is_byte_identical_to_the_current_generator() {
    let generated_root = temp_dir("generated");
    let planned_root = temp_dir("planned");
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: generated_root.clone(),
        seed: 7,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    fs::create_dir(&planned_root).unwrap();

    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    let lock_hash = sha256_hex(&fs::read("standards.lock.json").unwrap());
    let mut planned_cases = BTreeSet::new();
    let mut planned_artifacts = BTreeSet::new();

    for recipe in recipes
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.metadata_sc_plan")
    {
        planned_cases.insert(recipe.binding.case_id.clone());
        let dicom = recipe.dicom.as_ref().unwrap();
        for artifact in &dicom.artifacts {
            assert!(artifact.metadata_sc.is_some());
            let reference = artifact.template.as_ref().unwrap();
            let template = templates
                .resolve_qualified(
                    &dicom_test_suite::composition::TemplateId(reference.template_id.clone()),
                    Some(reference.template_version.parse().unwrap()),
                )
                .unwrap();
            let plan = resolved_metadata_sc_plan(MetadataScPlanInput {
                recipe,
                artifact,
                template,
                instance_id: &recipe.recipe_id,
                standards_lock_sha256: &lock_hash,
                seed: 7,
            })
            .unwrap();
            let implementation = ImplementationIdentityPlan {
                class_uid: plan
                    .identities
                    .get(&CompositionUidRole::ImplementationClass, 0)
                    .unwrap()
                    .to_owned(),
                version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
            };
            let encoding = encoding_plan_from_recipe(&artifact.encoding, implementation).unwrap();
            let planned_path =
                planned_root.join(format!("{}-{}.dcm", recipe.recipe_id, artifact.logical_id));
            Part10Materializer
                .materialize_with_encoding(&plan, &encoding, &planned_path)
                .unwrap();
            let relative_path = artifact
                .output
                .path
                .as_deref()
                .expect("metadata SC output paths are statically bounded");
            assert_eq!(
                fs::read(&planned_path).unwrap(),
                fs::read(generated_root.join(relative_path)).unwrap(),
                "{}/{}",
                recipe.binding.case_id,
                artifact.logical_id
            );
            assert!(planned_artifacts.insert((
                recipe.binding.case_id.clone(),
                artifact.order,
                artifact.logical_id.clone(),
                relative_path.to_owned(),
            )));
        }
    }

    assert!(!planned_cases.is_empty());
    assert!(planned_artifacts.len() >= planned_cases.len());
    assert!(planned_artifacts.iter().all(|(case_id, _, _, path)| {
        path.starts_with(case_id) && generated_root.join(path).is_file()
    }));

    fs::remove_dir_all(generated_root).unwrap();
    fs::remove_dir_all(planned_root).unwrap();
}

#[test]
fn metadata_planner_rejects_ordinary_sc_recipes_explicitly() {
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    let lock_hash = sha256_hex(&fs::read("standards.lock.json").unwrap());
    let identity = recipes
        .binding_for_case("classic/sc/mono2_u8_explicit_le")
        .unwrap();
    let recipe = recipes.recipes().get(identity).unwrap();
    let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
    let reference = artifact.template.as_ref().unwrap();
    let template = templates
        .resolve_qualified(
            &dicom_test_suite::composition::TemplateId(reference.template_id.clone()),
            Some(reference.template_version.parse().unwrap()),
        )
        .unwrap();

    let error = resolved_metadata_sc_plan(MetadataScPlanInput {
        recipe,
        artifact,
        template,
        instance_id: &recipe.recipe_id,
        standards_lock_sha256: &lock_hash,
        seed: 7,
    })
    .unwrap_err();
    assert!(error.to_string().contains("WrongPlanProvider"));
}
