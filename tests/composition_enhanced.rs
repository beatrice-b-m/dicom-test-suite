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
        "dts-composition-enhanced-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
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
                "instance_id":entry["instance_id"], "template_id":entry["template_id"],
                "uids":entry["uids"], "resolved_plan_sha256":entry["resolved_plan_sha256"],
                "content":entry["content"], "references":entry["references"],
                "path":path, "sha256":entry["sha256"],
                "payload_sha256":sha256_hex(&fs::read(root.join(path)).unwrap())
            })
        })
        .collect::<Vec<_>>();
    sha256_hex(
        &serde_json::to_vec(&json!({
            "entries":entries, "bundles":manifest["composition"]["bundles"]
        }))
        .unwrap(),
    )
}

#[test]
fn enhanced_defaults_are_valid_and_byte_reproducible() {
    let first = root("first");
    let second = root("second");
    for out in [&first, &second] {
        compose(&ComposeOptions {
            spec_path: "tests/fixtures/composition/valid/enhanced-defaults.json".into(),
            out_dir: out.clone(),
            seed: 54,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
    }
    for instance in ["enhanced_ct", "enhanced_mr", "enhanced_pet"] {
        assert_eq!(
            fs::read(first.join(format!("instances/{instance}.dcm"))).unwrap(),
            fs::read(second.join(format!("instances/{instance}.dcm"))).unwrap()
        );
    }
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    assert_eq!(
        oracle_digest(&first),
        "9c0ba8e9629aa19f7e480054b4f489997350a6022485b99704465820899f813a"
    );
    fs::remove_dir_all(first).unwrap();
    fs::remove_dir_all(second).unwrap();
}

#[test]
fn enhanced_caller_frames_round_trip_and_structural_overrides_fail() {
    let workspace = root("caller");
    fs::create_dir(&workspace).unwrap();
    let raw = (0_u16..8).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    fs::write(workspace.join("frames.raw"), &raw).unwrap();
    let instances = [
        ("enhanced_ct", "enhanced/ct"),
        ("enhanced_mr", "enhanced/mr"),
        ("enhanced_pet", "enhanced/pet"),
    ]
    .map(|(instance_id, template)| {
        json!({
            "instance_id": instance_id,
            "template": {"id": template},
            "content": [{"slot":"pixels", "source": {
                "kind":"local_file", "path":"frames.raw",
                "pixel": {
                    "rows":2, "columns":2, "frames":2, "samples_per_pixel":1,
                    "photometric_interpretation":"MONOCHROME2", "sample_type":"uint",
                    "bits_allocated":16, "bits_stored":16, "high_bit":15,
                    "byte_order":"little"
                }
            }}]
        })
    });
    let spec = json!({
        "composition_spec_schema_version":"0.1.0",
        "instances": instances
    });
    let spec_path = workspace.join("spec.json");
    fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let out = workspace.join("out");
    compose(&ComposeOptions {
        spec_path: spec_path.clone(),
        out_dir: out.clone(),
        seed: 55,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    assert_eq!(
        oracle_digest(&out),
        "342be50da9a2e2905a63c45f8bcfccd0c6e69186588709f9d35247d3fc926e77"
    );
    for instance in ["enhanced_ct", "enhanced_mr", "enhanced_pet"] {
        let object = open_file(out.join(format!("instances/{instance}.dcm"))).unwrap();
        assert_eq!(
            object
                .element_by_name("PixelData")
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            raw.as_slice()
        );
    }

    let mut invalid = spec;
    invalid["instances"][0]["attributes"] = json!([{
        "operation":"remove", "address":{"tag":"5200,9230"}
    }]);
    let invalid_path = workspace.join("invalid.json");
    fs::write(&invalid_path, serde_json::to_vec_pretty(&invalid).unwrap()).unwrap();
    let invalid_out = workspace.join("invalid-out");
    let error = compose(&ComposeOptions {
        spec_path: invalid_path,
        out_dir: invalid_out.clone(),
        seed: 55,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap_err();
    assert!(error.to_string().contains("5200,9230"));
    assert!(!invalid_out.exists());
    fs::remove_dir_all(workspace).unwrap();
}
