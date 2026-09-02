use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use synth_dicom_gen::composition::{ComposeOptions, compose};
use synth_dicom_gen::sha256_hex;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-codec-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn caller_native_rle_is_byte_stable_and_semantically_hash_qualified() {
    let workspace = root("caller-rle");
    fs::create_dir(&workspace).unwrap();
    let native = (0_u16..256)
        .flat_map(|sample| (sample & 0x0fff).to_le_bytes())
        .collect::<Vec<_>>();
    fs::write(workspace.join("pixels.raw"), &native).unwrap();
    let spec = json!({
        "composition_spec_schema_version": "0.1.0",
        "instances": [{
            "instance_id": "xa_rle",
            "template": {"id": "classic/xa"},
            "transfer_syntax_uid": "1.2.840.10008.1.2.5",
            "content": [{
                "slot": "pixels",
                "source": {
                    "kind": "local_file",
                    "path": "pixels.raw",
                    "sha256": sha256_hex(&native),
                    "pixel": {
                        "rows": 16,
                        "columns": 16,
                        "frames": 1,
                        "samples_per_pixel": 1,
                        "photometric_interpretation": "MONOCHROME2",
                        "sample_type": "uint",
                        "bits_allocated": 16,
                        "bits_stored": 12,
                        "high_bit": 11,
                        "byte_order": "little"
                    }
                }
            }]
        }]
    });
    let spec_path = workspace.join("spec.json");
    fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let first = workspace.join("first");
    let second = workspace.join("second");
    let (_, first_manifest) = compose(&ComposeOptions {
        spec_path: spec_path.clone(),
        out_dir: first.clone(),
        seed: 75,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    let (_, second_manifest) = compose(&ComposeOptions {
        spec_path,
        out_dir: second.clone(),
        seed: 75,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    assert_eq!(
        fs::read(first.join("instances/xa_rle.dcm")).unwrap(),
        fs::read(second.join("instances/xa_rle.dcm")).unwrap()
    );
    assert_eq!(first_manifest, second_manifest);

    let content = &first_manifest["composition"]["entries"][0]["content"][0];
    let properties = &content["properties"];
    assert_eq!(content["kind"], "encapsulated_pixels");
    assert_eq!(properties["native_sha256"], sha256_hex(&native));
    assert_eq!(
        properties["decoded_frame_sha256"],
        serde_json::to_string(&vec![sha256_hex(&native)]).unwrap()
    );
    assert_eq!(
        properties["codec_semantic_validation"],
        "decoded_frame_hashes_match"
    );
    assert_eq!(properties["codec_backend_kind"], "native");
    assert_eq!(properties["codec_feature_gate"], "none");
    assert_eq!(properties["codec_availability"], "available");
    assert_eq!(properties["codec_determinism"], "byte_stable");
    fs::remove_dir_all(workspace).unwrap();
}
