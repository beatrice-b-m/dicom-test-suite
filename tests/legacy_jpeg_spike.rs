#![cfg(feature = "jpeg")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::value::Value as DicomValue;
use dicom_dictionary_std::{tags, uids};
use dicom_encoding::{Codec, adapters::PixelDataReader};
use dicom_object::open_file;
use dicom_transfer_syntax_registry::entries::JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION;
use serde_json::Value;

const JPEG_LOSSLESS_SV1_UID: &str = "1.2.840.10008.1.2.4.70";
const SOURCE_CASE_ID: &str = "classic/sc/mono2_u16_explicit_le";

#[test]
fn dcmtk_dcmcjpeg_lossless_sv1_spike_preserves_metadata_and_pixels() {
    let dcmcjpeg = match dcmcjpeg_path() {
        Some(path) => path,
        None => {
            eprintln!("skipping DCMTK legacy JPEG spike because dcmcjpeg is not on PATH");
            return;
        }
    };
    let out_dir = unique_temp_dir("legacy-jpeg-sv1-spike");
    let source_path = generate_source(&out_dir);
    let compressed_a = out_dir.join("legacy-jpeg-sv1-a.dcm");
    let compressed_b = out_dir.join("legacy-jpeg-sv1-b.dcm");

    run_dcmcjpeg(&dcmcjpeg, &source_path, &compressed_a);
    run_dcmcjpeg(&dcmcjpeg, &source_path, &compressed_b);

    let source = open_file(&source_path).expect("source DICOM should parse");
    let compressed = open_file(&compressed_a).expect("compressed DICOM should parse");
    let repeated = fs::read(&compressed_b).expect("repeated compressed file should be readable");
    let compressed_bytes = fs::read(&compressed_a).expect("compressed file should be readable");

    assert_eq!(
        compressed.meta().transfer_syntax().trim_end_matches('\0'),
        JPEG_LOSSLESS_SV1_UID,
        "dcmcjpeg must write JPEG Lossless SV1 into File Meta Information"
    );
    assert_eq!(
        compressed
            .meta()
            .media_storage_sop_class_uid()
            .trim_end_matches('\0'),
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE
    );
    assert_eq!(
        compressed
            .meta()
            .media_storage_sop_instance_uid()
            .trim_end_matches('\0'),
        source
            .meta()
            .media_storage_sop_instance_uid()
            .trim_end_matches('\0'),
        "--uid-never must preserve the file meta SOP Instance UID"
    );
    assert_eq!(
        compressed
            .element(tags::SOP_INSTANCE_UID)
            .expect("compressed dataset should contain SOP Instance UID")
            .to_str()
            .expect("SOP Instance UID should be textual")
            .trim_end_matches('\0'),
        source
            .element(tags::SOP_INSTANCE_UID)
            .expect("source dataset should contain SOP Instance UID")
            .to_str()
            .expect("SOP Instance UID should be textual")
            .trim_end_matches('\0'),
        "--uid-never must preserve the dataset SOP Instance UID"
    );
    assert_eq!(
        compressed
            .element(tags::SYNTHETIC_DATA)
            .expect("compressed dataset should retain Synthetic Data")
            .to_str()
            .expect("Synthetic Data should be textual")
            .trim(),
        "YES"
    );
    assert_eq!(
        compressed
            .element(tags::PATIENT_ID)
            .expect("compressed dataset should retain Patient ID")
            .to_str()
            .expect("Patient ID should be textual")
            .trim(),
        "DICOMTEST-SMOKE-001"
    );

    let pixel_data = compressed
        .element(tags::PIXEL_DATA)
        .expect("compressed DICOM should contain Pixel Data");
    let DicomValue::PixelSequence(sequence) = pixel_data.value() else {
        panic!("legacy JPEG output must use encapsulated Pixel Data");
    };
    assert_eq!(
        sequence.offset_table(),
        &[0],
        "--offset-table-create should produce one Basic Offset Table entry for one frame"
    );
    assert_eq!(
        sequence.fragments().len(),
        1,
        "--fragment-per-frame should produce one fragment for one source frame"
    );
    assert!(
        sequence.fragments()[0].starts_with(&[0xff, 0xd8]),
        "legacy JPEG fragment must start with the JPEG SOI marker"
    );
    assert!(
        sequence.fragments()[0].ends_with(&[0xff, 0xd9]),
        "legacy JPEG fragment must end with the JPEG EOI marker"
    );

    let Codec::EncapsulatedPixelData(Some(reader), writer) =
        JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.codec()
    else {
        panic!("DICOM-rs should expose a legacy JPEG Lossless SV1 reader under the jpeg feature")
    };
    assert!(
        writer.is_none(),
        "DICOM-rs legacy JPEG SV1 remains decode-only from this project"
    );
    let mut decoded = Vec::new();
    reader
        .decode_frame(&compressed, 0, &mut decoded)
        .expect("DICOM-rs should decode the dcmcjpeg SV1 output");
    let source_pixels = source
        .element(tags::PIXEL_DATA)
        .expect("source DICOM should contain native Pixel Data")
        .to_bytes()
        .expect("source Pixel Data should be readable");
    assert_eq!(
        dicom_test_suite::sha256_hex(&decoded),
        dicom_test_suite::sha256_hex(&source_pixels),
        "legacy JPEG SV1 decoded bytes must match the deterministic source frame"
    );
    assert_eq!(
        dicom_test_suite::sha256_hex(&compressed_bytes),
        dicom_test_suite::sha256_hex(&repeated),
        "dcmcjpeg should produce byte-identical files for fixed source and options"
    );
}

fn generate_source(out_dir: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "core",
            "--out",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--seed",
            "1",
        ])
        .output()
        .expect("generate command should run");
    assert!(
        output.status.success(),
        "generate should succeed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest_path = out_dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("manifest should be readable"),
    )
    .expect("manifest should parse");
    let source = manifest
        .pointer("/files")
        .and_then(Value::as_array)
        .expect("manifest files should be an array")
        .iter()
        .find(|file| file.get("case_id").and_then(Value::as_str) == Some(SOURCE_CASE_ID))
        .expect("core generation should include the u16 MONOCHROME2 source");
    out_dir.join(
        source
            .get("path")
            .and_then(Value::as_str)
            .expect("source file should have a relative path"),
    )
}

fn run_dcmcjpeg(command: &Path, input: &Path, output: &Path) {
    let result = Command::new(command)
        .args([
            "--encode-lossless-sv1",
            "--true-lossless",
            "--fragment-per-frame",
            "--offset-table-create",
            "--uid-never",
        ])
        .arg(input)
        .arg(output)
        .output()
        .expect("dcmcjpeg command should start");
    assert!(
        result.status.success(),
        "dcmcjpeg should encode the source: stdout={}, stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn dcmcjpeg_path() -> Option<PathBuf> {
    let output = Command::new("dcmcjpeg").arg("--version").output().ok()?;
    if output.status.success() {
        Some(PathBuf::from("dcmcjpeg"))
    } else {
        None
    }
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{name}-{}-{nonce}",
        std::process::id()
    ))
}
