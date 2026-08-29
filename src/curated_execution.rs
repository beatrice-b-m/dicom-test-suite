//! Frontend-neutral execution services for curated Secondary Capture plans.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dicom_core::VR;
use serde::Deserialize;
use serde_json::Value;

use crate::composition::{
    CompositionUidRole, GenericPlanValidator, Part10Materializer, ResolvedInstancePlan,
    ValidationCheck,
};
use crate::corpus_plan::{
    EvidenceIndependence, EvidenceObligation, OffsetTablePolicy, PlannedArtifact,
};
use crate::curated_plan::{CuratedArtifactProjectionContext, CuratedScCorpusPlan};
use crate::curated_validation::{
    ExtendedOffsetTableValidationSpec, ScPaddingValidation, ScPaletteValidation,
    ScPart10ValidationInput, ScPixelLengthFormula, TypedValidationCheck, TypedValidationReport,
    validate_extended_offset_table_round_trip, validate_icc_profile_round_trip,
    validate_metadata_round_trip, validate_nonsquare_round_trip, validate_part10_with_expectations,
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
    SlotExecutionBinding, StagedAssetRegistry, ToolIdentity, ValidationRequest, ValidationResult,
    ValidationStatus,
};
use crate::recipes::classic_ct::{ClassicCtArtifactParameters, ClassicCtProviderParameters};
use crate::recipes::classic_dx_mg::{DxMgArtifactParameters, DxMgFamily};
use crate::recipes::classic_mr_cr::{CrArtifactParameters, MrArtifactParameters};
use crate::recipes::classic_nuclear::{
    ClassicNuclearArtifactParameters, ClassicNuclearProviderParameters,
};
use crate::recipes::classic_vl_projection::{
    ProjectionArtifactParameters, VlArtifactParameters, VlPhotometricInterpretation,
};
use crate::recipes::{
    CLASSIC_PIXEL_SLOT, EnhancedMrFrameAxis, PRESENTATION_ADVANCED_PROVIDER_ID, PresentationKind,
    REGISTRATION_PLAN_PROVIDER_ID, WsiArtifactParameters, WsiPixelAlgorithm,
};
use crate::validation::{
    AdvancedBlendingPresentationStateExpectations, AdvancedBlendingSourceSeriesExpectations,
    BlendingPresentationStateExpectations, BlendingSourceSeriesExpectations,
    ColorSoftcopyPresentationStateExpectations, CrImageExpectations, CtImageExpectations,
    DeformableSpatialRegistrationExpectations, DxImageExpectations,
    EnhancedCtConcatenationExpectations, EnhancedCtImageExpectations, EnhancedMrImageExpectations,
    EnhancedPetImageExpectations, MgImageExpectations, MrImageExpectations, NmDetectorExpectations,
    NmEnergyWindowExpectations, NmImageExpectations, PaletteExpectations, Part10Expectations,
    PetImageExpectations, PixelDataLengthFormula, PresentationStateExpectations,
    SpatialRegistrationExpectations, SpatialRegistrationReferenceExpectations, UsImageExpectations,
    UsMultiframeExpectations, XaImageExpectations, XrfImageExpectations,
    validate_advanced_blending_presentation_state_file, validate_blending_presentation_state_file,
    validate_color_softcopy_presentation_state_file, validate_deformable_spatial_registration_file,
    validate_part10_file, validate_presentation_state_file, validate_spatial_registration_file,
    validate_wsi_multiple_optical_paths_file, validate_wsi_pyramid_file,
    validate_wsi_tiled_full_file, validate_wsi_tiled_sparse_file,
};
use crate::{
    PACKAGE_VERSION, WsiPyramidLockedInputs, WsiPyramidMemberIdentity, WsiPyramidRole, sha256_hex,
    wsi_pyramid_locked_contract,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AdvancedCompatibilityProvider {
    Ct {
        common: AdvancedCompatibilityCommon,
        pixel_spacing: String,
        image_orientation_patient: String,
        slice_thickness: String,
        spacing_between_slices: String,
        rescale_intercept: String,
        rescale_slope: String,
        rescale_type: String,
        concatenation: bool,
        stress: bool,
    },
    Mr {
        common: AdvancedCompatibilityCommon,
        pixel_spacing: String,
        image_orientation_patient: String,
        slice_thickness: String,
        spacing_between_slices: String,
        rescale_intercept: String,
        rescale_slope: String,
        rescale_type: String,
        repetition_time: String,
        flip_angle: String,
        echo_train_length: String,
        rf_echo_train_length: u16,
        gradient_echo_train_length: u16,
        axis: EnhancedMrFrameAxis,
    },
    Pet {
        common: AdvancedCompatibilityCommon,
        pixel_spacing: String,
        image_orientation_patient: String,
        slice_thickness: String,
        spacing_between_slices: String,
        rescale_intercept: String,
        rescale_slope: String,
        units: String,
        counts_source: String,
        stack_id: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdvancedCompatibilityCommon {
    pub modality: String,
    pub study_id: String,
    pub device_serial_number: String,
    pub image_type: String,
    pub rows: u16,
    pub columns: u16,
    pub frame_type: String,
    pub pixel_presentation: String,
    pub volumetric_properties: String,
    pub volume_based_calculation_technique: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdvancedCompatibilityArtifact {
    pub frames: AdvancedCompatibilityFrames,
    pub pixels: AdvancedCompatibilityPixels,
    #[serde(default)]
    pub in_concatenation_number: Option<u16>,
    #[serde(default)]
    pub concatenation_frame_offset_number: Option<u32>,
    #[serde(default)]
    pub temporal_position_indices: Vec<u32>,
    #[serde(default)]
    pub in_stack_position_numbers: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AdvancedCompatibilityFrames {
    Literal {
        values: Vec<crate::recipes::EnhancedFrameGeometry>,
    },
    AxialLinear {
        frame_count: u32,
        start_z: f64,
        spacing: f64,
        first_dimension_index: u32,
    },
}

impl AdvancedCompatibilityFrames {
    pub(crate) fn expand(&self) -> Result<Vec<crate::recipes::EnhancedFrameGeometry>, String> {
        match self {
            Self::Literal { values } => Ok(values.clone()),
            Self::AxialLinear {
                frame_count,
                start_z,
                spacing,
                first_dimension_index,
            } => (0..*frame_count)
                .map(|index| {
                    let z = *start_z + *spacing * f64::from(index);
                    let z = if z.fract() == 0.0 {
                        format!("{z:.0}")
                    } else {
                        z.to_string()
                    };
                    Ok(crate::recipes::EnhancedFrameGeometry {
                        image_position_patient: format!("0\\0\\{z}"),
                        dimension_index_value: first_dimension_index
                            .checked_add(index)
                            .ok_or_else(|| "advanced frame dimension overflow".to_string())?,
                    })
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AdvancedCompatibilityPixels {
    Literal {
        stored_values: Vec<i64>,
        pixel_min: i64,
        pixel_max: i64,
    },
    ModuloRamp {
        modulus: u32,
    },
}

impl AdvancedCompatibilityPixels {
    pub(crate) fn values(&self, count: usize) -> Result<(Vec<i64>, i64, i64), String> {
        match self {
            Self::Literal {
                stored_values,
                pixel_min,
                pixel_max,
            } => Ok((stored_values.clone(), *pixel_min, *pixel_max)),
            Self::ModuloRamp { modulus } if *modulus > 0 => {
                let values = (0..count)
                    .map(|index| i64::try_from(index % *modulus as usize).unwrap())
                    .collect::<Vec<_>>();
                Ok((values, 0, i64::from(*modulus - 1)))
            }
            Self::ModuloRamp { .. } => Err("advanced pixel modulus is zero".into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WsiCompatibilityArtifact {
    pub kind: crate::recipes::WholeSlideArtifactKind,
    pub level: u32,
    pub file_index: usize,
    pub parameters: WsiArtifactParameters,
    pub pixel_algorithm: WsiPixelAlgorithm,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationValidationParameters {
    #[serde(default)]
    uid_reference_index: Option<u32>,
    presentation: PresentationKind,
    sources: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationValidationParameters {
    series_number: String,
    study_id: String,
    laterality: String,
    manufacturer_model_name: String,
    device_serial_number: String,
    content_label: String,
    content_description: String,
    registration: crate::recipes::RegistrationKindInput,
    sources: Vec<Value>,
}

pub(crate) fn advanced_provider_parameters(
    context: &CuratedArtifactProjectionContext,
) -> Result<AdvancedCompatibilityProvider, ServiceInvocationError> {
    serde_json::from_value(Value::Object(
        context.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| service_error("advanced recipe", error))
}

pub(crate) fn advanced_artifact_parameters(
    context: &CuratedArtifactProjectionContext,
) -> Result<AdvancedCompatibilityArtifact, ServiceInvocationError> {
    serde_json::from_value(Value::Object(context.artifact_recipe.parameters.clone()))
        .map_err(|error| service_error("advanced artifact", error))
}

pub(crate) fn wsi_artifact_parameters(
    context: &CuratedArtifactProjectionContext,
) -> Result<WsiCompatibilityArtifact, ServiceInvocationError> {
    serde_json::from_value(Value::Object(context.artifact_recipe.parameters.clone()))
        .map_err(|error| service_error("WSI artifact", error))
}

#[derive(Clone)]
pub struct CuratedExecutionServiceFactory {
    bindings: Arc<BTreeMap<String, ArtifactExecutionBindings>>,
    projection: Arc<BTreeMap<String, CuratedArtifactProjectionContext>>,
    planned_artifact_ids: Arc<BTreeSet<String>>,
    planned_artifacts: Arc<BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>>,
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
            planned_artifacts: Arc::new(
                bundle
                    .plan
                    .artifacts
                    .iter()
                    .filter_map(|artifact| match artifact {
                        PlannedArtifact::Dicom(artifact) => {
                            Some((artifact.logical_id.clone(), artifact.clone()))
                        }
                        PlannedArtifact::Auxiliary(_)
                        | PlannedArtifact::Mutation(_)
                        | PlannedArtifact::Qualification(_) => None,
                    })
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
            planned_artifacts: self.planned_artifacts.clone(),
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
    planned_artifacts: Arc<BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>>,
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
        let mut checks = GenericPlanValidator.validate_file(
            &plan,
            self.staging_root.join(declaration.relative_path.as_str()),
        );
        let context = self.projection.get(&artifact.logical_id).ok_or_else(|| {
            ServiceInvocationError::new("validation", "missing curated projection context")
        })?;
        qualify_declared_classic_vr_exceptions(context, &mut checks)?;
        if checks.iter().any(|check| check.status != "passed") {
            return Err(ServiceInvocationError::new(
                "validation",
                format!(
                    "shared resolved-plan validation failed: {}",
                    checks
                        .iter()
                        .filter(|check| check.status != "passed")
                        .map(|check| format!("{}: {}", check.rule_id, check.message))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            ));
        }
        if context.case_recipe.plan_provider_id == "native.classic_plan" {
            let mut typed = validate_classic_part10(
                &self.staging_root.join(declaration.relative_path.as_str()),
                artifact,
                context,
                &plan,
                &content,
                &self
                    .decoded_codec_frames
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .get(&artifact.logical_id)
                    .cloned()
                    .unwrap_or_default(),
            )?;
            typed.append(TypedValidationCheck::passed_internal(
                "curated_composition_plan",
                "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
            ));
            if context.artifact_recipe.algorithm_provider_id.as_deref()
                == Some("algorithm.classic_ct")
            {
                typed.append(validate_classic_ct_group(
                    context,
                    &self.projection,
                    &self.planned_artifacts,
                )?);
            }
            return validation_result(
                request,
                artifact,
                checks,
                typed,
                "curated_classic_plan_validator",
            );
        }
        if matches!(
            context.case_recipe.plan_provider_id.as_str(),
            "native.enhanced_plan" | "native.wsi_plan"
        ) {
            let planned_slots = plan
                .content
                .iter()
                .map(|item| item.slot.as_str())
                .collect::<BTreeSet<_>>();
            let observed_slots = content
                .iter()
                .map(|item| item.slot.as_str())
                .collect::<BTreeSet<_>>();
            if planned_slots != observed_slots {
                return Err(ServiceInvocationError::new(
                    "validation",
                    "advanced materialized content evidence does not close over planned slots",
                ));
            }
            let (mut typed, validator_id) =
                if context.case_recipe.plan_provider_id == "native.enhanced_plan" {
                    (
                        validate_enhanced_compatibility(
                            &self.staging_root.join(declaration.relative_path.as_str()),
                            artifact,
                            context,
                            &content,
                        )?,
                        "curated_enhanced_plan_validator",
                    )
                } else {
                    (
                        validate_wsi_compatibility(
                            &self.staging_root.join(declaration.relative_path.as_str()),
                            artifact,
                            context,
                            &content,
                            &self.projection,
                            &self.planned_artifacts,
                            &self.bindings,
                        )?,
                        "curated_wsi_plan_validator",
                    )
                };
            typed.append(TypedValidationCheck::passed_internal(
                if context.case_recipe.plan_provider_id == "native.enhanced_plan" {
                    "enhanced_plan_materialization_round_trip"
                } else {
                    "wsi_plan_materialization_round_trip"
                },
                "The provider-owned advanced plan identities, structure, and content survived shared materialization and typed reopen validation.",
            ));
            typed.append(TypedValidationCheck::passed_internal(
                "curated_composition_plan",
                "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
            ));
            return validation_result(request, artifact, checks, typed, validator_id);
        }
        if matches!(
            context.case_recipe.plan_provider_id.as_str(),
            REGISTRATION_PLAN_PROVIDER_ID | PRESENTATION_ADVANCED_PROVIDER_ID
        ) {
            let planned_slots = plan
                .content
                .iter()
                .map(|item| item.slot.as_str())
                .collect::<BTreeSet<_>>();
            let observed_slots = content
                .iter()
                .map(|item| item.slot.as_str())
                .collect::<BTreeSet<_>>();
            if planned_slots != observed_slots || !planned_slots.is_empty() {
                return Err(ServiceInvocationError::new(
                    "validation",
                    "reference object unexpectedly contains materialized payload slots",
                ));
            }
            validate_materialized_reference_sources(
                &plan,
                &self.materialized,
                &self.planned_artifacts,
                &self.staging_root,
            )?;
            let (mut typed, provider_name) =
                if context.case_recipe.plan_provider_id == PRESENTATION_ADVANCED_PROVIDER_ID {
                    (
                        validate_presentation_compatibility(
                            &self.staging_root.join(declaration.relative_path.as_str()),
                            artifact,
                            context,
                            &plan,
                            &self.planned_artifacts,
                        )?,
                        "presentation_state",
                    )
                } else {
                    (
                        validate_registration_compatibility(
                            &self.staging_root.join(declaration.relative_path.as_str()),
                            artifact,
                            context,
                            &plan,
                            &self.planned_artifacts,
                        )?,
                        "registration",
                    )
                };
            typed.append(TypedValidationCheck::passed_internal(
                "curated_composition_plan",
                "The curated dataset resolved through the shared composition plan before Part 10 materialization.",
            ));
            return validation_result(
                request,
                artifact,
                checks,
                typed,
                &format!("curated_{provider_name}_plan_validator"),
            );
        }
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
        if artifact.encoding.offset_table == OffsetTablePolicy::Extended {
            let projection = sc.encapsulation_projection.as_ref().ok_or_else(|| {
                ServiceInvocationError::new(
                    "validation",
                    "Extended offsets lack typed projection parameters",
                )
            })?;
            let page_numbers = (1..=sc.frames)
                .map(|value| {
                    i32::try_from(value).map_err(|error| service_error("validation", error))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (check, observation) = validate_extended_offset_table_round_trip(
                &self.staging_root.join(declaration.relative_path.as_str()),
                &ExtendedOffsetTableValidationSpec {
                    offsets: pixel_content.extended_offset_table.clone(),
                    lengths: pixel_content.extended_offset_table_lengths.clone(),
                    compressed_fragment_lengths: pixel_content.compressed_lengths.clone(),
                    padded_fragment_lengths: pixel_content.padded_fragment_lengths.clone(),
                    fragments_per_frame: pixel_content.fragments_per_frame.clone(),
                    fragment_item_start_offsets: pixel_content
                        .fragments
                        .iter()
                        .map(|fragment| fragment.item_start_offset)
                        .collect(),
                    page_numbers,
                    offset_origin: projection.offset_origin.clone(),
                    item_header_bytes: u64::from(projection.item_header_bytes),
                },
            )
            .map_err(|error| service_error("validation", error))?;
            typed.append(check);
            typed.metadata_observation = Some(observation);
        }
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

fn validation_result(
    request: &ValidationRequest,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    generic_checks: Vec<ValidationCheck>,
    typed: TypedValidationReport,
    validator_id: &str,
) -> Result<ValidationResult, ServiceInvocationError> {
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
            serde_json::to_value(generic_checks)
                .map_err(|error| service_error("validation", error))?,
        ),
    ]);
    if let Some(observation) = typed.metadata_observation {
        measurements.insert(
            "metadata_observation".into(),
            serde_json::to_value(observation)
                .map_err(|error| service_error("validation", error))?,
        );
    }
    Ok(ValidationResult {
        artifact_id: artifact.logical_id.clone(),
        validator: built_in_tool(validator_id),
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

fn validate_materialized_reference_sources(
    plan: &ResolvedInstancePlan,
    materialized: &Mutex<BTreeMap<String, MaterializedValidationState>>,
    planned_artifacts: &BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>,
    staging_root: &Path,
) -> Result<(), ServiceInvocationError> {
    let states = materialized
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for reference in &plan.references {
        let source = states.get(&reference.target_instance_id).ok_or_else(|| {
            ServiceInvocationError::new(
                "reference validation",
                format!(
                    "referenced source {} has no completed materialization",
                    reference.target_instance_id
                ),
            )
        })?;
        let source_artifact = planned_artifacts
            .get(&reference.target_instance_id)
            .ok_or_else(|| {
                ServiceInvocationError::new(
                    "reference validation",
                    format!(
                        "referenced source {} has no planned artifact",
                        reference.target_instance_id
                    ),
                )
            })?;
        let failures = GenericPlanValidator
            .validate_file(
                &source.plan,
                staging_root.join(source_artifact.output.relative_path.as_str()),
            )
            .into_iter()
            .filter(|check| check.status != "passed")
            .map(|check| format!("{}: {}", check.rule_id, check.message))
            .collect::<Vec<_>>();
        if !failures.is_empty() {
            return Err(ServiceInvocationError::new(
                "reference validation",
                format!(
                    "referenced source {} failed resolved-plan validation: {}",
                    reference.target_instance_id,
                    failures.join("; ")
                ),
            ));
        }
    }
    Ok(())
}

fn validate_presentation_compatibility(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    plan: &ResolvedInstancePlan,
    planned_artifacts: &BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    const ICC_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
    const PALETTE_SHA256: &str = "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5";
    let parameters: PresentationValidationParameters = serde_json::from_value(Value::Object(
        context.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| service_error("presentation validation", error))?;
    let _ = (parameters.uid_reference_index, parameters.sources.len());
    let sources = plan
        .references
        .iter()
        .map(|reference| {
            planned_artifacts
                .get(&reference.target_instance_id)
                .ok_or_else(|| {
                    ServiceInvocationError::new(
                        "presentation validation",
                        format!("missing source {}", reference.target_instance_id),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sop = required_identity(artifact, CompositionUidRole::SopInstance)?;
    let implementation = required_identity(artifact, CompositionUidRole::ImplementationClass)?;
    let study = required_identity(artifact, CompositionUidRole::StudyInstance)?;
    let series = required_identity(artifact, CompositionUidRole::SeriesInstance)?;
    let mut report = match &parameters.presentation {
        PresentationKind::Grayscale(item) => {
            let [source] = sources.as_slice() else {
                return Err(ServiceInvocationError::new(
                    "presentation validation",
                    "grayscale presentation requires one source",
                ));
            };
            legacy_validated_report(validate_presentation_state_file(
                path,
                &PresentationStateExpectations {
                    sop_class_uid: &artifact.instance.sop_class_uid,
                    sop_instance_uid: sop,
                    transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                    implementation_class_uid: implementation,
                    synthetic_data: "YES",
                    modality: "PR",
                    presentation_label: &item.content_label,
                    referenced_series_instance_uid: required_identity(
                        source,
                        CompositionUidRole::SeriesInstance,
                    )?,
                    referenced_sop_class_uid: &source.instance.sop_class_uid,
                    referenced_sop_instance_uid: required_identity(
                        source,
                        CompositionUidRole::SopInstance,
                    )?,
                    displayed_area_top_left: item.displayed_area.top_left.to_vec(),
                    displayed_area_bottom_right: item.displayed_area.bottom_right.to_vec(),
                    presentation_size_mode: &item.displayed_area.size_mode,
                    presentation_pixel_aspect_ratio: item
                        .displayed_area
                        .pixel_aspect_ratio
                        .to_vec(),
                    window_center: &item.window_center,
                    window_width: &item.window_width,
                    presentation_lut_shape: &item.presentation_lut_shape,
                },
            ))?
        }
        PresentationKind::Color(_) => {
            let [source] = sources.as_slice() else {
                return Err(ServiceInvocationError::new(
                    "presentation validation",
                    "color presentation requires one source",
                ));
            };
            legacy_validated_report(validate_color_softcopy_presentation_state_file(
                path,
                &ColorSoftcopyPresentationStateExpectations {
                    sop_class_uid: &artifact.instance.sop_class_uid,
                    sop_instance_uid: sop,
                    transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                    implementation_class_uid: implementation,
                    synthetic_data: "YES",
                    study_instance_uid: study,
                    series_instance_uid: series,
                    source_study_instance_uid: required_identity(
                        source,
                        CompositionUidRole::StudyInstance,
                    )?,
                    source_series_instance_uid: required_identity(
                        source,
                        CompositionUidRole::SeriesInstance,
                    )?,
                    source_sop_class_uid: &source.instance.sop_class_uid,
                    source_sop_instance_uid: required_identity(
                        source,
                        CompositionUidRole::SopInstance,
                    )?,
                    icc_profile_sha256: ICC_SHA256,
                },
            ))?
        }
        PresentationKind::Blending(_) | PresentationKind::AdvancedBlending(_) => {
            let [a, b, c, d] = sources.as_slice() else {
                return Err(ServiceInvocationError::new(
                    "presentation validation",
                    "blending presentation requires four sources",
                ));
            };
            let source_series = [
                (
                    required_identity(a, CompositionUidRole::SeriesInstance)?,
                    &a.instance.sop_class_uid,
                    [
                        required_identity(a, CompositionUidRole::SopInstance)?,
                        required_identity(b, CompositionUidRole::SopInstance)?,
                    ],
                ),
                (
                    required_identity(c, CompositionUidRole::SeriesInstance)?,
                    &c.instance.sop_class_uid,
                    [
                        required_identity(c, CompositionUidRole::SopInstance)?,
                        required_identity(d, CompositionUidRole::SopInstance)?,
                    ],
                ),
            ];
            match &parameters.presentation {
                PresentationKind::Blending(_) => {
                    legacy_validated_report(validate_blending_presentation_state_file(
                        path,
                        &BlendingPresentationStateExpectations {
                            sop_class_uid: &artifact.instance.sop_class_uid,
                            sop_instance_uid: sop,
                            transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                            implementation_class_uid: implementation,
                            synthetic_data: "YES",
                            study_instance_uid: study,
                            series_instance_uid: series,
                            source_series: [
                                BlendingSourceSeriesExpectations {
                                    series_instance_uid: source_series[0].0,
                                    sop_class_uid: source_series[0].1,
                                    sop_instance_uids: source_series[0].2,
                                },
                                BlendingSourceSeriesExpectations {
                                    series_instance_uid: source_series[1].0,
                                    sop_class_uid: source_series[1].1,
                                    sop_instance_uids: source_series[1].2,
                                },
                            ],
                            palette_channel_sha256: PALETTE_SHA256,
                            icc_profile_sha256: ICC_SHA256,
                        },
                    ))?
                }
                PresentationKind::AdvancedBlending(_) => {
                    legacy_validated_report(validate_advanced_blending_presentation_state_file(
                        path,
                        &AdvancedBlendingPresentationStateExpectations {
                            sop_class_uid: &artifact.instance.sop_class_uid,
                            sop_instance_uid: sop,
                            transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                            implementation_class_uid: implementation,
                            synthetic_data: "YES",
                            study_instance_uid: study,
                            series_instance_uid: series,
                            frame_of_reference_uid: required_identity(
                                artifact,
                                CompositionUidRole::FrameOfReference,
                            )?,
                            source_series: [
                                AdvancedBlendingSourceSeriesExpectations {
                                    series_instance_uid: source_series[0].0,
                                    sop_class_uid: source_series[0].1,
                                    sop_instance_uids: source_series[0].2,
                                },
                                AdvancedBlendingSourceSeriesExpectations {
                                    series_instance_uid: source_series[1].0,
                                    sop_class_uid: source_series[1].1,
                                    sop_instance_uids: source_series[1].2,
                                },
                            ],
                            icc_profile_sha256: ICC_SHA256,
                        },
                    ))?
                }
                _ => unreachable!(),
            }
        }
    };
    if !matches!(parameters.presentation, PresentationKind::Grayscale(_)) {
        let (name, message) = match parameters.presentation {
            PresentationKind::Color(_) => (
                "color_softcopy_source_precheck",
                "Rust reopened and hashed the RGB source, then verified its manifest identity, Explicit VR Little Endian encoding, single-frame 2x2 interleaved RGB shape, and 8-bit depth before construction.",
            ),
            PresentationKind::Blending(_) => (
                "blending_source_precheck",
                "Rust reopened and hashed all four source CT files and verified exact Study, Series, Frame of Reference, SOP, transfer syntax, geometry, and ordering before construction.",
            ),
            PresentationKind::AdvancedBlending(_) => (
                "advanced_blending_source_precheck",
                "Rust reopened and hashed all four source CT files and verified exact Study, Series, Frame of Reference, SOP, transfer syntax, geometry, and ordering before construction.",
            ),
            PresentationKind::Grayscale(_) => unreachable!(),
        };
        report.append(TypedValidationCheck::passed_internal(name, message));
    }
    Ok(report)
}

fn validate_registration_compatibility(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    plan: &ResolvedInstancePlan,
    planned_artifacts: &BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let parameters: RegistrationValidationParameters = serde_json::from_value(Value::Object(
        context.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| service_error("registration validation", error))?;
    let _ = parameters.sources.len();
    let sources = plan
        .references
        .iter()
        .map(|reference| {
            planned_artifacts
                .get(&reference.target_instance_id)
                .ok_or_else(|| {
                    ServiceInvocationError::new(
                        "registration validation",
                        format!("missing source {}", reference.target_instance_id),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let [target, source] = sources.as_slice() else {
        return Err(ServiceInvocationError::new(
            "registration validation",
            "registration requires two sources",
        ));
    };
    let sop = required_identity(artifact, CompositionUidRole::SopInstance)?;
    let implementation = required_identity(artifact, CompositionUidRole::ImplementationClass)?;
    let study = required_identity(artifact, CompositionUidRole::StudyInstance)?;
    let series = required_identity(artifact, CompositionUidRole::SeriesInstance)?;
    let registered = required_identity(target, CompositionUidRole::FrameOfReference)?;
    let mut report = match &parameters.registration {
        crate::recipes::RegistrationKindInput::Spatial(item) => {
            let fixed = parse_matrix(&item.fixed_matrix)?;
            let moving = parse_matrix(&item.moving_matrix)?;
            legacy_validated_report(validate_spatial_registration_file(
                path,
                &SpatialRegistrationExpectations {
                    sop_class_uid: &artifact.instance.sop_class_uid,
                    sop_instance_uid: sop,
                    transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                    implementation_class_uid: implementation,
                    synthetic_data: "YES",
                    patient_id: "DTS-PATIENT-001",
                    study_instance_uid: study,
                    study_id: &parameters.study_id,
                    series_instance_uid: series,
                    series_number: &parameters.series_number,
                    laterality: &parameters.laterality,
                    modality: "REG",
                    instance_number: "1",
                    content_date: "20260101",
                    content_time: "000000",
                    content_label: &parameters.content_label,
                    content_description: &parameters.content_description,
                    content_creator_name: "DTS^Generator",
                    manufacturer: "dicom-test-suite",
                    manufacturer_model_name: &parameters.manufacturer_model_name,
                    device_serial_number: &parameters.device_serial_number,
                    software_versions: PACKAGE_VERSION,
                    registered_frame_of_reference_uid: registered,
                    target: registration_reference(target)?,
                    source: registration_reference(source)?,
                    target_matrix: fixed,
                    source_to_registered_matrix: moving,
                    source_landmark_mm: [-0.625, -0.625, 0.0],
                    registered_landmark_mm: [0.0, 0.0, 2.5],
                    rigid_tolerance: 0.000001,
                },
            ))?
        }
        crate::recipes::RegistrationKindInput::Deformable(item) => {
            let pre = parse_matrix(&item.pre_deformation_matrix)?;
            let post = parse_matrix(&item.post_deformation_matrix)?;
            let vectors = item
                .vector_grid_data
                .chunks_exact(3)
                .map(|v| [v[0], v[1], v[2]])
                .collect::<Vec<_>>();
            let registered_points = [
                [0.0, 0.0, 2.5],
                [0.75, 0.0, 2.5],
                [0.0, 0.75, 2.5],
                [0.75, 0.75, 2.5],
            ];
            let source_points = [
                [-0.625, -0.625, 0.0],
                [0.0, -0.625, 0.0],
                [-0.625, 0.0, 0.0],
                [0.0, 0.0, 0.0],
            ];
            legacy_validated_report(validate_deformable_spatial_registration_file(
                path,
                &DeformableSpatialRegistrationExpectations {
                    sop_class_uid: &artifact.instance.sop_class_uid,
                    sop_instance_uid: sop,
                    transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                    implementation_class_uid: implementation,
                    synthetic_data: "YES",
                    patient_id: "DTS-PATIENT-001",
                    study_instance_uid: study,
                    study_id: &parameters.study_id,
                    series_instance_uid: series,
                    series_number: &parameters.series_number,
                    laterality: &parameters.laterality,
                    modality: "REG",
                    instance_number: "1",
                    content_date: "20260101",
                    content_time: "000000",
                    content_label: &parameters.content_label,
                    content_description: &parameters.content_description,
                    content_creator_name: "DTS^Generator",
                    manufacturer: "dicom-test-suite",
                    manufacturer_model_name: &parameters.manufacturer_model_name,
                    device_serial_number: &parameters.device_serial_number,
                    software_versions: PACKAGE_VERSION,
                    registered_frame_of_reference_uid: registered,
                    target: registration_reference(target)?,
                    source: registration_reference(source)?,
                    pre_matrix: pre,
                    post_matrix: post,
                    image_orientation_patient: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    image_position_patient: [0.0, 0.0, 2.5],
                    grid_dimensions: item.grid_dimensions,
                    grid_resolution: item.grid_resolution,
                    vector_grid_data_sha256: "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47",
                    decoded_vectors_mm: &vectors,
                    registered_points_mm: &registered_points,
                    source_points_mm: &source_points,
                    tolerance: 0.000001,
                },
            ))?
        }
    };
    let (name, message) = if matches!(
        parameters.registration,
        crate::recipes::RegistrationKindInput::Spatial(_)
    ) {
        (
            "spatial_registration_source_geometry",
            "Rust reopened both CT sources and verified identities, hashes, Frames of Reference, and locked geometry before construction.",
        )
    } else {
        (
            "deformable_registration_source_geometry",
            "Rust reopened both CT sources and verified identities, hashes, Frames of Reference, and locked geometry before construction.",
        )
    };
    report.append(TypedValidationCheck::passed_internal(name, message));
    Ok(report)
}

fn parse_matrix(values: &[String; 16]) -> Result<[f64; 16], ServiceInvocationError> {
    let mut parsed = [0.0; 16];
    for (index, value) in values.iter().enumerate() {
        parsed[index] = value
            .parse()
            .map_err(|error| service_error("registration validation", error))?;
    }
    Ok(parsed)
}

fn registration_reference(
    item: &crate::corpus_plan::PlannedDicomArtifact,
) -> Result<SpatialRegistrationReferenceExpectations<'_>, ServiceInvocationError> {
    Ok(SpatialRegistrationReferenceExpectations {
        study_instance_uid: required_identity(item, CompositionUidRole::StudyInstance)?,
        series_instance_uid: required_identity(item, CompositionUidRole::SeriesInstance)?,
        sop_class_uid: &item.instance.sop_class_uid,
        sop_instance_uid: required_identity(item, CompositionUidRole::SopInstance)?,
        frame_of_reference_uid: required_identity(item, CompositionUidRole::FrameOfReference)?,
    })
}

fn validate_enhanced_compatibility(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    content: &[MaterializedContentEvidence],
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let provider = advanced_provider_parameters(context)?;
    let item = advanced_artifact_parameters(context)?;
    let frames = item
        .frames
        .expand()
        .map_err(|error| ServiceInvocationError::new("advanced validation", error))?;
    let positions = frames
        .iter()
        .map(|frame| frame.image_position_patient.as_str())
        .collect::<Vec<_>>();
    let dimension_values = frames
        .iter()
        .map(|frame| frame.dimension_index_value)
        .collect::<Vec<_>>();
    let frame_count =
        u16::try_from(frames.len()).map_err(|error| service_error("advanced validation", error))?;
    let sop = required_identity(artifact, CompositionUidRole::SopInstance)?;
    let implementation = required_identity(artifact, CompositionUidRole::ImplementationClass)?;
    let frame_of_reference = required_identity(artifact, CompositionUidRole::FrameOfReference)?;
    let dimension = required_identity(artifact, CompositionUidRole::DimensionOrganization)?;
    let pixel = content
        .iter()
        .find(|item| item.slot == "pixels")
        .ok_or_else(|| ServiceInvocationError::new("advanced validation", "missing pixels"))?;
    let frame_hashes = pixel
        .decoded_frame_sha256
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut expected = base_advanced_expectations(
        artifact,
        sop,
        implementation,
        frame_count,
        1,
        "MONOCHROME2",
        16,
        16,
        15,
        0,
        None,
        VR::OW,
        &[],
    );
    match provider {
        AdvancedCompatibilityProvider::Ct {
            common,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness: _,
            spacing_between_slices: _,
            rescale_intercept,
            rescale_slope,
            rescale_type,
            concatenation,
            stress: _,
        } => {
            let irradiation = required_identity(artifact, CompositionUidRole::IrradiationEvent)?;
            let concatenation_uid = planned_text(&artifact.instance, "0020,9161");
            let concatenation_source = planned_text(&artifact.instance, "0020,0242");
            let concatenation_expectation = if concatenation {
                Some(EnhancedCtConcatenationExpectations {
                    concatenation_uid: concatenation_uid.as_deref().ok_or_else(|| {
                        ServiceInvocationError::new(
                            "advanced validation",
                            "missing Concatenation UID",
                        )
                    })?,
                    in_concatenation_number: item.in_concatenation_number.ok_or_else(|| {
                        ServiceInvocationError::new(
                            "advanced validation",
                            "missing concatenation number",
                        )
                    })?,
                    in_concatenation_total_number: u16::try_from(
                        context
                            .case_recipe
                            .dicom
                            .as_ref()
                            .map_or(0, |dicom| dicom.artifacts.len()),
                    )
                    .map_err(|error| service_error("advanced validation", error))?,
                    concatenation_frame_offset_number: item
                        .concatenation_frame_offset_number
                        .ok_or_else(|| {
                            ServiceInvocationError::new(
                                "advanced validation",
                                "missing concatenation frame offset",
                            )
                        })?,
                    sop_instance_uid_of_concatenation_source: concatenation_source
                        .as_deref()
                        .ok_or_else(|| {
                            ServiceInvocationError::new(
                                "advanced validation",
                                "missing concatenation source UID",
                            )
                        })?,
                })
            } else {
                None
            };
            expected.enhanced_ct_image = Some(EnhancedCtImageExpectations {
                modality: "CT",
                frame_of_reference_uid: frame_of_reference,
                image_type: &common.frame_type,
                number_of_frames: frame_count,
                shared_functional_groups: 1,
                per_frame_functional_groups: frames.len(),
                dimension_organization_uid: dimension,
                dimension_index_count: 1,
                pixel_spacing: &pixel_spacing,
                image_orientation_patient: &image_orientation_patient,
                image_position_patient: &positions,
                dimension_index_values: &dimension_values,
                frame_type: &common.frame_type,
                pixel_presentation: &common.pixel_presentation,
                volumetric_properties: &common.volumetric_properties,
                volume_based_calculation_technique: &common.volume_based_calculation_technique,
                rescale_intercept: &rescale_intercept,
                rescale_slope: &rescale_slope,
                rescale_type: &rescale_type,
                irradiation_event_uid: irradiation,
                concatenation: concatenation_expectation,
            });
            legacy_validated_report(validate_part10_file(path, &expected))
        }
        AdvancedCompatibilityProvider::Mr {
            common,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness: _,
            spacing_between_slices: _,
            rescale_intercept,
            rescale_slope,
            rescale_type,
            repetition_time,
            flip_angle,
            echo_train_length,
            rf_echo_train_length,
            gradient_echo_train_length,
            axis,
        } => {
            let (echoes, temporal, directions, minimum, maximum) = match &axis {
                EnhancedMrFrameAxis::EffectiveEchoTime { values } => {
                    (Some(values.as_slice()), None, None, None, None)
                }
                EnhancedMrFrameAxis::TemporalPositionTimeOffset { values } => {
                    (None, Some(values.as_slice()), None, None, None)
                }
                EnhancedMrFrameAxis::VelocityEncoding {
                    directions,
                    minimum,
                    maximum,
                } => (
                    None,
                    None,
                    Some(directions.as_slice()),
                    Some(*minimum),
                    Some(*maximum),
                ),
            };
            const OPERATING_MODES: &[(&str, &str)] = &[
                ("STATIC FIELD", "IEC_NORMAL"),
                ("RF", "IEC_NORMAL"),
                ("GRADIENT", "IEC_NORMAL"),
            ];
            expected.enhanced_mr_image = Some(EnhancedMrImageExpectations {
                modality: "MR",
                patient_position: "",
                frame_of_reference_uid: frame_of_reference,
                image_type: &common.frame_type,
                number_of_frames: frame_count,
                shared_functional_groups: 1,
                per_frame_functional_groups: frames.len(),
                dimension_organization_uid: dimension,
                dimension_index_count: 1,
                pixel_spacing: &pixel_spacing,
                image_orientation_patient: &image_orientation_patient,
                image_position_patient: &positions,
                frame_type: &common.frame_type,
                pixel_presentation: &common.pixel_presentation,
                volumetric_properties: &common.volumetric_properties,
                volume_based_calculation_technique: &common.volume_based_calculation_technique,
                content_qualification: "RESEARCH",
                applicable_safety_standard_agency: "IEC",
                complex_image_component: "MAGNITUDE",
                acquisition_contrast: "UNKNOWN",
                burned_in_annotation: "NO",
                lossy_image_compression: "00",
                presentation_lut_shape: "IDENTITY",
                anatomic_region_code_value: "69536005",
                anatomic_region_coding_scheme: "SCT",
                anatomic_region_code_meaning: "Head",
                rescale_intercept: &rescale_intercept,
                rescale_slope: &rescale_slope,
                rescale_type: &rescale_type,
                repetition_time: &repetition_time,
                flip_angle: &flip_angle,
                echo_train_length: &echo_train_length,
                rf_echo_train_length,
                gradient_echo_train_length,
                specific_absorption_rate_definition: "IEC_HEAD",
                specific_absorption_rate_value: 0.1,
                operating_modes: OPERATING_MODES,
                effective_echo_times: echoes,
                temporal_position_time_offsets: temporal,
                velocity_encoding_directions: directions,
                velocity_encoding_minimum_value: minimum,
                velocity_encoding_maximum_value: maximum,
            });
            legacy_validated_report(validate_part10_file(path, &expected))
        }
        AdvancedCompatibilityProvider::Pet {
            common,
            pixel_spacing,
            image_orientation_patient,
            slice_thickness: _,
            spacing_between_slices: _,
            rescale_intercept,
            rescale_slope,
            units: _,
            counts_source: _,
            stack_id,
        } => {
            expected.decoded_frame_hashes = &frame_hashes;
            let value_count = usize::from(common.rows)
                .checked_mul(usize::from(common.columns))
                .and_then(|value| value.checked_mul(frames.len()))
                .ok_or_else(|| {
                    ServiceInvocationError::new("advanced validation", "pixel count overflow")
                })?;
            let (values, _, _) = item
                .pixels
                .values(value_count)
                .map_err(|error| ServiceInvocationError::new("advanced validation", error))?;
            let stored = values
                .iter()
                .map(|value| {
                    u16::try_from(*value)
                        .map_err(|error| service_error("advanced validation", error))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let slope = rescale_slope
                .parse::<f64>()
                .map_err(|error| service_error("advanced validation", error))?;
            let activity = stored
                .iter()
                .map(|value| f64::from(*value) * slope)
                .collect::<Vec<_>>();
            expected.enhanced_pet_image = Some(EnhancedPetImageExpectations {
                modality: "PT",
                frame_of_reference_uid: frame_of_reference,
                image_type: &common.image_type,
                frame_type: &common.frame_type,
                number_of_frames: frame_count,
                dimension_organization_uid: dimension,
                pixel_spacing: &pixel_spacing,
                image_orientation_patient: &image_orientation_patient,
                image_position_patient: &positions,
                dimension_index_values: &dimension_values,
                temporal_position_indices: &item.temporal_position_indices,
                in_stack_position_numbers: &item.in_stack_position_numbers,
                stack_id: &stack_id,
                rescale_intercept: &rescale_intercept,
                rescale_slope: &rescale_slope,
                stored_values: &stored,
                activity_values_bqml: &activity,
            });
            legacy_validated_report(validate_part10_file(path, &expected))
        }
    }
}

fn validate_wsi_compatibility(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    content: &[MaterializedContentEvidence],
    contexts: &BTreeMap<String, CuratedArtifactProjectionContext>,
    planned_artifacts: &BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>,
    bindings: &BTreeMap<String, ArtifactExecutionBindings>,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let item = wsi_artifact_parameters(context)?;
    let sop = required_identity(artifact, CompositionUidRole::SopInstance)?;
    let implementation = required_identity(artifact, CompositionUidRole::ImplementationClass)?;
    let frame_of_reference = required_identity(artifact, CompositionUidRole::FrameOfReference)?;
    let dimension = required_identity(artifact, CompositionUidRole::DimensionOrganization)?;
    let specimen = required_identity(
        artifact,
        CompositionUidRole::TemplateDefined("specimen_uid".into()),
    )?;
    let frame_hashes = content
        .iter()
        .find(|item| item.slot == "pixels")
        .map(|item| {
            if item.decoded_frame_sha256.is_empty() {
                item.native_frame_sha256.as_slice()
            } else {
                item.decoded_frame_sha256.as_slice()
            }
        })
        .unwrap_or(&[]);
    let frame_hash_refs = frame_hashes.iter().map(String::as_str).collect::<Vec<_>>();
    let stress = matches!(
        item.pixel_algorithm,
        WsiPixelAlgorithm::ReducedStress { .. }
    );
    let expected = base_advanced_expectations(
        artifact,
        sop,
        implementation,
        item.parameters.frames,
        3,
        "RGB",
        8,
        8,
        7,
        0,
        Some(0),
        VR::OB,
        if stress { &frame_hash_refs } else { &[] },
    );
    let validated = match item.pixel_algorithm {
        WsiPixelAlgorithm::TiledColorQuadrants if !item.parameters.pyramid_membership => {
            validate_wsi_tiled_full_file(
                path,
                &expected,
                &crate::wsi_tiled_full_locked_contract(frame_of_reference, specimen),
            )
        }
        WsiPixelAlgorithm::SparseDiagonalTiles if !item.parameters.pyramid_membership => {
            validate_wsi_tiled_sparse_file(
                path,
                &expected,
                &crate::wsi_tiled_sparse_locked_contract(frame_of_reference, specimen, dimension),
            )
        }
        WsiPixelAlgorithm::MultipleOpticalPaths if !item.parameters.pyramid_membership => {
            validate_wsi_multiple_optical_paths_file(
                path,
                &expected,
                &crate::wsi_multiple_optical_paths_locked_contract(
                    frame_of_reference,
                    specimen,
                    dimension,
                ),
            )
        }
        WsiPixelAlgorithm::ReducedStress { .. } => validate_part10_file(path, &expected),
        WsiPixelAlgorithm::Thumbnail
        | WsiPixelAlgorithm::Label
        | WsiPixelAlgorithm::TiledColorQuadrants => {
            let Some((contract, role)) =
                wsi_pyramid_group_contract(context, contexts, planned_artifacts, bindings)?
            else {
                return Err(ServiceInvocationError::new(
                    "advanced validation",
                    "WSI pyramid member has no complete typed group contract",
                ));
            };
            validate_wsi_pyramid_file(path, &expected, &contract, role)
        }
        WsiPixelAlgorithm::SparseDiagonalTiles | WsiPixelAlgorithm::MultipleOpticalPaths => {
            return Err(ServiceInvocationError::new(
                "advanced validation",
                "non-pyramid WSI algorithm unexpectedly declares pyramid membership",
            ));
        }
    };
    legacy_validated_report(validated)
}

fn wsi_pyramid_group_contract(
    current: &CuratedArtifactProjectionContext,
    contexts: &BTreeMap<String, CuratedArtifactProjectionContext>,
    planned_artifacts: &BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>,
    bindings: &BTreeMap<String, ArtifactExecutionBindings>,
) -> Result<Option<(Value, WsiPyramidRole)>, ServiceInvocationError> {
    let same_case = contexts
        .iter()
        .filter(|(_, context)| {
            context.registry_case.case_id == current.registry_case.case_id
                && context.case_recipe.plan_provider_id == "native.wsi_plan"
        })
        .collect::<Vec<_>>();
    let find = |role: &str| {
        same_case
            .iter()
            .find(|(_, context)| context.artifact_recipe.output.role == role)
            .copied()
    };
    let (Some(volume), Some(thumbnail), Some(label)) =
        (find("volume"), find("thumbnail"), find("label"))
    else {
        return Ok(None);
    };
    if same_case.len() != 3 {
        return Err(ServiceInvocationError::new(
            "advanced validation",
            "WSI pyramid group contains unexpected extra artifacts",
        ));
    }
    let ordered = [
        (WsiPyramidRole::Volume, volume),
        (WsiPyramidRole::Thumbnail, thumbnail),
        (WsiPyramidRole::Label, label),
    ];
    let mut members = Vec::with_capacity(3);
    for (role, (logical_id, context)) in ordered {
        let planned = planned_artifacts.get(logical_id).ok_or_else(|| {
            ServiceInvocationError::new(
                "advanced validation",
                format!("WSI pyramid plan is missing {logical_id}"),
            )
        })?;
        let preview_plan = inline_preview_plan(
            planned,
            bindings.get(logical_id).ok_or_else(|| {
                ServiceInvocationError::new(
                    "advanced validation",
                    format!("WSI pyramid bindings are missing {logical_id}"),
                )
            })?,
        )?;
        let bytes = Part10Materializer
            .preview_part10_bytes_with_encoding(
                &preview_plan,
                &planned.encoding,
                planned.resources.output_bytes,
            )
            .map_err(|error| service_error("advanced validation", error))?;
        let path = context
            .artifact_recipe
            .output
            .path
            .as_deref()
            .ok_or_else(|| {
                ServiceInvocationError::new(
                    "advanced validation",
                    "WSI pyramid member has no output path",
                )
            })?;
        members.push((
            role,
            path.to_owned(),
            sha256_hex(&bytes),
            bytes.len() as u64,
            required_identity(planned, CompositionUidRole::SopInstance)?.to_owned(),
        ));
    }
    let root = planned_artifacts.get(volume.0).ok_or_else(|| {
        ServiceInvocationError::new("advanced validation", "WSI pyramid volume plan is missing")
    })?;
    let contract = wsi_pyramid_locked_contract(WsiPyramidLockedInputs {
        study_instance_uid: required_identity(root, CompositionUidRole::StudyInstance)?,
        series_instance_uid: required_identity(root, CompositionUidRole::SeriesInstance)?,
        frame_of_reference_uid: required_identity(root, CompositionUidRole::FrameOfReference)?,
        specimen_uid: required_identity(
            root,
            CompositionUidRole::TemplateDefined("specimen_uid".into()),
        )?,
        pyramid_uid: required_identity(
            root,
            CompositionUidRole::TemplateDefined("pyramid_uid".into()),
        )?,
        members: [
            WsiPyramidMemberIdentity {
                role: members[0].0,
                path: &members[0].1,
                sha256: &members[0].2,
                size_bytes: members[0].3,
                sop_instance_uid: &members[0].4,
            },
            WsiPyramidMemberIdentity {
                role: members[1].0,
                path: &members[1].1,
                sha256: &members[1].2,
                size_bytes: members[1].3,
                sop_instance_uid: &members[1].4,
            },
            WsiPyramidMemberIdentity {
                role: members[2].0,
                path: &members[2].1,
                sha256: &members[2].2,
                size_bytes: members[2].3,
                sop_instance_uid: &members[2].4,
            },
        ],
    });
    let selected = match current.artifact_recipe.output.role.as_str() {
        "volume" => WsiPyramidRole::Volume,
        "thumbnail" => WsiPyramidRole::Thumbnail,
        "label" => WsiPyramidRole::Label,
        role => {
            return Err(ServiceInvocationError::new(
                "advanced validation",
                format!("unknown WSI pyramid role {role}"),
            ));
        }
    };
    Ok(Some((contract, selected)))
}

fn inline_preview_plan(
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    bindings: &ArtifactExecutionBindings,
) -> Result<ResolvedInstancePlan, ServiceInvocationError> {
    let mut plan = artifact.instance.clone();
    for content in &mut plan.content {
        let Some(binding) = bindings.slots.get(&content.slot) else {
            continue;
        };
        let SlotExecutionBinding::NativeFrames { frames } = binding else {
            return Err(ServiceInvocationError::new(
                "advanced validation",
                format!("preview requires inline native frames for {}", content.slot),
            ));
        };
        let mut frames = frames.iter().collect::<Vec<_>>();
        frames.sort_by_key(|frame| frame.frame_number);
        let capacity = usize::try_from(content.size_bytes)
            .map_err(|error| service_error("advanced validation", error))?;
        let mut bytes = Vec::with_capacity(capacity);
        for (index, frame) in frames.into_iter().enumerate() {
            if frame.frame_number != index as u32 + 1 {
                return Err(ServiceInvocationError::new(
                    "advanced validation",
                    "preview native frame order is not contiguous",
                ));
            }
            let ByteBinding::Inline {
                bytes: frame_bytes,
                sha256,
            } = &frame.bytes
            else {
                return Err(ServiceInvocationError::new(
                    "advanced validation",
                    "preview native frame is not inline",
                ));
            };
            if sha256_hex(frame_bytes) != *sha256 {
                return Err(ServiceInvocationError::new(
                    "advanced validation",
                    "preview native frame hash differs from its binding",
                ));
            }
            bytes.extend_from_slice(frame_bytes);
        }
        if bytes.len() as u64 != content.size_bytes || sha256_hex(&bytes) != content.sha256 {
            return Err(ServiceInvocationError::new(
                "advanced validation",
                "preview native content differs from its planned identity",
            ));
        }
        content.materialization = Some(crate::composition::ContentMaterialization::Inline(bytes));
    }
    Ok(plan)
}

#[allow(clippy::too_many_arguments)]
fn base_advanced_expectations<'a>(
    artifact: &'a crate::corpus_plan::PlannedDicomArtifact,
    sop_instance_uid: &'a str,
    implementation_class_uid: &'a str,
    frames: u16,
    samples_per_pixel: u16,
    photometric_interpretation: &'a str,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_representation: u16,
    planar_configuration: Option<u16>,
    pixel_data_vr: VR,
    decoded_frame_hashes: &'a [&'a str],
) -> Part10Expectations<'a> {
    Part10Expectations {
        sop_class_uid: &artifact.instance.sop_class_uid,
        sop_instance_uid,
        transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
        implementation_class_uid,
        synthetic_data: "YES",
        rows: planned_u16(&artifact.instance, "0028,0010").unwrap_or(0),
        columns: planned_u16(&artifact.instance, "0028,0011").unwrap_or(0),
        frames,
        samples_per_pixel,
        photometric_interpretation,
        bits_allocated,
        bits_stored,
        high_bit,
        pixel_representation,
        planar_configuration,
        pixel_data_vr,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        decoded_frame_hashes,
        palette: None,
        padding: None,
        ct_image: None,
        enhanced_ct_image: None,
        enhanced_mr_image: None,
        enhanced_pet_image: None,
        mg_image: None,
        dx_image: None,
        xa_image: None,
        xrf_image: None,
        us_image: None,
        us_multiframe: None,
        nm_image: None,
        pet_image: None,
        cr_image: None,
        mr_image: None,
        segmentation: None,
    }
}

fn legacy_validated_report(
    result: Result<crate::validation::ValidatedPart10, crate::GenerateError>,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let validated = result.map_err(|error| service_error("advanced validation", error))?;
    let mut checks = Vec::new();
    for (field, layer) in [
        ("internal", crate::curated_validation::CheckLayer::Internal),
        (
            "standards",
            crate::curated_validation::CheckLayer::Standards,
        ),
        ("external", crate::curated_validation::CheckLayer::External),
    ] {
        let rows = validated.validation[field].as_array().ok_or_else(|| {
            ServiceInvocationError::new("advanced validation", "invalid validation row array")
        })?;
        for row in rows {
            checks.push(TypedValidationCheck {
                layer,
                name: legacy_string(row, "name")?,
                status: legacy_string(row, "status")?,
                message: legacy_string(row, "message")?,
            });
        }
    }
    Ok(TypedValidationReport {
        bytes: validated.bytes,
        checks,
        metadata_observation: None,
    })
}

fn legacy_string(value: &Value, field: &str) -> Result<String, ServiceInvocationError> {
    value[field].as_str().map(str::to_owned).ok_or_else(|| {
        ServiceInvocationError::new(
            "advanced validation",
            format!("validation row is missing {field}"),
        )
    })
}

fn required_identity(
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    role: CompositionUidRole,
) -> Result<&str, ServiceInvocationError> {
    artifact.instance.identities.get(&role, 0).ok_or_else(|| {
        ServiceInvocationError::new(
            "advanced validation",
            format!("missing {} identity", role.as_str()),
        )
    })
}

fn planned_text(plan: &ResolvedInstancePlan, tag: &str) -> Option<String> {
    plan.attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == tag)
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| match value {
            crate::composition::AttributeValue::Primitive(
                crate::composition::PrimitiveValue::String(value),
            ) => Some(value.clone()),
            _ => None,
        })
}

fn planned_u16(plan: &ResolvedInstancePlan, tag: &str) -> Option<u16> {
    plan.attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == tag)
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| match value {
            crate::composition::AttributeValue::Primitive(
                crate::composition::PrimitiveValue::Unsigned(value),
            ) => u16::try_from(*value).ok(),
            crate::composition::AttributeValue::Primitive(
                crate::composition::PrimitiveValue::String(value),
            ) => value.parse().ok(),
            _ => None,
        })
}

fn qualify_declared_classic_vr_exceptions(
    context: &CuratedArtifactProjectionContext,
    checks: &mut [ValidationCheck],
) -> Result<(), ServiceInvocationError> {
    if context.artifact_recipe.algorithm_provider_id.as_deref() != Some("algorithm.classic_dx_mg") {
        return Ok(());
    }
    let parameters: DxMgArtifactParameters =
        serde_json::from_value(Value::Object(context.artifact_recipe.parameters.clone()))
            .map_err(|error| service_error("validation", error))?;
    if parameters.field_of_view_dimensions_vr != "DS" {
        return Err(ServiceInvocationError::new(
            "validation",
            "DX/MG historical Field of View Dimensions VR is not declared as DS",
        ));
    }
    for check in checks {
        if check.status != "passed"
            && check.rule_id == "resolved_attributes"
            && check
                .message
                .contains("0018,1149 reopened as IS, expected DS")
        {
            check.status = "passed".into();
            check.message = "0018,1149 uses the explicitly declared historical DS compatibility contract legacy.dx_mg.field_of_view_dimensions.ds.".into();
        }
    }
    Ok(())
}

struct RejectAuxiliaryMaterialization;

struct ClassicPixelValidation<'a> {
    rows: u16,
    columns: u16,
    frames: u16,
    samples_per_pixel: u16,
    photometric_interpretation: &'a str,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_representation: u16,
    planar_configuration: Option<u16>,
    decoded_frame_hashes: &'a [&'a str],
    palette: Option<PaletteExpectations>,
}

fn validate_classic_base<'a>(
    path: &Path,
    artifact: &'a crate::corpus_plan::PlannedDicomArtifact,
    plan: &'a ResolvedInstancePlan,
    content: &'a MaterializedContentEvidence,
    pixels: ClassicPixelValidation<'a>,
    configure: impl FnOnce(&mut Part10Expectations<'a>),
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let sop_instance_uid = plan
        .identities
        .get(&CompositionUidRole::SopInstance, 0)
        .ok_or_else(|| ServiceInvocationError::new("validation", "missing SOP Instance UID"))?;
    let pixel_data_vr = match content.vr.as_str() {
        "OB" => dicom_core::VR::OB,
        "OW" => dicom_core::VR::OW,
        other => {
            return Err(ServiceInvocationError::new(
                "validation",
                format!("unsupported classic pixel VR {other}"),
            ));
        }
    };
    let pixel_data_length_formula = if artifact.encoding.transfer_syntax_uid
        == crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
    {
        PixelDataLengthFormula::Encapsulated {
            fragments: usize::try_from(content.fragment_count)
                .map_err(|error| service_error("validation", error))?,
            basic_offset_table_offsets: content.basic_offset_table.len(),
        }
    } else {
        PixelDataLengthFormula::ContiguousSamples
    };
    let mut expected = Part10Expectations {
        sop_class_uid: &plan.sop_class_uid,
        sop_instance_uid,
        transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
        implementation_class_uid: &artifact.encoding.implementation.class_uid,
        synthetic_data: "YES",
        rows: pixels.rows,
        columns: pixels.columns,
        frames: pixels.frames,
        samples_per_pixel: pixels.samples_per_pixel,
        photometric_interpretation: pixels.photometric_interpretation,
        bits_allocated: pixels.bits_allocated,
        bits_stored: pixels.bits_stored,
        high_bit: pixels.high_bit,
        pixel_representation: pixels.pixel_representation,
        planar_configuration: pixels.planar_configuration,
        pixel_data_vr,
        pixel_data_length_formula,
        decoded_frame_hashes: pixels.decoded_frame_hashes,
        palette: pixels.palette,
        padding: None,
        ct_image: None,
        enhanced_ct_image: None,
        enhanced_mr_image: None,
        enhanced_pet_image: None,
        mg_image: None,
        dx_image: None,
        xa_image: None,
        xrf_image: None,
        us_image: None,
        us_multiframe: None,
        nm_image: None,
        pet_image: None,
        cr_image: None,
        mr_image: None,
        segmentation: None,
    };
    configure(&mut expected);
    validate_part10_with_expectations(path, &expected)
        .map_err(|error| service_error("validation", error))
}

fn validate_classic_part10(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    plan: &ResolvedInstancePlan,
    content: &[MaterializedContentEvidence],
    codec_decoded: &[String],
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let pixel_content = content
        .iter()
        .find(|item| item.slot == CLASSIC_PIXEL_SLOT)
        .ok_or_else(|| {
            ServiceInvocationError::new("validation", "missing classic pixel evidence")
        })?;
    let encapsulated =
        artifact.encoding.transfer_syntax_uid == crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID;
    let decoded_source = if pixel_content.decoded_frame_sha256.is_empty() {
        codec_decoded
    } else {
        &pixel_content.decoded_frame_sha256
    };
    let decoded_hashes = decoded_source
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let parameters = Value::Object(context.artifact_recipe.parameters.clone());
    match context.artifact_recipe.algorithm_provider_id.as_deref() {
        Some("algorithm.classic_ct") => {
            let provider: ClassicCtProviderParameters = serde_json::from_value(Value::Object(
                context.case_recipe.provider_parameters.clone(),
            ))
            .map_err(|error| service_error("validation", error))?;
            let item: ClassicCtArtifactParameters = serde_json::from_value(parameters)
                .map_err(|error| service_error("validation", error))?;
            let frame_of_reference_uid = plan
                .identities
                .get(&CompositionUidRole::FrameOfReference, 0)
                .ok_or_else(|| {
                    ServiceInvocationError::new("validation", "CT lacks Frame of Reference UID")
                })?;
            let image_type = provider.image_type.join("\\");
            let pixel_spacing = provider.pixel_spacing.join("\\");
            let orientation = provider.image_orientation_patient.join("\\");
            let position = item.image_position_patient.join("\\");
            validate_classic_base(
                path,
                artifact,
                plan,
                pixel_content,
                ClassicPixelValidation {
                    rows: item.pixels.rows,
                    columns: item.pixels.columns,
                    frames: 1,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 16,
                    bits_stored: 12,
                    high_bit: 11,
                    pixel_representation: 1,
                    planar_configuration: None,
                    decoded_frame_hashes: if encapsulated { &decoded_hashes } else { &[] },
                    palette: None,
                },
                |expected| {
                    expected.ct_image = Some(CtImageExpectations {
                        modality: "CT",
                        frame_of_reference_uid,
                        image_type: &image_type,
                        pixel_spacing: &pixel_spacing,
                        image_orientation_patient: &orientation,
                        image_position_patient: &position,
                        slice_thickness: &provider.slice_thickness,
                        kvp: &provider.kvp,
                        acquisition_number: &item.acquisition_number,
                        rescale_intercept: &provider.rescale_intercept,
                        rescale_slope: &provider.rescale_slope,
                        rescale_type: &provider.rescale_type,
                        window_center: &provider.window_center,
                        window_width: &provider.window_width,
                    });
                },
            )
        }
        Some("algorithm.classic_mr_cr")
            if context
                .case_recipe
                .binding
                .case_id
                .starts_with("classic/mr/") =>
        {
            let item: MrArtifactParameters = serde_json::from_value(parameters)
                .map_err(|error| service_error("validation", error))?;
            let frame_of_reference_uid = plan
                .identities
                .get(&CompositionUidRole::FrameOfReference, 0)
                .ok_or_else(|| {
                    ServiceInvocationError::new("validation", "MR lacks Frame of Reference UID")
                })?;
            let pixel_spacing = item.pixel_spacing.join("\\");
            let orientation = item.image_orientation_patient.join("\\");
            let position = item.image_position_patient.join("\\");
            let slice_count = context
                .case_recipe
                .dicom
                .as_ref()
                .map_or(0, |dicom| dicom.artifacts.len());
            let slice_order_index = usize::try_from(context.artifact_recipe.order)
                .map_err(|error| service_error("validation", error))?
                + 1;
            validate_classic_base(
                path,
                artifact,
                plan,
                pixel_content,
                ClassicPixelValidation {
                    rows: u16::try_from(item.rows)
                        .map_err(|error| service_error("validation", error))?,
                    columns: u16::try_from(item.columns)
                        .map_err(|error| service_error("validation", error))?,
                    frames: 1,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 16,
                    bits_stored: 16,
                    high_bit: 15,
                    pixel_representation: 0,
                    planar_configuration: None,
                    decoded_frame_hashes: if encapsulated { &decoded_hashes } else { &[] },
                    palette: None,
                },
                |expected| {
                    expected.mr_image = Some(MrImageExpectations {
                        modality: "MR",
                        frame_of_reference_uid,
                        image_type: "ORIGINAL\\PRIMARY",
                        instance_number: &item.instance_number,
                        acquisition_number: "1",
                        pixel_spacing: &pixel_spacing,
                        image_orientation_patient: &orientation,
                        image_position_patient: &position,
                        slice_thickness: &item.slice_thickness,
                        spacing_between_slices: &item.spacing_between_slices,
                        slice_location: &item.slice_location,
                        scanning_sequence: "SE",
                        sequence_variant: "NONE",
                        scan_options: "",
                        mr_acquisition_type: "2D",
                        repetition_time: "500",
                        echo_time: "20",
                        echo_train_length: "1",
                        magnetic_field_strength: "1.5",
                        slice_order_index,
                        slice_count,
                        position_along_normal: item.position_along_normal,
                    });
                },
            )
        }
        Some("algorithm.classic_mr_cr") => {
            let item: CrArtifactParameters = serde_json::from_value(parameters)
                .map_err(|error| service_error("validation", error))?;
            validate_classic_base(
                path,
                artifact,
                plan,
                pixel_content,
                ClassicPixelValidation {
                    rows: u16::try_from(item.rows)
                        .map_err(|error| service_error("validation", error))?,
                    columns: u16::try_from(item.columns)
                        .map_err(|error| service_error("validation", error))?,
                    frames: 1,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 8,
                    bits_stored: 8,
                    high_bit: 7,
                    pixel_representation: 0,
                    planar_configuration: None,
                    decoded_frame_hashes: if encapsulated { &decoded_hashes } else { &[] },
                    palette: None,
                },
                |expected| {
                    expected.cr_image = Some(CrImageExpectations {
                        modality: "CR",
                        image_type: "ORIGINAL\\PRIMARY",
                        body_part_examined: &item.body_part_examined,
                        view_position: &item.view_position,
                        acquisition_number: "1",
                        overlay_rows: item.overlay.rows,
                        overlay_columns: item.overlay.columns,
                        overlay_type: &item.overlay.overlay_type,
                        overlay_origin: item.overlay.origin.to_vec(),
                        overlay_bits_allocated: item.overlay.bits_allocated,
                        overlay_bit_position: item.overlay.bit_position,
                        overlay_data_length: item.overlay.data.len(),
                        modality_lut_descriptor: item.modality_lut.descriptor,
                        modality_lut_type: item.modality_lut.lut_type.as_deref().unwrap_or("US"),
                        modality_lut_data_length: item.modality_lut.data.len(),
                        voi_lut_descriptor: item.voi_lut.descriptor,
                        voi_lut_data_length: item.voi_lut.data.len(),
                    });
                },
            )
        }
        Some("algorithm.classic_dx_mg") => validate_dx_mg_classic(
            path,
            artifact,
            plan,
            pixel_content,
            parameters,
            &decoded_hashes,
            encapsulated,
        ),
        Some("algorithm.classic_nuclear") => validate_nuclear_classic(
            path,
            artifact,
            context,
            plan,
            pixel_content,
            parameters,
            &decoded_hashes,
            encapsulated,
        ),
        Some("algorithm.classic_vl_projection") => validate_vl_projection_classic(
            path,
            artifact,
            context,
            plan,
            pixel_content,
            parameters,
            &decoded_hashes,
            encapsulated,
        ),
        other => Err(ServiceInvocationError::new(
            "validation",
            format!("unsupported classic validation provider {other:?}"),
        )),
    }
}

fn validate_dx_mg_classic(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    plan: &ResolvedInstancePlan,
    content: &MaterializedContentEvidence,
    parameters: Value,
    decoded_hashes: &[&str],
    encapsulated: bool,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let item: DxMgArtifactParameters =
        serde_json::from_value(parameters).map_err(|error| service_error("validation", error))?;
    let photometric = match item.photometric_interpretation {
        crate::native_pixel::PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        crate::native_pixel::PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        _ => {
            return Err(ServiceInvocationError::new(
                "validation",
                "DX/MG requires monochrome pixels",
            ));
        }
    };
    let imager_spacing = item.imager_pixel_spacing.join("\\");
    validate_classic_base(
        path,
        artifact,
        plan,
        content,
        ClassicPixelValidation {
            rows: u16::try_from(item.rows).map_err(|error| service_error("validation", error))?,
            columns: u16::try_from(item.columns)
                .map_err(|error| service_error("validation", error))?,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: photometric,
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 0,
            planar_configuration: None,
            decoded_frame_hashes: if encapsulated { decoded_hashes } else { &[] },
            palette: None,
        },
        |expected| match item.family {
            DxMgFamily::Dx => {
                let shutter = item.shutter.as_ref().expect("validated DX shutter");
                expected.dx_image = Some(DxImageExpectations {
                    modality: &item.modality,
                    presentation_intent_type: &item.presentation_intent_type,
                    image_type: "ORIGINAL\\PRIMARY",
                    image_laterality: &item.image_laterality,
                    body_part_examined: &item.body_part_examined,
                    imager_pixel_spacing: &imager_spacing,
                    detector_type: "DIRECT",
                    detector_configuration: "AREA",
                    detector_id: &item.detector_id,
                    pixel_intensity_relationship: "LIN",
                    pixel_intensity_relationship_sign: -1,
                    rescale_intercept: "0",
                    rescale_slope: "1",
                    rescale_type: "US",
                    presentation_lut_shape: &item.presentation_lut_shape,
                    lossy_image_compression: "00",
                    burned_in_annotation: "NO",
                    window_center: item.window_center.as_deref().expect("validated DX window"),
                    window_width: item.window_width.as_deref().expect("validated DX window"),
                    anatomic_region_code_value: &item.anatomic_region.value,
                    acquisition_context_items: 0,
                    shutter_shape: &shutter.shape,
                    shutter_left_vertical_edge: &shutter.left_vertical_edge,
                    shutter_right_vertical_edge: &shutter.right_vertical_edge,
                    shutter_upper_horizontal_edge: &shutter.upper_horizontal_edge,
                    shutter_lower_horizontal_edge: &shutter.lower_horizontal_edge,
                    shutter_presentation_value: shutter.presentation_value,
                });
            }
            DxMgFamily::Mammography => {
                expected.mg_image = Some(MgImageExpectations {
                    modality: &item.modality,
                    presentation_intent_type: &item.presentation_intent_type,
                    image_type: "ORIGINAL\\PRIMARY",
                    image_laterality: &item.image_laterality,
                    view_position: item.view_position.as_deref().expect("validated MG view"),
                    body_part_examined: &item.body_part_examined,
                    organ_exposed: item.organ_exposed.as_deref().expect("validated MG organ"),
                    positioner_type: item
                        .positioner_type
                        .as_deref()
                        .expect("validated MG positioner"),
                    imager_pixel_spacing: &imager_spacing,
                    detector_type: "DIRECT",
                    detector_configuration: "AREA",
                    detector_id: &item.detector_id,
                    pixel_intensity_relationship: "LIN",
                    pixel_intensity_relationship_sign: -1,
                    rescale_intercept: "0",
                    rescale_slope: "1",
                    rescale_type: "US",
                    presentation_lut_shape: &item.presentation_lut_shape,
                    lossy_image_compression: "00",
                    burned_in_annotation: "NO",
                    breast_implant_present: item
                        .breast_implant_present
                        .as_deref()
                        .expect("validated MG implant"),
                    window_center: item.window_center.as_deref(),
                    window_width: item.window_width.as_deref(),
                    anatomic_region_code_value: &item.anatomic_region.value,
                    view_code_value: &item
                        .view_code
                        .as_ref()
                        .expect("validated MG view code")
                        .value,
                    acquisition_context_items: 0,
                });
            }
        },
    )
}

fn validate_nuclear_classic(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    plan: &ResolvedInstancePlan,
    content: &MaterializedContentEvidence,
    parameters: Value,
    codec_decoded_hashes: &[&str],
    encapsulated: bool,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    let provider: ClassicNuclearProviderParameters = serde_json::from_value(Value::Object(
        context.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| service_error("validation", error))?;
    let item: ClassicNuclearArtifactParameters =
        serde_json::from_value(parameters).map_err(|error| service_error("validation", error))?;
    match item {
        ClassicNuclearArtifactParameters::UltrasoundSingleFrame {
            pixels,
            image_type,
            lossy_image_compression,
            ultrasound_color_data_present,
        } => {
            let image_type = image_type.join("\\");
            validate_classic_base(
                path,
                artifact,
                plan,
                content,
                ClassicPixelValidation {
                    rows: pixels.rows,
                    columns: pixels.columns,
                    frames: u16::try_from(pixels.frames)
                        .map_err(|error| service_error("validation", error))?,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 8,
                    bits_stored: 8,
                    high_bit: 7,
                    pixel_representation: 0,
                    planar_configuration: None,
                    decoded_frame_hashes: if encapsulated {
                        codec_decoded_hashes
                    } else {
                        &[]
                    },
                    palette: None,
                },
                |expected| {
                    expected.us_image = Some(UsImageExpectations {
                        modality: &provider.modality,
                        image_type: &image_type,
                        lossy_image_compression: &lossy_image_compression,
                        ultrasound_color_data_present,
                    });
                },
            )
        }
        ClassicNuclearArtifactParameters::UltrasoundMultiframe {
            pixels,
            image_type,
            frame_increment_pointer: _,
            frame_time_ms,
            frame_relative_times_ms: _,
            payload_sha256: _,
            lossy_image_compression,
            color_data_present,
            spatially_related_frames: _,
            region_calibrated: _,
        } => {
            let image_type = image_type.join("\\");
            let frame_time = frame_time_ms.to_string();
            let frame_hashes = pixels
                .frame_sha256
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            validate_classic_base(
                path,
                artifact,
                plan,
                content,
                ClassicPixelValidation {
                    rows: pixels.rows,
                    columns: pixels.columns,
                    frames: u16::try_from(pixels.frames)
                        .map_err(|error| service_error("validation", error))?,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 8,
                    bits_stored: 8,
                    high_bit: 7,
                    pixel_representation: 0,
                    planar_configuration: None,
                    decoded_frame_hashes: &frame_hashes,
                    palette: None,
                },
                |expected| {
                    expected.us_multiframe = Some(UsMultiframeExpectations {
                        modality: &provider.modality,
                        body_part_examined: provider.body_part_examined.as_deref().unwrap_or(""),
                        image_type: &image_type,
                        lossy_image_compression: &lossy_image_compression,
                        ultrasound_color_data_present: u16::from(color_data_present),
                        number_of_frames: u16::try_from(pixels.frames).expect("validated frames"),
                        frame_increment_pointer: dicom_dictionary_std::tags::FRAME_TIME,
                        frame_time_ms: &frame_time,
                    });
                },
            )
        }
        ClassicNuclearArtifactParameters::NuclearMedicine {
            pixels,
            image_type,
            pixel_spacing,
            energy_window_vector,
            detector_vector,
            energy_windows,
            detectors,
            actual_frame_duration_ms,
            counts_accumulated,
        } => {
            let image_type = image_type.join("\\");
            let pixel_spacing = pixel_spacing.join("\\");
            let duration = actual_frame_duration_ms.to_string();
            let counts = counts_accumulated.to_string();
            let frame_hashes = pixels
                .frame_sha256
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let energy = energy_windows
                .iter()
                .map(|window| NmEnergyWindowExpectations {
                    name: &window.name,
                    lower_limit_kev: &window.lower_limit_kev,
                    upper_limit_kev: &window.upper_limit_kev,
                })
                .collect::<Vec<_>>();
            let detector_orientation = detectors
                .iter()
                .map(|detector| detector.image_orientation_patient.join("\\"))
                .collect::<Vec<_>>();
            let detector_position = detectors
                .iter()
                .map(|detector| detector.image_position_patient.join("\\"))
                .collect::<Vec<_>>();
            let detector_expectations = detectors
                .iter()
                .enumerate()
                .map(|(index, detector)| NmDetectorExpectations {
                    collimator_type: &detector.collimator_type,
                    focal_distance_mm: &detector.focal_distance_mm,
                    start_angle_degrees: &detector.start_angle_degrees,
                    image_orientation_patient: &detector_orientation[index],
                    image_position_patient: &detector_position[index],
                })
                .collect::<Vec<_>>();
            validate_classic_base(
                path,
                artifact,
                plan,
                content,
                ClassicPixelValidation {
                    rows: pixels.rows,
                    columns: pixels.columns,
                    frames: u16::try_from(pixels.frames)
                        .map_err(|error| service_error("validation", error))?,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 16,
                    bits_stored: 16,
                    high_bit: 15,
                    pixel_representation: 0,
                    planar_configuration: None,
                    decoded_frame_hashes: &frame_hashes,
                    palette: None,
                },
                |expected| {
                    expected.nm_image = Some(NmImageExpectations {
                        modality: &provider.modality,
                        body_part_examined: provider.body_part_examined.as_deref().unwrap_or(""),
                        image_type: &image_type,
                        pixel_spacing: &pixel_spacing,
                        actual_frame_duration_ms: &duration,
                        counts_accumulated: &counts,
                        frame_increment_pointers: &[
                            dicom_dictionary_std::tags::ENERGY_WINDOW_VECTOR,
                            dicom_dictionary_std::tags::DETECTOR_VECTOR,
                        ],
                        energy_window_vector: &energy_window_vector,
                        detector_vector: &detector_vector,
                        energy_windows: &energy,
                        detectors: &detector_expectations,
                    });
                },
            )
        }
        ClassicNuclearArtifactParameters::Pet {
            pixels,
            image_type,
            units,
            counts_source,
            series_type,
            number_of_slices,
            corrected_image,
            decay_correction,
            dose_calibration_factor,
            frame_reference_time_ms,
            actual_frame_duration_ms,
            image_index,
            pixel_spacing,
            image_orientation_patient,
            image_position_patient,
            slice_thickness,
            rescale_intercept,
            rescale_slope,
            expected_activity_bqml,
        } => {
            let frame_of_reference_uid = plan
                .identities
                .get(&CompositionUidRole::FrameOfReference, 0)
                .ok_or_else(|| {
                    ServiceInvocationError::new("validation", "PET lacks Frame of Reference UID")
                })?;
            let image_type = image_type.join("\\");
            let series_type = series_type.join("\\");
            let corrected_image = corrected_image.join("\\");
            let pixel_spacing = pixel_spacing.join("\\");
            let orientation = image_orientation_patient.join("\\");
            let position = image_position_patient.join("\\");
            let stored_values = pixels
                .stored_values
                .iter()
                .map(|value| {
                    u16::try_from(*value).map_err(|error| service_error("validation", error))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let activity = expected_activity_bqml
                .iter()
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|error| service_error("validation", error))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let frame_hashes = pixels
                .frame_sha256
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            validate_classic_base(
                path,
                artifact,
                plan,
                content,
                ClassicPixelValidation {
                    rows: pixels.rows,
                    columns: pixels.columns,
                    frames: u16::try_from(pixels.frames)
                        .map_err(|error| service_error("validation", error))?,
                    samples_per_pixel: 1,
                    photometric_interpretation: "MONOCHROME2",
                    bits_allocated: 16,
                    bits_stored: 16,
                    high_bit: 15,
                    pixel_representation: 0,
                    planar_configuration: None,
                    decoded_frame_hashes: &frame_hashes,
                    palette: None,
                },
                |expected| {
                    expected.pet_image = Some(PetImageExpectations {
                        modality: &provider.modality,
                        body_part_examined: provider.body_part_examined.as_deref().unwrap_or(""),
                        image_type: &image_type,
                        series_date: provider.series_date.as_deref().unwrap_or(""),
                        series_time: provider.series_time.as_deref().unwrap_or(""),
                        units: &units,
                        counts_source: &counts_source,
                        series_type: &series_type,
                        frame_of_reference_uid,
                        position_reference_indicator: "",
                        number_of_slices,
                        corrected_image: &corrected_image,
                        decay_correction: &decay_correction,
                        collimator_type: "NONE",
                        rescale_intercept: &rescale_intercept,
                        rescale_slope: &rescale_slope,
                        stored_values: &stored_values,
                        activity_values_bqml: &activity,
                        dose_calibration_factor: &dose_calibration_factor,
                        frame_reference_time_ms: &frame_reference_time_ms,
                        acquisition_date: &provider.acquisition_date,
                        acquisition_time: &provider.acquisition_time,
                        actual_frame_duration_ms: &actual_frame_duration_ms,
                        image_index,
                        pixel_spacing: &pixel_spacing,
                        image_orientation_patient: &orientation,
                        image_position_patient: &position,
                        slice_thickness: &slice_thickness,
                        radiopharmaceutical_information_items: 0,
                        patient_orientation_code_items: 0,
                        patient_gantry_relationship_code_items: 0,
                    });
                },
            )
        }
    }
}

fn validate_vl_projection_classic(
    path: &Path,
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    context: &CuratedArtifactProjectionContext,
    plan: &ResolvedInstancePlan,
    content: &MaterializedContentEvidence,
    parameters: Value,
    codec_decoded_hashes: &[&str],
    encapsulated: bool,
) -> Result<TypedValidationReport, ServiceInvocationError> {
    if context.case_recipe.binding.case_id.starts_with("vl/") {
        let item: VlArtifactParameters = serde_json::from_value(parameters)
            .map_err(|error| service_error("validation", error))?;
        let photometric = match item.photometric_interpretation {
            VlPhotometricInterpretation::Rgb => "RGB",
            VlPhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        };
        let declared_hash = [item.frame_sha256.as_str()];
        let decoded = if encapsulated {
            codec_decoded_hashes
        } else {
            declared_hash.as_slice()
        };
        let palette = item
            .palette
            .as_ref()
            .map(
                |palette| -> Result<PaletteExpectations, ServiceInvocationError> {
                    Ok(PaletteExpectations {
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
                },
            )
            .transpose()?;
        let mut typed = validate_classic_base(
            path,
            artifact,
            plan,
            content,
            ClassicPixelValidation {
                rows: u16::try_from(item.rows)
                    .map_err(|error| service_error("validation", error))?,
                columns: u16::try_from(item.columns)
                    .map_err(|error| service_error("validation", error))?,
                frames: 1,
                samples_per_pixel: item.samples_per_pixel,
                photometric_interpretation: photometric,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                pixel_representation: 0,
                planar_configuration: item.planar_configuration.map(u16::from),
                decoded_frame_hashes: decoded,
                palette,
            },
            |_| {},
        )?;
        if let Some(expected_sha256) = item.icc_profile_sha256.as_deref() {
            let profile_hex = item.icc_profile_hex.as_deref().ok_or_else(|| {
                ServiceInvocationError::new("validation", "ICC hash lacks typed profile bytes")
            })?;
            let color_space = item.color_space.as_deref().ok_or_else(|| {
                ServiceInvocationError::new("validation", "ICC hash lacks typed DICOM Color Space")
            })?;
            typed.append(
                validate_icc_profile_round_trip(
                    path,
                    expected_sha256,
                    profile_hex.len() / 2,
                    color_space,
                )
                .map_err(|error| service_error("validation", error))?,
            );
        }
        Ok(typed)
    } else {
        let item: ProjectionArtifactParameters = serde_json::from_value(parameters)
            .map_err(|error| service_error("validation", error))?;
        let image_type = item.image_type.join("\\");
        let spacing = item.imager_pixel_spacing.join("\\");
        let declared_hash = [item.frame_sha256.as_str()];
        validate_classic_base(
            path,
            artifact,
            plan,
            content,
            ClassicPixelValidation {
                rows: u16::try_from(item.rows)
                    .map_err(|error| service_error("validation", error))?,
                columns: u16::try_from(item.columns)
                    .map_err(|error| service_error("validation", error))?,
                frames: 1,
                samples_per_pixel: 1,
                photometric_interpretation: "MONOCHROME2",
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                pixel_representation: 0,
                planar_configuration: None,
                decoded_frame_hashes: if encapsulated {
                    codec_decoded_hashes
                } else {
                    declared_hash.as_slice()
                },
                palette: None,
            },
            |expected| match item.modality.as_str() {
                "XA" => {
                    expected.xa_image = Some(XaImageExpectations {
                        modality: &item.modality,
                        body_part_examined: &item.body_part_examined,
                        image_type: &image_type,
                        patient_orientation: "",
                        pixel_intensity_relationship: &item.pixel_intensity_relationship,
                        lossy_image_compression: &item.lossy_image_compression,
                        radiation_setting: &item.radiation_setting,
                        kvp: &item.kvp,
                        exposure_mas: &item.exposure,
                        imager_pixel_spacing_mm: &spacing,
                        positioner_primary_angle_degrees: item
                            .positioner_primary_angle
                            .as_deref()
                            .expect("validated XA primary angle"),
                        positioner_secondary_angle_degrees: item
                            .positioner_secondary_angle
                            .as_deref()
                            .expect("validated XA secondary angle"),
                        distance_source_to_detector_mm: &item.distance_source_to_detector,
                        distance_source_to_patient_mm: &item.distance_source_to_patient,
                        estimated_radiographic_magnification_factor: &item
                            .estimated_magnification_factor,
                    });
                }
                "RF" => {
                    expected.xrf_image = Some(XrfImageExpectations {
                        modality: &item.modality,
                        body_part_examined: &item.body_part_examined,
                        image_type: &image_type,
                        patient_orientation: "",
                        pixel_intensity_relationship: &item.pixel_intensity_relationship,
                        lossy_image_compression: &item.lossy_image_compression,
                        radiation_setting: &item.radiation_setting,
                        kvp: &item.kvp,
                        exposure_mas: &item.exposure,
                        imager_pixel_spacing_mm: &spacing,
                        distance_source_to_detector_mm: &item.distance_source_to_detector,
                        distance_source_to_patient_mm: &item.distance_source_to_patient,
                        estimated_radiographic_magnification_factor: &item
                            .estimated_magnification_factor,
                        column_angulation_degrees: item
                            .column_angulation
                            .as_deref()
                            .expect("validated XRF column angulation"),
                    });
                }
                _ => unreachable!("classic projection recipe validates modality"),
            },
        )
    }
}

fn validate_classic_ct_group(
    current: &CuratedArtifactProjectionContext,
    contexts: &BTreeMap<String, CuratedArtifactProjectionContext>,
    planned: &BTreeMap<String, crate::corpus_plan::PlannedDicomArtifact>,
) -> Result<TypedValidationCheck, ServiceInvocationError> {
    let case_id = &current.case_recipe.binding.case_id;
    let provider: ClassicCtProviderParameters = serde_json::from_value(Value::Object(
        current.case_recipe.provider_parameters.clone(),
    ))
    .map_err(|error| service_error("validation", error))?;
    let mut members = contexts
        .values()
        .filter(|context| {
            context.case_recipe.binding.case_id == *case_id
                && context.artifact_recipe.algorithm_provider_id.as_deref()
                    == Some("algorithm.classic_ct")
        })
        .map(|context| {
            let parameters: ClassicCtArtifactParameters =
                serde_json::from_value(Value::Object(context.artifact_recipe.parameters.clone()))
                    .map_err(|error| service_error("validation", error))?;
            let artifact = planned.get(&context.artifact_id).ok_or_else(|| {
                ServiceInvocationError::new(
                    "validation",
                    "CT group member lacks a planned artifact",
                )
            })?;
            Ok((context, parameters, artifact))
        })
        .collect::<Result<Vec<_>, ServiceInvocationError>>()?;
    members.sort_by_key(|(context, _, _)| context.historical_artifact_order);
    if members.is_empty() {
        return Err(ServiceInvocationError::new(
            "validation",
            "CT group has no planned members",
        ));
    }
    let study = classic_identity(members[0].2, CompositionUidRole::StudyInstance)?;
    let frame = classic_identity(members[0].2, CompositionUidRole::FrameOfReference)?;
    let mut series_by_index = BTreeMap::<u32, &str>::new();
    let mut last_position = BTreeMap::<u32, f64>::new();
    for (_, parameters, artifact) in &members {
        if classic_identity(artifact, CompositionUidRole::StudyInstance)? != study
            || classic_identity(artifact, CompositionUidRole::FrameOfReference)? != frame
        {
            return Err(ServiceInvocationError::new(
                "validation",
                "CT group does not share Study/Frame of Reference identity",
            ));
        }
        let series = classic_identity(artifact, CompositionUidRole::SeriesInstance)?;
        if let Some(existing) = series_by_index.insert(parameters.series_index, series) {
            if existing != series {
                return Err(ServiceInvocationError::new(
                    "validation",
                    "CT series index maps to multiple Series Instance UIDs",
                ));
            }
        }
        if let Some(previous) =
            last_position.insert(parameters.series_index, parameters.position_along_normal)
        {
            if parameters.position_along_normal <= previous {
                return Err(ServiceInvocationError::new(
                    "validation",
                    "CT planned slice positions are not strictly increasing within series",
                ));
            }
        }
    }
    let distinct = series_by_index.values().copied().collect::<BTreeSet<_>>();
    if distinct.len() != series_by_index.len() {
        return Err(ServiceInvocationError::new(
            "validation",
            "distinct CT series indexes share a Series Instance UID",
        ));
    }
    if provider.series_organization.is_some() != (series_by_index.len() > 1) {
        return Err(ServiceInvocationError::new(
            "validation",
            "CT cross-series organization declaration does not match planned topology",
        ));
    }
    Ok(TypedValidationCheck::passed_internal(
        "classic_ct_group_topology",
        "All planned CT siblings have ordered spatial positions, shared Study/Frame of Reference identity, and consistent distinct Series identity topology.",
    ))
}

fn classic_identity(
    artifact: &crate::corpus_plan::PlannedDicomArtifact,
    role: CompositionUidRole,
) -> Result<&str, ServiceInvocationError> {
    artifact.instance.identities.get(&role, 0).ok_or_else(|| {
        ServiceInvocationError::new("validation", format!("CT group lacks {role:?}"))
    })
}

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
