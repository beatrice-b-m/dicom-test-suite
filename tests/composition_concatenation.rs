use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_object::open_file;
use dicom_test_suite::composition::{ComposeOptions, compose};
use serde_json::json;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-concatenation-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn enhanced_ct_concatenation_is_closed_contiguous_and_reproducible() {
    let first = root("first");
    let second = root("second");
    for out in [&first, &second] {
        compose(&ComposeOptions {
            spec_path: "tests/fixtures/composition/valid/enhanced-ct-concatenation.json".into(),
            out_dir: out.clone(),
            seed: 59,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
    }
    let names = ["ct_concat", "ct_concat__part_2"];
    let mut concatenation_uid = None;
    for (index, name) in names.iter().enumerate() {
        let object = open_file(first.join(format!("instances/{name}.dcm"))).unwrap();
        let uid = object
            .element_by_name("ConcatenationUID")
            .unwrap()
            .to_str()
            .unwrap()
            .trim()
            .to_string();
        assert_eq!(concatenation_uid.get_or_insert_with(|| uid.clone()), &uid);
        assert_eq!(
            object
                .element_by_name("InConcatenationNumber")
                .unwrap()
                .to_int::<u32>()
                .unwrap(),
            index as u32 + 1
        );
        assert_eq!(
            object
                .element_by_name("InConcatenationTotalNumber")
                .unwrap()
                .to_int::<u32>()
                .unwrap(),
            2
        );
        assert_eq!(
            object
                .element_by_name("ConcatenationFrameOffsetNumber")
                .unwrap()
                .to_int::<u32>()
                .unwrap(),
            index as u32
        );
        assert_eq!(
            fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
            fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
        );
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["composition"]["entries"].as_array().unwrap().len(),
        2
    );
    let bundle = &manifest["composition"]["bundles"][0];
    assert_eq!(bundle["dependency_closure"].as_array().unwrap().len(), 2);
    assert_eq!(bundle["references"].as_array().unwrap().len(), 1);
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn concatenation_structural_overrides_publish_nothing() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{
                "instance_id":"ct_concat",
                "template":{"id":"enhanced/ct/concatenation-part-1"},
                "attributes":[{"operation":"set", "address":{"tag":"0020,9228"}, "vr":"UL", "value":1}]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    let error = compose(&ComposeOptions {
        spec_path: spec,
        out_dir: out.clone(),
        seed: 59,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap_err();
    assert!(error.to_string().contains("0020,9228"));
    assert!(!out.exists());
    fs::remove_dir_all(workspace).unwrap();
}
