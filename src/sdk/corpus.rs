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

/// Captured at inspection time. No destination is accepted or probed.
#[derive(Debug, Clone)]
pub struct InspectCorpusRequest {
    source: Source,
    member_root: PathBuf,
    selector: Option<CorpusSelector>,
    seed: u64,
    parallelism: u32,
}
impl InspectCorpusRequest {
    pub fn from_file(descriptor: impl Into<PathBuf>, member_root: impl Into<PathBuf>) -> Self {
        Self::new(Source::File(descriptor.into()), member_root.into())
    }
    pub fn from_json_bytes(
        descriptor: impl Into<Vec<u8>>,
        member_root: impl Into<PathBuf>,
    ) -> Self {
        Self::new(Source::Bytes(descriptor.into()), member_root.into())
    }
    fn new(source: Source, member_root: PathBuf) -> Self {
        Self {
            source,
            member_root,
            selector: None,
            seed: 1,
            parallelism: 1,
        }
    }
    pub fn with_selection(mut self, selector: CorpusSelector) -> Self {
        self.selector = Some(selector);
        self
    }
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
    pub fn with_parallelism(mut self, parallelism: u32) -> Self {
        self.parallelism = parallelism;
        self
    }
}

/// Registry status is definition metadata, not runtime availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CorpusCaseStatus {
    Implemented,
    Planned,
    Blocked,
    Deprecated,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct CorpusCaseSupport {
    row: Value,
    status: CorpusCaseStatus,
}
impl CorpusCaseSupport {
    pub fn case_id(&self) -> &str {
        self.row["case_id"].as_str().expect("verified ID")
    }
    pub fn status(&self) -> CorpusCaseStatus {
        self.status
    }
    pub fn profiles(&self) -> impl Iterator<Item = &str> {
        self.row["profiles"]
            .as_array()
            .expect("verified profiles")
            .iter()
            .map(|p| p.as_str().unwrap())
    }
    /// Complete declared provider, requirements, standards, blockers, and skip facts.
    pub fn definition(&self) -> &Value {
        &self.row
    }
}

#[derive(Debug, Clone)]
pub struct CorpusProfileSupport {
    definition: Value,
}
impl CorpusProfileSupport {
    pub fn profile_id(&self) -> &str {
        self.definition["profile_id"].as_str().unwrap()
    }
    pub fn scope(&self) -> &str {
        self.definition["scope"].as_str().unwrap()
    }
    pub fn definition(&self) -> &Value {
        &self.definition
    }
}

