use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::header::Header;
use dicom_dictionary_std::tags;
use dicom_object::{InMemDicomObject, open_file};
use dicom_test_suite::composition::{ComposeOptions, compose};
use serde_json::{Value, json};

#[path = "support/prepared_backend.rs"]
mod prepared_backend;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-quantitative-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn run(spec: impl Into<PathBuf>, out: PathBuf, seed: u64) {
    let _backend = prepared_backend::PreparedBackendOverride::acquire();
    compose(&ComposeOptions {
        spec_path: spec.into(),
        out_dir: out,
        seed,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
}

fn referenced_sop_uids(object: &InMemDicomObject) -> BTreeSet<String> {
    fn visit(object: &InMemDicomObject, output: &mut BTreeSet<String>) {
        for element in object.iter() {
            if element.tag() == tags::REFERENCED_SOP_INSTANCE_UID {
                output.insert(element.to_str().unwrap().trim().to_string());
            }
            if let Some(items) = element.items() {
                for item in items {
                    visit(item, output);
                }
            }
        }
    }
    let mut output = BTreeSet::new();
    visit(object, &mut output);
    output
}

#[test]
fn quantitative_default_bundles_are_closed_provenanced_and_reproducible() {
    let first = root("defaults-a");
    let second = root("defaults-b");
    for out in [&first, &second] {
        run(
            "tests/fixtures/composition/valid/p6-quantitative-defaults.json",
            out.clone(),
            69,
        );
    }
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    let manifest_bytes = fs::read(first.join("manifest.json")).unwrap();
    assert_eq!(
        dicom_test_suite::sha256_hex(&manifest_bytes),
        "564d46f713085ef6471bfa5b37e5da19c42e13fe44e4f54a1729f0a96fa27f64"
    );
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    for (instance_id, resolved_plan_sha256) in [
        (
            "wsi_seg",
            "91e496df7d7ff13cccb8a5d9142efd6f94365033604c60000c4ca77ae46fa152",
        ),
        (
            "float32",
            "79c94d62904e9a421aa6f74a5735f49e0b6073696307066f0c46d34013b1f15c",
        ),
        (
            "float64",
            "96d8f90e920434bc097d43bab383bfffeec76f44e60b5869d2723b09a07c2e9e",
        ),
    ] {
        let entry = manifest["composition"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["instance_id"] == instance_id)
            .unwrap();
        assert_eq!(entry["resolved_plan_sha256"], resolved_plan_sha256);
    }
    for instance_id in [
        "binary",
        "fractional",
        "labelmap",
        "wsi_seg",
        "float32",
        "float64",
        "rwvm",
    ] {
        let entry = manifest["composition"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["instance_id"] == instance_id)
            .unwrap();
        let graph = entry["references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|reference| {
                reference["referenced_sop_instance_uid"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<BTreeSet<_>>();
        let object = open_file(first.join(entry["path"].as_str().unwrap()))
            .unwrap()
            .into_inner();
        assert_eq!(referenced_sop_uids(&object), graph, "{instance_id}");
        for content in entry["content"].as_array().unwrap() {
            assert_eq!(content["sha256"].as_str().unwrap().len(), 64);
            assert!(content["properties"]["bulk_source"].is_string());
            assert!(content["properties"]["semantic_validator"].is_string());
        }
    }
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn caller_segmentation_and_parametric_values_round_trip_at_fixed_shape() {
    let workspace = root("caller");
    fs::create_dir(&workspace).unwrap();
    let defaults = workspace.join("defaults");
    run(
        "tests/fixtures/composition/valid/p6-quantitative-defaults.json",
        defaults.clone(),
        70,
    );
    let binary = open_file(defaults.join("instances/binary.dcm"))
        .unwrap()
        .element(tags::PIXEL_DATA)
        .unwrap()
        .to_bytes()
        .unwrap()
        .into_owned();
    let mut float32 = open_file(defaults.join("instances/float32.dcm"))
        .unwrap()
        .element(tags::FLOAT_PIXEL_DATA)
        .unwrap()
        .to_bytes()
        .unwrap()
        .into_owned();
    float32[..4].copy_from_slice(&42.5_f32.to_le_bytes());
    fs::write(workspace.join("binary.raw"), &binary).unwrap();
    fs::write(workspace.join("float32.raw"), &float32).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[
                {"instance_id":"binary", "template":{"id":"derived/segmentation/binary"}, "content":[{"slot":"pixels", "source":{"kind":"local_file", "path":"binary.raw"}}]},
                {"instance_id":"float32", "template":{"id":"derived/parametric-map/float32"}, "content":[{"slot":"pixels", "source":{"kind":"local_file", "path":"float32.raw"}}]}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    run(spec, out.clone(), 70);
    assert_eq!(
        open_file(out.join("instances/binary.dcm"))
            .unwrap()
            .element(tags::PIXEL_DATA)
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        binary
    );
    assert_eq!(
        open_file(out.join("instances/float32.dcm"))
            .unwrap()
            .element(tags::FLOAT_PIXEL_DATA)
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        float32
    );
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn non_finite_or_wrong_length_quantitative_values_fail_before_publication() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("nan.raw"), f32::NAN.to_le_bytes()).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{"instance_id":"map", "template":{"id":"derived/parametric-map/float32"}, "content":[{"slot":"pixels", "source":{"kind":"local_file", "path":"nan.raw"}}]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    assert!(
        compose(&ComposeOptions {
            spec_path: spec,
            out_dir: out.clone(),
            seed: 71,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .is_err()
    );
    assert!(!out.exists());
    fs::remove_dir_all(workspace).unwrap();
}
