use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "jpegxl")]
use synth_dicom_gen::codecs::CjxlJpegXlLossyEncoder;
#[cfg(feature = "htj2k_openjph")]
use synth_dicom_gen::codecs::OpenJphHtj2kLosslessEncoder;
#[cfg(feature = "deflate")]
use synth_dicom_gen::corpus_plan::PlannedArtifact;
use synth_dicom_gen::curated_execution::CuratedExecutionServiceFactory;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlan, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
use synth_dicom_gen::executor::adapters::ManifestProjectionInput;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
#[cfg(any(
    feature = "deflate",
    feature = "charls",
    feature = "jpeg",
    feature = "jpeg2000",
    feature = "jpegxl",
    feature = "htj2k_openjph"
))]
use synth_dicom_gen::executor::evidence::ResultStatus;
use synth_dicom_gen::executor::frame_codec::{
    ExternalFrameCodecCommands, FrameCodecLimits, RegisteredFrameCodecService,
};
#[cfg(feature = "deflate")]
use synth_dicom_gen::executor::services::{ByteBinding, SlotExecutionBinding};
use synth_dicom_gen::runtime_capabilities::CapabilityInventory;
#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
use synth_dicom_gen::runtime_capabilities::QualifiedExecutableIdentity;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dts-exceptional-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Projector;

impl ManifestProjector for Projector {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        Ok(b"{}\n".to_vec())
    }
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn plan(case_id: &str, inventory: CapabilityInventory) -> CuratedScCorpusPlan {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .with_capability_inventory(inventory)
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap()
}

#[cfg(any(
    feature = "jpeg",
    feature = "jpeg2000",
    feature = "jpegxl",
    feature = "htj2k_openjph"
))]
fn assert_codec_execution(
    bundle: &CuratedScCorpusPlan,
    case_id: &str,
    backend_id: &str,
    expect_decoded_observation: bool,
    expect_metrics: bool,
    factory: CuratedExecutionServiceFactory,
) {
    let destination = TempRoot::absent(case_id.rsplit('/').next().unwrap());
    let result = CorpusExecutor::new(factory, Projector)
        .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
        .unwrap();
    let evidence = result
        .evidence
        .artifacts
        .iter()
        .find(|artifact| {
            artifact
                .codecs
                .iter()
                .any(|codec| codec.backend_id == backend_id)
        })
        .unwrap_or_else(|| panic!("case {case_id} emitted no {backend_id} evidence"));
    let codec = evidence
        .codecs
        .iter()
        .find(|codec| codec.backend_id == backend_id)
        .unwrap();
    assert_eq!(codec.status, ResultStatus::Passed);
    assert!(!codec.encoded_frame_sha256.is_empty());
    assert_eq!(
        !codec.decoded_frame_sha256.is_empty(),
        expect_decoded_observation,
        "unexpected decoded-frame evidence for {case_id}"
    );
    assert_eq!(
        !codec.metrics.is_empty(),
        expect_metrics,
        "unexpected lossy metrics for {case_id}"
    );
    assert!(evidence.materialization.is_some());
    assert!(
        destination
            .0
            .join(format!("{case_id}/instance.dcm"))
            .is_file()
    );
}

