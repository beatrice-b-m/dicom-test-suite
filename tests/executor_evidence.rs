use dicom_test_suite::executor::cancellation::{
    CancellationPoint, CancellationStage, CancellationToken,
};
use dicom_test_suite::executor::evidence::*;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn artifact(logical_id: &str, order: u64, path: &str) -> ArtifactExecutionEvidence {
    ArtifactExecutionEvidence {
        logical_id: logical_id.into(),
        order,
        artifact_kind: ArtifactKind::Dicom,
        status: ExecutionStatus::Succeeded,
        corpus_plan_sha256: HASH.into(),
        instance_plan_sha256: Some(HASH.into()),
        output: Some(OutputEvidence {
            relative_path: path.into(),
            publish: true,
            size_bytes: 10,
            sha256: HASH.into(),
        }),
        materialization: Some(MaterializationEvidence {
            backend_id: "part10_materializer".into(),
            transfer_syntax_uid: Some("1.2.840.10008.1.2.1".into()),
            streamed_slots: vec!["pixels".into()],
            completed: true,
            materialized_instance_plan_sha256: Some(HASH.into()),
            materialized_encoding_sha256: Some(HASH.into()),
            materialized_artifact_sha256: Some(HASH.into()),
            preamble_policy: Some("zero_filled".into()),
            preamble_sha256: Some(HASH.into()),
            file_meta_policy: Some("standard".into()),
            file_meta_sha256: Some(HASH.into()),
            file_meta_size_bytes: Some(256),
            implementation_class_uid: Some("2.25.100".into()),
            implementation_version_name: Some("DICOMTS010".into()),
            content: vec![],
            imported_dicom: None,
        }),
        validation: vec![ValidationResult {
            rule_id: "meta_identity".into(),
            layer: "part10".into(),
            required: true,
            status: ResultStatus::Passed,
            message: "identity matches".into(),
            details: BTreeMap::new(),
        }],
        obligations: vec![ObligationResult {
            obligation_id: "same_project_validation".into(),
            route_id: "generic_plan".into(),
            independence: EvidenceIndependence::SameProject,
            required: true,
            status: ResultStatus::Passed,
            message: "route passed".into(),
            tool: None,
        }],
        providers: vec![],
        codecs: vec![],
        resources: ArtifactResourceEvidence {
            planned_output_bytes: 10,
            planned_peak_working_bytes: 1024,
            actual_output_bytes: 10,
            actual_peak_working_bytes: Some(512),
            elapsed_milliseconds: 1,
        },
    }
}

fn evidence(artifacts: Vec<ArtifactExecutionEvidence>) -> RunEvidence {
    let output_bytes = artifacts
        .iter()
        .map(|artifact| artifact.resources.actual_output_bytes)
        .sum();
    RunEvidence {
        schema_version: RUN_EVIDENCE_SCHEMA_VERSION.into(),
        corpus_plan_sha256: HASH.into(),
        artifacts,
        unavailable: vec![UnavailableExecutionEvidence {
            capability_id: "optional_codec".into(),
            kind: "codec".into(),
            reason_code: "feature_disabled".into(),
            message: "optional codec is unavailable".into(),
            affected_artifact_ids: vec!["future_artifact".into()],
        }],
        resources: RunResourceEvidence {
            planned_max_artifacts: 10,
            planned_max_total_output_bytes: 1024,
            planned_max_peak_working_bytes: 1024,
            requested_parallelism: 4,
            used_parallelism: 2,
            actual_artifact_output_bytes: output_bytes,
            actual_publication_bytes: output_bytes + 100,
            actual_peak_working_bytes: Some(512),
        },
        publication: PublicationEvidence {
            manifest_relative_path: "manifest.json".into(),
            state: PublicationState::Promoted,
            private_staging: true,
            no_overwrite: true,
            validation_complete: true,
            cleanup_complete: true,
            manifest_sha256: Some(HASH.into()),
        },
    }
}

#[test]
fn run_evidence_binds_plan_hashes_actuals_unavailability_and_stable_order() {
    let evidence = evidence(vec![
        artifact("source", 0, "instances/source.dcm"),
        artifact("derived", 1, "instances/derived.dcm"),
    ]);
    let order = vec!["source".into(), "derived".into()];
    evidence.validate(&order).unwrap();
    let encoded = serde_json::to_vec(&evidence).unwrap();
    let decoded: RunEvidence = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, evidence);
}

#[test]
fn run_evidence_rejects_order_hash_resource_and_publication_drift() {
    let mut value = evidence(vec![artifact("source", 0, "instances/source.dcm")]);
    assert!(matches!(
        value.validate(&["other".into()]),
        Err(EvidenceError::ArtifactOrderMismatch { .. })
    ));

    value.artifacts[0].instance_plan_sha256 = Some("bad".into());
    assert!(matches!(
        value.validate(&["source".into()]),
        Err(EvidenceError::InvalidSha256 { .. })
    ));

    value.artifacts[0].instance_plan_sha256 = Some(HASH.into());
    value.resources.actual_artifact_output_bytes = 9;
    assert!(matches!(
        value.validate(&["source".into()]),
        Err(EvidenceError::ArtifactOutputTotalMismatch { .. })
    ));

    value.resources.actual_artifact_output_bytes = 10;
    value.publication.cleanup_complete = false;
    assert_eq!(
        value.validate(&["source".into()]),
        Err(EvidenceError::IncompletePromotionEvidence)
    );
}

#[test]
fn cancellation_is_shared_first_reason_wins_and_checkpoint_is_typed() {
    let token = CancellationToken::new();
    let worker = token.clone();
    assert!(!worker.is_cancelled());
    assert!(token.cancel_with_reason("caller requested stop"));
    assert!(!worker.cancel_with_reason("later reason"));
    assert_eq!(worker.reason().as_deref(), Some("caller requested stop"));

    let point = CancellationPoint::artifact(CancellationStage::BeforeMaterialization, "derived");
    let error = worker.checkpoint(point.clone()).unwrap_err();
    assert_eq!(error.point, point);
    assert_eq!(error.reason, "caller requested stop");
}

#[test]
fn uncancelled_checkpoint_passes_and_default_cancel_reason_is_stable() {
    let token = CancellationToken::new();
    token
        .checkpoint(CancellationPoint::run(CancellationStage::BeforeExecution))
        .unwrap();
    assert!(token.cancel());
    assert_eq!(token.reason().as_deref(), Some("requested"));
}
use std::collections::BTreeMap;
