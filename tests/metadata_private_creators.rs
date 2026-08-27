use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::{Tag, VR};
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/private_creator_blocks";
const RELATIVE_PATH: &str = "metadata/sc/private_creator_blocks/instance.dcm";
const FILE_SHA256: &str = "5a0726a68554bb55a6dc5f7a74f639138dc365e8a46f444013303261705141e9";

#[test]
fn private_creator_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("private-creators-first");
    let second_root = unique_temp_dir("private-creators-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["path"], RELATIVE_PATH);
    assert_eq!(first["sha256"], FILE_SHA256);
    assert_eq!(first["expected_metadata"], second["expected_metadata"]);
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).expect("first fixture must be readable"),
        fs::read(second_root.join(RELATIVE_PATH)).expect("second fixture must be readable")
    );

    let blocks = first["expected_metadata"]["private_creator_blocks"]
        .as_array()
        .expect("private creator blocks must be an array");
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        first["recipe"]["recipe_parameters"]["private_creator_block_count"],
        3
    );
    assert_eq!(
        blocks
            .iter()
            .map(|block| (
                block["creator_tag"].as_str().expect("creator tag"),
                block["creator_id"].as_str().expect("creator ID"),
                block["block_start_tag"].as_str().expect("block start"),
                block["block_end_tag"].as_str().expect("block end")
            ))
            .collect::<Vec<_>>(),
        [
            ("0011,0010", "DTS_PRIVATE_ALPHA", "0011,1000", "0011,10FF"),
            ("0011,0012", "DTS_PRIVATE_BETA", "0011,1200", "0011,12FF"),
            ("0013,0011", "DTS_PRIVATE_ALPHA", "0013,1100", "0013,11FF")
        ]
    );

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("fixture must parse");
    for (tag, expected) in [
        (Tag(0x0011, 0x0010), "DTS_PRIVATE_ALPHA"),
        (Tag(0x0011, 0x0012), "DTS_PRIVATE_BETA"),
        (Tag(0x0013, 0x0011), "DTS_PRIVATE_ALPHA"),
        (Tag(0x0011, 0x1001), "ALPHA-GROUP-0011"),
        (Tag(0x0011, 0x1201), "BETA-BLOCK-12"),
        (Tag(0x0013, 0x1101), "ALPHA-GROUP-0013"),
    ] {
        let element = object.element(tag).expect("private LO must exist");
        assert_eq!(element.vr(), VR::LO);
        assert_eq!(element.to_str().expect("private LO must decode"), expected);
    }
    let private_us = object
        .element(Tag(0x0011, 0x10F0))
        .expect("private US must exist");
    assert_eq!(private_us.vr(), VR::US);
    assert_eq!(private_us.to_int::<u16>().expect("US must decode"), 4660);

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
            .any(|result| result["name"] == "private_creator_block_round_trip")
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
        row["metadata_private_creator_tags"],
        serde_json::json!(["0011,0010", "0011,0012", "0013,0011"])
    );
    assert_eq!(
        row["metadata_private_creator_ids"],
        serde_json::json!(["DTS_PRIVATE_ALPHA", "DTS_PRIVATE_BETA", "DTS_PRIVATE_ALPHA"])
    );
    assert_eq!(
        row["metadata_private_block_ranges"],
        serde_json::json!([
            "0011,1000-0011,10FF",
            "0011,1200-0011,12FF",
            "0013,1100-0013,11FF"
        ])
    );
    assert_eq!(
        row["metadata_private_element_tags"],
        serde_json::json!(["0011,1001", "0011,10F0", "0011,1201", "0013,1101"])
    );
    assert_eq!(
        row["metadata_private_element_vrs"],
        serde_json::json!(["LO", "US", "LO", "LO"])
    );
    assert_eq!(
        row["metadata_private_creator_raw_sha256_values"],
        serde_json::json!([
            "02a7ccdec62f131efea4bb7c0954d15df2b1efd67abec69123ff0afcb197f8c3",
            "df2316ffa7d764760e6c7f6174d3b15a2d59687834a90474b7446ff323df073d",
            "02a7ccdec62f131efea4bb7c0954d15df2b1efd67abec69123ff0afcb197f8c3"
        ])
    );
    assert_eq!(
        row["metadata_private_element_raw_sha256_values"],
        serde_json::json!([
            "6b95b0cd9835f0ab50173c42a37511a7e8a547af8837f67e0a9bd0d6ff0da1ae",
            "e74d0e44a658ffcdc0ee7266ebd171413b8fcf182c97a27254d9f48abaea6266",
            "3329e2d8d73e62f294fd73110474122239fd4d75a8a2aefbe16c117f0265b328",
            "6374ee55ea117a6d46b516c6ca6f2550d95c849a16221c58bfea5c054b9e6919"
        ])
    );
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_private_creator_ids/DTS_PRIVATE_ALPHA"),
        Some(&Value::from(2))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_private_element_vrs/LO"),
        Some(&Value::from(3))
    );
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Private Creator Block Expectations"));
    assert!(markdown.contains("0011,0010; 0011,0012; 0013,0011"));
}

