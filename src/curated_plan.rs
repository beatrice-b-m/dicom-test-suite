//! Plan-only assembly for the feature-free curated SC and classic slices.
//!
//! This frontend joins registry selection, versioned recipes, qualified
//! templates, neutral native pixels, and encoding service requests before an
//! executor transaction exists. It has deliberately no output-root or writer
//! API.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::codecs::{NativeRleLosslessEncoder, RLE_LOSSLESS_TRANSFER_SYNTAX_UID};
use crate::composition::{
    CompositionUidRole, ContentMaterialization, MaterializedReference, TemplateCatalog, TemplateId,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION,
    CaseBinding, CorpusPlan, CorpusPlanError, EvidenceIndependence, EvidenceObligation,
    EvidencePlan, OutputPlan, OutputRelativePath, PlannedArtifact, PlannedDicomArtifact,
    PublicationPlan, PublicationTransaction, ResourcePlan, ValidationPlan, ValidationRequirement,
    ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, CodecRequest, NativeFrameBinding, SlotExecutionBinding,
};
use crate::native_pixel::{ByteOrder, NativePixelFactory, NativePixelRequest};
use crate::planning::RecipeIdentity;
use crate::recipes::classic_ct::plan_ct_recipe;
use crate::recipes::classic_dx_mg::plan_dx_mg_recipe;
use crate::recipes::classic_mr_cr::plan_mr_cr_recipe;
use crate::recipes::classic_nuclear::plan_nuclear_recipe;
use crate::recipes::classic_vl_projection::plan_vl_projection_recipe;
use crate::recipes::{
    AdvancedArtifactPlanningContext, AdvancedArtifactProvenance, AdvancedPlanProvider,
    AdvancedPlanProviderOutput, AdvancedPlanProviderRequest, AdvancedProviderFamily,
    AdvancedProviderLimits, AdvancedSourceRole, CaseRecipe, ClassicInstanceRequest,
    ClassicResolvedPlanInput,
    EnhancedPlanProvider, MetadataScPlanInput, OrderedSeriesProvider,
    PRESENTATION_ADVANCED_PROVIDER_ID, PresentationPlanProvider, PresentationSourceInput,
    REGISTRATION_PLAN_PROVIDER_ID, RecipeCatalog, RecipeReference, RegistrationPlanProvider,
    RegistrationSourceInput, SecondaryCapturePlanInput, WSI_ADVANCED_PROVIDER_ID,
    WsiAdvancedPlanProvider, encoding_plan_from_recipe, native_pixel_request_from_recipe,
    resolved_classic_instance_plan, resolved_metadata_sc_plan, resolved_secondary_capture_plan,
};
use crate::sha256_hex;

const SC_PLAN_PROVIDER: &str = "native.sc_plan";
const METADATA_SC_PLAN_PROVIDER: &str = "native.metadata_sc_plan";
const CLASSIC_PLAN_PROVIDER: &str = "native.classic_plan";
const ENHANCED_PLAN_PROVIDER: &str = "native.enhanced_plan";
const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const EXPLICIT_VR_BE: &str = "1.2.840.10008.1.2.2";
const ARTIFACT_OVERHEAD_BYTES: u64 = 16 * 1024;

