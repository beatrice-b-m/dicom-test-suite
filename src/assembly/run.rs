use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use super::planning::plan_assembly;
use super::{ASSEMBLY_REQUEST_SCHEMA_VERSION, AssemblyError};
use crate::composition::CompositionUidRole;
use crate::composition::executor_adapter::CompositionExecutionServiceFactory;
use crate::corpus_plan::{PlannedArtifact, PlannedAuxiliaryArtifact};
use crate::engine_resources::{EngineResourceIdentity, EngineResources};
use crate::executor::adapters::ManifestProjectionInput;
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::{CorpusExecutor, ManifestProjectionError, ManifestProjector};
use crate::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationError,
};
use crate::executor::services::{ArtifactExecutionBindings, StagedAssetRegistry};

pub const ASSEMBLY_MANIFEST_SCHEMA_VERSION: &str = "2.0.0";

#[derive(Debug, Clone)]
pub struct AssembleOptions {
    pub request_bytes: Vec<u8>,
    pub caller_asset_root: PathBuf,
    pub output_root: PathBuf,
    pub seed: u64,
    pub parallelism: u32,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembleSummary {
    pub output_root: PathBuf,
    pub manifest_path: Option<PathBuf>,
    pub artifacts_written: usize,
    pub output_bytes: u64,
    pub corpus_plan_sha256: String,
    pub published: bool,
    pub artifact_ids: Vec<String>,
}

pub fn assemble(
    options: &AssembleOptions,
    cancellation: &CancellationToken,
    resources: &EngineResources,
) -> Result<AssembleSummary, AssemblyRunError> {
    if options.output_root.exists() {
        return Err(AssemblyRunError::OutputExists(options.output_root.clone()));
    }
    resources.verify_integrity()?;
    let resource_identity = resources.legacy_identity_v1()?;
    let identity_projection =
        crate::identity::project_manifest_identities(resources, None, Vec::new())?;
    let plan = plan_assembly(
        &options.request_bytes,
        &options.caller_asset_root,
        options.seed,
        options.parallelism,
        &resource_identity.resource_set_sha256,
    )?;
    let corpus_plan_sha256 = plan.corpus.canonical_sha256().map_err(|error| {
        AssemblyRunError::Execution(format!("assembly planning failed: {error}"))
    })?;
    let artifact_ids = plan
        .corpus
        .artifacts
        .iter()
        .map(|artifact| artifact.logical_id().to_owned())
        .collect::<Vec<_>>();
    if options.dry_run {
        return Ok(AssembleSummary {
            output_root: options.output_root.clone(),
            manifest_path: None,
            artifacts_written: 0,
            output_bytes: 0,
            corpus_plan_sha256,
            published: false,
            artifact_ids,
        });
    }
    let parent = options
        .output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| AssemblyRunError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|source| AssemblyRunError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let destination = canonical_parent.join(
        options
            .output_root
            .file_name()
            .ok_or_else(|| AssemblyRunError::Execution("output has no final component".into()))?,
    );
    let bindings = plan
        .corpus
        .artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.logical_id().to_owned(),
                ArtifactExecutionBindings {
                    artifact_id: artifact.logical_id().to_owned(),
                    slots: BTreeMap::new(),
                },
            )
        })
        .collect();
    let services = CompositionExecutionServiceFactory::native_only(bindings, Arc::new(RejectAux));
    let projector = AssemblyManifestProjector {
        request_sha256: plan.request_sha256,
        seed: options.seed,
        product_resources: resource_identity,
        identity_projection,
        identity_evidence: plan.identity_evidence.clone(),
    };
    let execution = CorpusExecutor::new(services, projector)
        .execute(&plan.corpus, destination, options.parallelism, cancellation)
        .map_err(|error| {
            AssemblyRunError::Execution(format!("assembly execution failed: {error}"))
        })?;
    Ok(AssembleSummary {
        output_root: options.output_root.clone(),
        manifest_path: Some(options.output_root.join("manifest.json")),
        artifacts_written: execution.evidence.artifacts.len(),
        output_bytes: execution.evidence.resources.actual_artifact_output_bytes,
        corpus_plan_sha256,
        published: true,
        artifact_ids,
    })
}

struct RejectAux;

impl AuxiliaryMaterializationHandler for RejectAux {
    fn render(
        &self,
        artifact: &PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        Err(MaterializationError::Auxiliary(format!(
            "structural assembly cannot render auxiliary artifact {}",
            artifact.logical_id
        )))
    }
}

