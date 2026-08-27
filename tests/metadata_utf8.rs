use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/utf8_person_name";
const RELATIVE_PATH: &str = "metadata/sc/utf8_person_name/instance.dcm";
const PATIENT_NAME: &str = "Wang^XiaoDong=王^小東";
const RAW_PATIENT_NAME_SHA256: &str =
    "64a9d3d6b55142162489a8679e8643caa94efcff26dd30bf24650ac5186c1382";

#[test]
fn utf8_person_name_vertical_slice_is_exact_and_byte_stable() {
    let first_root = unique_temp_dir("utf8-pn-first");
    let second_root = unique_temp_dir("utf8-pn-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);

    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);
    assert_eq!(first["path"], RELATIVE_PATH);
    assert_eq!(first["expected_metadata"], second["expected_metadata"]);
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).expect("first fixture must be readable"),
        fs::read(second_root.join(RELATIVE_PATH)).expect("second fixture must be readable"),
        "the UTF-8 fixture must be byte-stable for the same seed"
    );

    let expected = &first["expected_metadata"];
    assert_eq!(expected["specific_character_sets"][0], "ISO_IR 192");
    let person_name = &expected["person_names"][0];
    assert_eq!(person_name["tag"], "0010,0010");
    assert_eq!(person_name["vr"], "PN");
    assert_eq!(person_name["decoded_value"], PATIENT_NAME);
    assert_eq!(person_name["raw_value_byte_length"], 24);
    assert_eq!(person_name["raw_value_sha256"], RAW_PATIENT_NAME_SHA256);
    assert_eq!(
        person_name["component_groups"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(person_name["component_groups"][0]["kind"], "alphabetic");
    assert_eq!(
        person_name["component_groups"][0]["components"][1]["decoded_value"],
        "XiaoDong"
    );
    assert_eq!(person_name["component_groups"][1]["kind"], "ideographic");
    assert_eq!(
        person_name["component_groups"][1]["components"][1]["decoded_value"],
        "小東"
    );

    let schema: Value = serde_json::from_slice(
        &fs::read("schemas/manifest.schema.json").expect("manifest schema must be readable"),
    )
    .expect("manifest schema must be JSON");
    let schema_validator =
        jsonschema::validator_for(&schema).expect("manifest schema must compile");
    let schema_errors = schema_validator
        .iter_errors(&first_manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        schema_errors.is_empty(),
        "generated manifest must satisfy the strict metadata schema: {schema_errors:?}"
    );

    let obj = open_file(first_root.join(RELATIVE_PATH)).expect("UTF-8 fixture must parse");
    assert_eq!(
        obj.element(tags::SPECIFIC_CHARACTER_SET)
            .expect("Specific Character Set must exist")
            .to_str()
            .expect("Specific Character Set must decode")
            .as_ref(),
        "ISO_IR 192"
    );
    assert_eq!(
        obj.element(tags::PATIENT_NAME)
            .expect("Patient Name must exist")
            .to_str()
            .expect("Patient Name must decode")
            .as_ref(),
        PATIENT_NAME
    );
    assert_eq!(
        obj.element(tags::LATERALITY)
            .expect("General Series Laterality must exist")
            .to_str()
            .expect("Laterality must decode")
            .as_ref(),
        "R"
    );

    let summary = dicom_test_suite::validate_generated_root(&first_root)
        .expect("generated UTF-8 corpus must be inspectable");
    assert!(
        summary.failures.is_empty(),
        "strict metadata validation must pass: {:?}",
        summary.failures
    );
    let internal_names = first["validation"]["internal"]
        .as_array()
        .expect("internal validation must be an array")
        .iter()
        .filter_map(|result| result["name"].as_str())
        .collect::<Vec<_>>();
    assert!(internal_names.contains(&"utf8_person_name_round_trip"));
}

#[test]
fn validator_rejects_tampered_utf8_metadata_expectations() {
    let root = unique_temp_dir("utf8-pn-tampered");
    let mut manifest = generate_core(&root);
    let file = manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"].as_str() == Some(CASE_ID))
        .expect("UTF-8 case must be generated");
    file["expected_metadata"]["specific_character_sets"][0] = Value::from("ISO 2022 IR 87");
    file["expected_metadata"]["person_names"][0]["decoded_value"] = Value::from("Wrong^Name");
    file["expected_metadata"]["person_names"][0]["raw_value_sha256"] = Value::from("0".repeat(64));
    file["expected_metadata"]["person_names"][0]["component_groups"][0]["decoded_value"] =
        Value::from("Wrong^Group");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");

    let summary = dicom_test_suite::validate_generated_root(&root)
        .expect("tampered corpus must remain inspectable");
    for expected_failure in [
        "metadata_specific_character_sets",
        "metadata_specific_character_sets_raw",
        "metadata_person_name_decoded",
        "metadata_person_name_component_group",
        "metadata_person_name_raw_hash",
    ] {
        assert!(
            summary
                .failures
                .iter()
                .any(|failure| failure.contains(expected_failure)),
            "validator must report {expected_failure}: {:?}",
            summary.failures
        );
    }
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(out_dir)
        .args(["--seed", "37"])
        .output()
        .expect("generate command must run");
    assert!(
        output.status.success(),
        "generation must pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(
        &fs::read(out_dir.join("manifest.json")).expect("manifest must be readable"),
    )
    .expect("manifest must be JSON")
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .find(|file| file["case_id"].as_str() == Some(CASE_ID))
        .expect("UTF-8 case must be generated")
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
