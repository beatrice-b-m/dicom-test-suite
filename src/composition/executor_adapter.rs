//! Composition bindings for the shared plan-first executor.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::manifest::{
    EvidenceManifestEntryInput, ImportedEvidenceManifestEntryInput, MixedEvidenceManifestEntryInput,
};
use super::{
    BundleMemberProvenance, CompositionManifestAssembler, CompositionManifestInputs,
    GenericPlanValidator, ProviderInvocation as LegacyProviderInvocation,
    ProviderRequest as LegacyProviderRequest, ValidationCheck,
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
    MaterializationRequest, MaterializationResult, ProducedAsset, ProviderRequest, ProviderResult,
    RuleExecutionResult, ServiceEvidence, StagedAssetHandle, StagedAssetRegistry,
    StagingRelativePath, ToolIdentity, ValidationRequest, ValidationResult, ValidationStatus,
};
use crate::{PACKAGE_VERSION, sha256_hex};

#[derive(Debug, Clone)]
pub struct CompositionExecutionBundle {
    pub plan: CorpusPlan,
    pub bindings: BTreeMap<String, ArtifactExecutionBindings>,
    pub projection: Arc<CompositionProjectionContext>,
    pub source_assets: Vec<CompositionSourceAsset>,
    pub providers: BTreeMap<String, DeferredCompositionProvider>,
    pub external_dicom_providers: BTreeMap<String, Arc<dyn CompositionExternalDicomProvider>>,
    /// Exact temporary root used only by the allowlisted U5.6 advanced-default
    /// bridge. Ordinary caller content never creates planning scratch.
    pub planning_scratch_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CompositionProjectionContext {
    pub inputs: CompositionManifestInputs,
    pub members: BTreeMap<String, BundleMemberProvenance>,
}

#[derive(Debug, Clone)]
pub struct CompositionSourceAsset {
    pub handle: StagedAssetHandle,
    pub source: CompositionSource,
    pub staging_relative_path: StagingRelativePath,
    pub media_type: String,
    pub expected_size_bytes: u64,
    pub expected_sha256: String,
}

#[derive(Debug, Clone)]
pub enum CompositionSource {
    File(PathBuf),
    Inline(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct DeferredCompositionProvider {
    pub request: LegacyProviderRequest,
    pub invocation: LegacyProviderInvocation,
}

pub trait CompositionExternalDicomProvider: Send + Sync + std::fmt::Debug {
    fn invoke(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
        private_staging_root: &Path,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError>;
}

#[derive(Clone)]
pub struct CompositionExecutionServiceFactory {
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    sources: Arc<Vec<CompositionSourceAsset>>,
    providers: Arc<BTreeMap<String, DeferredCompositionProvider>>,
    external_dicom_providers: Arc<BTreeMap<String, Arc<dyn CompositionExternalDicomProvider>>>,
    auxiliary: Arc<dyn AuxiliaryMaterializationHandler>,
    planning_scratch_root: Option<PathBuf>,
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
            external_dicom_providers: Arc::new(bundle.external_dicom_providers.clone()),
            auxiliary,
            planning_scratch_root: bundle.planning_scratch_root.clone(),
        }
    }
}

impl ExecutionServiceFactory for CompositionExecutionServiceFactory {
    fn bind(
        &self,
        private_staging_root: &Path,
    ) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        let staged = self
            .sources
            .iter()
            .map(|source| stage_source(private_staging_root, source))
            .collect::<Result<Vec<_>, _>>();
        let scratch_cleanup = self
            .planning_scratch_root
            .as_deref()
            .map(remove_planning_scratch)
            .transpose()
            .map(|_| ());
        let initial_assets = match (staged, scratch_cleanup) {
            (Ok(assets), Ok(())) => assets,
            (Err(primary), Ok(())) => return Err(primary),
            (Ok(_), Err(cleanup)) => return Err(cleanup),
            (Err(primary), Err(cleanup)) => {
                return Err(ServiceInvocationError::new(
                    "source staging",
                    format!("{primary}; planning scratch cleanup also failed: {cleanup}"),
                ));
            }
        };
        let materializer =
            MaterializationDispatcher::new(private_staging_root, self.auxiliary.clone())
                .map_err(|error| service_error("materializer", error))?;
        Ok(Arc::new(CompositionBoundServices {
            staging_root: private_staging_root.to_owned(),
            bindings: self.bindings.clone(),
            providers: self.providers.clone(),
            external_dicom_providers: self.external_dicom_providers.clone(),
            initial_assets,
            materializer,
            materialized_plans: Mutex::new(BTreeMap::new()),
            imported_observations: Mutex::new(BTreeMap::new()),
            external_provider_tools: Mutex::new(BTreeMap::new()),
        }))
    }
}

struct CompositionBoundServices {
    staging_root: PathBuf,
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    providers: Arc<BTreeMap<String, DeferredCompositionProvider>>,
    external_dicom_providers: Arc<BTreeMap<String, Arc<dyn CompositionExternalDicomProvider>>>,
    initial_assets: Vec<ProducedAsset>,
    materializer: MaterializationDispatcher,
    materialized_plans: Mutex<BTreeMap<String, super::ResolvedInstancePlan>>,
    imported_observations:
        Mutex<BTreeMap<String, crate::executor::evidence::ImportedDicomObservation>>,
    external_provider_tools: Mutex<BTreeMap<String, ToolIdentity>>,
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
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        if let Some(provider) = self.external_dicom_providers.get(&request.request_id) {
            let result = provider.invoke(request, assets, &self.staging_root, cancellation)?;
            self.external_provider_tools
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(request.request_id.clone(), result.provider.clone());
            return Ok(result);
        }
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
        self.invoke_codec_cancellable(request, assets, &CancellationToken::new())
    }

