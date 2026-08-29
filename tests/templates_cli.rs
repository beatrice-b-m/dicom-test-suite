use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_dicom-test-suite")
}

#[test]
fn templates_list_has_human_and_machine_readable_output() {
    let table = Command::new(binary())
        .args(["templates", "list"])
        .output()
        .unwrap();
    assert!(table.status.success());
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.contains("classic/secondary-capture/monochrome\t1.0.0\tQualified"));
    assert!(table.contains("classic/secondary-capture/rgb\t1.0.0\tQualified"));

    let json = Command::new(binary())
        .args(["templates", "list", "--format", "json"])
        .output()
        .unwrap();
    assert!(json.status.success());
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
}

#[test]
fn templates_describe_returns_the_complete_versioned_descriptor() {
    let output = Command::new(binary())
        .args([
            "templates",
            "describe",
            "classic/secondary-capture/rgb",
            "--version",
            "1.0.0",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let descriptor: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(descriptor["status"], "qualified");
    assert_eq!(descriptor["content_slots"][0]["slot"], "pixels");
    assert!(descriptor["standards_evidence"].as_array().unwrap().len() >= 2);
}

#[test]
fn templates_describe_rejects_unknown_or_unqualified_identity() {
    let output = Command::new(binary())
        .args(["templates", "describe", "classic/unknown"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown template"));
}
