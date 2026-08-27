use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/long_multivalue_text_numeric_strings";
const RELATIVE_PATH: &str = "metadata/sc/long_multivalue_text_numeric_strings/instance.dcm";
const FILE_SHA256: &str = "238f7478de59027060c3807a2075faf9deb9e32d2a4a33bf622170183470c5c2";

#[test]
fn string_boundary_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("string-boundary-first");
    let second_root = unique_temp_dir("string-boundary-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["path"], RELATIVE_PATH);
    assert_eq!(first["sha256"], FILE_SHA256);
    assert_eq!(first["expected_metadata"], second["expected_metadata"]);
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).expect("first fixture must be readable"),
        fs::read(second_root.join(RELATIVE_PATH)).expect("second fixture must be readable")
    );

    let elements = first["expected_metadata"]["string_elements"]
        .as_array()
        .expect("string expectations must be an array");
    assert_string_element(
        elements,
        "ImageComments",
        "LT",
        &[10_240],
        10_240,
        "75497849c172d88a38e271cc6ce82f31adbba1f16b6191d8ddaeb4e9f6268e52",
        "none",
    );
    assert_string_element(
        elements,
        "SoftwareVersions",
        "LO",
        &[64, 64],
        130,
        "e79f64c5853732dd713d14c3530ef494d800f684653fc5bf0aced3933241a260",
        "space",
    );
    assert_string_element(
        elements,
        "PixelSpacing",
        "DS",
        &[16, 16],
        34,
        "e09885a80758e44eaa4b9b544e7301c852395d3ee14ed7b7588e62a5f3b2db6a",
        "space",
    );
    assert_string_element(
        elements,
        "AcquisitionNumber",
        "IS",
        &[12],
        12,
        "f9cf9c74b83f0c66cdb48d3536a5a5d884babc2cfda813d01b3577b473de20cf",
        "none",
    );

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("fixture must parse");
    assert_eq!(
        object
            .element(tags::IMAGE_COMMENTS)
            .expect("Image Comments must exist")
            .to_str()
            .expect("Image Comments must decode")
            .as_ref(),
        "0123456789ABCDEF".repeat(640)
    );
    assert_eq!(
        decoded_values(&object, tags::SOFTWARE_VERSIONS),
        [
            "DTS-A-".to_string() + &"A".repeat(58),
            "DTS-B-".to_string() + &"B".repeat(58)
        ]
    );
    assert_eq!(
        decoded_values(&object, tags::PIXEL_SPACING),
        ["0.12345678901234", "0.98765432109876"]
    );
    assert_eq!(
        decoded_values(&object, tags::ACQUISITION_NUMBER),
        ["+02147483647"]
    );

    let manifest_schema = read_json("schemas/manifest.schema.json");
    assert!(
        jsonschema::validator_for(&manifest_schema)
            .expect("manifest schema must compile")
            .is_valid(&first_manifest)
    );
    let summary = dicom_test_suite::validate_generated_root(&first_root)
        .expect("generated corpus must be inspectable");
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert!(
        first["validation"]["internal"]
            .as_array()
            .expect("internal validation must be an array")
            .iter()
            .any(|result| result["name"] == "string_boundary_round_trip")
    );

    let report = report_json(&first_root);
    let report_schema = read_json("schemas/coverage-report.schema.json");
    assert!(
        jsonschema::validator_for(&report_schema)
            .expect("coverage report schema must compile")
            .is_valid(&report)
    );
    let row = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix must be an array")
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("coverage row must exist");
    assert_eq!(
        row["metadata_string_tags"],
        serde_json::json!(["0018,1020", "0020,0012", "0020,4000", "0028,0030"])
    );
    assert_eq!(
        row["metadata_string_vrs"],
        serde_json::json!(["LO", "IS", "LT", "DS"])
    );
    assert_eq!(
        row["metadata_string_value_multiplicities"],
        serde_json::json!([2, 1, 1, 2])
    );
    assert_eq!(
        row["metadata_string_max_component_encoded_length_bytes"],
        serde_json::json!([64, 12, 10240, 16])
    );
    assert_eq!(
        row["metadata_string_raw_value_lengths"],
        serde_json::json!([130, 12, 10240, 34])
    );
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_string_vrs/LO"),
        Some(&Value::from(1))
    );
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## String VR Boundary Expectations"));
    assert!(markdown.contains("0018,1020; 0020,0012; 0020,4000; 0028,0030"));
}

#[test]
fn validator_rejects_tampered_string_boundary_contract() {
    let root = unique_temp_dir("string-boundary-tampered");
    let mut manifest = generate_extended(&root);
    let elements = manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("string boundary case must be generated")["expected_metadata"]["string_elements"]
        .as_array_mut()
        .expect("string elements must be an array");
    elements[0]["raw_value_sha256"] = Value::from("0".repeat(64));
    elements[1]["value_multiplicity"] = Value::from(1);
    elements[1]["decoded_value_lengths"][0] = Value::from(63);
    elements[1]["padding"] = Value::from("none");
    elements[2]["decoded_values"][0] = Value::from("0.1");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");

    let summary = dicom_test_suite::validate_generated_root(&root)
        .expect("tampered corpus must remain inspectable");
    for failure_key in [
        "metadata_string_vm_lengths",
        "metadata_string_raw_contract",
        "metadata_string_decoded_values",
        "metadata_string_raw_hash",
    ] {
        assert!(
            summary
                .failures
                .iter()
                .any(|failure| failure.contains(failure_key)),
            "validator must report {failure_key}: {:?}",
            summary.failures
        );
    }
}

fn assert_string_element(
    elements: &[Value],
    keyword: &str,
    vr: &str,
    lengths: &[u64],
    raw_length: u64,
    raw_hash: &str,
    padding: &str,
) {
    let element = elements
        .iter()
        .find(|element| element["keyword"] == keyword)
        .unwrap_or_else(|| panic!("{keyword} expectation must exist"));
    assert_eq!(element["vr"], vr);
    assert_eq!(element["value_multiplicity"], lengths.len() as u64);
    assert_eq!(element["decoded_value_lengths"], serde_json::json!(lengths));
    assert_eq!(element["raw_value_byte_length"], raw_length);
    assert_eq!(element["raw_value_sha256"], raw_hash);
    assert_eq!(element["padding"], padding);
}

fn decoded_values(
    object: &dicom_object::FileDicomObject<
        dicom_object::InMemDicomObject<dicom_dictionary_std::StandardDataDictionary>,
    >,
    tag: dicom_core::Tag,
) -> Vec<String> {
    object
        .element(tag)
        .expect("string element must exist")
        .to_multi_str()
        .expect("string element must decode")
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn generate_extended(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "extended", "--out"])
        .arg(out_dir)
        .args(["--seed", "1"])
        .output()
        .expect("generate command must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_json(out_dir.join("manifest.json"))
}

fn report_json(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .arg("report")
        .arg(root)
        .args(["--format", "json"])
        .output()
        .expect("report command must run");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("report must be JSON")
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("string boundary case must be generated")
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).expect("JSON file must be readable"))
        .expect("file must contain JSON")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{nonce}",
        std::process::id()
    ))
}
