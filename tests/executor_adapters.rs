use std::collections::BTreeMap;

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION, CorpusPlan,
    EncodingPlan, EvidenceIndependence, EvidenceObligation, EvidencePlan, FileMetaPolicy,
    FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy,
    OutputPlan, OutputRelativePath, PlannedArtifact, PlannedDicomArtifact, PreamblePolicy,
    PublicationPlan, PublicationTransaction, ResourcePlan, SequenceLengthPolicy, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use dicom_test_suite::executor::adapters::{
    ArtifactServiceOutputs, CodecExecutionRecord, ProviderExecutionRecord, PublicationTransition,
    RunEvidenceAdapterInput, assemble_run_evidence, compatibility_projection_input,
};
use dicom_test_suite::executor::evidence::{
    EvidenceIndependence as ExecutionIndependence, ExecutionStatus, ObligationResult,
    PublicationState, ResultStatus,
};
use dicom_test_suite::executor::scheduler::{
    ActualResourceUsage, ResourceAccounting, ScheduleOutcome, ScheduledArtifact,
};
use dicom_test_suite::executor::services::{
    AssetVisibility, ByteBinding, CodecRequest, CodecResult, EncodedFrameResult,
    MaterializationResult, NativeFrameBinding, ProducedAsset, ProviderOutputExpectation,
    ProviderRequest, ProviderResult, RuleExecutionResult, ServiceEvidence, StagedAssetHandle,
    StagingRelativePath, ToolIdentity, ValidationResult as ServiceValidationResult,
    ValidationStatus,
};
use dicom_test_suite::sha256_hex;

const TS: &str = "1.2.840.10008.1.2.1";

fn tool(id: &str, executable_sha256: Option<String>) -> ToolIdentity {
    ToolIdentity {
        backend_id: id.into(),
        version: "1.0.0".into(),
        protocol_version: Some("1".into()),
        executable_sha256,
    }
}

fn planned(id: &str, order: u64, path: &str) -> PlannedArtifact {
    let instance = ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: id.into(),
        template_id: TemplateId("classic/secondary-capture/monochrome".into()),
        template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
        transfer_syntax_uid: TS.into(),
        identities: IdentityPlan::from_exact_values(
            id,
            [
                (
                    CompositionUidRole::SopInstance,
                    0,
                    format!("2.25.10{order}"),
                ),
                (CompositionUidRole::ImplementationClass, 0, "2.25.99".into()),
            ],
        )
        .unwrap(),
        attributes: vec![],
        content: vec![],
        references: vec![],
    };
    PlannedArtifact::Dicom(PlannedDicomArtifact {
        logical_id: id.into(),
        order,
        provenance: ArtifactProvenance::Requested,
        case_binding: None,
        instance,
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
                class_uid: "2.25.99".into(),
                version_name: Some("DICOMTS010".into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "part10.identity".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::from([("layer".into(), serde_json::json!("part10"))]),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: "same-project.part10".into(),
                route_id: "builtin.strict".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 100,
            peak_working_bytes: 200,
        },
    })
}

fn plan() -> CorpusPlan {
    CorpusPlan {
        schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed: 7,
        artifacts: vec![
            planned("first", 0, "first.dcm"),
            planned("second", 1, "second.dcm"),
        ],
        dependencies: vec![],
        unavailable: vec![],
        publication: PublicationPlan {
            manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
            transaction: PublicationTransaction::AtomicNoReplace,
            private_staging: true,
            no_overwrite: true,
        },
        resources: ResourcePlan {
            max_artifacts: 2,
            max_total_output_bytes: 1_000,
            max_peak_working_bytes: 500,
            max_parallelism: 2,
        },
    }
}

fn asset(handle: &str, path: &str, bytes: &[u8]) -> ProducedAsset {
    ProducedAsset::from_bytes(
        StagedAssetHandle::new(handle).unwrap(),
        StagingRelativePath::new(path).unwrap(),
        "application/dicom",
        bytes,
    )
}

