//! Frontend-neutral execution services for curated Secondary Capture plans.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::composition::{GenericPlanValidator, ResolvedInstancePlan};
use crate::corpus_plan::{EvidenceIndependence, EvidenceObligation, PlannedArtifact};
use crate::curated_plan::CuratedScCorpusPlan;
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::{
    BoundExecutionServices, CodecServiceOutcome, ExecutionServiceFactory, ServiceInvocationError,
};
use crate::executor::evidence::{
    EvidenceIndependence as ExecutionIndependence, MaterializedContentEvidence, ObligationResult,
    ResultStatus,
};
use crate::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, CodecRequest, MaterializationRequest,
    MaterializationResult, ProviderRequest, ProviderResult, RuleExecutionResult,
    StagedAssetRegistry, ToolIdentity, ValidationRequest, ValidationResult, ValidationStatus,
};
use crate::{PACKAGE_VERSION, sha256_hex};

#[derive(Clone)]
pub struct CuratedExecutionServiceFactory {
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    planned_artifact_ids: Arc<BTreeSet<String>>,
}

impl CuratedExecutionServiceFactory {
    pub fn new(bundle: &CuratedScCorpusPlan) -> Self {
        Self {
            bindings: Arc::new(bundle.bindings.clone()),
            planned_artifact_ids: Arc::new(
                bundle
                    .plan
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.logical_id().to_owned())
                    .collect(),
            ),
        }
    }
}

impl ExecutionServiceFactory for CuratedExecutionServiceFactory {
    fn bind(
        &self,
        private_staging_root: &Path,
    ) -> Result<Arc<dyn BoundExecutionServices>, ServiceInvocationError> {
        let bound_artifact_ids = self.bindings.keys().cloned().collect::<BTreeSet<_>>();
        if bound_artifact_ids != *self.planned_artifact_ids {
            return Err(ServiceInvocationError::new(
                "curated bindings",
                "bundle plan and execution binding artifact sets differ",
            ));
        }
        let empty_assets = StagedAssetRegistry::default();
        for (artifact_id, binding) in self.bindings.iter() {
            if binding.artifact_id != *artifact_id {
                return Err(ServiceInvocationError::new(
                    "curated bindings",
                    format!("binding key {artifact_id} differs from its artifact identity"),
                ));
            }
            binding
                .validate(&empty_assets)
                .map_err(|error| service_error("curated bindings", error))?;
        }
        let materializer = MaterializationDispatcher::new(
            private_staging_root,
            Arc::new(RejectAuxiliaryMaterialization),
        )
        .map_err(|error| service_error("materializer", error))?;
        Ok(Arc::new(CuratedBoundExecutionServices {
            staging_root: private_staging_root.to_owned(),
            bindings: self.bindings.clone(),
            materializer,
            materialized_plans: Mutex::new(BTreeMap::new()),
        }))
    }
}

struct CuratedBoundExecutionServices {
    staging_root: PathBuf,
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    materializer: MaterializationDispatcher,
    materialized_plans: Mutex<BTreeMap<String, ResolvedInstancePlan>>,
}

impl BoundExecutionServices for CuratedBoundExecutionServices {
    fn bindings_for(
        &self,
        artifact: &PlannedArtifact,
    ) -> Result<ArtifactExecutionBindings, ServiceInvocationError> {
        self.bindings
            .get(artifact.logical_id())
            .cloned()
            .ok_or_else(|| {
                ServiceInvocationError::new(
                    "curated bindings",
                    format!("missing bindings for {}", artifact.logical_id()),
                )
            })
    }