#[derive(Debug, Clone, Deserialize)]
struct ReferenceSourceDeclaration {
    recipe: RecipeReference,
    artifact_logical_id: String,
    role: AdvancedSourceRole,
    #[serde(default)]
    referenced_frames: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct ReferenceProviderParameters {
    sources: Vec<ReferenceSourceDeclaration>,
}

#[derive(Debug, Clone)]
struct CuratedSourceArtifact {
    planned: PlannedDicomArtifact,
    bindings: ArtifactExecutionBindings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratedCatalogPaths {
    pub recipes_root: PathBuf,
    pub registry_path: PathBuf,
    pub template_catalog_path: PathBuf,
    pub standards_lock_path: PathBuf,
}

impl CuratedCatalogPaths {
    pub fn from_repository_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            recipes_root: root.join("cases/recipes"),
            registry_path: root.join("cases/registry.json"),
            template_catalog_path: root.join("templates/catalog.json"),
            standards_lock_path: root.join("standards.lock.json"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CuratedScSelection {
    /// Every implemented, feature-free recipe currently owned by a migrated
    /// curated planner, including legacy-profile entries.
    AllFeatureFree,
    Profile {
        profile: String,
        include_stress: bool,
    },
    CaseIds(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratedScPlanRequest {
    pub selection: CuratedScSelection,
    pub seed: u64,
    pub max_parallelism: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingCuratedCase {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub reason_code: String,
    pub message: String,
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeContentServiceRequest {
    pub artifact_id: String,
    pub slot: String,
    pub factory_id: String,
    pub request: NativePixelRequest,
    pub unpadded_size_bytes: u64,
    pub unpadded_sha256: String,
    pub frame_sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuratedScCorpusPlan {
    pub plan: CorpusPlan,
    pub bindings: BTreeMap<String, ArtifactExecutionBindings>,
    pub native_content_requests: Vec<NativeContentServiceRequest>,
    pub projection: CuratedScProjectionContext,
    pub pending: Vec<PendingCuratedCase>,
}

/// Immutable source metadata needed to project a curated artifact after
/// execution. It contains no output-root path and no materialized bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuratedArtifactProjectionContext {
    pub artifact_id: String,
    pub plan_order: u64,
    /// Stable order of the case in the authoritative registry.
    pub registry_order: u64,
    /// Historical generator order carried by the versioned case recipe.
    pub historical_recipe_order: u32,
    pub historical_artifact_order: u32,
    pub registry_case: RegistryCaseProjection,
    pub case_recipe: CaseRecipe,
    pub artifact_recipe: crate::recipes::PlannedArtifactRecipe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuratedScProjectionContext {
    pub artifacts: Vec<CuratedArtifactProjectionContext>,
}

impl CuratedScProjectionContext {
    pub fn validate(&self, plan: &CorpusPlan) -> Result<(), CuratedPlanError> {
        if self.artifacts.len() != plan.artifacts.len() {
            return Err(CuratedPlanError::ProjectionArtifactSetMismatch);
        }
        let mut ids = BTreeSet::new();
        for (planned, projected) in plan.artifacts.iter().zip(&self.artifacts) {
            if !ids.insert(projected.artifact_id.clone())
                || planned.logical_id() != projected.artifact_id
                || planned.order() != projected.plan_order
                || projected.registry_case.case_id != projected.case_recipe.binding.case_id
                || projected.registry_case.recipe_id != projected.case_recipe.recipe_id
                || projected.registry_case.recipe_version != projected.case_recipe.recipe_version
                || projection_artifact_id(
                    &projected.case_recipe,
                    &projected.artifact_recipe.logical_id,
                ) != projected.artifact_id
                || projected.artifact_recipe.order != projected.historical_artifact_order
                || projected.case_recipe.planning_order != Some(projected.historical_recipe_order)
            {
                return Err(CuratedPlanError::ProjectionArtifactMismatch(
                    projected.artifact_id.clone(),
                ));
            }
        }
        Ok(())
    }
}

/// Lossless typed copy of an authoritative registry case. Open-ended nested
/// policy records stay as JSON values so projection retains their exact shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryCaseProjection {
    pub case_id: String,
    pub status: String,
    pub profiles: Vec<String>,
    pub recipe_id: String,
    pub recipe_version: String,
    pub iod_name: Option<String>,
    pub sop_class_name: Option<String>,
    pub sop_class_uid: Option<String>,
    pub transfer_syntax_uid: Option<String>,
    pub determinism: String,
    pub requirements: RegistryRequirements,
    pub skip: Value,
    pub standards_evidence: Vec<Value>,
    pub provider: Value,
    pub roadmap: Value,
    pub blockers: Vec<Value>,
    pub modality: Option<String>,
    pub object_family: String,
    pub compatibility_axes: Vec<String>,
    pub artifact_kind: String,
}

#[derive(Debug)]
pub struct CuratedScCorpusPlanProvider {
    registry: RegistryDocument,
    recipes: RecipeCatalog,
    templates: TemplateCatalog,
    standards_lock_sha256: String,
}

impl CuratedScCorpusPlanProvider {
    pub fn load(paths: CuratedCatalogPaths) -> Result<Self, CuratedPlanError> {
        let registry_bytes = read(&paths.registry_path)?;
        let registry =
            serde_json::from_slice(&registry_bytes).map_err(|error| CuratedPlanError::Parse {
                path: paths.registry_path.clone(),
                message: error.to_string(),
            })?;
        let recipes = RecipeCatalog::load(
            &paths.recipes_root,
            &paths.registry_path,
            &paths.template_catalog_path,
        )
        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
        let templates = TemplateCatalog::load(&paths.template_catalog_path)
            .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
        let standards_lock_sha256 = sha256_hex(&read(&paths.standards_lock_path)?);
        Ok(Self {
            registry,
            recipes,
            templates,
            standards_lock_sha256,
        })
    }

    pub fn plan(
        &self,
        request: &CuratedScPlanRequest,
    ) -> Result<CuratedScCorpusPlan, CuratedPlanError> {
        if request.max_parallelism == 0 {
            return Err(CuratedPlanError::ZeroParallelism);
        }
        let mut selected_ids = selected_case_ids(&self.registry, &request.selection)?;
        expand_recipe_dependency_closure(&self.recipes, &mut selected_ids)?;
        let mut artifacts = Vec::new();
        let mut bindings = BTreeMap::new();
        let mut native_content_requests = Vec::new();
        let mut projection_artifacts = Vec::new();
        let pending = Vec::new();
        let mut artifact_by_recipe_role = BTreeMap::new();
        let mut source_artifacts = BTreeMap::new();
        let mut selected_recipes = Vec::new();
        let mut classic_dependencies = Vec::new();
        let mut advanced_dependencies = Vec::new();

        let registry_order = self
            .registry
            .cases
            .iter()
            .enumerate()
            .map(|(order, case)| (case.case_id.as_str(), order as u64))
            .collect::<BTreeMap<_, _>>();
        let mut selected_registry_cases = self.registry.cases.iter().collect::<Vec<_>>();
        selected_registry_cases.sort_by_key(|registry_case| {
            self.recipes
                .binding_for_case(&registry_case.case_id)
                .and_then(|identity| self.recipes.recipes().get(identity))
                .and_then(|recipe| recipe.planning_order)
                .unwrap_or(u32::MAX)
        });

        for registry_case in selected_registry_cases {
            if !selected_ids.contains(&registry_case.case_id)
                || registry_case.status != "implemented"
                || !registry_case.requirements.is_feature_free()
            {
                continue;
            }
            let identity = self
                .recipes
                .binding_for_case(&registry_case.case_id)
                .ok_or_else(|| CuratedPlanError::MissingRecipe(registry_case.case_id.clone()))?;
            let recipe = self
                .recipes
                .recipes()
                .get(identity)
                .expect("recipe binding points to a loaded recipe");
            if matches!(
                recipe.plan_provider_id.as_str(),
                ENHANCED_PLAN_PROVIDER | WSI_ADVANCED_PROVIDER_ID
            ) {
                let family = if recipe.plan_provider_id == ENHANCED_PLAN_PROVIDER {
                    AdvancedProviderFamily::Enhanced
                } else {
                    AdvancedProviderFamily::WholeSlide
                };
                let provider_output = if family == AdvancedProviderFamily::Enhanced {
                    let input = self
                        .recipes
                        .enhanced_input_for_case(&registry_case.case_id)
                        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    let provider = EnhancedPlanProvider::new(self.standards_lock_sha256.clone())
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?;
                    let mut contexts = provider
                        .recipe_default_contexts(&input, request.seed)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?;
                    assign_global_context_order(&mut contexts, artifacts.len())?;
                    let provider_request = advanced_provider_request(
                        recipe,
                        family,
                        request.seed,
                        request.max_parallelism,
                        contexts,
                        0,
                    )?;
                    provider
                        .plan_typed(&provider_request, &input)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                } else {
                    let input = self
                        .recipes
                        .wsi_input_for_case(&registry_case.case_id)
                        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    let provider = WsiAdvancedPlanProvider::new(self.standards_lock_sha256.clone());
                    let mut contexts = provider
                        .recipe_default_contexts(&input, request.seed)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?;
                    assign_global_context_order(&mut contexts, artifacts.len())?;
                    let provider_request = advanced_provider_request(
                        recipe,
                        family,
                        request.seed,
                        request.max_parallelism,
                        contexts,
                        0,
                    )?;
                    provider.plan(&provider_request, &input).map_err(|error| {
                        CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        }
                    })?
                };
                merge_advanced_output(
                    recipe,
                    registry_case,
                    registry_order[registry_case.case_id.as_str()],
                    provider_output,
                    &mut artifacts,
                    &mut bindings,
                    &mut projection_artifacts,
                    &mut artifact_by_recipe_role,
                    &mut source_artifacts,
                    &mut advanced_dependencies,
                )?;
                selected_recipes.push(recipe);
                continue;
            }
            if matches!(
                recipe.plan_provider_id.as_str(),
                REGISTRATION_PLAN_PROVIDER_ID | PRESENTATION_ADVANCED_PROVIDER_ID
            ) {
                let declarations = reference_source_declarations(recipe)?;
                let target_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                let target_recipe_id = target_recipe.logical_id.clone();
                let target_id = artifact_id(recipe, &target_recipe_id);
                let provider_output = if recipe.plan_provider_id == REGISTRATION_PLAN_PROVIDER_ID {
                    let mut input = self
                        .recipes
                        .registration_input_for_case(
                            &registry_case.case_id,
                            registration_sources(
                                &declarations,
                                &source_artifacts,
                                &target_id,
                            )?,
                        )
                        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    restore_registration_sources(
                        &mut input.sources,
                        &declarations,
                        &source_artifacts,
                    )?;
                    let provider = RegistrationPlanProvider::new(self.standards_lock_sha256.clone())
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?;
                    let mut contexts = provider
                        .recipe_default_contexts(
                            &input,
                            &recipe.binding.case_id,
                            &recipe.identity(),
                            request.seed,
                        )
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?;
                    contexts[0].output.role = target_recipe.output.role.clone();
                    contexts[0].target_instance_id = target_id.clone();
                    contexts[0].identities.logical_instance_id = target_id.clone();
                    assign_global_context_order(&mut contexts, artifacts.len())?;
                    let provider_request = advanced_provider_request(
                        recipe,
                        AdvancedProviderFamily::Registration,
                        request.seed,
                        request.max_parallelism,
                        contexts,
                        declarations.len(),
                    )?;
                    provider
                        .plan_typed(&provider_request, &input)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                } else {
                    let mut input = self
                        .recipes
                        .presentation_input_for_case(
                            &registry_case.case_id,
                            presentation_sources(&declarations, &source_artifacts)?,
                        )
                        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    restore_presentation_sources(
                        &mut input.sources,
                        &declarations,
                        &source_artifacts,
                    )?;
                    let provider = PresentationPlanProvider::new(self.standards_lock_sha256.clone());
                    let mut contexts = provider
                        .recipe_default_contexts(&input, request.seed)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?;
                    contexts[0].target_instance_id = target_id.clone();
                    contexts[0].identities.logical_instance_id = target_id.clone();
                    assign_global_context_order(&mut contexts, artifacts.len())?;
                    let provider_request = advanced_provider_request(
                        recipe,
                        AdvancedProviderFamily::PresentationState,
                        request.seed,
                        request.max_parallelism,
                        contexts,
                        declarations.len(),
                    )?;
                    provider.plan(&provider_request, &input).map_err(|error| {
                        CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        }
                    })?
                };
                merge_reference_output(
                    recipe,
                    registry_case,
                    registry_order[registry_case.case_id.as_str()],
                    target_recipe,
                    provider_output,
                    &source_artifacts,
                    &mut artifacts,
                    &mut bindings,
                    &mut projection_artifacts,
                    &mut artifact_by_recipe_role,
                    &mut advanced_dependencies,
                )?;
                continue;
            }
            if recipe.plan_provider_id == CLASSIC_PLAN_PROVIDER {
                let requests = classic_requests(recipe, &self.standards_lock_sha256, request.seed)?;
                let scope = format!("curated_{}", recipe.recipe_id);
                let planned_instances = OrderedSeriesProvider
                    .plan_scoped(&scope, requests)
                    .map_err(|error| CuratedPlanError::ClassicPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    })?;
                let dicom = recipe
                    .dicom
                    .as_ref()
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                selected_recipes.push(recipe);
                for planned in planned_instances {
                    let global_id = planned.logical_id.clone();
                    let artifact_recipe = dicom
                        .artifacts
                        .iter()
                        .find(|artifact| artifact_id(recipe, &artifact.logical_id) == global_id)
                        .ok_or_else(|| CuratedPlanError::ClassicArtifactMismatch {
                            recipe_id: recipe.recipe_id.clone(),
                            artifact_id: global_id.clone(),
                        })?;
                    let reference = artifact_recipe
                        .template
                        .as_ref()
                        .ok_or_else(|| CuratedPlanError::MissingTemplate(global_id.clone()))?;
                    let template = self
                        .templates
                        .resolve_qualified(
                            &TemplateId(reference.template_id.clone()),
                            Some(reference.template_version.parse().map_err(|_| {
                                CuratedPlanError::InvalidTemplateVersion(
                                    reference.template_version.clone(),
                                )
                            })?),
                        )
                        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
                    let native_request = planned.pixels.content_request.clone();
                    let native = planned.pixels.content.clone();
                    let planned_output_path = planned.output_relative_path.clone();
                    for depends_on in &planned.dependencies {
                        classic_dependencies.push(ArtifactDependency {
                            artifact_id: global_id.clone(),
                            depends_on: depends_on.clone(),
                            relationship: "classic_instance_dependency".into(),
                            frame_numbers: Vec::new(),
                        });
                    }
                    let mut instance = resolved_classic_instance_plan(ClassicResolvedPlanInput {
                        planned,
                        template,
                        transfer_syntax_uid: &artifact_recipe.encoding.transfer_syntax_uid,
                        encoding_backend_id: artifact_recipe
                            .encoding
                            .non_template_encoding_provider_id
                            .as_deref()
                            .unwrap_or("dicom-rs.part10"),
                    })
                    .map_err(|error| CuratedPlanError::ClassicPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    })?;
                    patch_native_content(&mut instance, &native)?;
                    let implementation_class_uid = instance
                        .identities
                        .get(&CompositionUidRole::ImplementationClass, 0)
                        .ok_or_else(|| CuratedPlanError::MissingImplementation(global_id.clone()))?
                        .to_owned();
                    let encoding = encoding_plan_from_recipe(
                        &artifact_recipe.encoding,
                        crate::corpus_plan::ImplementationIdentityPlan {
                            class_uid: implementation_class_uid,
                            version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
                        },
                    )
                    .map_err(|error| CuratedPlanError::Encoding {
                        artifact_id: global_id.clone(),
                        message: error.to_string(),
                    })?;
                    let output_path = artifact_recipe.output.path.as_ref().ok_or_else(|| {
                        CuratedPlanError::ProviderDerivedOutput(global_id.clone())
                    })?;
                    if output_path != planned_output_path.as_str() {
                        return Err(CuratedPlanError::ClassicArtifactMismatch {
                            recipe_id: recipe.recipe_id.clone(),
                            artifact_id: global_id,
                        });
                    }
                    let resources = resource_estimate(&instance, native.plan.padded_value_bytes)?;
                    let order = u64::try_from(artifacts.len())
                        .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                    let historical_recipe_order = recipe.planning_order.ok_or_else(|| {
                        CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone())
                    })?;
                    artifacts.push(PlannedArtifact::Dicom(PlannedDicomArtifact {
                        logical_id: global_id.clone(),
                        order,
                        provenance: ArtifactProvenance::Requested,
                        case_binding: Some(CaseBinding {
                            case_id: recipe.binding.case_id.clone(),
                            recipe_id: recipe.recipe_id.clone(),
                            recipe_version: recipe.recipe_version.clone(),
                        }),
                        instance,
                        output: OutputPlan {
                            relative_path: OutputRelativePath::new(output_path.clone())?,
                            role: artifact_recipe.output.role.clone(),
                            publish: true,
                        },
                        encoding,
                        validation: validation_plan(recipe, artifact_recipe),
                        evidence: generation_evidence_plan(),
                        resources,
                    }));
                    projection_artifacts.push(CuratedArtifactProjectionContext {
                        artifact_id: global_id.clone(),
                        plan_order: order,
                        registry_order: registry_order[registry_case.case_id.as_str()],
                        historical_recipe_order,
                        historical_artifact_order: artifact_recipe.order,
                        registry_case: registry_case.clone().into(),
                        case_recipe: recipe.clone(),
                        artifact_recipe: artifact_recipe.clone(),
                    });
                    artifact_by_recipe_role.insert(
                        (recipe.identity(), artifact_recipe.output.role.clone()),
                        global_id.clone(),
                    );
                    native_content_requests.push(native_content_request(
                        &global_id,
                        artifact_recipe,
                        native_request,
                        &native,
                    ));
                    let execution_binding = execution_binding(&global_id, artifact_recipe, &native)?;
                    bindings.insert(global_id.clone(), execution_binding.clone());
                    let PlannedArtifact::Dicom(source_planned) = artifacts
                        .last()
                        .expect("classic artifact was just appended")
                    else {
                        unreachable!()
                    };
                    source_artifacts.insert(
                        (recipe.identity(), artifact_recipe.logical_id.clone()),
                        CuratedSourceArtifact {
                            planned: source_planned.clone(),
                            bindings: execution_binding,
                        },
                    );
                }
                continue;
            }
            if !matches!(
                recipe.plan_provider_id.as_str(),
                SC_PLAN_PROVIDER | METADATA_SC_PLAN_PROVIDER
            ) {
                if matches!(request.selection, CuratedScSelection::CaseIds(_)) {
                    return Err(CuratedPlanError::UnsupportedCase {
                        case_id: registry_case.case_id.clone(),
                        provider_id: recipe.plan_provider_id.clone(),
                    });
                }
                continue;
            }
            let dicom = recipe
                .dicom
                .as_ref()
                .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
            let mut recipe_artifacts = dicom.artifacts.iter().collect::<Vec<_>>();
            recipe_artifacts.sort_by_key(|artifact| artifact.order);
            selected_recipes.push(recipe);
            for artifact_recipe in recipe_artifacts {
                let global_id = artifact_id(recipe, &artifact_recipe.logical_id);
                let reference = artifact_recipe
                    .template
                    .as_ref()
                    .ok_or_else(|| CuratedPlanError::MissingTemplate(global_id.clone()))?;
                let template = self
                    .templates
                    .resolve_qualified(
                        &TemplateId(reference.template_id.clone()),
                        Some(reference.template_version.parse().map_err(|_| {
                            CuratedPlanError::InvalidTemplateVersion(
                                reference.template_version.clone(),
                            )
                        })?),
                    )
                    .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
                let mut instance = if recipe.plan_provider_id == METADATA_SC_PLAN_PROVIDER {
                    resolved_metadata_sc_plan(MetadataScPlanInput {
                        recipe,
                        artifact: artifact_recipe,
                        template,
                        instance_id: &global_id,
                        standards_lock_sha256: &self.standards_lock_sha256,
                        seed: request.seed,
                    })
                    .map_err(|error| CuratedPlanError::ScPlan {
                        artifact_id: global_id.clone(),
                        message: error.to_string(),
                    })?
                } else {
                    resolved_secondary_capture_plan(SecondaryCapturePlanInput {
                        recipe,
                        artifact: artifact_recipe,
                        template,
                        instance_id: &global_id,
                        standards_lock_sha256: &self.standards_lock_sha256,
                        seed: request.seed,
                    })
                    .map_err(|error| CuratedPlanError::ScPlan {
                        artifact_id: global_id.clone(),
                        message: error.to_string(),
                    })?
                };
                let sc = artifact_recipe
                    .secondary_capture
                    .as_ref()
                    .ok_or_else(|| CuratedPlanError::MissingSecondaryCapture(global_id.clone()))?;
                let mut native_request = native_pixel_request_from_recipe(sc).map_err(|error| {
                    CuratedPlanError::ScPlan {
                        artifact_id: global_id.clone(),
                        message: error.to_string(),
                    }
                })?;
                if artifact_recipe.encoding.transfer_syntax_uid == EXPLICIT_VR_BE {
                    native_request.shape.byte_order = ByteOrder::Big;
                    // Recipe frame identities describe the canonical LE source.
                    // The BE content identity is derived here from the same
                    // stored values and is intentionally distinct.
                    native_request.expected_frame_sha256.clear();
                }
                let native =
                    NativePixelFactory
                        .create(native_request.clone())
                        .map_err(|error| CuratedPlanError::ScPlan {
                            artifact_id: global_id.clone(),
                            message: error.to_string(),
                        })?;
                patch_native_content(&mut instance, &native)?;

                let implementation_class_uid = instance
                    .identities
                    .get(&CompositionUidRole::ImplementationClass, 0)
                    .ok_or_else(|| CuratedPlanError::MissingImplementation(global_id.clone()))?
                    .to_owned();
                let encoding = encoding_plan_from_recipe(
                    &artifact_recipe.encoding,
                    crate::corpus_plan::ImplementationIdentityPlan {
                        class_uid: implementation_class_uid,
                        version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
                    },
                )
                .map_err(|error| CuratedPlanError::Encoding {
                    artifact_id: global_id.clone(),
                    message: error.to_string(),
                })?;
                let output_path =
                    artifact_recipe.output.path.as_ref().ok_or_else(|| {
                        CuratedPlanError::ProviderDerivedOutput(global_id.clone())
                    })?;
                let resources = resource_estimate(&instance, native.plan.padded_value_bytes)?;
                let validation = validation_plan(recipe, artifact_recipe);
                let order = u64::try_from(artifacts.len())
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                let historical_recipe_order = recipe.planning_order.ok_or_else(|| {
                    CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone())
                })?;
                artifacts.push(PlannedArtifact::Dicom(PlannedDicomArtifact {
                    logical_id: global_id.clone(),
                    order,
                    provenance: ArtifactProvenance::Requested,
                    case_binding: Some(CaseBinding {
                        case_id: recipe.binding.case_id.clone(),
                        recipe_id: recipe.recipe_id.clone(),
                        recipe_version: recipe.recipe_version.clone(),
                    }),
                    instance,
                    output: OutputPlan {
                        relative_path: OutputRelativePath::new(output_path.clone())?,
                        role: artifact_recipe.output.role.clone(),
                        publish: true,
                    },
                    encoding,
                    validation,
                    evidence: generation_evidence_plan(),
                    resources,
                }));
                projection_artifacts.push(CuratedArtifactProjectionContext {
                    artifact_id: global_id.clone(),
                    plan_order: order,
                    registry_order: registry_order[registry_case.case_id.as_str()],
                    historical_recipe_order,
                    historical_artifact_order: artifact_recipe.order,
                    registry_case: registry_case.clone().into(),
                    case_recipe: recipe.clone(),
                    artifact_recipe: artifact_recipe.clone(),
                });
                artifact_by_recipe_role.insert(
                    (recipe.identity(), artifact_recipe.output.role.clone()),
                    global_id.clone(),
                );
                native_content_requests.push(native_content_request(
                    &global_id,
                    artifact_recipe,
                    native_request,
                    &native,
                ));
                let execution_binding = execution_binding(&global_id, artifact_recipe, &native)?;
                bindings.insert(global_id.clone(), execution_binding.clone());
                let PlannedArtifact::Dicom(source_planned) = artifacts
                    .last()
                    .expect("SC artifact was just appended")
                else {
                    unreachable!()
                };
                source_artifacts.insert(
                    (recipe.identity(), artifact_recipe.logical_id.clone()),
                    CuratedSourceArtifact {
                        planned: source_planned.clone(),
                        bindings: execution_binding,
                    },
                );
            }
        }

        let mut dependencies = dependencies(&selected_recipes, &artifact_by_recipe_role)?;
        dependencies.extend(classic_dependencies);
        dependencies.extend(advanced_dependencies);
        let unavailable = Vec::new();
        let (total_output, peak_working) = aggregate_resources(&artifacts)?;
        let plan = CorpusPlan {
            schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
            seed: request.seed,
            artifacts,
            dependencies,
            unavailable,
            publication: PublicationPlan {
                manifest_path: OutputRelativePath::new("manifest.json")?,
                transaction: PublicationTransaction::AtomicNoReplace,
                private_staging: true,
                no_overwrite: true,
            },
            resources: ResourcePlan {
                max_artifacts: u64::try_from(bindings.len())
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?
                    .max(1),
                max_total_output_bytes: total_output.max(1),
                max_peak_working_bytes: peak_working.max(1),
                max_parallelism: request.max_parallelism,
            },
        };
        plan.validate()?;
        let projection = CuratedScProjectionContext {
            artifacts: projection_artifacts,
        };
        projection.validate(&plan)?;
        Ok(CuratedScCorpusPlan {
            plan,
            bindings,
            native_content_requests,
            projection,
            pending,
        })
    }
}

fn advanced_provider_request(
    recipe: &CaseRecipe,
    family: AdvancedProviderFamily,
    seed: u64,
    max_parallelism: u32,
    artifact_contexts: Vec<AdvancedArtifactPlanningContext>,
    private_source_count: usize,
) -> Result<AdvancedPlanProviderRequest, CuratedPlanError> {
    let public_artifact_count = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?
        .artifacts
        .len() as u64;
    let artifact_count = public_artifact_count
        .checked_add(
            u64::try_from(private_source_count).map_err(|_| CuratedPlanError::ResourceOverflow)?,
        )
        .ok_or(CuratedPlanError::ResourceOverflow)?;
    if artifact_count == 0 {
        return Err(CuratedPlanError::MissingDicom(recipe.recipe_id.clone()));
    }
    let per_artifact_budget = 128_u64 * 1024 * 1024;
    let total_budget = artifact_count
        .checked_mul(per_artifact_budget)
        .ok_or(CuratedPlanError::ResourceOverflow)?;
    Ok(AdvancedPlanProviderRequest {
        provider_id: recipe.plan_provider_id.clone(),
        family,
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        seed,
        artifact_contexts,
        limits: AdvancedProviderLimits {
            max_artifacts: artifact_count,
            max_references: artifact_count,
            max_binding_slots: artifact_count,
            max_total_output_bytes: total_budget,
            max_peak_working_bytes: per_artifact_budget,
            max_parallelism,
        },
    })
}

fn assign_global_context_order(
    contexts: &mut [AdvancedArtifactPlanningContext],
    first: usize,
) -> Result<(), CuratedPlanError> {
    let first = u64::try_from(first).map_err(|_| CuratedPlanError::ResourceOverflow)?;
    for (offset, context) in contexts.iter_mut().enumerate() {
        context.order = first
            .checked_add(u64::try_from(offset).map_err(|_| CuratedPlanError::ResourceOverflow)?)
            .ok_or(CuratedPlanError::ResourceOverflow)?;
    }
    Ok(())
}

fn reference_source_declarations(
    recipe: &CaseRecipe,
) -> Result<Vec<ReferenceSourceDeclaration>, CuratedPlanError> {
    serde_json::from_value::<ReferenceProviderParameters>(Value::Object(
        recipe.provider_parameters.clone(),
    ))
    .map(|parameters| parameters.sources)
    .map_err(|error| CuratedPlanError::AdvancedPlan {
        recipe_id: recipe.recipe_id.clone(),
        message: format!("reference source declarations: {error}"),
    })
}

fn source_for_declaration<'a>(
    declaration: &ReferenceSourceDeclaration,
    sources: &'a BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<&'a CuratedSourceArtifact, CuratedPlanError> {
    sources
        .get(&(
            declaration.recipe.identity(),
            declaration.artifact_logical_id.clone(),
        ))
        .ok_or_else(|| CuratedPlanError::MissingDependency {
            recipe: declaration.recipe.identity(),
            dependency: declaration.recipe.identity(),
            role: declaration.artifact_logical_id.clone(),
        })
}

fn registration_sources(
    declarations: &[ReferenceSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
    target_id: &str,
) -> Result<Vec<RegistrationSourceInput>, CuratedPlanError> {
    declarations
        .iter()
        .map(|declaration| {
            let source = source_for_declaration(declaration, sources)?;
            let sop = source
                .planned
                .instance
                .identities
                .get(&CompositionUidRole::SopInstance, 0)
                .ok_or_else(|| {
                    CuratedPlanError::MissingImplementation(source.planned.logical_id.clone())
                })?;
            let mut planned = source.planned.clone();
            planned.logical_id = declaration.artifact_logical_id.clone();
            let mut bindings = source.bindings.clone();
            bindings.artifact_id = declaration.artifact_logical_id.clone();
            Ok(RegistrationSourceInput {
                role: declaration.role.clone(),
                reference: MaterializedReference {
                    source_instance_id: target_id.into(),
                    target_instance_id: planned.instance.instance_id.clone(),
                    role: "source_image".into(),
                    frame_role: None,
                    referenced_sop_class_uid: planned.instance.sop_class_uid.clone(),
                    referenced_sop_instance_uid: sop.into(),
                    referenced_frames: declaration.referenced_frames.clone(),
                },
                artifact: planned,
                bindings,
            })
        })
        .collect()
}

fn presentation_sources(
    declarations: &[ReferenceSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<Vec<PresentationSourceInput>, CuratedPlanError> {
    declarations
        .iter()
        .enumerate()
        .map(|(index, declaration)| {
            let source = source_for_declaration(declaration, sources)?;
            let mut planned = source.planned.clone();
            planned.logical_id = declaration.artifact_logical_id.clone();
            let mut bindings = source.bindings.clone();
            bindings.artifact_id = declaration.artifact_logical_id.clone();
            Ok(PresentationSourceInput {
                ordinal: u32::try_from(index + 1)
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?,
                role: declaration.role.clone(),
                referenced_frames: declaration.referenced_frames.clone(),
                artifact: planned,
                binding: bindings,
            })
        })
        .collect()
}

fn restore_registration_sources(
    provider_sources: &mut [RegistrationSourceInput],
    declarations: &[ReferenceSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<(), CuratedPlanError> {
    for (provider_source, declaration) in provider_sources.iter_mut().zip(declarations) {
        let source = source_for_declaration(declaration, sources)?;
        provider_source.artifact = source.planned.clone();
        provider_source.bindings = source.bindings.clone();
    }
    Ok(())
}

fn restore_presentation_sources(
    provider_sources: &mut [PresentationSourceInput],
    declarations: &[ReferenceSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<(), CuratedPlanError> {
    for (provider_source, declaration) in provider_sources.iter_mut().zip(declarations) {
        let source = source_for_declaration(declaration, sources)?;
        provider_source.artifact = source.planned.clone();
        provider_source.binding = source.bindings.clone();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn merge_reference_output(
    recipe: &CaseRecipe,
    registry_case: &RegistryCase,
    registry_order: u64,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    output: AdvancedPlanProviderOutput,
    source_artifacts: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
    artifacts: &mut Vec<PlannedArtifact>,
    bindings: &mut BTreeMap<String, ArtifactExecutionBindings>,
    projection_artifacts: &mut Vec<CuratedArtifactProjectionContext>,
    artifact_by_recipe_role: &mut BTreeMap<(RecipeIdentity, String), String>,
    dependencies: &mut Vec<ArtifactDependency>,
) -> Result<(), CuratedPlanError> {
    let mut output_bindings = output
        .bindings
        .into_iter()
        .map(|binding| (binding.artifact_id.clone(), binding))
        .collect::<BTreeMap<_, _>>();
    let target_id = artifact_id(recipe, &artifact_recipe.logical_id);
    let mut target = None;
    for advanced in output.artifacts {
        if advanced.provenance == AdvancedArtifactProvenance::Requested {
            if target.replace(advanced.planned).is_some() {
                return Err(CuratedPlanError::AdvancedArtifactMismatch {
                    recipe_id: recipe.recipe_id.clone(),
                    artifact_id: target_id.clone(),
                });
            }
            continue;
        }
        let source = source_artifacts
            .values()
            .find(|source| source.planned.logical_id == advanced.planned.logical_id)
            .ok_or_else(|| CuratedPlanError::AdvancedProvenance(advanced.planned.logical_id.clone()))?;
        let mut expected = source.planned.clone();
        expected.provenance = advanced.planned.provenance.clone();
        expected.output.publish = false;
        if advanced.planned != expected
            || output_bindings.remove(&advanced.planned.logical_id) != Some(source.bindings.clone())
        {
            return Err(CuratedPlanError::AdvancedArtifactMismatch {
                recipe_id: recipe.recipe_id.clone(),
                artifact_id: advanced.planned.logical_id,
            });
        }
    }
    let target = target.ok_or_else(|| CuratedPlanError::AdvancedArtifactMismatch {
        recipe_id: recipe.recipe_id.clone(),
        artifact_id: target_id.clone(),
    })?;
    if target.logical_id != target_id
        || target.order
            != u64::try_from(artifacts.len()).map_err(|_| CuratedPlanError::ResourceOverflow)?
        || target.output.relative_path.as_str() != artifact_recipe.output.path.as_deref().unwrap_or("")
        || target.output.role != artifact_recipe.output.role
    {
        return Err(CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: target.logical_id,
        });
    }
    let target_binding = output_bindings.remove(&target.logical_id).ok_or_else(|| {
        CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: target.logical_id.clone(),
        }
    })?;
    if !output_bindings.is_empty() {
        return Err(CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: "reference_bindings".into(),
        });
    }
    let order = target.order;
    let artifact_id = target.logical_id.clone();
    bindings.insert(artifact_id.clone(), target_binding.clone());
    artifacts.push(PlannedArtifact::Dicom(target.clone()));
    projection_artifacts.push(CuratedArtifactProjectionContext {
        artifact_id: artifact_id.clone(),
        plan_order: order,
        registry_order,
        historical_recipe_order: recipe
            .planning_order
            .ok_or_else(|| CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone()))?,
        historical_artifact_order: artifact_recipe.order,
        registry_case: registry_case.clone().into(),
        case_recipe: recipe.clone(),
        artifact_recipe: artifact_recipe.clone(),
    });
    artifact_by_recipe_role.insert(
        (recipe.identity(), artifact_recipe.output.role.clone()),
        artifact_id.clone(),
    );
    dependencies.extend(output.dependencies);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn merge_advanced_output(
    recipe: &CaseRecipe,
    registry_case: &RegistryCase,
    registry_order: u64,
    output: AdvancedPlanProviderOutput,
    artifacts: &mut Vec<PlannedArtifact>,
    bindings: &mut BTreeMap<String, ArtifactExecutionBindings>,
    projection_artifacts: &mut Vec<CuratedArtifactProjectionContext>,
    artifact_by_recipe_role: &mut BTreeMap<(RecipeIdentity, String), String>,
    source_artifacts: &mut BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
    dependencies: &mut Vec<ArtifactDependency>,
) -> Result<(), CuratedPlanError> {
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
    if output.artifacts.len() != dicom.artifacts.len()
        || output.bindings.len() != dicom.artifacts.len()
    {
        return Err(CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: "artifact_set".into(),
        });
    }
    let mut output_bindings = output
        .bindings
        .into_iter()
        .map(|binding| (binding.artifact_id.clone(), binding))
        .collect::<BTreeMap<_, _>>();
    let mut seen_recipe_artifacts = BTreeSet::new();
    for advanced in output.artifacts {
        if advanced.provenance != AdvancedArtifactProvenance::Requested
            || advanced.planned.provenance != ArtifactProvenance::Requested
        {
            return Err(CuratedPlanError::AdvancedProvenance(
                advanced.planned.logical_id,
            ));
        }
        let artifact_recipe = dicom
            .artifacts
            .iter()
            .find(|artifact| artifact.logical_id == advanced.planned.logical_id)
            .ok_or_else(|| CuratedPlanError::AdvancedArtifactMismatch {
                recipe_id: recipe.recipe_id.clone(),
                artifact_id: advanced.planned.logical_id.clone(),
            })?;
        if !seen_recipe_artifacts.insert(artifact_recipe.logical_id.as_str())
            || artifact_recipe.output.path.as_deref()
                != Some(advanced.planned.output.relative_path.as_str())
            || artifact_recipe.output.role != advanced.planned.output.role
            || advanced
                .planned
                .case_binding
                .as_ref()
                .is_none_or(|binding| {
                    binding.case_id != recipe.binding.case_id
                        || binding.recipe_id != recipe.recipe_id
                        || binding.recipe_version != recipe.recipe_version
                })
        {
            return Err(CuratedPlanError::AdvancedArtifactMismatch {
                recipe_id: recipe.recipe_id.clone(),
                artifact_id: advanced.planned.logical_id.clone(),
            });
        }
        let planned = advanced.planned;
        let global_order =
            u64::try_from(artifacts.len()).map_err(|_| CuratedPlanError::ResourceOverflow)?;
        if planned.order != global_order {
            return Err(CuratedPlanError::AdvancedArtifactMismatch {
                recipe_id: recipe.recipe_id.clone(),
                artifact_id: planned.logical_id,
            });
        }
        let artifact_id = planned.logical_id.clone();
        let binding = output_bindings.remove(&artifact_id).ok_or_else(|| {
            CuratedPlanError::AdvancedArtifactMismatch {
                recipe_id: recipe.recipe_id.clone(),
                artifact_id: artifact_id.clone(),
            }
        })?;
        if bindings.insert(artifact_id.clone(), binding).is_some() {
            return Err(CuratedPlanError::AdvancedArtifactMismatch {
                recipe_id: recipe.recipe_id.clone(),
                artifact_id,
            });
        }
        source_artifacts.insert(
            (recipe.identity(), artifact_recipe.logical_id.clone()),
            CuratedSourceArtifact {
                planned: planned.clone(),
                bindings: bindings[&artifact_id].clone(),
            },
        );
        artifact_by_recipe_role.insert(
            (recipe.identity(), artifact_recipe.output.role.clone()),
            planned.logical_id.clone(),
        );
        projection_artifacts.push(CuratedArtifactProjectionContext {
            artifact_id: planned.logical_id.clone(),
            plan_order: global_order,
            registry_order,
            historical_recipe_order: recipe.planning_order.ok_or_else(|| {
                CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone())
            })?,
            historical_artifact_order: artifact_recipe.order,
            registry_case: registry_case.clone().into(),
            case_recipe: recipe.clone(),
            artifact_recipe: artifact_recipe.clone(),
        });
        artifacts.push(PlannedArtifact::Dicom(planned));
    }
    if seen_recipe_artifacts.len() != dicom.artifacts.len() || !output_bindings.is_empty() {
        return Err(CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: "artifact_set".into(),
        });
    }
    dependencies.extend(output.dependencies);
    Ok(())
}

fn classic_requests(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Vec<ClassicInstanceRequest>, CuratedPlanError> {
    let mut matched = Vec::new();
    macro_rules! try_provider {
        ($provider:expr) => {
            if let Some(requests) = $provider.map_err(|error| CuratedPlanError::ClassicPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            })? {
                matched.push(requests);
            }
        };
    }
    try_provider!(plan_ct_recipe(recipe, standards_lock_sha256, seed));
    try_provider!(plan_dx_mg_recipe(recipe, standards_lock_sha256, seed));
    try_provider!(plan_mr_cr_recipe(recipe, standards_lock_sha256, seed));
    try_provider!(plan_nuclear_recipe(recipe, standards_lock_sha256, seed));
    try_provider!(plan_vl_projection_recipe(
        recipe,
        standards_lock_sha256,
        seed
    ));
    if matched.len() != 1 {
        return Err(CuratedPlanError::ClassicProviderCardinality {
            recipe_id: recipe.recipe_id.clone(),
            matches: matched.len(),
        });
    }
    Ok(matched.pop().expect("one classic provider matched"))
}

fn generation_evidence_plan() -> EvidencePlan {
    EvidencePlan {
        obligations: vec![EvidenceObligation {
            obligation_id: "curated_generation_validation".into(),
            route_id: "shared_corpus_executor".into(),
            independence: EvidenceIndependence::SameProject,
            required: true,
            parameters: BTreeMap::new(),
        }],
    }
}

fn native_content_request(
    artifact_id: &str,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    request: NativePixelRequest,
    native: &crate::native_pixel::NativePixelContent,
) -> NativeContentServiceRequest {
    NativeContentServiceRequest {
        artifact_id: artifact_id.into(),
        slot: crate::recipes::CLASSIC_PIXEL_SLOT.into(),
        factory_id: artifact_recipe.content.provider_id.clone(),
        request,
        unpadded_size_bytes: native.plan.unpadded_value_bytes,
        unpadded_sha256: native.unpadded_sha256.clone(),
        frame_sha256: native
            .frames
            .iter()
            .map(|frame| frame.decoded_sha256.clone())
            .collect(),
    }
}

fn execution_binding(
    artifact_id: &str,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    native: &crate::native_pixel::NativePixelContent,
) -> Result<ArtifactExecutionBindings, CuratedPlanError> {
    if artifact_recipe.encoding.transfer_syntax_uid == RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
        Ok(rle_execution_binding(artifact_id, native))
    } else {
        Ok(ArtifactExecutionBindings {
            artifact_id: artifact_id.into(),
            slots: BTreeMap::from([(
                crate::recipes::CLASSIC_PIXEL_SLOT.into(),
                SlotExecutionBinding::NativeFrames {
                    frames: native_frame_bindings(native)?,
                },
            )]),
        })
    }
}

fn patch_native_content(
    instance: &mut crate::composition::ResolvedInstancePlan,
    native: &crate::native_pixel::NativePixelContent,
) -> Result<(), CuratedPlanError> {
    let content = instance
        .content
        .iter_mut()
        .find(|content| content.slot == crate::recipes::CLASSIC_PIXEL_SLOT)
        .ok_or_else(|| CuratedPlanError::MissingPixelContent(instance.instance_id.clone()))?;
    content.size_bytes = native.unpadded_bytes.len() as u64;
    content.sha256 = native.unpadded_sha256.clone();
    content.materialization = Some(ContentMaterialization::Inline(
        native.unpadded_bytes.clone(),
    ));
    Ok(())
}

fn rle_execution_binding(
    artifact_id: &str,
    native: &crate::native_pixel::NativePixelContent,
) -> ArtifactExecutionBindings {
    let shape = &native.plan.shape;
    let frames = native
        .frames
        .iter()
        .map(|frame| NativeFrameBinding {
            frame_number: frame.frame_number,
            bytes: ByteBinding::Inline {
                bytes: frame.decoded_bytes.clone(),
                sha256: frame.decoded_sha256.clone(),
            },
            rows: shape.rows,
            columns: shape.columns,
            samples_per_pixel: shape.samples_per_pixel,
            bits_allocated: shape.bits_allocated,
            photometric_interpretation: photometric_name(shape.photometric_interpretation).into(),
        })
        .collect::<Vec<_>>();
    ArtifactExecutionBindings {
        artifact_id: artifact_id.into(),
        slots: BTreeMap::from([(
            "pixels".into(),
            SlotExecutionBinding::CodecRequest {
                request: CodecRequest {
                    request_id: format!("codec:{artifact_id}:pixels"),
                    artifact_id: artifact_id.into(),
                    slot: "pixels".into(),
                    backend_id: NativeRleLosslessEncoder::BACKEND_ID.into(),
                    source_transfer_syntax_uid: EXPLICIT_VR_LE.into(),
                    target_transfer_syntax_uid: RLE_LOSSLESS_TRANSFER_SYNTAX_UID.into(),
                    frames,
                    parameters: BTreeMap::from([(
                        "bits_stored".into(),
                        Value::from(shape.bits_stored),
                    )]),
                },
            },
        )]),
    }
}

fn native_frame_bindings(
    native: &crate::native_pixel::NativePixelContent,
) -> Result<Vec<NativeFrameBinding>, CuratedPlanError> {
    let shape = &native.plan.shape;
    let mut previous_end = 0usize;
    native
        .plan
        .frame_spans
        .iter()
        .map(|span| {
            let end_bit = span
                .bit_offset
                .checked_add(span.bit_length)
                .ok_or(CuratedPlanError::ResourceOverflow)?;
            let end = usize::try_from(
                end_bit
                    .checked_add(7)
                    .ok_or(CuratedPlanError::ResourceOverflow)?
                    / 8,
            )
            .map_err(|_| CuratedPlanError::ResourceOverflow)?;
            let bytes = native
                .unpadded_bytes
                .get(previous_end..end)
                .ok_or(CuratedPlanError::ResourceOverflow)?
                .to_vec();
            previous_end = end;
            Ok(NativeFrameBinding {
                frame_number: span.frame_number,
                bytes: ByteBinding::Inline {
                    sha256: sha256_hex(&bytes),
                    bytes,
                },
                rows: shape.rows,
                columns: shape.columns,
                samples_per_pixel: shape.samples_per_pixel,
                bits_allocated: shape.bits_allocated,
                photometric_interpretation: photometric_name(shape.photometric_interpretation)
                    .into(),
            })
        })
        .collect()
}

fn photometric_name(value: crate::native_pixel::PhotometricInterpretation) -> &'static str {
    match value {
        crate::native_pixel::PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        crate::native_pixel::PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        crate::native_pixel::PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        crate::native_pixel::PhotometricInterpretation::Rgb => "RGB",
        crate::native_pixel::PhotometricInterpretation::YbrFull => "YBR_FULL",
        crate::native_pixel::PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    }
}

fn validation_plan(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
) -> ValidationPlan {
    let mut seen = BTreeSet::new();
    let rules = recipe
        .validation_rule_ids
        .iter()
        .chain(&artifact.validation_rule_ids)
        .filter(|rule| seen.insert((*rule).clone()))
        .map(|rule_id| ValidationRule {
            rule_id: rule_id.clone(),
            requirement: ValidationRequirement::Required,
            parameters: BTreeMap::new(),
        })
        .collect();
    ValidationPlan { rules }
}

fn resource_estimate(
    instance: &crate::composition::ResolvedInstancePlan,
    native_bytes: u64,
) -> Result<ArtifactResourceEstimate, CuratedPlanError> {
    let structural_bytes = u64::try_from(
        serde_json::to_vec(instance)
            .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?
            .len(),
    )
    .map_err(|_| CuratedPlanError::ResourceOverflow)?;
    let output_bytes = structural_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(ARTIFACT_OVERHEAD_BYTES))
        .ok_or(CuratedPlanError::ResourceOverflow)?;
    let peak_working_bytes = output_bytes
        .checked_add(
            native_bytes
                .checked_mul(4)
                .ok_or(CuratedPlanError::ResourceOverflow)?,
        )
        .ok_or(CuratedPlanError::ResourceOverflow)?;
    Ok(ArtifactResourceEstimate {
        output_bytes,
        peak_working_bytes,
    })
}

fn aggregate_resources(artifacts: &[PlannedArtifact]) -> Result<(u64, u64), CuratedPlanError> {
    let mut total = 0_u64;
    let mut peak = 0_u64;
    for artifact in artifacts {
        total = total
            .checked_add(artifact.resource_estimate().output_bytes)
            .ok_or(CuratedPlanError::ResourceOverflow)?;
        peak = peak.max(artifact.resource_estimate().peak_working_bytes);
    }
    Ok((total, peak))
}

fn dependencies(
    recipes: &[&CaseRecipe],
    artifact_by_recipe_role: &BTreeMap<(RecipeIdentity, String), String>,
) -> Result<Vec<ArtifactDependency>, CuratedPlanError> {
    let mut dependencies = Vec::new();
    for recipe in recipes {
        let Some(dicom) = &recipe.dicom else {
            continue;
        };
        for dependency in &recipe.dependencies {
            let depends_on = artifact_by_recipe_role
                .get(&(dependency.recipe.identity(), dependency.role.clone()))
                .ok_or_else(|| CuratedPlanError::MissingDependency {
                    recipe: recipe.identity(),
                    dependency: dependency.recipe.identity(),
                    role: dependency.role.clone(),
                })?;
            for artifact in &dicom.artifacts {
                dependencies.push(ArtifactDependency {
                    artifact_id: artifact_id(recipe, &artifact.logical_id),
                    depends_on: depends_on.clone(),
                    relationship: "recipe_dependency".into(),
                    frame_numbers: Vec::new(),
                });
            }
        }
    }
    Ok(dependencies)
}

fn artifact_id(recipe: &CaseRecipe, logical_id: &str) -> String {
    format!("curated_{}_{logical_id}", recipe.recipe_id)
}

fn projection_artifact_id(recipe: &CaseRecipe, logical_id: &str) -> String {
    if matches!(
        recipe.plan_provider_id.as_str(),
        ENHANCED_PLAN_PROVIDER | WSI_ADVANCED_PROVIDER_ID
    ) {
        logical_id.to_owned()
    } else {
        artifact_id(recipe, logical_id)
    }
}

fn selected_case_ids(
    registry: &RegistryDocument,
    selection: &CuratedScSelection,
) -> Result<BTreeSet<String>, CuratedPlanError> {
    match selection {
        CuratedScSelection::AllFeatureFree => Ok(registry
            .cases
            .iter()
            .filter(|case| case.status == "implemented" && case.requirements.is_feature_free())
            .map(|case| case.case_id.clone())
            .collect()),
        CuratedScSelection::Profile {
            profile,
            include_stress,
        } => {
            if !matches!(
                profile.as_str(),
                "smoke" | "core" | "extended" | "legacy" | "stress" | "all"
            ) {
                return Err(CuratedPlanError::UnknownProfile(profile.clone()));
            }
            Ok(registry
                .cases
                .iter()
                .filter(|case| {
                    case.status == "implemented"
                        && case.requirements.is_feature_free()
                        && matches_profile(&case.profiles, profile, *include_stress)
                })
                .map(|case| case.case_id.clone())
                .collect())
        }
        CuratedScSelection::CaseIds(case_ids) => {
            let requested = case_ids.iter().cloned().collect::<BTreeSet<_>>();
            if requested.len() != case_ids.len() {
                return Err(CuratedPlanError::DuplicateCaseSelection);
            }
            let known = registry
                .cases
                .iter()
                .map(|case| case.case_id.clone())
                .collect::<BTreeSet<_>>();
            if let Some(case_id) = requested.iter().find(|case_id| !known.contains(*case_id)) {
                return Err(CuratedPlanError::UnknownCase(case_id.clone()));
            }
            Ok(requested)
        }
    }
}

fn expand_recipe_dependency_closure(
    catalog: &RecipeCatalog,
    selected: &mut BTreeSet<String>,
) -> Result<(), CuratedPlanError> {
    loop {
        let before = selected.len();
        let selected_snapshot = selected.iter().cloned().collect::<Vec<_>>();
        for case_id in selected_snapshot {
            let identity = catalog
                .binding_for_case(&case_id)
                .ok_or_else(|| CuratedPlanError::MissingRecipe(case_id.clone()))?;
            let recipe = &catalog.recipes()[identity];
            for dependency in &recipe.dependencies {
                let dependency_recipe = catalog
                    .recipes()
                    .get(&dependency.recipe.identity())
                    .ok_or_else(|| CuratedPlanError::MissingDependency {
                        recipe: recipe.identity(),
                        dependency: dependency.recipe.identity(),
                        role: dependency.role.clone(),
                    })?;
                selected.insert(dependency_recipe.binding.case_id.clone());
            }
        }
        if selected.len() == before {
            return Ok(());
        }
    }
}

fn matches_profile(profiles: &[String], requested: &str, include_stress: bool) -> bool {
    if requested == "all" {
        profiles.iter().any(|profile| {
            matches!(profile.as_str(), "smoke" | "core" | "extended")
                || (include_stress && profile == "stress")
        })
    } else {
        profiles.iter().any(|profile| profile == requested)
    }
}

fn read(path: &Path) -> Result<Vec<u8>, CuratedPlanError> {
    fs::read(path).map_err(|error| CuratedPlanError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[derive(Debug, Deserialize)]
struct RegistryDocument {
    cases: Vec<RegistryCase>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RegistryCase {
    case_id: String,
    status: String,
    profiles: Vec<String>,
    recipe_id: String,
    recipe_version: String,
    iod_name: Option<String>,
    sop_class_name: Option<String>,
    sop_class_uid: Option<String>,
    transfer_syntax_uid: Option<String>,
    determinism: String,
    requirements: RegistryRequirements,
    skip: Value,
    standards_evidence: Vec<Value>,
    provider: Value,
    roadmap: Value,
    blockers: Vec<Value>,
    modality: Option<String>,
    object_family: String,
    compatibility_axes: Vec<String>,
    artifact_kind: String,
}

impl From<RegistryCase> for RegistryCaseProjection {
    fn from(case: RegistryCase) -> Self {
        Self {
            case_id: case.case_id,
            status: case.status,
            profiles: case.profiles,
            recipe_id: case.recipe_id,
            recipe_version: case.recipe_version,
            iod_name: case.iod_name,
            sop_class_name: case.sop_class_name,
            sop_class_uid: case.sop_class_uid,
            transfer_syntax_uid: case.transfer_syntax_uid,
            determinism: case.determinism,
            requirements: case.requirements,
            skip: case.skip,
            standards_evidence: case.standards_evidence,
            provider: case.provider,
            roadmap: case.roadmap,
            blockers: case.blockers,
            modality: case.modality,
            object_family: case.object_family,
            compatibility_axes: case.compatibility_axes,
            artifact_kind: case.artifact_kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryRequirements {
    pub features: Vec<String>,
    pub external_codecs: Vec<String>,
    pub external_validators: Vec<String>,
}

impl RegistryRequirements {
    fn is_feature_free(&self) -> bool {
        self.features.is_empty()
            && self.external_codecs.is_empty()
            && self.external_validators.is_empty()
    }
}

#[derive(Debug)]
pub enum CuratedPlanError {
    Read {
        path: PathBuf,
        message: String,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    Catalog(String),
    UnknownProfile(String),
    UnknownCase(String),
    DuplicateCaseSelection,
    UnsupportedCase {
        case_id: String,
        provider_id: String,
    },
    MissingRecipe(String),
    MissingDicom(String),
    MissingTemplate(String),
    InvalidTemplateVersion(String),
    MissingSecondaryCapture(String),
    MissingPixelContent(String),
    MissingImplementation(String),
    ProviderDerivedOutput(String),
    ScPlan {
        artifact_id: String,
        message: String,
    },
    ClassicPlan {
        recipe_id: String,
        message: String,
    },
    ClassicProviderCardinality {
        recipe_id: String,
        matches: usize,
    },
    ClassicArtifactMismatch {
        recipe_id: String,
        artifact_id: String,
    },
    MissingAdvancedInput {
        case_id: String,
        provider_id: String,
    },
    AdvancedPlan {
        recipe_id: String,
        message: String,
    },
    AdvancedArtifactMismatch {
        recipe_id: String,
        artifact_id: String,
    },
    AdvancedProvenance(String),
    Encoding {
        artifact_id: String,
        message: String,
    },
    MissingDependency {
        recipe: RecipeIdentity,
        dependency: RecipeIdentity,
        role: String,
    },
    MissingProjectionOrder(String),
    ProjectionArtifactSetMismatch,
    ProjectionArtifactMismatch(String),
    ZeroParallelism,
    ResourceOverflow,
    CorpusPlan(CorpusPlanError),
}

impl fmt::Display for CuratedPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for CuratedPlanError {}

impl From<CorpusPlanError> for CuratedPlanError {
    fn from(value: CorpusPlanError) -> Self {
        Self::CorpusPlan(value)
    }
}