#[cfg(feature = "htj2k_openjph")]
fn openjph_command() -> Option<(PathBuf, QualifiedExecutableIdentity)> {
    let path = [
        PathBuf::from("/opt/homebrew/bin/ojph_compress"),
        PathBuf::from("/usr/local/bin/ojph_compress"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let identity = OpenJphHtj2kLosslessEncoder::with_command(&path)
        .discover_backend_identity()
        .unwrap();
    assert_eq!(identity.version_source, "executable_sha256");
    assert!(identity.version.is_none());
    Some((
        path,
        QualifiedExecutableIdentity {
            version: format!("sha256:{}", identity.executable_sha256),
            executable_sha256: identity.executable_sha256,
        },
    ))
}

#[cfg(feature = "jpegxl")]
fn cjxl_command() -> Option<(PathBuf, QualifiedExecutableIdentity)> {
    let path = [
        PathBuf::from("/opt/homebrew/bin/cjxl"),
        PathBuf::from("/usr/local/bin/cjxl"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let identity = CjxlJpegXlLossyEncoder::with_command(&path)
        .discover_backend_identity()
        .unwrap();
    Some((
        path,
        QualifiedExecutableIdentity {
            version: identity.version.unwrap(),
            executable_sha256: identity.executable_sha256,
        },
    ))
}

#[cfg(feature = "deflate")]
#[test]
fn qualified_deflated_dataset_executes_with_shared_materialization_evidence() {
    let bundle = plan(
        "classic/sc/mono2_u8_deflated_explicit_le",
        CapabilityInventory {
            compiled_features: set(&["deflate"]),
            executable_codec_backends: set(&["dicom_rs_deflated_dataset_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let destination = TempRoot::absent("deflated");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
        .unwrap();
    let artifact = &result.evidence.artifacts[0];
    assert!(artifact.codecs.is_empty());
    assert!(artifact.materialization.is_some());
    assert!(
        artifact
            .validation
            .iter()
            .all(|item| item.status == ResultStatus::Passed)
    );
    assert!(
        destination
            .0
            .join("classic/sc/mono2_u8_deflated_explicit_le/instance.dcm")
            .is_file()
    );
}

#[cfg(feature = "deflate")]
#[test]
fn qualified_deflated_seg_executes_per_frame_with_reference_closure() {
    let bundle = plan(
        "derived/seg/binary_multiframe_deflated_image_frame",
        CapabilityInventory {
            compiled_features: set(&["deflate"]),
            executable_codec_backends: set(&["dicom_rs_deflated_image_frame_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let target = bundle
        .plan
        .artifacts
        .iter()
        .find(|artifact| {
            let binding = match artifact {
                PlannedArtifact::Dicom(artifact) => artifact.case_binding.as_ref(),
                PlannedArtifact::ImportedDicom(artifact) => artifact.case_binding.as_ref(),
                PlannedArtifact::Auxiliary(_)
                | PlannedArtifact::Mutation(_)
                | PlannedArtifact::Qualification(_) => None,
            };
            binding.is_some_and(|binding| {
                binding.case_id == "derived/seg/binary_multiframe_deflated_image_frame"
            })
        })
        .unwrap();
    let SlotExecutionBinding::CodecRequest { request } =
        &bundle.bindings[target.logical_id()].slots["pixels"]
    else {
        panic!("Deflated Image Frame SEG must execute through the frame codec")
    };
    assert_eq!(request.backend_id, "dicom_rs_deflated_image_frame_writer");
    assert_eq!(request.frames.len(), 2);
    for frame in &request.frames {
        let ByteBinding::Inline { bytes, sha256 } = &frame.bytes else {
            panic!("Deflated Image Frame native payloads must be inline")
        };
        assert_eq!(bytes.len(), 1);
        assert_eq!(sha256, &synth_dicom_gen::sha256_hex(bytes));
    }
    assert!(bundle.plan.dependencies.iter().any(|dependency| {
        dependency.artifact_id == target.logical_id()
            && dependency.relationship == "source_image_for_segmentation"
    }));

    let destination = TempRoot::absent("deflated-seg");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
        .unwrap();
    let evidence = result
        .evidence
        .artifacts
        .iter()
        .find(|artifact| artifact.logical_id == target.logical_id())
        .unwrap();
    assert_eq!(evidence.codecs.len(), 1);
    assert_eq!(evidence.codecs[0].decoded_frame_sha256.len(), 2);
    assert!(
        destination
            .0
            .join("derived/seg/binary_multiframe_deflated_image_frame/instance.dcm")
            .is_file()
    );
}

#[cfg(feature = "charls")]
#[test]
fn qualified_lossless_codec_executes_with_typed_codec_evidence() {
    let bundle = plan(
        "classic/sc/mono2_u8_jpeg_ls_lossless",
        CapabilityInventory {
            compiled_features: set(&["charls"]),
            executable_codec_backends: set(&["dicom_rs_charls_jpeg_ls_lossless_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let destination = TempRoot::absent("jpeg-ls");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
        .unwrap();
    let artifact = &result.evidence.artifacts[0];
    assert_eq!(artifact.codecs.len(), 1);
    assert_eq!(
        artifact.codecs[0].backend_id,
        "dicom_rs_charls_jpeg_ls_lossless_writer"
    );
    assert_eq!(artifact.codecs[0].decoded_frame_sha256.len(), 1);
    assert!(artifact.materialization.is_some());
    assert!(
        artifact
            .validation
            .iter()
            .all(|item| item.status == ResultStatus::Passed)
    );
    assert!(
        destination
            .0
            .join("classic/sc/mono2_u8_jpeg_ls_lossless/instance.dcm")
            .is_file()
    );
}

#[cfg(feature = "jpeg")]
#[test]
fn qualified_jpeg_baseline_executes_with_lossy_metrics() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
            ..CapabilityInventory::default()
        },
    );
    assert_codec_execution(
        &bundle,
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        "dicom_rs_jpeg_baseline_writer",
        true,
        true,
        CuratedExecutionServiceFactory::new(&bundle),
    );
}

#[cfg(feature = "jpeg2000")]
#[test]
fn qualified_jpeg2000_lossless_executes_with_decoded_hashes() {
    let bundle = plan(
        "classic/sc/mono2_u16_jpeg2000_lossless",
        CapabilityInventory {
            compiled_features: set(&["jpeg2000"]),
            executable_codec_backends: set(&["project_openjp2_jpeg2000_lossless_writer"]),
            ..CapabilityInventory::default()
        },
    );
    assert_codec_execution(
        &bundle,
        "classic/sc/mono2_u16_jpeg2000_lossless",
        "project_openjp2_jpeg2000_lossless_writer",
        true,
        false,
        CuratedExecutionServiceFactory::new(&bundle),
    );
}

#[cfg(feature = "jpegxl")]
#[test]
fn qualified_jpegxl_lossless_executes_with_decoded_hashes() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpegxl_lossless",
        CapabilityInventory {
            compiled_features: set(&["jpegxl"]),
            executable_codec_backends: set(&["dicom_rs_jpegxl_lossless_writer"]),
            ..CapabilityInventory::default()
        },
    );
    assert_codec_execution(
        &bundle,
        "classic/sc/rgb_planar0_jpegxl_lossless",
        "dicom_rs_jpegxl_lossless_writer",
        true,
        false,
        CuratedExecutionServiceFactory::new(&bundle),
    );
}

#[cfg(feature = "jpegxl")]
#[test]
fn qualified_cjxl_lossy_executes_with_tool_and_metric_evidence() {
    let Some((command, identity)) = cjxl_command() else {
        return;
    };
    let bundle = plan(
        "classic/sc/rgb_jpegxl_lossy",
        CapabilityInventory {
            compiled_features: set(&["jpegxl"]),
            executable_codec_backends: set(&["cjxl_jpegxl_lossy_command_writer"]),
            available_executables: set(&["cjxl"]),
            executable_identities: [("cjxl".into(), identity)].into_iter().collect(),
            external_validators: set(&[
                "jxl-oxide 0.10.2 via dicom-transfer-syntax-registry 0.9.1",
            ]),
            ..CapabilityInventory::default()
        },
    );
    let factory = CuratedExecutionServiceFactory::with_frame_codec_commands(
        &bundle,
        ExternalFrameCodecCommands {
            openjph: None,
            cjxl: Some(command),
        },
    )
    .unwrap();
    assert_codec_execution(
        &bundle,
        "classic/sc/rgb_jpegxl_lossy",
        "cjxl_jpegxl_lossy_command_writer",
        true,
        true,
        factory,
    );
}

#[cfg(feature = "htj2k_openjph")]
fn assert_openjph_case(case_id: &str, backend_id: &str, lossy: bool) {
    let Some((command, identity)) = openjph_command() else {
        return;
    };
    let bundle = plan(
        case_id,
        CapabilityInventory {
            compiled_features: set(&["htj2k_openjph"]),
            executable_codec_backends: set(&[backend_id]),
            available_executables: set(&["ojph_compress"]),
            executable_identities: [("ojph_compress".into(), identity)].into_iter().collect(),
            external_validators: if lossy {
                set(&["OpenJPEG via dicom-transfer-syntax-registry 0.9.1"])
            } else {
                BTreeSet::new()
            },
            ..CapabilityInventory::default()
        },
    );
    let factory = CuratedExecutionServiceFactory::with_frame_codec_commands(
        &bundle,
        ExternalFrameCodecCommands {
            openjph: Some(command),
            cjxl: None,
        },
    )
    .unwrap();
    assert_codec_execution(&bundle, case_id, backend_id, true, lossy, factory);
}

#[cfg(feature = "htj2k_openjph")]
#[test]
fn qualified_openjph_lossless_executes_with_fingerprint_and_decoded_hashes() {
    assert_openjph_case(
        "classic/sc/mono2_u16_htj2k_lossless",
        "openjph_htj2k_lossless_command_writer",
        false,
    );
}

#[cfg(feature = "htj2k_openjph")]
#[test]
fn qualified_openjph_lossy_executes_with_fingerprint_and_metrics() {
    assert_openjph_case(
        "classic/sc/mono2_u16_htj2k_lossy",
        "openjph_htj2k_lossy_command_writer",
        true,
    );
}

#[cfg(feature = "htj2k_openjph")]
#[test]
fn openjph_rejects_fingerprint_or_command_path_drift_before_execution() {
    let Some((command, identity)) = openjph_command() else {
        return;
    };
    let mut wrong_identity = identity.clone();
    wrong_identity.executable_sha256 = "0".repeat(64);
    wrong_identity.version = format!("sha256:{}", wrong_identity.executable_sha256);
    let bundle = plan(
        "classic/sc/mono2_u16_htj2k_lossless",
        CapabilityInventory {
            compiled_features: set(&["htj2k_openjph"]),
            executable_codec_backends: set(&["openjph_htj2k_lossless_command_writer"]),
            available_executables: set(&["ojph_compress"]),
            executable_identities: [("ojph_compress".into(), wrong_identity)]
                .into_iter()
                .collect(),
            ..CapabilityInventory::default()
        },
    );
    let error = match CuratedExecutionServiceFactory::with_frame_codec_commands(
        &bundle,
        ExternalFrameCodecCommands {
            openjph: Some(command),
            cjxl: None,
        },
    ) {
        Ok(_) => panic!("fingerprint drift must fail"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("differs from planning inventory")
    );

    let bundle = plan(
        "classic/sc/mono2_u16_htj2k_lossless",
        CapabilityInventory {
            compiled_features: set(&["htj2k_openjph"]),
            executable_codec_backends: set(&["openjph_htj2k_lossless_command_writer"]),
            available_executables: set(&["ojph_compress"]),
            executable_identities: [("ojph_compress".into(), identity)].into_iter().collect(),
            ..CapabilityInventory::default()
        },
    );
    let error = match CuratedExecutionServiceFactory::with_frame_codec_commands(
        &bundle,
        ExternalFrameCodecCommands {
            openjph: Some(PathBuf::from("/not/present/ojph_compress")),
            cjxl: None,
        },
    ) {
        Ok(_) => panic!("command path drift must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("ojph_compress"));
}

#[cfg(feature = "jpegxl")]
#[test]
fn version_reporting_cjxl_does_not_accept_a_fingerprint_derived_version() {
    let Some((command, mut identity)) = cjxl_command() else {
        return;
    };
    identity.version = format!("sha256:{}", identity.executable_sha256);
    let bundle = plan(
        "classic/sc/rgb_jpegxl_lossy",
        CapabilityInventory {
            compiled_features: set(&["jpegxl"]),
            executable_codec_backends: set(&["cjxl_jpegxl_lossy_command_writer"]),
            available_executables: set(&["cjxl"]),
            executable_identities: [("cjxl".into(), identity)].into_iter().collect(),
            external_validators: set(&[
                "jxl-oxide 0.10.2 via dicom-transfer-syntax-registry 0.9.1",
            ]),
            ..CapabilityInventory::default()
        },
    );
    let error = match CuratedExecutionServiceFactory::with_frame_codec_commands(
        &bundle,
        ExternalFrameCodecCommands {
            openjph: None,
            cjxl: Some(command),
        },
    ) {
        Ok(_) => panic!("reported-version drift must fail"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("differs from planning inventory")
    );
}

#[test]
fn injected_planner_inventory_cannot_enable_an_absent_compiled_codec() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let destination = TempRoot::absent("missing-codec");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new());
    #[cfg(not(feature = "jpeg"))]
    {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("feature jpeg is disabled")
        );
        assert!(!destination.0.exists());
    }
    #[cfg(feature = "jpeg")]
    let _ = result;
}

#[cfg(feature = "jpeg")]
#[test]
fn injected_frame_codec_limits_fail_before_publication() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let service = RegisteredFrameCodecService::new(
        FrameCodecLimits {
            max_frames: 1,
            max_native_frame_bytes: 1024 * 1024,
            max_encoded_frame_bytes: 1,
            max_total_encoded_bytes: 1,
        },
        ExternalFrameCodecCommands::default(),
    )
    .unwrap();
    let destination = TempRoot::absent("bounded");
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::with_frame_codec_service(&bundle, service).unwrap(),
        Projector,
    )
    .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new());
    assert!(result.unwrap_err().to_string().contains("encoded bytes"));
    assert!(!destination.0.exists());
}

#[test]
fn cancellation_stops_exceptional_execution_before_publication() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let destination = TempRoot::absent("cancelled");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 1, &cancellation);
    let error = result.unwrap_err();
    assert!(
        error.to_string().to_ascii_lowercase().contains("cancel"),
        "{error}"
    );
    assert!(!destination.0.exists());
}

#[test]
fn execution_command_inventory_mismatch_fails_before_binding_or_invocation() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let service = RegisteredFrameCodecService::new(
        FrameCodecLimits::default(),
        ExternalFrameCodecCommands {
            openjph: Some(PathBuf::from("/not/invoked/ojph_compress")),
            cjxl: None,
        },
    )
    .unwrap();
    let error = match CuratedExecutionServiceFactory::with_frame_codec_service(&bundle, service) {
        Ok(_) => panic!("an execution command absent from planning must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("not available during planning"));
}

#[test]
fn execution_inventory_does_not_change_the_bundle_wire_contract() {
    let bundle = plan(
        "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
            ..CapabilityInventory::default()
        },
    );
    let encoded = serde_json::to_vec(&bundle).unwrap();
    let encoded_value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert!(encoded_value.get("capability_inventory").is_none());
    let decoded: CuratedScCorpusPlan = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        decoded.plan.canonical_sha256().unwrap(),
        bundle.plan.canonical_sha256().unwrap()
    );
    assert_eq!(decoded.bindings, bundle.bindings);
    assert_eq!(decoded.projection, bundle.projection);
    assert_eq!(decoded.pending, bundle.pending);
}
