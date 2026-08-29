use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use dicom_test_suite::composition::{TemplateCatalog, TemplateStatus};
use serde_json::Value;

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}")))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn inventory_exactly_covers_implemented_valid_dicom_sop_classes() {
    let registry = read_json("cases/registry.json");
    let inventory = read_json("templates/inventory.json");

    let mut expected = BTreeMap::new();
    for case in registry["cases"].as_array().expect("registry cases") {
        if case["status"] != "implemented" || case["artifact_kind"] != "dicom_instance" {
            continue;
        }
        let profiles = case["profiles"].as_array().expect("case profiles");
        if profiles.iter().any(|profile| profile == "negative" || profile == "fuzz") {
            continue;
        }
        let uid = case["sop_class_uid"].as_str().expect("valid instance SOP Class UID");
        let identity = (
            case["sop_class_name"].as_str().expect("SOP Class name"),
            case["iod_name"].as_str().expect("IOD name"),
        );
        if let Some(previous) = expected.insert(uid, identity) {
            assert_eq!(previous, identity, "registry identity drift for {uid}");
        }
    }

    let mappings = inventory["mappings"].as_array().expect("inventory mappings");
    let mut actual = BTreeMap::new();
    let allowed_tasks = BTreeSet::from([
        "P2.1", "P3.2", "P3.3", "P3.4", "P3.5", "P3.6", "P3.7", "P5.4",
        "P5.5", "P5.6", "P5.7", "P6.2", "P6.3", "P6.4", "P6.5", "P6.6",
    ]);
    for mapping in mappings {
        let uid = mapping["sop_class_uid"].as_str().expect("mapped SOP Class UID");
        let identity = (
            mapping["sop_class_name"].as_str().expect("mapped SOP Class name"),
            mapping["iod_name"].as_str().expect("mapped IOD name"),
        );
        assert!(actual.insert(uid, identity).is_none(), "duplicate mapping for {uid}");
        assert!(
            matches!(mapping["coverage_kind"].as_str(), Some("template" | "bundle")),
            "{uid} must declare template or bundle coverage"
        );
        assert!(
            mapping["template_family"].as_str().is_some_and(|value| !value.is_empty()),
            "{uid} must name a template family"
        );
        assert!(
            mapping["qualification_owner"].as_str().is_some_and(|value| !value.is_empty()),
            "{uid} must name a qualification owner"
        );
        assert!(
            mapping["phase_task"].as_str().is_some_and(|task| allowed_tasks.contains(task)),
            "{uid} must map to a composition-plan family task"
        );
    }

    assert_eq!(actual, expected, "template inventory must exactly match registry scope");
}

#[test]
fn inventory_records_all_out_of_scope_qualification_domains() {
    let inventory = read_json("templates/inventory.json");
    let scopes = inventory["excluded_qualification_scopes"]
        .as_array()
        .expect("excluded scopes")
        .iter()
        .map(|entry| entry["scope"].as_str().expect("scope"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        scopes,
        BTreeSet::from(["negative", "fuzz", "media", "protocol", "qualification"])
    );
}

#[test]
fn every_inventory_mapping_resolves_to_a_qualified_catalog_template() {
    let inventory = read_json("templates/inventory.json");
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();

    for mapping in inventory["mappings"].as_array().unwrap() {
        let uid = mapping["sop_class_uid"].as_str().unwrap();
        let family = mapping["template_family"].as_str().unwrap();
        let candidates = catalog
            .templates
            .iter()
            .filter(|template| template.sop_class_uid == uid)
            .collect::<Vec<_>>();
        assert!(!candidates.is_empty(), "{uid} has no catalog descriptor");
        assert!(
            candidates.iter().any(|template| {
                template.status == TemplateStatus::Qualified
                    && (template.template_id.0 == family
                        || template.template_id.0.starts_with(&format!("{family}/")))
            }),
            "{uid} inventory family {family} has no qualified catalog implementation"
        );
    }
}

#[test]
fn p6_bulk_templates_bind_content_rules_and_independent_semantic_routes() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    for template in catalog.templates.iter().filter(|template| {
        template.template_id.0.starts_with("derived/segmentation/")
            || template.template_id.0.starts_with("derived/parametric-map/")
            || template.template_id.0.starts_with("non-image/")
    }) {
        if template.content_slots.is_empty() {
            continue;
        }
        assert!(
            template.validation["content_rule_ids"]
                .as_array()
                .is_some_and(|rules| !rules.is_empty()),
            "{} must declare content validation rules",
            template.template_id
        );
        assert!(
            template.validation["independent_routes"]
                .as_array()
                .is_some_and(|routes| routes.iter().any(|route| {
                    route["required_for_qualification"] == true && route["kind"] != "iod"
                })),
            "{} must declare a required independent semantic route",
            template.template_id
        );
    }
}

#[test]
fn descriptors_expose_every_executable_caller_content_source() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    for template in &catalog.templates {
        for slot in &template.content_slots {
            let sources = slot["allowed_sources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|source| source.as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert!(sources.contains("default"), "{}", template.template_id);
            assert!(sources.contains("local_file"), "{}", template.template_id);
            assert!(
                sources.contains("inline_small_fixture"),
                "{}",
                template.template_id
            );
            assert!(sources.contains("provider"), "{}", template.template_id);
            let encoded = matches!(
                template.template_id.0.as_str(),
                "classic/xa" | "classic/xrf"
            );
            assert_eq!(
                sources.contains("encoded_frames"),
                encoded,
                "{}",
                template.template_id
            );
        }
    }
}
