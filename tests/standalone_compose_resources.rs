use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn compose_uses_embedded_catalogs_from_an_unrelated_working_directory() {
    let base = std::env::temp_dir().join(format!(
        "dicom-test-suite-standalone-compose-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let working = base.join("unrelated/working/directory");
    let request_root = base.join("caller");
    let output_root = base.join("published/composition");
    fs::create_dir_all(&working).unwrap();
    fs::create_dir_all(&request_root).unwrap();
    let spec_path = request_root.join("request.json");
    fs::write(
        &spec_path,
        fs::read("tests/fixtures/composition/valid/template-only.json").unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .current_dir(&working)
        .args([
            "compose",
            "--spec",
            spec_path.to_str().unwrap(),
            "--out",
            output_root.to_str().unwrap(),
            "--seed",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "standalone compose failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: Value =
        serde_json::from_slice(&fs::read(output_root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["manifest_schema_version"], "1.0.0");
    assert_eq!(manifest["product_resources"]["origin"], "embedded");
    assert_eq!(
        manifest["product_resources"]["resource_set_version"],
        "1.0.0"
    );
    assert_eq!(
        manifest["product_resources"]["resource_count"],
        manifest["product_resources"]["resources"]
            .as_array()
            .unwrap()
            .len()
    );
    let current_schema: Value =
        serde_json::from_slice(&fs::read("schemas/composition-manifest-v1.schema.json").unwrap())
            .unwrap();
    let identity_schema: Value =
        serde_json::from_slice(&fs::read("schemas/version-result-v2.schema.json").unwrap())
            .unwrap();
    let current_validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_resource(
            "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json",
            jsonschema::Resource::from_contents(identity_schema).unwrap(),
        )
        .build(&current_schema)
        .unwrap();
    assert!(current_validator.is_valid(&manifest));
    let legacy_schema: Value =
        serde_json::from_slice(&fs::read("schemas/composition-manifest.schema.json").unwrap())
            .unwrap();
    let legacy_validator = jsonschema::validator_for(&legacy_schema).unwrap();
    let mut prior_manifest = manifest.clone();
    prior_manifest["manifest_schema_version"] = Value::String("0.5.0".into());
    prior_manifest
        .as_object_mut()
        .unwrap()
        .remove("identity_projection");
    assert!(legacy_validator.is_valid(&prior_manifest));
    prior_manifest["manifest_schema_version"] = Value::String("0.4.0".into());
    prior_manifest
        .as_object_mut()
        .unwrap()
        .remove("product_resources");
    assert!(legacy_validator.is_valid(&prior_manifest));
    let mut missing_identity = manifest.clone();
    missing_identity
        .as_object_mut()
        .unwrap()
        .remove("product_resources");
    assert!(!current_validator.is_valid(&missing_identity));
    assert_eq!(
        manifest.pointer("/run/kind").and_then(Value::as_str),
        Some("composition")
    );
    let entries = manifest
        .pointer("/composition/entries")
        .and_then(Value::as_array)
        .unwrap();
    assert!(!entries.is_empty());
    for entry in entries {
        assert!(output_root.join(entry["path"].as_str().unwrap()).is_file());
    }

    fs::remove_dir_all(base).unwrap();
}