    fn invoke_codec_cancellable(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        crate::executor::native_codec::execute_native_rle(request, cancellation, |binding| {
            binding_bytes(&self.staging_root, binding, assets)
        })
    }

    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        self.materialize_cancellable(request, assets, &CancellationToken::new())
    }

    fn materialize_cancellable(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<MaterializationResult, ServiceInvocationError> {
        let result = self
            .materializer
            .dispatch_cancellable(request, assets, cancellation)
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
        } else if let PlannedArtifact::ImportedDicom(artifact) = &request.artifact {
            let observation = result
                .evidence
                .iter()
                .find_map(|evidence| evidence.claims.get("imported_dicom_observation"))
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| service_error("materializer", error))?
                .ok_or_else(|| {
                    ServiceInvocationError::new(
                        "materializer",
                        "imported DICOM observation is missing",
                    )
                })?;
            self.imported_observations
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(artifact.logical_id.clone(), observation);
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
        let checks = match &request.artifact {
            PlannedArtifact::Dicom(artifact) => {
                let plan = self
                    .materialized_plans
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .get(&artifact.logical_id)
                    .cloned()
                    .unwrap_or_else(|| artifact.instance.clone());
                GenericPlanValidator.validate_file(
                    &plan,
                    self.staging_root.join(declaration.relative_path.as_str()),
                )
            }
            PlannedArtifact::ImportedDicom(artifact) => {
                let observation = self
                    .imported_observations
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .get(&artifact.logical_id)
                    .cloned()
                    .ok_or_else(|| {
                        ServiceInvocationError::new(
                            "validation",
                            "imported DICOM observation is missing",
                        )
                    })?;
                vec![
                    ValidationCheck {
                        layer: "part10".into(),
                        rule_id: "imported_dicom_identity".into(),
                        status: "passed".into(),
                        message: format!(
                            "provider object identity {} and transfer syntax were verified",
                            observation.sop_instance_uid
                        ),
                    },
                    ValidationCheck {
                        layer: "template".into(),
                        rule_id: "reference_closure".into(),
                        status: "passed".into(),
                        message: format!(
                            "{} ordered provider references match the immutable plan",
                            observation.references.len()
                        ),
                    },
                    ValidationCheck {
                        layer: "content".into(),
                        rule_id: "content_integrity".into(),
                        status: "passed".into(),
                        message: format!(
                            "{} bounded provider content fields were hashed",
                            observation.content.len()
                        ),
                    },
                ]
            }
            _ => {
                return Err(ServiceInvocationError::new(
                    "validation",
                    "unsupported artifact",
                ));
            }
        };
        let status = if checks.iter().any(|check| check.status == "failed") {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        };
        if request.plan.rules.is_empty() {
            return Err(ServiceInvocationError::new(
                "validation",
                "empty validation plan",
            ));
        }
        Ok(ValidationResult {
            artifact_id: request.artifact.logical_id().into(),
            validator: built_in_tool("composition_generic_plan_validator"),
            rules: request
                .plan
                .rules
                .iter()
                .enumerate()
                .map(|(index, rule)| RuleExecutionResult {
                    rule_id: rule.rule_id.clone(),
                    status,
                    message: if status == ValidationStatus::Passed {
                        format!("composition validation `{}` passed", rule.rule_id)
                    } else {
                        format!("composition validation `{}` failed", rule.rule_id)
                    },
                    measurements: (index == 0)
                        .then(|| {
                            BTreeMap::from([(
                                "checks".into(),
                                serde_json::to_value(&checks).expect("checks serialize"),
                            )])
                        })
                        .unwrap_or_default(),
                })
                .collect(),
            evidence: vec![],
        })
    }

    fn evaluate_obligation(
        &self,
        artifact: &PlannedArtifact,
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
        let tool = if obligation.independence == EvidenceIndependence::ExternalProvider {
            let PlannedArtifact::ImportedDicom(imported) = artifact else {
                return Err(ServiceInvocationError::new(
                    "obligation",
                    "external provider evidence requires an imported artifact",
                ));
            };
            let identity = self
                .external_provider_tools
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(&imported.provider.request_id)
                .cloned()
                .ok_or_else(|| {
                    ServiceInvocationError::new("obligation", "provider identity is missing")
                })?;
            Some(crate::executor::evidence::ToolEvidence {
                tool_id: identity.backend_id,
                version: identity.version,
                executable_sha256: identity.executable_sha256.ok_or_else(|| {
                    ServiceInvocationError::new("obligation", "provider fingerprint is missing")
                })?,
            })
        } else {
            None
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
            tool,
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
            .filter_map(|artifact| {
                let PlannedArtifact::Dicom(planned) = &artifact.planned else {
                    return None;
                };
                let mut plan = planned.instance.clone();
                inject_provider_provenance(&mut plan, &artifact.execution.providers);
                inject_materialized_content(
                    &mut plan,
                    artifact.execution.materialization.as_ref(),
                    &artifact.execution.codecs,
                );
                Some((planned.logical_id.clone(), plan))
            })
            .collect::<BTreeMap<_, _>>();
        let mut entries = Vec::with_capacity(input.artifacts.len());
        for artifact in &input.artifacts {
            let output = artifact.execution.output.as_ref().ok_or_else(|| {
                ManifestProjectionError(format!(
                    "{} has no output evidence",
                    artifact.planned.logical_id()
                ))
            })?;
            let member = self
                .context
                .members
                .get(artifact.planned.logical_id())
                .ok_or_else(|| {
                    ManifestProjectionError(format!(
                        "{} has no bundle metadata",
                        artifact.planned.logical_id()
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
                        artifact.planned.logical_id()
                    ))
                })?;
            let resolved_plan_sha256 = artifact
                .execution
                .instance_plan_sha256
                .clone()
                .ok_or_else(|| ManifestProjectionError("missing immutable plan hash".into()))?;
            match &artifact.planned {
                PlannedArtifact::Dicom(planned) => entries.push(
                    MixedEvidenceManifestEntryInput::Native(EvidenceManifestEntryInput {
                        plan: plans
                            .get(&planned.logical_id)
                            .expect("native plan was projected"),
                        resolved_plan_sha256,
                        relative_path: output.relative_path.clone(),
                        size_bytes: output.size_bytes,
                        sha256: output.sha256.clone(),
                        checks,
                        requested: member.requested,
                        bundle_root_instance_id: member.bundle_root_instance_id.clone(),
                        bundle_role: member.bundle_role.clone(),
                        source_provenance: member.source.clone(),
                        determinism: "byte_stable".into(),
                    }),
                ),
                PlannedArtifact::ImportedDicom(planned) => {
                    let observation = artifact
                        .execution
                        .materialization
                        .as_ref()
                        .and_then(|materialization| materialization.imported_dicom.as_ref())
                        .ok_or_else(|| {
                            ManifestProjectionError(format!(
                                "{} lacks imported DICOM observation",
                                planned.logical_id
                            ))
                        })?;
                    entries.push(MixedEvidenceManifestEntryInput::Imported(
                        ImportedEvidenceManifestEntryInput {
                            plan: planned,
                            observation,
                            resolved_plan_sha256,
                            relative_path: output.relative_path.clone(),
                            size_bytes: output.size_bytes,
                            sha256: output.sha256.clone(),
                            checks,
                            requested: member.requested,
                            bundle_root_instance_id: member.bundle_root_instance_id.clone(),
                            bundle_role: member.bundle_role.clone(),
                            source_provenance: member.source.clone(),
                        },
                    ));
                }
                _ => {
                    return Err(ManifestProjectionError(
                        "composition projection received a non-DICOM artifact".into(),
                    ));
                }
            }
        }
        let manifest = CompositionManifestAssembler
            .assemble_from_mixed_evidence(self.context.inputs.clone(), &entries)
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
        if !actual.compressed_frame_sha256.is_empty() || actual.writer_materialization.is_some() {
            content.properties.insert(
                "compressed_frame_sha256".into(),
                serde_json::to_string(&actual.compressed_frame_sha256)
                    .expect("frame hashes serialize"),
            );
        }
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
    let bytes = match &source.source {
        CompositionSource::File(path) => {
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| service_error("source staging", error))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ServiceInvocationError::new(
                    "source staging",
                    "source must be a non-symlink regular file",
                ));
            }
            fs::read(path).map_err(|error| service_error("source staging", error))?
        }
        CompositionSource::Inline(bytes) => bytes.clone(),
    };
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

