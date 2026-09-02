use std::collections::BTreeSet;

use synth_dicom_gen::recipes::CODEC_BACKENDS;
use synth_dicom_gen::runtime_capabilities::{
    CapabilityEvaluationRequest, CapabilityInventory, CapabilityKind, RegistryRuntimeRequirements,
    RuntimeCapabilityEvaluator, UnavailableReason,
};
use serde::Deserialize;

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn builtin_backend_needs_no_injected_runtime_capability() {
    let result = RuntimeCapabilityEvaluator::committed().unwrap().evaluate(
        CapabilityEvaluationRequest {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            determinism: "byte_stable",
            requirements: &RegistryRuntimeRequirements::default(),
        },
        &CapabilityInventory::default(),
    );
    assert!(result.available);
    assert_eq!(result.backend_id.as_deref(), Some("dicom-rs.part10"));
}

#[test]
fn feature_backend_and_external_tool_are_distinct_capabilities() {
    let requirements = RegistryRuntimeRequirements {
        features: vec!["jpegxl".into()],
        external_codecs: vec!["cjxl 0.11.2".into()],
        ..RegistryRuntimeRequirements::default()
    };
    let evaluator = RuntimeCapabilityEvaluator::committed().unwrap();
    let missing = evaluator.evaluate(
        CapabilityEvaluationRequest {
            transfer_syntax_uid: "1.2.840.10008.1.2.4.112",
            determinism: "semantic_stable",
            requirements: &requirements,
        },
        &CapabilityInventory::default(),
    );
    assert_eq!(
        missing
            .unavailable
            .iter()
            .map(|item| item.kind)
            .collect::<BTreeSet<_>>(),
        set_kind(&[
            CapabilityKind::CompileTimeFeature,
            CapabilityKind::CodecBackend,
            CapabilityKind::CodecExecutable,
        ])
    );

    let available = evaluator.evaluate(
        CapabilityEvaluationRequest {
            transfer_syntax_uid: "1.2.840.10008.1.2.4.112",
            determinism: "semantic_stable",
            requirements: &requirements,
        },
        &CapabilityInventory {
            compiled_features: set(&["jpegxl"]),
            executable_codec_backends: set(&["cjxl_jpegxl_lossy_command_writer"]),
            available_executables: set(&["cjxl"]),
            ..CapabilityInventory::default()
        },
    );
    assert!(available.available);
}

fn set_kind(values: &[CapabilityKind]) -> BTreeSet<CapabilityKind> {
    values.iter().copied().collect()
}

#[test]
fn validators_and_providers_remain_separate_unavailable_reasons() {
    let requirements = RegistryRuntimeRequirements {
        external_validators: vec!["dciodvfy 3.3.8".into()],
        external_providers: vec!["highdicom.sr".into()],
        ..RegistryRuntimeRequirements::default()
    };
    let result = RuntimeCapabilityEvaluator::committed().unwrap().evaluate(
        CapabilityEvaluationRequest {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            determinism: "byte_stable",
            requirements: &requirements,
        },
        &CapabilityInventory::default(),
    );
    assert!(result.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::ExternalValidator
            && item.reason == UnavailableReason::ExternalValidatorUnavailable
    }));
    assert!(result.unavailable.iter().any(|item| {
        item.kind == CapabilityKind::ExternalProvider
            && item.reason == UnavailableReason::ExternalProviderUnavailable
    }));
}

#[test]
fn compiled_inventory_exactly_reflects_active_cfg_features() {
    let inventory = CapabilityInventory::compiled();
    for (name, enabled) in [
        ("charls", cfg!(feature = "charls")),
        ("deflate", cfg!(feature = "deflate")),
        ("jpeg", cfg!(feature = "jpeg")),
        ("jpegxl", cfg!(feature = "jpegxl")),
        ("jpeg2000", cfg!(feature = "jpeg2000")),
        ("htj2k_openjph", cfg!(feature = "htj2k_openjph")),
        ("legacy_jpeg_dcmtk", cfg!(feature = "legacy_jpeg_dcmtk")),
    ] {
        assert_eq!(inventory.compiled_features.contains(name), enabled);
    }
}

#[test]
fn malformed_registry_contract_is_typed_unavailability_not_a_probe() {
    let requirements = RegistryRuntimeRequirements {
        features: vec!["jpeg".into()],
        ..RegistryRuntimeRequirements::default()
    };
    let result = RuntimeCapabilityEvaluator::committed().unwrap().evaluate(
        CapabilityEvaluationRequest {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            determinism: "byte_stable",
            requirements: &requirements,
        },
        &CapabilityInventory {
            compiled_features: set(&["jpeg"]),
            ..CapabilityInventory::default()
        },
    );
    assert!(matches!(
        result.unavailable.as_slice(),
        [item] if item.kind == CapabilityKind::RegistryContract
            && matches!(item.reason, UnavailableReason::RegistryContractInvalid(_))
    ));
}

#[derive(Deserialize)]
struct RegistryDocument {
    cases: Vec<RegistryCase>,
}

#[derive(Deserialize)]
struct RegistryCase {
    transfer_syntax_uid: Option<String>,
    determinism: String,
    requirements: RegistryRuntimeRequirements,
    provider: RegistryProvider,
}

#[derive(Deserialize)]
struct RegistryProvider {
    kind: String,
}

#[test]
fn every_committed_registry_requirement_maps_to_the_codec_registry() {
    let registry: RegistryDocument =
        serde_json::from_str(include_str!("../cases/registry.json")).unwrap();
    assert!(!registry.cases.is_empty());
    let mut inventory = CapabilityInventory::default();
    inventory.compiled_features = registry
        .cases
        .iter()
        .flat_map(|case| case.requirements.features.iter().cloned())
        .collect();
    inventory.external_validators = registry
        .cases
        .iter()
        .flat_map(|case| case.requirements.external_validators.iter().cloned())
        .collect();
    inventory.executable_codec_backends = CODEC_BACKENDS
        .iter()
        .map(|backend| backend.backend_id.into())
        .collect();
    inventory.available_executables = CODEC_BACKENDS
        .iter()
        .filter_map(|backend| backend.external_tool.map(str::to_owned))
        .collect();
    let evaluator = RuntimeCapabilityEvaluator::committed().unwrap();
    for case in &registry.cases {
        let Some(transfer_syntax_uid) = case.transfer_syntax_uid.as_deref() else {
            continue;
        };
        let result = evaluator.evaluate(
            CapabilityEvaluationRequest {
                transfer_syntax_uid,
                determinism: &case.determinism,
                requirements: &case.requirements,
            },
            &inventory,
        );
        let executable_contract = case.provider.kind == "rust_native"
            || !case.requirements.features.is_empty()
            || !case.requirements.external_codecs.is_empty();
        if !executable_contract {
            continue;
        }
        if CODEC_BACKENDS
            .iter()
            .any(|backend| backend.transfer_syntax_uid == transfer_syntax_uid)
        {
            assert!(
                result.available,
                "{}: {:?}",
                transfer_syntax_uid, result.unavailable
            );
        } else {
            assert!(matches!(
                result.unavailable.as_slice(),
                [item] if item.kind == CapabilityKind::RegistryContract
                    && matches!(item.reason, UnavailableReason::RegistryContractInvalid(_))
            ));
        }
    }
}
