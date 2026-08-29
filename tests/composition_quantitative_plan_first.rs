use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::composition::{ComposeOptions, compose};
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-quantitative-plan-first-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn native_quantitative_defaults_use_planned_bundle_sources() {
    let workspace = root();
    fs::create_dir(&workspace).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                {"instance_id":"binary", "template":{"id":"derived/segmentation/binary"}},
                {"instance_id":"fractional", "template":{"id":"derived/segmentation/fractional-probability"}},
                {"instance_id":"labelmap", "template":{"id":"derived/segmentation/labelmap"}},
                {"instance_id":"rwvm", "template":{"id":"derived/real-world-value-mapping/linear"}}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    compose(&ComposeOptions {
        spec_path: spec,
        out_dir: out.clone(),
        seed: 69,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();

    let manifest: Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let entries = manifest["composition"]["entries"].as_array().unwrap();
    let ids = entries
        .iter()
        .map(|entry| entry["instance_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "binary",
            "binary__source",
            "fractional",
            "fractional__source",
            "labelmap",
            "labelmap__source",
            "rwvm",
            "rwvm__source"
        ]
    );
    for entry in entries {
        let id = entry["instance_id"].as_str().unwrap();
        assert_eq!(entry["requested"], !id.ends_with("__source"));
        if id.ends_with("__source") {
            assert_eq!(entry["source_provenance"], "default_template_dependency");
            assert_eq!(entry["bundle_role"], "source");
        } else {
            assert_eq!(entry["references"].as_array().unwrap().len(), 1);
        }
    }
    assert!(!out.join(".composition-private").exists());
    assert!(!include_str!("../src/composition/advanced_defaults.rs").contains("crate::generator"));
    assert!(!include_str!("../src/composition/run.rs").contains("crate::generator"));
    fs::remove_dir_all(workspace).unwrap();
}
