//! Plan-first corpus orchestration and atomic publication.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::corpus_plan::{
    CorpusPlan, EvidenceIndependence, EvidenceObligation, PlannedArtifact, ValidationPlan,
    ValidationRequirement,
};
use crate::executor::adapters::{
    AdapterError, ArtifactServiceOutputs, CodecExecutionRecord,
    ManifestProjectionCompatibilityInput, ProviderExecutionRecord, PublicationTransition,
    RunEvidenceAdapterInput, assemble_run_evidence, compatibility_projection_input,
};
use crate::executor::cancellation::{
    CancellationPoint, CancellationStage, CancellationToken, Cancelled,
};
use crate::executor::evidence::{ObligationResult, ResultStatus, RunEvidence};
use crate::executor::scheduler::{
    ActualResourceUsage, ArtifactWorker, ScheduleOutcome, SchedulerError, WorkerOutput, schedule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, AssetVisibility, CodecRequest, CodecResult, MaterializationRequest,
    MaterializationResult, ProviderRequest, ProviderResult, SlotExecutionBinding,
    StagedAssetRegistry, ValidationRequest, ValidationResult, ValidationStatus,
};
use crate::executor::transaction::{OutputTransaction, TransactionError};
use crate::sha256_hex;

pub trait BoundExecutionServices: Send + Sync {
    /// Verified caller inputs copied beneath private transaction staging during
    /// service binding. These seed the registry before any DAG node runs.
    fn initial_assets(
        &self,
    ) -> Result<Vec<crate::executor::services::ProducedAsset>, ServiceInvocationError> {
        Ok(Vec::new())
    }

    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError>;

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError>;

