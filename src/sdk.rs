//! Supported Rust integration facade for the standalone product.
//!
//! Modules outside `sdk` remain available during the productization migration,
//! but only this facade is the supported Rust compatibility surface.
//!
//! ```no_run
//! use synth_dicom_gen::sdk::{ComposeRequest, DicomTestSuite, ValidateRequest};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let product = DicomTestSuite::embedded()?;
//! let spec = br#"{
//!   "composition_spec_schema_version":"0.1.0",
//!   "instances":[{"instance_id":"primary","template":{
//!     "id":"classic/secondary-capture/monochrome"
//!   }}]
//! }"#;
//! let output = std::env::temp_dir().join("synth-dicom-gen-sdk-example");
//! let outcome = product.compose(ComposeRequest::from_json_bytes(spec.as_slice(), ".", &output))?;
//! let validation = product.validate(ValidateRequest::new(outcome.output_root()))?;
//! assert!(validation.is_valid());
//! # std::fs::remove_dir_all(output)?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::discovery::{CapabilitiesResult, VersionResult};
use crate::engine_resources::EngineResources;

/// A relocatable product handle backed by an integrity-checked resource set.
#[derive(Debug, Clone)]
pub struct DicomTestSuite {
    resources: EngineResources,
}

impl DicomTestSuite {
    /// Construct a product using the immutable resources embedded in the crate.
    pub fn embedded() -> Result<Self, SdkError> {
        Self::from_resources(EngineResources::embedded())
    }

    /// Construct a product from an explicit resource root.
    ///
    /// The root must contain the complete, byte-identical product resource set.
    /// There is no fallback to embedded or repository-relative resources.
    pub fn explicit_resource_root(root: impl AsRef<Path>) -> Result<Self, SdkError> {
        let resources = EngineResources::explicit(root.as_ref().to_path_buf())
            .map_err(|error| SdkError::classify("capabilities", error))?;
        Self::from_resources(resources)
    }

    fn from_resources(resources: EngineResources) -> Result<Self, SdkError> {
        resources
            .verify_integrity()
            .map_err(|error| SdkError::classify("capabilities", error))?;
        Ok(Self { resources })
    }

    /// Return typed product, build, CLI, feature, and resource identity.
    pub fn version(&self) -> Result<VersionResult, SdkError> {
        crate::discovery::version_result(&self.resources)
            .map_err(|error| SdkError::classify("version", error))
    }

    /// Return typed live capabilities without converting absence into support.
    pub fn capabilities(&self) -> Result<CapabilitiesResult, SdkError> {
        crate::discovery::capabilities_result(&self.resources)
            .map_err(|error| SdkError::classify("capabilities", error))
    }

    /// Execute a qualified composition request through the shared product pipeline.
    pub fn compose(&self, request: ComposeRequest) -> Result<ComposeOutcome, SdkError> {
        self.compose_cancellable(request, &CancellationToken::new())
    }

    /// Execute a qualified composition request with cooperative cancellation.
    pub fn compose_cancellable(
        &self,
        request: ComposeRequest,
        cancellation: &CancellationToken,
    ) -> Result<ComposeOutcome, SdkError> {
        let spec_bytes = match request.source {
            ComposeSource::File(path) => std::fs::read(&path).map_err(|error| {
                SdkError::classify(
                    "compose",
                    format!(
                        "composition input read failed at {}: {error}",
                        path.display()
                    ),
                )
            })?,
            ComposeSource::Bytes(bytes) => bytes,
        };
        let options = crate::composition::ComposeBytesOptions {
            spec_root: request.caller_asset_root,
            out_dir: request.output_root,
            seed: request.seed,
            catalog_path: PathBuf::from(crate::engine_resources::TEMPLATE_CATALOG_RESOURCE),
            dry_run: request.dry_run,
        };
        let (summary, document) =
            crate::composition::compose_from_bytes_with_cancellation_and_resources(
                &spec_bytes,
                &options,
                &cancellation.inner,
                &self.resources,
            )
            .map_err(|error| SdkError::classify("compose", error))?;

        let plan_preview = summary.dry_run.then(|| PlanPreview {
            artifact_ids: document
                .get("plans")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|plan| plan.get("instance_id").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .collect(),
        });
        let manifest = if summary.dry_run {
            None
        } else {
            Some(SchemaBoundManifest::load(
                &summary.out_dir,
                &self.resources,
                "compose",
            )?)
        };
        Ok(ComposeOutcome {
            output_root: summary.out_dir,
            seed: request.seed,
            published: !summary.dry_run,
            instances_written: summary.instances_written,
            output_bytes: summary.output_bytes,
            corpus_plan_sha256: summary.corpus_plan_sha256,
            manifest,
            plan_preview,
        })
    }

