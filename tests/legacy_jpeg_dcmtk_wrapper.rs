#![cfg(feature = "legacy_jpeg_dcmtk")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::value::Value as DicomValue;
use dicom_dictionary_std::{tags, uids};
use dicom_encoding::{Codec, adapters::PixelDataReader};
use dicom_object::open_file;
use synth_dicom_gen::codecs::{
    CodecBackendKind, CodecDeterminism, DcmtkDcmcjpegLosslessProcess,
    DcmtkDcmcjpegLosslessSv1Encoder, JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID,
    JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID,
};
use dicom_transfer_syntax_registry::entries::{
    JPEG_LOSSLESS_NON_HIERARCHICAL, JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION,
};
use serde_json::Value;

const SOURCE_CASE_ID: &str = "classic/sc/mono2_u16_explicit_le";

#[test]
fn dcmtk_wrapper_encodes_lossless_sv1_and_reports_runtime_identity() {
    let encoder = DcmtkDcmcjpegLosslessSv1Encoder::new();
    let backend = encoder.backend();
    assert_eq!(
        backend.backend_id,
        DcmtkDcmcjpegLosslessSv1Encoder::BACKEND_ID
    );
    assert_eq!(backend.backend_kind, CodecBackendKind::ExternalCommand);
    assert_eq!(
        backend.transfer_syntax_uid,
        JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID
    );
    assert_eq!(backend.feature_gate, Some("legacy_jpeg_dcmtk"));
    assert_eq!(backend.determinism, CodecDeterminism::SemanticStable);

    let out_dir = unique_temp_dir("legacy-jpeg-dcmtk-wrapper");
    let source_path = generate_source(&out_dir);
    let compressed_path = out_dir.join("legacy-jpeg-sv1.dcm");

    let encoded = match encoder.encode_file(&source_path, &compressed_path) {
        Ok(encoded) => encoded,
        Err(err)
            if matches!(
                err,
                synth_dicom_gen::codecs::CodecError::Unavailable { .. }
            ) =>
        {
            eprintln!("skipping DCMTK wrapper test because dcmcjpeg is unavailable: {err}");
            return;
        }
        Err(err) => panic!("DCMTK wrapper should encode the source: {err}"),
    };

    assert_eq!(encoded.backend_identity.command, "dcmcjpeg");
    assert!(
        encoded.backend_identity.executable_path.is_absolute(),
        "runtime identity should record a canonical executable path"
    );
    assert_eq!(
        encoded.backend_identity.executable_sha256.len(),
        64,
        "runtime identity should include an executable SHA-256 fingerprint"
    );
    assert_eq!(
        encoded.backend_identity.version_source, "command_stdout",
        "dcmcjpeg exposes version identity through --version stdout"
    );
    assert!(
        encoded
            .backend_identity
            .version
            .as_deref()
            .is_some_and(|version| version.contains("dcmcjpeg")),
        "runtime identity should include the dcmcjpeg version banner"
    );
    assert_eq!(
        encoded.output_bytes,
        fs::read(&compressed_path).expect("compressed output should be readable")
    );

    let source = open_file(&source_path).expect("source DICOM should parse");
    let compressed = open_file(&compressed_path).expect("compressed DICOM should parse");
    assert_eq!(
        compressed.meta().transfer_syntax().trim_end_matches('\0'),
        JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID
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
        "--uid-never must preserve the source SOP Instance UID"
    );
    assert_eq!(
        compressed
            .element(tags::SYNTHETIC_DATA)
            .expect("compressed output should retain Synthetic Data")
            .to_str()
            .expect("Synthetic Data should be textual")
            .trim(),
        "YES"
    );

    let pixel_data = compressed
        .element(tags::PIXEL_DATA)
        .expect("compressed DICOM should contain Pixel Data");
    let DicomValue::PixelSequence(sequence) = pixel_data.value() else {
        panic!("legacy JPEG output must use encapsulated Pixel Data");
    };
    assert_eq!(sequence.offset_table(), &[0]);
    assert_eq!(sequence.fragments().len(), 1);
    assert!(sequence.fragments()[0].starts_with(&[0xff, 0xd8]));
    assert!(sequence.fragments()[0].ends_with(&[0xff, 0xd9]));

    let Codec::EncapsulatedPixelData(Some(reader), writer) =
        JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.codec()
    else {
        panic!("DICOM-rs should expose a legacy JPEG Lossless SV1 reader")
    };
    assert!(
        writer.is_none(),
        "DICOM-rs legacy JPEG SV1 should remain decode-only"
    );
    let mut decoded = Vec::new();
    reader
        .decode_frame(&compressed, 0, &mut decoded)
        .expect("DICOM-rs should decode the wrapper output");
    let source_pixels = source
        .element(tags::PIXEL_DATA)
        .expect("source DICOM should contain native Pixel Data")
        .to_bytes()
        .expect("source Pixel Data should be readable");
    assert_eq!(
        synth_dicom_gen::sha256_hex(&decoded),
        synth_dicom_gen::sha256_hex(&source_pixels)
    );
}

