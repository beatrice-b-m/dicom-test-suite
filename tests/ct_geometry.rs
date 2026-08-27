use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "geometry/ct/spatial_sort_conflicts_instance_number";
const NONUNIFORM_CASE_ID: &str = "geometry/ct/nonuniform_slice_spacing";
const GANTRY_TILT_CASE_ID: &str = "geometry/ct/gantry_tilt_series";
const GANTRY_TILT_DEGREES: f64 = 11.309_932_47;

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
        assert!(
            obj.element(tags::LATERALITY).is_err(),
            "Series Laterality should be absent when Image Laterality is present"
        );
        assert_eq!(
            obj.element(tags::IMAGE_LATERALITY)
                .expect("Image Laterality should resolve the Series Laterality condition")
                .to_str()
                .expect("Image Laterality must be text")
                .trim(),
            "U"
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
    assert!(markdown.contains("| 0.0 | 1 | numeric | 30 | 3 | 5.0, 5.0 | true |"));

    fs::remove_dir_all(out_dir).expect("temporary output must be removable");
}

#[test]
fn core_generates_nonuniform_ct_spacing_without_scalar_spacing_claim() {
    let out_dir = unique_temp_dir("ct-nonuniform-spacing");
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(&out_dir)
        .args(["--seed", "23"])
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
        .filter(|file| file["case_id"].as_str() == Some(NONUNIFORM_CASE_ID))
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 3);

    let expected_positions = [0.0_f64, 4.0, 10.0];
    for (index, file) in files.iter().enumerate() {
        assert_eq!(
            file.pointer("/expected_geometry/position_along_normal_mm")
                .and_then(Value::as_f64),
            Some(expected_positions[index])
        );
        assert_eq!(
            file.pointer("/expected_geometry/adjacent_spacing_mm"),
            Some(&serde_json::json!([4.0, 6.0]))
        );
        assert_eq!(
            file.pointer("/expected_geometry/spacing_uniform")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            file.pointer("/expected_geometry/instance_number_state")
                .and_then(Value::as_str),
            Some("numeric")
        );
        assert_eq!(
            file.pointer("/expected_geometry/sorting_conflict_expected")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            file.pointer("/recipe/recipe_parameters/geometry/spacing_between_slices")
                .is_none(),
            "nonuniform recipe must not claim one scalar spacing"
        );

        let object =
            open_file(out_dir.join(file["path"].as_str().expect("manifest path must be text")))
                .expect("nonuniform CT slice must parse");
        assert!(
            object.element(tags::SPACING_BETWEEN_SLICES).is_err(),
            "optional Spacing Between Slices must be absent for unequal intervals"
        );
        assert_eq!(
            object
                .element(tags::IMAGE_POSITION_PATIENT)
                .expect("Image Position Patient must be present")
                .value()
                .to_multi_float64()
                .expect("Image Position Patient must be numeric"),
            vec![0.0, 0.0, expected_positions[index]]
        );
    }

    let validation = dicom_test_suite::validate_generated_root(&out_dir)
        .expect("generated nonuniform CT corpus must be validatable");
    assert!(
        validation.failures.is_empty(),
        "nonuniform CT geometry validation must pass: {:?}",
        validation.failures
    );

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("coverage report should include nonuniform spacing expectations");
    let rows = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix")
        .iter()
        .filter(|row| row["case_id"].as_str() == Some(NONUNIFORM_CASE_ID))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0]["geometry_adjacent_spacing_mm"],
        serde_json::json!([4.0, 6.0])
    );
    assert_eq!(rows[0]["geometry_spacing_uniform"], false);
    assert_eq!(rows[0]["geometry_instance_number_state"], "numeric");
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains(NONUNIFORM_CASE_ID));
    assert!(markdown.contains("4.0, 6.0"));
    assert!(markdown.contains("false"));

    fs::remove_dir_all(out_dir).expect("temporary output must be removable");
}

