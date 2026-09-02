use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/iso2022_person_name_component_groups";
const RELATIVE_PATH: &str = "metadata/sc/iso2022_person_name_component_groups/instance.dcm";
const PATIENT_NAME: &str = "Yamada^Tarou=山田^太郎=やまだ^たろう";
const RAW_PATIENT_NAME_HEX: &str = "59616D6164615E5461726F753D1B24423B3345441B28425E1B244242404F3A1B28423D1B24422464245E24401B28425E1B2442243F246D24261B2842";
const RAW_PATIENT_NAME_SHA256: &str =
    "b206df163ce0b4d071469834428bf0b87b241931c81110362ce480d73d7490af";

#[test]
fn iso2022_person_name_is_byte_stable_strictly_valid_and_reported() {
    let first_root = unique_temp_dir("iso2022-pn-first");
    let second_root = unique_temp_dir("iso2022-pn-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["path"], RELATIVE_PATH);
    assert_eq!(first["expected_metadata"], second["expected_metadata"]);
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).expect("first fixture must be readable"),
        fs::read(second_root.join(RELATIVE_PATH)).expect("second fixture must be readable"),
        "the ISO 2022 fixture must be byte-stable for the same seed"
    );

    let expected = &first["expected_metadata"];
    assert_eq!(
        expected["specific_character_sets"],
        serde_json::json!(["", "ISO 2022 IR 87"])
    );
    let person_name = &expected["person_names"][0];
    assert_eq!(person_name["decoded_value"], PATIENT_NAME);
    assert_eq!(person_name["raw_value_hex"], RAW_PATIENT_NAME_HEX);
    assert_eq!(person_name["raw_value_sha256"], RAW_PATIENT_NAME_SHA256);
    assert_eq!(person_name["raw_value_byte_length"], 60);
    assert_eq!(
        person_name["component_groups"].as_array().map(Vec::len),
        Some(3)
    );
    assert_eq!(person_name["component_groups"][1]["kind"], "ideographic");
    assert_eq!(
        person_name["component_groups"][1]["decoded_value"],
        "山田^太郎"
    );
    assert_eq!(person_name["component_groups"][2]["kind"], "phonetic");
    assert_eq!(
        person_name["component_groups"][2]["decoded_value"],
        "やまだ^たろう"
    );

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("ISO 2022 fixture must parse");
    let character_sets = object
        .element(tags::SPECIFIC_CHARACTER_SET)
        .expect("Specific Character Set must exist")
        .to_multi_str()
        .expect("Specific Character Set must be readable");
    assert_eq!(
        character_sets
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["", "ISO 2022 IR 87"]
    );
    let patient_name_bytes = object
        .element(tags::PATIENT_NAME)
        .expect("Patient Name must exist")
        .value()
        .to_bytes()
        .expect("controlled Patient Name bytes must be readable");
    assert_eq!(
        uppercase_hex(patient_name_bytes.as_ref()),
        RAW_PATIENT_NAME_HEX
    );

    let summary = synth_dicom_gen::validate_generated_root(&first_root)
        .expect("generated ISO 2022 corpus must be inspectable");
    assert!(
        summary.failures.is_empty(),
        "strict metadata validation must pass: {:?}",
        summary.failures
    );
    assert!(
        first["validation"]["internal"]
            .as_array()
            .expect("internal validation must be an array")
            .iter()
            .any(|result| result["name"] == "iso2022_person_name_encoded_round_trip")
    );

    let report = report_json(&first_root);
    let row = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix must be an array")
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("ISO 2022 report row must exist");
    assert_eq!(row["metadata_specific_character_sets"], "\\ISO 2022 IR 87");
    assert_eq!(row["metadata_person_name"], PATIENT_NAME);
    assert_eq!(
        row["metadata_person_name_component_groups"],
        "alphabetic:Yamada^Tarou | ideographic:山田^太郎 | phonetic:やまだ^たろう"
    );
    assert_eq!(row["metadata_person_name_component_group_count"], 3);
    assert_eq!(
        row["metadata_person_name_encoded_sha256"],
        RAW_PATIENT_NAME_SHA256
    );
    assert_eq!(row["metadata_person_name_encoded_length_bytes"], 60);
}

#[test]
fn validator_rejects_tampered_iso2022_byte_contract() {
    let root = unique_temp_dir("iso2022-pn-tampered");
    let mut manifest = generate_extended(&root);
    let file = manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("ISO 2022 case must be generated");
    file["expected_metadata"]["person_names"][0]["raw_value_hex"] = Value::from("00".repeat(60));
    file["expected_metadata"]["person_names"][0]["raw_value_sha256"] = Value::from("0".repeat(64));
    file["expected_metadata"]["person_names"][0]["component_groups"][2]["decoded_value"] =
        Value::from("wrong^phonetic");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");

    let summary = synth_dicom_gen::validate_generated_root(&root)
        .expect("tampered corpus must remain inspectable");
    for expected_failure in [
        "metadata_person_name_raw_hex",
        "metadata_person_name_raw_hash",
        "metadata_person_name_component_group",
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

fn generate_extended(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "extended", "--out"])
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

fn report_json(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .arg("report")
        .arg(root)
        .args(["--format", "json"])
        .output()
        .expect("report command must run");
    assert!(
        output.status.success(),
        "report must pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("report must emit JSON")
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("ISO 2022 case must be generated")
}

fn uppercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02X}").expect("writing to a String cannot fail");
    }
    encoded
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
