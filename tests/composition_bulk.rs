use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::Tag;
use dicom_object::open_file;
use dicom_test_suite::composition::{ComposeOptions, compose};
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-bulk-{label}-{}-{}",
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

fn waveform_payloads(path: PathBuf) -> Vec<Vec<u8>> {
    let object = open_file(path).unwrap();
    object
        .element(Tag(0x5400, 0x0100))
        .unwrap()
        .items()
        .unwrap()
        .iter()
        .map(|item| {
            item.element(Tag(0x5400, 0x1010))
                .unwrap()
                .to_bytes()
                .unwrap()
                .into_owned()
        })
        .collect()
}

fn document_payload(path: PathBuf) -> Vec<u8> {
    let object = open_file(path).unwrap();
    let length = object
        .element(Tag(0x0042, 0x0015))
        .unwrap()
        .to_int::<u32>()
        .unwrap() as usize;
    object
        .element(Tag(0x0042, 0x0011))
        .unwrap()
        .to_bytes()
        .unwrap()[..length]
        .to_vec()
}

#[test]
fn p6_waveform_document_and_mesh_defaults_are_reproducible_and_provenanced() {
    let first = root("defaults-a");
    let second = root("defaults-b");
    for out in [&first, &second] {
        run(
            "tests/fixtures/composition/valid/p6-bulk-defaults.json",
            out.clone(),
            66,
        );
    }
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
    for entry in manifest["composition"]["entries"].as_array().unwrap() {
        assert!(!entry["content"].as_array().unwrap().is_empty());
        for content in entry["content"].as_array().unwrap() {
            assert!(content["size_bytes"].as_u64().unwrap() > 0);
            assert_eq!(content["sha256"].as_str().unwrap().len(), 64);
            assert!(content["properties"]["bulk_source"].is_string());
            assert!(content["properties"]["semantic_validator"].is_string());
        }
        let relative = entry["path"].as_str().unwrap();
        assert_eq!(
            fs::read(first.join(relative)).unwrap(),
            fs::read(second.join(relative)).unwrap()
        );
    }
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn caller_waveforms_pdf_and_stl_round_trip_through_typed_slots() {
    let workspace = root("caller");
    fs::create_dir(&workspace).unwrap();
    let defaults = workspace.join("defaults");
    run(
        "tests/fixtures/composition/valid/p6-bulk-defaults.json",
        defaults.clone(),
        67,
    );
    let twelve = waveform_payloads(defaults.join("instances/twelve.dcm"));
    let general = waveform_payloads(defaults.join("instances/general.dcm"));
    let pdf = document_payload(defaults.join("instances/pdf.dcm"));
    let stl = document_payload(defaults.join("instances/stl.dcm"));
    fs::write(workspace.join("twelve.raw"), &twelve[0]).unwrap();
    fs::write(workspace.join("general-1.raw"), &general[0]).unwrap();
    fs::write(workspace.join("general-2.raw"), &general[1]).unwrap();
    fs::write(workspace.join("document.pdf"), &pdf).unwrap();
    fs::write(workspace.join("model.stl"), &stl).unwrap();
    let spec = json!({
        "composition_spec_schema_version":"0.1.0",
        "instances":[
            {"instance_id":"twelve", "template":{"id":"non-image/waveform/twelve-lead-ecg"}, "content":[{"slot":"waveform_samples", "source":{"kind":"local_file", "path":"twelve.raw", "media_type":"application/octet-stream"}}]},
            {"instance_id":"general", "template":{"id":"non-image/waveform/general-ecg"}, "content":[
                {"slot":"waveform_samples_1", "source":{"kind":"local_file", "path":"general-1.raw"}},
                {"slot":"waveform_samples_2", "source":{"kind":"local_file", "path":"general-2.raw"}}
            ]},
            {"instance_id":"pdf", "template":{"id":"non-image/encapsulated-document/pdf"}, "content":[{"slot":"document", "source":{"kind":"local_file", "path":"document.pdf", "media_type":"application/pdf"}}]},
            {"instance_id":"stl", "template":{"id":"non-image/mesh/stl"}, "content":[{"slot":"mesh", "source":{"kind":"local_file", "path":"model.stl", "media_type":"model/stl"}}]}
        ]
    });
    let spec_path = workspace.join("spec.json");
    fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let out = workspace.join("out");
    run(spec_path, out.clone(), 67);
    assert_eq!(waveform_payloads(out.join("instances/twelve.dcm")), twelve);
    assert_eq!(
        waveform_payloads(out.join("instances/general.dcm")),
        general
    );
    assert_eq!(document_payload(out.join("instances/pdf.dcm")), pdf);
    assert_eq!(document_payload(out.join("instances/stl.dcm")), stl);
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn malformed_or_wrong_size_bulk_payloads_fail_before_publication() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("bad.bin"), b"not a valid payload").unwrap();
    for (label, template, slot) in [
        (
            "waveform",
            "non-image/waveform/twelve-lead-ecg",
            "waveform_samples",
        ),
        ("pdf", "non-image/encapsulated-document/pdf", "document"),
        ("stl", "non-image/mesh/stl", "mesh"),
    ] {
        let spec = workspace.join(format!("{label}.json"));
        fs::write(&spec, serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{"instance_id":label, "template":{"id":template}, "content":[{"slot":slot, "source":{"kind":"local_file", "path":"bad.bin"}}]}]
        })).unwrap()).unwrap();
        let out = workspace.join(format!("{label}-out"));
        assert!(
            compose(&ComposeOptions {
                spec_path: spec,
                out_dir: out.clone(),
                seed: 68,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .is_err()
        );
        assert!(!out.exists());
    }
    fs::remove_dir_all(workspace).unwrap();
}
