use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::Tag;
use dicom_core::header::Header;
use dicom_dictionary_std::tags;
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};
use synth_dicom_gen::composition::{ComposeOptions, compose};
use synth_dicom_gen::sha256_hex;

#[path = "support/prepared_backend.rs"]
mod prepared_backend;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-sr-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn run(spec: impl Into<PathBuf>, out: PathBuf, seed: u64) -> Result<(), PathBuf> {
    let _backend = prepared_backend::PreparedBackendOverride::try_acquire()?;
    compose(&ComposeOptions {
        spec_path: spec.into(),
        out_dir: out,
        seed,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    Ok(())
}

fn assert_explicit_backend_unavailable(path: &PathBuf) {
    assert!(
        !path.is_file(),
        "unavailable backend path became executable"
    );
}

fn oracle_digest(root: &PathBuf) -> String {
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let entries = manifest["composition"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let path = entry["path"].as_str().unwrap();
            json!({
                "instance_id": entry["instance_id"],
                "template_id": entry["template_id"],
                "uids": entry["uids"],
                "resolved_plan_sha256": entry["resolved_plan_sha256"],
                "content": entry["content"],
                "references": entry["references"],
                "path": path,
                "sha256": entry["sha256"],
                "payload_sha256": sha256_hex(&fs::read(root.join(path)).unwrap()),
            })
        })
        .collect::<Vec<_>>();
    sha256_hex(
        &serde_json::to_vec(&json!({
            "entries": entries,
            "bundles": manifest["composition"]["bundles"],
        }))
        .unwrap(),
    )
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

fn first_f32_values(object: &InMemDicomObject, tag: Tag) -> Option<Vec<f32>> {
    for element in object.iter() {
        if element.tag() == tag {
            return element.to_multi_float32().ok();
        }
        if let Some(items) = element.items() {
            for item in items {
                if let Some(values) = first_f32_values(item, tag) {
                    return Some(values);
                }
            }
        }
    }
    None
}

#[test]
fn structured_report_defaults_have_closed_reproducible_reference_graphs() {
    let first = root("defaults-a");
    let second = root("defaults-b");
    for out in [&first, &second] {
        if let Err(path) = run(
            "tests/fixtures/composition/valid/p6-structured-report-defaults.json",
            out.clone(),
            72,
        ) {
            assert_explicit_backend_unavailable(&path);
            return;
        }
    }
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
    for instance_id in ["basic", "comprehensive", "scoord3d", "tid1500", "kos"] {
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
        assert_eq!(
            fs::read(first.join(entry["path"].as_str().unwrap())).unwrap(),
            fs::read(second.join(entry["path"].as_str().unwrap())).unwrap()
        );
    }
    assert_eq!(
        oracle_digest(&first),
        "643a8a3ff5ad797c680332c4d6c8426aac395049ad3a19cfc299c7e07db76568"
    );
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn typed_sr_parameters_change_only_known_content_item_values() {
    let workspace = root("parameters");
    fs::create_dir(&workspace).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[
                {"instance_id":"basic", "template":{"id":"derived/structured-report/basic-text"}, "parameters":{"observation_text":"Caller synthetic observation"}},
                {"instance_id":"comprehensive", "template":{"id":"derived/structured-report/comprehensive"}, "parameters":{"measurement_value_mm":42.25}},
                {"instance_id":"scoord3d", "template":{"id":"derived/structured-report/comprehensive-3d"}, "parameters":{"graphic_data_patient_mm":[0,0,0,0,0,5], "measurement_value_mm":5}},
                {"instance_id":"tid1500", "template":{"id":"derived/structured-report/tid1500"}, "parameters":{"measurement_value_mm3":125}}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    if let Err(path) = run(spec, out.clone(), 73) {
        assert_explicit_backend_unavailable(&path);
        fs::remove_dir_all(workspace).unwrap();
        return;
    }
    let basic = open_file(out.join("instances/basic.dcm"))
        .unwrap()
        .into_inner();
    assert!(
        strings_for(&basic, Tag(0x0040, 0xA160)).contains(&"Caller synthetic observation".into())
    );
    let comprehensive = open_file(out.join("instances/comprehensive.dcm"))
        .unwrap()
        .into_inner();
    assert!(strings_for(&comprehensive, Tag(0x0040, 0xA30A)).contains(&"42.25".into()));
    let scoord3d = open_file(out.join("instances/scoord3d.dcm"))
        .unwrap()
        .into_inner();
    assert!(strings_for(&scoord3d, Tag(0x0040, 0xA30A)).contains(&"5".into()));
    assert_eq!(
        first_f32_values(&scoord3d, Tag(0x0070, 0x0022)).unwrap(),
        vec![0.0, 0.0, 0.0, 0.0, 0.0, 5.0]
    );
    let tid1500 = open_file(out.join("instances/tid1500.dcm"))
        .unwrap()
        .into_inner();
    assert!(strings_for(&tid1500, Tag(0x0040, 0xA30A)).contains(&"125".into()));
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn arbitrary_sr_trees_and_out_of_range_values_are_schema_rejected() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    for (label, template, parameters) in [
        (
            "tree",
            "derived/structured-report/basic-text",
            json!({"content_tree":[]}),
        ),
        (
            "negative",
            "derived/structured-report/tid1500",
            json!({"measurement_value_mm3":-1}),
        ),
    ] {
        let spec = workspace.join(format!("{label}.json"));
        fs::write(&spec, serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{"instance_id":label, "template":{"id":template}, "parameters":parameters}]
        })).unwrap()).unwrap();
        let out = workspace.join(format!("{label}-out"));
        assert!(
            compose(&ComposeOptions {
                spec_path: spec,
                out_dir: out.clone(),
                seed: 74,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .is_err()
        );
        assert!(!out.exists());
    }
    fs::remove_dir_all(workspace).unwrap();
}
