use std::collections::BTreeMap;

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION,
    CapabilityKind, CaseBinding, CorpusPlan, CorpusPlanError, DatasetLengthPolicy, EncodingPlan,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, FragmentationPolicy,
    ImplementationIdentityPlan, MutationPlan, OffsetTablePolicy, OutputPlan, OutputRelativePath,
    PlannedArtifact, PlannedAuxiliaryArtifact, PlannedByteRange, PlannedDicomArtifact,
    PlannedMutationArtifact, PlannedMutationOperation, PlannedQualification, PreamblePolicy,
    PublicationPlan, PublicationTransaction, QualificationPayloadPolicy, ResourcePlan,
    UnavailableCapability, ValidationPlan, ValidationRequirement, ValidationRule,
};

const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";
const SC_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.7";
const IMPLEMENTATION_UID: &str = "2.25.100";

fn instance(logical_id: &str, sop_uid: &str) -> ResolvedInstancePlan {
    ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: logical_id.into(),
        template_id: TemplateId("classic/secondary-capture/monochrome".into()),
        template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
        sop_class_uid: SC_STORAGE.into(),
        transfer_syntax_uid: EXPLICIT_LE.into(),
        identities: IdentityPlan::from_exact_values(
            logical_id,
            [
                (CompositionUidRole::SopInstance, 0, sop_uid.to_owned()),
                (
                    CompositionUidRole::ImplementationClass,
                    0,
                    IMPLEMENTATION_UID.to_owned(),
                ),
            ],
        )
        .unwrap(),
        attributes: vec![],
        content: vec![],
        references: vec![],
    }
}

fn validation() -> ValidationPlan {
    ValidationPlan {
        rules: vec![ValidationRule {
            rule_id: "part10.identity".into(),
            requirement: ValidationRequirement::Required,
            parameters: BTreeMap::new(),
        }],
    }
}

fn evidence() -> EvidencePlan {
    EvidencePlan {
        obligations: vec![EvidenceObligation {
            obligation_id: "same-project.part10".into(),
            route_id: "builtin.strict".into(),
            independence: EvidenceIndependence::SameProject,
            required: true,
            parameters: BTreeMap::new(),
        }],
    }
}

fn resources() -> ArtifactResourceEstimate {
    ArtifactResourceEstimate {
        output_bytes: 4096,
        peak_working_bytes: 8192,
    }
}

fn artifact_order(logical_id: &str) -> u64 {
    match logical_id {
        "source" | "z" | "fuzz" | "one" | "first" | "manifest-artifact" | "large" | "private" => 0,
        "derived" | "a" | "private-source" | "second" | "consumer" => 1,
        "evidence" | "b" | "c" | "invalid" => 2,
        other => panic!("test artifact order is not declared for {other}"),
    }
}

