use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn generate_uses_embedded_resources_from_an_unrelated_working_directory() {
    let base = std::env::temp_dir().join(format!(
        "dicom-test-suite-standalone-generate-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let working = base.join("unrelated/working/directory");
    let output_root = base.join("published/smoke");
    fs::create_dir_all(&working).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .current_dir(&working)
        .args([
            "generate",
            "--profile",
            "smoke",
            "--out",
            output_root.to_str().unwrap(),
            "--seed",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "standalone generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: Value =
        serde_json::from_slice(&fs::read(output_root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["manifest_schema_version"], "0.3.0");
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
    assert_eq!(
        manifest["product_resources"]["resource_set_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/manifest.schema.json").unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&manifest));
    let mut prior_manifest = manifest.clone();
    prior_manifest["manifest_schema_version"] = Value::String("0.2.0".into());
    prior_manifest
        .as_object_mut()
        .unwrap()
        .remove("product_resources");
    assert!(validator.is_valid(&prior_manifest));
    let mut missing_identity = manifest.clone();
    missing_identity
        .as_object_mut()
        .unwrap()
        .remove("product_resources");
    assert!(!validator.is_valid(&missing_identity));
    assert_eq!(manifest["files"].as_array().unwrap().len(), 3);
    for entry in manifest["files"].as_array().unwrap() {
        assert!(output_root.join(entry["path"].as_str().unwrap()).is_file());
    }
    let expected = [
        (
            "classic/sc/mono1_u8_explicit_le",
            926,
            "76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc",
        ),
        (
            "classic/sc/mono2_u8_explicit_le",
            926,
            "fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15",
        ),
        (
            "classic/sc/rgb_planar0_explicit_le",
            938,
            "33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5",
        ),
    ];
    for (case_id, size, sha256) in expected {
        let file = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["case_id"] == case_id)
            .unwrap();
        assert_eq!(file["size_bytes"], size);
        assert_eq!(file["sha256"], sha256);
    }

    for arguments in [
        vec!["validate".to_string(), output_root.display().to_string()],
        vec![
            "report".to_string(),
            output_root.display().to_string(),
            "--format".to_string(),
            "json".to_string(),
        ],
        vec![
            "templates".to_string(),
            "list".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ],
        vec![
            "list-cases".to_string(),
            "--profile".to_string(),
            "smoke".to_string(),
        ],
        vec!["standards".to_string(), "check-lock".to_string()],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .current_dir(&working)
            .args(&arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{} failed outside the repository: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    fs::remove_dir_all(base).unwrap();
}