#[test]
fn validator_rejects_tampered_private_creator_contract() {
    let root = unique_temp_dir("private-creators-tampered");
    let mut manifest = generate_core(&root);
    let blocks = manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("private creator case must be generated")["expected_metadata"]
        ["private_creator_blocks"]
        .as_array_mut()
        .expect("private blocks must be an array");
    blocks[0]["creator_id"] = Value::from("WRONG_CREATOR");
    blocks[0]["block_end_tag"] = Value::from("0011,11FF");
    blocks[1]["raw_value_sha256"] = Value::from("0".repeat(64));
    blocks[0]["elements"][1]["decoded_value"] = Value::from(1);
    blocks[1]["elements"][0]["tag"] = Value::from("0011,1101");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");

    let summary = dicom_test_suite::validate_generated_root(&root)
        .expect("tampered corpus must remain inspectable");
    for failure_key in [
        "metadata_private_creator_contract",
        "metadata_private_block_range",
        "metadata_private_creator_raw_contract",
        "metadata_private_creator_raw_value",
        "metadata_private_element_manifest_value",
        "metadata_private_element_ownership",
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

#[test]
fn schema_and_validator_require_private_metadata_and_block_count() {
    let root = unique_temp_dir("private-creators-required-contract");
    let original = generate_core(&root);
    let schema = read_json("schemas/manifest.schema.json");
    let validator = jsonschema::validator_for(&schema).expect("manifest schema must compile");

    let mut wrong_count = original.clone();
    let file = case_file_mut(&mut wrong_count);
    file["recipe"]["recipe_parameters"]["private_creator_block_count"] = Value::from(2);
    assert!(!validator.is_valid(&wrong_count));
    write_manifest(&root, &wrong_count);
    let summary = dicom_test_suite::validate_generated_root(&root)
        .expect("wrong-count corpus must remain inspectable");
    assert!(
        summary
            .failures
            .iter()
            .any(|failure| failure.contains("metadata_private_creator_block_count")),
        "{:?}",
        summary.failures
    );

    let mut missing_metadata = original;
    case_file_mut(&mut missing_metadata)
        .as_object_mut()
        .expect("file entry must be an object")
        .remove("expected_metadata");
    assert!(!validator.is_valid(&missing_metadata));
    write_manifest(&root, &missing_metadata);
    let summary = dicom_test_suite::validate_generated_root(&root)
        .expect("missing-metadata corpus must remain inspectable");
    assert!(
        summary
            .failures
            .iter()
            .any(|failure| failure.contains("metadata_expected_metadata")),
        "{:?}",
        summary.failures
    );
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
        .expect("private creator case must be generated")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("private creator case must be generated")
}

fn write_manifest(root: &Path, manifest: &Value) {
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(manifest).expect("manifest must serialize"),
    )
    .expect("manifest must be writable");
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
