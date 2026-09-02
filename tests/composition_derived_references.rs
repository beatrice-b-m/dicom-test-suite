use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::header::Header;
use dicom_dictionary_std::tags;
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};
use synth_dicom_gen::composition::{ComposeOptions, compose};
use synth_dicom_gen::sha256_hex;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-derived-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn oracle_digest(root: &PathBuf) -> String {
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let entries=manifest["composition"]["entries"].as_array().unwrap().iter().map(|entry| {
        let path=entry["path"].as_str().unwrap();
        json!({"instance_id":entry["instance_id"],"template_id":entry["template_id"],
            "uids":entry["uids"],"resolved_plan_sha256":entry["resolved_plan_sha256"],
            "content":entry["content"],"references":entry["references"],"path":path,
            "sha256":entry["sha256"],"payload_sha256":sha256_hex(&fs::read(root.join(path)).unwrap())})
    }).collect::<Vec<_>>();
    sha256_hex(
        &serde_json::to_vec(
            &json!({"entries":entries,"bundles":manifest["composition"]["bundles"]}),
        )
        .unwrap(),
    )
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
fn registration_and_presentation_defaults_are_closed_and_reproducible() {
    let first = root("first");
    let second = root("second");
    for out in [&first, &second] {
        run(
            "tests/fixtures/composition/valid/registration-presentation-defaults.json",
            out.clone(),
            60,
        );
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["composition"]["entries"].as_array().unwrap().len(),
        20
    );
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    for instance_id in [
        "spatial",
        "deformable",
        "grayscale",
        "color",
        "blending",
        "advanced_blending",
    ] {
        let entry = manifest["composition"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["instance_id"] == instance_id)
            .unwrap();
        let graph_uids = entry["references"]
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
        assert_eq!(referenced_sop_uids(&object), graph_uids, "{instance_id}");
        assert_eq!(
            fs::read(first.join(entry["path"].as_str().unwrap())).unwrap(),
            fs::read(second.join(entry["path"].as_str().unwrap())).unwrap()
        );
    }
    assert_eq!(
        oracle_digest(&first),
        "f7a01cfcf7046d43a70fad9304345509f49f72a9d300a1b86312b5cd539439ce"
    );
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn explicit_source_suppresses_default_and_rewrites_embedded_reference() {
    let workspace = root("explicit");
    fs::create_dir(&workspace).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[
                {
                    "instance_id":"color", "template":{"id":"derived/presentation-state/color"},
                    "references":[{"role":"source_image", "target_instance_id":"rgb"}]
                },
                {"instance_id":"rgb", "template":{"id":"classic/secondary-capture/rgb"}}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    run(&spec, out.clone(), 61);
    assert!(!out.join("instances/color__source.dcm").exists());
    let rgb = open_file(out.join("instances/rgb.dcm")).unwrap();
    let rgb_uid = rgb
        .element_by_name("SOPInstanceUID")
        .unwrap()
        .to_str()
        .unwrap()
        .trim()
        .to_string();
    let color = open_file(out.join("instances/color.dcm"))
        .unwrap()
        .into_inner();
    assert_eq!(referenced_sop_uids(&color), BTreeSet::from([rgb_uid]));
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn derived_reference_sequence_overrides_publish_nothing() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{
                "instance_id":"color", "template":{"id":"derived/presentation-state/color"},
                "attributes":[{"operation":"remove", "address":{"tag":"0008,1115"}}]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    let error = compose(&ComposeOptions {
        spec_path: spec,
        out_dir: out.clone(),
        seed: 62,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap_err();
    assert!(error.to_string().contains("0008,1115"));
    assert!(!out.exists());
    fs::remove_dir_all(workspace).unwrap();
}
