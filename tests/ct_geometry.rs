use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "geometry/ct/spatial_sort_conflicts_instance_number";

#[test]
fn core_generates_ct_series_with_conflicting_instance_number_order() {
    let out_dir = unique_temp_dir("ct-spatial-sort-conflict");
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(&out_dir)
        .args(["--seed", "17"])
        .output()
        .expect("generate command must run");
    assert!(
        output.status.success(),
        "generate should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: Value = serde_json::from_slice(
        &fs::read(out_dir.join("manifest.json")).expect("manifest must be readable"),
    )
    .expect("manifest must contain JSON");
    let files = manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .filter(|file| file["case_id"].as_str() == Some(CASE_ID))
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 3);

    let expected_paths = [
        format!("{CASE_ID}/slice-001.dcm"),
        format!("{CASE_ID}/slice-002.dcm"),
        format!("{CASE_ID}/slice-003.dcm"),
    ];
    let expected_instance_numbers = [30_i64, 10, 20];
    let expected_instance_order_indices = [3_u64, 1, 2];
    let expected_positions = [0.0_f64, 5.0, 10.0];
    let mut sop_instance_uids = BTreeSet::new();

    for (index, file) in files.iter().enumerate() {
        assert_eq!(file["path"].as_str(), Some(expected_paths[index].as_str()));
        assert_eq!(
            file.pointer("/expected_geometry/sort_basis")
                .and_then(Value::as_str),
            Some("image_position_patient_projected_on_slice_normal")
        );
        assert_eq!(
            file.pointer("/expected_geometry/sort_direction")
                .and_then(Value::as_str),
            Some("ascending")
        );
        assert_eq!(
            file.pointer("/expected_geometry/geometric_order_index")
                .and_then(Value::as_u64),
            Some(index as u64 + 1)
        );
        assert_eq!(
            file.pointer("/expected_geometry/position_along_normal_mm")
                .and_then(Value::as_f64),
            Some(expected_positions[index])
        );
        assert_eq!(
            file.pointer("/expected_geometry/instance_number")
                .and_then(Value::as_i64),
            Some(expected_instance_numbers[index])
        );
        assert_eq!(
            file.pointer("/expected_geometry/instance_number_order_index")
                .and_then(Value::as_u64),
            Some(expected_instance_order_indices[index])
        );
        assert_eq!(
            file.pointer("/expected_geometry/series_instance_count")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            file.pointer("/expected_geometry/sorting_conflict_expected")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            file.pointer("/expected_semantics/geometry_sort_key/position_along_normal")
                .and_then(Value::as_f64),
            Some(expected_positions[index])
        );
        sop_instance_uids.insert(
            file.pointer("/uids/sop_instance_uid")
                .and_then(Value::as_str)
                .expect("SOP Instance UID must be recorded"),
        );
    }
    assert_eq!(sop_instance_uids.len(), 3);
    for uid_path in [
        "/uids/study_instance_uid",
        "/uids/series_instance_uid",
        "/uids/frame_of_reference_uid",
    ] {
        assert!(
            files
                .iter()
                .all(|file| file.pointer(uid_path) == files[0].pointer(uid_path))
        );
    }

    for (index, relative_path) in expected_paths.iter().enumerate() {
        let obj = open_file(out_dir.join(relative_path)).expect("CT slice must parse");
        assert_eq!(
            obj.element(tags::INSTANCE_NUMBER)
                .expect("Instance Number must be present")
                .to_int::<i64>()
                .expect("Instance Number must be numeric"),
            expected_instance_numbers[index]
        );
        assert_eq!(
            obj.element(tags::LATERALITY)
                .expect("Laterality Type 2C attribute must be present")
                .to_str()
                .expect("Laterality must be text")
                .trim(),
            ""
        );
        assert_eq!(
            obj.element(tags::PATIENT_POSITION)
                .expect("Patient Position Type 2C attribute must be present")
                .to_str()
                .expect("Patient Position must be text")
                .trim(),
            ""
        );
        assert_eq!(
            obj.element(tags::IMAGE_POSITION_PATIENT)
                .expect("Image Position Patient must be present")
                .value()
                .to_multi_float64()
                .expect("Image Position Patient must be numeric"),
            vec![0.0, 0.0, expected_positions[index]]
        );
    }

    let validation = dicom_test_suite::validate_generated_root(&out_dir)
        .expect("generated CT geometry corpus must be validatable");
    assert!(
        validation.failures.is_empty(),
        "CT series geometry validation must pass: {:?}",
        validation.failures
    );

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("coverage report should include geometry expectations");
    let report_schema: Value = serde_json::from_slice(
        &fs::read("schemas/coverage-report.schema.json").expect("coverage schema"),
    )
    .expect("coverage schema JSON");
    let report_validator =
        jsonschema::validator_for(&report_schema).expect("coverage schema should compile");
    let report_errors = report_validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        report_errors.is_empty(),
        "geometry coverage report must match schema: {report_errors:?}"
    );
    let geometry_rows = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix")
        .iter()
        .filter(|row| row["case_id"].as_str() == Some(CASE_ID))
        .collect::<Vec<_>>();
    assert_eq!(geometry_rows.len(), 3);
    assert_eq!(geometry_rows[0]["geometry_geometric_order_index"], 1);
    assert_eq!(geometry_rows[0]["geometry_instance_number"], 30);
    assert_eq!(geometry_rows[0]["geometry_sorting_conflict_expected"], true);
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Geometry Sorting Expectations"));
    assert!(markdown.contains("| 0.0 | 1 | 30 | 3 |"));

    fs::remove_dir_all(out_dir).expect("temporary output must be removable");
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must follow Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("dicom-test-suite-{label}-{nonce}"))
}
