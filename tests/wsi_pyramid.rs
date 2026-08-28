use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const CASE_ID: &str = "vl/wsi/pyramid_multiresolution";

#[test]
fn stress_profile_emits_complete_three_instance_wsi_pyramid() {
    let root = unique_temp_dir("wsi-pyramid-stress");
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "stress",
            "--out",
            root.to_str().unwrap(),
            "--seed",
            "7",
        ])
        .output()
        .expect("stress generation must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("profile\tstress"));
    assert!(stdout.contains("include_stress\tfalse"));
    assert!(stdout.contains("files_written\t3"));

    let manifest = read_json(&root.join("manifest.json"));
    assert_schema_valid("schemas/manifest.schema.json", &manifest);
    assert_eq!(manifest.pointer("/run/profile"), Some(&json!("stress")));
    let files = manifest["files"].as_array().expect("manifest files");
    assert_eq!(files.len(), 3);
    let expected = [
        ("volume", 1, "vl/wsi/pyramid_multiresolution/volume.dcm"),
        (
            "thumbnail",
            2,
            "vl/wsi/pyramid_multiresolution/thumbnail.dcm",
        ),
        ("label", 3, "vl/wsi/pyramid_multiresolution/label.dcm"),
    ];
    for (file, (role, ordinal, path)) in files.iter().zip(expected) {
        assert_eq!(file["case_id"], CASE_ID);
        assert_eq!(file["wsi_pyramid_role"], role);
        assert_eq!(file["wsi_pyramid_ordinal"], ordinal);
        assert_eq!(file["path"], path);
        assert!(root.join(path).is_file());
    }
    let skipped = manifest["skipped_cases"].as_array().expect("skipped cases");
    assert_eq!(skipped.len(), 5);
    assert!(
        skipped
            .iter()
            .all(|row| { row["status"] == "unavailable" && row["reason_code"] == "case_planned" })
    );

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report", root.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("coverage report must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_schema_valid("schemas/coverage-report.schema.json", &report);
    let rows = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix")
        .iter()
        .filter(|row| row["case_id"] == CASE_ID)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3, "all same-case rows must survive reporting");
    for (row, (role, ordinal, _)) in rows.iter().zip(expected) {
        assert_eq!(row["wsi_pyramid_role"], role);
        assert_eq!(row["wsi_pyramid_ordinal"], ordinal);
        assert_eq!(row["wsi_pyramid_member_count"], 3);
        assert_eq!(row["wsi_pyramid_group_closure"], true);
        assert_eq!(row["wsi_pyramid_member_binding_verified"], true);
        assert_eq!(row["wsi_pyramid_shared_identity_closure"], true);
        assert_eq!(row["wsi_pyramid_total_frame_count"], 6);
        assert!(
            row["wsi_pyramid_total_dicom_bytes"]
                .as_u64()
                .is_some_and(|n| n <= 65_536)
        );
    }
    assert_eq!(
        report.pointer("/grouped_coverage/wsi_pyramid_roles"),
        Some(&json!({"label": 1, "thumbnail": 1, "volume": 1}))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/wsi_pyramid_group_closure_states/true"),
        Some(&json!(3))
    );
    fs::remove_dir_all(root).unwrap();
}

fn assert_schema_valid(path: &str, value: &Value) {
    let schema = read_json(Path::new(path));
    let validator = jsonschema::validator_for(&schema).expect("schema must compile");
    let errors = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{path} validation failed: {errors:#?}");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
