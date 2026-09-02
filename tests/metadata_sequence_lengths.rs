use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

const CASE_ID: &str = "metadata/sc/defined_undefined_sequence_lengths";

#[test]
fn sequence_length_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("sequence-length-first");
    let second_root = unique_temp_dir("sequence-length-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);
    let first = case_files(&first_manifest);
    let second = case_files(&second_manifest);
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);

    for (variant, file_name, hash, raw_length, delimiter) in [
        (
            "defined",
            "defined.dcm",
            "8bb4d67a7774cb4276b0b632fb22f2123e709e52d2fbee87e6e30d804f25115a",
            [0x38, 0, 0, 0],
            false,
        ),
        (
            "undefined",
            "undefined.dcm",
            "0c734d4cd5f6419c916bcc95664654c8c7aaba68eaed84f6aa5ae40d3fcc5642",
            [0xFF; 4],
            true,
        ),
    ] {
        let relative = format!("{CASE_ID}/{file_name}");
        let entry = first
            .iter()
            .find(|file| file["path"] == relative)
            .expect("variant manifest entry must exist");
        assert_eq!(entry["sha256"], hash);
        assert_eq!(
            entry["expected_metadata"]["sequence_length_encoding"]["variant_id"],
            variant
        );
        assert_eq!(
            fs::read(first_root.join(&relative)).expect("first fixture must be readable"),
            fs::read(second_root.join(&relative)).expect("second fixture must be readable")
        );

        let bytes = fs::read(first_root.join(&relative)).expect("fixture bytes must be readable");
        let offset = sequence_offset(&bytes);
        assert_eq!(&bytes[offset + 8..offset + 12], &raw_length);
        assert_eq!(
            bytes.get(offset + 12 + 56..offset + 12 + 64)
                == Some(&[0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]),
            delimiter
        );
        let object = open_file(first_root.join(&relative)).expect("fixture must parse");
        let item = &object
            .element(tags::ANATOMIC_REGION_SEQUENCE)
            .expect("Anatomic Region Sequence must exist")
            .items()
            .expect("SQ must decode")[0];
        assert_eq!(
            item.element(tags::CODE_VALUE).unwrap().to_str().unwrap(),
            "69536005"
        );
        assert_eq!(
            item.element(tags::CODING_SCHEME_DESIGNATOR)
                .unwrap()
                .to_str()
                .unwrap(),
            "SCT"
        );
        assert_eq!(
            item.element(tags::CODE_MEANING).unwrap().to_str().unwrap(),
            "Head"
        );
        assert!(
            entry["validation"]["internal"]
                .as_array()
                .unwrap()
                .iter()
                .any(|result| result["name"] == "sequence_length_encoding_round_trip")
        );
    }

    assert!(
        jsonschema::validator_for(&read_json("schemas/manifest.schema.json"))
            .unwrap()
            .is_valid(&first_manifest)
    );
    let summary = synth_dicom_gen::validate_generated_root(&first_root).unwrap();
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);

    let report = report_json(&first_root);
    assert!(
        jsonschema::validator_for(&read_json("schemas/coverage-report.schema.json"))
            .unwrap()
            .is_valid(&report)
    );
    let rows = report["coverage_matrix"].as_array().unwrap();
    let sequence_rows = rows
        .iter()
        .filter(|row| row["case_id"] == CASE_ID)
        .collect::<Vec<_>>();
    assert_eq!(sequence_rows.len(), 2);
    assert_eq!(
        sequence_rows[0]["metadata_sequence_length_variant"],
        "defined"
    );
    assert_eq!(sequence_rows[0]["metadata_sequence_value_length"], 56);
    assert_eq!(
        sequence_rows[0]["metadata_sequence_length_field_hex"],
        "38000000"
    );
    assert_eq!(
        sequence_rows[1]["metadata_sequence_length_variant"],
        "undefined"
    );
    assert!(sequence_rows[1]["metadata_sequence_value_length"].is_null());
    assert_eq!(
        sequence_rows[1]["metadata_sequence_length_field_hex"],
        "FFFFFFFF"
    );
    assert_eq!(
        report.pointer("/grouped_coverage/metadata_sequence_item_length_encodings/undefined"),
        Some(&Value::from(2))
    );
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Sequence Length Encoding Expectations"));
    assert!(markdown.contains("69536005\\|SCT\\|Head"));
}

#[test]
fn validator_rejects_tampered_sequence_length_contract() {
    let root = unique_temp_dir("sequence-length-tampered");
    let mut manifest = generate_extended(&root);
    let files = manifest["files"].as_array_mut().unwrap();
    let mut entries = files
        .iter_mut()
        .filter(|file| file["case_id"] == CASE_ID)
        .collect::<Vec<_>>();
    entries[0]["expected_metadata"]["sequence_length_encoding"]["sequence_value_length"] =
        Value::from(55);
    entries[0]["expected_metadata"]["sequence_length_encoding"]["sequence_delimitation_present"] =
        Value::from(true);
    entries[0]["expected_metadata"]["sequence_length_encoding"]["decoded_items"][0]["code_meaning"] =
        Value::from("Wrong");
    entries[1]["expected_metadata"]["sequence_length_encoding"]["variant_id"] =
        Value::from("defined");
    write_manifest(&root, &manifest);

    let summary = synth_dicom_gen::validate_generated_root(&root).unwrap();
    for key in [
        "metadata_sequence_length_manifest_contract",
        "metadata_sequence_length_variant",
        "metadata_sequence_length_variant_set",
    ] {
        assert!(
            summary.failures.iter().any(|failure| failure.contains(key)),
            "missing {key}: {:?}",
            summary.failures
        );
    }
}

fn sequence_offset(bytes: &[u8]) -> usize {
    bytes
        .windows(6)
        .position(|window| window == [0x08, 0x00, 0x18, 0x22, b'S', b'Q'])
        .expect("SQ header must exist")
}

fn case_files(manifest: &Value) -> Vec<&Value> {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|file| file["case_id"] == CASE_ID)
        .collect()
}

fn generate_extended(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "extended", "--out"])
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
    assert!(output.status.success());
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
