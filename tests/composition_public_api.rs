use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::composition::{
    ComposeBytesOptions, ComposeOptions, compose, compose_from_bytes,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-api-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn byte_and_file_rust_apis_share_the_exact_pipeline() {
    let spec_path = PathBuf::from("tests/fixtures/composition/valid/template-only.json");
    let spec_bytes = fs::read(&spec_path).unwrap();
    let file_out = root("file");
    let bytes_out = root("bytes");

    let file_result = compose(&ComposeOptions {
        spec_path: spec_path.clone(),
        out_dir: file_out.clone(),
        seed: 81,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    let bytes_result = compose_from_bytes(
        &spec_bytes,
        &ComposeBytesOptions {
            spec_root: spec_path.parent().unwrap().into(),
            out_dir: bytes_out.clone(),
            seed: 81,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        },
    )
    .unwrap();

    assert_eq!(
        file_result.0.instances_written,
        bytes_result.0.instances_written
    );
    assert_eq!(file_result.0.output_bytes, bytes_result.0.output_bytes);
    assert_eq!(file_result.1, bytes_result.1);
    assert_eq!(
        fs::read(file_out.join("instances/primary.dcm")).unwrap(),
        fs::read(bytes_out.join("instances/primary.dcm")).unwrap()
    );
    fs::remove_dir_all(file_out).unwrap();
    fs::remove_dir_all(bytes_out).unwrap();
}

#[test]
fn byte_api_resolves_local_assets_against_the_explicit_root() {
    let workspace = root("assets");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("pixels.raw"), [0_u8, 1, 2, 3]).unwrap();
    let spec = br#"{
      "composition_spec_schema_version":"0.1.0",
      "instances":[{
        "instance_id":"primary",
        "template":{"id":"classic/secondary-capture/monochrome"},
        "content":[{"slot":"pixels","source":{"kind":"local_file","path":"pixels.raw","pixel":{"rows":2,"columns":2,"frames":1,"samples_per_pixel":1,"photometric_interpretation":"MONOCHROME2","sample_type":"uint","bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}}}]
      }]
    }"#;
    let out = workspace.join("out");
    compose_from_bytes(
        spec,
        &ComposeBytesOptions {
            spec_root: workspace.clone(),
            out_dir: out.clone(),
            seed: 82,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        },
    )
    .unwrap();
    assert!(out.join("instances/primary.dcm").is_file());
    fs::remove_dir_all(workspace).unwrap();
}
