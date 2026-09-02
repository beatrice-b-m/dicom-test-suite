use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::VR;
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/empty_type2_attributes";
const RELATIVE_PATH: &str = "metadata/sc/empty_type2_attributes/instance.dcm";
const FILE_SHA256: &str = "7f457e4f9593a8d41dff970d32de86c8b5493841546dd6d60b219f311a7abc7c";

#[test]
fn empty_type2_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("empty-type2-first");
    let second_root = unique_temp_dir("empty-type2-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["path"], RELATIVE_PATH);
    assert_eq!(first["sha256"], FILE_SHA256);
    assert_eq!(first["expected_metadata"], second["expected_metadata"]);
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).expect("first fixture must be readable"),
        fs::read(second_root.join(RELATIVE_PATH)).expect("second fixture must be readable"),
        "the empty Type 2 fixture must be byte-stable for the same seed"
    );

    let attributes = first["expected_metadata"]["empty_type2_attributes"]
        .as_array()
        .expect("empty Type 2 expectations must be an array");
    let expected = [
        (tags::PATIENT_NAME, "0010,0010", "PatientName", VR::PN),
        (
            tags::PATIENT_BIRTH_DATE,
            "0010,0030",
            "PatientBirthDate",
            VR::DA,
        ),
        (tags::PATIENT_SEX, "0010,0040", "PatientSex", VR::CS),
        (
            tags::REFERRING_PHYSICIAN_NAME,
            "0008,0090",
            "ReferringPhysicianName",
            VR::PN,
        ),
        (
            tags::ACCESSION_NUMBER,
            "0008,0050",
            "AccessionNumber",
            VR::SH,
        ),
    ];
    assert_eq!(attributes.len(), expected.len());

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("fixture must parse");
    for (attribute, (tag, tag_text, keyword, vr)) in attributes.iter().zip(expected) {
        assert_eq!(attribute["tag"], tag_text);
        assert_eq!(attribute["keyword"], keyword);
        assert_eq!(attribute["vr"], format!("{vr:?}"));
        assert_eq!(attribute["value_length"], 0);
        let element = object
            .element(tag)
            .unwrap_or_else(|_| panic!("{keyword} must be present"));
        assert_eq!(element.vr(), vr);
        assert!(
            element
                .to_bytes()
                .unwrap_or_else(|_| panic!("{keyword} must decode"))
                .is_empty(),
            "{keyword} must have no value bytes"
        );
    }
    assert_eq!(
        object
            .element(tags::LATERALITY)
            .expect("Laterality must remain populated")
            .to_str()
            .expect("Laterality must decode")
            .as_ref(),
        "R"
    );

    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);
    let summary = synth_dicom_gen::validate_generated_root(&first_root)
        .expect("generated corpus must be inspectable");
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert!(
        first["validation"]["internal"]
            .as_array()
            .expect("internal validation must be an array")
            .iter()
            .any(|result| result["name"] == "empty_type2_round_trip")
    );

    let report = report_json(&first_root);
    let row = report["coverage_matrix"]
        .as_array()
        .expect("coverage matrix must be an array")
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("coverage row must exist");
    assert_eq!(row["metadata_empty_type2_attribute_count"], 5);
    assert!(
        row["metadata_empty_type2_attributes"]
            .as_str()
            .is_some_and(|value| value.contains("0008,0050 AccessionNumber SH VL=0"))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_empty_type2_attribute_counts/5"),
        Some(&Value::from(1))
    );
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Empty Type 2 Metadata Expectations"));
    assert!(markdown.contains("0010,0010 PatientName PN VL=0"));
}

#[test]
fn validator_rejects_tampered_empty_type2_contract() {
    let root = unique_temp_dir("empty-type2-tampered");
    let mut manifest = generate_core(&root);
    let file = manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("empty Type 2 case must be generated");
    file["expected_metadata"]["empty_type2_attributes"][0]["value_length"] = Value::from(2);
    file["expected_metadata"]["empty_type2_attributes"][1]["vr"] = Value::from("PN");
    file["expected_metadata"]["empty_type2_attributes"][2]["keyword"] = Value::from("WrongKeyword");
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_rejected(&manifest);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");

    let error = synth_dicom_gen::validate_generated_root(&root)
        .expect_err("schema-invalid tampering must fail before semantic inspection");
    assert!(
        error.to_string().contains("manifest schema invalid"),
        "{error}"
    );
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(out_dir)
        .args(["--seed", "1"])
        .output()
        .expect("generate command must run");
    assert!(
        output.status.success(),
        "generation must pass: {}",
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
        .expect("empty Type 2 case must be generated")
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