    /// Validate a published product output root and return typed findings.
    pub fn validate(&self, request: ValidateRequest) -> Result<ValidationOutcome, SdkError> {
        let summary =
            crate::validate_generated_root_with_resources(&request.output_root, &self.resources)
                .map_err(|error| SdkError::classify("validate", error))?;
        let manifest =
            SchemaBoundManifest::load(&request.output_root, &self.resources, "validate")?;
        Ok(ValidationOutcome {
            output_root: request.output_root,
            files_checked: summary.files_checked,
            valid: summary.failures.is_empty(),
            failures: summary.failures,
            manifest,
        })
    }

    /// Build a typed schema-versioned report wrapper for a published output root.
    pub fn report(&self, request: ReportRequest) -> Result<ReportOutcome, SdkError> {
        let report =
            crate::build_coverage_report_with_resources(&request.output_root, &self.resources)
                .map_err(|error| SdkError::classify("report", error))?;
        ReportOutcome::from_value(request.output_root, &report)
    }

    /// Execute a structural assembly request without claiming IOD conformance.
    pub fn assemble(&self, request: AssembleRequest) -> Result<AssembleOutcome, SdkError> {
        self.assemble_cancellable(request, &CancellationToken::new())
    }

    pub fn assemble_cancellable(
        &self,
        request: AssembleRequest,
        cancellation: &CancellationToken,
    ) -> Result<AssembleOutcome, SdkError> {
        let request_bytes = match request.source {
            AssembleSource::File(path) => std::fs::read(&path).map_err(|error| {
                SdkError::classify(
                    "assemble",
                    format!("assembly input read failed at {}: {error}", path.display()),
                )
            })?,
            AssembleSource::Bytes(bytes) => bytes,
        };
        let summary = crate::assembly::assemble(
            &crate::assembly::AssembleOptions {
                request_bytes,
                caller_asset_root: request.caller_asset_root,
                output_root: request.output_root,
                seed: request.seed,
                parallelism: request.parallelism,
                dry_run: request.dry_run,
            },
            &cancellation.assembly,
            &self.resources,
        )
        .map_err(|error| SdkError::classify("assemble", error))?;
        let manifest = if summary.published {
            Some(SchemaBoundManifest::load(
                &summary.output_root,
                &self.resources,
                "assemble",
            )?)
        } else {
            None
        };
        let plan_preview = (!summary.published).then(|| PlanPreview {
            artifact_ids: summary.artifact_ids.clone(),
        });
        Ok(AssembleOutcome {
            output_root: summary.output_root,
            seed: request.seed,
            published: summary.published,
            artifacts_written: summary.artifacts_written,
            output_bytes: summary.output_bytes,
            corpus_plan_sha256: summary.corpus_plan_sha256,
            manifest,
            plan_preview,
        })
    }
}

#[derive(Debug, Clone)]
enum AssembleSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// Structural assembly input with an explicit caller-asset root.
#[derive(Debug, Clone)]
pub struct AssembleRequest {
    source: AssembleSource,
    caller_asset_root: PathBuf,
    output_root: PathBuf,
    seed: u64,
    parallelism: u32,
    dry_run: bool,
}

impl AssembleRequest {
    pub fn from_file(request_path: impl AsRef<Path>, output_root: impl AsRef<Path>) -> Self {
        let request_path = request_path.as_ref().to_path_buf();
        let caller_asset_root = request_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self {
            source: AssembleSource::File(request_path),
            caller_asset_root,
            output_root: output_root.as_ref().to_path_buf(),
            seed: 1,
            parallelism: 1,
            dry_run: false,
        }
    }
    pub fn from_json_bytes(
        bytes: impl Into<Vec<u8>>,
        caller_asset_root: impl AsRef<Path>,
        output_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            source: AssembleSource::Bytes(bytes.into()),
            caller_asset_root: caller_asset_root.as_ref().to_path_buf(),
            output_root: output_root.as_ref().to_path_buf(),
            seed: 1,
            parallelism: 1,
            dry_run: false,
        }
    }
    pub fn with_caller_asset_root(mut self, root: impl AsRef<Path>) -> Self {
        self.caller_asset_root = root.as_ref().to_path_buf();
        self
    }
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
    pub fn with_parallelism(mut self, parallelism: u32) -> Self {
        self.parallelism = parallelism.max(1);
        self
    }
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

