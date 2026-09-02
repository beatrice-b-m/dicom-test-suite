use std::collections::BTreeMap;

use synth_dicom_gen::composition::{
    CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use synth_dicom_gen::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION,
    CapabilityKind, CaseBinding, CorpusPlan, CorpusPlanError, EncodingPlan, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, MutationPlan, OffsetTablePolicy, OutputPlan,
    OutputRelativePath, PlannedArtifact, PlannedAuxiliaryArtifact, PlannedByteRange,
    PlannedChangedByteRange, PlannedDicomArtifact, PlannedMutationArtifact,
    PlannedMutationOperation, PlannedMutationSource, PlannedQualification,
    PlannedQualificationSource, PreamblePolicy, PublicationPlan, PublicationTransaction,
    QualificationPayloadPolicy, ResourcePlan, SequenceLengthPolicy, UnavailableCapability,
    ValidationPlan, ValidationRequirement, ValidationRule,
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

#[test]
fn qualification_sources_are_ordered_private_and_identity_bound() {
    let mut source = dicom(
        "private-source",
        ArtifactProvenance::PrivateSource {
            consumed_by: vec!["fuzz".into()],
        },
        "private/fuzz-source.dcm",
    );
    let PlannedArtifact::Dicom(source_artifact) = &mut source else {
        unreachable!()
    };
    source_artifact.output.publish = false;
    let case_binding = source_artifact.case_binding.clone().unwrap();
    let qualification_artifact = PlannedArtifact::Qualification(PlannedQualification {
        logical_id: "fuzz".into(),
        order: 0,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: "fuzz/parser/bounded_seed_corpus".into(),
            recipe_id: "fuzz_parser_bounded_seed_corpus".into(),
            recipe_version: "0.1.0".into(),
        }),
        profile: Some("fuzz".into()),
        run_seed: Some(1),
        qualification_kind: "bounded_deterministic_fuzz".into(),
        parameters: BTreeMap::new(),
        sources: vec![PlannedQualificationSource {
            artifact_id: "private-source".into(),
            case_binding,
            artifact_logical_id: "instance".into(),
            dependency_role: "part10_explicit_vr_le".into(),
            binding_slot: "source_0".into(),
            expected_sha256: "a".repeat(64),
            expected_size_bytes: 926,
            parameters: BTreeMap::from([
                (
                    "seed_description_id".into(),
                    serde_json::json!("part10-explicit-vr-le-v1"),
                ),
                (
                    "mutation_surfaces".into(),
                    serde_json::json!(["file_meta", "pixel_data"]),
                ),
            ]),
        }],
        payload_policy: QualificationPayloadPolicy::NoPayload,
        validation: validation(),
        evidence: evidence(),
        resources: ArtifactResourceEstimate {
            output_bytes: 0,
            peak_working_bytes: 8192,
        },
    });
    let plan = plan(
        vec![qualification_artifact, source],
        vec![edge("fuzz", "private-source")],
    );
    plan.validate().unwrap();

    fn qualification_mut(plan: &mut CorpusPlan) -> &mut PlannedQualification {
        plan.artifacts
            .iter_mut()
            .find_map(|artifact| match artifact {
                PlannedArtifact::Qualification(value) => Some(value),
                _ => None,
            })
            .unwrap()
    }
    let mut duplicate = plan.clone();
    let repeated = qualification_mut(&mut duplicate).sources[0].clone();
    qualification_mut(&mut duplicate).sources.push(repeated);
    assert!(matches!(
        duplicate.validate(),
        Err(CorpusPlanError::DuplicateQualificationSource(_))
    ));

    let mut missing_edge = plan.clone();
    missing_edge.dependencies.clear();
    assert!(matches!(
        missing_edge.validate(),
        Err(CorpusPlanError::ProvenanceDependencyMismatch { .. })
            | Err(CorpusPlanError::MissingQualificationDependency { .. })
    ));

    let mut identity_drift = plan.clone();
    qualification_mut(&mut identity_drift).sources[0]
        .case_binding
        .recipe_version = "2.0.0".into();
    assert!(matches!(
        identity_drift.validate(),
        Err(CorpusPlanError::QualificationSourceIdentityMismatch { .. })
    ));

    let mut public = plan;
    let PlannedArtifact::Dicom(source) = public
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.logical_id() == "private-source")
        .unwrap()
    else {
        unreachable!()
    };
    source.provenance = ArtifactProvenance::Requested;
    assert!(matches!(
        public.validate(),
        Err(CorpusPlanError::QualificationSourceNotPrivate(_))
    ));
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
        case_binding: CaseBinding {
            case_id: "negative/invalid".into(),
            recipe_id: "negative_invalid".into(),
            recipe_version: "1.0.0".into(),
        },
        source_artifact_id: "private-source".into(),
        mutation: MutationPlan {
            contract_version: "1.0.0".into(),
            source_identity: PlannedMutationSource {
                artifact_id: "private-source".into(),
                case_id: "classic/sc/source".into(),
                recipe_id: "source_recipe".into(),
                recipe_version: "1.0.0".into(),
                expected_sha256: "1".repeat(64),
            },
            operations: vec![PlannedMutationOperation {
                order: 0,
                operation_id: "truncate_dataset".into(),
                source_ranges: vec![PlannedByteRange {
                    start: 128,
                    end: 256,
                }],
                changed_byte_ranges: vec![PlannedChangedByteRange {
                    source: PlannedByteRange {
                        start: 128,
                        end: 256,
                    },
                    output: PlannedByteRange {
                        start: 128,
                        end: 128,
                    },
                }],
                expected_source_sha256: "1".repeat(64),
                expected_output_sha256: "2".repeat(64),
                expected_failure_layer: "dataset_parser".into(),
                acceptable_outcomes: vec!["clean_rejection".into()],
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
        case_binding: None,
        profile: None,
        run_seed: None,
        qualification_kind: "bounded_fuzz".into(),
        parameters: BTreeMap::new(),
        sources: vec![],
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
    fn mutate(plan: &mut CorpusPlan) -> &mut PlannedMutationArtifact {
        plan.artifacts
            .iter_mut()
            .find_map(|artifact| match artifact {
                PlannedArtifact::Mutation(mutation) => Some(mutation),
                _ => None,
            })
            .unwrap()
    }
    let mut broken = plan.clone();
    mutate(&mut broken).mutation.operations[0].order = 1;
    assert!(matches!(
        broken.validate(),
        Err(CorpusPlanError::MutationOperationOrder { .. })
    ));
    let mut broken = plan.clone();
    mutate(&mut broken).mutation.operations[0].expected_output_sha256 = "3".repeat(64);
    assert!(matches!(
        broken.validate(),
        Err(CorpusPlanError::MutationHashChainMismatch)
    ));
    let mut broken = plan.clone();
    mutate(&mut broken).mutation.operations[0].changed_byte_ranges[0]
        .source
        .start = 256;
    assert!(matches!(
        broken.validate(),
        Err(CorpusPlanError::MutationRangeContractMismatch(0))
    ));
    let mut broken = plan.clone();
    mutate(&mut broken).mutation.source_identity.artifact_id = "other-source".into();
    assert!(matches!(
        broken.validate(),
        Err(CorpusPlanError::MutationSourceArtifactMismatch { .. })
    ));
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
fn encoding_contract_serializes_distinct_length_file_meta_and_identity_fields() {
    let value = plan(
        vec![dicom("one", ArtifactProvenance::Requested, "one.dcm")],
        vec![],
    );
    let json: serde_json::Value =
        serde_json::from_slice(&value.canonical_bytes().unwrap()).unwrap();
    let encoding = &json["artifacts"][0]["encoding"];
    assert_eq!(json["schema_version"], CORPUS_PLAN_SCHEMA_VERSION);
    assert_eq!(encoding["sequence_length"], "writer_default");
    assert_eq!(encoding["item_length"], "writer_default");
    assert_eq!(encoding["file_meta"], "standard");
    assert!(encoding.get("dataset_length").is_none());
    assert_eq!(encoding["implementation"]["class_uid"], IMPLEMENTATION_UID);
}

#[test]
fn encoding_contract_rejects_invalid_fragment_offset_backend_and_zero_limit_matrix() {
    let base = dicom("one", ArtifactProvenance::Requested, "one.dcm");
    let expect_invalid = |mutate: fn(&mut EncodingPlan), expected: fn(&CorpusPlanError) -> bool| {
        let mut artifact = base.clone();
        let PlannedArtifact::Dicom(value) = &mut artifact else {
            unreachable!()
        };
        mutate(&mut value.encoding);
        let error = plan(vec![artifact], vec![]).validate().unwrap_err();
        assert!(expected(&error), "unexpected error: {error}");
    };
    expect_invalid(
        |encoding| encoding.offset_table = OffsetTablePolicy::EmptyBasic,
        |error| matches!(error, CorpusPlanError::InvalidEncodingCombination(_)),
    );
    let mut artifact = base.clone();
    let PlannedArtifact::Dicom(value) = &mut artifact else {
        unreachable!()
    };
    value.instance.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
    value.encoding.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
    value.encoding.fragmentation = FragmentationPolicy::FixedMaximumBytes { maximum_bytes: 0 };
    value.encoding.offset_table = OffsetTablePolicy::EmptyBasic;
    assert!(matches!(
        plan(vec![artifact], vec![]).validate(),
        Err(CorpusPlanError::ZeroFragmentSizeLimit)
    ));

    let mut artifact = base.clone();
    let PlannedArtifact::Dicom(value) = &mut artifact else {
        unreachable!()
    };
    value.instance.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
    value.encoding.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
    value.encoding.fragmentation = FragmentationPolicy::PreserveEncodedFrames;
    value.encoding.offset_table = OffsetTablePolicy::Extended;
    assert!(matches!(
        plan(vec![artifact], vec![]).validate(),
        Err(CorpusPlanError::InvalidEncodingCombination(_))
    ));

    let mut artifact = base.clone();
    let PlannedArtifact::Dicom(value) = &mut artifact else {
        unreachable!()
    };
    value.encoding.fragmentation = FragmentationPolicy::OneFragmentPerFrame;
    value.encoding.offset_table = OffsetTablePolicy::NotApplicable;
    assert!(matches!(
        plan(vec![artifact], vec![]).validate(),
        Err(CorpusPlanError::InvalidEncodingCombination(_))
    ));

    let mut artifact = base;
    let PlannedArtifact::Dicom(value) = &mut artifact else {
        unreachable!()
    };
    value.encoding.backend_id = "encoding.native.rle_lossless".into();
    assert!(matches!(
        plan(vec![artifact], vec![]).validate(),
        Err(CorpusPlanError::BackendTransferSyntaxMismatch { .. })
    ));
}

#[test]
fn encoding_contract_cross_checks_implementation_class_identity() {
    let mut artifact = dicom("one", ArtifactProvenance::Requested, "one.dcm");
    let PlannedArtifact::Dicom(value) = &mut artifact else {
        unreachable!()
    };
    value.encoding.implementation.class_uid = "2.25.999".into();
    assert!(matches!(
        plan(vec![artifact], vec![]).validate(),
        Err(CorpusPlanError::ImplementationIdentityMismatch { .. })
    ));

    let mut artifact = dicom("one", ArtifactProvenance::Requested, "one.dcm");
    let PlannedArtifact::Dicom(value) = &mut artifact else {
        unreachable!()
    };
    value.instance.identities = IdentityPlan::from_exact_values(
        "one",
        [(CompositionUidRole::SopInstance, 0, "2.25.1003".into())],
    )
    .unwrap();
    assert!(matches!(
        plan(vec![artifact], vec![]).validate(),
        Err(CorpusPlanError::MissingImplementationIdentity { .. })
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
