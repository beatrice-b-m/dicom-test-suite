use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlan, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
#[cfg(any(feature = "deflate", feature = "charls"))]
use dicom_test_suite::executor::evidence::ResultStatus;
use dicom_test_suite::executor::frame_codec::{
    ExternalFrameCodecCommands, FrameCodecLimits, RegisteredFrameCodecService,
};
use dicom_test_suite::runtime_capabilities::CapabilityInventory;

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
    fn project(
        &self,
        _: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
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