fn outputs(id: &str, path: &str, bytes: &[u8], include_services: bool) -> ArtifactServiceOutputs {
    let mut output = asset(&format!("{id}-output"), path, bytes);
    output.declaration.visibility = AssetVisibility::PublicationCandidate;
    let materialization = MaterializationResult {
        artifact_id: id.into(),
        output: Some(output),
        backend: tool("dicom-rs.part10", None),
        evidence: vec![ServiceEvidence {
            evidence_id: "qualification_record".into(),
            evidence_kind: "bounded_qualification".into(),
            producer: tool("qualification-service", Some("e".repeat(64))),
            claims: BTreeMap::from([("candidate_count".into(), serde_json::json!(7))]),
        }],
    };
    let validation = ServiceValidationResult {
        artifact_id: id.into(),
        validator: tool("builtin.strict", None),
        rules: vec![RuleExecutionResult {
            rule_id: "part10.identity".into(),
            status: ValidationStatus::Passed,
            message: "identity is valid".into(),
            measurements: BTreeMap::new(),
        }],
        evidence: vec![],
    };
    let obligations = vec![ObligationResult {
        obligation_id: "same-project.part10".into(),
        route_id: "builtin.strict".into(),
        independence: ExecutionIndependence::SameProject,
        required: true,
        status: ResultStatus::Passed,
        message: "strict validation passed".into(),
        tool: None,
    }];
    let (providers, codecs) = if include_services {
        let provider_request = ProviderRequest {
            request_id: "provider-request".into(),
            artifact_id: id.into(),
            provider_id: "fixture-provider".into(),
            required_version: "1.0.0".into(),
            parameters: BTreeMap::from([("seed".into(), serde_json::json!(7))]),
            input_assets: BTreeMap::new(),
            expected_outputs: vec![
                ProviderOutputExpectation {
                    slot: "alpha".into(),
                    media_type: "application/dicom".into(),
                    maximum_size_bytes: 10,
                    expected_sha256: None,
                },
                ProviderOutputExpectation {
                    slot: "beta".into(),
                    media_type: "application/dicom".into(),
                    maximum_size_bytes: 10,
                    expected_sha256: None,
                },
            ],
        };
        let provider_result = ProviderResult {
            request_id: "provider-request".into(),
            provider: tool("fixture-provider", None),
            outputs: BTreeMap::from([
                (
                    "beta".into(),
                    asset("provider-beta", "private/beta.bin", b"b"),
                ),
                (
                    "alpha".into(),
                    asset("provider-alpha", "private/alpha.bin", b"a"),
                ),
            ]),
            evidence: vec![],
        };
        let frame_bytes = vec![1, 2, 3];
        let frame_hash = sha256_hex(&frame_bytes);
        let frame = NativeFrameBinding {
            frame_number: 1,
            bytes: ByteBinding::Inline {
                bytes: frame_bytes.clone(),
                sha256: frame_hash.clone(),
            },
            rows: 1,
            columns: 3,
            samples_per_pixel: 1,
            bits_allocated: 8,
            photometric_interpretation: "MONOCHROME2".into(),
        };
        let codec_request = CodecRequest {
            request_id: "codec-request".into(),
            artifact_id: id.into(),
            slot: "pixel_data".into(),
            backend_id: "fixture-codec".into(),
            source_transfer_syntax_uid: TS.into(),
            target_transfer_syntax_uid: "1.2.840.10008.1.2.4.50".into(),
            frames: vec![frame],
            parameters: BTreeMap::new(),
        };
        let codec_result = CodecResult {
            request_id: "codec-request".into(),
            backend: tool("fixture-codec", Some("c".repeat(64))),
            frames: vec![EncodedFrameResult {
                frame_number: 1,
                bytes: ByteBinding::Inline {
                    bytes: frame_bytes,
                    sha256: frame_hash.clone(),
                },
                encoded_size_bytes: 3,
                encoded_sha256: frame_hash,
            }],
            evidence: vec![],
        };
        (
            vec![ProviderExecutionRecord {
                request: provider_request,
                result: provider_result,
            }],
            vec![CodecExecutionRecord {
                request: codec_request,
                result: codec_result,
                backend_kind: "in_process".into(),
                display_name: "Fake codec".into(),
                feature_gate: None,
                determinism: "byte_stable".into(),
                decoded_frame_sha256: BTreeMap::from([(1, "d".repeat(64))]),
                metrics: BTreeMap::from([("psnr".into(), 42.0)]),
                claims: BTreeMap::new(),
            }],
        )
    } else {
        (vec![], vec![])
    };
    ArtifactServiceOutputs {
        status: ExecutionStatus::Succeeded,
        materialization: Some(materialization),
        validation: Some(validation),
        obligations,
        providers,
        codecs,
        elapsed_milliseconds: 5,
    }
}

