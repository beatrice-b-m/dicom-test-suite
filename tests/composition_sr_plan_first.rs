use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use synth_dicom_gen::composition::{ComposeOptions, compose};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-sr-plan-first-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn native_sr_defaults_use_planned_bundle_sources() {
    let workspace = root();
    fs::create_dir(&workspace).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                {"instance_id":"basic", "template":{"id":"derived/structured-report/basic-text"}},
                {"instance_id":"comprehensive", "template":{"id":"derived/structured-report/comprehensive"}},
                {"instance_id":"kos", "template":{"id":"derived/structured-report/key-object"}}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    compose(&ComposeOptions {
        spec_path: spec,
        out_dir: out.clone(),
        seed: 72,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();

    let manifest: Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let requested = manifest["composition"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["requested"] == true)
        .collect::<Vec<_>>();
    assert_eq!(
        requested
            .iter()
            .map(|entry| entry["instance_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["basic", "comprehensive", "kos"]
    );
    assert!(
        requested
            .iter()
            .all(|entry| !entry["references"].as_array().unwrap().is_empty())
    );
    assert!(
        !include_str!("../src/composition/advanced_semantic_defaults.rs")
            .contains("crate::generator")
    );
    fs::remove_dir_all(workspace).unwrap();
}
