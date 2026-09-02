use std::collections::BTreeSet;

use serde::Deserialize;
use synth_dicom_gen::corpus_plan::PlannedArtifact;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::executor::services::SlotExecutionBinding;
use synth_dicom_gen::recipes::{
    BackendBoundary, QUANTITATIVE_NATIVE_PROVIDER_ID, RecipeCatalog, TransferSyntaxBackendRegistry,
};
use synth_dicom_gen::runtime_capabilities::{CapabilityInventory, QualifiedExecutableIdentity};

#[derive(Debug, Deserialize)]
struct Registry {
    cases: Vec<RegistryCase>,
}

#[derive(Debug, Deserialize)]
struct RegistryCase {
    case_id: String,
    status: String,
    transfer_syntax_uid: Option<String>,
    requirements: Requirements,
}

#[derive(Debug, Deserialize)]
struct Requirements {
    features: Vec<String>,
    external_codecs: Vec<String>,
    external_validators: Vec<String>,
}

fn exceptional_cases() -> Vec<RegistryCase> {
    let registry: Registry = serde_json::from_str(include_str!("../cases/registry.json")).unwrap();
    registry
        .cases
        .into_iter()
        .filter(|case| {
            case.status == "implemented"
                && (!case.requirements.features.is_empty()
                    || !case.requirements.external_codecs.is_empty()
                    || !case.requirements.external_validators.is_empty())
        })
        .collect()
}

fn requirement_summary(requirements: &Requirements) -> String {
    let mut parts = Vec::new();
    if !requirements.features.is_empty() {
        parts.push(format!("features={}", requirements.features.join(",")));
    }
    if !requirements.external_codecs.is_empty() {
        parts.push(format!(
            "external_codecs={}",
            requirements.external_codecs.join(",")
        ));
    }
    if !requirements.external_validators.is_empty() {
        parts.push(format!(
            "external_validators={}",
            requirements.external_validators.join(",")
        ));
    }
    parts.join("; ")
}

fn request(case_id: &str) -> CuratedScPlanRequest {
    CuratedScPlanRequest {
        selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
        seed: 1,
        max_parallelism: 1,
    }
}

fn case_binding(artifact: &PlannedArtifact) -> Option<&str> {
    match artifact {
        PlannedArtifact::Dicom(artifact) if artifact.output.publish => {
            artifact.case_binding.as_ref()
        }
        PlannedArtifact::ImportedDicom(artifact) => artifact.case_binding.as_ref(),
        PlannedArtifact::Dicom(_)
        | PlannedArtifact::Auxiliary(_)
        | PlannedArtifact::Mutation(_)
        | PlannedArtifact::Qualification(_) => None,
    }
    .map(|binding| binding.case_id.as_str())
}

#[test]
fn every_implemented_exceptional_binding_is_reachable_or_explicitly_unavailable() {
    let cases = exceptional_cases();
    assert!(!cases.is_empty());
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let codecs = TransferSyntaxBackendRegistry::load_committed().unwrap();

    for case in cases {
        let transfer_syntax_uid = case.transfer_syntax_uid.as_deref().unwrap();
        let backend = codecs.for_transfer_syntax(transfer_syntax_uid).unwrap();
        let identity = catalog.binding_for_case(&case.case_id).unwrap();
        let recipe = &catalog.recipes()[identity];
        assert!(
            recipe.plan_provider_id == "native.exceptional_sc_plan"
                || (recipe.plan_provider_id == QUANTITATIVE_NATIVE_PROVIDER_ID
                    && transfer_syntax_uid == "1.2.840.10008.1.2.8.1"),
            "implemented exceptional case {} is not owned by a typed plan provider",
            case.case_id
        );

        let unavailable =
            CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
                .unwrap()
                .with_capability_inventory(CapabilityInventory::default())
                .plan(&request(&case.case_id))
                .unwrap();
        let pending = unavailable
            .pending
            .iter()
            .find(|pending| pending.case_id == case.case_id)
            .unwrap();
        assert_eq!(pending.reason_code, "feature_gated_case_unavailable");
        assert_eq!(
            pending.message,
            format!(
                "case requires unavailable build/runtime capabilities: {}",
                requirement_summary(&case.requirements)
            )
        );
        assert!(unavailable.plan.unavailable.iter().any(|item| {
            item.affected_artifact_ids
                .iter()
                .any(|artifact| pending.artifact_ids.contains(artifact))
        }));

        let mut inventory = CapabilityInventory {
            compiled_features: case.requirements.features.iter().cloned().collect(),
            executable_codec_backends: BTreeSet::from([backend.backend_id.into()]),
            external_validators: case
                .requirements
                .external_validators
                .iter()
                .cloned()
                .collect(),
            ..CapabilityInventory::default()
        };
        if let Some(executable) = backend.external_tool {
            inventory.available_executables.insert(executable.into());
            inventory.executable_identities.insert(
                executable.into(),
                QualifiedExecutableIdentity {
                    version: "qualified-test-version".into(),
                    executable_sha256: "0".repeat(64),
                },
            );
        }
        let planned =
            CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
                .unwrap()
                .with_capability_inventory(inventory)
                .plan(&request(&case.case_id))
                .unwrap();
        assert!(
            planned
                .pending
                .iter()
                .all(|pending| pending.case_id != case.case_id),
            "fully injected case remained pending: {}",
            case.case_id
        );
        let target = planned
            .plan
            .artifacts
            .iter()
            .find(|artifact| case_binding(artifact) == Some(case.case_id.as_str()))
            .unwrap_or_else(|| panic!("case {} produced no requested artifact", case.case_id));
        let binding = &planned.bindings[target.logical_id()];
        match backend.boundary {
            BackendBoundary::DatasetWriter => assert!(
                binding
                    .slots
                    .values()
                    .any(|slot| matches!(slot, SlotExecutionBinding::NativeFrames { .. }))
            ),
            BackendBoundary::EncodedFrames => assert!(
                binding
                    .slots
                    .values()
                    .any(|slot| matches!(slot, SlotExecutionBinding::CodecRequest { .. }))
            ),
            BackendBoundary::LockedFullFileTransform => {
                assert!(matches!(target, PlannedArtifact::ImportedDicom(_)));
                assert!(
                    binding
                        .slots
                        .values()
                        .any(|slot| matches!(slot, SlotExecutionBinding::ProviderRequest { .. }))
                );
                assert!(planned.plan.dependencies.iter().any(|dependency| {
                    dependency.artifact_id == target.logical_id()
                        && dependency.relationship == "locked_full_file_source"
                }));
            }
        }
        planned.plan.validate().unwrap();
    }
}
