use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};

const CASE_ID: &str = "classic/nm/multiframe_explicit_le";
const RELATIVE_PATH: &str = "classic/nm/multiframe_explicit_le/instance.dcm";
const INSTANCE_SHA256: &str = "6f0f857b35c1abd133043cb0ae27543b1f56add494891f4b6ea7f8d50c96a7f4";
const FRAME_HASHES: [&str; 4] = [
    "245bbd9d484dcf27c714e2690cd6544973de5d54aa9cd82eab23d6046a65faa8",
    "a58214fbfec2da6f1e9fc6a2641c8a0af73fb383860180a73d4439fe31b44189",
    "4908c41ec85a7552278ed886fa3c43819f44d4df5b73138a9c5855926c750a58",
    "a12837f26e181e5420b019bae0940e221d2927e13fea963ad899945c34c697fe",
];

#[test]
fn nm_multiframe_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("nm-first");
    let second_root = unique_temp_dir("nm-second");
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
        Some(&Value::from(uids::NUCLEAR_MEDICINE_IMAGE_STORAGE))
    );
    assert_eq!(first.pointer("/dicom/modality"), Some(&Value::from("NM")));
    assert_eq!(first.pointer("/image/frames"), Some(&Value::from(4)));
    assert_eq!(
        first.pointer("/pixel_data/value_length"),
        Some(&Value::from(32))
    );
    assert_eq!(
        first.pointer("/pixel_data/frame_hashes"),
        Some(&json!(FRAME_HASHES))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/frame_increment_pointers"),
        Some(&json!(["0054,0010", "0054,0020"]))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/energy_window_vector"),
        Some(&json!([1, 1, 2, 2]))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/detector_vector"),
        Some(&json!([1, 2, 1, 2]))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/number_of_energy_windows"),
        Some(&Value::from(2))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/number_of_detectors"),
        Some(&Value::from(2))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/energy_windows"),
        Some(&json!([
            {"index": 1, "name": "Tc99m Photopeak", "lower_limit_kev": 126.0, "upper_limit_kev": 154.0},
            {"index": 2, "name": "Tc99m Scatter", "lower_limit_kev": 100.0, "upper_limit_kev": 120.0}
        ]))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/detectors"),
        Some(&json!([
            {"index": 1, "collimator_type": "PARA", "focal_distance_mm": 0.0, "start_angle_degrees": 0.0, "image_orientation_patient": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0], "image_position_patient": [0.0, 0.0, 0.0]},
            {"index": 2, "collimator_type": "PARA", "focal_distance_mm": 0.0, "start_angle_degrees": 180.0, "image_orientation_patient": [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0], "image_position_patient": [0.0, 0.0, 0.0]}
        ]))
    );
    assert_eq!(
        first.pointer("/expected_nm_multiframe/frame_dimensions"),
        Some(&json!([
            {"frame_number": 1, "energy_window_index": 1, "detector_index": 1, "frame_sha256": FRAME_HASHES[0]},
            {"frame_number": 2, "energy_window_index": 1, "detector_index": 2, "frame_sha256": FRAME_HASHES[1]},
            {"frame_number": 3, "energy_window_index": 2, "detector_index": 1, "frame_sha256": FRAME_HASHES[2]},
            {"frame_number": 4, "energy_window_index": 2, "detector_index": 2, "frame_sha256": FRAME_HASHES[3]}
        ]))
    );

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("NM fixture must parse");
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
        uids::NUCLEAR_MEDICINE_IMAGE_STORAGE
    );
    assert_eq!(
        object.element(tags::MODALITY).unwrap().to_str().unwrap(),
        "NM"
    );
    assert_eq!(
        object
            .element(tags::NUMBER_OF_FRAMES)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        4
    );
    assert_eq!(
        object
            .element(tags::FRAME_INCREMENT_POINTER)
            .unwrap()
            .value()
            .tags()
            .unwrap(),
        &[tags::ENERGY_WINDOW_VECTOR, tags::DETECTOR_VECTOR]
    );
    assert_eq!(
        object
            .element(tags::ENERGY_WINDOW_VECTOR)
            .unwrap()
            .to_multi_int::<u16>()
            .unwrap(),
        vec![1, 1, 2, 2]
    );
    assert_eq!(
        object
            .element(tags::DETECTOR_VECTOR)
            .unwrap()
            .to_multi_int::<u16>()
            .unwrap(),
        vec![1, 2, 1, 2]
    );

    let pixel_bytes = object
        .element(tags::PIXEL_DATA)
        .unwrap()
        .value()
        .to_bytes()
        .unwrap();
    let decoded = pixel_bytes
        .chunks_exact(2)
        .map(|sample| u16::from_le_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    assert_eq!(
        decoded,
        vec![
            0, 1, 2, 3, 10, 11, 12, 13, 100, 101, 102, 103, 110, 111, 112, 113
        ]
    );

    let windows = object
        .element(tags::ENERGY_WINDOW_INFORMATION_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(windows.len(), 2);
    assert_eq!(
        windows[0]
            .element(tags::ENERGY_WINDOW_NAME)
            .unwrap()
            .to_str()
            .unwrap(),
        "Tc99m Photopeak"
    );
    assert_eq!(
        windows[1]
            .element(tags::ENERGY_WINDOW_NAME)
            .unwrap()
            .to_str()
            .unwrap(),
        "Tc99m Scatter"
    );
    let detectors = object
        .element(tags::DETECTOR_INFORMATION_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(detectors.len(), 2);
    assert_eq!(
        detectors[0]
            .element(tags::START_ANGLE)
            .unwrap()
            .to_float64()
            .unwrap(),
        0.0
    );
    assert_eq!(
        detectors[1]
            .element(tags::START_ANGLE)
            .unwrap()
            .to_float64()
            .unwrap(),
        180.0
    );

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
    assert_eq!(row["nm_frame_increment_pointers"], "0054,0010; 0054,0020");
    assert_eq!(row["nm_energy_window_vector"], "1; 1; 2; 2");
    assert_eq!(row["nm_detector_vector"], "1; 2; 1; 2");
    assert_eq!(
        row["nm_energy_window_names"],
        "Tc99m Photopeak; Tc99m Scatter"
    );
    assert_eq!(row["nm_detector_start_angles_degrees"], "0.0; 180.0");
    assert_eq!(
        row["nm_frame_dimension_tuples"],
        "1:1:1; 2:1:2; 3:2:1; 4:2:2"
    );
    assert_eq!(
        report.pointer("/grouped_coverage/nm_energy_window_vectors/1; 1; 2; 2"),
        Some(&Value::from(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/nm_detector_vectors/1; 2; 1; 2"),
        Some(&Value::from(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/nm_frame_dimension_tuples/1:1:1; 2:1:2; 3:2:1; 4:2:2"),
        Some(&Value::from(1))
    );
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Nuclear Medicine Multi-frame Expectations"));
    assert!(markdown.contains("Tc99m Photopeak; Tc99m Scatter"));
    assert!(markdown.contains("1:1:1; 2:1:2; 3:2:1; 4:2:2"));
}

#[test]
fn validator_rejects_tampered_nm_dimension_contract() {
    let root = unique_temp_dir("nm-tampered");
    let manifest = generate_core(&root);
    for (pointer, replacement, failure_key) in [
        (
            "/expected_nm_multiframe/frame_increment_pointers",
            json!(["0054,0020", "0054,0010"]),
            "nm_frame_increment_pointers",
        ),
        (
            "/expected_nm_multiframe/energy_window_vector",
            json!([2, 1, 2, 2]),
            "nm_energy_window_vector",
        ),
        (
            "/expected_nm_multiframe/number_of_energy_windows",
            json!(1),
            "nm_number_of_energy_windows",
        ),
        (
            "/expected_nm_multiframe/energy_windows/0/name",
            json!("Wrong window"),
            "nm_energy_window_name",
        ),
        (
            "/expected_nm_multiframe/detectors/1/start_angle_degrees",
            json!(90.0),
            "nm_detector_start_angle",
        ),
        (
            "/expected_nm_multiframe/frame_dimensions/1/detector_index",
            json!(1),
            "nm_frame_detector_index",
        ),
        (
            "/expected_nm_multiframe/frame_dimensions/0/frame_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "nm_frame_dimension_hash",
        ),
        (
            "/pixel_data/frame_hashes/0",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "native_pixel_data_frame_hash",
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
        .expect("NM manifest entry must exist")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("NM manifest entry must exist")
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