    fn invoke_codec(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError>;

    fn invoke_codec_cancellable(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        if cancellation.is_cancelled() {
            return Err(ServiceInvocationError::new("codec", "execution cancelled"));
        }
        self.invoke_codec(request, assets)
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError>;

    fn materialize_cancellable(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        if cancellation.is_cancelled() {
            return Err(ServiceInvocationError::new(
                "materializer",
                "execution cancelled",
            ));
        }
        self.materialize(request, assets)
    }

    fn validate(
        &self,
        request: &ValidationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceInvocationError>;

    fn evaluate_obligation(
        &self,
        artifact: &PlannedArtifact,
        obligation: &EvidenceObligation,
        materialization: &MaterializationResult,
        validation: &ValidationResult,
        assets: &StagedAssetRegistry,
    ) -> Result<ObligationResult, ServiceInvocationError>;

    /// Observed peak working bytes for this artifact. Implementations must use
    /// measured service data rather than the planned ceiling.
    fn actual_peak_working_bytes(
        &self,
        artifact: &PlannedArtifact,
        materialization: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError>;

    /// Remove execution-only assets before the transaction is made public.
    fn finalize_private_assets(
        &self,
        _assets: &StagedAssetRegistry,
    ) -> Result<(), ServiceInvocationError> {
        Ok(())
    }
}

pub trait ExecutionServiceFactory: Send + Sync {
    fn bind(
        &self,
        private_staging_root: &Path,
    ) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodecServiceOutcome {
    pub result: CodecResult,
    pub determinism: String,
    pub decoded_frame_sha256: BTreeMap<u32, String>,
    pub metrics: BTreeMap<String, f64>,
    pub claims: BTreeMap<String, serde_json::Value>,
}

pub trait ManifestProjector: Send + Sync {
    fn project(
        &self,
        input: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError>;
}

pub trait ExecutorTransaction: Send {
    fn staging_root(&self) -> &Path;
    fn write_manifest(&mut self, bytes: &[u8]) -> Result<(), TransactionError>;
    fn cleanup(self: Box<Self>) -> Result<(), TransactionError>;
    fn promote(self: Box<Self>) -> Result<PathBuf, TransactionError>;
}

pub trait ExecutorTransactionFactory: Send + Sync {
    fn begin(&self, destination: &Path) -> Result<Box<dyn ExecutorTransaction>, TransactionError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RealExecutorTransactionFactory;

impl ExecutorTransactionFactory for RealExecutorTransactionFactory {
    fn begin(&self, destination: &Path) -> Result<Box<dyn ExecutorTransaction>, TransactionError> {
        Ok(Box::new(RealExecutorTransaction(OutputTransaction::begin(
            destination,
        )?)))
    }
}

struct RealExecutorTransaction(OutputTransaction);

impl ExecutorTransaction for RealExecutorTransaction {
    fn staging_root(&self) -> &Path {
        self.0.staging_root()
    }

    fn write_manifest(&mut self, bytes: &[u8]) -> Result<(), TransactionError> {
        self.0.write_manifest(bytes)
    }

    fn cleanup(self: Box<Self>) -> Result<(), TransactionError> {
        let Self(transaction) = *self;
        transaction.cleanup()
    }

    fn promote(self: Box<Self>) -> Result<PathBuf, TransactionError> {
        let Self(transaction) = *self;
        transaction.promote()
    }
}

pub struct CorpusExecutor<S, P, T = RealExecutorTransactionFactory> {
    services: S,
    projector: P,
    transactions: T,
}

impl<S, P> CorpusExecutor<S, P, RealExecutorTransactionFactory> {
    pub fn new(services: S, projector: P) -> Self {
        Self {
            services,
            projector,
            transactions: RealExecutorTransactionFactory,
        }
    }
}

impl<S, P, T> CorpusExecutor<S, P, T> {
    pub fn with_transaction_factory(services: S, projector: P, transactions: T) -> Self {
        Self {
            services,
            projector,
            transactions,
        }
    }
}

impl<S, P, T> CorpusExecutor<S, P, T>
where
    S: ExecutionServiceFactory,
    P: ManifestProjector,
    T: ExecutorTransactionFactory,
{
    pub fn execute(
        &self,
        plan: &CorpusPlan,
        destination: impl AsRef<Path>,
        requested_parallelism: u32,
        cancellation: &CancellationToken,
    ) -> Result<CorpusExecutionResult, CorpusExecutorError> {
        validate_execution_request(plan, cancellation)?;
        let transaction = self
            .transactions
            .begin(destination.as_ref())
            .map_err(CorpusExecutorError::Transaction)?;
        let staged = match self.execute_staging(
            plan,
            transaction.staging_root(),
            requested_parallelism,
            cancellation,
        ) {
            Ok(staged) => staged,
            Err(error) => return Err(cleanup_failure(transaction, error)),
        };
        let manifest = match self.projector.project(&staged.projection) {
            Ok(manifest) => manifest,
            Err(error) => return Err(cleanup_failure(transaction, error.into())),
        };
        let manifest_sha256 = sha256_hex(&manifest);
        let evidence = match evidence_for(
            plan,
            staged.outcome,
            requested_parallelism,
            manifest.len() as u64,
            PublicationTransition::promoted(manifest_sha256.clone()),
        ) {
            Ok(evidence) => evidence,
            Err(error) => return Err(cleanup_failure(transaction, error)),
        };
        let mut transaction = transaction;
        if let Err(error) = transaction.write_manifest(&manifest) {
            return Err(cleanup_failure(transaction, error.into()));
        }
        if let Err(error) =
            cancellation.checkpoint(CancellationPoint::run(CancellationStage::BeforePromotion))
        {
            return Err(cleanup_failure(transaction, error.into()));
        }
        let destination = transaction
            .promote()
            .map_err(CorpusExecutorError::Transaction)?;
        Ok(CorpusExecutionResult {
            destination,
            manifest_sha256,
            manifest_size_bytes: manifest.len() as u64,
            manifest_bytes: manifest,
            evidence,
        })
    }

    pub fn execute_into_staging(
        &self,
        plan: &CorpusPlan,
        private_staging_root: impl AsRef<Path>,
        requested_parallelism: u32,
        cancellation: &CancellationToken,
    ) -> Result<StagedCorpusExecution, CorpusExecutorError> {
        validate_execution_request(plan, cancellation)?;
        let staged = self.execute_staging(
            plan,
            private_staging_root.as_ref(),
            requested_parallelism,
            cancellation,
        )?;
        Ok(StagedCorpusExecution {
            projection: staged.projection,
            evidence: staged.preliminary,
        })
    }

    fn execute_staging(
        &self,
        plan: &CorpusPlan,
        private_staging_root: &Path,
        requested_parallelism: u32,
        cancellation: &CancellationToken,
    ) -> Result<StagingExecutionCore, CorpusExecutorError> {
        let services = self.services.bind(private_staging_root)?;
        let mut initial_registry = StagedAssetRegistry::default();
        for asset in services.initial_assets()? {
            initial_registry
                .register(asset)
                .map_err(CorpusExecutorError::ServiceContract)?;
        }
        let registry = Arc::new(Mutex::new(initial_registry));
        let worker = ExecutionWorker {
            services,
            registry,
            cancellation,
        };
        let outcome = schedule(plan, requested_parallelism, cancellation, &worker)
            .map_err(CorpusExecutorError::Scheduler)?;
        cancellation.checkpoint(CancellationPoint::run(CancellationStage::BeforeManifest))?;
        let preliminary = evidence_for(
            plan,
            outcome.clone(),
            requested_parallelism,
            0,
            PublicationTransition::staging(),
        )?;
        let projection = compatibility_projection_input(plan, &preliminary)?;
        let asset_snapshot = worker
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        worker.services.finalize_private_assets(&asset_snapshot)?;
        sanitize_publication_staging(private_staging_root, plan, &asset_snapshot)?;
        Ok(StagingExecutionCore {
            outcome,
            preliminary,
            projection,
        })
    }
}

struct StagingExecutionCore {
    outcome: ScheduleOutcome<ArtifactServiceOutputs>,
    preliminary: RunEvidence,
    projection: ManifestProjectionCompatibilityInput,
}

fn validate_execution_request(
    plan: &CorpusPlan,
    cancellation: &CancellationToken,
) -> Result<(), CorpusExecutorError> {
    cancellation.checkpoint(CancellationPoint::run(CancellationStage::BeforeExecution))?;
    plan.validate().map_err(CorpusExecutorError::InvalidPlan)?;
    if plan.artifacts.is_empty() {
        return Err(CorpusExecutorError::EmptyPlan);
    }
    if plan.publication.manifest_path.as_str() != "manifest.json" {
        return Err(CorpusExecutorError::UnsupportedManifestPath(
            plan.publication.manifest_path.as_str().to_owned(),
        ));
    }
    Ok(())
}

fn sanitize_publication_staging(
    root: &Path,
    plan: &CorpusPlan,
    assets: &StagedAssetRegistry,
) -> Result<(), ServiceInvocationError> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(ServiceInvocationError::new(
                "publication sanitization",
                "transaction staging root is not a safe directory",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ServiceInvocationError::new(
                "publication sanitization",
                error.to_string(),
            ));
        }
    }
    let allowed = plan
        .artifacts
        .iter()
        .filter_map(|artifact| artifact.output())
        .filter(|output| output.publish)
        .map(|output| output.relative_path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let registered_public = assets
        .iter()
        .filter(|asset| asset.visibility == AssetVisibility::PublicationCandidate)
        .map(|asset| asset.relative_path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    if registered_public != allowed {
        return Err(ServiceInvocationError::new(
            "publication sanitization",
            format!(
                "registered publication paths do not match the plan: registered={registered_public:?}, planned={allowed:?}"
            ),
        ));
    }
    for asset in assets
        .iter()
        .filter(|asset| asset.visibility == AssetVisibility::PublicationCandidate)
    {
        let path = root.join(asset.relative_path.as_str());
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ServiceInvocationError::new("publication sanitization", error.to_string())
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ServiceInvocationError::new(
                "publication sanitization",
                format!("planned output is not a regular file: {}", path.display()),
            ));
        }
        let (size_bytes, sha256) = hash_regular_file(&path)?;
        if size_bytes != asset.size_bytes || sha256 != asset.sha256 {
            return Err(ServiceInvocationError::new(
                "publication sanitization",
                format!("planned output identity changed: {}", path.display()),
            ));
        }
    }
    sanitize_directory(root, root, &allowed)
}

fn hash_regular_file(path: &Path) -> Result<(u64, String), ServiceInvocationError> {
    let mut file = fs::File::open(path).map_err(|error| {
        ServiceInvocationError::new("publication sanitization", error.to_string())
    })?;
    let mut hasher = crate::hashing::StreamingSha256::new();
    let mut buffer = [0_u8; 8192];
    let mut size = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            ServiceInvocationError::new("publication sanitization", error.to_string())
        })?;
        if read == 0 {
            break;
        }
        size = size.checked_add(read as u64).ok_or_else(|| {
            ServiceInvocationError::new("publication sanitization", "output size overflow")
        })?;
        hasher.update(&buffer[..read]);
    }
    Ok((size, hasher.finish_hex()))
}

fn sanitize_directory(
    root: &Path,
    directory: &Path,
    allowed: &BTreeSet<String>,
) -> Result<(), ServiceInvocationError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        ServiceInvocationError::new("publication sanitization", error.to_string())
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ServiceInvocationError::new("publication sanitization", error.to_string())
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ServiceInvocationError::new("publication sanitization", error.to_string())
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ServiceInvocationError::new(
                "publication sanitization",
                format!("staging contains a symlink: {}", path.display()),
            ));
        }
        if metadata.is_dir() {
            sanitize_directory(root, &path, allowed)?;
            if fs::read_dir(&path)
                .map_err(|error| {
                    ServiceInvocationError::new("publication sanitization", error.to_string())
                })?
                .next()
                .is_none()
            {
                fs::remove_dir(&path).map_err(|error| {
                    ServiceInvocationError::new("publication sanitization", error.to_string())
                })?;
            }
        } else if metadata.is_file() {
            let relative = path.strip_prefix(root).map_err(|_| {
                ServiceInvocationError::new(
                    "publication sanitization",
                    "staging entry escaped its root",
                )
            })?;
            let relative = relative.to_string_lossy().replace('\\', "/");
            if !allowed.contains(&relative) {
                fs::remove_file(&path).map_err(|error| {
                    ServiceInvocationError::new("publication sanitization", error.to_string())
                })?;
            }
        } else {
            return Err(ServiceInvocationError::new(
                "publication sanitization",
                format!("unsupported staging entry: {}", path.display()),
            ));
        }
    }
    Ok(())
}