#[test]
fn core_generates_gantry_tilt_with_independent_sheared_geometry() {
    let out_dir = unique_temp_dir("ct-gantry-tilt");
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(&out_dir)
        .args(["--seed", "29"])
        .output()
        .expect("generate command must run");
    assert!(
        output.status.success(),
        "generate should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest must be readable"))
            .expect("manifest must contain JSON");
    let files = manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .filter(|file| file["case_id"].as_str() == Some(GANTRY_TILT_CASE_ID))
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 3);

    let expected_positions = [0.0_f64, 5.0, 10.0];
    let expected_image_positions = [
        vec![0.0_f64, 0.0, 0.0],
        vec![0.0_f64, -1.0, 5.0],
        vec![0.0_f64, -2.0, 10.0],
    ];
    for (index, file) in files.iter().enumerate() {
        assert_eq!(
            file.pointer("/expected_geometry/image_orientation_patient"),
            Some(&serde_json::json!([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]))
        );
        assert_eq!(
            file.pointer("/expected_geometry/image_position_patient"),
            Some(&serde_json::json!(expected_image_positions[index]))
        );
        assert_eq!(
            file.pointer("/expected_geometry/position_along_normal_mm")
                .and_then(Value::as_f64),
            Some(expected_positions[index])
        );
        assert_eq!(
            file.pointer("/expected_geometry/adjacent_spacing_mm"),
            Some(&serde_json::json!([5.0, 5.0]))
        );
        assert_eq!(
            file.pointer("/expected_geometry/spacing_uniform")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            file.pointer("/expected_geometry/instance_number")
                .and_then(Value::as_i64),
            Some(index as i64 + 1)
        );
        assert_eq!(
            file.pointer("/expected_geometry/instance_number_order_index")
                .and_then(Value::as_u64),
            Some(index as u64 + 1)
        );
        assert_eq!(
            file.pointer("/expected_geometry/sorting_conflict_expected")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            file.pointer("/expected_geometry/gantry_detector_tilt_degrees")
                .and_then(Value::as_f64),
            Some(GANTRY_TILT_DEGREES)
        );
        assert_eq!(
            file.pointer("/recipe/recipe_parameters/geometry/gantry_detector_tilt_degrees")
                .and_then(Value::as_f64),
            Some(GANTRY_TILT_DEGREES)
        );
        assert!(
            file["expected_capabilities"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == "interpret_gantry_tilt"))
        );
        for stressor in ["gantry_detector_tilt", "sheared_slice_origins"] {
            assert!(
                file["known_stressors"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|value| value == stressor)),
                "gantry tilt manifest should record {stressor}"
            );
        }
        let internal_validation_names = file
            .pointer("/validation/internal")
            .and_then(Value::as_array)
            .expect("internal validation results must be recorded")
            .iter()
            .filter_map(|result| result["name"].as_str())
            .collect::<BTreeSet<_>>();
        for name in ["ct_image_orientation_patient", "ct_image_position_patient"] {
            assert!(
                internal_validation_names.contains(name),
                "serialized CT geometry should be internally reopened and checked"
            );
        }

        let object = open_file(
            out_dir.join(
                file["path"]
                    .as_str()
                    .expect("gantry tilt manifest path must be text"),
            ),
        )
        .expect("gantry tilt CT slice must parse");
        assert_eq!(
            object
                .element(tags::GANTRY_DETECTOR_TILT)
                .expect("Gantry/Detector Tilt must be present")
                .to_float64()
                .expect("Gantry/Detector Tilt must be numeric"),
            GANTRY_TILT_DEGREES
        );
        assert_eq!(
            object
                .element(tags::IMAGE_ORIENTATION_PATIENT)
                .expect("Image Orientation Patient must be present")
                .value()
                .to_multi_float64()
                .expect("Image Orientation Patient must be numeric"),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        );
        assert_eq!(
            object
                .element(tags::IMAGE_POSITION_PATIENT)
                .expect("Image Position Patient must be present")
                .value()
                .to_multi_float64()
                .expect("Image Position Patient must be numeric"),
            expected_image_positions[index]
        );
        assert_eq!(
            object
                .element(tags::SPACING_BETWEEN_SLICES)
                .expect("uniform tilted stack should declare spacing")
                .to_float64()
                .expect("Spacing Between Slices must be numeric"),
            5.0
        );
    }

    let validation = dicom_test_suite::validate_generated_root(&out_dir)
        .expect("generated gantry tilt corpus must be validatable");
    assert!(
        validation.failures.is_empty(),
        "gantry tilt geometry validation must pass: {:?}",
        validation.failures
    );

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("coverage report should include gantry tilt expectations");
    let rows = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix")
        .iter()
        .filter(|row| row["case_id"].as_str() == Some(GANTRY_TILT_CASE_ID))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| {
        row["geometry_gantry_detector_tilt_degrees"].as_f64() == Some(GANTRY_TILT_DEGREES)
    }));
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains(GANTRY_TILT_CASE_ID));
    assert!(markdown.contains("11.30993247"));

    let mut tampered_manifest = manifest;
    let first_tilt_file = tampered_manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"].as_str() == Some(GANTRY_TILT_CASE_ID))
        .expect("gantry tilt manifest row must exist");
    first_tilt_file["expected_geometry"]["gantry_detector_tilt_degrees"] =
        Value::from(GANTRY_TILT_DEGREES + 1.0);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&tampered_manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");
    let validation = dicom_test_suite::validate_generated_root(&out_dir)
        .expect("tampered gantry tilt corpus must still be inspectable");
    assert!(
        validation
            .failures
            .iter()
            .any(|failure| failure.contains("geometry_gantry_detector_tilt")),
        "false tilt expectations must be rejected: {:?}",
        validation.failures
    );

    fs::remove_dir_all(out_dir).expect("temporary output must be removable");
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must follow Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("dicom-test-suite-{label}-{nonce}"))
}
