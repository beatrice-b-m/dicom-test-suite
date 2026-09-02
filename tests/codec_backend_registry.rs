use std::collections::BTreeSet;
use std::fs;

use synth_dicom_gen::recipes::{
    BackendBoundary, CODEC_BACKENDS, CodecDispatchRequest, CodecEvidenceRequirement,
    CodecRegistryError, CodecSourceRequest, TransferSyntaxBackendRegistry,
};
use serde_json::Value;

fn registry() -> Value {
    serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap()
}

fn htj2k_request<'a>(
    features: &'a BTreeSet<String>,
    tools: &'a BTreeSet<String>,
) -> CodecDispatchRequest<'a> {
    CodecDispatchRequest {
        transfer_syntax_uid: "1.2.840.10008.1.2.4.201",
        backend_id: "openjph_htj2k_lossless_command_writer",
        enabled_features: features,
        available_tools: tools,
        source: CodecSourceRequest::NativeFrame {
            samples_per_pixel: 1,
            bits_allocated: 16,
            photometric_interpretation: "MONOCHROME2",
        },
    }
}

#[test]
fn executable_registry_exactly_covers_implemented_dicom_transfer_syntaxes() {
    let registry = registry();
    let implemented = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented" && case["artifact_kind"] == "dicom_instance")
        .map(|case| case["transfer_syntax_uid"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let executable = CODEC_BACKENDS
        .iter()
        .map(|backend| backend.transfer_syntax_uid)
        .collect::<BTreeSet<_>>();
    assert_eq!(executable, implemented);

    let codecs = TransferSyntaxBackendRegistry::load_committed().unwrap();
    for case in registry["cases"].as_array().unwrap().iter().filter(|case| {
        case["status"] == "implemented"
            && case["artifact_kind"] == "dicom_instance"
            && (case["provider"]["kind"] == "rust_native"
                || case["requirements"]["features"]
                    .as_array()
                    .is_some_and(|values| !values.is_empty())
                || case["requirements"]["external_codecs"]
                    .as_array()
                    .is_some_and(|values| !values.is_empty()))
    }) {
        let strings = |name: &str| {
            case["requirements"][name]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        codecs
            .validate_registry_requirements(
                case["transfer_syntax_uid"].as_str().unwrap(),
                case["determinism"].as_str().unwrap(),
                &strings("features"),
                &strings("external_codecs"),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case["case_id"]));
    }
}

#[test]
fn dispatch_is_exact_feature_tool_and_source_shape_checked() {
    let codecs = TransferSyntaxBackendRegistry::load_committed().unwrap();
    let mut features = BTreeSet::from(["htj2k_openjph".to_owned()]);
    let mut tools = BTreeSet::from(["ojph_compress".to_owned()]);
    let backend = codecs.resolve(htj2k_request(&features, &tools)).unwrap();
    assert_eq!(backend.boundary, BackendBoundary::EncodedFrames);
    assert!(
        backend
            .evidence
            .contains(&CodecEvidenceRequirement::ExecutableSha256)
    );
    tools.clear();
    assert!(matches!(
        codecs.resolve(htj2k_request(&features, &tools)),
        Err(CodecRegistryError::MissingTool(_))
    ));
    tools.insert("ojph_compress".into());
    features.clear();
    assert!(matches!(
        codecs.resolve(htj2k_request(&features, &tools)),
        Err(CodecRegistryError::MissingFeature(_))
    ));

    let enabled = BTreeSet::from(["jpeg".to_owned()]);
    assert!(matches!(
        codecs.resolve(CodecDispatchRequest {
            transfer_syntax_uid: "1.2.840.10008.1.2.4.50",
            backend_id: "dicom_rs_jpeg_baseline_writer",
            enabled_features: &enabled,
            available_tools: &BTreeSet::new(),
            source: CodecSourceRequest::NativeFrame {
                samples_per_pixel: 1,
                bits_allocated: 16,
                photometric_interpretation: "MONOCHROME2",
            },
        }),
        Err(CodecRegistryError::UnsupportedSourceShape { .. })
    ));
}

#[test]
fn only_the_locked_dcmtk_boundary_accepts_a_full_part10_source() {
    let full_file = CODEC_BACKENDS
        .iter()
        .filter(|backend| backend.boundary == BackendBoundary::LockedFullFileTransform)
        .collect::<Vec<_>>();
    assert_eq!(
        full_file
            .iter()
            .map(|backend| backend.transfer_syntax_uid)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["1.2.840.10008.1.2.4.57", "1.2.840.10008.1.2.4.70"])
    );
    for backend in full_file {
        assert!(
            backend
                .evidence
                .contains(&CodecEvidenceRequirement::RuntimeVersion)
        );
        assert!(
            backend
                .evidence
                .contains(&CodecEvidenceRequirement::ExecutableSha256)
        );
        assert!(
            backend
                .evidence
                .contains(&CodecEvidenceRequirement::ByteReproducibility)
        );
    }
}

#[test]
fn malformed_or_incomplete_capability_matrices_fail_closed() {
    let incomplete = r#"{
      "entries": [{
        "uid":"1.2.840.10008.1.2", "status":"available",
        "read_dataset":true, "decode_pixel":true,
        "write_dataset":true, "encode_pixel":true,
        "feature_flags":[], "external_libraries":[], "determinism":"byte_stable"
      }]
    }"#;
    assert!(matches!(
        TransferSyntaxBackendRegistry::from_capability_matrix(incomplete),
        Err(CodecRegistryError::MatrixMismatch(_))
    ));

    let duplicate = synth_dicom_gen::recipes::CAPABILITY_MATRIX_JSON.replacen(
        "\"entries\": [",
        "\"entries\": [{\"uid\":\"1.2.840.10008.1.2\",\"status\":\"available\",\"read_dataset\":true,\"decode_pixel\":true,\"write_dataset\":true,\"encode_pixel\":true,\"feature_flags\":[],\"external_libraries\":[],\"determinism\":\"byte_stable\"},",
        1,
    );
    assert!(matches!(
        TransferSyntaxBackendRegistry::from_capability_matrix(&duplicate),
        Err(CodecRegistryError::MatrixMismatch(message)) if message.contains("duplicate")
    ));
}
