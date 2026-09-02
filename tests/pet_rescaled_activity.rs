use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};

const CASE_ID: &str = "classic/pet/rescaled_activity_explicit_le";
const RELATIVE_PATH: &str = "classic/pet/rescaled_activity_explicit_le/instance.dcm";
const INSTANCE_SHA256: &str = "78ced6c57926cafc6538ebf65459bb9efd7ecbb9a3c4ec90b28b4457cc795ce6";
const FRAME_SHA256: &str = "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784";

#[test]
fn pet_rescaled_activity_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("pet-first");
    let second_root = unique_temp_dir("pet-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["sha256"], INSTANCE_SHA256);
    assert_eq!(second["sha256"], INSTANCE_SHA256);
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).unwrap(),
        fs::read(second_root.join(RELATIVE_PATH)).unwrap()
    );
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);

    assert_eq!(
        first.pointer("/dicom/sop_class_uid"),
        Some(&Value::from(
            uids::POSITRON_EMISSION_TOMOGRAPHY_IMAGE_STORAGE
        ))
    );
    assert_eq!(first.pointer("/dicom/modality"), Some(&Value::from("PT")));
    assert_eq!(first.pointer("/image/frames"), Some(&Value::from(1)));
    assert_eq!(
        first.pointer("/pixel_data/value_length"),
        Some(&Value::from(8))
    );
    assert_eq!(
        first.pointer("/pixel_data/frame_hashes"),
        Some(&json!([FRAME_SHA256]))
    );
    assert_eq!(
        first.pointer("/expected_pet_activity"),
        Some(&json!({
            "activity_values_bqml": [0.0, 250.0, 500.0, 1000.0],
            "actual_frame_duration_ms": 60000,
            "corrected_image": ["DCAL"],
            "counts_source": "EMISSION",
            "decay_correction": "NONE",
            "dose_calibration_factor": 1.0,
            "frame_reference_time_ms": 30000.0,
            "image_index": 1,
            "image_type": ["ORIGINAL", "PRIMARY"],
            "number_of_slices": 1,
            "radiopharmaceutical_information_item_count": 0,
            "rescale_intercept": 0.0,
            "rescale_slope": 2.5,
            "series_type": ["STATIC", "IMAGE"],
            "stored_values": [0, 100, 200, 400],
            "units": "BQML"
        }))
    );

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("PET fixture must parse");
    assert_eq!(
        object.meta().transfer_syntax.trim_end_matches('\0'),
        uids::EXPLICIT_VR_LITTLE_ENDIAN
    );
    assert_eq!(
        object
            .element(tags::SOP_CLASS_UID)
            .unwrap()
            .to_str()
            .unwrap(),
        uids::POSITRON_EMISSION_TOMOGRAPHY_IMAGE_STORAGE
    );
    for (tag, expected) in [
        (tags::MODALITY, "PT"),
        (tags::UNITS, "BQML"),
        (tags::COUNTS_SOURCE, "EMISSION"),
        (tags::SERIES_TYPE, "STATIC\\IMAGE"),
        (tags::CORRECTED_IMAGE, "DCAL"),
        (tags::DECAY_CORRECTION, "NONE"),
        (tags::DOSE_CALIBRATION_FACTOR, "1"),
        (tags::RESCALE_INTERCEPT, "0"),
        (tags::RESCALE_SLOPE, "2.5"),
        (tags::FRAME_REFERENCE_TIME, "30000"),
        (tags::ACTUAL_FRAME_DURATION, "60000"),
        (tags::IMAGE_TYPE, "ORIGINAL\\PRIMARY"),
        (tags::PIXEL_SPACING, "4\\4"),
        (tags::IMAGE_ORIENTATION_PATIENT, "1\\0\\0\\0\\1\\0"),
        (tags::IMAGE_POSITION_PATIENT, "0\\0\\0"),
        (tags::SLICE_THICKNESS, "4"),
    ] {
        assert_eq!(
            object.element(tag).unwrap().to_str().unwrap(),
            expected,
            "unexpected value for {tag:?}"
        );
    }
    assert_eq!(
        object
            .element(tags::NUMBER_OF_SLICES)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        1
    );
    assert_eq!(
        object
            .element(tags::IMAGE_INDEX)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        1
    );
    for tag in [
        tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
        tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
        tags::PATIENT_GANTRY_RELATIONSHIP_CODE_SEQUENCE,
    ] {
        assert_eq!(object.element(tag).unwrap().items().unwrap().len(), 0);
    }

    let pixel_bytes = object
        .element(tags::PIXEL_DATA)
        .unwrap()
        .value()
        .to_bytes()
        .unwrap();
    let stored_values = pixel_bytes
        .chunks_exact(2)
        .map(|sample| u16::from_le_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    assert_eq!(stored_values, vec![0, 100, 200, 400]);
    let intercept = object
        .element(tags::RESCALE_INTERCEPT)
        .unwrap()
        .to_float64()
        .unwrap();
    let slope = object
        .element(tags::RESCALE_SLOPE)
        .unwrap()
        .to_float64()
        .unwrap();
    let activity_values = stored_values
        .iter()
        .map(|stored| f64::from(*stored) * slope + intercept)
        .collect::<Vec<_>>();
    assert_eq!(activity_values, vec![0.0, 250.0, 500.0, 1000.0]);

    let summary = synth_dicom_gen::validate_generated_root(&first_root).unwrap();
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);

    let report = report_json(&first_root);
    coverage_report::assert_current_contract(&first_root, &report);
    let row = report["coverage_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .unwrap();
    assert_eq!(row["pet_units"], "BQML");
    assert_eq!(row["pet_counts_source"], "EMISSION");
    assert_eq!(row["pet_series_type"], "STATIC; IMAGE");
    assert_eq!(row["pet_corrected_image"], "DCAL");
    assert_eq!(row["pet_decay_correction"], "NONE");
    assert_eq!(row["pet_dose_calibration_factor"], 1.0);
    assert_eq!(row["pet_rescale_intercept"], 0.0);
    assert_eq!(row["pet_rescale_slope"], 2.5);
    assert_eq!(row["pet_stored_values"], "0; 100; 200; 400");
    assert_eq!(row["pet_activity_values_bqml"], "0.0; 250.0; 500.0; 1000.0");
    assert_eq!(row["pet_frame_reference_time_ms"], 30000.0);
    assert_eq!(row["pet_actual_frame_duration_ms"], 60000);
    assert_eq!(row["pet_image_index"], 1);
    assert_eq!(row["pet_radiopharmaceutical_information_item_count"], 0);
    for pointer in [
        "/grouped_coverage/pet_units/BQML",
        "/grouped_coverage/pet_counts_sources/EMISSION",
        "/grouped_coverage/pet_series_types/STATIC; IMAGE",
        "/grouped_coverage/pet_corrected_images/DCAL",
        "/grouped_coverage/pet_decay_corrections/NONE",
        "/grouped_coverage/pet_dose_calibration_factors/1.0",
        "/grouped_coverage/pet_rescale_intercepts/0.0",
        "/grouped_coverage/pet_rescale_slopes/2.5",
        "/grouped_coverage/pet_stored_values/0; 100; 200; 400",
        "/grouped_coverage/pet_activity_values_bqml/0.0; 250.0; 500.0; 1000.0",
        "/grouped_coverage/pet_frame_reference_times_ms/30000.0",
        "/grouped_coverage/pet_actual_frame_durations_ms/60000",
        "/grouped_coverage/pet_image_indices/1",
        "/grouped_coverage/pet_radiopharmaceutical_information_item_counts/0",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## PET Activity Expectations"));
    assert!(markdown.contains(CASE_ID));
    assert!(markdown.contains("0.0; 250.0; 500.0; 1000.0"));
    assert!(markdown.contains("## PET Units"));
    assert!(markdown.contains("## PET Activity Values (BQML)"));
}

#[test]
fn validator_rejects_tampered_pet_activity_contract() {
    let root = unique_temp_dir("pet-tampered");
    let manifest = generate_core(&root);
    for (pointer, replacement, failure_key) in [
        (
            "/expected_pet_activity/units",
            json!("CNTS"),
            "pet_units_manifest_contract",
        ),
        (
            "/expected_pet_activity/series_type",
            json!(["DYNAMIC", "IMAGE"]),
            "pet_series_type_manifest_contract",
        ),
        (
            "/expected_pet_activity/corrected_image",
            json!(["ATTN"]),
            "pet_corrected_image_manifest_contract",
        ),
        (
            "/expected_pet_activity/decay_correction",
            json!("START"),
            "pet_decay_correction_manifest_contract",
        ),
        (
            "/expected_pet_activity/rescale_slope",
            json!(3.0),
            "pet_rescale_slope_manifest_contract",
        ),
        (
            "/expected_pet_activity/rescale_intercept",
            json!(1.0),
            "pet_rescale_intercept_manifest_contract",
        ),
        (
            "/expected_pet_activity/stored_values/1",
            json!(101),
            "pet_stored_values_manifest_contract",
        ),
        (
            "/expected_pet_activity/activity_values_bqml/1",
            json!(251.0),
            "pet_activity_values_bqml_manifest_contract",
        ),
        (
            "/expected_pet_activity/radiopharmaceutical_information_item_count",
            json!(1),
            "pet_radiopharmaceutical_information_empty_manifest_contract",
        ),
        (
            "/expected_pet_activity/frame_reference_time_ms",
            json!(31000.0),
            "pet_frame_reference_time_manifest_contract",
        ),
        (
            "/expected_pet_activity/actual_frame_duration_ms",
            json!(61000),
            "pet_actual_frame_duration_manifest_contract",
        ),
        (
            "/expected_pet_activity/image_index",
            json!(2),
            "pet_image_index_manifest_contract",
        ),
        (
            "/pixel_data/frame_hashes/0",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "pet_pixel_frame_hash",
        ),
    ] {
        let mut tampered = manifest.clone();
        *case_file_mut(&mut tampered).pointer_mut(pointer).unwrap() = replacement;
        write_manifest(&root, &tampered);
        if !crate::curated_manifest_contract_support::curated_manifest_schema_is_valid(&tampered) {
            let error = synth_dicom_gen::validate_generated_root(&root)
                .expect_err("schema-invalid tampering must fail before semantic validation");
            assert!(
                error.to_string().contains("manifest schema invalid"),
                "{error}"
            );
            continue;
        }
        let summary = synth_dicom_gen::validate_generated_root(&root).unwrap();
        assert!(
            summary
                .failures
                .iter()
                .any(|failure| failure.contains(failure_key)),
            "missing {failure_key} for {pointer}: {:?}",
            summary.failures
        );
    }
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("PET manifest entry must exist")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("PET manifest entry must exist")
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(out_dir)
        .args(["--seed", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_json(out_dir.join("manifest.json"))
}

fn report_json(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .arg("report")
        .arg(root)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn write_manifest(root: &Path, manifest: &Value) {
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(manifest).unwrap(),
    )
    .unwrap();
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{nonce}",
        std::process::id()
    ))
}
#[path = "support/coverage_report.rs"]
mod coverage_report;