#[derive(Debug, Clone)]
pub struct AssembleOutcome {
    output_root: PathBuf,
    seed: u64,
    published: bool,
    artifacts_written: usize,
    output_bytes: u64,
    corpus_plan_sha256: String,
    manifest: Option<SchemaBoundManifest>,
    plan_preview: Option<PlanPreview>,
}
impl AssembleOutcome {
    pub fn output_root(&self) -> &Path {
        &self.output_root
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn published(&self) -> bool {
        self.published
    }
    pub fn artifacts_written(&self) -> usize {
        self.artifacts_written
    }
    pub fn output_bytes(&self) -> u64 {
        self.output_bytes
    }
    pub fn corpus_plan_sha256(&self) -> &str {
        &self.corpus_plan_sha256
    }
    pub fn manifest(&self) -> Option<&SchemaBoundManifest> {
        self.manifest.as_ref()
    }
    pub fn plan_preview(&self) -> Option<&PlanPreview> {
        self.plan_preview.as_ref()
    }
}

#[derive(Debug, Clone)]
enum ComposeSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// A qualified-composition request with an explicit output and caller-asset root.
#[derive(Debug, Clone)]
pub struct ComposeRequest {
    source: ComposeSource,
    caller_asset_root: PathBuf,
    output_root: PathBuf,
    seed: u64,
    dry_run: bool,
}

impl ComposeRequest {
    /// Read a JSON request file; relative caller assets resolve below its parent.
    pub fn from_file(spec_path: impl AsRef<Path>, output_root: impl AsRef<Path>) -> Self {
        let spec_path = spec_path.as_ref().to_path_buf();
        let caller_asset_root = spec_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self {
            source: ComposeSource::File(spec_path),
            caller_asset_root,
            output_root: output_root.as_ref().to_path_buf(),
            seed: 1,
            dry_run: false,
        }
    }

    /// Use JSON bytes and resolve relative caller assets below `caller_asset_root`.
    pub fn from_json_bytes(
        spec_bytes: impl Into<Vec<u8>>,
        caller_asset_root: impl AsRef<Path>,
        output_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            source: ComposeSource::Bytes(spec_bytes.into()),
            caller_asset_root: caller_asset_root.as_ref().to_path_buf(),
            output_root: output_root.as_ref().to_path_buf(),
            seed: 1,
            dry_run: false,
        }
    }

    /// Override the root beneath which relative caller asset paths are resolved.
    pub fn with_caller_asset_root(mut self, root: impl AsRef<Path>) -> Self {
        self.caller_asset_root = root.as_ref().to_path_buf();
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

/// Cooperative cancellation handle for long-running SDK operations.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    inner: crate::composition::ComposeCancellationToken,
    assembly: crate::executor::cancellation::CancellationToken,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.inner.cancel();
        self.assembly.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled() || self.assembly.is_cancelled()
    }
}

/// Typed dry-run plan summary. The full internal plan is intentionally private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPreview {
    artifact_ids: Vec<String>,
}

impl PlanPreview {
    pub fn artifact_count(&self) -> usize {
        self.artifact_ids.len()
    }

    pub fn artifact_ids(&self) -> &[String] {
        &self.artifact_ids
    }
}

/// Typed qualified-composition outcome.
#[derive(Debug, Clone)]
pub struct ComposeOutcome {
    output_root: PathBuf,
    seed: u64,
    published: bool,
    instances_written: usize,
    output_bytes: u64,
    corpus_plan_sha256: String,
    manifest: Option<SchemaBoundManifest>,
    plan_preview: Option<PlanPreview>,
}

impl ComposeOutcome {
    pub fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn published(&self) -> bool {
        self.published
    }

    pub fn instances_written(&self) -> usize {
        self.instances_written
    }

    pub fn output_bytes(&self) -> u64 {
        self.output_bytes
    }

    pub fn corpus_plan_sha256(&self) -> &str {
        &self.corpus_plan_sha256
    }

    pub fn manifest(&self) -> Option<&SchemaBoundManifest> {
        self.manifest.as_ref()
    }

    pub fn plan_preview(&self) -> Option<&PlanPreview> {
        self.plan_preview.as_ref()
    }
}

/// Evidence class declared by a schema-validated manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestKind {
    CuratedGeneration,
    QualifiedComposition,
    StructuralAssembly,
}

/// A schema-bound manifest wrapper that does not expose untyped JSON as the outcome.
#[derive(Debug, Clone)]
pub struct SchemaBoundManifest {
    path: PathBuf,
    schema_version: String,
    kind: ManifestKind,
    seed: u64,
    bytes: Vec<u8>,
}