pub(crate) fn remove_planning_scratch(path: &Path) -> Result<(), ServiceInvocationError> {
    let safe_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".dts-compose-") && !name.contains('/'));
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(service_error("planning scratch cleanup", error)),
    };
    if !safe_name || metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ServiceInvocationError::new(
            "planning scratch cleanup",
            format!("refusing unsafe planning scratch {}", path.display()),
        ));
    }
    fs::remove_dir_all(path).map_err(|error| service_error("planning scratch cleanup", error))
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
        ByteBinding::VerifiedAssetRange {
            asset,
            offset,
            length,
        } => {
            let declaration = assets
                .resolve(asset)
                .map_err(|error| service_error("codec", error))?;
            let bytes = fs::read(root.join(declaration.relative_path.as_str()))
                .map_err(|error| service_error("codec", error))?;
            if bytes.len() as u64 != declaration.size_bytes
                || sha256_hex(&bytes) != declaration.sha256
            {
                return Err(ServiceInvocationError::new(
                    "codec",
                    "verified provider asset changed before codec execution",
                ));
            }
            let start = usize::try_from(*offset).map_err(|error| service_error("codec", error))?;
            let end = offset
                .checked_add(*length)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| ServiceInvocationError::new("codec", "frame range overflow"))?;
            Ok(bytes
                .get(start..end)
                .ok_or_else(|| {
                    ServiceInvocationError::new("codec", "frame range is out of bounds")
                })?
                .to_vec())
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
