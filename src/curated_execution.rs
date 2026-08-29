//! Frontend-neutral execution services for curated Secondary Capture plans.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::composition::{CompositionUidRole, GenericPlanValidator, ResolvedInstancePlan};
use crate::corpus_plan::{EvidenceIndependence, EvidenceObligation, PlannedArtifact};
use crate::curated_plan::{CuratedArtifactProjectionContext, CuratedScCorpusPlan};
use crate::curated_validation::{
    ScPaddingValidation, ScPaletteValidation, ScPart10ValidationInput, ScPixelLengthFormula,
    TypedValidationCheck, validate_metadata_round_trip, validate_nonsquare_round_trip,
    validate_sc_part10,
};
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
    projection: Arc<BTreeMap<String, CuratedArtifactProjectionContext>>,
    planned_artifact_ids: Arc<BTreeSet<String>>,
}

impl CuratedExecutionServiceFactory {
    pub fn new(bundle: &CuratedScCorpusPlan) -> Self {
        Self {
            bindings: Arc::new(bundle.bindings.clone()),
            projection: Arc::new(
                bundle
                    .projection
                    .artifacts
                    .iter()
                    .cloned()
                    .map(|context| (context.artifact_id.clone(), context))
                    .collect(),
            ),
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
            projection: self.projection.clone(),
            materializer,
            materialized: Mutex::new(BTreeMap::new()),
            decoded_codec_frames: Mutex::new(BTreeMap::new()),
        }))
    }
}

struct CuratedBoundExecutionServices {
    staging_root: PathBuf,
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    projection: Arc<BTreeMap<String, CuratedArtifactProjectionContext>>,
    materializer: MaterializationDispatcher,
    materialized: Mutex<BTreeMap<String, MaterializedValidationState>>,
    decoded_codec_frames: Mutex<BTreeMap<String, Vec<String>>>,
}

