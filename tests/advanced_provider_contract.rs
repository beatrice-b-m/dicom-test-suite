use std::collections::BTreeMap;

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, MaterializedReference, ResolvedInstancePlan, TemplateId,
    TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, EncodingPlan,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, OutputPlan,
    OutputRelativePath, PlannedDicomArtifact, PreamblePolicy, PublicationPlan,
    PublicationTransaction, SequenceLengthPolicy, ValidationPlan, ValidationRequirement,
    ValidationRule,
};
use dicom_test_suite::executor::services::ArtifactExecutionBindings;
use dicom_test_suite::recipes::{
    AdvancedArtifactPlanningContext, AdvancedArtifactProvenance, AdvancedArtifactRole,
    AdvancedPlanProviderOutput, AdvancedPlanProviderRequest, AdvancedPlannedArtifact,
    AdvancedProviderContractError, AdvancedProviderFamily, AdvancedProviderLimits,
    AdvancedSourceConsumer, AdvancedSourceReference, AdvancedSourceRole, RecipeIdentity,
};

const TS: &str = "1.2.840.10008.1.2.1";
const SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.66.1";
const IMPLEMENTATION_UID: &str = "2.25.999";

fn request() -> AdvancedPlanProviderRequest {
    let context_artifact = artifact(
        "fixed",
        1,
        "advanced/registration/fixed.dcm",
        "2.25.101",
        ArtifactProvenance::Requested,
        vec![],
    );
    AdvancedPlanProviderRequest {
        provider_id: "native.advanced.registration".into(),
        family: AdvancedProviderFamily::Registration,
        case_id: "advanced/registration/spatial".into(),
        recipe: RecipeIdentity {
            recipe_id: "advanced.registration.spatial".into(),
            recipe_version: "1.0.0".into(),
        },
        seed: 7,
        artifact_contexts: vec![AdvancedArtifactPlanningContext {
            recipe_artifact_logical_id: "fixed".into(),
            target_instance_id: "fixed".into(),
            order: context_artifact.order,
            output: context_artifact.output,
            identities: context_artifact.instance.identities,
        }],
        limits: AdvancedProviderLimits {
            max_artifacts: 4,
            max_references: 4,
            max_binding_slots: 4,
            max_total_output_bytes: 10_000,
            max_peak_working_bytes: 2_000,
            max_parallelism: 2,
        },
    }
}

fn reference() -> MaterializedReference {
    MaterializedReference {
        source_instance_id: "registration".into(),
        target_instance_id: "fixed".into(),
        role: "referenced_image".into(),
        frame_role: None,
        referenced_sop_class_uid: SOP_CLASS.into(),
        referenced_sop_instance_uid: "2.25.101".into(),
        referenced_frames: vec![],
    }
}

fn artifact(
    id: &str,
    order: u64,
    path: &str,
    sop_uid: &str,
    provenance: ArtifactProvenance,
    references: Vec<MaterializedReference>,
) -> PlannedDicomArtifact {
    PlannedDicomArtifact {
        logical_id: id.into(),
        order,
        provenance,
        case_binding: None,
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: id.into(),
            template_id: TemplateId("advanced/registration".into()),
            template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
            sop_class_uid: SOP_CLASS.into(),
            transfer_syntax_uid: TS.into(),
            identities: IdentityPlan::from_exact_values(
                id,
                [
                    (CompositionUidRole::SopInstance, 0, sop_uid.into()),
                    (
                        CompositionUidRole::ImplementationClass,
                        0,
                        IMPLEMENTATION_UID.into(),
                    ),
                ],
            )
            .unwrap(),
            attributes: vec![],
            content: vec![],
            references,
        },
        output: OutputPlan {
            relative_path: OutputRelativePath::new(path).unwrap(),
            role: "dicom_instance".into(),
            publish: true,
        },
        encoding: EncodingPlan {
            transfer_syntax_uid: TS.into(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: IMPLEMENTATION_UID.into(),
                version_name: Some("DICOMTS010".into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "advanced.reference_closure".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: "same-project.reference".into(),
                route_id: "builtin.strict".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 500,
            peak_working_bytes: 1_000,
        },
    }
}

fn valid_output() -> AdvancedPlanProviderOutput {
    let role = AdvancedSourceRole::RegistrationFixed;
    AdvancedPlanProviderOutput {
        artifacts: vec![
            AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::Registration { ordinal: 1 },
                planned: artifact(
                    "fixed",
                    1,
                    "advanced/registration/fixed.dcm",
                    "2.25.101",
                    ArtifactProvenance::Requested,
                    vec![],
                ),
                provenance: AdvancedArtifactProvenance::Requested,
            },
            AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::Registration { ordinal: 2 },
                planned: artifact(
                    "registration",
                    2,
                    "advanced/registration/registration.dcm",
                    "2.25.102",
                    ArtifactProvenance::Dependency {
                        requested_by: vec!["fixed".into()],
                    },
                    vec![reference()],
                ),
                provenance: AdvancedArtifactProvenance::Dependency {
                    requested_by: vec![AdvancedSourceConsumer {
                        artifact_id: "fixed".into(),
                        role: role.clone(),
                    }],
                },
            },
        ],
        dependencies: vec![ArtifactDependency {
            artifact_id: "registration".into(),
            depends_on: "fixed".into(),
            relationship: role.dependency_relationship().into(),
            frame_numbers: vec![],
        }],
        references: vec![AdvancedSourceReference {
            owner_artifact_id: "registration".into(),
            source_artifact_id: "fixed".into(),
            source_role: role,
            reference: reference(),
        }],
        bindings: vec![
            ArtifactExecutionBindings {
                artifact_id: "fixed".into(),
                slots: BTreeMap::new(),
            },
            ArtifactExecutionBindings {
                artifact_id: "registration".into(),
                slots: BTreeMap::new(),
            },
        ],
    }
}

