use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use dicom_test_suite::composition::{TemplateCatalog, TemplateStatus};
use serde_json::Value;

fn json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}")))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn rust_sources(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_sources(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

#[test]
fn implemented_registry_recipe_identities_are_complete_and_unique() {
    let registry = json("cases/registry.json");
    let mut identities = BTreeMap::new();
    for case in registry["cases"].as_array().expect("registry cases") {
        if case["status"] != "implemented" {
            continue;
        }
        let case_id = case["case_id"].as_str().expect("implemented case ID");
        let recipe_id = case["recipe_id"].as_str().expect("implemented recipe ID");
        let recipe_version = case["recipe_version"]
            .as_str()
            .expect("implemented recipe version");
        assert!(!recipe_id.is_empty(), "{case_id} has an empty recipe ID");
        assert!(
            !recipe_version.is_empty(),
            "{case_id} has an empty recipe version"
        );
        assert!(
            identities
                .insert((recipe_id, recipe_version), case_id)
                .is_none(),
            "duplicate implemented recipe binding {recipe_id}@{recipe_version}"
        );
        assert!(
            case["standards_evidence"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty()),
            "{case_id} has no standards evidence"
        );
    }
    assert!(
        !identities.is_empty(),
        "implemented registry must not be empty"
    );
}

#[test]
fn valid_registry_sop_classes_resolve_to_qualified_template_families() {
    let registry = json("cases/registry.json");
    let inventory = json("templates/inventory.json");
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();

    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["status"] == "implemented"
                && case["artifact_kind"] == "dicom_instance"
                && !case["profiles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|profile| profile == "negative" || profile == "fuzz")
        })
        .map(|case| case["sop_class_uid"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let mappings = inventory["mappings"].as_array().unwrap();
    let actual = mappings
        .iter()
        .map(|mapping| mapping["sop_class_uid"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    for mapping in mappings {
        let uid = mapping["sop_class_uid"].as_str().unwrap();
        let family = mapping["template_family"].as_str().unwrap();
        assert!(
            catalog.templates.iter().any(|template| {
                template.status == TemplateStatus::Qualified
                    && template.sop_class_uid == uid
                    && (template.template_id.0 == family
                        || template.template_id.0.starts_with(&format!("{family}/")))
            }),
            "{uid} does not resolve through qualified family {family}"
        );
    }
}

#[test]
fn every_qualified_template_retains_validation_and_evidence_routes() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    for template in catalog
        .templates
        .iter()
        .filter(|template| template.status == TemplateStatus::Qualified)
    {
        assert!(
            template.validation["generic_rule_ids"]
                .as_array()
                .is_some_and(|rules| !rules.is_empty()),
            "{} has no generic validation rule",
            template.template_id
        );
        assert!(
            template.validation["template_rule_ids"]
                .as_array()
                .is_some_and(|rules| !rules.is_empty()),
            "{} has no template validation rule",
            template.template_id
        );
        assert!(
            template.validation["independent_routes"]
                .as_array()
                .is_some_and(|routes| !routes.is_empty()),
            "{} has no independent evidence route",
            template.template_id
        );
        assert!(
            !template.standards_evidence.is_empty(),
            "{} has no standards evidence",
            template.template_id
        );
    }
}

#[test]
fn every_current_production_direct_writer_is_classified_for_removal() {
    let audit = fs::read_to_string("docs/unified-generation-spine-audit.md").unwrap();
    let allowed = BTreeSet::from([
        PathBuf::from("src/composition/materializer.rs"),
        PathBuf::from("src/executor/materialization.rs"),
    ]);
    let mut sources = Vec::new();
    rust_sources(Path::new("src"), &mut sources);
    for path in sources {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_tests.rs"))
        {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let Some(writer_offset) = source.find(".write_to_file(") else {
            continue;
        };
        if source
            .find("#[cfg(test)]")
            .is_some_and(|test_offset| test_offset < writer_offset)
        {
            continue;
        }
        assert!(
            allowed.contains(&path),
            "unclassified production direct writer in {}",
            path.display()
        );
        assert!(
            audit.contains(path.to_str().unwrap()),
            "{} is not classified in the U0 audit",
            path.display()
        );
    }

    for (path, marker, removal) in [
        ("src/codecs.rs", "dcmcjpeg", "U7.2"),
        ("src/generation_backends/", "external", "U6.7"),
        ("src/negative.rs", "mutation", "U8"),
    ] {
        assert!(audit.contains(path), "audit does not classify {path}");
        assert!(audit.contains(marker), "audit does not name {marker}");
        assert!(
            audit.contains(removal),
            "audit does not assign {path} to {removal}"
        );
    }
}

#[test]
fn every_temporary_bridge_is_named_and_assigned_to_a_removal_task() {
    let audit = fs::read_to_string("docs/unified-generation-spine-audit.md").unwrap();
    let advanced = fs::read_to_string("src/composition/advanced_family.rs").unwrap();
    assert!(
        !Path::new("src/generator.rs").exists(),
        "the retired curated generator module must be absent"
    );
    assert!(
        !advanced.contains("write_composition_default_artifacts"),
        "composition defaults must not invoke or retain a curated generator bridge"
    );
    assert!(!Path::new("src/composition/curated.rs").exists());
    assert!(audit.contains("resolved_plan_from_curated_dataset"));
    let mut sources = Vec::new();
    rust_sources(Path::new("src"), &mut sources);
    assert!(sources.into_iter().all(|path| {
        !fs::read_to_string(path)
            .unwrap()
            .contains("resolved_plan_from_curated_dataset")
    }));
}
