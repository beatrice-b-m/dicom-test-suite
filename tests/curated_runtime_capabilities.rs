use std::collections::BTreeSet;

use dicom_test_suite::corpus_plan::{CapabilityKind, PlannedArtifact};
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::services::SlotExecutionBinding;
use dicom_test_suite::runtime_capabilities::CapabilityInventory;

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn provider(inventory: CapabilityInventory) -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .with_capability_inventory(inventory)
}

fn request(case_id: &str) -> CuratedScPlanRequest {
    CuratedScPlanRequest {
        selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
        seed: 1,
        max_parallelism: 1,
    }
}

#[test]
fn no_default_inventory_preserves_typed_feature_unavailability() {
    let bundle = provider(CapabilityInventory::default())
        .plan(&request("classic/sc/rgb_planar0_jpeg_baseline_8bit"))
        .unwrap();
    assert!(bundle.plan.artifacts.is_empty());
    assert_eq!(bundle.pending.len(), 1);
    assert_eq!(
        bundle.pending[0].reason_code,
        "feature_gated_case_unavailable"
    );
    assert_eq!(
        bundle.pending[0].message,
        "case requires unavailable build/runtime capabilities: features=jpeg"
    );
    assert!(bundle.plan.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::Feature
            && item.reason_code == "feature_disabled"
            && item.message.starts_with("jpeg:")
    }));
    assert!(bundle.plan.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::Codec && item.reason_code == "codec_backend_unavailable"
    }));
    bundle.plan.validate().unwrap();
}

#[test]
fn enabled_feature_and_backend_still_require_injected_command() {
    let bundle = provider(CapabilityInventory {
        compiled_features: set(&["jpegxl"]),
        executable_codec_backends: set(&["cjxl_jpegxl_lossy_command_writer"]),
        external_validators: set(&["jxl-oxide 0.10.2 via dicom-transfer-syntax-registry 0.9.1"]),
        ..CapabilityInventory::default()
    })
    .plan(&request("classic/sc/rgb_jpegxl_lossy"))
    .unwrap();
    assert!(bundle.plan.artifacts.is_empty());
    assert!(bundle.plan.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::ExternalBackend
            && item.reason_code == "codec_executable_unavailable"
            && item.message.starts_with("cjxl:")
    }));
    assert!(
        bundle
            .plan
            .unavailable
            .iter()
            .all(|item| item.reason_code != "feature_disabled")
    );
}

#[test]
fn fully_injected_requirements_reach_the_next_planning_boundary() {
    let bundle = provider(CapabilityInventory {
        compiled_features: set(&["deflate"]),
        executable_codec_backends: set(&["dicom_rs_deflated_dataset_writer"]),
        ..CapabilityInventory::default()
    })
    .plan(&request("classic/sc/mono2_u8_deflated_explicit_le"))
    .unwrap();
    assert!(bundle.pending.is_empty());
    assert!(bundle.plan.unavailable.is_empty());
    let PlannedArtifact::Dicom(artifact) = &bundle.plan.artifacts[0] else {
        panic!("exceptional SC must be DICOM")
    };
    assert_eq!(
        artifact.encoding.transfer_syntax_uid,
        "1.2.840.10008.1.2.1.99"
    );
    assert!(matches!(
        bundle.bindings[&artifact.logical_id].slots["pixels"],
        SlotExecutionBinding::NativeFrames { .. }
    ));
}

#[test]
fn fully_injected_encoded_frame_case_yields_a_codec_binding() {
    let bundle = provider(CapabilityInventory {
        compiled_features: set(&["jpeg"]),
        executable_codec_backends: set(&["dicom_rs_jpeg_baseline_writer"]),
        ..CapabilityInventory::default()
    })
    .plan(&request("classic/sc/rgb_planar0_jpeg_baseline_8bit"))
    .unwrap();
    let PlannedArtifact::Dicom(artifact) = &bundle.plan.artifacts[0] else {
        panic!("exceptional SC must be DICOM")
    };
    let SlotExecutionBinding::CodecRequest { request } =
        &bundle.bindings[&artifact.logical_id].slots["pixels"]
    else {
        panic!("encoded-frame SC requires a codec request")
    };
    assert_eq!(request.backend_id, "dicom_rs_jpeg_baseline_writer");
    assert_eq!(request.artifact_id, artifact.logical_id);
    assert_eq!(request.slot, "pixels");
}

#[test]
fn qualified_locked_full_file_codec_remains_explicitly_pending() {
    let bundle = provider(CapabilityInventory {
        compiled_features: set(&["legacy_jpeg_dcmtk"]),
        executable_codec_backends: set(&["dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer"]),
        available_executables: set(&["dcmcjpeg"]),
        ..CapabilityInventory::default()
    })
    .plan(&request("classic/sc/mono2_u16_jpeg_lossless_process_14"))
    .unwrap();
    assert!(bundle.plan.artifacts.is_empty());
    assert_eq!(
        bundle.pending[0].reason_code,
        "locked_full_file_codec_unavailable"
    );
    assert!(bundle.plan.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::ExternalBackend
            && item.reason_code == "locked_full_file_codec_unavailable"
    }));
}

#[cfg(not(feature = "jpeg"))]
#[test]
fn default_provider_inventory_is_compiled_only_and_does_not_probe_runtime() {
    let bundle = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&request("classic/sc/rgb_planar0_jpeg_baseline_8bit"))
        .unwrap();
    assert!(bundle.plan.artifacts.is_empty());
    assert!(
        bundle
            .plan
            .unavailable
            .iter()
            .any(|item| item.reason_code == "feature_disabled")
    );
}

#[cfg(feature = "jpegxl")]
#[test]
fn cfg_enabled_default_inventory_still_does_not_discover_cjxl() {
    let bundle = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&request("classic/sc/rgb_jpegxl_lossy"))
        .unwrap();
    assert!(bundle.plan.artifacts.is_empty());
    assert!(
        bundle
            .plan
            .unavailable
            .iter()
            .all(|item| item.reason_code != "feature_disabled")
    );
    assert!(bundle.plan.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::ExternalBackend
            && item.reason_code == "codec_executable_unavailable"
    }));
}

#[cfg(feature = "jpeg")]
#[test]
fn cfg_enabled_default_inventory_executes_the_linked_jpeg_backend() {
    let bundle = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&request("classic/sc/rgb_planar0_jpeg_baseline_8bit"))
        .unwrap();
    assert!(bundle.pending.is_empty());
    assert_eq!(bundle.plan.artifacts.len(), 1);
    assert!(bundle.plan.unavailable.is_empty());
}