#[test]
fn dcmtk_wrapper_encodes_lossless_process_14_and_reports_runtime_identity() {
    let encoder = DcmtkDcmcjpegLosslessSv1Encoder::new();
    let backend = encoder.backend_for(DcmtkDcmcjpegLosslessProcess::Process14);
    assert_eq!(
        backend.backend_id,
        "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer"
    );
    assert_eq!(backend.backend_kind, CodecBackendKind::ExternalCommand);
    assert_eq!(
        backend.transfer_syntax_uid,
        JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID
    );
    assert_eq!(backend.feature_gate, Some("legacy_jpeg_dcmtk"));
    assert_eq!(backend.determinism, CodecDeterminism::SemanticStable);

    let out_dir = unique_temp_dir("legacy-jpeg-process-14-dcmtk-wrapper");
    let source_path = generate_source(&out_dir);
    let compressed_path = out_dir.join("legacy-jpeg-process-14.dcm");

    let encoded = match encoder.encode_file_with_process(
        DcmtkDcmcjpegLosslessProcess::Process14,
        &source_path,
        &compressed_path,
    ) {
        Ok(encoded) => encoded,
        Err(err)
            if matches!(
                err,
                synth_dicom_gen::codecs::CodecError::Unavailable { .. }
            ) =>
        {
            eprintln!("skipping DCMTK wrapper test because dcmcjpeg is unavailable: {err}");
            return;
        }
        Err(err) => panic!("DCMTK wrapper should encode the source: {err}"),
    };

    assert_eq!(encoded.backend_identity.command, "dcmcjpeg");
    assert!(
        encoded.backend_identity.executable_path.is_absolute(),
        "runtime identity should record a canonical executable path"
    );
    assert_eq!(
        encoded.backend_identity.executable_sha256.len(),
        64,
        "runtime identity should include an executable SHA-256 fingerprint"
    );
    assert_eq!(
        encoded.output_bytes,
        fs::read(&compressed_path).expect("compressed output should be readable")
    );

    let source = open_file(&source_path).expect("source DICOM should parse");
    let compressed = open_file(&compressed_path).expect("compressed DICOM should parse");
    assert_eq!(
        compressed.meta().transfer_syntax().trim_end_matches('\0'),
        JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID
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
            .trim_end_matches('\0')
    );
    assert_eq!(
        compressed
            .element(tags::SYNTHETIC_DATA)
            .expect("compressed object should keep Synthetic Data")
            .to_str()
            .expect("Synthetic Data should be text")
            .trim_end_matches('\0'),
        "YES"
    );

    let pixel_data = compressed
        .element(tags::PIXEL_DATA)
        .expect("compressed object should contain Pixel Data");
    let DicomValue::PixelSequence(sequence) = pixel_data.value() else {
        panic!("compressed Pixel Data should be encapsulated");
    };
    assert_eq!(sequence.offset_table().len(), 1);
    assert_eq!(sequence.fragments().len(), 1);
    assert_eq!(&sequence.fragments()[0][0..2], &[0xff, 0xd8]);
    let fragment_len = sequence.fragments()[0].len();
    assert_eq!(
        &sequence.fragments()[0][fragment_len - 2..fragment_len],
        &[0xff, 0xd9]
    );

    let Codec::EncapsulatedPixelData(Some(reader), _) = JPEG_LOSSLESS_NON_HIERARCHICAL.codec()
    else {
        panic!("DICOM-rs should expose a legacy JPEG Lossless Process 14 reader");
    };
    let mut decoded = Vec::new();
    reader
        .decode_frame(&compressed, 0, &mut decoded)
        .expect("DICOM-rs should decode DCMTK Process 14 output");
    let source_bytes = source
        .element(tags::PIXEL_DATA)
        .expect("source should contain Pixel Data")
        .value()
        .to_bytes()
        .expect("source Pixel Data should be bytes");
    assert_eq!(decoded, source_bytes.as_ref());

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

fn generate_source(out_dir: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