fn evidence_for(
    plan: &CorpusPlan,
    outcome: ScheduleOutcome<ArtifactServiceOutputs>,
    requested_parallelism: u32,
    manifest_size_bytes: u64,
    publication: PublicationTransition,
) -> Result<RunEvidence, CorpusExecutorError> {
    assemble_run_evidence(
        plan,
        outcome,
        RunEvidenceAdapterInput {
            requested_parallelism,
            used_parallelism: requested_parallelism
                .min(plan.resources.max_parallelism)
                .min(plan.artifacts.len() as u32),
            manifest_size_bytes,
            publication,
        },
    )
    .map_err(CorpusExecutorError::Adapter)
}

struct ExecutionWorker<'a> {
    services: Arc<dyn BoundExecutionServices>,
    registry: Arc<Mutex<StagedAssetRegistry>>,
    cancellation: &'a CancellationToken,
}

impl ArtifactWorker<ArtifactServiceOutputs, ArtifactExecutionError> for ExecutionWorker<'_> {
    fn execute(
        &self,
        artifact: &PlannedArtifact,
        _: &dyn crate::executor::scheduler::Cancellation,
    ) -> Result<WorkerOutput<ArtifactServiceOutputs>, ArtifactExecutionError> {
        let artifact_id = artifact.logical_id();
        let mut bindings = self.services.bindings_for(artifact)?;
        if bindings.artifact_id != artifact_id {
            return Err(ArtifactExecutionError::BindingIdentityMismatch(
                artifact_id.to_owned(),
            ));
        }
        let mut providers = Vec::new();
        let mut codecs = Vec::new();
        let mut transient_peak_working_bytes = 0_u64;
        for (slot, binding) in bindings.slots.clone() {
            match binding {
                SlotExecutionBinding::ProviderRequest { request } => {
                    self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                    let assets = self.asset_snapshot();
                    request.validate(&assets)?;
                    let result =
                        match self
                            .services
                            .invoke_provider(&request, &assets, self.cancellation)
                        {
                            Ok(result) => result,
                            Err(error) => {
                                self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                                return Err(error.into());
                            }
                        };
                    self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                    result.validate(&request)?;
                    let selected_handle = select_provider_output(&slot, &result)?
                        .declaration
                        .handle
                        .clone();
                    let mut registry = self.registry.lock().unwrap_or_else(|e| e.into_inner());
                    for output in result.outputs.values() {
                        registry.register(output.clone())?;
                    }
                    drop(registry);
                    bindings.slots.insert(
                        slot,
                        SlotExecutionBinding::StagedAsset {
                            asset: selected_handle,
                        },
                    );
                    providers.push(ProviderExecutionRecord { request, result });
                }
                SlotExecutionBinding::CodecRequest { request } => {
                    self.checkpoint(CancellationStage::BeforeCodec, artifact_id)?;
                    let assets = self.asset_snapshot();
                    request.validate(&assets)?;
                    let outcome = match self.services.invoke_codec_cancellable(
                        &request,
                        &assets,
                        self.cancellation,
                    ) {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            self.checkpoint(CancellationStage::BeforeCodec, artifact_id)?;
                            return Err(error.into());
                        }
                    };
                    self.checkpoint(CancellationStage::BeforeCodec, artifact_id)?;
                    outcome.result.validate(&request, &assets)?;
                    transient_peak_working_bytes = transient_peak_working_bytes
                        .max(codec_transient_bytes(&request, &outcome.result)?);
                    bindings.slots.insert(
                        slot,
                        SlotExecutionBinding::EncodedFrames {
                            frames: outcome.result.frames.clone(),
                        },
                    );
                    codecs.push(CodecExecutionRecord {
                        request,
                        result: outcome.result,
                        determinism: outcome.determinism,
                        decoded_frame_sha256: outcome.decoded_frame_sha256,
                        metrics: outcome.metrics,
                        claims: outcome.claims,
                    });
                }
                SlotExecutionBinding::ProviderCodecPipeline { provider, codec } => {
                    self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                    let assets = self.asset_snapshot();
                    provider.validate(&assets)?;
                    let provider_result =
                        match self
                            .services
                            .invoke_provider(&provider, &assets, self.cancellation)
                        {
                            Ok(result) => result,
                            Err(error) => {
                                self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                                return Err(error.into());
                            }
                        };
                    self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                    provider_result.validate(&provider)?;
                    select_provider_output(&slot, &provider_result)?;
                    let produced_handles = provider_result
                        .outputs
                        .values()
                        .map(|output| output.declaration.handle.clone())
                        .collect::<std::collections::BTreeSet<_>>();
                    if codec.frames.iter().any(|frame| match &frame.bytes {
                        crate::executor::services::ByteBinding::VerifiedAssetRange {
                            asset,
                            ..
                        } => !produced_handles.contains(asset),
                        _ => true,
                    }) {
                        return Err(ArtifactExecutionError::PipelineAssetMismatch {
                            request_id: provider.request_id.clone(),
                            codec_request_id: codec.request_id.clone(),
                        });
                    }
                    let mut registry = self.registry.lock().unwrap_or_else(|e| e.into_inner());
                    for output in provider_result.outputs.values() {
                        registry.register(output.clone())?;
                    }
                    drop(registry);
                    providers.push(ProviderExecutionRecord {
                        request: provider,
                        result: provider_result,
                    });

                    self.checkpoint(CancellationStage::BeforeCodec, artifact_id)?;
                    let assets = self.asset_snapshot();
                    codec.validate(&assets)?;
                    let outcome = match self.services.invoke_codec_cancellable(
                        &codec,
                        &assets,
                        self.cancellation,
                    ) {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            self.checkpoint(CancellationStage::BeforeCodec, artifact_id)?;
                            return Err(error.into());
                        }
                    };
                    self.checkpoint(CancellationStage::BeforeCodec, artifact_id)?;
                    outcome.result.validate(&codec, &assets)?;
                    transient_peak_working_bytes = transient_peak_working_bytes
                        .max(codec_transient_bytes(&codec, &outcome.result)?);
                    bindings.slots.insert(
                        slot,
                        SlotExecutionBinding::EncodedFrames {
                            frames: outcome.result.frames.clone(),
                        },
                    );
                    codecs.push(CodecExecutionRecord {
                        request: codec,
                        result: outcome.result,
                        determinism: outcome.determinism,
                        decoded_frame_sha256: outcome.decoded_frame_sha256,
                        metrics: outcome.metrics,
                        claims: outcome.claims,
                    });
                }
                _ => {}
            }
        }

        self.checkpoint(CancellationStage::BeforeMaterialization, artifact_id)?;
        let request = MaterializationRequest {
            artifact: artifact.clone(),
            bindings,
        };
        let assets = self.asset_snapshot();
        request.validate(&assets)?;
        let materialization =
            match self
                .services
                .materialize_cancellable(&request, &assets, self.cancellation)
            {
                Ok(materialization) => materialization,
                Err(error) => {
                    self.checkpoint(CancellationStage::BeforeMaterialization, artifact_id)?;
                    return Err(error.into());
                }
            };
        self.checkpoint(CancellationStage::BeforeMaterialization, artifact_id)?;
        materialization.validate(&request)?;
        if let Some(output) = &materialization.output {
            self.registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .register(output.clone())?;
        }

        self.checkpoint(CancellationStage::BeforeValidation, artifact_id)?;
        let validation_plan = artifact_validation(artifact).clone();
        let validation_request = ValidationRequest {
            artifact: artifact.clone(),
            materialized_asset: materialization
                .output
                .as_ref()
                .map(|output| output.declaration.handle.clone()),
            plan: validation_plan.clone(),
        };
        let assets = self.asset_snapshot();
        validation_request.validate(&assets)?;
        let validation = self.services.validate(&validation_request, &assets)?;
        validation.validate(&validation_request)?;
        require_successful_validation(&validation_plan, &validation)?;

        let mut obligations = Vec::new();
        for obligation in artifact_obligations(artifact) {
            let result = self.services.evaluate_obligation(
                artifact,
                obligation,
                &materialization,
                &validation,
                &assets,
            )?;
            require_valid_obligation(obligation, &result)?;
            obligations.push(result);
        }
        let reported_peak_working_bytes = self
            .services
            .actual_peak_working_bytes(artifact, &materialization)?;
        let materialization_transient =
            materialization_transient_bytes(&request, &materialization)?;
        let peak_working_bytes = reported_peak_working_bytes
            .max(transient_peak_working_bytes)
            .max(materialization_transient);
        let output_bytes = materialization
            .output
            .as_ref()
            .map_or(0, |output| output.observed_size_bytes);
        Ok(WorkerOutput {
            value: ArtifactServiceOutputs {
                status: crate::executor::evidence::ExecutionStatus::Succeeded,
                materialization: Some(materialization),
                validation: Some(validation),
                obligations,
                providers,
                codecs,
                elapsed_milliseconds: 0,
            },
            resources: ActualResourceUsage {
                output_bytes,
                peak_working_bytes,
            },
        })
    }
}

