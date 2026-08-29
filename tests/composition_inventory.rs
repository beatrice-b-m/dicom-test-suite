use std::collections::{BTreeMap, BTreeSet};
use std::fs;

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
