use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::composition::{TemplateCatalog, TemplateId};
use synth_dicom_gen::recipes::{
    PRESENTATION_ADVANCED_PROVIDER_ID, REGISTRATION_PLAN_PROVIDER_ID, RecipeCatalog,
    WSI_ADVANCED_PROVIDER_ID,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const ENHANCED_PROVIDER_ID: &str = "native.enhanced_plan";

struct TempFile(PathBuf);

impl TempFile {
    fn write(label: &str, value: &Value) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dicom-test-suite-default-recipe-{label}-{}-{}.json",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.0.is_file() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

fn recipe_catalog(template_path: impl AsRef<Path>) -> Result<RecipeCatalog, String> {
    RecipeCatalog::load("cases/recipes", "cases/registry.json", template_path)
        .map_err(|error| error.to_string())
}

fn provider_family(template_id: &str, artifact_kind: &str) -> Option<&'static str> {
    match artifact_kind {
        "enhanced_image" if template_id.starts_with("enhanced/") => Some(ENHANCED_PROVIDER_ID),
        "whole_slide_image" if template_id.starts_with("vl/wsi/") => Some(WSI_ADVANCED_PROVIDER_ID),
        "registration" if template_id.starts_with("derived/registration/") => {
            Some(REGISTRATION_PLAN_PROVIDER_ID)
        }
        "presentation_state" if template_id.starts_with("derived/presentation-state/") => {
            Some(PRESENTATION_ADVANCED_PROVIDER_ID)
        }
        _ => None,
    }
}

#[test]
fn every_committed_advanced_template_has_one_exact_default_recipe_artifact() {
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    let recipes = recipe_catalog("templates/catalog.json").unwrap();
    let provider_templates = templates
        .templates
        .iter()
        .filter_map(|template| {
            provider_family(&template.template_id.0, &template.artifact_kind)
                .map(|provider| (template, provider))
        })
        .collect::<Vec<_>>();
    assert!(!provider_templates.is_empty());

    let mut bindings = BTreeSet::new();
    let mut families = BTreeMap::<&str, usize>::new();
    for (template, provider_family) in provider_templates {
        let binding = template.default_recipe.as_ref().unwrap_or_else(|| {
            panic!(
                "{}@{} lacks a default recipe",
                template.template_id, template.template_version
            )
        });
        assert!(bindings.insert((
            binding.recipe_id.as_str(),
            binding.recipe_version.as_str(),
            binding.artifact_logical_id.as_str(),
        )));
        let identity = synth_dicom_gen::planning::RecipeIdentity {
            recipe_id: binding.recipe_id.clone(),
            recipe_version: binding.recipe_version.clone(),
        };
        let recipe = &recipes.recipes()[&identity];
        let artifact = recipe
            .dicom
            .as_ref()
            .unwrap()
            .artifacts
            .iter()
            .find(|artifact| artifact.logical_id == binding.artifact_logical_id)
            .unwrap();
        let artifact_template = artifact.template.as_ref().unwrap();
        assert_eq!(artifact_template.template_id, template.template_id.0);
        assert_eq!(
            artifact_template.template_version,
            template.template_version.to_string()
        );
        match provider_family {
            ENHANCED_PROVIDER_ID | WSI_ADVANCED_PROVIDER_ID => {
                assert_eq!(recipe.plan_provider_id, provider_family)
            }
            REGISTRATION_PLAN_PROVIDER_ID => {
                assert!(recipe.binding.case_id.starts_with("derived/registration/"))
            }
            PRESENTATION_ADVANCED_PROVIDER_ID => assert!(
                recipe
                    .binding
                    .case_id
                    .starts_with("derived/presentation-state/")
            ),
            _ => unreachable!(),
        }
        *families.entry(provider_family).or_default() += 1;
    }
    assert_eq!(
        families.keys().copied().collect::<BTreeSet<_>>(),
        [
            ENHANCED_PROVIDER_ID,
            WSI_ADVANCED_PROVIDER_ID,
            REGISTRATION_PLAN_PROVIDER_ID,
            PRESENTATION_ADVANCED_PROVIDER_ID,
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn default_recipe_schema_is_strict() {
    let mut catalog: Value =
        serde_json::from_slice(&fs::read("templates/catalog.json").unwrap()).unwrap();
    let template = catalog["advanced_family_templates"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|template| template["default_recipe"].is_object())
        .unwrap();
    template["default_recipe"]["unexpected"] = Value::Bool(true);
    assert!(TemplateCatalog::from_slice(&serde_json::to_vec(&catalog).unwrap()).is_err());
}

#[test]
fn recipe_catalog_rejects_dangling_and_template_mismatched_defaults() {
    let original: Value =
        serde_json::from_slice(&fs::read("templates/catalog.json").unwrap()).unwrap();

    let mut dangling = original.clone();
    let enhanced_ct = advanced_template_mut(&mut dangling, "enhanced/ct");
    enhanced_ct["default_recipe"]["recipe_id"] = Value::String("missing_recipe".into());
    let dangling_path = TempFile::write("dangling", &dangling);
    let error = recipe_catalog(&dangling_path.0).unwrap_err();
    assert!(error.contains("references unknown"), "{error}");

    let mut missing_artifact = original.clone();
    let enhanced_ct = advanced_template_mut(&mut missing_artifact, "enhanced/ct");
    enhanced_ct["default_recipe"]["artifact_logical_id"] = Value::String("missing_artifact".into());
    let missing_path = TempFile::write("missing-artifact", &missing_artifact);
    let error = recipe_catalog(&missing_path.0).unwrap_err();
    assert!(error.contains("is missing"), "{error}");

    let mut mismatched = original;
    let enhanced_ct = advanced_template_mut(&mut mismatched, "enhanced/ct");
    enhanced_ct["default_recipe"] = serde_json::json!({
        "recipe_id": "enhanced_ct_concatenation_two_part",
        "recipe_version": "0.1.0",
        "artifact_logical_id": "advanced_enhanced_ct_concatenation_two_part_artifact_1"
    });
    let mismatched_path = TempFile::write("mismatched", &mismatched);
    let error = recipe_catalog(&mismatched_path.0).unwrap_err();
    assert!(
        error.contains("resolves as enhanced/ct/concatenation-part-1@1.0.0"),
        "{error}"
    );
}

fn advanced_template_mut<'a>(catalog: &'a mut Value, template_id: &str) -> &'a mut Value {
    catalog["advanced_family_templates"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|template| template["template_id"] == template_id)
        .unwrap()
}

#[test]
fn default_recipe_bindings_are_exposed_by_exact_template_identity() {
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    for template in templates
        .templates
        .iter()
        .filter(|template| template.default_recipe.is_some())
    {
        let resolved = templates
            .resolve_qualified(
                &TemplateId(template.template_id.0.clone()),
                Some(template.template_version),
            )
            .unwrap();
        assert_eq!(resolved.default_recipe, template.default_recipe);
    }
}