impl ExecutionWorker<'_> {
    fn asset_snapshot(&self) -> StagedAssetRegistry {
        self.registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn checkpoint(
        &self,
        stage: CancellationStage,
        artifact_id: &str,
    ) -> Result<(), ArtifactExecutionError> {
        self.cancellation
            .checkpoint(CancellationPoint::artifact(stage, artifact_id))
            .map_err(ArtifactExecutionError::Cancelled)
    }
}

fn select_provider_output<'a>(
    slot: &str,
    result: &'a ProviderResult,
) -> Result<&'a crate::executor::services::ProducedAsset, ArtifactExecutionError> {
    if let Some(output) = result.outputs.get(slot) {
        return Ok(output);
    }
    if result.outputs.len() == 1 {
        return Ok(result.outputs.values().next().expect("one provider output"));
    }
    Err(ArtifactExecutionError::AmbiguousProviderOutput {
        request_id: result.request_id.clone(),
        slot: slot.to_owned(),
    })
}

fn codec_transient_bytes(
    request: &CodecRequest,
    result: &CodecResult,
) -> Result<u64, ArtifactExecutionError> {
    let native = request.frames.iter().try_fold(0_u64, |total, frame| {
        let length = match &frame.bytes {
            crate::executor::services::ByteBinding::Inline { bytes, .. } => bytes.len() as u64,
            crate::executor::services::ByteBinding::StagedRange { length, .. }
            | crate::executor::services::ByteBinding::VerifiedAssetRange { length, .. } => *length,
        };
        total.checked_add(length)
    });
    let encoded = result.frames.iter().try_fold(0_u64, |total, frame| {
        total.checked_add(frame.encoded_size_bytes)
    });
    native
        .and_then(|native| encoded.and_then(|encoded| native.checked_add(encoded)))
        .ok_or(ArtifactExecutionError::ResourceAccountingOverflow(
            "codec transient bytes",
        ))
}