struct AssemblyManifestProjector {
    request_sha256: String,
    seed: u64,
    product_resources: EngineResourceIdentity,
    identity_projection: crate::identity::ManifestIdentityProjection,
    identity_evidence: BTreeMap<String, serde_json::Value>,
}

impl ManifestProjector for AssemblyManifestProjector {
    fn project(&self, input: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        let external_runtime = crate::terminal_external_runtime_identities(input)?;
        let identity_projection = crate::identity::finalize_manifest_runtime_identities(
            self.identity_projection.clone(),
            external_runtime,
        )
        .map_err(|error| ManifestProjectionError(error.to_string()))?;
        let instances = input
            .artifacts
            .iter()
            .map(|artifact| {
                let PlannedArtifact::Dicom(planned) = &artifact.planned else {
                    return Err(ManifestProjectionError(
                        "structural manifest received a non-DICOM artifact".into(),
                    ));
                };
                let output = artifact.execution.output.as_ref().ok_or_else(|| {
                    ManifestProjectionError("structural output evidence missing".into())
                })?;
                let sop_instance_uid = planned
                    .instance
                    .identities
                    .get(&CompositionUidRole::SopInstance, 0)
                    .ok_or_else(|| {
                        ManifestProjectionError("structural SOP identity missing".into())
                    })?;
                let identity =
                    self.identity_evidence
                        .get(&planned.logical_id)
                        .ok_or_else(|| {
                            ManifestProjectionError("structural identity evidence missing".into())
                        })?;
                Ok(json!({
                    "instance_id": planned.logical_id,
                    "output_path": output.relative_path,
                    "size_bytes": output.size_bytes,
                    "sha256": output.sha256,
                    "resolved_plan_sha256": planned.instance.canonical_sha256(),
                    "sop_class_uid": planned.instance.sop_class_uid,
                    "sop_instance_uid": sop_instance_uid,
                    "identity": identity,
                    "transfer_syntax_uid": planned.instance.transfer_syntax_uid,
                    "iod_conformance": "not_assessed",
                    "elements": planned.instance.attributes,
                    "bulk": planned.instance.content,
                    "references": planned.instance.references,
                    "validation": artifact.execution.validation
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let manifest = json!({
            "manifest_schema_version": ASSEMBLY_MANIFEST_SCHEMA_VERSION,
            "generated_at": "2000-01-01T00:00:00Z",
            "generator": { "name": crate::PACKAGE_NAME, "version": crate::PACKAGE_VERSION, "target": crate::TARGET_TRIPLE, "rustc": crate::RUSTC_VERSION },
            "product_resources": self.product_resources,
            "identity_projection": identity_projection,
            "run": { "kind": "structural_assembly", "assembly_request_schema_version": ASSEMBLY_REQUEST_SCHEMA_VERSION, "request_sha256": self.request_sha256, "seed": self.seed, "corpus_plan_sha256": input.corpus_plan_sha256, "caller_asset_root_policy": "explicit_bounded_relative_paths", "iod_conformance": "not_assessed" },
            "instances": instances,
            "unavailable_capabilities": input.unavailable,
            "resources": input.resources,
            "publication": input.publication,
            "warnings": ["IOD conformance was not assessed"]
        });
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| ManifestProjectionError(error.to_string()))
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum AssemblyRunError {
    Request(AssemblyError),
    Resources(crate::engine_resources::EngineResourceError),
    Identity(crate::identity::IdentityProjectionError),
    OutputExists(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Execution(String),
}

impl From<AssemblyError> for AssemblyRunError {
    fn from(value: AssemblyError) -> Self {
        Self::Request(value)
    }
}
impl From<crate::engine_resources::EngineResourceError> for AssemblyRunError {
    fn from(value: crate::engine_resources::EngineResourceError) -> Self {
        Self::Resources(value)
    }
}
impl From<crate::identity::IdentityProjectionError> for AssemblyRunError {
    fn from(value: crate::identity::IdentityProjectionError) -> Self {
        Self::Identity(value)
    }
}
impl fmt::Display for AssemblyRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(error) => write!(f, "{error}"),
            Self::Resources(error) => write!(f, "product resources: {error}"),
            Self::Identity(error) => write!(f, "identity projection: {error}"),
            Self::OutputExists(path) => {
                write!(f, "assembly output path already exists: {}", path.display())
            }
            Self::Io { path, source } => {
                write!(f, "assembly I/O failed at {}: {source}", path.display())
            }
            Self::Execution(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for AssemblyRunError {}
