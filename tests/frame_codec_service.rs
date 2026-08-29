use std::collections::BTreeMap;

#[cfg(feature = "deflate")]
use dicom_test_suite::codecs::DicomRsDeflatedImageFrameEncoder;
#[cfg(feature = "jpeg")]
use dicom_test_suite::codecs::DicomRsJpegBaselineEncoder;
use dicom_test_suite::codecs::NativeRleLosslessEncoder;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::frame_codec::{
    ExternalFrameCodecCommands, FrameCodecLimits, RegisteredFrameCodecService,
};
use dicom_test_suite::executor::services::{
    ByteBinding, CodecRequest, NativeFrameBinding, StagedAssetRegistry,
};
use dicom_test_suite::sha256_hex;
use serde_json::Value;

fn request(
    backend_id: &str,
    transfer_syntax_uid: &str,
    native: Vec<u8>,
    rows: u32,
    columns: u32,
    samples_per_pixel: u16,
    bits_allocated: u16,
    photometric: &str,
) -> CodecRequest {
    CodecRequest {
        request_id: "codec:fixture:pixels".into(),
        artifact_id: "fixture".into(),
        slot: "pixels".into(),
        backend_id: backend_id.into(),
        source_transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        target_transfer_syntax_uid: transfer_syntax_uid.into(),
        frames: vec![NativeFrameBinding {
            frame_number: 1,
            bytes: ByteBinding::Inline {
                sha256: sha256_hex(&native),
                bytes: native,
            },
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            photometric_interpretation: photometric.into(),
        }],
        parameters: BTreeMap::from([("bits_stored".into(), Value::from(bits_allocated))]),
    }
}

fn resolve(
    binding: &ByteBinding,
) -> Result<Vec<u8>, dicom_test_suite::executor::engine::ServiceInvocationError> {
    let ByteBinding::Inline { bytes, .. } = binding else {
        panic!("fixture uses inline bytes")
    };
    Ok(bytes.clone())
}

#[test]
fn native_rle_runs_through_the_registry_with_bounded_typed_evidence() {
    let service = RegisteredFrameCodecService::default();
    let request = request(
        NativeRleLosslessEncoder::BACKEND_ID,
        "1.2.840.10008.1.2.5",
        vec![0, 1, 2, 3],
        2,
        2,
        1,
        8,
        "MONOCHROME2",
    );
    let outcome = service
        .encode(&request, &CancellationToken::new(), resolve)
        .unwrap();
    assert_eq!(
        outcome.result.backend.backend_id,
        NativeRleLosslessEncoder::BACKEND_ID
    );
    assert_eq!(outcome.result.frames.len(), 1);
    assert_eq!(outcome.result.evidence.len(), 1);
    outcome
        .result
        .validate(&request, &StagedAssetRegistry::default())
        .unwrap();
    assert_eq!(outcome.claims["source_boundary"], "verified_native_frames");
    assert_eq!(outcome.decoded_frame_sha256[&1], sha256_hex(&[0, 1, 2, 3]));
}

#[test]
fn unavailable_feature_and_locked_full_file_backends_fail_closed() {
    let service = RegisteredFrameCodecService::default();
    #[cfg(not(feature = "jpeg"))]
    {
        let jpeg = request(
            "dicom_rs_jpeg_baseline_writer",
            "1.2.840.10008.1.2.4.50",
            vec![0; 12],
            2,
            2,
            3,
            8,
            "RGB",
        );
        assert!(
            service
                .encode(&jpeg, &CancellationToken::new(), resolve)
                .unwrap_err()
                .to_string()
                .contains("required feature jpeg is disabled")
        );
    }

    let dcmtk = request(
        "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
        "1.2.840.10008.1.2.4.70",
        vec![0; 8],
        2,
        2,
        1,
        16,
        "MONOCHROME2",
    );
    assert!(
        service
            .encode(&dcmtk, &CancellationToken::new(), resolve)
            .unwrap_err()
            .to_string()
            .contains("locked full-file boundary")
    );
}

#[test]
fn output_limits_and_cancellation_stop_before_publication() {
    let service = RegisteredFrameCodecService::new(
        FrameCodecLimits {
            max_frames: 1,
            max_native_frame_bytes: 4,
            max_encoded_frame_bytes: 1,
            max_total_encoded_bytes: 1,
        },
        ExternalFrameCodecCommands::default(),
    )
    .unwrap();
    let request = request(
        NativeRleLosslessEncoder::BACKEND_ID,
        "1.2.840.10008.1.2.5",
        vec![0, 1, 2, 3],
        2,
        2,
        1,
        8,
        "MONOCHROME2",
    );
    assert!(
        service
            .encode(&request, &CancellationToken::new(), resolve)
            .unwrap_err()
            .to_string()
            .contains("encoded bytes")
    );

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(
        RegisteredFrameCodecService::default()
            .encode(&request, &cancellation, resolve)
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
}

#[cfg(feature = "htj2k_openjph")]
#[test]
fn external_openjph_requires_an_explicit_command_before_execution() {
    let request = request(
        "openjph_htj2k_lossless_command_writer",
        "1.2.840.10008.1.2.4.201",
        vec![0; 8],
        2,
        2,
        1,
        16,
        "MONOCHROME2",
    );
    assert!(
        RegisteredFrameCodecService::default()
            .encode(&request, &CancellationToken::new(), resolve)
            .unwrap_err()
            .to_string()
            .contains("required external tool ojph_compress is unavailable")
    );
}

#[test]
fn executor_adapter_contains_no_iod_or_full_file_bridge() {
    let source = std::fs::read_to_string("src/executor/frame_codec.rs").unwrap();
    for forbidden in [
        "generator::",
        "InMemDicomObject",
        "resolved_plan_from_curated_dataset",
        "open_file(",
        "write_to_file(",
    ] {
        assert!(!source.contains(forbidden), "forbidden bridge {forbidden}");
    }
}

#[cfg(feature = "jpeg")]
#[test]
fn jpeg_baseline_feature_service_emits_lossy_metrics() {
    let request = request(
        DicomRsJpegBaselineEncoder::BACKEND_ID,
        "1.2.840.10008.1.2.4.50",
        vec![0, 20, 40, 60, 80, 100, 120, 140, 160, 180, 200, 220],
        2,
        2,
        3,
        8,
        "RGB",
    );
    let outcome = RegisteredFrameCodecService::default()
        .encode(&request, &CancellationToken::new(), resolve)
        .unwrap();
    assert_eq!(outcome.feature_gate.as_deref(), Some("jpeg"));
    assert_eq!(outcome.determinism, "semantic_stable");
    assert!(outcome.metrics.contains_key("frame_1_overall_rmse"));
}

#[cfg(feature = "deflate")]
#[test]
fn deflated_image_frame_feature_service_round_trips_exactly() {
    let native = vec![0, 1, 2, 3];
    let request = request(
        DicomRsDeflatedImageFrameEncoder::BACKEND_ID,
        "1.2.840.10008.1.2.8.1",
        native.clone(),
        2,
        2,
        1,
        8,
        "MONOCHROME2",
    );
    let outcome = RegisteredFrameCodecService::default()
        .encode(&request, &CancellationToken::new(), resolve)
        .unwrap();
    assert_eq!(outcome.decoded_frame_sha256[&1], sha256_hex(&native));
}