fn materialization_transient_bytes(
    request: &MaterializationRequest,
    result: &MaterializationResult,
) -> Result<u64, ArtifactExecutionError> {
    let encoded = request
        .bindings
        .slots
        .values()
        .try_fold(0_u64, |total, binding| {
            let SlotExecutionBinding::EncodedFrames { frames } = binding else {
                return Some(total);
            };
            frames.iter().try_fold(total, |subtotal, frame| {
                subtotal.checked_add(frame.encoded_size_bytes)
            })
        });
    let encoded = encoded.ok_or(ArtifactExecutionError::ResourceAccountingOverflow(
        "materialization encoded bytes",
    ))?;
    if encoded == 0 {
        return Ok(result
            .output
            .as_ref()
            .map_or(1, |output| output.observed_size_bytes.max(1)));
    }
    let frame_count = request
        .bindings
        .slots
        .values()
        .try_fold(0_u64, |total, binding| {
            let count = match binding {
                SlotExecutionBinding::EncodedFrames { frames } => frames.len() as u64,
                _ => 0,
            };
            total.checked_add(count)
        });
    let bot_bytes = frame_count.and_then(|count| count.checked_mul(4)).ok_or(
        ArtifactExecutionError::ResourceAccountingOverflow("basic offset table bytes"),
    )?;
    let output = result
        .output
        .as_ref()
        .map_or(0, |asset| asset.observed_size_bytes);
    encoded
        .checked_mul(3)
        .and_then(|value| value.checked_add(bot_bytes))
        .and_then(|value| value.checked_add(output))
        .ok_or(ArtifactExecutionError::ResourceAccountingOverflow(
            "materialization transient bytes",
        ))
}