fn publication() -> PublicationPlan {
    PublicationPlan {
        manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
        transaction: PublicationTransaction::AtomicNoReplace,
        private_staging: true,
        no_overwrite: true,
    }
}

#[test]
fn valid_advanced_contract_assembles_a_deterministic_corpus_plan() {
    let output = valid_output();
    output.validate(&request()).unwrap();
    let plan = output.to_corpus_plan(&request(), publication()).unwrap();
    assert_eq!(
        plan.topological_order().unwrap(),
        vec!["fixed", "registration"]
    );
    assert_eq!(plan.seed, request().seed);
    assert_eq!(plan.artifacts.len(), output.artifacts.len());
    assert_eq!(
        plan.canonical_sha256().unwrap(),
        output
            .to_corpus_plan(&request(), publication())
            .unwrap()
            .canonical_sha256()
            .unwrap()
    );
}

#[test]
fn request_requires_exact_nonempty_artifact_contexts() {
    let mut empty = request();
    empty.artifact_contexts.clear();
    assert!(matches!(
        empty.validate().unwrap_err(),
        AdvancedProviderContractError::EmptyArtifactContexts
    ));

    let mut wrong_owner = request();
    wrong_owner.artifact_contexts[0]
        .identities
        .logical_instance_id = "different_owner".into();
    assert!(matches!(
        wrong_owner.validate().unwrap_err(),
        AdvancedProviderContractError::ArtifactContextIdentityOwner
    ));
}

#[test]
fn rejects_duplicate_roles_paths_and_misordered_artifacts() {
    let mut duplicate_role = valid_output();
    duplicate_role.artifacts[1].role = duplicate_role.artifacts[0].role.clone();
    assert!(matches!(
        duplicate_role.validate(&request()),
        Err(AdvancedProviderContractError::DuplicateArtifactRole)
    ));

    let mut duplicate_path = valid_output();
    duplicate_path.artifacts[1].planned.output.relative_path = duplicate_path.artifacts[0]
        .planned
        .output
        .relative_path
        .clone();
    assert!(matches!(
        duplicate_path.validate(&request()),
        Err(AdvancedProviderContractError::DuplicateOutputPath(_))
    ));

    let mut misordered = valid_output();
    misordered.artifacts.swap(0, 1);
    assert!(matches!(
        misordered.validate(&request()),
        Err(AdvancedProviderContractError::MisorderedArtifacts)
    ));
}

#[test]
fn rejects_missing_reference_dependency_and_reference_binding() {
    let mut missing_dependency = valid_output();
    missing_dependency.dependencies.clear();
    assert!(matches!(
        missing_dependency.validate(&request()),
        Err(AdvancedProviderContractError::MissingReferenceDependency)
            | Err(AdvancedProviderContractError::ProvenanceDependencyMismatch)
    ));

    let mut missing_reference = valid_output();
    missing_reference.references.clear();
    assert!(matches!(
        missing_reference.validate(&request()),
        Err(AdvancedProviderContractError::UnboundMaterializedReference)
    ));
}

#[test]
fn rejects_duplicate_missing_and_wrong_slot_bindings() {
    let mut duplicate = valid_output();
    duplicate.bindings.push(duplicate.bindings[0].clone());
    assert!(matches!(
        duplicate.validate(&request()),
        Err(AdvancedProviderContractError::DuplicateBinding(_))
    ));

    let mut missing = valid_output();
    missing.bindings.pop();
    assert!(matches!(
        missing.validate(&request()),
        Err(AdvancedProviderContractError::MissingBinding)
    ));

    let mut unknown = valid_output();
    unknown.bindings[0].artifact_id = "unknown".into();
    assert!(matches!(
        unknown.validate(&request()),
        Err(AdvancedProviderContractError::UnknownBinding(_))
    ));
}

#[test]
fn contract_source_has_no_filesystem_or_output_root_channel() {
    let source = include_str!("../src/recipes/advanced.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "PathBuf",
        "output_root",
        "File::",
        "generator::",
        "Part10Materializer",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden boundary: {forbidden}"
        );
    }
}

#[test]
fn family_roles_and_resource_bounds_are_closed_and_checked() {
    let mut wrong_family = valid_output();
    wrong_family.artifacts[0].role = AdvancedArtifactRole::EnhancedInstance { ordinal: 1 };
    assert!(matches!(
        wrong_family.validate(&request()),
        Err(AdvancedProviderContractError::FamilyRoleMismatch)
    ));

    let mut bounded_request = request();
    bounded_request.limits.max_total_output_bytes = 999;
    assert!(matches!(
        valid_output().validate(&bounded_request),
        Err(AdvancedProviderContractError::ResourceLimitExceeded)
    ));

    let roles = serde_json::to_value([
        AdvancedArtifactRole::EnhancedInstance { ordinal: 1 },
        AdvancedArtifactRole::WholeSlidePyramid {
            level: 0,
            artifact_kind: dicom_test_suite::recipes::WholeSlideArtifactKind::Volume,
        },
        AdvancedArtifactRole::Registration { ordinal: 1 },
        AdvancedArtifactRole::PresentationState { ordinal: 1 },
    ])
    .unwrap();
    assert_eq!(roles.as_array().unwrap().len(), 4);
}
