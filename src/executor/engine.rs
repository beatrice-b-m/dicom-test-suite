//! Plan-first corpus orchestration and atomic publication.

use std::collections::BTreeMap;
use std::fmt;
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
    ArtifactExecutionBindings, CodecRequest, CodecResult, MaterializationRequest,
    MaterializationResult, ProviderRequest, ProviderResult, SlotExecutionBinding,
    StagedAssetRegistry, ValidationRequest, ValidationResult, ValidationStatus,
};
use crate::executor::transaction::{OutputTransaction, TransactionError};
use crate::sha256_hex;

pub trait BoundExecutionServices: Send + Sync {
    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError>;

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ProviderResult, ServiceInvocationError>;

    fn invoke_codec(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError>;

    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError>;

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
        let transaction = self
            .transactions
            .begin(destination.as_ref())
            .map_err(CorpusExecutorError::Transaction)?;
        let services = match self.services.bind(transaction.staging_root()) {
            Ok(services) => services,
            Err(error) => return Err(cleanup_failure(transaction, error.into())),
        };
        let registry = Arc::new(Mutex::new(StagedAssetRegistry::default()));
        let worker = ExecutionWorker {
            services,
            registry,
            cancellation,
        };
        let outcome = match schedule(plan, requested_parallelism, cancellation, &worker) {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(cleanup_failure(
                    transaction,
                    CorpusExecutorError::Scheduler(error),
                ));
            }
        };
        if let Err(error) =
            cancellation.checkpoint(CancellationPoint::run(CancellationStage::BeforeManifest))
        {
            return Err(cleanup_failure(transaction, error.into()));
        }

        let preliminary = match evidence_for(
            plan,
            outcome.clone(),
            requested_parallelism,
            0,
            PublicationTransition::staging(),
        ) {
            Ok(evidence) => evidence,
            Err(error) => return Err(cleanup_failure(transaction, error)),
        };
        let projection = match compatibility_projection_input(plan, &preliminary) {
            Ok(projection) => projection,
            Err(error) => return Err(cleanup_failure(transaction, error.into())),
        };
        let manifest = match self.projector.project(&projection) {
            Ok(manifest) => manifest,
            Err(error) => return Err(cleanup_failure(transaction, error.into())),
        };
        let manifest_sha256 = sha256_hex(&manifest);
        let evidence = match evidence_for(
            plan,
            outcome,
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
            evidence,
        })
    }
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
        for (slot, binding) in bindings.slots.clone() {
            match binding {
                SlotExecutionBinding::ProviderRequest { request } => {
                    self.checkpoint(CancellationStage::BeforeProvider, artifact_id)?;
                    let assets = self.asset_snapshot();
                    request.validate(&assets)?;
                    let result = self.services.invoke_provider(&request, &assets)?;
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
                    let outcome = self.services.invoke_codec(&request, &assets)?;
                    outcome.result.validate(&request, &assets)?;
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
        let materialization = self.services.materialize(&request, &assets)?;
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
        let peak_working_bytes = self
            .services
            .actual_peak_working_bytes(artifact, &materialization)?;
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
