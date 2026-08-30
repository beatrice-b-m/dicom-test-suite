use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use dicom_test_suite::corpus_plan::*;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::*;
use dicom_test_suite::executor::evidence::{
    EvidenceIndependence as ExecutionIndependence, ObligationResult, PublicationState, ResultStatus,
};
use dicom_test_suite::executor::services::*;
use dicom_test_suite::executor::transaction::TransactionError;
use dicom_test_suite::sha256_hex;

const TS: &str = "1.2.840.10008.1.2.1";
static NEXT_REAL_TRANSACTION: AtomicU64 = AtomicU64::new(0);

fn plan() -> CorpusPlan {
    let instance = ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: "artifact".into(),
        template_id: TemplateId("classic/secondary-capture/monochrome".into()),
        template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
        transfer_syntax_uid: TS.into(),
        identities: IdentityPlan::from_exact_values(
            "artifact",
            [
                (CompositionUidRole::SopInstance, 0, "2.25.101".into()),
                (CompositionUidRole::ImplementationClass, 0, "2.25.99".into()),
            ],
        )
        .unwrap(),
        attributes: vec![],
        content: vec![],
        references: vec![],
    };
    CorpusPlan {
        schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed: 5,
        artifacts: vec![PlannedArtifact::Dicom(PlannedDicomArtifact {
            logical_id: "artifact".into(),
            order: 0,
            provenance: ArtifactProvenance::Requested,
            case_binding: None,
            instance,
            output: OutputPlan {
                relative_path: OutputRelativePath::new("artifact.dcm").unwrap(),
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
                backend_id: "fake-materializer".into(),
            },
            validation: ValidationPlan {
                rules: vec![ValidationRule {
                    rule_id: "part10.identity".into(),
                    requirement: ValidationRequirement::Required,
                    parameters: BTreeMap::new(),
                }],
            },
            evidence: EvidencePlan {
                obligations: vec![EvidenceObligation {
                    obligation_id: "same-project.part10".into(),
                    route_id: "fake-validator".into(),
                    independence: EvidenceIndependence::SameProject,
                    required: true,
                    parameters: BTreeMap::new(),
                }],
            },
            resources: ArtifactResourceEstimate {
                output_bytes: 100,
                peak_working_bytes: 100,
            },
        })],
        dependencies: vec![],
        unavailable: vec![],
        publication: PublicationPlan {
            manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
            transaction: PublicationTransaction::AtomicNoReplace,
            private_staging: true,
            no_overwrite: true,
        },
        resources: ResourcePlan {
            max_artifacts: 1,
            max_total_output_bytes: 1_000,
            max_peak_working_bytes: 100,
            max_parallelism: 2,
        },
    }
}

fn two_artifact_plan() -> CorpusPlan {
    let mut plan = plan();
    let mut second = plan.artifacts[0].clone();
    let PlannedArtifact::Dicom(second) = &mut second else {
        unreachable!()
    };
    second.logical_id = "artifact-two".into();
    second.instance.instance_id = "artifact-two".into();
    second.order = 1;
    second.output.relative_path = OutputRelativePath::new("artifact-two.dcm").unwrap();
    plan.artifacts.push(PlannedArtifact::Dicom(second.clone()));
    plan.resources.max_artifacts = 2;
    plan.resources.max_total_output_bytes = 2_000;
    plan.resources.max_peak_working_bytes = 200;
    plan
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Success,
    ProviderFailure,
    CodecFailure,
    MaterializerFailure,
    ValidationFailure,
    Panic,
}

#[derive(Clone)]
struct Factory(Mode);

impl ExecutionServiceFactory for Factory {
    fn bind(&self, _: &Path) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        Ok(Arc::new(Services(self.0)))
    }
}

struct Services(Mode);

fn service_error(stage: &'static str) -> ServiceInvocationError {
    ServiceInvocationError::new(stage, "injected failure")
}

fn tool(id: &str) -> ToolIdentity {
    ToolIdentity {
        backend_id: id.into(),
        version: "1.0.0".into(),
        protocol_version: None,
        executable_sha256: None,
    }
}

