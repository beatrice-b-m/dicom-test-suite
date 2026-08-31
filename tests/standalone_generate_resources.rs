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
    assert_eq!(manifest["files"].as_array().unwrap().len(), 3);
    for entry in manifest["files"].as_array().unwrap() {
        assert!(output_root.join(entry["path"].as_str().unwrap()).is_file());
    }

    fs::remove_dir_all(base).unwrap();
}