fn artifact_validation(artifact: &PlannedArtifact) -> &ValidationPlan {
    match artifact {
        PlannedArtifact::Dicom(value) => &value.validation,
        PlannedArtifact::Mutation(value) => &value.validation,
        PlannedArtifact::Qualification(value) => &value.validation,
        PlannedArtifact::Auxiliary(value) => &value.validation,
    }
}

fn artifact_obligations(artifact: &PlannedArtifact) -> &[EvidenceObligation] {
    match artifact {
        PlannedArtifact::Dicom(value) => &value.evidence.obligations,
        PlannedArtifact::Mutation(value) => &value.evidence.obligations,
        PlannedArtifact::Qualification(value) => &value.evidence.obligations,
        PlannedArtifact::Auxiliary(value) => &value.evidence.obligations,
    }
}

fn require_successful_validation(
    plan: &ValidationPlan,
    result: &ValidationResult,
) -> Result<(), ArtifactExecutionError> {
    let requirements = plan
        .rules
        .iter()
        .map(|rule| (rule.rule_id.as_str(), rule.requirement))
        .collect::<BTreeMap<_, _>>();
    for rule in &result.rules {
        match (requirements[rule.rule_id.as_str()], rule.status) {
            (_, ValidationStatus::Failed)
            | (
                ValidationRequirement::Required | ValidationRequirement::IndependentRequired,
                ValidationStatus::Unavailable,
            ) => {
                return Err(ArtifactExecutionError::ValidationFailed {
                    rule_id: rule.rule_id.clone(),
                    status: rule.status,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn require_valid_obligation(
    planned: &EvidenceObligation,
    actual: &ObligationResult,
) -> Result<(), ArtifactExecutionError> {
    let independence = match planned.independence {
        EvidenceIndependence::SameProject => {
            crate::executor::evidence::EvidenceIndependence::SameProject
        }
        EvidenceIndependence::IndependentTool => {
            crate::executor::evidence::EvidenceIndependence::IndependentTool
        }
        EvidenceIndependence::ExternalProvider => {
            crate::executor::evidence::EvidenceIndependence::ExternalProvider
        }
    };
    if actual.obligation_id != planned.obligation_id
        || actual.route_id != planned.route_id
        || actual.independence != independence
        || actual.required != planned.required
        || actual.status == ResultStatus::Failed
        || (planned.required && actual.status != ResultStatus::Passed)
        || (planned.independence != EvidenceIndependence::SameProject
            && actual.status == ResultStatus::Passed
            && actual.tool.is_none())
    {
        return Err(ArtifactExecutionError::ObligationFailed(
            planned.obligation_id.clone(),
        ));
    }
    Ok(())
}

fn cleanup_failure(
    transaction: Box<dyn ExecutorTransaction>,
    primary: CorpusExecutorError,
) -> CorpusExecutorError {
    match transaction.cleanup() {
        Ok(()) => primary,
        Err(cleanup) => CorpusExecutorError::PrimaryAndCleanup {
            primary: Box::new(primary),
            cleanup,
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorpusExecutionResult {
    pub destination: PathBuf,
    pub manifest_sha256: String,
    pub manifest_size_bytes: u64,
    pub manifest_bytes: Vec<u8>,
    pub evidence: RunEvidence,
}

/// Completed shared execution inside a caller-owned private staging root.
///
/// This seam lets a frontend combine plan-first artifacts with temporarily
/// unmigrated artifacts while retaining one outer publication transaction.
/// It never writes a manifest or promotes the staging directory.
pub struct StagedCorpusExecution {
    pub projection: ManifestProjectionCompatibilityInput,
    pub evidence: RunEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInvocationError {
    pub stage: &'static str,
    pub message: String,
}

impl ServiceInvocationError {
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }
}

impl fmt::Display for ServiceInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} service failed: {}", self.stage, self.message)
    }
}

impl std::error::Error for ServiceInvocationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestProjectionError(pub String);

impl fmt::Display for ManifestProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ManifestProjectionError {}

#[derive(Debug)]
pub enum ArtifactExecutionError {
    Service(ServiceInvocationError),
    ServiceContract(crate::executor::services::ServiceError),
    Cancelled(Cancelled),
    BindingIdentityMismatch(String),
    AmbiguousProviderOutput {
        request_id: String,
        slot: String,
    },
    PipelineAssetMismatch {
        request_id: String,
        codec_request_id: String,
    },
    ResourceAccountingOverflow(&'static str),
    ValidationFailed {
        rule_id: String,
        status: ValidationStatus,
    },
    ObligationFailed(String),
}

impl From<ServiceInvocationError> for ArtifactExecutionError {
    fn from(value: ServiceInvocationError) -> Self {
        Self::Service(value)
    }
}

impl From<crate::executor::services::ServiceError> for ArtifactExecutionError {
    fn from(value: crate::executor::services::ServiceError) -> Self {
        Self::ServiceContract(value)
    }
}

impl fmt::Display for ArtifactExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactExecutionError {}

#[derive(Debug)]
pub enum CorpusExecutorError {
    InvalidPlan(crate::corpus_plan::CorpusPlanError),
    EmptyPlan,
    UnsupportedManifestPath(String),
    Service(ServiceInvocationError),
    ServiceContract(crate::executor::services::ServiceError),
    Cancelled(Cancelled),
    Scheduler(SchedulerError<ArtifactExecutionError>),
    Adapter(AdapterError),
    Manifest(ManifestProjectionError),
    Transaction(TransactionError),
    PrimaryAndCleanup {
        primary: Box<CorpusExecutorError>,
        cleanup: TransactionError,
    },
}

impl From<ServiceInvocationError> for CorpusExecutorError {
    fn from(value: ServiceInvocationError) -> Self {
        Self::Service(value)
    }
}

impl From<Cancelled> for CorpusExecutorError {
    fn from(value: Cancelled) -> Self {
        Self::Cancelled(value)
    }
}

impl From<AdapterError> for CorpusExecutorError {
    fn from(value: AdapterError) -> Self {
        Self::Adapter(value)
    }
}

impl From<ManifestProjectionError> for CorpusExecutorError {
    fn from(value: ManifestProjectionError) -> Self {
        Self::Manifest(value)
    }
}

impl From<TransactionError> for CorpusExecutorError {
    fn from(value: TransactionError) -> Self {
        Self::Transaction(value)
    }
}

impl fmt::Display for CorpusExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CorpusExecutorError {}