fn produced(handle: &str, path: &str, bytes: &[u8], publish: bool) -> ProducedAsset {
    let mut asset = ProducedAsset::from_bytes(
        StagedAssetHandle::new(handle).unwrap(),
        StagingRelativePath::new(path).unwrap(),
        "application/dicom",
        bytes,
    );
    if publish {
        asset.declaration.visibility = AssetVisibility::PublicationCandidate;
    }
    asset
}

impl BoundExecutionServices for Services {
    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError> {
        let provider = ProviderRequest {
            request_id: format!("provider-request-{}", artifact.logical_id()),
            artifact_id: artifact.logical_id().into(),
            provider_id: "fake-provider".into(),
            required_version: "1.0.0".into(),
            parameters: BTreeMap::new(),
            input_assets: BTreeMap::new(),
            expected_outputs: vec![ProviderOutputExpectation {
                slot: "provider".into(),
                media_type: "application/dicom".into(),
                maximum_size_bytes: 10,
                expected_sha256: None,
            }],
        };
        let bytes = vec![1, 2];
        let hash = sha256_hex(&bytes);
        let codec = CodecRequest {
            request_id: format!("codec-request-{}", artifact.logical_id()),
            artifact_id: artifact.logical_id().into(),
            slot: "codec".into(),
            backend_id: "fake-codec".into(),
            source_transfer_syntax_uid: TS.into(),
            target_transfer_syntax_uid: "1.2.840.10008.1.2.4.50".into(),
            frames: vec![NativeFrameBinding {
                frame_number: 1,
                bytes: ByteBinding::Inline {
                    bytes,
                    sha256: hash,
                },
                rows: 1,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                photometric_interpretation: "MONOCHROME2".into(),
            }],
            parameters: BTreeMap::new(),
        };
        Ok(ArtifactExecutionBindings {
            artifact_id: artifact.logical_id().into(),
            slots: BTreeMap::from([
                (
                    "provider".into(),
                    SlotExecutionBinding::ProviderRequest { request: provider },
                ),
                (
                    "codec".into(),
                    SlotExecutionBinding::CodecRequest { request: codec },
                ),
            ]),
        })
    }

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        _: &StagedAssetRegistry,
        _: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        if self.0 == Mode::Panic {
            panic!("injected worker panic");
        }
        if self.0 == Mode::ProviderFailure {
            return Err(service_error("provider"));
        }
        Ok(ProviderResult {
            request_id: request.request_id.clone(),
            provider: tool("fake-provider"),
            outputs: BTreeMap::from([(
                "provider".into(),
                produced(
                    &format!("provider-output:artifact:{}", request.artifact_id),
                    &format!("private/{}.bin", request.artifact_id),
                    b"p",
                    false,
                ),
            )]),
            evidence: vec![],
        })
    }

    fn invoke_codec(
        &self,
        request: &CodecRequest,
        _: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        if self.0 == Mode::CodecFailure {
            return Err(service_error("codec"));
        }
        let bytes = vec![3, 4];
        let hash = sha256_hex(&bytes);
        Ok(CodecServiceOutcome {
            result: CodecResult {
                request_id: request.request_id.clone(),
                backend: tool("fake-codec"),
                frames: vec![EncodedFrameResult {
                    frame_number: 1,
                    bytes: ByteBinding::Inline {
                        bytes,
                        sha256: hash.clone(),
                    },
                    encoded_size_bytes: 2,
                    encoded_sha256: hash,
                }],
                evidence: vec![],
            },
            backend_kind: "in_process".into(),
            display_name: "Fake codec".into(),
            feature_gate: None,
            determinism: "byte_stable".into(),
            decoded_frame_sha256: BTreeMap::from([(1, "d".repeat(64))]),
            metrics: BTreeMap::new(),
            claims: BTreeMap::new(),
        })
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        _: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        if self.0 == Mode::MaterializerFailure {
            return Err(service_error("materializer"));
        }
        let output = request.artifact.output().expect("test DICOM has output");
        Ok(MaterializationResult {
            artifact_id: request.artifact.logical_id().into(),
            output: Some(produced(
                &format!("output:{}", request.artifact.logical_id()),
                output.relative_path.as_str(),
                b"dicom",
                true,
            )),
            backend: tool("fake-materializer"),
            evidence: vec![],
        })
    }

    fn validate(
        &self,
        request: &ValidationRequest,
        _: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceInvocationError> {
        Ok(ValidationResult {
            artifact_id: request.artifact.logical_id().into(),
            validator: tool("fake-validator"),
            rules: vec![RuleExecutionResult {
                rule_id: "part10.identity".into(),
                status: if self.0 == Mode::ValidationFailure {
                    ValidationStatus::Failed
                } else {
                    ValidationStatus::Passed
                },
                message: "validation result".into(),
                measurements: BTreeMap::new(),
            }],
            evidence: vec![],
        })
    }

    fn evaluate_obligation(
        &self,
        _: &PlannedArtifact,
        obligation: &EvidenceObligation,
        _: &MaterializationResult,
        _: &ValidationResult,
        _: &StagedAssetRegistry,
    ) -> Result<ObligationResult, ServiceInvocationError> {
        Ok(ObligationResult {
            obligation_id: obligation.obligation_id.clone(),
            route_id: obligation.route_id.clone(),
            independence: ExecutionIndependence::SameProject,
            required: obligation.required,
            status: ResultStatus::Passed,
            message: "obligation passed".into(),
            tool: None,
        })
    }

    fn actual_peak_working_bytes(
        &self,
        _: &PlannedArtifact,
        _: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError> {
        Ok(10)
    }
}

#[derive(Default)]
struct OverlapGate {
    arrived: Mutex<usize>,
    ready: Condvar,
}

impl OverlapGate {
    fn rendezvous(&self) -> Result<(), ServiceInvocationError> {
        let mut arrived = self.arrived.lock().unwrap();
        *arrived += 1;
        self.ready.notify_all();
        while *arrived < 2 {
            let (next, timeout) = self
                .ready
                .wait_timeout(arrived, Duration::from_secs(2))
                .unwrap();
            arrived = next;
            if timeout.timed_out() && *arrived < 2 {
                return Err(ServiceInvocationError::new(
                    "provider",
                    "independent provider calls were serialized",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct ConcurrentFactory(Arc<OverlapGate>);

impl ExecutionServiceFactory for ConcurrentFactory {
    fn bind(&self, _: &Path) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        Ok(Arc::new(ConcurrentServices(self.0.clone())))
    }
}

struct ConcurrentServices(Arc<OverlapGate>);

impl BoundExecutionServices for ConcurrentServices {
    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError> {
        Services(Mode::Success).bindings_for(artifact)
    }

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        self.0.rendezvous()?;
        Services(Mode::Success).invoke_provider(request, assets, cancellation)
    }

    fn invoke_codec(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        Services(Mode::Success).invoke_codec(request, assets)
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        Services(Mode::Success).materialize(request, assets)
    }

    fn validate(
        &self,
        request: &ValidationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceInvocationError> {
        Services(Mode::Success).validate(request, assets)
    }

    fn evaluate_obligation(
        &self,
        artifact: &PlannedArtifact,
        obligation: &EvidenceObligation,
        materialization: &MaterializationResult,
        validation: &ValidationResult,
        assets: &StagedAssetRegistry,
    ) -> Result<ObligationResult, ServiceInvocationError> {
        Services(Mode::Success).evaluate_obligation(
            artifact,
            obligation,
            materialization,
            validation,
            assets,
        )
    }

    fn actual_peak_working_bytes(
        &self,
        artifact: &PlannedArtifact,
        materialization: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError> {
        Services(Mode::Success).actual_peak_working_bytes(artifact, materialization)
    }
}

#[derive(Clone, Copy)]
enum BlockingStage {
    Codec,
    Materialization,
}

#[derive(Clone)]
struct BlockingFactory {
    stage: BlockingStage,
    entered: Arc<AtomicBool>,
}

impl ExecutionServiceFactory for BlockingFactory {
    fn bind(&self, _: &Path) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        Ok(Arc::new(BlockingServices {
            stage: self.stage,
            entered: self.entered.clone(),
        }))
    }
}

struct BlockingServices {
    stage: BlockingStage,
    entered: Arc<AtomicBool>,
}

#[derive(Clone)]
struct RealFileFactory;

impl ExecutionServiceFactory for RealFileFactory {
    fn bind(
        &self,
        staging: &Path,
    ) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        fs::create_dir(staging.join("private"))
            .map_err(|error| ServiceInvocationError::new("test bind", error.to_string()))?;
        fs::write(staging.join("private/input.bin"), b"private")
            .and_then(|_| fs::write(staging.join("undeclared.bin"), b"rogue"))
            .map_err(|error| ServiceInvocationError::new("test bind", error.to_string()))?;
        Ok(Arc::new(RealFileServices {
            staging: staging.to_owned(),
        }))
    }
}

struct RealFileServices {
    staging: PathBuf,
}

impl BoundExecutionServices for RealFileServices {
    fn initial_assets(&self) -> Result<Vec<ProducedAsset>, ServiceInvocationError> {
        Ok(vec![ProducedAsset::from_bytes(
            StagedAssetHandle::new("private-seed").unwrap(),
            StagingRelativePath::new("private/input.bin").unwrap(),
            "application/octet-stream",
            b"private",
        )])
    }

    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError> {
        Ok(ArtifactExecutionBindings {
            artifact_id: artifact.logical_id().into(),
            slots: BTreeMap::new(),
        })
    }

    fn invoke_provider(
        &self,
        _: &ProviderRequest,
        _: &StagedAssetRegistry,
        _: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        Err(service_error("unexpected provider"))
    }

    fn invoke_codec(
        &self,
        _: &CodecRequest,
        _: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        Err(service_error("unexpected codec"))
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        _: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        let output = request.artifact.output().unwrap();
        let path = self.staging.join(output.relative_path.as_str());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| ServiceInvocationError::new("test write", error.to_string()))?;
        }
        fs::write(&path, b"dicom")
            .map_err(|error| ServiceInvocationError::new("test write", error.to_string()))?;
        Ok(MaterializationResult {
            artifact_id: request.artifact.logical_id().into(),
            output: Some(produced(
                &format!("output:{}", request.artifact.logical_id()),
                output.relative_path.as_str(),
                b"dicom",
                true,
            )),
            backend: tool("real-file-materializer"),
            evidence: vec![],
        })
    }

    fn validate(
        &self,
        request: &ValidationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceInvocationError> {
        Services(Mode::Success).validate(request, assets)
    }

    fn evaluate_obligation(
        &self,
        artifact: &PlannedArtifact,
        obligation: &EvidenceObligation,
        materialization: &MaterializationResult,
        validation: &ValidationResult,
        assets: &StagedAssetRegistry,
    ) -> Result<ObligationResult, ServiceInvocationError> {
        Services(Mode::Success).evaluate_obligation(
            artifact,
            obligation,
            materialization,
            validation,
            assets,
        )
    }

    fn actual_peak_working_bytes(
        &self,
        _: &PlannedArtifact,
        _: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError> {
        Ok(5)
    }
}

impl BlockingServices {
    fn wait_for_cancel(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ServiceInvocationError> {
        self.entered.store(true, Ordering::Release);
        while !cancellation.is_cancelled() {
            std::thread::sleep(Duration::from_millis(2));
        }
        Err(ServiceInvocationError::new("blocking", "cancelled"))
    }
}

impl BoundExecutionServices for BlockingServices {
    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError> {
        Services(Mode::Success).bindings_for(artifact)
    }

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        Services(Mode::Success).invoke_provider(request, assets, cancellation)
    }

    fn invoke_codec(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        Services(Mode::Success).invoke_codec(request, assets)
    }

    fn invoke_codec_cancellable(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        if matches!(self.stage, BlockingStage::Codec) {
            self.wait_for_cancel(cancellation)?;
        }
        self.invoke_codec(request, assets)
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        Services(Mode::Success).materialize(request, assets)
    }

    fn materialize_cancellable(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        if matches!(self.stage, BlockingStage::Materialization) {
            self.wait_for_cancel(cancellation)?;
        }
        self.materialize(request, assets)
    }

    fn validate(
        &self,
        request: &ValidationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceInvocationError> {
        Services(Mode::Success).validate(request, assets)
    }

    fn evaluate_obligation(
        &self,
        artifact: &PlannedArtifact,
        obligation: &EvidenceObligation,
        materialization: &MaterializationResult,
        validation: &ValidationResult,
        assets: &StagedAssetRegistry,
    ) -> Result<ObligationResult, ServiceInvocationError> {
        Services(Mode::Success).evaluate_obligation(
            artifact,
            obligation,
            materialization,
            validation,
            assets,
        )
    }

    fn actual_peak_working_bytes(
        &self,
        artifact: &PlannedArtifact,
        materialization: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError> {
        Services(Mode::Success).actual_peak_working_bytes(artifact, materialization)
    }
}

#[derive(Default)]
struct TransactionState {
    cleanups: usize,
    promotions: usize,
    manifest_writes: usize,
}

#[derive(Clone, Default)]
struct Transactions {
    state: Arc<Mutex<TransactionState>>,
    fail_manifest: bool,
    fail_cleanup: bool,
    fail_promote: bool,
}

impl ExecutorTransactionFactory for Transactions {
    fn begin(&self, destination: &Path) -> Result<Box<dyn ExecutorTransaction>, TransactionError> {
        Ok(Box::new(FakeTransaction {
            destination: destination.to_owned(),
            staging: PathBuf::from("/private/tmp/fake-executor-staging"),
            factory: self.clone(),
        }))
    }
}

struct FakeTransaction {
    destination: PathBuf,
    staging: PathBuf,
    factory: Transactions,
}

impl ExecutorTransaction for FakeTransaction {
    fn staging_root(&self) -> &Path {
        &self.staging
    }

    fn write_manifest(&mut self, _: &[u8]) -> Result<(), TransactionError> {
        self.factory.state.lock().unwrap().manifest_writes += 1;
        if self.factory.fail_manifest {
            Err(tx_error("write manifest"))
        } else {
            Ok(())
        }
    }

    fn cleanup(self: Box<Self>) -> Result<(), TransactionError> {
        self.factory.state.lock().unwrap().cleanups += 1;
        if self.factory.fail_cleanup {
            Err(tx_error("cleanup"))
        } else {
            Ok(())
        }
    }

    fn promote(self: Box<Self>) -> Result<PathBuf, TransactionError> {
        self.factory.state.lock().unwrap().promotions += 1;
        if self.factory.fail_promote {
            Err(tx_error("promote"))
        } else {
            Ok(self.destination.clone())
        }
    }
}

fn tx_error(operation: &'static str) -> TransactionError {
    TransactionError::Io {
        operation,
        path: PathBuf::from("fake"),
        source: io::Error::other("injected transaction failure"),
    }
}

#[derive(Clone)]
struct Projector {
    fail: bool,
    cancel: Option<CancellationToken>,
}

impl ManifestProjector for Projector {
    fn project(
        &self,
        input: &dicom_test_suite::executor::adapters::ManifestProjectionInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        if let Some(token) = &self.cancel {
            token.cancel_with_reason("projector requested stop");
        }
        if self.fail {
            Err(ManifestProjectionError("injected manifest failure".into()))
        } else {
            serde_json::to_vec(&serde_json::json!({
                "plan_sha256": input.corpus_plan_sha256,
                "artifacts": input.artifacts.len(),
            }))
            .map_err(|error| ManifestProjectionError(error.to_string()))
        }
    }
}

fn executor(
    mode: Mode,
    transactions: Transactions,
    projector: Projector,
) -> CorpusExecutor<Factory, Projector, Transactions> {
    CorpusExecutor::with_transaction_factory(Factory(mode), projector, transactions)
}

#[test]
fn executes_services_projects_manifest_and_promotes_once() {
    let transactions = Transactions::default();
    let result = executor(
        Mode::Success,
        transactions.clone(),
        Projector {
            fail: false,
            cancel: None,
        },
    )
    .execute(&plan(), "/tmp/final-corpus", 2, &CancellationToken::new())
    .unwrap();

    assert_eq!(result.destination, PathBuf::from("/tmp/final-corpus"));
    assert_eq!(
        result.evidence.publication.state,
        PublicationState::Promoted
    );
    assert_eq!(result.evidence.artifacts[0].providers.len(), 1);
    assert_eq!(result.evidence.artifacts[0].codecs.len(), 1);
    assert_eq!(
        result.evidence.artifacts[0]
            .resources
            .actual_peak_working_bytes,
        Some(15)
    );
    assert_eq!(result.evidence.resources.used_parallelism, 1);
    let state = transactions.state.lock().unwrap();
    assert_eq!(state.manifest_writes, 1);
    assert_eq!(state.promotions, 1);
    assert_eq!(state.cleanups, 0);
}

#[test]
fn independent_service_calls_overlap_but_evidence_keeps_plan_order() {
    let transactions = Transactions::default();
    let executor = CorpusExecutor::with_transaction_factory(
        ConcurrentFactory(Arc::new(OverlapGate::default())),
        Projector {
            fail: false,
            cancel: None,
        },
        transactions,
    );
    let result = executor
        .execute(
            &two_artifact_plan(),
            "/tmp/concurrent-corpus",
            2,
            &CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(result.evidence.resources.used_parallelism, 2);
    assert_eq!(
        result
            .evidence
            .artifacts
            .iter()
            .map(|artifact| artifact.logical_id.as_str())
            .collect::<Vec<_>>(),
        vec!["artifact", "artifact-two"]
    );
}

#[test]
fn service_and_manifest_failures_cleanup_without_publication() {
    for mode in [
        Mode::ProviderFailure,
        Mode::CodecFailure,
        Mode::MaterializerFailure,
        Mode::ValidationFailure,
    ] {
        let transactions = Transactions::default();
        assert!(
            executor(
                mode,
                transactions.clone(),
                Projector {
                    fail: false,
                    cancel: None,
                },
            )
            .execute(&plan(), "/tmp/failing-corpus", 1, &CancellationToken::new())
            .is_err()
        );
        let state = transactions.state.lock().unwrap();
        assert_eq!(state.cleanups, 1, "mode {mode:?}");
        assert_eq!(state.promotions, 0, "mode {mode:?}");
    }

    let transactions = Transactions::default();
    assert!(
        executor(
            Mode::Success,
            transactions.clone(),
            Projector {
                fail: true,
                cancel: None,
            },
        )
        .execute(
            &plan(),
            "/tmp/manifest-failure",
            1,
            &CancellationToken::new()
        )
        .is_err()
    );
    assert_eq!(transactions.state.lock().unwrap().cleanups, 1);
}

#[test]
fn cancellation_is_checked_before_execution_and_before_promotion() {
    let cancelled = CancellationToken::new();
    cancelled.cancel_with_reason("before start");
    let transactions = Transactions::default();
    assert!(
        executor(
            Mode::Success,
            transactions.clone(),
            Projector {
                fail: false,
                cancel: None,
            },
        )
        .execute(&plan(), "/tmp/cancelled-before", 1, &cancelled)
        .is_err()
    );
    assert_eq!(transactions.state.lock().unwrap().cleanups, 0);

    let token = CancellationToken::new();
    let transactions = Transactions::default();
    assert!(
        executor(
            Mode::Success,
            transactions.clone(),
            Projector {
                fail: false,
                cancel: Some(token.clone()),
            },
        )
        .execute(&plan(), "/tmp/cancelled-promotion", 1, &token)
        .is_err()
    );
    let state = transactions.state.lock().unwrap();
    assert_eq!(state.manifest_writes, 1);
    assert_eq!(state.cleanups, 1);
    assert_eq!(state.promotions, 0);
}

#[test]
fn worker_panic_is_caught_and_staging_is_cleaned() {
    let transactions = Transactions::default();
    let error = executor(
        Mode::Panic,
        transactions.clone(),
        Projector {
            fail: false,
            cancel: None,
        },
    )
    .execute(&plan(), "/tmp/panic-corpus", 1, &CancellationToken::new())
    .unwrap_err();
    assert!(format!("{error:?}").contains("WorkerPanic"));
    assert_eq!(transactions.state.lock().unwrap().cleanups, 1);
}

#[test]
fn manifest_cleanup_and_promotion_races_preserve_both_failures() {
    let transactions = Transactions {
        fail_manifest: true,
        fail_cleanup: true,
        ..Transactions::default()
    };
    let error = executor(
        Mode::Success,
        transactions,
        Projector {
            fail: false,
            cancel: None,
        },
    )
    .execute(&plan(), "/tmp/paired-failure", 1, &CancellationToken::new())
    .unwrap_err();
    assert!(matches!(
        error,
        CorpusExecutorError::PrimaryAndCleanup { .. }
    ));

    let transactions = Transactions {
        fail_promote: true,
        ..Transactions::default()
    };
    assert!(
        executor(
            Mode::Success,
            transactions.clone(),
            Projector {
                fail: false,
                cancel: None,
            },
        )
        .execute(
            &plan(),
            "/tmp/raced-destination",
            1,
            &CancellationToken::new()
        )
        .is_err()
    );
    assert_eq!(transactions.state.lock().unwrap().promotions, 1);
}

#[test]
fn codec_and_encapsulation_transients_enforce_planned_peak_limit() {
    let mut bounded = plan();
    let PlannedArtifact::Dicom(artifact) = &mut bounded.artifacts[0] else {
        unreachable!()
    };
    // The fake codec holds two native plus two encoded bytes. Encapsulation
    // then accounts encoded frames, fragment copies, BOT, and the Part 10
    // output, producing a deterministic peak above this declared ceiling.
    artifact.resources.peak_working_bytes = 14;
    let transactions = Transactions::default();
    let error = executor(
        Mode::Success,
        transactions.clone(),
        Projector {
            fail: false,
            cancel: None,
        },
    )
    .execute(
        &bounded,
        "/tmp/resource-bounded-codec",
        1,
        &CancellationToken::new(),
    )
    .unwrap_err();
    assert!(format!("{error:?}").contains("ArtifactWorkingLimitExceeded"));
    let state = transactions.state.lock().unwrap();
    assert_eq!(state.cleanups, 1);
    assert_eq!(state.promotions, 0);
}

#[test]
fn blocking_codec_and_materializer_cancel_promptly_without_publication() {
    for stage in [BlockingStage::Codec, BlockingStage::Materialization] {
        let entered = Arc::new(AtomicBool::new(false));
        let transactions = Transactions::default();
        let executor = CorpusExecutor::with_transaction_factory(
            BlockingFactory {
                stage,
                entered: entered.clone(),
            },
            Projector {
                fail: false,
                cancel: None,
            },
            transactions.clone(),
        );
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            executor.execute(&plan(), "/tmp/blocking-cancel", 1, &worker_token)
        });
        while !entered.load(Ordering::Acquire) && started.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            entered.load(Ordering::Acquire),
            "blocking service never started"
        );
        token.cancel_with_reason("bounded cancellation test");
        let error = worker.join().unwrap().unwrap_err();
        assert!(format!("{error:?}").contains("Cancelled"));
        assert!(started.elapsed() < Duration::from_secs(1));
        let state = transactions.state.lock().unwrap();
        assert_eq!(state.cleanups, 1);
        assert_eq!(state.promotions, 0);
    }
}

#[test]
fn real_transaction_promotes_only_planned_public_outputs_and_manifest() {
    let parent = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "dts-executor-sanitize-{}-{}",
            std::process::id(),
            NEXT_REAL_TRANSACTION.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&parent).unwrap();
    let destination = parent.join("corpus");
    CorpusExecutor::new(
        RealFileFactory,
        Projector {
            fail: false,
            cancel: None,
        },
    )
    .execute(&plan(), &destination, 1, &CancellationToken::new())
    .unwrap();
    assert!(destination.join("artifact.dcm").is_file());
    assert!(destination.join("manifest.json").is_file());
    assert!(!destination.join("private").exists());
    assert!(!destination.join("undeclared.bin").exists());
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 2);
    fs::remove_dir_all(parent).unwrap();
}