/// Selected planning facts only: `Ready` is not generated or validated evidence.
#[derive(Debug, Clone)]
pub struct CorpusAssessment {
    selector: CorpusSelector,
    seed: u64,
    parallelism: u32,
    cases: Vec<CorpusCaseEvidence>,
    identity_projection: Value,
    artifact_ids: Vec<String>,
    plan_sha256: String,
}
impl CorpusAssessment {
    pub fn selector(&self) -> &CorpusSelector {
        &self.selector
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn parallelism(&self) -> u32 {
        self.parallelism
    }
    pub fn cases(&self) -> &[CorpusCaseEvidence] {
        &self.cases
    }
    pub fn identity_projection(&self) -> &Value {
        &self.identity_projection
    }
    pub fn artifact_ids(&self) -> &[String] {
        &self.artifact_ids
    }
    pub fn corpus_plan_sha256(&self) -> &str {
        &self.plan_sha256
    }
    pub fn has_executable_artifacts(&self) -> bool {
        !self.artifact_ids.is_empty()
    }
    pub fn validation_state(&self) -> CorpusValidationState {
        CorpusValidationState::NotRun
    }
    pub fn publication_state(&self) -> CorpusPublicationState {
        CorpusPublicationState::NotRun
    }
}

/// SDK accessor evidence, not a standalone serialized document. An absent
/// assessment means runtime/selection support has not been assessed.
#[derive(Debug, Clone)]
pub struct CorpusInspection {
    identity: Value,
    pub(crate) identity_domains: crate::identity::InstalledIdentityDomains,
    profiles: Vec<CorpusProfileSupport>,
    cases: Vec<CorpusCaseSupport>,
    assessment: Option<CorpusAssessment>,
}
impl CorpusInspection {
    pub fn evidence_version(&self) -> &str {
        "1.0.0"
    }
    pub fn corpus_definition_identity(&self) -> &Value {
        &self.identity
    }
    pub fn profiles(&self) -> &[CorpusProfileSupport] {
        &self.profiles
    }
    pub fn cases(&self) -> &[CorpusCaseSupport] {
        &self.cases
    }
    pub fn assessment(&self) -> Option<&CorpusAssessment> {
        self.assessment.as_ref()
    }
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
    pub fn reason_code(&self) -> Option<&str> {
        self.row["reason_code"].as_str()
    }
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
        let cases = planning_cases(value.selection_ledger)?;
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

fn planning_cases(rows: Vec<Value>) -> Result<Vec<CorpusCaseEvidence>, SdkError> {
    rows.into_iter()
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
        .collect()
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
    /// Installed capabilities plus one captured caller corpus; no input reload.
    pub fn capabilities_with_corpus(
        &self,
        request: InspectCorpusRequest,
    ) -> Result<CapabilitiesResult, SdkError> {
        let inspection = self.inspect_corpus(request)?;
        let mut result = self.capabilities()?;
        result.identity_domains = inspection.identity_domains.clone();
        let assessment = inspection.assessment.as_ref().map(|value| {
            let (profile, include_stress, selector) = match &value.selector {
                CorpusSelector::Profile { profile, include_stress } => (profile, include_stress, serde_json::json!({"kind":"profile"})),
                CorpusSelector::CaseIds { profile, include_stress, case_ids } => {
                    let mut ids = case_ids.clone(); ids.sort();
                    (profile, include_stress, serde_json::json!({"kind":"case_ids","case_ids":ids}))
                }
            };
            serde_json::json!({"selector":selector,"profile":profile,"include_stress":include_stress,"seed":value.seed,"parallelism":value.parallelism,"selection_ledger":value.cases.iter().map(|c| c.evidence()).collect::<Vec<_>>(),"identity_projection":value.identity_projection,"artifact_ids":value.artifact_ids,"corpus_plan_sha256":value.plan_sha256,"has_executable_artifacts":value.has_executable_artifacts(),"validation":"not_run","publication":"not_run"})
        });
        result.loaded_corpus = Some(
            serde_json::json!({"inspection_schema_version":"1.0.0","corpus_definition_identity":inspection.identity,"profiles":inspection.profiles.iter().map(|p| p.definition()).collect::<Vec<_>>(),"cases":inspection.cases.iter().map(|c| c.definition()).collect::<Vec<_>>(),"assessment_state":if assessment.is_some(){"assessed"}else{"not_assessed"},"assessment":assessment}),
        );
        Ok(result)
    }
    pub fn inspect_corpus(
        &self,
        request: InspectCorpusRequest,
    ) -> Result<CorpusInspection, SdkError> {
        self.inspect_corpus_cancellable(request, &CancellationToken::new())
    }
    pub fn inspect_corpus_cancellable(
        &self,
        request: InspectCorpusRequest,
        cancellation: &CancellationToken,
    ) -> Result<CorpusInspection, SdkError> {
        let checkpoint = || {
            if cancellation.is_cancelled() {
                Err(SdkError::coded(
                    "generation.execution.cancelled",
                    "corpus inspection cancelled",
                ))
            } else {
                Ok(())
            }
        };
        checkpoint()?;
        if request.parallelism == 0
            || (request.selector.is_none() && (request.seed != 1 || request.parallelism != 1))
        {
            return Err(SdkError::coded(
                "request.schema.invalid",
                "positive parallelism and a selection for planning options are required",
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
        checkpoint()?;
        let registry: Value = serde_json::from_slice(
            bundle
                .bytes(&bundle.manifest().registry.path)
                .expect("verified registry"),
        )
        .map_err(|e| SdkError::coded("internal.invariant.failed", e))?;
        let cases = registry["cases"]
            .as_array()
            .expect("verified cases")
            .iter()
            .map(|row| {
                let status = match row["status"].as_str() {
                    Some("implemented") => CorpusCaseStatus::Implemented,
                    Some("planned") => CorpusCaseStatus::Planned,
                    Some("blocked") => CorpusCaseStatus::Blocked,
                    Some("deprecated") => CorpusCaseStatus::Deprecated,
                    Some("skipped") => CorpusCaseStatus::Skipped,
                    _ => {
                        return Err(SdkError::coded(
                            "internal.invariant.failed",
                            "unknown verified registry status",
                        ));
                    }
                };
                Ok(CorpusCaseSupport {
                    row: row.clone(),
                    status,
                })
            })
            .collect::<Result<Vec<_>, SdkError>>()?;
        let profiles = bundle
            .manifest()
            .profiles
            .iter()
            .map(|profile| {
                serde_json::to_value(profile)
                    .map(|definition| CorpusProfileSupport { definition })
                    .map_err(|e| SdkError::coded("internal.invariant.failed", e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let identity = serde_json::to_value(bundle.identity())
            .map_err(|e| SdkError::coded("internal.invariant.failed", e))?;
        let identity_domains = crate::identity::project_installed_identities(
            &self.resources,
            crate::identity::IdentityInspectionContext {
                corpus_definition: Some(&bundle),
            },
        )
        .map_err(|e| SdkError::coded("evidence.integrity.failed", e))?;
        let assessment = if let Some(selector) = request.selector {
            let prepared = crate::corpus_generation::prepare_captured_corpus(
                &bundle,
                &self.resources,
                &selector.clone().into(),
                request.seed,
                request.parallelism,
                &cancellation.assembly,
            )
            .map_err(|e| SdkError::coded(e.code(), e))?;
            let value = prepared.preview;
            let cases = planning_cases(value.selection_ledger)?;
            let identity_projection = serde_json::to_value(value.identities)
                .map_err(|e| SdkError::coded("internal.invariant.failed", e))?;
            let plan_sha256 = value
                .plan
                .canonical_sha256()
                .map_err(|e| SdkError::coded("generation.planning.failed", e))?;
            let artifact_ids = value
                .plan
                .artifacts
                .iter()
                .map(|artifact| artifact.logical_id().to_owned())
                .collect();
            Some(CorpusAssessment {
                selector,
                seed: request.seed,
                parallelism: request.parallelism,
                cases,
                identity_projection,
                artifact_ids,
                plan_sha256,
            })
        } else {
            None
        };
        checkpoint()?;
        Ok(CorpusInspection {
            identity,
            identity_domains,
            profiles,
            cases,
            assessment,
        })
    }
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
