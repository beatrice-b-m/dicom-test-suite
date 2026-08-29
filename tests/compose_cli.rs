use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_object::open_file;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_dicom-test-suite")
}

fn output(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-compose-cli-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn compose_default_sc_publishes_a_valid_root_and_summary() {
    let out = output("default");
    let result = Command::new(binary())
        .args([
            "compose",
            "--spec",
            "tests/fixtures/composition/valid/template-only.json",
            "--out",
            out.to_str().unwrap(),
            "--seed",
            "9",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("instances_written\t1"));
    assert!(open_file(out.join("instances/primary.dcm")).is_ok());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["run"]["kind"], "composition");
    fs::remove_dir_all(out).unwrap();
}

#[test]
fn compose_dry_run_prints_plans_without_creating_the_output_root() {
    let out = output("dry");
    let result = Command::new(binary())
        .args([
            "compose",
            "--spec",
            "tests/fixtures/composition/valid/template-only.json",
            "--out",
            out.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let resolved: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(resolved["plans"].as_array().unwrap().len(), 1);
    assert!(!out.exists());
}

#[test]
fn compose_rejects_protected_rows_before_promotion() {
    let root = output("protected");
    fs::create_dir(&root).unwrap();
    let spec = root.join("spec.json");
    fs::write(
        &spec,
        r#"{
          "composition_spec_schema_version":"0.1.0",
          "instances":[{
            "instance_id":"primary",
            "template":{"id":"classic/secondary-capture/monochrome"},
            "attributes":[{
              "address":{"tag":"0028,0010"},"operation":"set","vr":"US",
              "value":{"kind":"integer","value":99}
            }]
          }]
        }"#,
    )
    .unwrap();
    let out = root.join("output");
    let result = Command::new(binary())
        .args([
            "compose",
            "--spec",
            spec.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("ProtectedCollision"));
    assert!(!out.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn validate_and_report_dispatch_on_composition_manifests() {
    let out = output("validate-report");
    let composed = Command::new(binary())
        .args([
            "compose",
            "--spec",
            "tests/fixtures/composition/valid/template-only.json",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(composed.status.success());

    let validated = Command::new(binary())
        .args(["validate", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    assert!(String::from_utf8_lossy(&validated.stdout).contains("validation_failures\t0"));

    let report = Command::new(binary())
        .args(["report", out.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(report.status.success());
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["report_kind"], "composition");
    assert_eq!(report["counts"]["instances"], 1);
    assert!(!report.to_string().contains("case_id"));
    assert!(!report.to_string().contains("profile"));

    fs::write(out.join("instances/primary.dcm"), b"tampered").unwrap();
    let rejected = Command::new(binary())
        .args(["validate", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("output SHA-256 differs"));
    fs::remove_dir_all(out).unwrap();
}
