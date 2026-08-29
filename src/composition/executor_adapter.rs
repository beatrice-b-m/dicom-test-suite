//! Composition bindings for the shared plan-first executor.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::manifest::EvidenceManifestEntryInput;
use super::{
    BundleMemberProvenance, CompositionManifestAssembler, CompositionManifestInputs,
    GenericPlanValidator, ProviderInvocation as LegacyProviderInvocation,
    ProviderRequest as LegacyProviderRequest, ValidationCheck,
};
use crate::codecs::{
    FrameDecodeInput, FrameDecoder, FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder,
};
use crate::corpus_plan::{CorpusPlan, EvidenceIndependence, EvidenceObligation, PlannedArtifact};
use crate::executor::adapters::ManifestProjectionCompatibilityInput;
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::{
    BoundExecutionServices, CodecServiceOutcome, ExecutionServiceFactory, ManifestProjectionError,
    ManifestProjector, ServiceInvocationError,
};
use crate::executor::evidence::{
    EvidenceIndependence as ExecutionIndependence, ObligationResult, ResultStatus,
};
use crate::executor::materialization::{
    AuxiliaryMaterializationHandler, MaterializationDispatcher,
};
use crate::executor::services::{
    ArtifactExecutionBindings, AssetDeclaration, AssetVisibility, ByteBinding, CodecRequest,
    CodecResult, EncodedFrameResult, MaterializationRequest, MaterializationResult, ProducedAsset,
    ProviderRequest, ProviderResult, RuleExecutionResult, ServiceEvidence, StagedAssetHandle,
    StagedAssetRegistry, StagingRelativePath, ToolIdentity, ValidationRequest, ValidationResult,
    ValidationStatus,
};
use crate::{PACKAGE_VERSION, sha256_hex};

#[derive(Debug, Clone)]
pub struct CompositionExecutionBundle {
    pub plan: CorpusPlan,
    pub bindings: BTreeMap<String, ArtifactExecutionBindings>,
    pub projection: Arc<CompositionProjectionContext>,
    pub source_assets: Vec<CompositionSourceAsset>,
    pub providers: BTreeMap<String, DeferredCompositionProvider>,
}

#[derive(Debug, Clone)]
pub struct CompositionProjectionContext {
    pub inputs: CompositionManifestInputs,
    pub members: BTreeMap<String, BundleMemberProvenance>,
}

#[derive(Debug, Clone)]
pub struct CompositionSourceAsset {
    pub handle: StagedAssetHandle,
    pub source_path: PathBuf,
    pub staging_relative_path: StagingRelativePath,
    pub media_type: String,
    pub expected_size_bytes: u64,
    pub expected_sha256: String,
}

#[derive(Debug, Clone)]
pub struct DeferredCompositionProvider {
    pub request: LegacyProviderRequest,
    pub invocation: LegacyProviderInvocation,
}

#[derive(Clone)]
pub struct CompositionExecutionServiceFactory {
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    sources: Arc<Vec<CompositionSourceAsset>>,
    providers: Arc<BTreeMap<String, DeferredCompositionProvider>>,
    auxiliary: Arc<dyn AuxiliaryMaterializationHandler>,
}

impl CompositionExecutionServiceFactory {
    pub fn new(
        bundle: &CompositionExecutionBundle,
        auxiliary: Arc<dyn AuxiliaryMaterializationHandler>,
    ) -> Self {
        Self {
            bindings: Arc::new(bundle.bindings.clone()),
            sources: Arc::new(bundle.source_assets.clone()),
            providers: Arc::new(bundle.providers.clone()),
            auxiliary,
        }
    }
}

impl ExecutionServiceFactory for CompositionExecutionServiceFactory {
    fn bind(
        &self,
        private_staging_root: &Path,
    ) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        let initial_assets = self
            .sources
            .iter()
            .map(|source| stage_source(private_staging_root, source))
            .collect::<Result<Vec<_>, _>>()?;
        let materializer =
            MaterializationDispatcher::new(private_staging_root, self.auxiliary.clone())
                .map_err(|error| service_error("materializer", error))?;
        Ok(Arc::new(CompositionBoundServices {
            staging_root: private_staging_root.to_owned(),
            bindings: self.bindings.clone(),
            providers: self.providers.clone(),
            initial_assets,
            materializer,
            materialized_plans: Mutex::new(BTreeMap::new()),
        }))
    }
}