fn dicom(logical_id: &str, provenance: ArtifactProvenance, path: &str) -> PlannedArtifact {
    PlannedArtifact::Dicom(PlannedDicomArtifact {
        logical_id: logical_id.into(),
        order: artifact_order(logical_id),
        provenance,
        case_binding: Some(CaseBinding {
            case_id: format!("classic/sc/{logical_id}"),
            recipe_id: format!("recipe_{logical_id}"),
            recipe_version: "1.0.0".into(),
        }),
        instance: instance(logical_id, &format!("2.25.{}", 1000 + logical_id.len())),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(path).unwrap(),
            role: "dicom_instance".into(),
            publish: true,
        },
        encoding: EncodingPlan {
            transfer_syntax_uid: EXPLICIT_LE.into(),
            dataset_length: DatasetLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            implementation: ImplementationIdentityPlan {
                class_uid: IMPLEMENTATION_UID.into(),
                version_name: Some("DICOMTS010".into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: validation(),
        evidence: evidence(),
        resources: resources(),
    })
}

fn publication() -> PublicationPlan {
    PublicationPlan {
        manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
        transaction: PublicationTransaction::AtomicNoReplace,
        private_staging: true,
        no_overwrite: true,
    }
}

fn plan(artifacts: Vec<PlannedArtifact>, dependencies: Vec<ArtifactDependency>) -> CorpusPlan {
    CorpusPlan {
        schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed: 17,
        artifacts,
        dependencies,
        unavailable: vec![],
        publication: publication(),
        resources: ResourcePlan {
            max_artifacts: 16,
            max_total_output_bytes: 1_000_000,
            max_peak_working_bytes: 100_000,
            max_parallelism: 4,
        },
    }
}

fn edge(artifact_id: &str, depends_on: &str) -> ArtifactDependency {
    ArtifactDependency {
        artifact_id: artifact_id.into(),
        depends_on: depends_on.into(),
        relationship: "source".into(),
        frame_numbers: vec![],
    }
}

#[test]
fn canonical_hash_normalizes_artifact_dependency_and_unavailable_order() {
    let source = dicom(
        "source",
        ArtifactProvenance::Requested,
        "classic/source.dcm",
    );
    let derived = dicom(
        "derived",
        ArtifactProvenance::Dependency {
            requested_by: vec!["source".into()],
        },
        "derived/derived.dcm",
    );
    let auxiliary = PlannedArtifact::Auxiliary(PlannedAuxiliaryArtifact {
        logical_id: "evidence".into(),
        order: artifact_order("evidence"),
        provenance: ArtifactProvenance::Dependency {
            requested_by: vec!["derived".into()],
        },
        auxiliary_kind: "semantic_projection".into(),
        output: OutputPlan {
            relative_path: OutputRelativePath::new("evidence/result.json").unwrap(),
            role: "evidence".into(),
            publish: true,
        },
        parameters: BTreeMap::new(),
        validation: validation(),
        evidence: evidence(),
        resources: resources(),
    });
    let dependencies = vec![edge("evidence", "derived"), edge("derived", "source")];
    let mut first = plan(
        vec![auxiliary.clone(), derived.clone(), source.clone()],
        dependencies.clone(),
    );
    first.unavailable = vec![
        UnavailableCapability {
            capability_id: "validator.external".into(),
            kind: CapabilityKind::Validator,
            reason_code: "tool_unavailable".into(),
            message: "The pinned validator is not installed.".into(),
            affected_artifact_ids: vec!["planned-z".into(), "planned-a".into()],
            requirements: BTreeMap::new(),
        },
        UnavailableCapability {
            capability_id: "codec.optional".into(),
            kind: CapabilityKind::Codec,
            reason_code: "feature_disabled".into(),
            message: "The optional codec feature is disabled.".into(),
            affected_artifact_ids: vec!["planned-codec".into()],
            requirements: BTreeMap::new(),
        },
    ];

    let mut second = plan(
        vec![source, derived, auxiliary],
        dependencies.into_iter().rev().collect(),
    );
    second.unavailable = first.unavailable.iter().cloned().rev().collect();
    second.unavailable[0].affected_artifact_ids.reverse();

    assert_eq!(
        first.topological_order().unwrap(),
        vec!["source", "derived", "evidence"]
    );
    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert_eq!(
        first.canonical_sha256().unwrap(),
        second.canonical_sha256().unwrap()
    );
}

#[test]
fn deterministic_topology_uses_explicit_order_as_the_ready_node_tie_breaker() {
    let plan = plan(
        vec![
            dicom("z", ArtifactProvenance::Requested, "z.dcm"),
            dicom("c", ArtifactProvenance::Requested, "c.dcm"),
            dicom("a", ArtifactProvenance::Requested, "a.dcm"),
        ],
        vec![edge("c", "a")],
    );
    assert_eq!(plan.topological_order().unwrap(), vec!["z", "a", "c"]);
}

#[test]
fn artifact_order_is_unique_even_when_logical_ids_differ() {
    let first = dicom("first", ArtifactProvenance::Requested, "first.dcm");
    let mut second = dicom("second", ArtifactProvenance::Requested, "second.dcm");
    let PlannedArtifact::Dicom(second_plan) = &mut second else {
        unreachable!()
    };
    second_plan.order = 0;
    assert!(matches!(
        plan(vec![first, second], vec![]).validate(),
        Err(CorpusPlanError::DuplicateArtifactOrder(0))
    ));
}

#[test]
fn graph_validation_rejects_cycles_unknown_nodes_and_duplicate_edges() {
    let artifacts = vec![
        dicom("a", ArtifactProvenance::Requested, "a.dcm"),
        dicom("b", ArtifactProvenance::Requested, "b.dcm"),
    ];
    assert!(matches!(
        plan(artifacts.clone(), vec![edge("a", "b"), edge("b", "a")]).validate(),
        Err(CorpusPlanError::DependencyCycle(_))
    ));
    assert!(matches!(
        plan(artifacts.clone(), vec![edge("a", "missing")]).validate(),
        Err(CorpusPlanError::UnknownDependency { .. })
    ));
    assert!(matches!(
        plan(artifacts, vec![edge("a", "b"), edge("a", "b")]).validate(),
        Err(CorpusPlanError::DuplicateDependency { .. })
    ));
}

#[test]
fn output_paths_are_platform_neutral_and_cannot_escape_the_publication_root() {
    for unsafe_path in [
        "",
        "/absolute.dcm",
        "../escape.dcm",
        "nested/../../escape.dcm",
        "nested//file.dcm",
        "nested/./file.dcm",
        "nested\\file.dcm",
        "C:/windows.dcm",
        "trailing/",
    ] {
        assert!(
            matches!(
                OutputRelativePath::new(unsafe_path),
                Err(CorpusPlanError::UnsafeOutputPath(_))
            ),
            "{unsafe_path:?}"
        );
    }
    assert_eq!(
        OutputRelativePath::new("nested/instance.dcm")
            .unwrap()
            .as_str(),
        "nested/instance.dcm"
    );
}

#[test]
fn all_artifact_kinds_and_unavailable_capabilities_are_canonical_data() {
    let mutation = PlannedArtifact::Mutation(PlannedMutationArtifact {
        logical_id: "invalid".into(),
        order: artifact_order("invalid"),
        provenance: ArtifactProvenance::Requested,
        source_artifact_id: "private-source".into(),
        mutation: MutationPlan {
            contract_version: "1.0.0".into(),
            operations: vec![PlannedMutationOperation {
                operation_id: "truncate_dataset".into(),
                source_ranges: vec![PlannedByteRange {
                    start: 128,
                    end: 256,
                }],
                parameters: BTreeMap::new(),
            }],
            expected_source_sha256: "1".repeat(64),
            expected_output_sha256: "2".repeat(64),
            expected_failure_layers: vec!["dataset_parser".into()],
            acceptable_outcomes: vec!["clean_rejection".into()],
        },
        output: OutputPlan {
            relative_path: OutputRelativePath::new("negative/invalid.dcm").unwrap(),
            role: "expected_invalid".into(),
            publish: true,
        },
        validation: validation(),
        evidence: evidence(),
        resources: resources(),
    });
    let qualification = PlannedArtifact::Qualification(PlannedQualification {
        logical_id: "fuzz".into(),
        order: artifact_order("fuzz"),
        provenance: ArtifactProvenance::Requested,
        qualification_kind: "bounded_fuzz".into(),
        parameters: BTreeMap::new(),
        payload_policy: QualificationPayloadPolicy::NoPayload,
        validation: validation(),
        evidence: evidence(),
        resources: resources(),
    });
    let mut source = dicom(
        "private-source",
        ArtifactProvenance::PrivateSource {
            consumed_by: vec!["invalid".into()],
        },
        "private/source.dcm",
    );
    let PlannedArtifact::Dicom(source_plan) = &mut source else {
        unreachable!()
    };
    source_plan.output.publish = false;
    let plan = plan(
        vec![mutation, qualification, source],
        vec![edge("invalid", "private-source")],
    );
    plan.validate().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&plan.canonical_bytes().unwrap()).unwrap();
    let kinds = json["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|artifact| artifact["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(kinds, vec!["qualification", "dicom", "mutation"]);
    assert!(
        !String::from_utf8(plan.canonical_bytes().unwrap())
            .unwrap()
            .contains("/tmp/")
    );
}

#[test]
fn validation_rejects_schema_identity_encoding_and_publication_drift() {
    let mut value = plan(
        vec![dicom("one", ArtifactProvenance::Requested, "one.dcm")],
        vec![],
    );
    value.schema_version = "99.0.0".into();
    assert!(matches!(
        value.validate(),
        Err(CorpusPlanError::UnsupportedSchemaVersion(_))
    ));

    let mut value = plan(
        vec![dicom("one", ArtifactProvenance::Requested, "one.dcm")],
        vec![],
    );
    let PlannedArtifact::Dicom(artifact) = &mut value.artifacts[0] else {
        unreachable!()
    };
    artifact.instance.instance_id = "different".into();
    assert!(matches!(
        value.validate(),
        Err(CorpusPlanError::InstanceIdentityMismatch { .. })
    ));

    let mut value = plan(
        vec![dicom("one", ArtifactProvenance::Requested, "one.dcm")],
        vec![],
    );
    let PlannedArtifact::Dicom(artifact) = &mut value.artifacts[0] else {
        unreachable!()
    };
    artifact.encoding.transfer_syntax_uid = "1.2.840.10008.1.2".into();
    assert!(matches!(
        value.validate(),
        Err(CorpusPlanError::TransferSyntaxMismatch { .. })
    ));

    let mut value = plan(
        vec![dicom("one", ArtifactProvenance::Requested, "one.dcm")],
        vec![],
    );
    value.publication.no_overwrite = false;
    assert!(matches!(
        value.validate(),
        Err(CorpusPlanError::UnsafePublicationPolicy)
    ));
}

#[test]
fn planned_and_unavailable_artifact_identities_cannot_contradict_each_other() {
    let mut value = plan(
        vec![dicom("one", ArtifactProvenance::Requested, "one.dcm")],
        vec![],
    );
    value.unavailable.push(UnavailableCapability {
        capability_id: "codec".into(),
        kind: CapabilityKind::Codec,
        reason_code: "not_available".into(),
        message: "Codec unavailable in this build.".into(),
        affected_artifact_ids: vec!["one".into()],
        requirements: BTreeMap::new(),
    });
    assert!(matches!(
        value.validate(),
        Err(CorpusPlanError::AvailableArtifactMarkedUnavailable { .. })
    ));
}

#[test]
fn private_sources_are_non_public_and_bound_to_their_consumers() {
    let mut private = dicom(
        "private",
        ArtifactProvenance::PrivateSource {
            consumed_by: vec!["consumer".into()],
        },
        "private/source.dcm",
    );
    let consumer = dicom("consumer", ArtifactProvenance::Requested, "consumer.dcm");
    let published = plan(
        vec![private.clone(), consumer.clone()],
        vec![edge("consumer", "private")],
    );
    assert!(matches!(
        published.validate(),
        Err(CorpusPlanError::PrivateSourcePublished(id)) if id == "private"
    ));

    let PlannedArtifact::Dicom(private_plan) = &mut private else {
        unreachable!()
    };
    private_plan.output.publish = false;
    let disconnected = plan(vec![private.clone(), consumer.clone()], vec![]);
    assert!(matches!(
        disconnected.validate(),
        Err(CorpusPlanError::ProvenanceDependencyMismatch { .. })
    ));
    plan(vec![private, consumer], vec![edge("consumer", "private")])
        .validate()
        .unwrap();
}

#[test]
fn output_and_resource_preflight_rejects_collisions_and_overcommitment() {
    let first = dicom("first", ArtifactProvenance::Requested, "same.dcm");
    let second = dicom("second", ArtifactProvenance::Requested, "same.dcm");
    assert!(matches!(
        plan(vec![first, second], vec![]).validate(),
        Err(CorpusPlanError::DuplicateOutputPath(path)) if path == "same.dcm"
    ));

    let mut manifest_collision = plan(
        vec![dicom(
            "manifest-artifact",
            ArtifactProvenance::Requested,
            "manifest.json",
        )],
        vec![],
    );
    assert!(matches!(
        manifest_collision.validate(),
        Err(CorpusPlanError::ManifestPathCollision(path)) if path == "manifest.json"
    ));

    manifest_collision.artifacts = vec![dicom("large", ArtifactProvenance::Requested, "large.dcm")];
    manifest_collision.resources.max_total_output_bytes = 1024;
    assert!(matches!(
        manifest_collision.validate(),
        Err(CorpusPlanError::ResourceEstimateExceedsLimit {
            output_bytes: 4096,
            ..
        })
    ));
}
