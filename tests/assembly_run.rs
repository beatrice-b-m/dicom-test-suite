use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_object::open_file;
use dicom_test_suite::assembly::{AssembleOptions, assemble};
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::product_resources::ProductResources;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-assembly-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn request() -> Vec<u8> {
    br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[{
        "instance_id":"primary",
        "sop_class_uid":"1.2.840.10008.5.1.4.1.1.7",
        "modality":"OT",
        "elements":[
          {"address":{"keyword":"PatientName"},"value":{"kind":"string","value":"SYNTHETIC^ASSEMBLY"}},
          {"address":{"private_group":"0011","private_creator":"DTS_ASSEMBLY","private_offset":"10"},"vr":"LO","value":{"kind":"string","value":"STRUCTURAL"}}
        ],
        "bulk":[{
          "kind":"integer_pixel_data","source":{"kind":"inline_base64","base64":"AAECAw=="},
          "rows":2,"columns":2,"frames":1,"samples_per_pixel":1,"bits_allocated":8,"bits_stored":8,"signed":false,
          "photometric_interpretation":"MONOCHROME2"
        }]
      }]
    }"#
        .to_vec()
}

#[test]
fn structural_assembly_executes_through_shared_writer_and_manifest() {
    let root = output("published");
    let summary = assemble(
        &AssembleOptions {
            request_bytes: request(),
            caller_asset_root: PathBuf::from("."),
            output_root: root.clone(),
            seed: 5,
            parallelism: 2,
            dry_run: false,
        },
        &CancellationToken::new(),
        &ProductResources::embedded(),
    )
    .unwrap();
    assert!(summary.published);
    assert_eq!(summary.artifacts_written, 1);
    assert!(summary.output_bytes > 0);
    assert!(open_file(root.join("instances/primary.dcm")).is_ok());

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let schema: serde_json::Value = serde_json::from_slice(
        &fs::read("schemas/structural-assembly-manifest.schema.json").unwrap(),
    )
    .unwrap();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&manifest));
    assert_eq!(manifest["run"]["kind"], "structural_assembly");
    assert_eq!(manifest["run"]["iod_conformance"], "not_assessed");
    assert_eq!(manifest["instances"][0]["iod_conformance"], "not_assessed");
    for forbidden in ["case_id", "profile", "template_id", "qualification_status"] {
        assert!(
            !contains_key(&manifest, forbidden),
            "forbidden claim {forbidden}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn structural_dry_run_has_same_plan_hash_and_creates_nothing() {
    let published_root = output("hash-published");
    let dry_root = output("hash-dry");
    let options = |root, dry_run| AssembleOptions {
        request_bytes: request(),
        caller_asset_root: PathBuf::from("."),
        output_root: root,
        seed: 5,
        parallelism: 1,
        dry_run,
    };
    let published = assemble(
        &options(published_root.clone(), false),
        &CancellationToken::new(),
        &ProductResources::embedded(),
    )
    .unwrap();
    let dry = assemble(
        &options(dry_root.clone(), true),
        &CancellationToken::new(),
        &ProductResources::embedded(),
    )
    .unwrap();
    assert_eq!(published.corpus_plan_sha256, dry.corpus_plan_sha256);
    assert!(!dry.published);
    assert!(dry.manifest_path.is_none());
    assert!(!dry_root.exists());
    fs::remove_dir_all(published_root).unwrap();
}

#[test]
fn structural_cancellation_publishes_nothing() {
    let root = output("cancelled");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = assemble(
        &AssembleOptions {
            request_bytes: request(),
            caller_asset_root: PathBuf::from("."),
            output_root: root.clone(),
            seed: 5,
            parallelism: 1,
            dry_run: false,
        },
        &cancellation,
        &ProductResources::embedded(),
    )
    .unwrap_err();
    assert!(
        error.to_string().to_ascii_lowercase().contains("cancel"),
        "{error}"
    );
    assert!(!root.exists());
}

fn contains_key(value: &serde_json::Value, key: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.contains_key(key) || object.values().any(|value| contains_key(value, key))
        }
        serde_json::Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        _ => false,
    }
}
