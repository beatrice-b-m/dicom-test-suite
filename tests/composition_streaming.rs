use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use synth_dicom_gen::composition::{ComposeOptions, compose};
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-streaming-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn large_native_pixels_use_the_hash_checked_streaming_writer() {
    let workspace = root("pixels");
    fs::create_dir(&workspace).unwrap();
    let pixels = vec![0x5a_u8; 4096 * 4096];
    fs::write(workspace.join("pixels.raw"), &pixels).unwrap();
    let spec = workspace.join("spec.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits": {
                "max_instances": 2, "max_input_files": 2,
                "max_file_bytes": 33554432, "max_total_input_bytes": 33554432,
                "max_total_output_bytes": 67108864
            },
            "instances":[{
                "instance_id":"large",
                "template":{"id":"classic/secondary-capture/monochrome"},
                "content":[{"slot":"pixels","source":{
                    "kind":"local_file", "path":"pixels.raw",
                    "pixel":{"rows":4096,"columns":4096,"frames":1,"samples_per_pixel":1,
                        "photometric_interpretation":"MONOCHROME2","sample_type":"uint",
                        "bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}
                }}]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    let (_, manifest) = compose(&ComposeOptions {
        spec_path: spec,
        out_dir: out.clone(),
        seed: 83,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();

    let content = &manifest["composition"]["entries"][0]["content"][0];
    assert_eq!(
        content["properties"]["writer_materialization"],
        "stream_copy"
    );
    assert_eq!(content["size_bytes"], pixels.len());
    assert_eq!(content["sha256"], synth_dicom_gen::sha256_hex(&pixels));
    let object = open_file(out.join("instances/large.dcm")).unwrap();
    assert_eq!(
        object
            .element(tags::PIXEL_DATA)
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        pixels
    );
    assert!(!fs::read_dir(out.join("instances")).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|value| value == "dts-streaming")
    }));
    let manifest_value: Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest_value, manifest);
    fs::remove_dir_all(workspace).unwrap();
}