impl SchemaBoundManifest {
    fn load(
        output_root: &Path,
        resources: &EngineResources,
        command: &str,
    ) -> Result<Self, SdkError> {
        let validated = crate::manifest_contract::load_manifest_contract(output_root, resources)
            .map_err(|error| SdkError::classify(command, error))?;
        let kind = match validated.kind() {
            crate::manifest_contract::ManifestContractKind::CuratedGeneration => {
                ManifestKind::CuratedGeneration
            }
            crate::manifest_contract::ManifestContractKind::QualifiedComposition => {
                ManifestKind::QualifiedComposition
            }
            crate::manifest_contract::ManifestContractKind::StructuralAssembly => {
                ManifestKind::StructuralAssembly
            }
        };
        Ok(Self {
            path: validated.path().to_path_buf(),
            schema_version: validated.schema_version().to_owned(),
            kind,
            seed: validated.seed(),
            bytes: validated.bytes().to_vec(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn kind(&self) -> ManifestKind {
        self.kind
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn json_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Deserialize the already schema-validated document into a consumer model.
    pub fn deserialize<T: DeserializeOwned>(&self) -> Result<T, SdkError> {
        serde_json::from_slice(&self.bytes).map_err(|error| SdkError::classify("compose", error))
    }
}

/// Request validation of a published product output root.
#[derive(Debug, Clone)]
pub struct ValidateRequest {
    output_root: PathBuf,
}

impl ValidateRequest {
    pub fn new(output_root: impl AsRef<Path>) -> Self {
        Self {
            output_root: output_root.as_ref().to_path_buf(),
        }
    }
}

/// Typed validation result with the schema-bound manifest used as authority.
#[derive(Debug, Clone)]
pub struct ValidationOutcome {
    output_root: PathBuf,
    files_checked: usize,
    valid: bool,
    failures: Vec<String>,
    manifest: SchemaBoundManifest,
}

impl ValidationOutcome {
    pub fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub fn files_checked(&self) -> usize {
        self.files_checked
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    pub fn manifest(&self) -> &SchemaBoundManifest {
        &self.manifest
    }
}

/// Request a report for a published product output root.
#[derive(Debug, Clone)]
pub struct ReportRequest {
    output_root: PathBuf,
}

impl ReportRequest {
    pub fn new(output_root: impl AsRef<Path>) -> Self {
        Self {
            output_root: output_root.as_ref().to_path_buf(),
        }
    }
}

/// Evidence class represented by a report document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportKind {
    CuratedCoverage,
    QualifiedComposition,
    StructuralAssembly,
}

/// Typed, schema-versioned report wrapper.
#[derive(Debug, Clone)]
pub struct ReportOutcome {
    output_root: PathBuf,
    kind: ReportKind,
    schema_version: String,
    bytes: Vec<u8>,
}

impl ReportOutcome {
    fn from_value(output_root: PathBuf, value: &serde_json::Value) -> Result<Self, SdkError> {
        crate::report_contract::validate_report_contract(value)
            .map_err(|error| SdkError::classify("report", error))?;
        let (kind, version_field) = match value.get("report_kind").and_then(|kind| kind.as_str()) {
            Some("composition") => (
                ReportKind::QualifiedComposition,
                "composition_report_schema_version",
            ),
            Some("structural_assembly") => (
                ReportKind::StructuralAssembly,
                "structural_assembly_report_schema_version",
            ),
            None => (
                ReportKind::CuratedCoverage,
                "coverage_report_schema_version",
            ),
            Some(_) => return Err(SdkError::classify("report", "report kind invalid")),
        };
        let schema_version = value
            .get(version_field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SdkError::classify("report", "report schema version missing"))?
            .to_owned();
        let bytes =
            serde_json::to_vec(value).map_err(|error| SdkError::classify("report", error))?;
        Ok(Self {
            output_root,
            kind,
            schema_version,
            bytes,
        })
    }

    pub fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub fn kind(&self) -> ReportKind {
        self.kind
    }

    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn json_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn deserialize<T: DeserializeOwned>(&self) -> Result<T, SdkError> {
        serde_json::from_slice(&self.bytes).map_err(|error| SdkError::classify("report", error))
    }
}

/// Stable broad classification for SDK failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SdkErrorKind {
    Request,
    Unavailable,
    Output,
    Execution,
    Internal,
}

/// A typed SDK failure carrying the same stable code taxonomy as CLI API 1.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SdkError {
    kind: SdkErrorKind,
    code: &'static str,
    message: String,
    retryable: bool,
    diagnostic: String,
}

impl SdkError {
    pub(crate) fn classify(command: &str, error: impl fmt::Display) -> Self {
        let diagnostic = error.to_string();
        let failure = crate::cli_protocol::CliFailure::classify(command, &diagnostic);
        let kind = match failure.exit {
            2 => SdkErrorKind::Request,
            3 => SdkErrorKind::Unavailable,
            4 => SdkErrorKind::Output,
            5 => SdkErrorKind::Execution,
            _ => SdkErrorKind::Internal,
        };
        Self {
            kind,
            code: failure.error.code,
            message: failure.error.message,
            retryable: failure.error.retryable,
            diagnostic,
        }
    }

    /// Stable namespaced code shared with the CLI error registry.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Stable broad failure category.
    pub fn kind(&self) -> SdkErrorKind {
        self.kind
    }

    /// Stable public error description associated with [`Self::code`].
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether retrying after an external state change can be meaningful.
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    /// Human diagnostic detail; callers must branch on [`Self::code`] instead.
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.diagnostic)
    }
}

impl std::error::Error for SdkError {}
