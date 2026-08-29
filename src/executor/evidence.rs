use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const RUN_EVIDENCE_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunEvidence {
    pub schema_version: String,
    pub corpus_plan_sha256: String,
    pub artifacts: Vec<ArtifactExecutionEvidence>,
    #[serde(default)]
    pub unavailable: Vec<UnavailableExecutionEvidence>,
    pub resources: RunResourceEvidence,
    pub publication: PublicationEvidence,
}

impl RunEvidence {
    pub fn validate(&self, expected_artifact_order: &[String]) -> Result<(), EvidenceError> {
        if self.schema_version != RUN_EVIDENCE_SCHEMA_VERSION {
            return Err(EvidenceError::UnsupportedSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        validate_sha256("corpus plan", &self.corpus_plan_sha256)?;
        let actual_order = self
            .artifacts
            .iter()
            .map(|artifact| artifact.logical_id.clone())
            .collect::<Vec<_>>();
        if actual_order != expected_artifact_order {
            return Err(EvidenceError::ArtifactOrderMismatch {
                expected: expected_artifact_order.to_vec(),
                actual: actual_order,
            });
        }

        let mut logical_ids = BTreeSet::new();
        let mut orders = BTreeSet::new();
        let mut output_paths = BTreeSet::new();
        let mut artifact_output_bytes = 0_u64;
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !logical_ids.insert(&artifact.logical_id) {
                return Err(EvidenceError::DuplicateArtifact(
                    artifact.logical_id.clone(),
                ));
            }
            if !orders.insert(artifact.order) {
                return Err(EvidenceError::DuplicateArtifactOrder(artifact.order));
            }
            if let Some(output) = &artifact.output {
                if !output_paths.insert(&output.relative_path) {
                    return Err(EvidenceError::DuplicateOutputPath(
                        output.relative_path.clone(),
                    ));
                }
                artifact_output_bytes = artifact_output_bytes
                    .checked_add(output.size_bytes)
                    .ok_or(EvidenceError::ResourceOverflow)?;
            }
        }
        if artifact_output_bytes != self.resources.actual_artifact_output_bytes {
            return Err(EvidenceError::ArtifactOutputTotalMismatch {
                expected: artifact_output_bytes,
                actual: self.resources.actual_artifact_output_bytes,
            });
        }
        self.resources.validate()?;

        let mut unavailable_ids = BTreeSet::new();
        for unavailable in &self.unavailable {
            unavailable.validate()?;
            if !unavailable_ids.insert(&unavailable.capability_id) {
                return Err(EvidenceError::DuplicateUnavailableCapability(
                    unavailable.capability_id.clone(),
                ));
            }
        }
        self.publication.validate()?;
        if self.publication.state == PublicationState::Promoted {
            if self
                .artifacts
                .iter()
                .any(|artifact| artifact.status != ExecutionStatus::Succeeded)
            {
                return Err(EvidenceError::PromotedWithIncompleteArtifact);
            }
            if !self.publication.cleanup_complete || self.publication.manifest_sha256.is_none() {
                return Err(EvidenceError::IncompletePromotionEvidence);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Dicom,
    Mutation,
    Qualification,
    Auxiliary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
    Cancelled,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactExecutionEvidence {
    pub logical_id: String,
    pub order: u64,
    pub artifact_kind: ArtifactKind,
    pub status: ExecutionStatus,
    pub corpus_plan_sha256: String,
    pub instance_plan_sha256: Option<String>,
    pub output: Option<OutputEvidence>,
    pub materialization: Option<MaterializationEvidence>,
    #[serde(default)]
    pub validation: Vec<ValidationResult>,
    #[serde(default)]
    pub obligations: Vec<ObligationResult>,
    #[serde(default)]
    pub providers: Vec<ProviderEvidence>,
    #[serde(default)]
    pub codecs: Vec<CodecEvidence>,
    pub resources: ArtifactResourceEvidence,
}

impl ArtifactExecutionEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("artifact logical ID", &self.logical_id)?;
        validate_sha256("artifact corpus plan", &self.corpus_plan_sha256)?;
        if let Some(hash) = &self.instance_plan_sha256 {
            validate_sha256("instance plan", hash)?;
        }
        if self.artifact_kind == ArtifactKind::Dicom && self.instance_plan_sha256.is_none() {
            return Err(EvidenceError::MissingInstancePlanHash(
                self.logical_id.clone(),
            ));
        }
        if let Some(output) = &self.output {
            output.validate()?;
        }
        if let Some(materialization) = &self.materialization {
            materialization.validate()?;
        }
        if self.status == ExecutionStatus::Succeeded
            && self.artifact_kind == ArtifactKind::Dicom
            && (self.output.is_none() || self.materialization.is_none())
        {
            return Err(EvidenceError::IncompleteDicomEvidence(
                self.logical_id.clone(),
            ));
        }
        validate_unique_results(
            "validation rule",
            self.validation.iter().map(|result| &result.rule_id),
        )?;
        validate_unique_results(
            "evidence obligation",
            self.obligations.iter().map(|result| &result.obligation_id),
        )?;
        for result in &self.validation {
            result.validate()?;
        }
        for result in &self.obligations {
            result.validate()?;
        }
        for provider in &self.providers {
            provider.validate()?;
        }
        for codec in &self.codecs {
            codec.validate()?;
        }
        self.resources.validate()?;
        if let Some(output) = &self.output {
            if output.size_bytes != self.resources.actual_output_bytes {
                return Err(EvidenceError::ArtifactResourceMismatch {
                    logical_id: self.logical_id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputEvidence {
    pub relative_path: String,
    pub publish: bool,
    pub size_bytes: u64,
    pub sha256: String,
}

impl OutputEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_relative_path(&self.relative_path)?;
        validate_sha256("output", &self.sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationEvidence {
    pub backend_id: String,
    pub transfer_syntax_uid: Option<String>,
    #[serde(default)]
    pub streamed_slots: Vec<String>,
    pub completed: bool,
    pub materialized_instance_plan_sha256: Option<String>,
    /// Hash of the encoding policy together with the execution-observed
    /// fragmentation/table facts. This deliberately excludes payload bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized_encoding_sha256: Option<String>,
    /// Hash of the completed staged artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized_artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_meta_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_meta_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_meta_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_class_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_version_name: Option<String>,
    #[serde(default)]
    pub content: Vec<MaterializedContentEvidence>,
}

impl MaterializationEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("materialization backend", &self.backend_id)?;
        validate_unique_results("streamed slot", self.streamed_slots.iter())?;
        if let Some(hash) = &self.materialized_instance_plan_sha256 {
            validate_sha256("materialized instance plan", hash)?;
        }
        if let Some(hash) = &self.materialized_encoding_sha256 {
            validate_sha256("materialized encoding", hash)?;
        }
        if let Some(hash) = &self.materialized_artifact_sha256 {
            validate_sha256("materialized artifact", hash)?;
        }
        if let Some(hash) = &self.preamble_sha256 {
            validate_sha256("Part 10 preamble", hash)?;
        }
        if let Some(hash) = &self.file_meta_sha256 {
            validate_sha256("Part 10 File Meta", hash)?;
        }
        validate_unique_results(
            "materialized content slot",
            self.content.iter().map(|c| &c.slot),
        )?;
        for content in &self.content {
            validate_identifier("materialized content kind", &content.kind)?;
            validate_identifier("materialized content VR", &content.vr)?;
            validate_sha256("materialized content", &content.sha256)?;
            for hash in &content.compressed_frame_sha256 {
                validate_sha256("compressed frame", hash)?;
            }
            for hash in &content.native_frame_sha256 {
                validate_sha256("native frame", hash)?;
            }
            for hash in &content.decoded_frame_sha256 {
                validate_sha256("decoded frame", hash)?;
            }
            if content.native_frame_sha256.len() != content.native_frame_lengths.len()
                || content.decoded_frame_sha256.len() != content.decoded_frame_lengths.len()
            {
                return Err(EvidenceError::FrameEvidenceCardinality(
                    content.slot.clone(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedContentEvidence {
    pub slot: String,
    pub kind: String,
    pub vr: String,
    pub size_bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub basic_offset_table: Vec<u32>,
    #[serde(default)]
    pub compressed_frame_sha256: Vec<String>,
    /// Execution-observed hashes of the ordered native frame bindings. For
    /// continuously packed U1, these retain the binding chunk identities.
    #[serde(default)]
    pub native_frame_sha256: Vec<String>,
    #[serde(default)]
    pub native_frame_lengths: Vec<u64>,
    /// Hashes of logical decoded frames. U1 frames are expanded to one 0/1
    /// byte per sample from the continuous LSB-first aggregate.
    #[serde(default)]
    pub decoded_frame_sha256: Vec<String>,
    #[serde(default)]
    pub decoded_frame_lengths: Vec<u64>,
    #[serde(default)]
    pub fragment_count: u64,
    #[serde(default)]
    pub compressed_lengths: Vec<u64>,
    #[serde(default)]
    pub padded_fragment_lengths: Vec<u64>,
    #[serde(default)]
    pub extended_offset_table: Vec<u64>,
    #[serde(default)]
    pub extended_offset_table_lengths: Vec<u64>,
    /// How the writer consumed this slot when execution used a distinct
    /// materialization path (for example, bounded file streaming).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer_materialization: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Passed,
    Failed,
    Unavailable,
    NotRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationResult {
    pub rule_id: String,
    pub layer: String,
    pub required: bool,
    pub status: ResultStatus,
    pub message: String,
    #[serde(default)]
    pub details: BTreeMap<String, serde_json::Value>,
}

impl ValidationResult {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("validation rule", &self.rule_id)?;
        validate_identifier("validation layer", &self.layer)?;
        validate_message("validation result", &self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceIndependence {
    SameProject,
    IndependentTool,
    ExternalProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationResult {
    pub obligation_id: String,
    pub route_id: String,
    pub independence: EvidenceIndependence,
    pub required: bool,
    pub status: ResultStatus,
    pub message: String,
    pub tool: Option<ToolEvidence>,
}

impl ObligationResult {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("obligation", &self.obligation_id)?;
        validate_identifier("evidence route", &self.route_id)?;
        validate_message("obligation result", &self.message)?;
        if let Some(tool) = &self.tool {
            tool.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolEvidence {
    pub tool_id: String,
    pub version: String,
    pub executable_sha256: String,
}

impl ToolEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("tool ID", &self.tool_id)?;
        validate_identifier("tool version", &self.version)?;
        validate_sha256("tool executable", &self.executable_sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvidence {
    pub provider_id: String,
    pub provider_version: String,
    pub status: ResultStatus,
    pub executable_sha256: Option<String>,
    pub argument_sha256: String,
    pub request_sha256: String,
    pub response_sha256: String,
    pub outputs: BTreeMap<String, String>,
    #[serde(default)]
    pub claims: BTreeMap<String, serde_json::Value>,
}

impl ProviderEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("provider ID", &self.provider_id)?;
        validate_identifier("provider version", &self.provider_version)?;
        if let Some(hash) = &self.executable_sha256 {
            validate_sha256("provider executable", hash)?;
        }
        for (label, hash) in [
            ("provider arguments", &self.argument_sha256),
            ("provider request", &self.request_sha256),
            ("provider response", &self.response_sha256),
        ] {
            validate_sha256(label, hash)?;
        }
        if self.outputs.is_empty() {
            return Err(EvidenceError::EmptyProviderOutputs);
        }
        for (slot, hash) in &self.outputs {
            validate_identifier("provider output slot", slot)?;
            validate_sha256("provider output", hash)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecEvidence {
    pub backend_id: String,
    pub backend_version: String,
    pub slot: String,
    pub request_sha256: String,
    pub transfer_syntax_uid: String,
    pub status: ResultStatus,
    pub determinism: String,
    #[serde(default)]
    pub encoded_frame_sha256: Vec<String>,
    #[serde(default)]
    pub decoded_frame_sha256: Vec<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub claims: BTreeMap<String, serde_json::Value>,
    pub tool: Option<ToolEvidence>,
}

impl CodecEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("codec backend", &self.backend_id)?;
        validate_identifier("codec version", &self.backend_version)?;
        validate_identifier("codec slot", &self.slot)?;
        validate_sha256("codec request", &self.request_sha256)?;
        validate_identifier("codec determinism", &self.determinism)?;
        for hash in self
            .encoded_frame_sha256
            .iter()
            .chain(&self.decoded_frame_sha256)
        {
            validate_sha256("codec frame", hash)?;
        }
        if self.metrics.values().any(|value| !value.is_finite()) {
            return Err(EvidenceError::NonFiniteCodecMetric);
        }
        if let Some(tool) = &self.tool {
            tool.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactResourceEvidence {
    pub planned_output_bytes: u64,
    pub planned_peak_working_bytes: u64,
    pub actual_output_bytes: u64,
    pub actual_peak_working_bytes: Option<u64>,
    pub elapsed_milliseconds: u64,
}

impl ArtifactResourceEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        if self.actual_output_bytes > self.planned_output_bytes {
            return Err(EvidenceError::ArtifactOutputLimitExceeded);
        }
        if self
            .actual_peak_working_bytes
            .is_some_and(|actual| actual > self.planned_peak_working_bytes)
        {
            return Err(EvidenceError::ArtifactWorkingLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResourceEvidence {
    pub planned_max_artifacts: u64,
    pub planned_max_total_output_bytes: u64,
    pub planned_max_peak_working_bytes: u64,
    pub requested_parallelism: u32,
    pub used_parallelism: u32,
    pub actual_artifact_output_bytes: u64,
    pub actual_publication_bytes: u64,
    pub actual_peak_working_bytes: Option<u64>,
}

impl RunResourceEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        if self.planned_max_artifacts == 0
            || self.planned_max_total_output_bytes == 0
            || self.planned_max_peak_working_bytes == 0
            || self.requested_parallelism == 0
            || self.used_parallelism == 0
            || self.used_parallelism > self.requested_parallelism
        {
            return Err(EvidenceError::InvalidRunResourceEnvelope);
        }
        if self.actual_publication_bytes > self.planned_max_total_output_bytes
            || self.actual_artifact_output_bytes > self.actual_publication_bytes
            || self
                .actual_peak_working_bytes
                .is_some_and(|actual| actual > self.planned_max_peak_working_bytes)
        {
            return Err(EvidenceError::RunResourceLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableExecutionEvidence {
    pub capability_id: String,
    pub kind: String,
    pub reason_code: String,
    pub message: String,
    #[serde(default)]
    pub affected_artifact_ids: Vec<String>,
}

impl UnavailableExecutionEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("capability ID", &self.capability_id)?;
        validate_identifier("capability kind", &self.kind)?;
        validate_identifier("unavailable reason", &self.reason_code)?;
        validate_message("unavailable capability", &self.message)?;
        validate_unique_results("unavailable artifact", self.affected_artifact_ids.iter())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationState {
    NotStarted,
    Staging,
    ManifestReady,
    Promoted,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationEvidence {
    pub manifest_relative_path: String,
    pub state: PublicationState,
    pub private_staging: bool,
    pub no_overwrite: bool,
    pub validation_complete: bool,
    pub cleanup_complete: bool,
    pub manifest_sha256: Option<String>,
}

impl PublicationEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_relative_path(&self.manifest_relative_path)?;
        if !self.private_staging || !self.no_overwrite {
            return Err(EvidenceError::UnsafePublication);
        }
        if let Some(hash) = &self.manifest_sha256 {
            validate_sha256("manifest", hash)?;
        }
        if matches!(
            self.state,
            PublicationState::ManifestReady | PublicationState::Promoted
        ) && (!self.validation_complete || self.manifest_sha256.is_none())
        {
            return Err(EvidenceError::IncompleteManifestEvidence);
        }
        Ok(())
    }
}

fn validate_unique_results<'a>(
    label: &'static str,
    values: impl IntoIterator<Item = &'a String>,
) -> Result<(), EvidenceError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_identifier(label, value)?;
        if !unique.insert(value) {
            return Err(EvidenceError::DuplicateResult {
                label,
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn validate_identifier(label: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.is_empty()
        || value.len() > 256
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(EvidenceError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_message(label: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(EvidenceError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_sha256(label: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvidenceError::InvalidSha256 {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), EvidenceError> {
    let path = std::path::Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::CurDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(EvidenceError::UnsafeRelativePath(value.to_owned()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    UnsupportedSchemaVersion(String),
    ArtifactOrderMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    DuplicateArtifact(String),
    DuplicateArtifactOrder(u64),
    DuplicateOutputPath(String),
    MissingInstancePlanHash(String),
    IncompleteDicomEvidence(String),
    DuplicateResult {
        label: &'static str,
        value: String,
    },
    InvalidIdentifier {
        label: &'static str,
        value: String,
    },
    InvalidSha256 {
        label: &'static str,
        value: String,
    },
    FrameEvidenceCardinality(String),
    UnsafeRelativePath(String),
    ArtifactOutputLimitExceeded,
    ArtifactWorkingLimitExceeded,
    InvalidRunResourceEnvelope,
    RunResourceLimitExceeded,
    ResourceOverflow,
    ArtifactResourceMismatch {
        logical_id: String,
    },
    ArtifactOutputTotalMismatch {
        expected: u64,
        actual: u64,
    },
    DuplicateUnavailableCapability(String),
    NonFiniteCodecMetric,
    EmptyProviderOutputs,
    UnsafePublication,
    IncompleteManifestEvidence,
    PromotedWithIncompleteArtifact,
    IncompletePromotionEvidence,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EvidenceError {}
