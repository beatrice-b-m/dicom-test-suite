use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::header::Header;
use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_object::{open_file, InMemDicomObject};
use dicom_test_suite::composition::{compose, ComposeOptions};
use serde_json::{json, Value};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-rt-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn run(spec: impl Into<PathBuf>, out: PathBuf, seed: u64) {
    compose(&ComposeOptions {
        spec_path: spec.into(),
        out_dir: out,
        seed,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
}

fn strings_for(object: &InMemDicomObject, tag: Tag) -> Vec<String> {
    fn visit(object: &InMemDicomObject, tag: Tag, output: &mut Vec<String>) {
        for element in object.iter() {
            if element.tag() == tag {
                output.push(element.to_str().unwrap().trim().to_string());
            }
            if let Some(items) = element.items() {
                for item in items {
                    visit(item, tag, output);
                }
            }
        }
    }
    let mut output = Vec::new();
    visit(object, tag, &mut output);
    output
}

fn referenced_sop_uids(object: &InMemDicomObject) -> BTreeSet<String> {
    strings_for(object, tags::REFERENCED_SOP_INSTANCE_UID)
        .into_iter()
        .collect()
}

#[test]
fn radiotherapy_defaults_have_closed_reproducible_reference_graphs() {
    let first = root("defaults-a");
    let second = root("defaults-b");
    for out in [&first, &second] {
        run(
            "tests/fixtures/composition/valid/p6-radiotherapy-defaults.json",
            out.clone(),
            75,
        );
    }
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
    for instance_id in [
        "structure",
        "dose",
        "plan",
        "image",
        "radiation",
        "radiation-set",
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
    }
    for instance_id in ["dose", "image"] {
        let entry = manifest["composition"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["instance_id"] == instance_id)
            .unwrap();
        let content = &entry["content"][0];
        assert!(content["properties"]["bulk_source"]
            .as_str()
            .is_some_and(|source| source.contains("default_synthetic")));
        assert!(content["sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64));
        assert_eq!(content["size_bytes"], 16);
    }
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn radiotherapy_typed_pixels_and_semantic_parameters_round_trip() {
    let workspace = root("caller");
    fs::create_dir(&workspace).unwrap();
    let dose_bytes = (0_u16..8).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    let image_bytes = (0_u8..16).rev().collect::<Vec<_>>();
    fs::write(workspace.join("dose.raw"), &dose_bytes).unwrap();
    fs::write(workspace.join("image.raw"), &image_bytes).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(&spec, serde_json::to_vec_pretty(&json!({
        "composition_spec_schema_version":"0.1.0",
        "instances":[
            {"instance_id":"structure", "template":{"id":"non-image/rt/structure-set"}, "parameters":{"roi_name":"CALLER_ROI"}},
            {"instance_id":"dose", "template":{"id":"non-image/rt/dose"}, "parameters":{"dose_grid_scaling":0.002}, "content":[{"slot":"pixels", "source":{"kind":"local_file","path":"dose.raw"}}]},
            {"instance_id":"plan", "template":{"id":"non-image/rt/plan"}, "parameters":{"plan_label":"CALLER_PLAN"}},
            {"instance_id":"image", "template":{"id":"non-image/rt/image"}, "content":[{"slot":"pixels", "source":{"kind":"local_file","path":"image.raw"}}]}
        ]
    })).unwrap()).unwrap();
    let out = workspace.join("out");
    run(spec, out.clone(), 76);

    let structure = open_file(out.join("instances/structure.dcm"))
        .unwrap()
        .into_inner();
    assert!(strings_for(&structure, Tag(0x3006, 0x0026)).contains(&"CALLER_ROI".into()));
    let dose = open_file(out.join("instances/dose.dcm"))
        .unwrap()
        .into_inner();
    assert_eq!(
        dose.element(tags::PIXEL_DATA).unwrap().to_bytes().unwrap(),
        dose_bytes.as_slice()
    );
    assert!(strings_for(&dose, Tag(0x3004, 0x000E)).contains(&"0.002".into()));
    let plan = open_file(out.join("instances/plan.dcm"))
        .unwrap()
        .into_inner();
    assert!(strings_for(&plan, Tag(0x300A, 0x0002)).contains(&"CALLER_PLAN".into()));
    let image = open_file(out.join("instances/image.dcm"))
        .unwrap()
        .into_inner();
    assert_eq!(
        image.element(tags::PIXEL_DATA).unwrap().to_bytes().unwrap(),
        image_bytes.as_slice()
    );
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn radiotherapy_rejects_wrong_payload_size_and_unknown_parameters_atomically() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("short.raw"), [0_u8; 2]).unwrap();
    for (label, instance) in [
        (
            "short",
            json!({"instance_id":"short", "template":{"id":"non-image/rt/dose"}, "content":[{"slot":"pixels", "source":{"kind":"local_file","path":"short.raw"}}]}),
        ),
        (
            "unknown",
            json!({"instance_id":"unknown", "template":{"id":"non-image/rt/plan"}, "parameters":{"beam_tree":[]}}),
        ),
    ] {
        let spec = workspace.join(format!("{label}.json"));
        fs::write(
            &spec,
            serde_json::to_vec_pretty(&json!({
                "composition_spec_schema_version":"0.1.0", "instances":[instance]
            }))
            .unwrap(),
        )
        .unwrap();
        let out = workspace.join(format!("{label}-out"));
        assert!(compose(&ComposeOptions {
            spec_path: spec,
            out_dir: out.clone(),
            seed: 77,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .is_err());
        assert!(!out.exists());
    }
    fs::remove_dir_all(workspace).unwrap();
}
