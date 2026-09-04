//! Supported captured-corpus facade. No internal plan is part of this API.
use super::*;
use crate::corpus_definition::CorpusDefinitionBundle;
use crate::corpus_generation::{
    CapturedCorpusOutcome, CapturedCorpusRequest, CapturedPlanningOutcome, CorpusSelection,
};
use serde_json::Value;
use std::sync::Arc;

/// Explicit profile scope. Case IDs select direct cases; dependencies are added separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusSelector {
    Profile {
        profile: String,
        include_stress: bool,
    },
    CaseIds {
        profile: String,
        include_stress: bool,
        case_ids: Vec<String>,
    },
}
impl From<CorpusSelector> for CorpusSelection {
    fn from(value: CorpusSelector) -> Self {
        match value {
            CorpusSelector::Profile {
                profile,
                include_stress,
            } => Self::Profile {
                profile,
                include_stress,
            },
            CorpusSelector::CaseIds {
                profile,
                include_stress,
                case_ids,
            } => Self::CaseIds {
                profile,
                include_stress,
                case_ids,
            },
        }
    }
}
#[derive(Debug, Clone)]
enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// Inputs are captured when generation is called, not when this request is built.
/// Both constructors require the dedicated member root independently of descriptor location.
#[derive(Debug, Clone)]
pub struct GenerateCorpusRequest {
    source: Source,
    member_root: PathBuf,
    output_root: PathBuf,
    selector: CorpusSelector,
    seed: u64,
    parallelism: u32,
    dry_run: bool,
}
impl GenerateCorpusRequest {
    pub fn from_file(
        descriptor: impl Into<PathBuf>,
        member_root: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
        selector: CorpusSelector,
    ) -> Self {
        Self::new(
            Source::File(descriptor.into()),
            member_root.into(),
            output_root.into(),
            selector,
        )
    }
    pub fn from_json_bytes(
        descriptor: impl Into<Vec<u8>>,
        member_root: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
        selector: CorpusSelector,
    ) -> Self {
        Self::new(
            Source::Bytes(descriptor.into()),
            member_root.into(),
            output_root.into(),
            selector,
        )
    }
    fn new(
        source: Source,
        member_root: PathBuf,
        output_root: PathBuf,
        selector: CorpusSelector,
    ) -> Self {
        Self {
            source,
            member_root,
            output_root,
            selector,
            seed: 1,
            parallelism: 1,
            dry_run: false,
        }
    }
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
    pub fn with_parallelism(mut self, parallelism: u32) -> Self {
        self.parallelism = parallelism;
        self
    }
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusPublicationState {
    Published,
    NotRun,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusValidationState {
    Passed,
    NotRun,
}
/// `Ready` means planned, never generated or validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CorpusCaseDisposition {
    Ready,
    Generated,
    Unavailable,
    Planned,
    Blocked,
    Deprecated,
    Skipped,
}

/// Lossless manifest2-style case evidence, with typed disposition for control flow.
#[derive(Debug, Clone)]
pub struct CorpusCaseEvidence {
    row: Value,
    disposition: CorpusCaseDisposition,
}
impl CorpusCaseEvidence {
    pub fn case_id(&self) -> &str {
        self.row["case_id"].as_str().expect("validated case ID")
    }
    pub fn is_direct(&self) -> bool {
        self.row["selection"] == "direct"
    }
    pub fn disposition(&self) -> CorpusCaseDisposition {
        self.disposition
    }
    pub fn evidence(&self) -> &Value {
        &self.row
    }
}

/// SDK evidence-accessor contract 1.0.0, not a standalone JSON document schema.
/// Ledger and identity fields have manifest2 meanings; only previews permit `ready`.
/// No artifact payloads, private paths or internal plan serialization are exposed.
#[derive(Debug, Clone)]
pub struct CorpusPreview {
    output_root: PathBuf,
    selector: CorpusSelector,
    cases: Vec<CorpusCaseEvidence>,
    identities: Value,
    artifact_ids: Vec<String>,
    plan_sha256: String,
    seed: u64,
}
impl CorpusPreview {
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn evidence_version(&self) -> &str {
        "1.0.0"
    }
    pub fn requested_output_root(&self) -> &Path {
        &self.output_root
    }
    pub fn selector(&self) -> &CorpusSelector {
        &self.selector
    }
    pub fn cases(&self) -> &[CorpusCaseEvidence] {
        &self.cases
    }
    pub fn identity_projection(&self) -> &Value {
        &self.identities
    }
    pub fn artifact_ids(&self) -> &[String] {
        &self.artifact_ids
    }
    pub fn corpus_plan_sha256(&self) -> &str {
        &self.plan_sha256
    }
    pub fn publication_state(&self) -> CorpusPublicationState {
        CorpusPublicationState::NotRun
    }
    pub fn validation_state(&self) -> CorpusValidationState {
        CorpusValidationState::NotRun
    }
    fn from_internal(
        value: CapturedPlanningOutcome,
        output_root: PathBuf,
        selector: CorpusSelector,
    ) -> Result<Self, SdkError> {
        let cases = value
            .selection_ledger
            .into_iter()
            .map(|row| {
                let disposition = match row["outcome"].as_str() {
                    Some("ready") => CorpusCaseDisposition::Ready,
                    Some("unavailable") => CorpusCaseDisposition::Unavailable,
                    Some("planned") => CorpusCaseDisposition::Planned,
                    Some("blocked") => CorpusCaseDisposition::Blocked,
                    Some("deprecated") => CorpusCaseDisposition::Deprecated,
                    Some("skipped") => CorpusCaseDisposition::Skipped,
                    _ => {
                        return Err(SdkError::coded(
                            "internal.invariant.failed",
                            "unknown planning disposition",
                        ));
                    }
                };
                Ok(CorpusCaseEvidence { row, disposition })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            seed: value.plan.seed,
            output_root,
            selector,
            cases,
            identities: serde_json::to_value(value.identities)
                .map_err(|e| SdkError::coded("internal.serialization.failed", e))?,
            artifact_ids: value
                .plan
                .artifacts
                .iter()
                .map(|a| a.logical_id().to_owned())
                .collect(),
            plan_sha256: value
                .plan
                .canonical_sha256()
                .map_err(|e| SdkError::coded("generation.planning.failed", e))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PublishedCorpus {
    manifest: SchemaBoundManifest,
    emitted_file_count: usize,
    output_bytes: u64,
    plan_sha256: String,
}
impl PublishedCorpus {
    pub fn emitted_file_count(&self) -> usize {
        self.emitted_file_count
    }
    pub fn output_bytes(&self) -> u64 {
        self.output_bytes
    }
    pub fn corpus_plan_sha256(&self) -> &str {
        &self.plan_sha256
    }
    pub fn seed(&self) -> u64 {
        self.manifest.seed()
    }
    pub fn output_root(&self) -> &Path {
        self.manifest.path.parent().expect("manifest parent")
    }
    pub fn manifest(&self) -> &SchemaBoundManifest {
        &self.manifest
    }
    pub fn publication_state(&self) -> CorpusPublicationState {
        CorpusPublicationState::Published
    }
    /// Successful generation-time same-project checks, not independent conformance.
    pub fn validation_state(&self) -> CorpusValidationState {
        CorpusValidationState::Passed
    }
}
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum GenerateCorpusOutcome {
    Published(PublishedCorpus),
    Planned(CorpusPreview),
    NoExecutableCases(CorpusPreview),
}

impl DicomTestSuite {
    pub fn generate_corpus(
        &self,
        request: GenerateCorpusRequest,
    ) -> Result<GenerateCorpusOutcome, SdkError> {
        self.generate_corpus_cancellable(request, &CancellationToken::new())
    }
    pub fn generate_corpus_cancellable(
        &self,
        request: GenerateCorpusRequest,
        cancellation: &CancellationToken,
    ) -> Result<GenerateCorpusOutcome, SdkError> {
        if cancellation.assembly.is_cancelled() {
            return Err(SdkError::coded(
                "generation.execution.cancelled",
                "cancelled before corpus capture",
            ));
        }
        let bundle = match request.source {
            Source::File(path) => {
                CorpusDefinitionBundle::load_descriptor_file(path, &request.member_root)
            }
            Source::Bytes(bytes) => {
                CorpusDefinitionBundle::load_descriptor_bytes(&bytes, &request.member_root)
            }
        }
        .map_err(|e| SdkError::coded(e.code(), e))?;
        let outcome = crate::corpus_generation::run_captured_corpus(
            Arc::new(bundle),
            self.resources.clone(),
            CapturedCorpusRequest {
                selection: request.selector.clone().into(),
                destination: request.output_root.clone(),
                seed: request.seed,
                parallelism: request.parallelism,
                dry_run: request.dry_run,
                cancellation: cancellation.assembly.clone(),
            },
        )
        .map_err(|e| SdkError::coded(e.code(), e))?;
        match outcome {
            CapturedCorpusOutcome::Planned(value) => Ok(GenerateCorpusOutcome::Planned(
                CorpusPreview::from_internal(value, request.output_root, request.selector)?,
            )),
            CapturedCorpusOutcome::NoExecutableCases(value) => {
                Ok(GenerateCorpusOutcome::NoExecutableCases(
                    CorpusPreview::from_internal(value, request.output_root, request.selector)?,
                ))
            }
            CapturedCorpusOutcome::Published(value) => {
                let document: Value = serde_json::from_slice(&value.manifest_bytes)
                    .map_err(|e| SdkError::coded("validation.manifest.invalid", e))?;
                let files = document["files"]
                    .as_array()
                    .expect("validated manifest files");
                Ok(GenerateCorpusOutcome::Published(PublishedCorpus {
                    emitted_file_count: files.len(),
                    output_bytes: files
                        .iter()
                        .map(|file| file["size_bytes"].as_u64().expect("validated file size"))
                        .sum(),
                    plan_sha256: value.evidence.corpus_plan_sha256,
                    manifest: SchemaBoundManifest {
                        path: value.destination.join("manifest.json"),
                        schema_version: "2.0.0".into(),
                        kind: ManifestKind::ExternalCorpus,
                        seed: request.seed,
                        bytes: value.manifest_bytes,
                    },
                }))
            }
        }
    }
}
