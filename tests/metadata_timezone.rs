use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/timezone_boundaries";

#[test]
fn timezone_boundaries_are_deterministic_strict_and_reported() {
    let first_root = unique_temp_dir("timezone-first");
    let second_root = unique_temp_dir("timezone-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);
    let first_files = case_files(&first_manifest);
    let second_files = case_files(&second_manifest);
    assert_eq!(first_files.len(), 2);
    assert_eq!(second_files.len(), 2);

    for (first, second) in first_files.iter().zip(second_files.iter()) {
        assert_eq!(first["path"], second["path"]);
        assert_eq!(first["expected_metadata"], second["expected_metadata"]);
        let relative_path = first["path"].as_str().expect("path must be a string");
        assert_eq!(
            fs::read(first_root.join(relative_path)).expect("first file must be readable"),
            fs::read(second_root.join(relative_path)).expect("second file must be readable"),
            "timezone fixture {relative_path} must be byte-stable"
        );
    }

    let boundary_ids = first_files
        .iter()
        .map(|file| {
            file["expected_metadata"]["temporal"]["boundary_id"]
                .as_str()
                .expect("boundary ID must be a string")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        boundary_ids,
        BTreeSet::from(["negative_min", "positive_max"])
    );
    assert_ne!(
        first_files[0]["uids"]["study_instance_uid"],
        first_files[1]["uids"]["study_instance_uid"]
    );
    assert_ne!(
        first_files[0]["uids"]["series_instance_uid"],
        first_files[1]["uids"]["series_instance_uid"]
    );
    assert_ne!(
        first_files[0]["uids"]["sop_instance_uid"],
        first_files[1]["uids"]["sop_instance_uid"]
    );

    assert_boundary(
        &first_root,
        first_files
            .iter()
            .find(|file| file["expected_metadata"]["temporal"]["boundary_id"] == "positive_max")
            .expect("positive boundary must exist"),
        "20240229",
        "235959.999999",
        "20240229235959.999999+1400",
        "+1400",
        "2024-02-29T09:59:59.999999Z",
    );
    assert_boundary(
        &first_root,
        first_files
            .iter()
            .find(|file| file["expected_metadata"]["temporal"]["boundary_id"] == "negative_min")
            .expect("negative boundary must exist"),
        "20240301",
        "000000.000000",
        "20240301000000.000000-1200",
        "-1200",
        "2024-03-01T12:00:00.000000Z",
    );

    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);
    let summary = synth_dicom_gen::validate_generated_root(&first_root)
        .expect("timezone corpus must be inspectable");
    assert!(
        summary.failures.is_empty(),
        "strict failures: {:?}",
        summary.failures
    );

    let report = report_json(&first_root);
    let report_schema = read_json("schemas/coverage-report.schema.json");
    let report_validator =
        jsonschema::validator_for(&report_schema).expect("report schema must compile");
    assert!(report_validator.is_valid(&report));
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_temporal_boundary_ids/positive_max"),
        Some(&Value::from(1))
    );
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_timezone_offsets_from_utc/+1400"),
        Some(&Value::from(1))
    );
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Temporal Metadata Expectations"));
    assert!(markdown.contains("20240229235959.999999+1400"));
    assert!(markdown.contains("2024-03-01T12:00:00.000000Z"));
}

#[test]
fn validator_rejects_tampered_timezone_contract_and_boundary_set() {
    let root = unique_temp_dir("timezone-tampered");
    let mut manifest = generate_core(&root);
    let mut files = manifest["files"]
        .as_array_mut()
        .expect("manifest files must be an array")
        .iter_mut()
        .filter(|file| file["case_id"] == CASE_ID)
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 2);

    let positive = files
        .iter()
        .position(|file| file["expected_metadata"]["temporal"]["boundary_id"] == "positive_max")
        .expect("positive boundary must exist");
    files[positive]["expected_metadata"]["temporal"]["timezone_offset_from_utc"]["offset_minutes"] =
        Value::from(0);
    files[positive]["expected_metadata"]["temporal"]["timezone_offset_from_utc"]["raw_value_hex"] =
        Value::from("00".repeat(6));
    files[positive]["expected_metadata"]["temporal"]["timezone_offset_from_utc"]["raw_value_sha256"] =
        Value::from("0".repeat(64));
    files[positive]["expected_metadata"]["temporal"]["combined_da_tm_utc"] =
        Value::from("2000-01-01T00:00:00.000000Z");

    let negative = 1 - positive;
    files[negative]["expected_metadata"]["temporal"]["boundary_id"] = Value::from("positive_max");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest must serialize"),
    )
    .expect("tampered manifest must be writable");

    let summary = synth_dicom_gen::validate_generated_root(&root)
        .expect("tampered corpus must remain inspectable");
    for failure_key in [
        "metadata_temporal_timezone_offset_minutes",
        "metadata_temporal_raw_hex",
        "metadata_temporal_raw_hash",
        "metadata_temporal_combined_utc",
        "metadata_temporal_boundary_set",
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

fn assert_boundary(
    root: &Path,
    file: &Value,
    date: &str,
    time: &str,
    date_time: &str,
    offset: &str,
    normalized_utc: &str,
) {
    let temporal = &file["expected_metadata"]["temporal"];
    assert_eq!(temporal["date_values"][0]["decoded_value"], date);
    assert_eq!(temporal["time_values"][0]["decoded_value"], time);
    assert_eq!(temporal["date_time_values"][0]["decoded_value"], date_time);
    assert_eq!(
        temporal["timezone_offset_from_utc"]["decoded_value"],
        offset
    );
    assert_eq!(temporal["combined_da_tm_utc"], normalized_utc);
    assert_eq!(
        temporal["date_time_values"][0]["normalized_utc"],
        normalized_utc
    );

    let object = open_file(root.join(file["path"].as_str().expect("path must be a string")))
        .expect("timezone fixture must parse");
    for (tag, expected) in [
        (tags::STUDY_DATE, date),
        (tags::STUDY_TIME, time),
        (tags::ACQUISITION_DATE_TIME, date_time),
        (tags::TIMEZONE_OFFSET_FROM_UTC, offset),
    ] {
        assert_eq!(
            object
                .element(tag)
                .expect("temporal element must exist")
                .to_str()
                .expect("temporal element must decode")
                .trim_end_matches([' ', '\0']),
            expected
        );
    }
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
    read_json(out_dir.join("manifest.json"))
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

fn case_files(manifest: &Value) -> Vec<&Value> {
    manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .filter(|file| file["case_id"] == CASE_ID)
        .collect()
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
