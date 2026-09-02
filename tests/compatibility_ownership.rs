use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde_json::Value;

#[test]
fn every_public_schema_and_api_has_exactly_one_owner_and_window() {
    let registry: Value =
        serde_json::from_slice(&fs::read("product/compatibility-owners.json").unwrap()).unwrap();
    assert_eq!(registry["compatibility_ownership_schema_version"], "1.0.0");
    let contracts = registry["contracts"].as_array().unwrap();
    let mut schema_owners = BTreeMap::<String, Vec<String>>::new();
    let mut apis = BTreeSet::new();
    for contract in contracts {
        let id = contract["contract_id"].as_str().unwrap();
        assert!(!contract["owner"].as_str().unwrap().is_empty(), "{id}");
        assert!(
            contract["support_window"].as_str().unwrap().contains(' '),
            "{id} lacks a meaningful support window"
        );
        for schema in contract["schemas"].as_array().unwrap() {
            schema_owners
                .entry(schema.as_str().unwrap().to_string())
                .or_default()
                .push(id.to_string());
        }
        for api in contract["apis"].as_array().unwrap() {
            assert!(apis.insert(api.as_str().unwrap().to_string()));
        }
    }
    let public_schemas = fs::read_dir("schemas")
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".schema.json"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        schema_owners.keys().cloned().collect::<BTreeSet<_>>(),
        public_schemas
    );
    assert!(schema_owners.values().all(|owners| owners.len() == 1));
    for required in [
        "cli_json_envelopes",
        "generate",
        "compose",
        "assemble",
        "templates",
        "conformance",
        "interoperate",
        "synth_dicom_gen::sdk",
        "native_archive",
    ] {
        assert!(apis.contains(required), "unowned API: {required}");
    }
}
