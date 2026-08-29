use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_object::open_file;
use dicom_test_suite::composition::{ComposeOptions, compose};
use dicom_test_suite::sha256_hex;
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-wsi-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn oracle_digest(root: &PathBuf) -> String {
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let entries = manifest["composition"]["entries"].as_array().unwrap().iter().map(|entry| {
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

fn compose_spec(spec_path: impl Into<PathBuf>, out_dir: PathBuf, seed: u64) {
    compose(&ComposeOptions {
        spec_path: spec_path.into(),
        out_dir,
        seed,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
}

#[test]
fn wsi_defaults_and_pyramid_closure_are_byte_reproducible() {
    let first = root("first");
    let second = root("second");
    for out in [&first, &second] {
        compose_spec(
            "tests/fixtures/composition/valid/wsi-defaults.json",
            out.clone(),
            56,
        );
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["composition"]["entries"].as_array().unwrap().len(),
        6
    );
    assert_eq!(
        manifest["composition"]["bundles"][0]["bundle_root_instance_id"],
        "multiple_paths"
    );
    let pyramid = manifest["composition"]["bundles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bundle| bundle["bundle_root_instance_id"] == "pyramid")
        .unwrap();
    assert_eq!(pyramid["members"].as_array().unwrap().len(), 3);
    assert_eq!(pyramid["dependency_closure"].as_array().unwrap().len(), 3);
    assert_eq!(pyramid["references"].as_array().unwrap().len(), 2);
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    for entry in manifest["composition"]["entries"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        assert_eq!(
            fs::read(first.join(path)).unwrap(),
            fs::read(second.join(path)).unwrap()
        );
    }
    assert_eq!(
        oracle_digest(&first),
        "1ab1fa2404fd5a19c76b3ae6cef18e604a630323e3b5fbb2e83e199389cd7be8"
    );
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

fn pixel_source(path: &str, frames: u32) -> Value {
    json!({
        "kind":"local_file", "path":path,
        "pixel": {
            "rows":2, "columns":2, "frames":frames, "samples_per_pixel":3,
            "photometric_interpretation":"RGB", "sample_type":"uint",
            "bits_allocated":8, "bits_stored":8, "high_bit":7,
            "byte_order":"little", "planar_configuration":0
        }
    })
}

#[test]
fn every_wsi_variant_accepts_exact_shape_caller_frames() {
    let workspace = root("caller");
    fs::create_dir(&workspace).unwrap();
    let variants = [
        ("full", "vl/wsi/tiled-full", 4_u32),
        ("sparse", "vl/wsi/tiled-sparse", 2),
        ("paths", "vl/wsi/multiple-optical-paths", 8),
        ("pyramid", "vl/wsi/pyramid-volume", 4),
    ];
    let instances = variants
        .iter()
        .map(|(instance_id, template, frames)| {
            let file = format!("{instance_id}.raw");
            let bytes = (0..(frames * 12))
                .map(|value| (value % 251) as u8)
                .collect::<Vec<_>>();
            fs::write(workspace.join(&file), bytes).unwrap();
            json!({
                "instance_id":instance_id,
                "template":{"id":template},
                "content":[{"slot":"pixels", "source":pixel_source(&file, *frames)}]
            })
        })
        .collect::<Vec<_>>();
    let spec_path = workspace.join("spec.json");
    fs::write(
        &spec_path,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0", "instances":instances
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join("out");
    compose_spec(&spec_path, out.clone(), 57);
    assert_eq!(
        oracle_digest(&out),
        "78ab6a8c851ea7d78aa8113fbdc4a76b4b0dac6bbe9821bf24182a17fa5b1590"
    );
    for (instance_id, _, _) in variants {
        let expected = fs::read(workspace.join(format!("{instance_id}.raw"))).unwrap();
        let object = open_file(out.join(format!("instances/{instance_id}.dcm"))).unwrap();
        assert_eq!(
            object
                .element_by_name("PixelData")
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            expected
        );
    }
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn wsi_tiling_and_frame_reference_inconsistencies_publish_nothing() {
    let workspace = root("invalid");
    fs::create_dir(&workspace).unwrap();
    let protected_spec = workspace.join("protected.json");
    fs::write(
        &protected_spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{
                "instance_id":"sparse", "template":{"id":"vl/wsi/tiled-sparse"},
                "attributes":[{"operation":"remove", "address":{"tag":"0048,0006"}}]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let protected_out = workspace.join("protected-out");
    let error = compose(&ComposeOptions {
        spec_path: protected_spec,
        out_dir: protected_out.clone(),
        seed: 58,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap_err();
    assert!(error.to_string().contains("0048,0006"));
    assert!(!protected_out.exists());

    let reference_spec = workspace.join("reference.json");
    fs::write(
        &reference_spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[
                {
                    "instance_id":"pyramid", "template":{"id":"vl/wsi/pyramid-volume"},
                    "references":[{"role":"pyramid_label", "target_instance_id":"label", "frames":[2]}]
                },
                {"instance_id":"label", "template":{"id":"vl/wsi/pyramid-label"}}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let reference_out = workspace.join("reference-out");
    let error = compose(&ComposeOptions {
        spec_path: reference_spec,
        out_dir: reference_out.clone(),
        seed: 58,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap_err();
    assert!(error.to_string().contains("frame"));
    assert!(!reference_out.exists());
    fs::remove_dir_all(workspace).unwrap();
}
