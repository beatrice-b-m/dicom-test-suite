use std::collections::BTreeSet;

use dicom_test_suite::corpus_plan::CapabilityKind;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedPlanError, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
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
    let error = provider(CapabilityInventory {
        compiled_features: set(&["deflate"]),
        executable_codec_backends: set(&["dicom_rs_deflated_dataset_writer"]),
        ..CapabilityInventory::default()
    })
    .plan(&request("classic/sc/mono2_u8_deflated_explicit_le"))
    .unwrap_err();
    assert!(matches!(
        error,
        CuratedPlanError::UnsupportedCase { case_id, provider_id }
            if case_id == "classic/sc/mono2_u8_deflated_explicit_le"
                && provider_id == "native.exceptional_sc_plan"
    ));
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
