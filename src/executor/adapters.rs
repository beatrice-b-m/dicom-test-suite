//! Deterministic adapters from executor service records to run evidence.
//!
//! This module is deliberately filesystem-free.  Every byte identity used by
//! evidence or a manifest projector comes from a validated staged-asset
//! declaration returned by an execution service.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::corpus_plan::{
    CapabilityKind, CorpusPlan, EvidenceIndependence as PlannedIndependence, EvidencePlan,
    PlannedArtifact, ValidationPlan, ValidationRequirement,
};
use crate::executor::evidence::{
    ArtifactExecutionEvidence, ArtifactKind, ArtifactResourceEvidence, CodecEvidence,
    EvidenceError, EvidenceIndependence, ExecutionStatus, ImportedDicomObservation,
    MaterializationEvidence, MaterializationServiceEvidence, MaterializedContentEvidence,
    ObligationResult, OutputEvidence, ProviderEvidence, PublicationEvidence, PublicationState,
    RUN_EVIDENCE_SCHEMA_VERSION, ResultStatus, RunEvidence, RunResourceEvidence, ToolEvidence,
    UnavailableExecutionEvidence, ValidationResult,
};
use crate::executor::scheduler::{ScheduleOutcome, ScheduledArtifact};
use crate::executor::services::{
    AssetVisibility, CodecRequest, CodecResult, MaterializationResult, ProviderRequest,
    ProviderResult, ToolIdentity, ValidationResult as ServiceValidationResult, ValidationStatus,
};
use crate::sha256_hex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionRecord {
    pub request: ProviderRequest,
    pub result: ProviderResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecExecutionRecord {
    pub request: CodecRequest,
    pub result: CodecResult,
    #[serde(default)]
    pub backend_kind: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub feature_gate: Option<String>,
    pub determinism: String,
    #[serde(default)]
    pub decoded_frame_sha256: BTreeMap<u32, String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub claims: BTreeMap<String, Value>,
}

/// Complete service-side outcome for one scheduled artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactServiceOutputs {
    pub status: ExecutionStatus,
    pub materialization: Option<MaterializationResult>,
    pub validation: Option<ServiceValidationResult>,
    #[serde(default)]
    pub obligations: Vec<ObligationResult>,
    #[serde(default)]
    pub providers: Vec<ProviderExecutionRecord>,
    #[serde(default)]
    pub codecs: Vec<CodecExecutionRecord>,
    pub elapsed_milliseconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationTransition {
    pub state: PublicationState,
    pub validation_complete: bool,
    pub cleanup_complete: bool,
    pub manifest_sha256: Option<String>,
}

impl PublicationTransition {
    pub fn not_started() -> Self {
        Self {
            state: PublicationState::NotStarted,
            validation_complete: false,
            cleanup_complete: false,
            manifest_sha256: None,
        }
    }

    pub fn staging() -> Self {
        Self {
            state: PublicationState::Staging,
            ..Self::not_started()
        }
    }

    pub fn manifest_ready(manifest_sha256: impl Into<String>) -> Self {
        Self {
            state: PublicationState::ManifestReady,
            validation_complete: true,
            cleanup_complete: false,
            manifest_sha256: Some(manifest_sha256.into()),
        }
    }

    pub fn promoted(manifest_sha256: impl Into<String>) -> Self {
        Self {
            state: PublicationState::Promoted,
            validation_complete: true,
            cleanup_complete: true,
            manifest_sha256: Some(manifest_sha256.into()),
        }
    }

    pub fn cancelled(cleanup_complete: bool) -> Self {
        Self {
            state: PublicationState::Cancelled,
            validation_complete: false,
            cleanup_complete,
            manifest_sha256: None,
        }
    }

    pub fn failed(cleanup_complete: bool) -> Self {
        Self {
            state: PublicationState::Failed,
            validation_complete: false,
            cleanup_complete,
            manifest_sha256: None,
        }
    }

    pub fn for_plan(&self, plan: &CorpusPlan) -> PublicationEvidence {
        PublicationEvidence {
            manifest_relative_path: plan.publication.manifest_path.as_str().to_owned(),
            state: self.state,
            private_staging: plan.publication.private_staging,
            no_overwrite: plan.publication.no_overwrite,
            validation_complete: self.validation_complete,
            cleanup_complete: self.cleanup_complete,
            manifest_sha256: self.manifest_sha256.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEvidenceAdapterInput {
    pub requested_parallelism: u32,
    pub used_parallelism: u32,
    /// Size of the manifest added after artifact execution.
    pub manifest_size_bytes: u64,
    pub publication: PublicationTransition,
}

/// Filesystem-free typed input shared by terminal manifest projectors.
///
/// The planned artifact retains recipe/instance/reference semantics, while the
/// paired execution record supplies observed byte identities and all evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProjectionInput {
    pub corpus_plan_sha256: String,
    pub artifacts: Vec<ManifestProjectionArtifact>,
    pub unavailable: Vec<UnavailableExecutionEvidence>,
    pub resources: RunResourceEvidence,
    pub publication: PublicationEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProjectionArtifact {
    pub planned: PlannedArtifact,
    pub execution: ArtifactExecutionEvidence,
}

pub fn assemble_run_evidence(
    plan: &CorpusPlan,
    outcome: ScheduleOutcome<ArtifactServiceOutputs>,
    input: RunEvidenceAdapterInput,
) -> Result<RunEvidence, AdapterError> {
    plan.validate().map_err(AdapterError::InvalidPlan)?;
    let plan_sha256 = plan.canonical_sha256().map_err(AdapterError::InvalidPlan)?;
    let mut scheduled = outcome
        .artifacts
        .into_iter()
        .map(|artifact| (artifact.logical_id.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    if scheduled.len() != plan.artifacts.len() {
        return Err(AdapterError::ArtifactSetMismatch);
    }

    let mut artifacts = Vec::with_capacity(plan.artifacts.len());
    for planned in &plan.artifacts {
        let scheduled_artifact = scheduled
            .remove(planned.logical_id())
            .ok_or_else(|| AdapterError::MissingArtifact(planned.logical_id().to_owned()))?;
        artifacts.push(adapt_artifact(planned, scheduled_artifact, &plan_sha256)?);
    }
    if !scheduled.is_empty() {
        return Err(AdapterError::ArtifactSetMismatch);
    }

    let actual_artifact_output_bytes = artifacts.iter().try_fold(0_u64, |total, artifact| {
        total
            .checked_add(artifact.resources.actual_output_bytes)
            .ok_or(AdapterError::ResourceOverflow)
    })?;
    if actual_artifact_output_bytes != outcome.actual.total_output_bytes {
        return Err(AdapterError::SchedulerOutputMismatch {
            scheduler: outcome.actual.total_output_bytes,
            evidence: actual_artifact_output_bytes,
        });
    }
    if outcome.actual.artifact_count != artifacts.len() as u64 {
        return Err(AdapterError::ArtifactSetMismatch);
    }
    let planned_output_bytes = plan.artifacts.iter().try_fold(0_u64, |total, artifact| {
        total
            .checked_add(artifact.resource_estimate().output_bytes)
            .ok_or(AdapterError::ResourceOverflow)
    })?;
    let planned_peak_working_bytes = plan
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().peak_working_bytes)
        .max()
        .unwrap_or(0);
    if outcome.planned.artifact_count != plan.artifacts.len() as u64
        || outcome.planned.total_output_bytes != planned_output_bytes
        || outcome.planned.peak_working_bytes != planned_peak_working_bytes
    {
        return Err(AdapterError::SchedulerPlanMismatch);
    }
    let actual_publication_bytes = actual_artifact_output_bytes
        .checked_add(input.manifest_size_bytes)
        .ok_or(AdapterError::ResourceOverflow)?;
    let resources = RunResourceEvidence {
        planned_max_artifacts: plan.resources.max_artifacts,
        planned_max_total_output_bytes: plan.resources.max_total_output_bytes,
        planned_max_peak_working_bytes: plan.resources.max_peak_working_bytes,
        requested_parallelism: input.requested_parallelism,
        used_parallelism: input.used_parallelism,
        actual_artifact_output_bytes,
        actual_publication_bytes,
        actual_peak_working_bytes: Some(outcome.actual.peak_working_bytes),
    };
    let unavailable = plan
        .unavailable
        .iter()
        .map(|capability| UnavailableExecutionEvidence {
            capability_id: capability.capability_id.clone(),
            kind: capability_kind_name(&capability.kind).to_owned(),
            reason_code: capability.reason_code.clone(),
            message: capability.message.clone(),
            affected_artifact_ids: sorted_unique(&capability.affected_artifact_ids),
        })
        .collect();
    let evidence = RunEvidence {
        schema_version: RUN_EVIDENCE_SCHEMA_VERSION.to_owned(),
        corpus_plan_sha256: plan_sha256,
        artifacts,
        unavailable,
        resources,
        publication: input.publication.for_plan(plan),
    };
    let order = plan
        .artifacts
        .iter()
        .map(|artifact| artifact.logical_id().to_owned())
        .collect::<Vec<_>>();
    evidence.validate(&order).map_err(AdapterError::Evidence)?;
    Ok(evidence)
}

pub fn manifest_projection_input(
    plan: &CorpusPlan,
    evidence: &RunEvidence,
) -> Result<ManifestProjectionInput, AdapterError> {
    let expected_hash = plan.canonical_sha256().map_err(AdapterError::InvalidPlan)?;
    if evidence.corpus_plan_sha256 != expected_hash {
        return Err(AdapterError::PlanHashMismatch);
    }
    let by_id = evidence
        .artifacts
        .iter()
        .map(|artifact| (artifact.logical_id.as_str(), artifact))
        .collect::<BTreeMap<_, _>>();
    let mut artifacts = Vec::with_capacity(plan.artifacts.len());
    for planned in &plan.artifacts {
        let execution = by_id
            .get(planned.logical_id())
            .ok_or_else(|| AdapterError::MissingArtifact(planned.logical_id().to_owned()))?;
        artifacts.push(ManifestProjectionArtifact {
            planned: planned.clone(),
            execution: (*execution).clone(),
        });
    }
    if by_id.len() != artifacts.len() {
        return Err(AdapterError::ArtifactSetMismatch);
    }
    Ok(ManifestProjectionInput {
        corpus_plan_sha256: expected_hash,
        artifacts,
        unavailable: evidence.unavailable.clone(),
        resources: evidence.resources.clone(),
        publication: evidence.publication.clone(),
    })
}

fn adapt_artifact(
    planned: &PlannedArtifact,
    scheduled: ScheduledArtifact<ArtifactServiceOutputs>,
    plan_sha256: &str,
) -> Result<ArtifactExecutionEvidence, AdapterError> {
    if scheduled.logical_id != planned.logical_id() || scheduled.order != planned.order() {
        return Err(AdapterError::ArtifactIdentityMismatch(
            planned.logical_id().to_owned(),
        ));
    }
    let (validation_plan, evidence_plan) = artifact_contracts(planned);
    let validation = adapt_validation(
        planned.logical_id(),
        validation_plan,
        scheduled.value.validation.as_ref(),
    )?;
    let obligations = adapt_obligations(evidence_plan, scheduled.value.obligations)?;
    let streamed_slots = scheduled
        .value
        .codecs
        .iter()
        .map(|record| record.request.slot.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut providers = scheduled
        .value
        .providers
        .into_iter()
        .map(adapt_provider)
        .collect::<Result<Vec<_>, _>>()?;
    providers.sort_by(|left, right| {
        (&left.provider_id, &left.request_sha256).cmp(&(&right.provider_id, &right.request_sha256))
    });
    let mut codecs = scheduled
        .value
        .codecs
        .into_iter()
        .map(adapt_codec)
        .collect::<Result<Vec<_>, _>>()?;
    codecs.sort_by(|left, right| {
        (&left.slot, &left.backend_id, &left.transfer_syntax_uid).cmp(&(
            &right.slot,
            &right.backend_id,
            &right.transfer_syntax_uid,
        ))
    });

    let output = match (&scheduled.value.materialization, planned.output()) {
        (Some(result), Some(planned_output)) => {
            if result.artifact_id != planned.logical_id() {
                return Err(AdapterError::ArtifactIdentityMismatch(
                    planned.logical_id().to_owned(),
                ));
            }
            let asset = result.output.as_ref().ok_or_else(|| {
                AdapterError::MissingMaterializedOutput(planned.logical_id().to_owned())
            })?;
            asset
                .validate()
                .map_err(AdapterError::InvalidServiceRecord)?;
            if asset.declaration.relative_path.as_str() != planned_output.relative_path.as_str() {
                return Err(AdapterError::OutputPathMismatch(
                    planned.logical_id().to_owned(),
                ));
            }
            let expected_visibility = if planned_output.publish {
                AssetVisibility::PublicationCandidate
            } else {
                AssetVisibility::Private
            };
            if asset.declaration.visibility != expected_visibility {
                return Err(AdapterError::OutputVisibilityMismatch(
                    planned.logical_id().to_owned(),
                ));
            }
            Some(OutputEvidence {
                relative_path: asset.declaration.relative_path.as_str().to_owned(),
                publish: planned_output.publish,
                size_bytes: asset.observed_size_bytes,
                sha256: asset.observed_sha256.clone(),
            })
        }
        (None, None) => None,
        (Some(result), None)
            if matches!(
                planned,
                PlannedArtifact::Qualification(value)
                    if value.resources.output_bytes == 0
                        && matches!(
                            value.payload_policy,
                            crate::corpus_plan::QualificationPayloadPolicy::NoPayload
                                | crate::corpus_plan::QualificationPayloadPolicy::EvidenceOnly
                        )
            ) && result.output.is_none() =>
        {
            if result.artifact_id != planned.logical_id() {
                return Err(AdapterError::ArtifactIdentityMismatch(
                    planned.logical_id().to_owned(),
                ));
            }
            None
        }
        (Some(_), None) => {
            return Err(AdapterError::UnexpectedMaterializedOutput(
                planned.logical_id().to_owned(),
            ));
        }
        (None, Some(_)) if scheduled.value.status == ExecutionStatus::Succeeded => {
            return Err(AdapterError::MissingMaterializedOutput(
                planned.logical_id().to_owned(),
            ));
        }
        (None, Some(_)) => None,
    };
    let actual_output_bytes = output.as_ref().map_or(0, |output| output.size_bytes);
    if actual_output_bytes != scheduled.resources.output_bytes {
        return Err(AdapterError::ArtifactOutputMismatch(
            planned.logical_id().to_owned(),
        ));
    }
    let materialization = scheduled
        .value
        .materialization
        .as_ref()
        .map(|result| -> Result<MaterializationEvidence, AdapterError> {
            Ok(MaterializationEvidence {
                backend_id: result.backend.backend_id.clone(),
                transfer_syntax_uid: match planned {
                    PlannedArtifact::Dicom(value) => {
                        Some(value.encoding.transfer_syntax_uid.clone())
                    }
                    PlannedArtifact::ImportedDicom(value) => {
                        Some(value.provider.transfer_syntax_uid.clone())
                    }
                    _ => None,
                },
                streamed_slots,
                completed: scheduled.value.status == ExecutionStatus::Succeeded,
                materialized_instance_plan_sha256: service_claim(
                    result,
                    "materialized_instance_plan_sha256",
                )
                .and_then(Value::as_str)
                .map(str::to_owned),
                materialized_encoding_sha256: service_claim(result, "materialized_encoding_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                materialized_artifact_sha256: service_claim(result, "materialized_artifact_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                preamble_policy: service_claim(result, "preamble_policy")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                preamble_sha256: service_claim(result, "preamble_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                file_meta_policy: service_claim(result, "file_meta_policy")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                file_meta_sha256: service_claim(result, "file_meta_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                file_meta_size_bytes: service_claim(result, "file_meta_size_bytes")
                    .and_then(Value::as_u64),
                implementation_class_uid: service_claim(result, "implementation_class_uid")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                implementation_version_name: service_claim(result, "implementation_version_name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                content: service_claim(result, "materialized_content")
                    .cloned()
                    .map(serde_json::from_value::<Vec<MaterializedContentEvidence>>)
                    .transpose()
                    .map_err(AdapterError::Serialize)?
                    .unwrap_or_default(),
                imported_dicom: service_claim(result, "imported_dicom_observation")
                    .cloned()
                    .map(serde_json::from_value::<ImportedDicomObservation>)
                    .transpose()
                    .map_err(AdapterError::Serialize)?,
                service_evidence: result
                    .evidence
                    .iter()
                    .map(|evidence| MaterializationServiceEvidence {
                        evidence_id: evidence.evidence_id.clone(),
                        evidence_kind: evidence.evidence_kind.clone(),
                        producer_id: evidence.producer.backend_id.clone(),
                        producer_version: evidence.producer.version.clone(),
                        producer_executable_sha256: evidence.producer.executable_sha256.clone(),
                        claims: evidence.claims.clone(),
                    })
                    .collect(),
            })
        })
        .transpose()?;
    let (artifact_kind, instance_plan_sha256) = match planned {
        PlannedArtifact::Dicom(value) => {
            (ArtifactKind::Dicom, Some(value.instance.canonical_sha256()))
        }
        PlannedArtifact::ImportedDicom(value) => (
            ArtifactKind::Dicom,
            Some(value.declared_instance.canonical_sha256()),
        ),
        PlannedArtifact::Mutation(_) => (ArtifactKind::Mutation, None),
        PlannedArtifact::Qualification(_) => (ArtifactKind::Qualification, None),
        PlannedArtifact::Auxiliary(_) => (ArtifactKind::Auxiliary, None),
    };
    Ok(ArtifactExecutionEvidence {
        logical_id: planned.logical_id().to_owned(),
        order: planned.order(),
        artifact_kind,
        status: scheduled.value.status,
        corpus_plan_sha256: plan_sha256.to_owned(),
        instance_plan_sha256,
        output,
        materialization,
        validation,
        obligations,
        providers,
        codecs,
        resources: ArtifactResourceEvidence {
            planned_output_bytes: planned.resource_estimate().output_bytes,
            planned_peak_working_bytes: planned.resource_estimate().peak_working_bytes,
            actual_output_bytes,
            actual_peak_working_bytes: Some(scheduled.resources.peak_working_bytes),
            elapsed_milliseconds: scheduled.value.elapsed_milliseconds,
        },
    })
}

fn adapt_validation(
    artifact_id: &str,
    plan: &ValidationPlan,
    result: Option<&ServiceValidationResult>,
) -> Result<Vec<ValidationResult>, AdapterError> {
    let Some(result) = result else {
        return if plan.rules.is_empty() {
            Ok(Vec::new())
        } else {
            Err(AdapterError::MissingValidation(artifact_id.to_owned()))
        };
    };
    if result.artifact_id != artifact_id {
        return Err(AdapterError::ArtifactIdentityMismatch(
            artifact_id.to_owned(),
        ));
    }
    let by_id = result
        .rules
        .iter()
        .map(|rule| (rule.rule_id.as_str(), rule))
        .collect::<BTreeMap<_, _>>();
    if by_id.len() != plan.rules.len() {
        return Err(AdapterError::ValidationSetMismatch(artifact_id.to_owned()));
    }
    plan.rules
        .iter()
        .map(|planned| {
            let actual = by_id
                .get(planned.rule_id.as_str())
                .ok_or_else(|| AdapterError::ValidationSetMismatch(artifact_id.to_owned()))?;
            Ok(ValidationResult {
                rule_id: planned.rule_id.clone(),
                layer: planned
                    .parameters
                    .get("layer")
                    .and_then(|value| value.as_str())
                    .unwrap_or("generic")
                    .to_owned(),
                required: planned.requirement != ValidationRequirement::CapabilityConditional,
                status: match actual.status {
                    ValidationStatus::Passed => ResultStatus::Passed,
                    ValidationStatus::Failed => ResultStatus::Failed,
                    ValidationStatus::Unavailable => ResultStatus::Unavailable,
                },
                message: actual.message.clone(),
                details: actual.measurements.clone(),
            })
        })
        .collect()
}

fn adapt_obligations(
    plan: &EvidencePlan,
    supplied: Vec<ObligationResult>,
) -> Result<Vec<ObligationResult>, AdapterError> {
    let mut by_id = supplied
        .into_iter()
        .map(|result| (result.obligation_id.clone(), result))
        .collect::<BTreeMap<_, _>>();
    if by_id.len() != plan.obligations.len() {
        return Err(AdapterError::ObligationSetMismatch);
    }
    plan.obligations
        .iter()
        .map(|planned| {
            let actual = by_id
                .remove(&planned.obligation_id)
                .ok_or(AdapterError::ObligationSetMismatch)?;
            if actual.route_id != planned.route_id
                || actual.required != planned.required
                || actual.independence != independence(planned.independence)
            {
                return Err(AdapterError::ObligationSetMismatch);
            }
            Ok(actual)
        })
        .collect()
}

fn adapt_provider(record: ProviderExecutionRecord) -> Result<ProviderEvidence, AdapterError> {
    record
        .result
        .validate(&record.request)
        .map_err(AdapterError::InvalidServiceRecord)?;
    if record.result.request_id != record.request.request_id
        || record.result.provider.backend_id != record.request.provider_id
        || record.result.provider.version != record.request.required_version
    {
        return Err(AdapterError::ServiceIdentityMismatch(
            record.request.request_id,
        ));
    }
    let outputs = record
        .result
        .outputs
        .iter()
        .map(|(slot, asset)| {
            asset
                .validate()
                .map_err(AdapterError::InvalidServiceRecord)?;
            Ok((slot.clone(), asset.observed_sha256.clone()))
        })
        .collect::<Result<BTreeMap<_, _>, AdapterError>>()?;
    let claims = record
        .result
        .evidence
        .iter()
        .flat_map(|evidence| evidence.claims.clone())
        .collect();
    Ok(ProviderEvidence {
        provider_id: record.result.provider.backend_id.clone(),
        provider_version: record.result.provider.version.clone(),
        status: ResultStatus::Passed,
        executable_sha256: record.result.provider.executable_sha256.clone(),
        argument_sha256: canonical_hash(&record.request.parameters)?,
        request_sha256: canonical_hash(&record.request)?,
        response_sha256: canonical_hash(&record.result)?,
        outputs,
        claims,
    })
}

fn adapt_codec(mut record: CodecExecutionRecord) -> Result<CodecEvidence, AdapterError> {
    if record.result.request_id != record.request.request_id
        || record.result.backend.backend_id != record.request.backend_id
    {
        return Err(AdapterError::ServiceIdentityMismatch(
            record.request.request_id,
        ));
    }
    record.result.frames.sort_by_key(|frame| frame.frame_number);
    let encoded_frame_sha256 = record
        .result
        .frames
        .iter()
        .map(|frame| frame.encoded_sha256.clone())
        .collect();
    Ok(CodecEvidence {
        backend_id: record.result.backend.backend_id.clone(),
        backend_version: record.result.backend.version.clone(),
        backend_kind: record.backend_kind,
        display_name: record.display_name,
        feature_gate: record.feature_gate,
        slot: record.request.slot.clone(),
        request_sha256: canonical_hash(&record.request)?,
        transfer_syntax_uid: record.request.target_transfer_syntax_uid,
        status: ResultStatus::Passed,
        determinism: record.determinism,
        encoded_frame_sha256,
        decoded_frame_sha256: record.decoded_frame_sha256.into_values().collect(),
        metrics: record.metrics,
        claims: record.claims,
        tool: tool_evidence(&record.result.backend),
    })
}

fn tool_evidence(identity: &ToolIdentity) -> Option<ToolEvidence> {
    identity
        .executable_sha256
        .as_ref()
        .map(|digest| ToolEvidence {
            tool_id: identity.backend_id.clone(),
            version: identity.version.clone(),
            executable_sha256: digest.clone(),
        })
}

fn service_claim<'a>(result: &'a MaterializationResult, key: &str) -> Option<&'a Value> {
    result
        .evidence
        .iter()
        .find_map(|evidence| evidence.claims.get(key))
}

fn artifact_contracts(artifact: &PlannedArtifact) -> (&ValidationPlan, &EvidencePlan) {
    match artifact {
        PlannedArtifact::Dicom(value) => (&value.validation, &value.evidence),
        PlannedArtifact::ImportedDicom(value) => (&value.validation, &value.evidence),
        PlannedArtifact::Mutation(value) => (&value.validation, &value.evidence),
        PlannedArtifact::Qualification(value) => (&value.validation, &value.evidence),
        PlannedArtifact::Auxiliary(value) => (&value.validation, &value.evidence),
    }
}

fn canonical_hash(value: &impl Serialize) -> Result<String, AdapterError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(AdapterError::Serialize)
}

fn independence(value: PlannedIndependence) -> EvidenceIndependence {
    match value {
        PlannedIndependence::SameProject => EvidenceIndependence::SameProject,
        PlannedIndependence::IndependentTool => EvidenceIndependence::IndependentTool,
        PlannedIndependence::ExternalProvider => EvidenceIndependence::ExternalProvider,
    }
}

fn capability_kind_name(kind: &CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Feature => "feature",
        CapabilityKind::Codec => "codec",
        CapabilityKind::Provider => "provider",
        CapabilityKind::ExternalBackend => "external_backend",
        CapabilityKind::Validator => "validator",
        CapabilityKind::ResourceScale => "resource_scale",
        CapabilityKind::Platform => "platform",
    }
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug)]
pub enum AdapterError {
    InvalidPlan(crate::corpus_plan::CorpusPlanError),
    Evidence(EvidenceError),
    InvalidServiceRecord(crate::executor::services::ServiceError),
    Serialize(serde_json::Error),
    MissingArtifact(String),
    ArtifactSetMismatch,
    ArtifactIdentityMismatch(String),
    MissingMaterializedOutput(String),
    UnexpectedMaterializedOutput(String),
    OutputPathMismatch(String),
    OutputVisibilityMismatch(String),
    ArtifactOutputMismatch(String),
    SchedulerOutputMismatch { scheduler: u64, evidence: u64 },
    SchedulerPlanMismatch,
    MissingValidation(String),
    ValidationSetMismatch(String),
    ObligationSetMismatch,
    ServiceIdentityMismatch(String),
    PlanHashMismatch,
    ResourceOverflow,
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AdapterError {}