struct MaterializedValidationState {
    plan: ResolvedInstancePlan,
    content: Vec<MaterializedContentEvidence>,
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
        let outcome =
            crate::executor::native_codec::execute_native_rle(request, cancellation, |binding| {
                binding_bytes(&self.staging_root, binding, assets)
            })?;
        self.decoded_codec_frames
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                request.artifact_id.clone(),
                outcome.decoded_frame_sha256.values().cloned().collect(),
            );
        Ok(outcome)
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
            self.materialized
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(
                    artifact.logical_id.clone(),
                    MaterializedValidationState { plan, content },
                );
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
        let materialized = self
            .materialized
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&artifact.logical_id)
            .map(|state| (state.plan.clone(), state.content.clone()))
            .ok_or_else(|| {
                ServiceInvocationError::new(
                    "validation",
                    "curated artifact has no completed materialization state",
                )
            })?;
        let (plan, content) = materialized;
        let checks = GenericPlanValidator.validate_file(
            &plan,
            self.staging_root.join(declaration.relative_path.as_str()),
        );
        if checks.iter().any(|check| check.status != "passed") {
            return Err(ServiceInvocationError::new(
                "validation",
                "shared resolved-plan validation failed",
            ));
        }
        let context = self.projection.get(&artifact.logical_id).ok_or_else(|| {
            ServiceInvocationError::new("validation", "missing curated projection context")
        })?;
        let sc = context
            .artifact_recipe
            .secondary_capture
            .as_ref()
            .ok_or_else(|| ServiceInvocationError::new("validation", "missing SC recipe"))?;
        let rows = u16::try_from(sc.rows).map_err(|error| service_error("validation", error))?;
        let columns =
            u16::try_from(sc.columns).map_err(|error| service_error("validation", error))?;
        let frames =
            u16::try_from(sc.frames).map_err(|error| service_error("validation", error))?;
        let pixel_content = content
            .iter()
            .find(|item| item.slot == "pixels")
            .ok_or_else(|| ServiceInvocationError::new("validation", "missing pixel evidence"))?;
        let encapsulated = artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.5";
        let codec_decoded = self
            .decoded_codec_frames
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&artifact.logical_id)
            .cloned()
            .unwrap_or_default();
        let decoded_source = if pixel_content.decoded_frame_sha256.is_empty() {
            &codec_decoded
        } else {
            &pixel_content.decoded_frame_sha256
        };
        let decoded_hashes = decoded_source
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let length_formula = if encapsulated {
            ScPixelLengthFormula::Encapsulated {
                fragments: usize::try_from(pixel_content.fragment_count)
                    .map_err(|error| service_error("validation", error))?,
                basic_offset_table_offsets: pixel_content.basic_offset_table.len(),
            }
        } else if sc.bits_allocated == 1 {
            ScPixelLengthFormula::BitPackedContinuousFrames
        } else if sc.photometric_interpretation == "YBR_FULL_422" {
            ScPixelLengthFormula::YbrFull422
        } else {
            ScPixelLengthFormula::ContiguousSamples
        };
        let palette = sc
            .palette
            .as_ref()
            .map(|palette| -> Result<_, ServiceInvocationError> {
                Ok(ScPaletteValidation {
                    descriptor: [
                        u16::try_from(palette.descriptor[0])
                            .map_err(|error| service_error("validation", error))?,
                        u16::try_from(palette.descriptor[1])
                            .map_err(|error| service_error("validation", error))?,
                        u16::try_from(palette.descriptor[2])
                            .map_err(|error| service_error("validation", error))?,
                    ],
                    red_data_length: palette.red.len() * 2,
                    green_data_length: palette.green.len() * 2,
                    blue_data_length: palette.blue.len() * 2,
                })
            })
            .transpose()?;
        let padding = sc
            .padding
            .as_ref()
            .map(|padding| -> Result<_, ServiceInvocationError> {
                Ok(ScPaddingValidation {
                    value: i16::try_from(padding.value)
                        .map_err(|error| service_error("validation", error))?,
                    range_limit: padding
                        .range_limit
                        .map(i16::try_from)
                        .transpose()
                        .map_err(|error| service_error("validation", error))?,
                })
            })
            .transpose()?;
        let sop_instance_uid = plan
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .ok_or_else(|| ServiceInvocationError::new("validation", "missing SOP Instance UID"))?;
        let pixel_vr = match sc.pixel_data_vr.as_str() {
            "OB" => dicom_core::VR::OB,
            "OW" => dicom_core::VR::OW,
            other => {
                return Err(ServiceInvocationError::new(
                    "validation",
                    format!("unsupported pixel VR {other}"),
                ));
            }
        };
        let mut typed = validate_sc_part10(
            &self.staging_root.join(declaration.relative_path.as_str()),
            &ScPart10ValidationInput {
                sop_class_uid: &plan.sop_class_uid,
                sop_instance_uid,
                transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                implementation_class_uid: &artifact.encoding.implementation.class_uid,
                rows,
                columns,
                frames,
                samples_per_pixel: sc.samples_per_pixel,
                photometric_interpretation: &sc.photometric_interpretation,
                bits_allocated: sc.bits_allocated,
                bits_stored: sc.bits_stored,
                high_bit: sc.high_bit,
                pixel_representation: sc.pixel_representation,
                planar_configuration: sc
                    .color
                    .as_ref()
                    .and_then(|color| color.planar_configuration)
                    .map(u16::from),
                pixel_data_vr: pixel_vr,
                pixel_data_length_formula: length_formula,
                decoded_frame_hashes: if encapsulated { &decoded_hashes } else { &[] },
                palette,
                padding,
            },
        )
        .map_err(|error| service_error("validation", error))?;
        typed.append(TypedValidationCheck::passed_internal(
            "curated_composition_plan",
            "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
        ));
        if let Some(metadata) = &context.artifact_recipe.metadata_sc {
            let (check, observation) = validate_metadata_round_trip(
                &self.staging_root.join(declaration.relative_path.as_str()),
                metadata,
            )
            .map_err(|error| service_error("validation", error))?;
            typed.append(check);
            typed.metadata_observation = Some(observation);
        } else if context
            .artifact_recipe
            .validation_rule_ids
            .iter()
            .any(|rule| rule == "validation.sc.geometry")
        {
            let (check, observation) = validate_nonsquare_round_trip(
                &self.staging_root.join(declaration.relative_path.as_str()),
                &context.artifact_recipe,
            )
            .map_err(|error| service_error("validation", error))?;
            typed.append(check);
            typed.metadata_observation = Some(observation);
        }
        let status = if typed.checks.iter().all(TypedValidationCheck::passed) {
            ValidationStatus::Passed
        } else {
            ValidationStatus::Failed
        };
        let mut measurements = BTreeMap::from([
            (
                "checks".into(),
                serde_json::to_value(&typed.checks)
                    .map_err(|error| service_error("validation", error))?,
            ),
            (
                "generic_plan_checks".into(),
                serde_json::to_value(checks).map_err(|error| service_error("validation", error))?,
            ),
        ]);
        if let Some(observation) = &typed.metadata_observation {
            measurements.insert(
                "metadata_observation".into(),
                serde_json::to_value(observation)
                    .map_err(|error| service_error("validation", error))?,
            );
        }
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
