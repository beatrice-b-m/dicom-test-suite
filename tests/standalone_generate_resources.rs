use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn compile_manifest_v1() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/manifest-v1.schema.json").unwrap()).unwrap();
    let legacy: Value =
        serde_json::from_slice(&fs::read("schemas/manifest.schema.json").unwrap()).unwrap();
    let identities: Value =
        serde_json::from_slice(&fs::read("schemas/version-result-v2.schema.json").unwrap())
            .unwrap();
    jsonschema::options()
        .with_resource(
            "https://dicom-test-suite.local/schemas/manifest.schema.json",
            jsonschema::Resource::from_contents(legacy).unwrap(),
        )
        .with_resource(
            "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json",
            jsonschema::Resource::from_contents(identities).unwrap(),
        )
        .build(&schema)
        .unwrap()
}

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
    assert_eq!(
        manifest["product_resources"]["resource_set_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(compile_manifest_v1().is_valid(&manifest));
    let legacy_schema: Value =
        serde_json::from_slice(&fs::read("schemas/manifest.schema.json").unwrap()).unwrap();
    let legacy_validator = jsonschema::validator_for(&legacy_schema).unwrap();
    for fixture in [
        "tests/fixtures/cli/curated-manifest-v0.2.json",
        "tests/fixtures/cli/curated-manifest-v0.3.json",
    ] {
        let prior: Value = serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
        assert!(
            legacy_validator.is_valid(&prior),
            "{fixture} must remain readable"
        );
    }
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

    for version in ["0.2.0", "0.3.0"] {
        let mut prior = manifest.clone();
        prior["manifest_schema_version"] = Value::String(version.into());
        prior.as_object_mut().unwrap().remove("identity_projection");
        if version == "0.2.0" {
            prior.as_object_mut().unwrap().remove("product_resources");
        }
        fs::write(
            output_root.join("manifest.json"),
            serde_json::to_vec_pretty(&prior).unwrap(),
        )
        .unwrap();
        for arguments in [
            vec!["validate".to_string(), output_root.display().to_string()],
            vec![
                "report".to_string(),
                output_root.display().to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
        ] {
            let result = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
                .current_dir(&working)
                .args(&arguments)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "legacy manifest {version} {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }

    fs::remove_dir_all(base).unwrap();
}