    fn invoke_provider(
        &self,
        request: &ProviderRequest,
        _: &StagedAssetRegistry,
        _: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        Err(ServiceInvocationError::new(
            "provider",
            format!(
                "curated SC artifact unexpectedly requested provider {}",
                request.provider_id
            ),
        ))
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
            let content = materialized_content(&result)?;
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
            ServiceInvocationError::new("validation", "curated output is missing")
        })?;
        let declaration = assets
            .resolve(handle)
            .map_err(|error| service_error("validation", error))?;
        let PlannedArtifact::Dicom(artifact) = &request.artifact else {
            return Err(ServiceInvocationError::new(
                "validation",
                "curated execution accepts only DICOM artifacts",
            ));
        };
        let plan = self
            .materialized_plans
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&artifact.logical_id)
            .cloned()
            .unwrap_or_else(|| artifact.instance.clone());
        let checks = GenericPlanValidator.validate_file(
            &plan,
            self.staging_root.join(declaration.relative_path.as_str()),
        );
        let status = if checks.iter().any(|check| check.status == "failed") {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        };
        let measurements = BTreeMap::from([(
            "checks".into(),
            serde_json::to_value(checks).map_err(|error| service_error("validation", error))?,
        )]);
        Ok(ValidationResult {
            artifact_id: artifact.logical_id.clone(),
            validator: built_in_tool("curated_sc_plan_validator"),
            rules: request
                .plan
                .rules
                .iter()
                .map(|rule| RuleExecutionResult {
                    rule_id: rule.rule_id.clone(),
                    status,
                    message: format!(
                        "{}: shared curated DICOM plan validation {}",
                        rule.rule_id,
                        if status == ValidationStatus::Passed {
                            "passed"
                        } else {
                            "failed"
                        }
                    ),
                    measurements: measurements.clone(),
                })
                .collect(),
            evidence: Vec::new(),
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
            message: "curated execution obligation follows shared plan validation".into(),
            tool: None,
        })
    }

    fn actual_peak_working_bytes(
        &self,
        _: &PlannedArtifact,
        materialization: &MaterializationResult,
    ) -> Result<u64, ServiceInvocationError> {
        let Some(output) = &materialization.output else {
            return Ok(1);
        };
        let path = self
            .staging_root
            .join(output.declaration.relative_path.as_str());
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| service_error("resource accounting", error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ServiceInvocationError::new(
                "resource accounting",
                format!("refusing unsafe materialized output {}", path.display()),
            ));
        }
        if metadata.len() != output.observed_size_bytes {
            return Err(ServiceInvocationError::new(
                "resource accounting",
                "materialized output size changed before accounting",
            ));
        }
        Ok(metadata.len().max(1))
    }
}

struct RejectAuxiliaryMaterialization;

impl AuxiliaryMaterializationHandler for RejectAuxiliaryMaterialization {
    fn render(
        &self,
        artifact: &crate::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        Err(MaterializationError::Auxiliary(format!(
            "curated SC execution does not support auxiliary artifact {}",
            artifact.logical_id
        )))
    }
}

fn materialized_content(
    result: &MaterializationResult,
) -> Result<Vec<MaterializedContentEvidence>, ServiceInvocationError> {
    result
        .evidence
        .iter()
        .find_map(|evidence| evidence.claims.get("materialized_content"))
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| service_error("materializer evidence", error))
        .map(Option::unwrap_or_default)
}

fn patch_materialized_content(
    plan: &mut ResolvedInstancePlan,
    actuals: &[MaterializedContentEvidence],
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
            content.vr = crate::composition::DicomVr::OB;
        }
        content.size_bytes = actual.size_bytes;
        content.sha256 = actual.sha256.clone();
    }
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
            let selected = read_asset_range(root, assets, asset, *offset, *length, false)?;
            if sha256_hex(&selected) != *sha256 {
                return Err(ServiceInvocationError::new("codec", "staged frame changed"));
            }
            Ok(selected)
        }
        ByteBinding::VerifiedAssetRange {
            asset,
            offset,
            length,
        } => read_asset_range(root, assets, asset, *offset, *length, true),
    }
}

fn read_asset_range(
    root: &Path,
    assets: &StagedAssetRegistry,
    handle: &crate::executor::services::StagedAssetHandle,
    offset: u64,
    length: u64,
    verify_whole_asset: bool,
) -> Result<Vec<u8>, ServiceInvocationError> {
    let declaration = assets
        .resolve(handle)
        .map_err(|error| service_error("codec", error))?;
    let path = root.join(declaration.relative_path.as_str());
    let metadata = fs::symlink_metadata(&path).map_err(|error| service_error("codec", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ServiceInvocationError::new(
            "codec",
            format!("unsafe staged codec asset {}", path.display()),
        ));
    }
    let bytes = fs::read(&path).map_err(|error| service_error("codec", error))?;
    if verify_whole_asset
        && (bytes.len() as u64 != declaration.size_bytes
            || sha256_hex(&bytes) != declaration.sha256)
    {
        return Err(ServiceInvocationError::new(
            "codec",
            "verified asset changed before codec execution",
        ));
    }
    let start = usize::try_from(offset).map_err(|error| service_error("codec", error))?;
    let end = offset
        .checked_add(length)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| ServiceInvocationError::new("codec", "frame range overflow"))?;
    bytes
        .get(start..end)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ServiceInvocationError::new("codec", "frame range is out of bounds"))
}

fn built_in_tool(id: &str) -> ToolIdentity {
    ToolIdentity {
        backend_id: id.into(),
        version: PACKAGE_VERSION.into(),
        protocol_version: Some("0.1.0".into()),
        executable_sha256: None,
    }
}

fn service_error(stage: &'static str, error: impl std::fmt::Display) -> ServiceInvocationError {
    ServiceInvocationError::new(stage, error.to_string())
}
