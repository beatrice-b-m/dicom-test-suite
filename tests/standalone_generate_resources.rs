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

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
        let result = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
