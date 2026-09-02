use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::Tag;
use dicom_object::open_file;
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_synth-dicom-gen")
}

fn root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "dts-composition-p2-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}

fn run_compose(spec: &Path, out: &Path, seed: u64) -> std::process::Output {
    Command::new(binary())
        .args([
            "compose",
            "--spec",
            spec.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--seed",
            &seed.to_string(),
        ])
        .output()
        .unwrap()
}

fn local_pixel_spec(template: &str, bytes: &[u8], pixel: Value) -> Value {
    json!({
        "composition_spec_schema_version": "0.1.0",
        "instances": [{
            "instance_id": "primary",
            "template": { "id": template },
            "content": [{
                "slot": "pixels",
                "source": {
                    "kind": "local_file",
                    "path": "pixels.raw",
                    "sha256": synth_dicom_gen::sha256_hex(bytes),
                    "pixel": pixel
                }
            }]
        }]
    })
}

#[test]
fn raw_monochrome_and_rgb_pixels_round_trip_exactly() {
    for (label, template, bytes, pixel) in [
        (
            "mono",
            "classic/secondary-capture/monochrome",
            vec![0_u8, 63, 127, 255],
            json!({
                "rows": 2, "columns": 2, "frames": 1,
                "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                "sample_type": "uint", "bits_allocated": 8, "bits_stored": 8,
                "high_bit": 7, "byte_order": "little"
            }),
        ),
        (
            "rgb",
            "classic/secondary-capture/rgb",
            vec![255_u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
            json!({
                "rows": 2, "columns": 2, "frames": 1,
                "samples_per_pixel": 3, "photometric_interpretation": "RGB",
                "sample_type": "uint", "bits_allocated": 8, "bits_stored": 8,
                "high_bit": 7, "byte_order": "little", "planar_configuration": 0
            }),
        ),
    ] {
        let root = root(label);
        fs::write(root.join("pixels.raw"), &bytes).unwrap();
        let spec = root.join("spec.json");
        fs::write(
            &spec,
            serde_json::to_vec_pretty(&local_pixel_spec(template, &bytes, pixel)).unwrap(),
        )
        .unwrap();
        let out = root.join("out");
        let result = run_compose(&spec, &out, 17);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let object = open_file(out.join("instances/primary.dcm")).unwrap();
        assert_eq!(
            object
                .element(Tag(0x0008, 0x001C))
                .unwrap()
                .to_str()
                .unwrap(),
            "YES"
        );
        assert_eq!(
            object
                .element(Tag(0x7FE0, 0x0010))
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            bytes
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
        let properties = &manifest["composition"]["entries"][0]["content"][0]["properties"];
        assert_eq!(properties["spec_relative_path"], "pixels.raw");
        assert!(
            properties["frame_sha256"]
                .as_str()
                .unwrap()
                .contains(&synth_dicom_gen::sha256_hex(&bytes))
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn identical_runs_are_byte_and_manifest_stable() {
    let root = root("reproducibility");
    let first = root.join("first");
    let second = root.join("second");
    let spec = Path::new("tests/fixtures/composition/valid/template-only.json");
    assert!(run_compose(spec, &first, 23).status.success());
    assert!(run_compose(spec, &second, 23).status.success());
    assert_eq!(
        fs::read(first.join("instances/primary.dcm")).unwrap(),
        fs::read(second.join("instances/primary.dcm")).unwrap()
    );
    assert_eq!(
        fs::read(first.join("manifest.json")).unwrap(),
        fs::read(second.join("manifest.json")).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn every_p2_identity_shape_syntax_and_content_contradiction_blocks_promotion() {
    let root = root("contradictions");
    let operations = [
        (
            "rows",
            "0028,0010",
            "US",
            json!({"kind":"integer","value":99}),
        ),
        (
            "sop-class",
            "0008,0016",
            "UI",
            json!({"kind":"string","value":"1.2.3"}),
        ),
        (
            "study-uid",
            "0020,000D",
            "UI",
            json!({"kind":"string","value":"2.25.99"}),
        ),
        (
            "pixel-data",
            "7FE0,0010",
            "OB",
            json!({"kind":"binary","base64":"AAEC"}),
        ),
    ];
    for (label, tag, vr, value) in operations {
        let spec = root.join(format!("{label}.json"));
        fs::write(
            &spec,
            serde_json::to_vec_pretty(&json!({
                "composition_spec_schema_version":"0.1.0",
                "instances":[{
                    "instance_id":"primary",
                    "template":{"id":"classic/secondary-capture/monochrome"},
                    "attributes":[{"address":{"tag":tag},"operation":"set","vr":vr,"value":value}]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let out = root.join(format!("{label}-out"));
        let result = run_compose(&spec, &out, 1);
        assert!(!result.status.success(), "{label} unexpectedly succeeded");
        assert!(!out.exists(), "{label} output was promoted");
    }

    let syntax_spec = root.join("syntax.json");
    fs::write(
        &syntax_spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{
                "instance_id":"primary",
                "template":{"id":"classic/secondary-capture/monochrome"},
                "transfer_syntax_uid":"1.2.840.10008.1.2"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let syntax_out = root.join("syntax-out");
    assert!(!run_compose(&syntax_spec, &syntax_out, 1).status.success());
    assert!(!syntax_out.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsafe_local_content_path_fails_before_any_output_root_exists() {
    let root = root("unsafe-path");
    let spec = root.join("unsafe.json");
    fs::write(
        &spec,
        r#"{
          "composition_spec_schema_version":"0.1.0",
          "instances":[{
            "instance_id":"primary",
            "template":{"id":"classic/secondary-capture/monochrome"},
            "content":[{"slot":"pixels","source":{"kind":"local_file","path":"../pixels.raw"}}]
          }]
        }"#,
    )
    .unwrap();
    let out = root.join("out");
    let result = run_compose(&spec, &out, 1);
    assert!(!result.status.success());
    assert!(!out.exists());
    fs::remove_dir_all(root).unwrap();
}