#[test]
fn adapter_orders_records_and_preserves_typed_service_evidence() {
    let plan = plan();
    let first_bytes = b"first";
    let second_bytes = b"second";
    let outcome = ScheduleOutcome {
        artifacts: vec![
            ScheduledArtifact {
                logical_id: "second".into(),
                order: 1,
                value: outputs("second", "second.dcm", second_bytes, false),
                resources: ActualResourceUsage {
                    output_bytes: second_bytes.len() as u64,
                    peak_working_bytes: 20,
                },
            },
            ScheduledArtifact {
                logical_id: "first".into(),
                order: 0,
                value: outputs("first", "first.dcm", first_bytes, true),
                resources: ActualResourceUsage {
                    output_bytes: first_bytes.len() as u64,
                    peak_working_bytes: 20,
                },
            },
        ],
        planned: ResourceAccounting {
            artifact_count: 2,
            total_output_bytes: 200,
            peak_working_bytes: 200,
        },
        actual: ResourceAccounting {
            artifact_count: 2,
            total_output_bytes: (first_bytes.len() + second_bytes.len()) as u64,
            peak_working_bytes: 20,
        },
    };
    let evidence = assemble_run_evidence(
        &plan,
        outcome,
        RunEvidenceAdapterInput {
            requested_parallelism: 2,
            used_parallelism: 2,
            manifest_size_bytes: 100,
            publication: PublicationTransition::staging(),
        },
    )
    .unwrap();

    assert_eq!(
        evidence
            .artifacts
            .iter()
            .map(|a| a.logical_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(
        evidence.artifacts[0].corpus_plan_sha256,
        plan.canonical_sha256().unwrap()
    );
    let expected_instance_hash = match &plan.artifacts[0] {
        PlannedArtifact::Dicom(value) => Some(value.instance.canonical_sha256()),
        _ => None,
    };
    assert_eq!(
        evidence.artifacts[0].instance_plan_sha256,
        expected_instance_hash
    );
    assert_eq!(
        evidence.artifacts[0].providers[0]
            .outputs
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["alpha", "beta"]
    );
    assert_eq!(evidence.artifacts[0].providers[0].executable_sha256, None);
    assert_eq!(
        evidence.artifacts[0].codecs[0].encoded_frame_sha256.len(),
        1
    );
    assert_eq!(
        evidence.artifacts[0].codecs[0].decoded_frame_sha256,
        vec!["d".repeat(64)]
    );
    assert_eq!(evidence.artifacts[0].codecs[0].backend_kind, "in_process");
    assert_eq!(evidence.artifacts[0].codecs[0].display_name, "Fake codec");
    assert_eq!(evidence.artifacts[0].codecs[0].feature_gate, None);
    assert_eq!(evidence.artifacts[0].validation[0].layer, "part10");
    let service_evidence = &evidence.artifacts[0]
        .materialization
        .as_ref()
        .unwrap()
        .service_evidence[0];
    assert_eq!(service_evidence.evidence_id, "qualification_record");
    assert_eq!(service_evidence.evidence_kind, "bounded_qualification");
    assert_eq!(service_evidence.producer_id, "qualification-service");
    assert_eq!(service_evidence.producer_version, "1.0.0");
    assert_eq!(
        service_evidence.producer_executable_sha256,
        Some("e".repeat(64))
    );
    assert_eq!(service_evidence.claims["candidate_count"], 7);
    assert_eq!(
        evidence.artifacts[0].obligations[0].route_id,
        "builtin.strict"
    );

    let projection = compatibility_projection_input(&plan, &evidence).unwrap();
    assert_eq!(projection.artifacts[0].planned, plan.artifacts[0]);
    assert_eq!(
        projection.artifacts[0]
            .execution
            .output
            .as_ref()
            .unwrap()
            .sha256,
        sha256_hex(first_bytes)
    );
}

#[test]
fn publication_transitions_are_plan_bound_and_deterministic() {
    let plan = plan();
    let hash = "a".repeat(64);
    let staging = PublicationTransition::staging().for_plan(&plan);
    let ready = PublicationTransition::manifest_ready(hash.clone()).for_plan(&plan);
    let promoted = PublicationTransition::promoted(hash.clone()).for_plan(&plan);

    assert_eq!(staging.state, PublicationState::Staging);
    assert_eq!(ready.state, PublicationState::ManifestReady);
    assert!(ready.validation_complete);
    assert!(!ready.cleanup_complete);
    assert_eq!(promoted.state, PublicationState::Promoted);
    assert!(promoted.cleanup_complete);
    assert_eq!(promoted.manifest_sha256.as_deref(), Some(hash.as_str()));
    assert_eq!(promoted.manifest_relative_path, "manifest.json");
}