struct CompositionBoundServices {
    staging_root: PathBuf,
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    providers: Arc<BTreeMap<String, DeferredCompositionProvider>>,
    initial_assets: Vec<ProducedAsset>,
    materializer: MaterializationDispatcher,
    materialized_plans: Mutex<BTreeMap<String, super::ResolvedInstancePlan>>,
}

impl BoundExecutionServices for CompositionBoundServices {
    fn initial_assets(&self) -> Result<Vec<ProducedAsset>, ServiceInvocationError> {
        Ok(self.initial_assets.clone())
    }

    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError> {
        self.bindings
            .get(artifact.logical_id())
            .cloned()
            .ok_or_else(|| ServiceInvocationError::new("bindings", artifact.logical_id()))
    }

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        _: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        let deferred = self.providers.get(&request.request_id).ok_or_else(|| {
            ServiceInvocationError::new("provider", "missing legacy provider invocation")
        })?;
        let root = self
            .staging_root
            .join(".providers")
            .join(&request.request_id);
        fs::create_dir_all(root.parent().expect("provider request root has a parent"))
            .map_err(|error| service_error("provider", error))?;
        let output = super::provider::invoke_content_provider_cancellable(
            &deferred.invocation,
            &deferred.request,
            &root,
            &|| cancellation.is_cancelled(),
        )
        .map_err(|error| service_error("provider", error))?;
        let relative = output.path.strip_prefix(&self.staging_root).map_err(|_| {
            ServiceInvocationError::new("provider", "provider output escaped staging")
        })?;
        let slot = deferred.request.output.slot.clone();
        let asset = ProducedAsset {
            declaration: AssetDeclaration {
                handle: StagedAssetHandle::new(format!("provider:{}:{}", request.request_id, slot))
                    .map_err(|error| service_error("provider", error))?,
                relative_path: StagingRelativePath::new(relative.to_string_lossy())
                    .map_err(|error| service_error("provider", error))?,
                size_bytes: output.size_bytes,
                sha256: output.sha256.clone(),
                media_type: deferred
                    .request
                    .output
                    .media_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".into()),
                visibility: AssetVisibility::Private,
            },
            observed_size_bytes: output.size_bytes,
            observed_sha256: output.sha256,
        };
        Ok(ProviderResult {
            request_id: request.request_id.clone(),
            provider: ToolIdentity {
                backend_id: output.provider_id,
                version: output.provider_version,
                protocol_version: Some(super::CONTENT_PROVIDER_PROTOCOL_VERSION.into()),
                executable_sha256: Some(output.executable_sha256.clone()),
            },
            outputs: BTreeMap::from([(slot, asset)]),
            evidence: vec![ServiceEvidence {
                evidence_id: format!("provider:legacy:{}", request.request_id),
                evidence_kind: "composition_provider".into(),
                producer: ToolIdentity {
                    backend_id: deferred.request.provider_id.clone(),
                    version: deferred.request.expected_provider_version.clone(),
                    protocol_version: Some(super::CONTENT_PROVIDER_PROTOCOL_VERSION.into()),
                    executable_sha256: Some(output.executable_sha256),
                },
                claims: BTreeMap::from([
                    (
                        "legacy_argument_sha256".into(),
                        json!(output.argument_sha256),
                    ),
                    ("legacy_request_sha256".into(), json!(output.request_sha256)),
                    (
                        "legacy_response_sha256".into(),
                        json!(output.response_sha256),
                    ),
                    ("network_policy".into(), json!("disabled")),
                    ("resource_outcome".into(), json!("within_limits")),
                    ("termination".into(), json!("exit_zero")),
                ]),
            }],
        })
    }

    fn invoke_codec(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        if request.backend_id != NativeRleLosslessEncoder::BACKEND_ID {
            return Err(ServiceInvocationError::new(
                "codec",
                format!("unsupported composition codec {}", request.backend_id),
            ));
        }
        let encoder = NativeRleLosslessEncoder::new();
        let backend = FrameEncoder::backend(&encoder);
        let bits_stored = request
            .parameters
            .get("bits_stored")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok());
        let mut encoded = Vec::with_capacity(request.frames.len());
        let mut decoded_frame_sha256 = BTreeMap::new();
        let mut native_content = Vec::new();
        for frame in &request.frames {
            let native = binding_bytes(&self.staging_root, &frame.bytes, assets)?;
            native_content.extend_from_slice(&native);
            let result = encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: &native,
                    rows: u16::try_from(frame.rows)
                        .map_err(|error| service_error("codec", error))?,
                    columns: u16::try_from(frame.columns)
                        .map_err(|error| service_error("codec", error))?,
                    samples_per_pixel: frame.samples_per_pixel,
                    bits_allocated: frame.bits_allocated,
                    bits_stored: bits_stored.unwrap_or(frame.bits_allocated),
                    photometric_interpretation: &frame.photometric_interpretation,
                })
                .map_err(|error| service_error("codec", error))?;
            let decoded = encoder
                .decode_frame(FrameDecodeInput {
                    encoded_frame: &result.bytes,
                    rows: u16::try_from(frame.rows)
                        .map_err(|error| service_error("codec", error))?,
                    columns: u16::try_from(frame.columns)
                        .map_err(|error| service_error("codec", error))?,
                    samples_per_pixel: frame.samples_per_pixel,
                    bits_allocated: frame.bits_allocated,
                    bits_stored: bits_stored.unwrap_or(frame.bits_allocated),
                    photometric_interpretation: &frame.photometric_interpretation,
                })
                .map_err(|error| service_error("codec", error))?;
            if decoded.native_bytes != native {
                return Err(ServiceInvocationError::new(
                    "codec",
                    format!("frame {} semantic round trip changed", frame.frame_number),
                ));
            }
            decoded_frame_sha256.insert(frame.frame_number, sha256_hex(&decoded.native_bytes));
            let hash = sha256_hex(&result.bytes);
            encoded.push(EncodedFrameResult {
                frame_number: frame.frame_number,
                encoded_size_bytes: result.bytes.len() as u64,
                encoded_sha256: hash.clone(),
                bytes: ByteBinding::Inline {
                    bytes: result.bytes,
                    sha256: hash,
                },
            });
        }
        Ok(CodecServiceOutcome {
            result: CodecResult {
                request_id: request.request_id.clone(),
                backend: ToolIdentity {
                    backend_id: backend.backend_id.into(),
                    version: backend.version.into(),
                    protocol_version: None,
                    executable_sha256: None,
                },
                frames: encoded,
                evidence: vec![],
            },
            determinism: "byte_stable".into(),
            decoded_frame_sha256,
            metrics: BTreeMap::new(),
            claims: BTreeMap::from([
                ("native_sha256".into(), json!(sha256_hex(&native_content))),
                (
                    "codec_backend_kind".into(),
                    json!(backend.backend_kind.as_str()),
                ),
                (
                    "codec_feature_gate".into(),
                    json!(backend.feature_gate.unwrap_or("none")),
                ),
                ("codec_availability".into(), json!("available")),
            ]),
        })
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        let result = self
            .materializer
            .dispatch(request, assets)
            .map_err(|error| service_error("materializer", error))?;
        if let PlannedArtifact::Dicom(artifact) = &request.artifact {
            let mut plan = artifact.instance.clone();
            let content = result
                .evidence
                .iter()
                .find_map(|evidence| evidence.claims.get("materialized_content"))
                .cloned()
                .map(
                    serde_json::from_value::<
                        Vec<crate::executor::evidence::MaterializedContentEvidence>,
                    >,
                )
                .transpose()
                .map_err(|error| service_error("materializer", error))?
                .unwrap_or_default();
            patch_materialized_content(&mut plan, &content);
            self.materialized_plans
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(artifact.logical_id.clone(), plan);
        }
        Ok(result)
    }

    fn validate(
        &self,
        request: &ValidationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceInvocationError> {
        let handle = request.materialized_asset.as_ref().ok_or_else(|| {
            ServiceInvocationError::new("validation", "composition output is missing")
        })?;
        let declaration = assets
            .resolve(handle)
            .map_err(|error| service_error("validation", error))?;
        let plan = match &request.artifact {
            PlannedArtifact::Dicom(artifact) => self
                .materialized_plans
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(&artifact.logical_id)
                .cloned()
                .unwrap_or_else(|| artifact.instance.clone()),
            _ => {
                return Err(ServiceInvocationError::new(
                    "validation",
                    "composition adapter accepts only DICOM artifacts",
                ));
            }
        };
        let checks = GenericPlanValidator.validate_file(
            &plan,
            self.staging_root.join(declaration.relative_path.as_str()),
        );
        let status = if checks.iter().any(|check| check.status == "failed") {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        };
        let rule_id = request
            .plan
            .rules
            .first()
            .map(|rule| rule.rule_id.clone())
            .ok_or_else(|| ServiceInvocationError::new("validation", "empty validation plan"))?;
        Ok(ValidationResult {
            artifact_id: request.artifact.logical_id().into(),
            validator: built_in_tool("composition_generic_plan_validator"),
            rules: vec![RuleExecutionResult {
                rule_id,
                status,
                message: if status == ValidationStatus::Passed {
                    "generic composition plan validation passed".into()
                } else {
                    "generic composition plan validation failed".into()
                },
                measurements: BTreeMap::from([(
                    "checks".into(),
                    serde_json::to_value(checks).expect("checks serialize"),
                )]),
            }],
            evidence: vec![],
        })
    }

    fn evaluate_obligation(
        &self,
        _: &PlannedArtifact,
        obligation: &EvidenceObligation,
        _: &MaterializationResult,
        validation: &ValidationResult,
        _: &StagedAssetRegistry,
    ) -> Result<ObligationResult, ServiceInvocationError> {
        let status = if validation
            .rules
            .iter()
            .all(|rule| rule.status == ValidationStatus::Passed)
        {
            ResultStatus::Passed
        } else {
            ResultStatus::Failed
        };
        Ok(ObligationResult {
            obligation_id: obligation.obligation_id.clone(),
            route_id: obligation.route_id.clone(),
            independence: match obligation.independence {
                EvidenceIndependence::SameProject => ExecutionIndependence::SameProject,
                EvidenceIndependence::IndependentTool => ExecutionIndependence::IndependentTool,
                EvidenceIndependence::ExternalProvider => ExecutionIndependence::ExternalProvider,
            },
            required: obligation.required,
            status,
            message: "composition manifest validation follows generic plan validation".into(),
            tool: None,
        })
    }

    fn actual_peak_working_bytes(
        &self,
        _: &PlannedArtifact,
        materialization: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError> {
        Ok(materialization
            .output
            .as_ref()
            .map_or(1, |output| output.observed_size_bytes.max(1)))
    }

    fn finalize_private_assets(
        &self,
        _: &StagedAssetRegistry,
    ) -> Result<(), ServiceInvocationError> {
        for relative in [".providers", ".composition-inputs"] {
            remove_owned_private_tree(&self.staging_root, relative)?;
        }
        Ok(())
    }
}

fn remove_owned_private_tree(
    staging_root: &Path,
    relative: &str,
) -> Result<(), ServiceInvocationError> {
    let path = staging_root.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(service_error("private asset cleanup", error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ServiceInvocationError::new(
            "private asset cleanup",
            format!(
                "refusing to remove unsafe private staging path {}",
                path.display()
            ),
        ));
    }
    fs::remove_dir_all(&path).map_err(|error| service_error("private asset cleanup", error))
}

#[derive(Clone)]
pub struct CompositionExecutorManifestProjector {
    context: Arc<CompositionProjectionContext>,
}

impl CompositionExecutorManifestProjector {
    pub fn new(context: Arc<CompositionProjectionContext>) -> Self {
        Self { context }
    }
}

impl ManifestProjector for CompositionExecutorManifestProjector {
    fn project(
        &self,
        input: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        let plans = input
            .artifacts
            .iter()
            .map(|artifact| {
                let PlannedArtifact::Dicom(planned) = &artifact.planned else {
                    return Err(ManifestProjectionError(
                        "composition projection received a non-DICOM artifact".into(),
                    ));
                };
                let mut plan = planned.instance.clone();
                inject_provider_provenance(&mut plan, &artifact.execution.providers);
                inject_materialized_content(
                    &mut plan,
                    artifact.execution.materialization.as_ref(),
                    &artifact.execution.codecs,
                );
                Ok(plan)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut entries = Vec::with_capacity(input.artifacts.len());
        for (artifact, plan) in input.artifacts.iter().zip(&plans) {
            let PlannedArtifact::Dicom(planned) = &artifact.planned else {
                return Err(ManifestProjectionError(
                    "composition projection received a non-DICOM artifact".into(),
                ));
            };
            let output = artifact.execution.output.as_ref().ok_or_else(|| {
                ManifestProjectionError(format!("{} has no output evidence", planned.logical_id))
            })?;
            let member = self
                .context
                .members
                .get(&planned.logical_id)
                .ok_or_else(|| {
                    ManifestProjectionError(format!(
                        "{} has no bundle metadata",
                        planned.logical_id
                    ))
                })?;
            let checks = artifact
                .execution
                .validation
                .iter()
                .find_map(|validation| validation.details.get("checks"))
                .cloned()
                .map(serde_json::from_value::<Vec<ValidationCheck>>)
                .transpose()
                .map_err(|error| ManifestProjectionError(error.to_string()))?
                .ok_or_else(|| {
                    ManifestProjectionError(format!(
                        "{} lacks generic validation checks",
                        planned.logical_id
                    ))
                })?;
            entries.push(EvidenceManifestEntryInput {
                plan,
                resolved_plan_sha256: artifact
                    .execution
                    .instance_plan_sha256
                    .clone()
                    .ok_or_else(|| ManifestProjectionError("missing immutable plan hash".into()))?,
                relative_path: output.relative_path.clone(),
                size_bytes: output.size_bytes,
                sha256: output.sha256.clone(),
                checks,
                requested: member.requested,
                bundle_root_instance_id: member.bundle_root_instance_id.clone(),
                bundle_role: member.bundle_role.clone(),
                source_provenance: member.source.clone(),
                determinism: "byte_stable".into(),
            });
        }
        let manifest = CompositionManifestAssembler
            .assemble_from_evidence(self.context.inputs.clone(), &entries)
            .map_err(|error| ManifestProjectionError(error.to_string()))?;
        let mut bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| ManifestProjectionError(error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn inject_provider_provenance(
    plan: &mut super::ResolvedInstancePlan,
    providers: &[crate::executor::evidence::ProviderEvidence],
) {
    for provider in providers {
        for (slot, output_sha256) in &provider.outputs {
            let Some(content) = plan
                .content
                .iter_mut()
                .find(|content| content.slot == *slot)
            else {
                continue;
            };
            content.properties.extend(BTreeMap::from([
                ("content_origin".into(), "provider".into()),
                ("provider_id".into(), provider.provider_id.clone()),
                ("provider_version".into(), provider.provider_version.clone()),
                ("provider_output_sha256".into(), output_sha256.clone()),
                (
                    "provider_protocol_version".into(),
                    super::CONTENT_PROVIDER_PROTOCOL_VERSION.into(),
                ),
                ("provider_network_policy".into(), "disabled".into()),
                ("provider_resource_outcome".into(), "within_limits".into()),
                ("provider_termination".into(), "exit_zero".into()),
            ]));
            if let Some(hash) = &provider.executable_sha256 {
                content
                    .properties
                    .insert("provider_executable_sha256".into(), hash.clone());
            }
            for (claim, property) in [
                ("legacy_argument_sha256", "provider_argument_sha256"),
                ("legacy_request_sha256", "provider_request_sha256"),
                ("legacy_response_sha256", "provider_response_sha256"),
            ] {
                if let Some(value) = provider.claims.get(claim).and_then(Value::as_str) {
                    content.properties.insert(property.into(), value.into());
                }
            }
        }
    }
}

fn inject_materialized_content(
    plan: &mut super::ResolvedInstancePlan,
    materialization: Option<&crate::executor::evidence::MaterializationEvidence>,
    codecs: &[crate::executor::evidence::CodecEvidence],
) {
    let Some(materialization) = materialization else {
        return;
    };
    patch_materialized_content(plan, &materialization.content);
    for actual in &materialization.content {
        let Some(content) = plan
            .content
            .iter_mut()
            .find(|content| content.slot == actual.slot)
        else {
            continue;
        };
        if let Some(codec) = codecs.iter().find(|codec| codec.slot == actual.slot) {
            content
                .properties
                .insert("codec_backend".into(), codec.backend_id.clone());
            content
                .properties
                .insert("codec_version".into(), codec.backend_version.clone());
            content
                .properties
                .insert("codec_determinism".into(), codec.determinism.clone());
            content.properties.insert(
                "decoded_frame_sha256".into(),
                serde_json::to_string(&codec.decoded_frame_sha256).expect("frame hashes serialize"),
            );
            content.properties.insert(
                "codec_semantic_validation".into(),
                "decoded_frame_hashes_match".into(),
            );
            for (name, value) in &codec.claims {
                if let Some(value) = value.as_str() {
                    content.properties.insert(name.clone(), value.into());
                }
            }
        }
    }
}

fn patch_materialized_content(
    plan: &mut super::ResolvedInstancePlan,
    actuals: &[crate::executor::evidence::MaterializedContentEvidence],
) {
    for actual in actuals {
        let Some(content) = plan
            .content
            .iter_mut()
            .find(|content| content.slot == actual.slot)
        else {
            continue;
        };
        content.kind = actual.kind.clone();
        if actual.vr == "OB" {
            content.vr = super::DicomVr::OB;
        }
        content.size_bytes = actual.size_bytes;
        content.sha256 = actual.sha256.clone();
        content.properties.insert(
            "compressed_frame_sha256".into(),
            serde_json::to_string(&actual.compressed_frame_sha256).expect("frame hashes serialize"),
        );
        if let Some(writer_materialization) = &actual.writer_materialization {
            content.properties.insert(
                "writer_materialization".into(),
                writer_materialization.clone(),
            );
        }
    }
}

fn stage_source(
    root: &Path,
    source: &CompositionSourceAsset,
) -> Result<ProducedAsset, ServiceInvocationError> {
    let metadata = fs::symlink_metadata(&source.source_path)
        .map_err(|error| service_error("source staging", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ServiceInvocationError::new(
            "source staging",
            "source must be a non-symlink regular file",
        ));
    }
    let bytes =
        fs::read(&source.source_path).map_err(|error| service_error("source staging", error))?;
    let hash = sha256_hex(&bytes);
    if bytes.len() as u64 != source.expected_size_bytes || hash != source.expected_sha256 {
        return Err(ServiceInvocationError::new(
            "source staging",
            "source identity changed before executor staging",
        ));
    }
    let destination = root.join(source.staging_relative_path.as_str());
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| service_error("source staging", error))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|error| service_error("source staging", error))?;
    file.write_all(&bytes)
        .map_err(|error| service_error("source staging", error))?;
    Ok(ProducedAsset {
        declaration: AssetDeclaration {
            handle: source.handle.clone(),
            relative_path: source.staging_relative_path.clone(),
            size_bytes: bytes.len() as u64,
            sha256: hash.clone(),
            media_type: source.media_type.clone(),
            visibility: AssetVisibility::Private,
        },
        observed_size_bytes: bytes.len() as u64,
        observed_sha256: hash,
    })
}

fn binding_bytes(
    root: &Path,
    binding: &ByteBinding,
    assets: &StagedAssetRegistry,
) -> Result<Vec<u8>, ServiceInvocationError> {
    match binding {
        ByteBinding::Inline { bytes, sha256 } => {
            if sha256_hex(bytes) != *sha256 {
                return Err(ServiceInvocationError::new("codec", "inline frame changed"));
            }
            Ok(bytes.clone())
        }
        ByteBinding::StagedRange {
            asset,
            offset,
            length,
            sha256,
        } => {
            let declaration = assets
                .resolve(asset)
                .map_err(|error| service_error("codec", error))?;
            let bytes = fs::read(root.join(declaration.relative_path.as_str()))
                .map_err(|error| service_error("codec", error))?;
            let start = usize::try_from(*offset).map_err(|error| service_error("codec", error))?;
            let end = offset
                .checked_add(*length)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| ServiceInvocationError::new("codec", "frame range overflow"))?;
            let selected = bytes.get(start..end).ok_or_else(|| {
                ServiceInvocationError::new("codec", "frame range is out of bounds")
            })?;
            if sha256_hex(selected) != *sha256 {
                return Err(ServiceInvocationError::new("codec", "staged frame changed"));
            }
            Ok(selected.to_vec())
        }
    }
}

fn built_in_tool(id: &str) -> ToolIdentity {
    ToolIdentity {
        backend_id: id.into(),
        version: PACKAGE_VERSION.into(),
        protocol_version: None,
        executable_sha256: None,
    }
}

fn service_error(stage: &'static str, error: impl std::fmt::Display) -> ServiceInvocationError {
    ServiceInvocationError::new(stage, error.to_string())
}
