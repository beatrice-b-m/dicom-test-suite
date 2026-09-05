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

use dicom_core::Tag;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::codecs::{NativeRleLosslessEncoder, RLE_LOSSLESS_TRANSFER_SYNTAX_UID};
use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    CompositionUidRole, ContentMaterialization, ContentPlacement, DicomVr, IdentityPlan,
    MaterializedReference, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
    SequenceItemPlacement, TemplateCatalog, TemplateDescriptor, TemplateId, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION,
    CapabilityKind, CaseBinding, CorpusPlan, CorpusPlanError, EncodingPlan, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImportedDicomProviderPlan, ItemLengthPolicy, OffsetTablePolicy, OutputPlan, OutputRelativePath,
    PlannedArtifact, PlannedDicomArtifact, PlannedImportedDicomArtifact, PreamblePolicy,
    PublicationPlan, PublicationTransaction, ResourcePlan, SequenceLengthPolicy,
    UnavailableCapability, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, CodecRequest, NativeFrameBinding,
    ProviderOutputExpectation, ProviderRequest, SlotExecutionBinding, StagedAssetHandle,
};
use crate::executor::stress_content::{
    STRESS_CONTENT_PROVIDER_ID, STRESS_CONTENT_PROVIDER_VERSION, StressPayloadRequest,
    stress_payload_identity,
};
use crate::native_pixel::{ByteOrder, NativePixelFactory, NativePixelRequest};
use crate::negative_plan::{NegativePlanProvider, NegativePlanProviderRequest};
use crate::planning::RecipeIdentity;
use crate::planning_preview::{PlanningPreviewLimits, preview_planned_dicom};
use crate::qualification_plan::{
    PreparedQualificationSource, QualificationPlanRequest, plan_qualification,
};
use crate::recipes::classic_ct::plan_ct_recipe;
use crate::recipes::classic_dx_mg::plan_dx_mg_recipe;
use crate::recipes::classic_mr_cr::plan_mr_cr_recipe;
use crate::recipes::classic_nuclear::plan_nuclear_recipe;
use crate::recipes::classic_vl_projection::plan_vl_projection_recipe;
use crate::recipes::{
    AdvancedArtifactPlanningContext, AdvancedArtifactProvenance, AdvancedPlanProvider,
    AdvancedPlanProviderOutput, AdvancedPlanProviderRequest, AdvancedProviderFamily,
    AdvancedProviderLimits, AdvancedSourceRole, CaseRecipe, ClassicInstanceRequest,
    ClassicResolvedPlanInput, ContentProviderLimits, ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID,
    EOT_ARITHMETIC_PLAN_PROVIDER_ID, EXCEPTIONAL_SC_PLAN_PROVIDER_ID, EncapsulatedPayload,
    EncapsulatedPayloadPlanProvider, EnhancedPlanProvider, ExceptionalScEncodingRequest,
    ExceptionalScPlanInput, FUZZ_PLAN_PROVIDER_ID, HIGH_DICOM_SR_IMPORT_PROVIDER_ID,
    LockedFullFileCodecRequest, MetadataScPlanInput, OrderedSeriesProvider,
    PRESENTATION_ADVANCED_PROVIDER_ID, PresentationPlanProvider, PresentationSourceInput,
    QUANTITATIVE_NATIVE_PROVIDER_ID, QualificationParameters, QuantitativeArtifactContext,
    QuantitativePlanInput, QuantitativePlanOutput, QuantitativePlanProvider,
    QuantitativeProviderLimits, QuantitativeSourceInput, QuantitativeSourceRole,
    REGISTRATION_PLAN_PROVIDER_ID, RT_PLAN_PROVIDER_ID, RecipeCatalog, RecipeReference,
    RegistrationPlanProvider, RegistrationSourceInput, RtPlanProvider, SR_PLAN_PROVIDER_ID,
    STRESS_CT_PLAN_PROVIDER_ID, STRESS_SC_PLAN_PROVIDER_ID, SecondaryCapturePlanInput,
    SemanticPlanContext, SemanticSource, SrPlanProvider, TypedBulkPlanningContext,
    WAVEFORM_PLAN_PROVIDER_ID, WSI_ADVANCED_PROVIDER_ID, WaveformPlanProvider,
    WsiAdvancedPlanProvider, encapsulated_payload_input_from_recipe, encoding_plan_from_recipe,
    native_pixel_request_from_recipe, plan_exceptional_sc, plan_stress_ct_recipe,
    plan_stress_sc_recipe, qualification_parameters, resolved_classic_instance_plan,
    resolved_metadata_sc_plan, resolved_secondary_capture_plan, rt_input_from_recipe,
    sr_input_from_recipe, waveform_input_from_recipe,
};

/// Hard publication allowance for the curated manifest. The maximal reduced
/// stress profile projects a large, schema-bounded fragment ledger, so the
/// run envelope must budget the manifest in addition to DICOM artifacts.
pub const MAX_CURATED_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
use crate::runtime_capabilities::{
    CapabilityEvaluationRequest, CapabilityInventory, CapabilityKind as RuntimeCapabilityKind,
    RegistryRuntimeRequirements, RuntimeCapabilityEvaluator,
    UnavailableCapability as RuntimeUnavailableCapability, UnavailableReason,
};
use crate::sha256_hex;
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};

#[cfg(test)]
#[path = "../tests/captured_curated_plan.rs"]
mod captured_input_tests;

const SC_PLAN_PROVIDER: &str = "native.sc_plan";
const METADATA_SC_PLAN_PROVIDER: &str = "native.metadata_sc_plan";
const CLASSIC_PLAN_PROVIDER: &str = "native.classic_plan";
const ENHANCED_PLAN_PROVIDER: &str = "native.enhanced_plan";
const NEGATIVE_PLAN_PROVIDER: &str = "mutation.named_plan";
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

#[derive(Debug, Clone, Deserialize)]
struct QuantitativeSourceDeclaration {
    recipe: RecipeReference,
    artifact_logical_id: String,
    role: QuantitativeSourceRole,
    #[serde(default)]
    referenced_frames: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct QuantitativeProviderParameters {
    sources: Vec<QuantitativeSourceDeclaration>,
}

#[derive(Debug, Clone, Deserialize)]
struct SemanticSourceDeclaration {
    recipe: RecipeReference,
    artifact_logical_id: String,
    role: String,
    #[serde(default)]
    referenced_frames: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct SemanticProviderParameters {
    sources: Vec<SemanticSourceDeclaration>,
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
    pub standards_evidence: Vec<Value>,
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
    /// Exact caller-supplied capability assertions used to qualify this plan.
    /// Execution adapters reconcile external command identities against this
    /// immutable snapshot before invoking a backend.
    #[serde(skip, default = "CapabilityInventory::compiled")]
    pub capability_inventory: CapabilityInventory,
    #[serde(skip, default)]
    pub locked_full_file_requests: BTreeMap<String, LockedFullFileCodecRequest>,
    #[serde(skip, default)]
    pub external_provider_repository_root: PathBuf,
    #[serde(skip, default)]
    pub external_provider_standards_lock_path: PathBuf,
    #[serde(skip, default)]
    pub external_provider_standards_lock_sha256: String,
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
        let projected_plan_artifacts = plan
            .artifacts
            .iter()
            .filter(|artifact| !matches!(artifact, PlannedArtifact::Qualification(_)))
            .collect::<Vec<_>>();
        if self.artifacts.len() != projected_plan_artifacts.len() {
            return Err(CuratedPlanError::ProjectionArtifactSetMismatch);
        }
        let mut ids = BTreeSet::new();
        for (planned, projected) in projected_plan_artifacts.into_iter().zip(&self.artifacts) {
            let mutation_projection =
                projected.case_recipe.plan_provider_id == NEGATIVE_PLAN_PROVIDER;
            let private_source_projection = matches!(
                planned.provenance(),
                ArtifactProvenance::PrivateSource { .. }
            );
            if !ids.insert(projected.artifact_id.clone())
                || planned.logical_id() != projected.artifact_id
                || planned.order() != projected.plan_order
                || projected.registry_case.case_id != projected.case_recipe.binding.case_id
                || projected.registry_case.recipe_id != projected.case_recipe.recipe_id
                || projected.registry_case.recipe_version != projected.case_recipe.recipe_version
                || (!mutation_projection
                    && !private_source_projection
                    && projection_artifact_id(
                        &projected.case_recipe,
                        &projected.artifact_recipe.logical_id,
                    ) != projected.artifact_id)
                || (!mutation_projection
                    && !private_source_projection
                    && projected.artifact_recipe.order != projected.historical_artifact_order)
                || projected
                    .case_recipe
                    .planning_order
                    .is_some_and(|order| order != projected.historical_recipe_order)
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
    capability_inventory: CapabilityInventory,
    repository_root: PathBuf,
    standards_lock_path: PathBuf,
    installed_codec_matrix: Option<String>,
}

/// Internal bridge for the later batch runner. No caller filesystem paths are
/// retained: corpus inputs are captured, engine service paths own a private lease.
#[allow(dead_code)]
pub(crate) struct CapturedCuratedPlanningContext {
    provider: CuratedScCorpusPlanProvider,
    engine_lease: crate::engine_resources::EngineResourceSnapshot,
    corpus_identity: crate::corpus_definition::CorpusDefinitionIdentity,
}

#[allow(dead_code)]
pub(crate) struct CapturedCuratedPlan {
    pub(crate) planned: CuratedScCorpusPlan,
    pub(crate) corpus_identity: crate::corpus_definition::CorpusDefinitionIdentity,
    engine_lease: crate::engine_resources::EngineResourceSnapshot,
}

#[allow(dead_code)]
impl CapturedCuratedPlanningContext {
    pub(crate) fn from_verified_bundle(
        bundle: &crate::corpus_definition::CorpusDefinitionBundle,
        resources: &crate::engine_resources::EngineResources,
    ) -> Result<Self, CuratedPlanError> {
        let engine_lease = resources
            .shared_snapshot()
            .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
        let root = engine_lease.root();
        let path = &bundle.manifest().registry.path;
        let registry =
            serde_json::from_slice(bundle.bytes(path).expect("verified registry capture"))
                .map_err(|error| CuratedPlanError::Parse {
                    path: path.into(),
                    message: error.to_string(),
                })?;
        let recipes = RecipeCatalog::from_verified_bundle(bundle, root)
            .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
        let templates = TemplateCatalog::load(root.join("templates/catalog.json"))
            .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
        let standards_lock_path = root.join("standards.lock.json");
        let standards_lock_sha256 = sha256_hex(
            &resources
                .bytes("standards.lock.json")
                .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?,
        );
        let installed_codec_matrix = resources
            .text("transfer-syntax/capability-matrix.json")
            .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?
            .into_owned();
        Ok(Self {
            provider: CuratedScCorpusPlanProvider {
                registry,
                recipes,
                templates,
                standards_lock_sha256,
                capability_inventory: CapabilityInventory::compiled(),
                repository_root: root.to_path_buf(),
                standards_lock_path,
                installed_codec_matrix: Some(installed_codec_matrix),
            },
            engine_lease,
            corpus_identity: bundle.identity().clone(),
        })
    }

    pub(crate) fn plan(
        &self,
        request: CuratedScPlanRequest,
    ) -> Result<CapturedCuratedPlan, CuratedPlanError> {
        Ok(CapturedCuratedPlan {
            planned: self.provider.plan(&request)?,
            corpus_identity: self.corpus_identity.clone(),
            engine_lease: self.engine_lease.clone(),
        })
    }
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
        let repository_root = paths
            .registry_path
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Ok(Self {
            registry,
            recipes,
            templates,
            standards_lock_sha256,
            capability_inventory: CapabilityInventory::compiled(),
            repository_root,
            standards_lock_path: paths.standards_lock_path,
            installed_codec_matrix: None,
        })
    }

    /// Replace the default compile-time-only inventory with capabilities
    /// qualified by the caller. Planning never discovers tools, validators,
    /// providers, or executable backends on its own.
    pub fn with_capability_inventory(mut self, inventory: CapabilityInventory) -> Self {
        self.capability_inventory = inventory;
        self
    }

    pub fn plan(
        &self,
        request: &CuratedScPlanRequest,
    ) -> Result<CuratedScCorpusPlan, CuratedPlanError> {
        if request.max_parallelism == 0 {
            return Err(CuratedPlanError::ZeroParallelism);
        }
        let directly_selected_ids = selected_case_ids(&self.registry, &request.selection)?;
        let mut selected_ids = directly_selected_ids.clone();
        expand_recipe_dependency_closure(&self.recipes, &mut selected_ids)?;
        let mut required_mutation_sources: BTreeMap<RecipeIdentity, BTreeSet<String>> =
            BTreeMap::new();
        let mut required_qualification_sources: BTreeMap<RecipeIdentity, BTreeSet<String>> =
            BTreeMap::new();
        let mut required_other_dependencies = BTreeSet::new();
        for case_id in &directly_selected_ids {
            let Some(identity) = self.recipes.binding_for_case(case_id) else {
                continue;
            };
            let recipe = &self.recipes.recipes()[identity];
            if let Some(mutation) = &recipe.mutation {
                required_mutation_sources
                    .entry(mutation.source.identity())
                    .or_default()
                    .insert(mutation.source_logical_role.clone());
            }
            if recipe.plan_provider_id == FUZZ_PLAN_PROVIDER_ID {
                let parameters = qualification_parameters(recipe).map_err(|error| {
                    CuratedPlanError::QualificationPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    }
                })?;
                let QualificationParameters::BoundedDeterministicFuzz { sources, .. } = parameters
                else {
                    return Err(CuratedPlanError::QualificationPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: "fuzz provider has a non-fuzz qualification contract".into(),
                    });
                };
                for source in sources {
                    required_qualification_sources
                        .entry(source.recipe.identity())
                        .or_default()
                        .insert(source.artifact_logical_id);
                }
            } else {
                required_other_dependencies.extend(
                    recipe
                        .dependencies
                        .iter()
                        .map(|dependency| dependency.recipe.identity()),
                );
            }
        }
        let mut artifacts: Vec<PlannedArtifact> = Vec::new();
        let mut bindings = BTreeMap::new();
        let mut native_content_requests = Vec::new();
        let mut projection_artifacts: Vec<CuratedArtifactProjectionContext> = Vec::new();
        let mut pending = Vec::new();
        let mut unavailable = Vec::new();
        let mut artifact_by_recipe_role = BTreeMap::new();
        let mut source_artifacts: BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact> =
            BTreeMap::new();
        let mut selected_recipes = Vec::new();
        let mut classic_dependencies = Vec::new();
        let mut advanced_dependencies = Vec::new();
        let mut locked_full_file_requests = BTreeMap::new();
        let capability_evaluator = match &self.installed_codec_matrix {
            Some(matrix) => RuntimeCapabilityEvaluator::from_installed_matrix(matrix),
            None => RuntimeCapabilityEvaluator::committed(),
        }
        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;

        let registry_order = self
            .registry
            .cases
            .iter()
            .enumerate()
            .map(|(order, case)| (case.case_id.as_str(), order as u64))
            .collect::<BTreeMap<_, _>>();
        let mut selected_registry_cases = self.registry.cases.iter().collect::<Vec<_>>();
        selected_registry_cases.sort_by_key(|registry_case| {
            let recipe = self
                .recipes
                .binding_for_case(&registry_case.case_id)
                .and_then(|identity| self.recipes.recipes().get(identity));
            (
                if self.capability_inventory.external_providers.is_empty() {
                    0
                } else {
                    recipe
                        .map(|recipe| recipe_dependency_depth(recipe, &self.recipes))
                        .unwrap_or(u32::MAX)
                },
                recipe
                    .and_then(|recipe| recipe.planning_order)
                    .unwrap_or(u32::MAX),
            )
        });

        for registry_case in selected_registry_cases {
            if !selected_ids.contains(&registry_case.case_id)
                || registry_case.status != "implemented"
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
            if !directly_selected_ids.contains(&registry_case.case_id)
                && required_qualification_sources.contains_key(&recipe.identity())
                && !required_mutation_sources.contains_key(&recipe.identity())
                && !required_other_dependencies.contains(&recipe.identity())
            {
                // Fuzz sources are replanned below with their recipe-locked
                // source-generation seed. They must never leak into the
                // public corpus merely because dependency closure selected
                // their owning recipes.
                continue;
            }
            if !registry_case.requirements.is_feature_free() {
                let runtime_requirements = RegistryRuntimeRequirements {
                    features: registry_case.requirements.features.clone(),
                    external_codecs: registry_case.requirements.external_codecs.clone(),
                    external_validators: registry_case.requirements.external_validators.clone(),
                    external_providers: Vec::new(),
                };
                let evaluation = capability_evaluator.evaluate(
                    CapabilityEvaluationRequest {
                        transfer_syntax_uid: registry_case
                            .transfer_syntax_uid
                            .as_deref()
                            .unwrap_or_default(),
                        determinism: &registry_case.determinism,
                        requirements: &runtime_requirements,
                    },
                    &self.capability_inventory,
                );
                if !evaluation.available {
                    record_runtime_unavailable_case(
                        registry_case,
                        recipe,
                        &evaluation.unavailable,
                        &mut unavailable,
                        &mut pending,
                    );
                    continue;
                }
            }
            let external_provider = matches!(
                recipe.plan_provider_id.as_str(),
                HIGH_DICOM_SR_IMPORT_PROVIDER_ID
                    | crate::recipes::QUANTITATIVE_EXTERNAL_PROVIDER_ID
            )
            .then(|| {
                registry_case
                    .provider
                    .get("kind")
                    .and_then(Value::as_str)
                    .filter(|kind| *kind == "external_backend")
                    .and_then(|_| registry_case.provider.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .flatten();
            if let Some(provider_id) = external_provider.as_deref() {
                if !self
                    .capability_inventory
                    .external_providers
                    .contains(provider_id)
                {
                    record_unavailable_case(
                        registry_case,
                        recipe,
                        CapabilityKind::ExternalBackend,
                        "external_backend_unavailable",
                        format!(
                            "external plan provider {} ({provider_id}) is not caller-qualified",
                            recipe.plan_provider_id
                        ),
                        &mut unavailable,
                        &mut pending,
                    );
                    continue;
                }
            }
            if recipe.plan_provider_id == FUZZ_PLAN_PROVIDER_ID {
                let parameters = qualification_parameters(recipe).map_err(|error| {
                    CuratedPlanError::QualificationPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    }
                })?;
                let QualificationParameters::BoundedDeterministicFuzz {
                    source_generation_seed,
                    ref sources,
                    ..
                } = parameters
                else {
                    return Err(CuratedPlanError::QualificationPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: "fuzz provider has a non-fuzz qualification contract".into(),
                    });
                };
                let logical_id = artifact_id(recipe, "qualification");
                let first_source_order = u64::try_from(artifacts.len())
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                let mut prepared_sources = Vec::with_capacity(sources.len());
                let mut private_projections = Vec::with_capacity(sources.len());
                let mut private_native_requests = Vec::new();

                for (index, source_contract) in sources.iter().enumerate() {
                    let source_recipe = self
                        .recipes
                        .recipes()
                        .get(&source_contract.recipe.identity())
                        .ok_or_else(|| CuratedPlanError::MissingQualificationSource {
                            recipe_id: recipe.recipe_id.clone(),
                            source: source_contract.recipe.identity(),
                            role: source_contract.artifact_logical_id.clone(),
                        })?;
                    let source_bundle = self.plan(&CuratedScPlanRequest {
                        selection: CuratedScSelection::CaseIds(vec![
                            source_recipe.binding.case_id.clone(),
                        ]),
                        seed: source_generation_seed,
                        max_parallelism: request.max_parallelism,
                    })?;
                    let source_global_id =
                        artifact_id(source_recipe, &source_contract.artifact_logical_id);
                    let mut source = source_bundle
                        .plan
                        .artifacts
                        .iter()
                        .find_map(|artifact| match artifact {
                            PlannedArtifact::Dicom(source)
                                if source.logical_id == source_global_id =>
                            {
                                Some(source.clone())
                            }
                            _ => None,
                        })
                        .ok_or_else(|| CuratedPlanError::MissingQualificationSource {
                            recipe_id: recipe.recipe_id.clone(),
                            source: source_contract.recipe.identity(),
                            role: source_contract.artifact_logical_id.clone(),
                        })?;
                    let private_source_id = format!("{logical_id}__source_{index:02}");
                    let source_order = first_source_order
                        .checked_add(
                            u64::try_from(index).map_err(|_| CuratedPlanError::ResourceOverflow)?,
                        )
                        .ok_or(CuratedPlanError::ResourceOverflow)?;
                    source.logical_id = private_source_id.clone();
                    source.instance.instance_id = private_source_id.clone();
                    source.order = source_order;
                    source.provenance = ArtifactProvenance::PrivateSource {
                        consumed_by: vec![logical_id.clone()],
                    };
                    source.output.relative_path = OutputRelativePath::new(format!(
                        "private-sources/{private_source_id}.dcm"
                    ))?;
                    source.output.publish = false;

                    let source_bindings = source_bundle
                        .bindings
                        .get(&source_global_id)
                        .cloned()
                        .ok_or_else(|| CuratedPlanError::MissingQualificationSource {
                            recipe_id: recipe.recipe_id.clone(),
                            source: source_contract.recipe.identity(),
                            role: source_contract.artifact_logical_id.clone(),
                        })?;
                    let source_bindings = retarget_bindings(source_bindings, &private_source_id);
                    let preview = preview_planned_dicom(
                        &source,
                        &source_bindings,
                        PlanningPreviewLimits::default(),
                        &|| false,
                    )
                    .map_err(|error| CuratedPlanError::QualificationPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: format!(
                            "source {} planning preview failed: {error}",
                            source_contract.seed_description_id
                        ),
                    })?;

                    let mut source_projection = source_bundle
                        .projection
                        .artifacts
                        .iter()
                        .find(|projection| projection.artifact_id == source_global_id)
                        .cloned()
                        .ok_or_else(|| CuratedPlanError::MissingQualificationSource {
                            recipe_id: recipe.recipe_id.clone(),
                            source: source_contract.recipe.identity(),
                            role: source_contract.artifact_logical_id.clone(),
                        })?;
                    source_projection.artifact_id = private_source_id.clone();
                    source_projection.plan_order = source_order;
                    source_projection.artifact_recipe.output.path =
                        Some(source.output.relative_path.as_str().to_owned());
                    private_projections.push(source_projection);

                    if let Some(native_request) = source_bundle
                        .native_content_requests
                        .iter()
                        .find(|request| request.artifact_id == source_global_id)
                    {
                        let mut native_request = native_request.clone();
                        native_request.artifact_id = private_source_id.clone();
                        private_native_requests.push(native_request);
                    }
                    prepared_sources.push(PreparedQualificationSource {
                        artifact: source,
                        bindings: source_bindings,
                        dependency_role: source_contract.dependency_role.clone(),
                        recipe_artifact_logical_id: source_contract.artifact_logical_id.clone(),
                        preflight_sha256: preview.sha256,
                        preflight_size_bytes: preview.size_bytes,
                    });
                }

                let qualification_order = first_source_order
                    .checked_add(
                        u64::try_from(prepared_sources.len())
                            .map_err(|_| CuratedPlanError::ResourceOverflow)?,
                    )
                    .ok_or(CuratedPlanError::ResourceOverflow)?;
                let output = plan_qualification(QualificationPlanRequest {
                    recipe,
                    parameters,
                    logical_id: logical_id.clone(),
                    order: qualification_order,
                    run_seed: Some(request.seed),
                    profile: Some("fuzz".into()),
                    sources: prepared_sources,
                })
                .map_err(|error| CuratedPlanError::QualificationPlan {
                    recipe_id: recipe.recipe_id.clone(),
                    message: error.to_string(),
                })?;
                projection_artifacts.extend(private_projections);
                native_content_requests.extend(private_native_requests);
                artifacts.extend(output.artifacts);
                bindings.extend(output.bindings);
                advanced_dependencies.extend(output.dependencies);
                continue;
            }
            if recipe.plan_provider_id == EOT_ARITHMETIC_PLAN_PROVIDER_ID {
                let parameters = qualification_parameters(recipe).map_err(|error| {
                    CuratedPlanError::QualificationPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    }
                })?;
                let logical_id = artifact_id(recipe, "qualification");
                let order = u64::try_from(artifacts.len())
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                let output = plan_qualification(QualificationPlanRequest {
                    recipe,
                    parameters,
                    logical_id,
                    order,
                    run_seed: None,
                    profile: None,
                    sources: Vec::new(),
                })
                .map_err(|error| CuratedPlanError::QualificationPlan {
                    recipe_id: recipe.recipe_id.clone(),
                    message: error.to_string(),
                })?;
                artifacts.extend(output.artifacts);
                bindings.extend(output.bindings);
                advanced_dependencies.extend(output.dependencies);
                continue;
            }
            if recipe.plan_provider_id == NEGATIVE_PLAN_PROVIDER {
                let mutation_recipe = recipe
                    .mutation
                    .clone()
                    .ok_or_else(|| CuratedPlanError::MissingMutation(recipe.recipe_id.clone()))?;
                let source_recipe = self
                    .recipes
                    .recipes()
                    .get(&mutation_recipe.source.identity())
                    .ok_or_else(|| CuratedPlanError::MissingMutationSource {
                        recipe_id: recipe.recipe_id.clone(),
                        source: mutation_recipe.source.identity(),
                    })?;
                let source_matches = source_recipe
                    .dicom
                    .as_ref()
                    .map(|dicom| {
                        dicom
                            .artifacts
                            .iter()
                            .filter(|artifact| {
                                artifact.logical_id == mutation_recipe.source_logical_role
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if source_matches.len() != 1 {
                    return Err(CuratedPlanError::MissingMutationSourceRole {
                        recipe_id: recipe.recipe_id.clone(),
                        role: mutation_recipe.source_logical_role.clone(),
                    });
                }
                let source_artifact_recipe = source_matches[0];
                let source_key = (
                    mutation_recipe.source.identity(),
                    source_artifact_recipe.logical_id.clone(),
                );
                let source = source_artifacts.get(&source_key).cloned().ok_or_else(|| {
                    CuratedPlanError::MissingMutationSourceRole {
                        recipe_id: recipe.recipe_id.clone(),
                        role: mutation_recipe.source_logical_role.clone(),
                    }
                })?;
                let logical_id = artifact_id(recipe, "instance");
                let source_is_explicitly_requested =
                    directly_selected_ids.contains(&source_recipe.binding.case_id);
                let mut planned_source = source.planned.clone();
                let mut planned_source_bindings = source.bindings.clone();
                if source_is_explicitly_requested {
                    let private_source_id = format!("{logical_id}__private_source");
                    planned_source.logical_id = private_source_id.clone();
                    planned_source.instance.instance_id = private_source_id.clone();
                    planned_source.order = u64::try_from(artifacts.len())
                        .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                    planned_source.output.relative_path = OutputRelativePath::new(format!(
                        "private-sources/{private_source_id}.dcm"
                    ))?;
                    planned_source_bindings =
                        retarget_bindings(planned_source_bindings, &private_source_id);
                }
                let order =
                    u64::try_from(artifacts.len() + usize::from(source_is_explicitly_requested))
                        .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                let output_path =
                    mutation_recipe.output.path.clone().ok_or_else(|| {
                        CuratedPlanError::ProviderDerivedOutput(logical_id.clone())
                    })?;
                let provider_output = NegativePlanProvider
                    .plan(NegativePlanProviderRequest {
                        case_binding: CaseBinding {
                            case_id: recipe.binding.case_id.clone(),
                            recipe_id: recipe.recipe_id.clone(),
                            recipe_version: recipe.recipe_version.clone(),
                        },
                        logical_id: logical_id.clone(),
                        order,
                        output: OutputPlan {
                            relative_path: OutputRelativePath::new(output_path.clone())?,
                            role: mutation_recipe.output.role.clone(),
                            publish: true,
                        },
                        mutation_recipe,
                        source: planned_source,
                        source_logical_role: source_artifact_recipe.logical_id.clone(),
                        source_bindings: planned_source_bindings,
                        max_source_bytes: source.planned.resources.output_bytes,
                    })
                    .map_err(|error| CuratedPlanError::NegativePlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    })?;

                let provider_source = provider_output
                    .artifacts
                    .iter()
                    .find_map(|artifact| match artifact {
                        PlannedArtifact::Dicom(source) => Some(source),
                        _ => None,
                    })
                    .ok_or_else(|| CuratedPlanError::MissingMutationSourceRole {
                        recipe_id: recipe.recipe_id.clone(),
                        role: source_artifact_recipe.output.role.clone(),
                    })?;
                if source_is_explicitly_requested {
                    artifacts.push(PlannedArtifact::Dicom(provider_source.clone()));
                    let mut private_projection = projection_artifacts
                        .iter()
                        .find(|projected| projected.artifact_id == source.planned.logical_id)
                        .cloned()
                        .ok_or_else(|| CuratedPlanError::MissingMutationSourceRole {
                            recipe_id: recipe.recipe_id.clone(),
                            role: source_artifact_recipe.logical_id.clone(),
                        })?;
                    private_projection.artifact_id = provider_source.logical_id.clone();
                    private_projection.plan_order = provider_source.order;
                    private_projection.artifact_recipe.output.path =
                        Some(provider_source.output.relative_path.as_str().to_owned());
                    projection_artifacts.push(private_projection);
                    let provider_binding = provider_output
                        .bindings
                        .get(&provider_source.logical_id)
                        .cloned()
                        .ok_or_else(|| CuratedPlanError::MissingMutationSourceRole {
                            recipe_id: recipe.recipe_id.clone(),
                            role: source_artifact_recipe.logical_id.clone(),
                        })?;
                    bindings.insert(provider_source.logical_id.clone(), provider_binding);
                }
                let source_position = artifacts
                    .iter()
                    .position(|artifact| artifact.logical_id() == provider_source.logical_id)
                    .ok_or_else(|| CuratedPlanError::MissingMutationSourceRole {
                        recipe_id: recipe.recipe_id.clone(),
                        role: source_artifact_recipe.output.role.clone(),
                    })?;
                let PlannedArtifact::Dicom(existing_source) = &mut artifacts[source_position]
                else {
                    return Err(CuratedPlanError::MissingMutationSourceRole {
                        recipe_id: recipe.recipe_id.clone(),
                        role: source_artifact_recipe.output.role.clone(),
                    });
                };
                let consumed_by = match &mut existing_source.provenance {
                    ArtifactProvenance::PrivateSource { consumed_by } => consumed_by,
                    _ => {
                        existing_source.provenance = ArtifactProvenance::PrivateSource {
                            consumed_by: Vec::new(),
                        };
                        let ArtifactProvenance::PrivateSource { consumed_by } =
                            &mut existing_source.provenance
                        else {
                            unreachable!()
                        };
                        consumed_by
                    }
                };
                if !consumed_by.contains(&logical_id) {
                    consumed_by.push(logical_id.clone());
                }
                existing_source.output.publish = false;
                existing_source.resources = provider_source.resources.clone();
                if !source_is_explicitly_requested {
                    source_artifacts.insert(
                        source_key,
                        CuratedSourceArtifact {
                            planned: existing_source.clone(),
                            bindings: source.bindings,
                        },
                    );
                }

                let mutation_artifact = provider_output
                    .artifacts
                    .into_iter()
                    .find(|artifact| matches!(artifact, PlannedArtifact::Mutation(_)))
                    .ok_or_else(|| CuratedPlanError::NegativePlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: "provider omitted the mutation artifact".into(),
                    })?;
                artifacts.push(mutation_artifact);
                bindings.extend(provider_output.bindings);
                advanced_dependencies.extend(provider_output.dependencies);
                artifact_by_recipe_role.insert(
                    (
                        recipe.identity(),
                        recipe.mutation.as_ref().unwrap().output.role.clone(),
                    ),
                    logical_id.clone(),
                );
                let mut projection_recipe = source_artifact_recipe.clone();
                projection_recipe.logical_id = "instance".into();
                projection_recipe.output.role =
                    recipe.mutation.as_ref().unwrap().output.role.clone();
                projection_recipe.output.path = Some(output_path);
                projection_artifacts.push(CuratedArtifactProjectionContext {
                    artifact_id: logical_id,
                    plan_order: order,
                    registry_order: registry_order[registry_case.case_id.as_str()],
                    historical_recipe_order: recipe.planning_order.unwrap_or_else(|| {
                        u32::try_from(registry_order[registry_case.case_id.as_str()])
                            .unwrap_or(u32::MAX)
                    }),
                    historical_artifact_order: 0,
                    registry_case: registry_case.clone().into(),
                    case_recipe: recipe.clone(),
                    artifact_recipe: projection_recipe,
                });
                continue;
            }
            if recipe.plan_provider_id == EXCEPTIONAL_SC_PLAN_PROVIDER_ID {
                let artifact_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
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
                let output = plan_exceptional_sc(ExceptionalScPlanInput {
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
                })?;
                if matches!(
                    output.encoding,
                    ExceptionalScEncodingRequest::LockedFullFile(_)
                ) && !self
                    .capability_inventory
                    .executable_identities
                    .contains_key("dcmcjpeg")
                {
                    record_unavailable_case(
                        registry_case,
                        recipe,
                        CapabilityKind::ExternalBackend,
                        "locked_full_file_codec_unavailable",
                        "the locked full-file transform requires an explicitly qualified dcmcjpeg executable identity"
                            .into(),
                        &mut unavailable,
                        &mut pending,
                    );
                    continue;
                }
                let implementation_class_uid = output
                    .instance
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
                let content_slot = output
                    .instance
                    .content
                    .first()
                    .map(|content| content.slot.clone())
                    .ok_or_else(|| CuratedPlanError::MissingPixelContent(global_id.clone()))?;
                let locked_request = match &output.encoding {
                    ExceptionalScEncodingRequest::LockedFullFile(request) => Some(request.clone()),
                    _ => None,
                };
                let locked_source_id = locked_request
                    .as_ref()
                    .map(|_| format!("{global_id}_locked_source"));
                let execution_binding = match output.encoding {
                    ExceptionalScEncodingRequest::Dataset(_) => ArtifactExecutionBindings {
                        artifact_id: global_id.clone(),
                        slots: BTreeMap::from([(
                            content_slot.clone(),
                            SlotExecutionBinding::NativeFrames {
                                frames: native_frame_bindings(&output.native_pixels)?,
                            },
                        )]),
                    },
                    ExceptionalScEncodingRequest::EncodedFrames(mut codec) => {
                        codec.artifact_id = global_id.clone();
                        codec.slot = content_slot.clone();
                        ArtifactExecutionBindings {
                            artifact_id: global_id.clone(),
                            slots: BTreeMap::from([(
                                content_slot.clone(),
                                SlotExecutionBinding::CodecRequest { request: codec },
                            )]),
                        }
                    }
                    ExceptionalScEncodingRequest::LockedFullFile(locked) => {
                        let identity = &self.capability_inventory.executable_identities["dcmcjpeg"];
                        let source_id = locked_source_id.as_ref().expect("locked source identity");
                        ArtifactExecutionBindings {
                            artifact_id: global_id.clone(),
                            slots: BTreeMap::from([(
                                "dicom".into(),
                                SlotExecutionBinding::ProviderRequest {
                                    request: ProviderRequest {
                                        request_id: locked.request_id.clone(),
                                        artifact_id: global_id.clone(),
                                        provider_id: locked.backend_id.clone(),
                                        required_version: crate::runtime_capabilities::qualified_executable_version_id(identity),
                                        parameters: locked.parameters.clone(),
                                        input_assets: BTreeMap::from([(
                                            "source_dicom".into(),
                                            StagedAssetHandle::new(format!("output:{source_id}"))
                                                .map_err(|error| {
                                                CuratedPlanError::Catalog(error.to_string())
                                            })?,
                                        )]),
                                        expected_outputs: vec![ProviderOutputExpectation {
                                            slot: "dicom".into(),
                                            media_type: "application/dicom".into(),
                                            maximum_size_bytes: 128 * 1024 * 1024,
                                            expected_sha256: None,
                                        }],
                                    },
                                },
                            )]),
                        }
                    }
                };
                let output_path =
                    artifact_recipe.output.path.as_ref().ok_or_else(|| {
                        CuratedPlanError::ProviderDerivedOutput(global_id.clone())
                    })?;
                let order = u64::try_from(artifacts.len())
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                let resources = resource_estimate(
                    &output.instance,
                    output.native_pixels.plan.padded_value_bytes,
                )?;
                let planned = PlannedDicomArtifact {
                    logical_id: global_id.clone(),
                    order,
                    provenance: ArtifactProvenance::Requested,
                    case_binding: Some(CaseBinding {
                        case_id: recipe.binding.case_id.clone(),
                        recipe_id: recipe.recipe_id.clone(),
                        recipe_version: recipe.recipe_version.clone(),
                    }),
                    instance: output.instance,
                    output: OutputPlan {
                        relative_path: OutputRelativePath::new(output_path.clone())?,
                        role: artifact_recipe.output.role.clone(),
                        publish: true,
                    },
                    encoding,
                    validation: validation_plan(recipe, artifact_recipe),
                    evidence: generation_evidence_plan(),
                    resources,
                };
                projection_artifacts.push(CuratedArtifactProjectionContext {
                    artifact_id: global_id.clone(),
                    plan_order: order,
                    registry_order: registry_order[registry_case.case_id.as_str()],
                    historical_recipe_order: recipe.planning_order.ok_or_else(|| {
                        CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone())
                    })?,
                    historical_artifact_order: artifact_recipe.order,
                    registry_case: registry_case.clone().into(),
                    case_recipe: recipe.clone(),
                    artifact_recipe: artifact_recipe.clone(),
                });
                let native_request = native_pixel_request_from_recipe(
                    artifact_recipe.secondary_capture.as_ref().ok_or_else(|| {
                        CuratedPlanError::MissingSecondaryCapture(global_id.clone())
                    })?,
                )
                .map_err(|error| CuratedPlanError::ScPlan {
                    artifact_id: global_id.clone(),
                    message: error.to_string(),
                })?;
                native_content_requests.push(native_content_request(
                    locked_source_id.as_deref().unwrap_or(&global_id),
                    artifact_recipe,
                    native_request,
                    &output.native_pixels,
                ));
                bindings.insert(global_id.clone(), execution_binding.clone());
                source_artifacts.insert(
                    (recipe.identity(), artifact_recipe.logical_id.clone()),
                    CuratedSourceArtifact {
                        planned: planned.clone(),
                        bindings: execution_binding,
                    },
                );
                artifact_by_recipe_role.insert(
                    (recipe.identity(), artifact_recipe.output.role.clone()),
                    global_id.clone(),
                );
                if let Some(locked) = locked_request {
                    let source_id = locked_source_id.expect("locked source identity");
                    let mut source_encoding = planned.encoding.clone();
                    source_encoding.transfer_syntax_uid = locked.source_transfer_syntax_uid.clone();
                    source_encoding.fragmentation = FragmentationPolicy::Native;
                    source_encoding.offset_table = OffsetTablePolicy::NotApplicable;
                    source_encoding.backend_id = "encoding.dicom_rs.explicit_vr_le".into();
                    let mut source_instance = locked.source_plan.clone();
                    source_instance.instance_id = source_id.clone();
                    source_instance.identities.logical_instance_id = source_id.clone();
                    let source_path = format!("private/locked-sources/{source_id}.dcm");
                    let source = PlannedDicomArtifact {
                        logical_id: source_id.clone(),
                        order,
                        provenance: ArtifactProvenance::PrivateSource {
                            consumed_by: vec![global_id.clone()],
                        },
                        case_binding: planned.case_binding.clone(),
                        instance: source_instance,
                        output: OutputPlan {
                            relative_path: OutputRelativePath::new(source_path.clone())?,
                            role: "locked_source".into(),
                            publish: false,
                        },
                        encoding: source_encoding,
                        validation: planned.validation.clone(),
                        evidence: planned.evidence.clone(),
                        resources: planned.resources.clone(),
                    };
                    let source_binding = ArtifactExecutionBindings {
                        artifact_id: source_id.clone(),
                        slots: BTreeMap::from([(
                            content_slot.clone(),
                            SlotExecutionBinding::NativeFrames {
                                frames: native_frame_bindings(&output.native_pixels)?,
                            },
                        )]),
                    };
                    bindings.insert(source_id.clone(), source_binding);
                    let mut source_artifact_recipe = artifact_recipe.clone();
                    source_artifact_recipe.logical_id =
                        format!("{}_locked_source", source_artifact_recipe.logical_id);
                    source_artifact_recipe.output.role = "locked_source".into();
                    source_artifact_recipe.output.path = Some(source_path);
                    projection_artifacts.insert(
                        projection_artifacts.len() - 1,
                        CuratedArtifactProjectionContext {
                            artifact_id: source_id.clone(),
                            plan_order: order,
                            registry_order: registry_order[registry_case.case_id.as_str()],
                            historical_recipe_order: recipe.planning_order.ok_or_else(|| {
                                CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone())
                            })?,
                            historical_artifact_order: artifact_recipe.order,
                            registry_case: registry_case.clone().into(),
                            case_recipe: recipe.clone(),
                            artifact_recipe: source_artifact_recipe,
                        },
                    );
                    projection_artifacts
                        .last_mut()
                        .expect("target projection")
                        .plan_order = order + 1;
                    artifacts.push(PlannedArtifact::Dicom(source));
                    let mut declared_instance = planned.instance.clone();
                    declared_instance.transfer_syntax_uid =
                        locked.target_transfer_syntax_uid.clone();
                    // The locked provider declares the encoded Pixel Data through its
                    // bounded imported observation. Its canonical native content stays
                    // on the private source dependency and is verified after decoding;
                    // it must not be compared byte-for-byte with the encapsulated value.
                    declared_instance.content.clear();
                    let maximum_size_bytes = 128 * 1024 * 1024;
                    artifacts.push(PlannedArtifact::ImportedDicom(
                        PlannedImportedDicomArtifact {
                            logical_id: global_id.clone(),
                            order: order + 1,
                            provenance: ArtifactProvenance::Requested,
                            case_binding: planned.case_binding.clone(),
                            provider: ImportedDicomProviderPlan {
                                request_id: locked.request_id.clone(),
                                provider_id: locked.backend_id.clone(),
                                required_version: crate::runtime_capabilities::qualified_executable_version_id(
                                    &self.capability_inventory.executable_identities["dcmcjpeg"],
                                ),
                                output_slot: "dicom".into(),
                                media_type: "application/dicom".into(),
                                maximum_size_bytes,
                                expected_sha256: None,
                                transfer_syntax_uid: locked.target_transfer_syntax_uid.clone(),
                                parameters: locked.parameters.clone(),
                                source_assets: BTreeMap::from([(
                                    "source_dicom".into(),
                                    source_id.clone(),
                                )]),
                            },
                            declared_instance,
                            output: planned.output.clone(),
                            validation: planned.validation.clone(),
                            evidence: planned.evidence.clone(),
                            resources: ArtifactResourceEstimate {
                                output_bytes: maximum_size_bytes,
                                peak_working_bytes: maximum_size_bytes,
                            },
                        },
                    ));
                    advanced_dependencies.push(ArtifactDependency {
                        artifact_id: global_id.clone(),
                        depends_on: source_id,
                        relationship: "locked_full_file_source".into(),
                        frame_numbers: Vec::new(),
                    });
                    locked_full_file_requests.insert(global_id.clone(), locked);
                } else {
                    artifacts.push(PlannedArtifact::Dicom(planned));
                }
                selected_recipes.push(recipe);
                continue;
            }
            if matches!(
                recipe.plan_provider_id.as_str(),
                ENHANCED_PLAN_PROVIDER | WSI_ADVANCED_PROVIDER_ID
            ) {
                let family = if recipe.plan_provider_id == ENHANCED_PLAN_PROVIDER {
                    AdvancedProviderFamily::Enhanced
                } else {
                    AdvancedProviderFamily::WholeSlide
                };
                let mut provider_output = if family == AdvancedProviderFamily::Enhanced {
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
                if let Some(reason) = match registry_case.case_id.as_str() {
                    "stress/wsi/large_pyramid" => Some(
                        "Full-scale WSI resource behavior is not qualified by the bounded repository corpus.",
                    ),
                    "stress/enhanced-ct/many_frames" => Some(
                        "Full-scale enhanced CT resource behavior is not qualified by the bounded repository corpus.",
                    ),
                    _ => None,
                } {
                    for artifact in &mut provider_output.artifacts {
                        artifact.planned.evidence = reduced_stress_generation_evidence_plan(reason);
                    }
                }
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
                            registration_sources(&declarations, &source_artifacts, &target_id)?,
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
                    let provider = RegistrationPlanProvider::new(
                        self.standards_lock_sha256.clone(),
                    )
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
                    let provider =
                        PresentationPlanProvider::new(self.standards_lock_sha256.clone());
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
            if matches!(
                recipe.plan_provider_id.as_str(),
                WAVEFORM_PLAN_PROVIDER_ID | ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID
            ) {
                let artifact_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                let target_id = artifact_id(recipe, &artifact_recipe.logical_id);
                let context = typed_bulk_context(
                    recipe,
                    artifact_recipe,
                    &target_id,
                    artifacts.len(),
                    request.seed,
                    &self.standards_lock_sha256,
                )?;
                let output = if recipe.plan_provider_id == WAVEFORM_PLAN_PROVIDER_ID {
                    let input = waveform_input_from_recipe(recipe)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    WaveformPlanProvider
                        .plan(&input, &context, ContentProviderLimits::default())
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                } else {
                    let input = encapsulated_payload_input_from_recipe(recipe)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    EncapsulatedPayloadPlanProvider
                        .plan(&input, &context, ContentProviderLimits::default())
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                };
                merge_typed_artifact(
                    recipe,
                    registry_case,
                    registry_order[registry_case.case_id.as_str()],
                    artifact_recipe,
                    output.artifact,
                    output.bindings,
                    Vec::new(),
                    &mut artifacts,
                    &mut bindings,
                    &mut projection_artifacts,
                    &mut artifact_by_recipe_role,
                    &mut source_artifacts,
                    &mut advanced_dependencies,
                )?;
                continue;
            }
            if matches!(
                recipe.plan_provider_id.as_str(),
                SR_PLAN_PROVIDER_ID | RT_PLAN_PROVIDER_ID
            ) {
                let artifact_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                let target_id = artifact_id(recipe, &artifact_recipe.logical_id);
                let declarations = semantic_source_declarations(recipe)?;
                let sources = semantic_sources(&declarations, &source_artifacts, &target_id)?;
                let mut context = semantic_context(
                    recipe,
                    artifact_recipe,
                    &target_id,
                    artifacts.len(),
                    request.seed,
                    &self.standards_lock_sha256,
                    sources,
                    &source_artifacts,
                )?;
                // Recipe parsers verify the document-local ID. The unified spine
                // scopes that ID globally before the provider constructs output.
                let scoped_id = context.logical_id.clone();
                context.logical_id = artifact_recipe.logical_id.clone();
                let output = if recipe.plan_provider_id == SR_PLAN_PROVIDER_ID {
                    let mut input = sr_input_from_recipe(recipe, context)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    input.context.logical_id = scoped_id.clone();
                    SrPlanProvider.plan_native(&input).map_err(|error| {
                        CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        }
                    })?
                } else {
                    let mut input = rt_input_from_recipe(recipe, context)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                    input.context.logical_id = scoped_id.clone();
                    RtPlanProvider
                        .plan(&input)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                };
                merge_typed_artifact(
                    recipe,
                    registry_case,
                    registry_order[registry_case.case_id.as_str()],
                    artifact_recipe,
                    output.artifact,
                    output.bindings,
                    semantic_dependencies(&target_id, &declarations, &source_artifacts)?,
                    &mut artifacts,
                    &mut bindings,
                    &mut projection_artifacts,
                    &mut artifact_by_recipe_role,
                    &mut source_artifacts,
                    &mut advanced_dependencies,
                )?;
                continue;
            }
            if recipe.plan_provider_id == HIGH_DICOM_SR_IMPORT_PROVIDER_ID {
                let artifact_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                let target_id = artifact_id(recipe, &artifact_recipe.logical_id);
                let declarations = semantic_source_declarations(recipe)?;
                let sources = semantic_sources(&declarations, &source_artifacts, &target_id)?;
                let mut context = semantic_context(
                    recipe,
                    artifact_recipe,
                    &target_id,
                    artifacts.len(),
                    request.seed,
                    &self.standards_lock_sha256,
                    sources,
                    &source_artifacts,
                )?;
                let scoped_id = context.logical_id.clone();
                context.logical_id = artifact_recipe.logical_id.clone();
                let mut input = sr_input_from_recipe(recipe, context)
                    .map_err(|error| CuratedPlanError::AdvancedPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    })?
                    .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                        case_id: registry_case.case_id.clone(),
                        provider_id: recipe.plan_provider_id.clone(),
                    })?;
                input.context.logical_id = scoped_id;
                let import = SrPlanProvider.external_import(&input).map_err(|error| {
                    CuratedPlanError::AdvancedPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    }
                })?;
                let dependencies =
                    semantic_dependencies(&target_id, &declarations, &source_artifacts)?;
                let (artifact, execution_binding) = external_sr_artifact(
                    recipe,
                    artifact_recipe,
                    &input,
                    &import,
                    request.seed,
                    &self.standards_lock_sha256,
                    &source_artifacts,
                )?;
                merge_imported_artifact(
                    recipe,
                    registry_case,
                    registry_order[registry_case.case_id.as_str()],
                    artifact_recipe,
                    artifact,
                    execution_binding,
                    dependencies,
                    &mut artifacts,
                    &mut bindings,
                    &mut projection_artifacts,
                    &mut artifact_by_recipe_role,
                    &mut advanced_dependencies,
                )?;
                continue;
            }
            if matches!(
                recipe.plan_provider_id.as_str(),
                QUANTITATIVE_NATIVE_PROVIDER_ID | crate::recipes::QUANTITATIVE_EXTERNAL_PROVIDER_ID
            ) {
                let artifact_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                let target_id = artifact_id(recipe, &artifact_recipe.logical_id);
                let declarations = quantitative_source_declarations(recipe)?;
                let actual_sources = quantitative_sources(&declarations, &source_artifacts)?;
                let parser_sources = quantitative_parser_sources(&declarations, &actual_sources);
                let context = quantitative_context(
                    recipe,
                    artifact_recipe,
                    &target_id,
                    u64::from(artifact_recipe.order),
                    request.seed,
                    &self.standards_lock_sha256,
                    &actual_sources,
                )?;
                let mut input =
                    crate::recipes::quantitative_input_from_recipe(recipe, context, parser_sources)
                        .map_err(|error| CuratedPlanError::AdvancedPlan {
                            recipe_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                        .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                            case_id: registry_case.case_id.clone(),
                            provider_id: recipe.plan_provider_id.clone(),
                        })?;
                assign_quantitative_order(&mut input, artifacts.len())?;
                restore_quantitative_sources(&mut input, actual_sources)?;
                let output = QuantitativePlanProvider
                    .plan(&input, QuantitativeProviderLimits::default())
                    .map_err(|error| CuratedPlanError::AdvancedPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    })?;
                match output {
                    QuantitativePlanOutput::Native {
                        artifact,
                        bindings: execution_binding,
                        dependencies,
                    } => merge_typed_artifact(
                        recipe,
                        registry_case,
                        registry_order[registry_case.case_id.as_str()],
                        artifact_recipe,
                        artifact,
                        execution_binding,
                        dependencies,
                        &mut artifacts,
                        &mut bindings,
                        &mut projection_artifacts,
                        &mut artifact_by_recipe_role,
                        &mut source_artifacts,
                        &mut advanced_dependencies,
                    )?,
                    external @ QuantitativePlanOutput::ExternalImport { .. } => {
                        let (artifact, execution_binding, dependencies) =
                            external_quantitative_artifact(
                                recipe,
                                artifact_recipe,
                                external,
                                request.seed,
                                &self.standards_lock_sha256,
                            )?;
                        merge_imported_artifact(
                            recipe,
                            registry_case,
                            registry_order[registry_case.case_id.as_str()],
                            artifact_recipe,
                            artifact,
                            execution_binding,
                            dependencies,
                            &mut artifacts,
                            &mut bindings,
                            &mut projection_artifacts,
                            &mut artifact_by_recipe_role,
                            &mut advanced_dependencies,
                        )?;
                    }
                }
                continue;
            }
            if recipe.plan_provider_id == STRESS_SC_PLAN_PROVIDER_ID {
                let stress =
                    plan_stress_sc_recipe(recipe, &self.standards_lock_sha256, request.seed)
                        .map_err(|error| CuratedPlanError::ScPlan {
                            artifact_id: recipe.recipe_id.clone(),
                            message: error.to_string(),
                        })?
                        .ok_or_else(|| CuratedPlanError::ScPlan {
                            artifact_id: recipe.recipe_id.clone(),
                            message: "stress SC provider did not own its selected recipe".into(),
                        })?;
                let artifact_recipe = recipe
                    .dicom
                    .as_ref()
                    .and_then(|dicom| dicom.artifacts.first())
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                let artifact_id = artifact_id(recipe, &stress.logical_id);
                let template = self
                    .templates
                    .resolve_qualified(
                        &TemplateId(stress.template_id.clone()),
                        Some(stress.template_version.parse().map_err(|_| {
                            CuratedPlanError::InvalidTemplateVersion(
                                stress.template_version.clone(),
                            )
                        })?),
                    )
                    .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?;
                let (instance, execution_binding, implementation_class_uid) =
                    resolved_stress_sc_artifact(&artifact_id, &stress, template)?;
                let encoding =
                    stress_sc_encoding(artifact_recipe, &stress, implementation_class_uid)?;
                let order = u64::try_from(artifacts.len())
                    .map_err(|_| CuratedPlanError::ResourceOverflow)?;
                let planned = PlannedDicomArtifact {
                    logical_id: artifact_id.clone(),
                    order,
                    provenance: ArtifactProvenance::Requested,
                    case_binding: Some(CaseBinding {
                        case_id: recipe.binding.case_id.clone(),
                        recipe_id: recipe.recipe_id.clone(),
                        recipe_version: recipe.recipe_version.clone(),
                    }),
                    instance,
                    output: OutputPlan {
                        relative_path: stress.output_relative_path.clone(),
                        role: artifact_recipe.output.role.clone(),
                        publish: true,
                    },
                    encoding,
                    validation: validation_plan(recipe, artifact_recipe),
                    evidence: stress_generation_evidence_plan(stress.parameters.policy()),
                    resources: stress.resources.clone(),
                };
                projection_artifacts.push(CuratedArtifactProjectionContext {
                    artifact_id: artifact_id.clone(),
                    plan_order: order,
                    registry_order: registry_order[registry_case.case_id.as_str()],
                    historical_recipe_order: recipe.planning_order.ok_or_else(|| {
                        CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone())
                    })?,
                    historical_artifact_order: artifact_recipe.order,
                    registry_case: registry_case.clone().into(),
                    case_recipe: recipe.clone(),
                    artifact_recipe: artifact_recipe.clone(),
                });
                artifact_by_recipe_role.insert(
                    (recipe.identity(), artifact_recipe.output.role.clone()),
                    artifact_id.clone(),
                );
                bindings.insert(artifact_id.clone(), execution_binding.clone());
                source_artifacts.insert(
                    (recipe.identity(), artifact_recipe.logical_id.clone()),
                    CuratedSourceArtifact {
                        planned: planned.clone(),
                        bindings: execution_binding,
                    },
                );
                artifacts.push(PlannedArtifact::Dicom(planned));
                selected_recipes.push(recipe);
                continue;
            }
            if matches!(
                recipe.plan_provider_id.as_str(),
                CLASSIC_PLAN_PROVIDER | STRESS_CT_PLAN_PROVIDER_ID
            ) {
                let (requests, explicit_resources, reduced_policy) =
                    classic_requests(recipe, &self.standards_lock_sha256, request.seed)?;
                let scope = format!("curated_{}", recipe.recipe_id);
                let planned_instances = OrderedSeriesProvider
                    .plan_scoped(&scope, requests)
                    .map_err(|error| CuratedPlanError::ClassicPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    })?;
                if explicit_resources
                    .as_ref()
                    .is_some_and(|resources| resources.len() != planned_instances.len())
                {
                    return Err(CuratedPlanError::ClassicArtifactMismatch {
                        recipe_id: recipe.recipe_id.clone(),
                        artifact_id: "resource_set".into(),
                    });
                }
                let dicom = recipe
                    .dicom
                    .as_ref()
                    .ok_or_else(|| CuratedPlanError::MissingDicom(recipe.recipe_id.clone()))?;
                selected_recipes.push(recipe);
                for (instance_index, planned) in planned_instances.into_iter().enumerate() {
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
                    let resources = explicit_resources
                        .as_ref()
                        .map(|resources| resources[instance_index].clone())
                        .map(Ok)
                        .unwrap_or_else(|| {
                            resource_estimate(&instance, native.plan.padded_value_bytes)
                        })?;
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
                        evidence: reduced_policy
                            .as_ref()
                            .map(stress_generation_evidence_plan)
                            .unwrap_or_else(generation_evidence_plan),
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
                    let execution_binding =
                        execution_binding(&global_id, artifact_recipe, &native)?;
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
            if !directly_selected_ids.contains(&recipe.binding.case_id) {
                if let Some(required_roles) = required_mutation_sources.get(&recipe.identity()) {
                    recipe_artifacts.retain(|artifact| {
                        required_roles.contains(&artifact.logical_id)
                            || required_roles.contains(&artifact.output.role)
                    });
                }
            }
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
                let PlannedArtifact::Dicom(source_planned) =
                    artifacts.last().expect("SC artifact was just appended")
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
        let (total_output, peak_working) =
            aggregate_resources(&artifacts, request.max_parallelism)?;
        let total_publication = total_output
            .checked_add(MAX_CURATED_MANIFEST_BYTES)
            .ok_or(CuratedPlanError::ResourceOverflow)?;
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
                max_total_output_bytes: total_publication.max(1),
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
            capability_inventory: self.capability_inventory.clone(),
            locked_full_file_requests,
            external_provider_repository_root: self.repository_root.clone(),
            external_provider_standards_lock_path: self.standards_lock_path.clone(),
            external_provider_standards_lock_sha256: self.standards_lock_sha256.clone(),
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

fn quantitative_source_declarations(
    recipe: &CaseRecipe,
) -> Result<Vec<QuantitativeSourceDeclaration>, CuratedPlanError> {
    serde_json::from_value::<QuantitativeProviderParameters>(Value::Object(
        recipe.provider_parameters.clone(),
    ))
    .map(|parameters| parameters.sources)
    .map_err(|error| CuratedPlanError::AdvancedPlan {
        recipe_id: recipe.recipe_id.clone(),
        message: format!("quantitative source declarations: {error}"),
    })
}

fn semantic_source_declarations(
    recipe: &CaseRecipe,
) -> Result<Vec<SemanticSourceDeclaration>, CuratedPlanError> {
    serde_json::from_value::<SemanticProviderParameters>(Value::Object(
        recipe.provider_parameters.clone(),
    ))
    .map(|parameters| parameters.sources)
    .map_err(|error| CuratedPlanError::AdvancedPlan {
        recipe_id: recipe.recipe_id.clone(),
        message: format!("semantic source declarations: {error}"),
    })
}

fn semantic_sources(
    declarations: &[SemanticSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
    target_id: &str,
) -> Result<Vec<SemanticSource>, CuratedPlanError> {
    declarations
        .iter()
        .map(|declaration| {
            let source = sources
                .get(&(
                    declaration.recipe.identity(),
                    declaration.artifact_logical_id.clone(),
                ))
                .ok_or_else(|| CuratedPlanError::MissingDependency {
                    recipe: declaration.recipe.identity(),
                    dependency: declaration.recipe.identity(),
                    role: declaration.role.clone(),
                })?;
            let sop_instance_uid = source
                .planned
                .instance
                .identities
                .get(&CompositionUidRole::SopInstance, 0)
                .ok_or_else(|| {
                    CuratedPlanError::MissingImplementation(source.planned.logical_id.clone())
                })?;
            let study_instance_uid = source
                .planned
                .instance
                .identities
                .get(&CompositionUidRole::StudyInstance, 0)
                .ok_or_else(|| {
                    CuratedPlanError::MissingImplementation(source.planned.logical_id.clone())
                })?;
            let series_instance_uid = source
                .planned
                .instance
                .identities
                .get(&CompositionUidRole::SeriesInstance, 0)
                .ok_or_else(|| {
                    CuratedPlanError::MissingImplementation(source.planned.logical_id.clone())
                })?;
            Ok(SemanticSource {
                recipe: declaration.recipe.identity(),
                recipe_artifact_logical_id: declaration.artifact_logical_id.clone(),
                artifact_id: source.planned.logical_id.clone(),
                role: declaration.role.clone(),
                study_instance_uid: study_instance_uid.into(),
                series_instance_uid: series_instance_uid.into(),
                reference: MaterializedReference {
                    source_instance_id: target_id.into(),
                    target_instance_id: source.planned.logical_id.clone(),
                    role: declaration.role.clone(),
                    frame_role: None,
                    referenced_sop_class_uid: source.planned.instance.sop_class_uid.clone(),
                    referenced_sop_instance_uid: sop_instance_uid.into(),
                    referenced_frames: declaration.referenced_frames.clone(),
                },
            })
        })
        .collect()
}

fn semantic_dependencies(
    target_id: &str,
    declarations: &[SemanticSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<Vec<ArtifactDependency>, CuratedPlanError> {
    declarations
        .iter()
        .map(|declaration| {
            let source = sources
                .get(&(
                    declaration.recipe.identity(),
                    declaration.artifact_logical_id.clone(),
                ))
                .ok_or_else(|| CuratedPlanError::MissingDependency {
                    recipe: declaration.recipe.identity(),
                    dependency: declaration.recipe.identity(),
                    role: declaration.role.clone(),
                })?;
            Ok(ArtifactDependency {
                artifact_id: target_id.into(),
                depends_on: source.planned.logical_id.clone(),
                relationship: format!("semantic_reference:{}", declaration.role),
                frame_numbers: declaration.referenced_frames.clone(),
            })
        })
        .collect()
}

fn semantic_context(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
    target_id: &str,
    order: usize,
    seed: u64,
    standards_lock_sha256: &str,
    sources: Vec<SemanticSource>,
    source_artifacts: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<SemanticPlanContext, CuratedPlanError> {
    let output_version = semantic_output_version(recipe);
    let source = sources
        .first()
        .ok_or_else(|| CuratedPlanError::MissingDependency {
            recipe: recipe.identity(),
            dependency: recipe.identity(),
            role: "semantic_source".into(),
        })?;
    let planned_source = source_artifacts
        .get(&(
            source.recipe.clone(),
            source.recipe_artifact_logical_id.clone(),
        ))
        .ok_or_else(|| CuratedPlanError::MissingDependency {
            recipe: recipe.identity(),
            dependency: source.recipe.clone(),
            role: source.role.clone(),
        })?;
    let identity = |role: CompositionUidRole| {
        planned_source
            .planned
            .instance
            .identities
            .get(&role, 0)
            .map(str::to_owned)
    };
    let rt_object_kind = recipe
        .provider_parameters
        .get("object")
        .and_then(serde_json::Value::as_object)
        .and_then(|object| object.get("kind"))
        .and_then(serde_json::Value::as_str);
    let referenced_object_index =
        if matches!(rt_object_kind, Some("carm_radiation" | "radiation_set")) {
            None
        } else {
            Some(0)
        };
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: &recipe.binding.case_id,
            recipe_version: &recipe.recipe_version,
            run_seed: seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index,
            role,
        })
    };
    let implementation = deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: output_version,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    });
    let mut exact = vec![
        (
            CompositionUidRole::StudyInstance,
            0,
            identity(CompositionUidRole::StudyInstance).ok_or_else(|| {
                CuratedPlanError::MissingImplementation(source.artifact_id.clone())
            })?,
        ),
        (
            CompositionUidRole::SeriesInstance,
            0,
            uid(UidRole::SeriesInstance),
        ),
        (
            CompositionUidRole::SopInstance,
            0,
            uid(UidRole::SopInstance),
        ),
        (
            CompositionUidRole::ImplementationClass,
            0,
            implementation.clone(),
        ),
    ];
    if let Some(value) = identity(CompositionUidRole::FrameOfReference) {
        exact.push((CompositionUidRole::FrameOfReference, 0, value));
    }
    if rt_object_kind == Some("radiation_set") {
        exact.push((
            CompositionUidRole::TemplateDefined("derived_reference_0".into()),
            0,
            deterministic_uid(&DeterministicUidInput {
                standards_lock_sha256,
                case_id: &recipe.binding.case_id,
                recipe_version: &recipe.recipe_version,
                run_seed: seed,
                file_index: 0,
                frame_index: None,
                referenced_object_index: Some(0),
                role: UidRole::DerivedReference,
            }),
        ));
    }
    if recipe.plan_provider_id == HIGH_DICOM_SR_IMPORT_PROVIDER_ID {
        for (index, name) in [(0, "tracking_uid"), (1, "observer_uid")] {
            exact.push((
                CompositionUidRole::TemplateDefined(name.into()),
                0,
                deterministic_uid(&DeterministicUidInput {
                    standards_lock_sha256,
                    case_id: &recipe.binding.case_id,
                    recipe_version: &recipe.recipe_version,
                    run_seed: seed,
                    file_index: 0,
                    frame_index: None,
                    referenced_object_index: Some(index),
                    role: UidRole::DerivedReference,
                }),
            ));
        }
        if recipe.binding.case_id.ends_with("comprehensive3d_scoord3d") {
            exact.push((
                CompositionUidRole::TemplateDefined("fiducial_uid".into()),
                0,
                deterministic_uid(&DeterministicUidInput {
                    standards_lock_sha256,
                    case_id: &recipe.binding.case_id,
                    recipe_version: &recipe.recipe_version,
                    run_seed: seed,
                    file_index: 0,
                    frame_index: None,
                    referenced_object_index: Some(2),
                    role: UidRole::DerivedReference,
                }),
            ));
        }
    }
    let template = artifact
        .template
        .as_ref()
        .ok_or_else(|| CuratedPlanError::MissingTemplate(target_id.into()))?;
    let output = OutputPlan {
        relative_path: OutputRelativePath::new(
            artifact
                .output
                .path
                .clone()
                .ok_or_else(|| CuratedPlanError::ProviderDerivedOutput(target_id.into()))?,
        )?,
        role: artifact.output.role.clone(),
        publish: true,
    };
    Ok(SemanticPlanContext {
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        logical_id: target_id.into(),
        order: u64::try_from(order).map_err(|_| CuratedPlanError::ResourceOverflow)?,
        output,
        template_id: template.template_id.clone(),
        template_version: template.template_version.clone(),
        identities: IdentityPlan::from_exact_values(target_id, exact).map_err(|error| {
            CuratedPlanError::AdvancedPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            }
        })?,
        encoding: if recipe.plan_provider_id == HIGH_DICOM_SR_IMPORT_PROVIDER_ID {
            EncodingPlan {
                transfer_syntax_uid: artifact.encoding.transfer_syntax_uid.clone(),
                sequence_length: SequenceLengthPolicy::WriterDefault,
                item_length: ItemLengthPolicy::WriterDefault,
                fragmentation: FragmentationPolicy::Native,
                offset_table: OffsetTablePolicy::NotApplicable,
                preamble: PreamblePolicy::ZeroFilled,
                file_meta: FileMetaPolicy::Standard,
                implementation: crate::corpus_plan::ImplementationIdentityPlan {
                    class_uid: implementation,
                    version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
                },
                backend_id: "external.highdicom_sr".into(),
            }
        } else {
            encoding_plan_from_recipe(
                &artifact.encoding,
                crate::corpus_plan::ImplementationIdentityPlan {
                    class_uid: implementation,
                    version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
                },
            )
            .map_err(|error| CuratedPlanError::Encoding {
                artifact_id: target_id.into(),
                message: error.to_string(),
            })?
        },
        base_attributes: semantic_common_attributes(recipe, output_version)?,
        sources,
        resources: ArtifactResourceEstimate {
            output_bytes: 1024 * 1024,
            peak_working_bytes: 2 * 1024 * 1024,
        },
    })
}

fn semantic_output_version(recipe: &CaseRecipe) -> &'static str {
    if recipe.plan_provider_id == HIGH_DICOM_SR_IMPORT_PROVIDER_ID {
        crate::PACKAGE_VERSION
    } else {
        crate::BYTE_STABLE_OUTPUT_VERSION
    }
}

fn semantic_common_attributes(
    recipe: &CaseRecipe,
    output_version: &str,
) -> Result<Vec<ResolvedAttribute>, CuratedPlanError> {
    let mut values = vec![
        (Tag(0x0008, 0x001C), DicomVr::CS, "YES"),
        (Tag(0x0010, 0x0010), DicomVr::PN, "DTS^Synthetic^Patient001"),
        (Tag(0x0010, 0x0020), DicomVr::LO, "DTS-PATIENT-001"),
        (Tag(0x0010, 0x0030), DicomVr::DA, "19700101"),
        (Tag(0x0010, 0x0040), DicomVr::CS, "O"),
        (Tag(0x0008, 0x0020), DicomVr::DA, "20260101"),
        (Tag(0x0008, 0x0030), DicomVr::TM, "000000"),
        (Tag(0x0008, 0x0090), DicomVr::PN, ""),
        (Tag(0x0008, 0x0050), DicomVr::SH, ""),
        (Tag(0x0008, 0x0070), DicomVr::LO, "dicom-test-suite"),
        (Tag(0x0018, 0x1020), DicomVr::LO, output_version),
    ];
    let rt_kind = recipe
        .provider_parameters
        .get("object")
        .and_then(serde_json::Value::as_object)
        .and_then(|object| object.get("kind"))
        .and_then(serde_json::Value::as_str);
    if recipe.plan_provider_id != RT_PLAN_PROVIDER_ID
        || matches!(rt_kind, Some("structure_set" | "dose"))
    {
        values.push((Tag(0x0008, 0x1090), DicomVr::LO, recipe.recipe_id.as_str()));
    }
    values
        .into_iter()
        .map(|(tag, vr, value)| {
            Ok(ResolvedAttribute {
                address: AttributeAddress::standard(tag).map_err(|error| {
                    CuratedPlanError::AdvancedPlan {
                        recipe_id: recipe.recipe_id.clone(),
                        message: error.to_string(),
                    }
                })?,
                vr,
                value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                    value.into(),
                ))),
                origin: ValueOrigin::DerivedStructural,
            })
        })
        .collect()
}

fn quantitative_sources(
    declarations: &[QuantitativeSourceDeclaration],
    sources: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<Vec<QuantitativeSourceInput>, CuratedPlanError> {
    declarations
        .iter()
        .map(|declaration| {
            let source = sources
                .get(&(
                    declaration.recipe.identity(),
                    declaration.artifact_logical_id.clone(),
                ))
                .ok_or_else(|| CuratedPlanError::MissingDependency {
                    recipe: declaration.recipe.identity(),
                    dependency: declaration.recipe.identity(),
                    role: declaration.artifact_logical_id.clone(),
                })?;
            Ok(QuantitativeSourceInput {
                role: declaration.role,
                artifact: source.planned.clone(),
                bindings: source.bindings.clone(),
                referenced_frames: declaration.referenced_frames.clone(),
            })
        })
        .collect()
}

fn quantitative_parser_sources(
    declarations: &[QuantitativeSourceDeclaration],
    sources: &[QuantitativeSourceInput],
) -> Vec<QuantitativeSourceInput> {
    declarations
        .iter()
        .zip(sources)
        .map(|(declaration, source)| {
            let mut source = source.clone();
            source.artifact.logical_id = declaration.artifact_logical_id.clone();
            source.bindings.artifact_id = declaration.artifact_logical_id.clone();
            source
        })
        .collect()
}

fn restore_quantitative_sources(
    input: &mut QuantitativePlanInput,
    actual: Vec<QuantitativeSourceInput>,
) -> Result<(), CuratedPlanError> {
    let planned = match input {
        QuantitativePlanInput::NativeSeg { sources, .. }
        | QuantitativePlanInput::NativeRwvm { sources, .. }
        | QuantitativePlanInput::ExternalImport { sources, .. } => sources,
    };
    if planned.len() != actual.len() {
        return Err(CuratedPlanError::ResourceOverflow);
    }
    for (planned, mut actual) in planned.iter_mut().zip(actual) {
        actual.referenced_frames = planned.referenced_frames.clone();
        *planned = actual;
    }
    Ok(())
}

fn assign_quantitative_order(
    input: &mut QuantitativePlanInput,
    order: usize,
) -> Result<(), CuratedPlanError> {
    let order = u64::try_from(order).map_err(|_| CuratedPlanError::ResourceOverflow)?;
    match input {
        QuantitativePlanInput::NativeSeg { artifact, .. }
        | QuantitativePlanInput::NativeRwvm { artifact, .. }
        | QuantitativePlanInput::ExternalImport { artifact, .. } => artifact.order = order,
    }
    Ok(())
}

fn quantitative_context(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
    target_id: &str,
    order: u64,
    seed: u64,
    standards_lock_sha256: &str,
    sources: &[QuantitativeSourceInput],
) -> Result<QuantitativeArtifactContext, CuratedPlanError> {
    let source = sources
        .first()
        .ok_or_else(|| CuratedPlanError::MissingDependency {
            recipe: recipe.identity(),
            dependency: recipe.identity(),
            role: "quantitative_source".into(),
        })?;
    let source_identity = |role| {
        source
            .artifact
            .instance
            .identities
            .get(&role, 0)
            .map(str::to_owned)
    };
    let uid = |case_id: &str, version: &str, run_seed: u64, role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id,
            recipe_version: version,
            run_seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: Some(0),
            role,
        })
    };
    let implementation_recipe_version =
        if recipe.plan_provider_id == crate::recipes::QUANTITATIVE_EXTERNAL_PROVIDER_ID {
            crate::PACKAGE_VERSION
        } else {
            crate::BYTE_STABLE_OUTPUT_VERSION
        };
    let mut identities = vec![
        (
            CompositionUidRole::StudyInstance,
            0,
            source_identity(CompositionUidRole::StudyInstance).ok_or_else(|| {
                CuratedPlanError::MissingImplementation(source.artifact.logical_id.clone())
            })?,
        ),
        (
            CompositionUidRole::SeriesInstance,
            0,
            uid(
                &recipe.binding.case_id,
                &recipe.recipe_version,
                seed,
                UidRole::SeriesInstance,
            ),
        ),
        (
            CompositionUidRole::SopInstance,
            0,
            uid(
                &recipe.binding.case_id,
                &recipe.recipe_version,
                seed,
                UidRole::SopInstance,
            ),
        ),
        (
            CompositionUidRole::ImplementationClass,
            0,
            deterministic_uid(&DeterministicUidInput {
                standards_lock_sha256,
                case_id: "dicom-test-suite/implementation",
                recipe_version: implementation_recipe_version,
                run_seed: 0,
                file_index: 0,
                frame_index: None,
                referenced_object_index: None,
                role: UidRole::ImplementationClass,
            }),
        ),
    ];
    if recipe.provider_parameters.contains_key("segmentation")
        || recipe.plan_provider_id == crate::recipes::QUANTITATIVE_EXTERNAL_PROVIDER_ID
    {
        identities.push((
            CompositionUidRole::DimensionOrganization,
            0,
            uid(
                &recipe.binding.case_id,
                &recipe.recipe_version,
                seed,
                UidRole::DimensionOrganization,
            ),
        ));
    }
    if recipe.provider_parameters.contains_key("segmentation")
        || recipe.plan_provider_id == crate::recipes::QUANTITATIVE_EXTERNAL_PROVIDER_ID
    {
        if let Some(frame) = source_identity(CompositionUidRole::FrameOfReference) {
            identities.push((CompositionUidRole::FrameOfReference, 0, frame));
        }
    }
    Ok(QuantitativeArtifactContext {
        recipe_artifact_logical_id: artifact.logical_id.clone(),
        target_instance_id: target_id.into(),
        order,
        output: OutputPlan {
            relative_path: OutputRelativePath::new(
                artifact
                    .output
                    .path
                    .clone()
                    .ok_or_else(|| CuratedPlanError::ProviderDerivedOutput(target_id.into()))?,
            )?,
            role: artifact.output.role.clone(),
            publish: true,
        },
        identities: IdentityPlan::from_exact_values(target_id, identities).map_err(|error| {
            CuratedPlanError::AdvancedPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            }
        })?,
    })
}

fn typed_bulk_context(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
    target_id: &str,
    order: usize,
    seed: u64,
    standards_lock_sha256: &str,
) -> Result<TypedBulkPlanningContext, CuratedPlanError> {
    let uid = |case_id: &str, version: &str, run_seed: u64, role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id,
            recipe_version: version,
            run_seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let mut identities = vec![
        (
            CompositionUidRole::StudyInstance,
            0,
            uid(
                &recipe.binding.case_id,
                &recipe.recipe_version,
                seed,
                UidRole::StudyInstance,
            ),
        ),
        (
            CompositionUidRole::SeriesInstance,
            0,
            uid(
                &recipe.binding.case_id,
                &recipe.recipe_version,
                seed,
                UidRole::SeriesInstance,
            ),
        ),
        (
            CompositionUidRole::SopInstance,
            0,
            uid(
                &recipe.binding.case_id,
                &recipe.recipe_version,
                seed,
                UidRole::SopInstance,
            ),
        ),
        (
            CompositionUidRole::ImplementationClass,
            0,
            uid(
                "dicom-test-suite/implementation",
                crate::BYTE_STABLE_OUTPUT_VERSION,
                0,
                UidRole::ImplementationClass,
            ),
        ),
    ];
    if recipe.plan_provider_id == ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID {
        let input = encapsulated_payload_input_from_recipe(recipe)
            .map_err(|error| CuratedPlanError::AdvancedPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            })?
            .ok_or_else(|| CuratedPlanError::MissingAdvancedInput {
                case_id: recipe.binding.case_id.clone(),
                provider_id: recipe.plan_provider_id.clone(),
            })?;
        if matches!(
            input.payload,
            EncapsulatedPayload::ClosedTetrahedronBinaryStl { .. }
        ) {
            identities.push((
                CompositionUidRole::FrameOfReference,
                0,
                uid(
                    &recipe.binding.case_id,
                    &recipe.recipe_version,
                    seed,
                    UidRole::FrameOfReference,
                ),
            ));
        }
    }
    Ok(TypedBulkPlanningContext {
        recipe_artifact_logical_id: artifact.logical_id.clone(),
        target_instance_id: target_id.into(),
        order: u64::try_from(order).map_err(|_| CuratedPlanError::ResourceOverflow)?,
        output: OutputPlan {
            relative_path: OutputRelativePath::new(
                artifact
                    .output
                    .path
                    .clone()
                    .ok_or_else(|| CuratedPlanError::ProviderDerivedOutput(target_id.into()))?,
            )?,
            role: artifact.output.role.clone(),
            publish: true,
        },
        identities: IdentityPlan::from_exact_values(target_id, identities).map_err(|error| {
            CuratedPlanError::AdvancedPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            }
        })?,
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
fn merge_typed_artifact(
    recipe: &CaseRecipe,
    registry_case: &RegistryCase,
    registry_order: u64,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    mut artifact: PlannedDicomArtifact,
    execution_binding: ArtifactExecutionBindings,
    dependencies: Vec<ArtifactDependency>,
    artifacts: &mut Vec<PlannedArtifact>,
    bindings: &mut BTreeMap<String, ArtifactExecutionBindings>,
    projection_artifacts: &mut Vec<CuratedArtifactProjectionContext>,
    artifact_by_recipe_role: &mut BTreeMap<(RecipeIdentity, String), String>,
    source_artifacts: &mut BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
    all_dependencies: &mut Vec<ArtifactDependency>,
) -> Result<(), CuratedPlanError> {
    let expected_id = artifact_id(recipe, &artifact_recipe.logical_id);
    let expected_order =
        u64::try_from(artifacts.len()).map_err(|_| CuratedPlanError::ResourceOverflow)?;
    if artifact.logical_id != expected_id
        || artifact.order != expected_order
        || execution_binding.artifact_id != expected_id
        || artifact.output.relative_path.as_str()
            != artifact_recipe.output.path.as_deref().unwrap_or("")
        || artifact.output.role != artifact_recipe.output.role
    {
        return Err(CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: artifact.logical_id,
        });
    }
    let mut seen_rules = artifact
        .validation
        .rules
        .iter()
        .map(|rule| rule.rule_id.clone())
        .collect::<BTreeSet<_>>();
    artifact.validation.rules.extend(
        validation_plan(recipe, artifact_recipe)
            .rules
            .into_iter()
            .filter(|rule| seen_rules.insert(rule.rule_id.clone())),
    );
    let historical_recipe_order = recipe
        .planning_order
        .ok_or_else(|| CuratedPlanError::MissingProjectionOrder(recipe.recipe_id.clone()))?;
    projection_artifacts.push(CuratedArtifactProjectionContext {
        artifact_id: expected_id.clone(),
        plan_order: expected_order,
        registry_order,
        historical_recipe_order,
        historical_artifact_order: artifact_recipe.order,
        registry_case: registry_case.clone().into(),
        case_recipe: recipe.clone(),
        artifact_recipe: artifact_recipe.clone(),
    });
    artifact_by_recipe_role.insert(
        (recipe.identity(), artifact_recipe.output.role.clone()),
        expected_id.clone(),
    );
    bindings.insert(expected_id.clone(), execution_binding.clone());
    source_artifacts.insert(
        (recipe.identity(), artifact_recipe.logical_id.clone()),
        CuratedSourceArtifact {
            planned: artifact.clone(),
            bindings: execution_binding,
        },
    );
    all_dependencies.extend(dependencies);
    artifacts.push(PlannedArtifact::Dicom(artifact));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn merge_imported_artifact(
    recipe: &CaseRecipe,
    registry_case: &RegistryCase,
    registry_order: u64,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    artifact: PlannedImportedDicomArtifact,
    execution_binding: ArtifactExecutionBindings,
    dependencies: Vec<ArtifactDependency>,
    artifacts: &mut Vec<PlannedArtifact>,
    bindings: &mut BTreeMap<String, ArtifactExecutionBindings>,
    projection_artifacts: &mut Vec<CuratedArtifactProjectionContext>,
    artifact_by_recipe_role: &mut BTreeMap<(RecipeIdentity, String), String>,
    all_dependencies: &mut Vec<ArtifactDependency>,
) -> Result<(), CuratedPlanError> {
    let expected_id = artifact_id(recipe, &artifact_recipe.logical_id);
    let expected_order =
        u64::try_from(artifacts.len()).map_err(|_| CuratedPlanError::ResourceOverflow)?;
    if artifact.logical_id != expected_id
        || artifact.order != expected_order
        || execution_binding.artifact_id != expected_id
        || artifact.output.relative_path.as_str()
            != artifact_recipe.output.path.as_deref().unwrap_or("")
        || artifact.output.role != artifact_recipe.output.role
    {
        return Err(CuratedPlanError::AdvancedArtifactMismatch {
            recipe_id: recipe.recipe_id.clone(),
            artifact_id: artifact.logical_id,
        });
    }
    projection_artifacts.push(CuratedArtifactProjectionContext {
        artifact_id: expected_id.clone(),
        plan_order: expected_order,
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
        expected_id.clone(),
    );
    bindings.insert(expected_id, execution_binding);
    all_dependencies.extend(dependencies);
    artifacts.push(PlannedArtifact::ImportedDicom(artifact));
    Ok(())
}

fn required_identity(
    identities: &IdentityPlan,
    role: CompositionUidRole,
    recipe_id: &str,
) -> Result<String, CuratedPlanError> {
    identities
        .get(&role, 0)
        .map(str::to_owned)
        .ok_or_else(|| CuratedPlanError::AdvancedPlan {
            recipe_id: recipe_id.into(),
            message: format!("external import target lacks {}", role.as_str()),
        })
}

fn external_quantitative_artifact(
    recipe: &CaseRecipe,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    output: QuantitativePlanOutput,
    seed: u64,
    standards_lock_sha256: &str,
) -> Result<
    (
        PlannedImportedDicomArtifact,
        ArtifactExecutionBindings,
        Vec<ArtifactDependency>,
    ),
    CuratedPlanError,
> {
    let QuantitativePlanOutput::ExternalImport {
        artifact,
        import,
        sources,
        dependencies,
        references,
        ..
    } = output
    else {
        return Err(CuratedPlanError::AdvancedPlan {
            recipe_id: recipe.recipe_id.clone(),
            message: "external quantitative route returned native output".into(),
        });
    };
    let target_id = artifact.target_instance_id.clone();
    let provider_sources = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let binding_role = if sources.len() == 1 {
                "source_image".to_string()
            } else {
                format!("source_{}", index + 1)
            };
            let identities = &source.artifact.instance.identities;
            Ok(serde_json::json!({
                "binding_role": binding_role,
                "case_id": source.artifact.case_binding.as_ref().map(|binding| binding.case_id.clone()).unwrap_or_else(|| source.artifact.logical_id.clone()),
                "sop_class_uid": source.artifact.instance.sop_class_uid,
                "sop_instance_uid": required_identity(identities, CompositionUidRole::SopInstance, &recipe.recipe_id)?,
                "series_instance_uid": identities.get(&CompositionUidRole::SeriesInstance, 0),
                "frame_numbers": source.referenced_frames,
            }))
        })
        .collect::<Result<Vec<_>, CuratedPlanError>>()?;
    let source_assets = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let role = if sources.len() == 1 {
                "source_image".to_string()
            } else {
                format!("source_{}", index + 1)
            };
            (role, source.artifact.logical_id.clone())
        })
        .collect::<BTreeMap<_, _>>();
    let input_assets = source_assets
        .iter()
        .map(|(role, artifact_id)| {
            Ok((
                role.clone(),
                StagedAssetHandle::new(format!("output:{artifact_id}"))
                    .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, CuratedPlanError>>()?;
    let parameters = BTreeMap::from([
        ("route".into(), Value::String("quantitative".into())),
        ("seed".into(), Value::from(seed)),
        (
            "standards_lock_sha256".into(),
            Value::String(standards_lock_sha256.into()),
        ),
        ("import".into(), serde_json::to_value(&import).unwrap()),
        (
            "artifact_parameters".into(),
            Value::Object(artifact_recipe.parameters.clone()),
        ),
        ("sources".into(), Value::Array(provider_sources)),
        (
            "identities".into(),
            serde_json::to_value(&artifact.identities).unwrap(),
        ),
    ]);
    let provider = ImportedDicomProviderPlan {
        request_id: format!("{}-{target_id}", import.request_id),
        provider_id: import.dependency.executable_provider_id.clone(),
        required_version: import.dependency.required_tool_version.clone(),
        output_slot: "dicom".into(),
        media_type: import.output_media_type.clone(),
        maximum_size_bytes: import.maximum_output_bytes,
        expected_sha256: None,
        transfer_syntax_uid: import.semantic_evidence.transfer_syntax_uid.clone(),
        parameters: parameters.clone(),
        source_assets,
    };
    let template = artifact_recipe
        .template
        .as_ref()
        .ok_or_else(|| CuratedPlanError::MissingTemplate(target_id.clone()))?;
    let planned = PlannedImportedDicomArtifact {
        logical_id: target_id.clone(),
        order: artifact.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: recipe.binding.case_id.clone(),
            recipe_id: recipe.recipe_id.clone(),
            recipe_version: recipe.recipe_version.clone(),
        }),
        provider: provider.clone(),
        declared_instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: target_id.clone(),
            template_id: TemplateId(template.template_id.clone()),
            template_version: template.template_version.parse().map_err(|_| {
                CuratedPlanError::InvalidTemplateVersion(template.template_version.clone())
            })?,
            sop_class_uid: import.semantic_evidence.sop_class_uid.clone(),
            transfer_syntax_uid: import.semantic_evidence.transfer_syntax_uid.clone(),
            identities: artifact.identities,
            attributes: vec![],
            content: vec![],
            references,
        },
        output: artifact.output,
        validation: external_import_validation_plan(recipe, artifact_recipe, &import.kind),
        evidence: external_generation_evidence(&provider.provider_id),
        resources: ArtifactResourceEstimate {
            output_bytes: import.maximum_output_bytes,
            peak_working_bytes: import.maximum_output_bytes,
        },
    };
    let request = ProviderRequest {
        request_id: provider.request_id.clone(),
        artifact_id: target_id.clone(),
        provider_id: provider.provider_id.clone(),
        required_version: provider.required_version.clone(),
        parameters,
        input_assets,
        expected_outputs: vec![ProviderOutputExpectation {
            slot: provider.output_slot.clone(),
            media_type: provider.media_type.clone(),
            maximum_size_bytes: provider.maximum_size_bytes,
            expected_sha256: None,
        }],
    };
    Ok((
        planned,
        ArtifactExecutionBindings {
            artifact_id: target_id,
            slots: BTreeMap::from([(
                provider.output_slot,
                SlotExecutionBinding::ProviderRequest { request },
            )]),
        },
        dependencies,
    ))
}

fn external_sr_artifact(
    recipe: &CaseRecipe,
    artifact_recipe: &crate::recipes::PlannedArtifactRecipe,
    input: &crate::recipes::SrPlanInput,
    import: &crate::recipes::ExternalSrImportRequest,
    seed: u64,
    standards_lock_sha256: &str,
    source_artifacts: &BTreeMap<(RecipeIdentity, String), CuratedSourceArtifact>,
) -> Result<(PlannedImportedDicomArtifact, ArtifactExecutionBindings), CuratedPlanError> {
    let target_id = input.context.logical_id.clone();
    let mut source_assets = BTreeMap::new();
    let mut input_assets = BTreeMap::new();
    let mut provider_sources = Vec::new();
    for (index, source) in input.context.sources.iter().enumerate() {
        let source_case_id = source_artifacts
            .get(&(
                source.recipe.clone(),
                source.recipe_artifact_logical_id.clone(),
            ))
            .and_then(|source| source.planned.case_binding.as_ref())
            .map(|binding| binding.case_id.clone())
            .ok_or_else(|| CuratedPlanError::MissingDependency {
                recipe: recipe.identity(),
                dependency: source.recipe.clone(),
                role: source.role.clone(),
            })?;
        let binding_role = format!("source_{index}");
        source_assets.insert(binding_role.clone(), source.artifact_id.clone());
        input_assets.insert(
            binding_role.clone(),
            StagedAssetHandle::new(format!("output:{}", source.artifact_id))
                .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?,
        );
        provider_sources.push(serde_json::json!({
            "binding_role": binding_role,
            "backend_role": if index == 0 { "source_image" } else { "segmentation" },
            "case_id": source_case_id,
            "sop_class_uid": source.reference.referenced_sop_class_uid,
            "sop_instance_uid": source.reference.referenced_sop_instance_uid,
            "series_instance_uid": source.series_instance_uid,
            "frame_numbers": source.reference.referenced_frames,
        }));
    }
    let parameters = BTreeMap::from([
        ("route".into(), Value::String("structured_report".into())),
        ("seed".into(), Value::from(seed)),
        (
            "standards_lock_sha256".into(),
            Value::String(standards_lock_sha256.into()),
        ),
        ("input".into(), serde_json::to_value(input).unwrap()),
        ("sources".into(), Value::Array(provider_sources)),
    ]);
    let provider = ImportedDicomProviderPlan {
        request_id: import.request_id.clone(),
        provider_id: import.provider_id.clone(),
        required_version: import.required_version.clone(),
        output_slot: "dicom".into(),
        media_type: import.output_media_type.clone(),
        maximum_size_bytes: import.maximum_output_bytes,
        expected_sha256: None,
        transfer_syntax_uid: input.context.encoding.transfer_syntax_uid.clone(),
        parameters: parameters.clone(),
        source_assets,
    };
    let template = artifact_recipe
        .template
        .as_ref()
        .ok_or_else(|| CuratedPlanError::MissingTemplate(target_id.clone()))?;
    let planned = PlannedImportedDicomArtifact {
        logical_id: target_id.clone(),
        order: input.context.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: recipe.binding.case_id.clone(),
            recipe_id: recipe.recipe_id.clone(),
            recipe_version: recipe.recipe_version.clone(),
        }),
        provider: provider.clone(),
        declared_instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: target_id.clone(),
            template_id: TemplateId(template.template_id.clone()),
            template_version: template.template_version.parse().map_err(|_| {
                CuratedPlanError::InvalidTemplateVersion(template.template_version.clone())
            })?,
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.88.34".into(),
            transfer_syntax_uid: input.context.encoding.transfer_syntax_uid.clone(),
            identities: input.context.identities.clone(),
            attributes: vec![],
            content: vec![],
            references: input
                .context
                .sources
                .iter()
                .map(|source| source.reference.clone())
                .collect(),
        },
        output: input.context.output.clone(),
        validation: external_sr_validation_plan(recipe, artifact_recipe),
        evidence: external_generation_evidence(&provider.provider_id),
        resources: input.context.resources.clone(),
    };
    let request = ProviderRequest {
        request_id: provider.request_id.clone(),
        artifact_id: target_id.clone(),
        provider_id: provider.provider_id.clone(),
        required_version: provider.required_version.clone(),
        parameters,
        input_assets,
        expected_outputs: vec![ProviderOutputExpectation {
            slot: provider.output_slot.clone(),
            media_type: provider.media_type.clone(),
            maximum_size_bytes: provider.maximum_size_bytes,
            expected_sha256: None,
        }],
    };
    Ok((
        planned,
        ArtifactExecutionBindings {
            artifact_id: target_id,
            slots: BTreeMap::from([(
                provider.output_slot,
                SlotExecutionBinding::ProviderRequest { request },
            )]),
        },
    ))
}

fn external_generation_evidence(provider_id: &str) -> EvidencePlan {
    EvidencePlan {
        obligations: vec![
            EvidenceObligation {
                obligation_id: "curated_generation_validation".into(),
                route_id: "shared_corpus_executor".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            },
            EvidenceObligation {
                obligation_id: "external_import_provider".into(),
                route_id: provider_id.into(),
                independence: EvidenceIndependence::ExternalProvider,
                required: true,
                parameters: BTreeMap::new(),
            },
        ],
    }
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
            .ok_or_else(|| {
                CuratedPlanError::AdvancedProvenance(advanced.planned.logical_id.clone())
            })?;
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
        || target.output.relative_path.as_str()
            != artifact_recipe.output.path.as_deref().unwrap_or("")
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

fn resolved_stress_sc_artifact(
    artifact_id: &str,
    stress: &crate::recipes::StressScArtifactPlan,
    template: &TemplateDescriptor,
) -> Result<(ResolvedInstancePlan, ArtifactExecutionBindings, String), CuratedPlanError> {
    if template.sop_class_uid != stress.sop_class_uid {
        return Err(CuratedPlanError::ScPlan {
            artifact_id: artifact_id.into(),
            message: "stress SC template SOP class mismatch".into(),
        });
    }
    let identities = IdentityPlan::from_exact_values(
        artifact_id,
        vec![
            (
                CompositionUidRole::StudyInstance,
                0,
                stress.identities.study_instance_uid.clone(),
            ),
            (
                CompositionUidRole::SeriesInstance,
                0,
                stress.identities.series_instance_uid.clone(),
            ),
            (
                CompositionUidRole::SopInstance,
                0,
                stress.identities.sop_instance_uid.clone(),
            ),
            (
                CompositionUidRole::ImplementationClass,
                0,
                stress.identities.implementation_class_uid.clone(),
            ),
        ],
    )
    .map_err(|error| CuratedPlanError::ScPlan {
        artifact_id: artifact_id.into(),
        message: error.to_string(),
    })?;
    let mut attributes = stress_sc_common_attributes(stress)?;
    let (pixel_payload, rows, columns, frames, bits_allocated, pixel_vr) = match &stress.pixels {
        crate::recipes::StressScPixelRequest::RepeatedU16 {
            rows,
            columns,
            value,
        } => {
            if *value != 0 {
                return Err(CuratedPlanError::ScPlan {
                    artifact_id: artifact_id.into(),
                    message: "unsupported nonzero repeated stress pixels".into(),
                });
            }
            (
                StressPayloadRequest::RepeatedByte {
                    byte: 0,
                    length: u64::from(*rows)
                        .checked_mul(u64::from(*columns))
                        .and_then(|value| value.checked_mul(2))
                        .ok_or(CuratedPlanError::ResourceOverflow)?,
                },
                *rows,
                *columns,
                1,
                16,
                DicomVr::OW,
            )
        }
        crate::recipes::StressScPixelRequest::LiteralU8 {
            rows,
            columns,
            values,
        } => (
            StressPayloadRequest::Literal {
                bytes: values.clone(),
            },
            *rows,
            *columns,
            1,
            8,
            DicomVr::OB,
        ),
        crate::recipes::StressScPixelRequest::AlgorithmicU8Multiframe {
            rows,
            columns,
            frames,
            algorithm,
        } => (
            StressPayloadRequest::DeterministicU8Frames {
                rows: *rows,
                columns: *columns,
                frames: *frames,
                algorithm: algorithm.clone(),
            },
            *rows,
            *columns,
            *frames,
            8,
            DicomVr::OB,
        ),
    };
    stress_sc_pixel_attributes(&mut attributes, rows, columns, frames, bits_allocated)?;
    let (pixel_content, pixel_request, pixel_identity) = stress_content_slot(
        artifact_id,
        "pixels",
        pixel_payload,
        AttributeAddress::standard(Tag(0x7FE0, 0x0010)).map_err(attribute_error)?,
        pixel_vr,
        ContentPlacement::TopLevel,
    )?;
    let mut contents = vec![pixel_content];
    let mut slots = BTreeMap::new();
    if stress.transfer_syntax_uid == RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
        let handle = stress_asset_handle(artifact_id, "pixels")?;
        let frame_bindings = pixel_identity
            .frame_ranges
            .iter()
            .enumerate()
            .map(|(index, (offset, length))| NativeFrameBinding {
                frame_number: index as u32 + 1,
                bytes: ByteBinding::VerifiedAssetRange {
                    asset: handle.clone(),
                    offset: *offset,
                    length: *length,
                },
                rows,
                columns,
                samples_per_pixel: 1,
                bits_allocated,
                photometric_interpretation: "MONOCHROME2".into(),
            })
            .collect();
        slots.insert(
            "pixels".into(),
            SlotExecutionBinding::ProviderCodecPipeline {
                provider: pixel_request,
                codec: CodecRequest {
                    request_id: format!("stress_codec_{artifact_id}_pixels"),
                    artifact_id: artifact_id.into(),
                    slot: "pixels".into(),
                    backend_id: NativeRleLosslessEncoder::BACKEND_ID.into(),
                    source_transfer_syntax_uid: EXPLICIT_VR_LE.into(),
                    target_transfer_syntax_uid: RLE_LOSSLESS_TRANSFER_SYNTAX_UID.into(),
                    frames: frame_bindings,
                    parameters: BTreeMap::from([("bits_stored".into(), Value::from(8))]),
                },
            },
        );
    } else {
        slots.insert(
            "pixels".into(),
            SlotExecutionBinding::ProviderRequest {
                request: pixel_request,
            },
        );
    }

    match &stress.content {
        crate::recipes::StressScContentRequest::NestedPrivateBulk {
            sequence_depth,
            creator,
            byte,
            length,
        } => {
            let sequence =
                AttributeAddress::raw_private(Tag(0x7777, 0x1002)).map_err(attribute_error)?;
            attributes.push(nested_sequence_attribute(
                *sequence_depth,
                creator,
                sequence.clone(),
            )?);
            let (content, request, _) = stress_content_slot(
                artifact_id,
                "nested_bulk",
                StressPayloadRequest::RepeatedByte {
                    byte: *byte,
                    length: *length,
                },
                AttributeAddress::private(Tag(0x7777, 0x1001), creator.clone())
                    .map_err(attribute_error)?,
                DicomVr::OB,
                ContentPlacement::Nested {
                    sequence_path: (0..*sequence_depth)
                        .map(|depth| SequenceItemPlacement {
                            sequence: if depth == 0 {
                                sequence.clone()
                            } else {
                                AttributeAddress::private(sequence.tag(), creator.clone())
                                    .expect("validated stress private sequence")
                            },
                            item_index: 0,
                        })
                        .collect(),
                },
            )?;
            contents.push(content);
            slots.insert(
                "nested_bulk".into(),
                SlotExecutionBinding::ProviderRequest { request },
            );
        }
        crate::recipes::StressScContentRequest::RepeatedPrivateText {
            creator_blocks,
            values_per_block,
            value_bytes,
            fill_character,
        } => add_stress_private_text(
            &mut attributes,
            *creator_blocks,
            *values_per_block,
            *value_bytes,
            *fill_character,
        )?,
        crate::recipes::StressScContentRequest::RepeatedNativeBytes { .. }
        | crate::recipes::StressScContentRequest::DeterministicRleFrames { .. } => {}
    }
    attributes.sort_by(|left, right| left.address.cmp(&right.address));
    Ok((
        ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: artifact_id.into(),
            template_id: template.template_id.clone(),
            template_version: template.template_version,
            sop_class_uid: stress.sop_class_uid.clone(),
            transfer_syntax_uid: stress.transfer_syntax_uid.clone(),
            identities,
            attributes,
            content: contents,
            references: Vec::new(),
        },
        ArtifactExecutionBindings {
            artifact_id: artifact_id.into(),
            slots,
        },
        stress.identities.implementation_class_uid.clone(),
    ))
}

fn stress_content_slot(
    artifact_id: &str,
    slot: &str,
    payload: StressPayloadRequest,
    address: AttributeAddress,
    vr: DicomVr,
    placement: ContentPlacement,
) -> Result<
    (
        CanonicalContent,
        ProviderRequest,
        crate::executor::stress_content::StressPayloadIdentity,
    ),
    CuratedPlanError,
> {
    let identity = stress_payload_identity(&payload).map_err(|error| CuratedPlanError::ScPlan {
        artifact_id: artifact_id.into(),
        message: error.to_string(),
    })?;
    let request = ProviderRequest {
        request_id: format!("stress_content_{artifact_id}_{slot}"),
        artifact_id: artifact_id.into(),
        provider_id: STRESS_CONTENT_PROVIDER_ID.into(),
        required_version: STRESS_CONTENT_PROVIDER_VERSION.into(),
        parameters: BTreeMap::from([(
            "payload".into(),
            serde_json::to_value(&payload)
                .map_err(|error| CuratedPlanError::Catalog(error.to_string()))?,
        )]),
        input_assets: BTreeMap::new(),
        expected_outputs: vec![ProviderOutputExpectation {
            slot: slot.into(),
            media_type: "application/octet-stream".into(),
            maximum_size_bytes: identity.size_bytes,
            expected_sha256: Some(identity.sha256.clone()),
        }],
    };
    Ok((
        CanonicalContent {
            slot: slot.into(),
            kind: if slot == "pixels" {
                "native_pixel_data"
            } else {
                "private_bulk_data"
            }
            .into(),
            address,
            vr,
            size_bytes: identity.size_bytes,
            sha256: identity.sha256.clone(),
            properties: BTreeMap::from([("provider_id".into(), STRESS_CONTENT_PROVIDER_ID.into())]),
            placement,
            materialization: None,
        },
        request,
        identity,
    ))
}

fn stress_asset_handle(
    artifact_id: &str,
    slot: &str,
) -> Result<StagedAssetHandle, CuratedPlanError> {
    StagedAssetHandle::new(format!("stress_{artifact_id}_{slot}"))
        .map_err(|error| CuratedPlanError::Catalog(error.to_string()))
}

fn stress_sc_encoding(
    artifact: &crate::recipes::PlannedArtifactRecipe,
    stress: &crate::recipes::StressScArtifactPlan,
    implementation_class_uid: String,
) -> Result<EncodingPlan, CuratedPlanError> {
    if stress.transfer_syntax_uid != RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
        return encoding_plan_from_recipe(
            &artifact.encoding,
            crate::corpus_plan::ImplementationIdentityPlan {
                class_uid: implementation_class_uid,
                version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
            },
        )
        .map_err(|error| CuratedPlanError::Encoding {
            artifact_id: stress.logical_id.clone(),
            message: error.to_string(),
        });
    }
    let fragments_per_frame = match stress.content {
        crate::recipes::StressScContentRequest::DeterministicRleFrames {
            fragments_per_frame,
            ..
        } => fragments_per_frame,
        _ => {
            return Err(CuratedPlanError::ScPlan {
                artifact_id: stress.logical_id.clone(),
                message: "RLE stress artifact lacks deterministic frame content".into(),
            });
        }
    };
    Ok(EncodingPlan {
        transfer_syntax_uid: stress.transfer_syntax_uid.clone(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: FragmentationPolicy::FixedFragmentsPerFrame {
            fragments_per_frame,
        },
        offset_table: OffsetTablePolicy::Extended,
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: crate::corpus_plan::ImplementationIdentityPlan {
            class_uid: implementation_class_uid,
            version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
        },
        backend_id: "encoding.native.rle_lossless".into(),
    })
}

fn stress_sc_common_attributes(
    stress: &crate::recipes::StressScArtifactPlan,
) -> Result<Vec<ResolvedAttribute>, CuratedPlanError> {
    let common = &stress.common;
    let mut attributes = vec![
        stress_string("0008,0016", DicomVr::UI, &stress.sop_class_uid)?,
        stress_string(
            "0008,0018",
            DicomVr::UI,
            &stress.identities.sop_instance_uid,
        )?,
        stress_string("0008,001C", DicomVr::CS, "YES")?,
        stress_string("0010,0010", DicomVr::PN, &common.patient_name)?,
        stress_string("0010,0020", DicomVr::LO, &common.patient_id)?,
        stress_string("0010,0030", DicomVr::DA, &common.patient_birth_date)?,
        stress_string("0010,0040", DicomVr::CS, &common.patient_sex)?,
        stress_string(
            "0020,000D",
            DicomVr::UI,
            &stress.identities.study_instance_uid,
        )?,
        stress_string("0008,0020", DicomVr::DA, &common.study_date)?,
        stress_string("0008,0030", DicomVr::TM, &common.study_time)?,
        stress_string("0008,0090", DicomVr::PN, "")?,
        stress_string("0020,0010", DicomVr::SH, &common.study_id)?,
        stress_string("0008,0050", DicomVr::SH, "")?,
        stress_string("0008,0060", DicomVr::CS, &common.modality)?,
        stress_string(
            "0020,000E",
            DicomVr::UI,
            &stress.identities.series_instance_uid,
        )?,
        stress_string("0020,0011", DicomVr::IS, &common.series_number)?,
        stress_string("0020,0060", DicomVr::CS, "")?,
        stress_string("0008,0064", DicomVr::CS, &common.conversion_type)?,
        stress_string("0008,0070", DicomVr::LO, &common.manufacturer)?,
        stress_string("0008,1090", DicomVr::LO, &common.manufacturer_model_name)?,
        stress_string("0018,1020", DicomVr::LO, crate::BYTE_STABLE_OUTPUT_VERSION)?,
        stress_string("0020,0013", DicomVr::IS, &common.instance_number)?,
        stress_string("0020,0020", DicomVr::CS, "")?,
        stress_string("0008,0023", DicomVr::DA, "20260101")?,
        stress_string("0008,0033", DicomVr::TM, "000000")?,
    ];
    if stress.transfer_syntax_uid == RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
        attributes.extend([
            stress_string("0008,002A", DicomVr::DT, "20260101000000")?,
            stress_string("0020,0012", DicomVr::IS, "1")?,
            stress_string("0028,0301", DicomVr::CS, "NO")?,
            stress_string("0028,2110", DicomVr::CS, "00")?,
            stress_string("0028,1052", DicomVr::DS, "0")?,
            stress_string("0028,1053", DicomVr::DS, "1")?,
            stress_string("0028,1054", DicomVr::LO, "US")?,
            stress_string("2050,0020", DicomVr::CS, "IDENTITY")?,
        ]);
    }
    Ok(attributes)
}

fn stress_sc_pixel_attributes(
    attributes: &mut Vec<ResolvedAttribute>,
    rows: u32,
    columns: u32,
    frames: u32,
    bits_allocated: u16,
) -> Result<(), CuratedPlanError> {
    attributes.extend([
        stress_unsigned("0028,0002", DicomVr::US, 1)?,
        stress_string("0028,0004", DicomVr::CS, "MONOCHROME2")?,
        stress_unsigned("0028,0010", DicomVr::US, u64::from(rows))?,
        stress_unsigned("0028,0011", DicomVr::US, u64::from(columns))?,
        stress_unsigned("0028,0100", DicomVr::US, u64::from(bits_allocated))?,
        stress_unsigned("0028,0101", DicomVr::US, u64::from(bits_allocated))?,
        stress_unsigned(
            "0028,0102",
            DicomVr::US,
            u64::from(bits_allocated.saturating_sub(1)),
        )?,
        stress_unsigned("0028,0103", DicomVr::US, 0)?,
    ]);
    if frames > 1 {
        attributes.push(stress_string(
            "0028,0008",
            DicomVr::IS,
            &frames.to_string(),
        )?);
        let page = AttributeAddress::standard(Tag(0x0018, 0x2001)).map_err(attribute_error)?;
        attributes.push(ResolvedAttribute {
            address: AttributeAddress::standard(Tag(0x0028, 0x0009)).map_err(attribute_error)?,
            vr: DicomVr::AT,
            value: Some(AttributeValue::Primitive(PrimitiveValue::Tag(page))),
            origin: ValueOrigin::InstanceOverride,
        });
        attributes.push(ResolvedAttribute {
            address: AttributeAddress::standard(Tag(0x0018, 0x2001)).map_err(attribute_error)?,
            vr: DicomVr::IS,
            value: Some(AttributeValue::Multi(
                (1..=frames)
                    .map(|frame| PrimitiveValue::String(frame.to_string()))
                    .collect(),
            )),
            origin: ValueOrigin::InstanceOverride,
        });
    }
    Ok(())
}

fn nested_sequence_attribute(
    depth: u32,
    creator: &str,
    top_level_sequence: AttributeAddress,
) -> Result<ResolvedAttribute, CuratedPlanError> {
    if depth == 0 {
        return Err(CuratedPlanError::Catalog(
            "zero stress sequence depth".into(),
        ));
    }
    let creator_operation = || AttributeOperation::Set {
        address: AttributeAddress::standard(Tag(0x7777, 0x0010)).expect("private creator tag"),
        vr: DicomVr::LO,
        value: AttributeValue::Primitive(PrimitiveValue::String(creator.into())),
    };
    let nested_sequence = AttributeAddress::private(top_level_sequence.tag(), creator.to_owned())
        .map_err(attribute_error)?;
    let mut item_operations = vec![creator_operation()];
    for _ in 1..depth {
        item_operations = vec![
            creator_operation(),
            AttributeOperation::Set {
                address: nested_sequence.clone(),
                vr: DicomVr::SQ,
                value: AttributeValue::Sequence(vec![AttributeItem {
                    attributes: item_operations,
                }]),
            },
        ];
    }
    Ok(ResolvedAttribute {
        address: top_level_sequence,
        vr: DicomVr::SQ,
        value: Some(AttributeValue::Sequence(vec![AttributeItem {
            attributes: item_operations,
        }])),
        origin: ValueOrigin::InstanceOverride,
    })
}

fn add_stress_private_text(
    attributes: &mut Vec<ResolvedAttribute>,
    creator_blocks: u32,
    values_per_block: u32,
    value_bytes: u32,
    fill_character: char,
) -> Result<(), CuratedPlanError> {
    let value = fill_character.to_string().repeat(value_bytes as usize);
    for block in 0..creator_blocks {
        let creator = format!("DTS_STRESS_LONG_{block}");
        attributes.push(ResolvedAttribute {
            address: AttributeAddress::standard(Tag(0x7777, 0x0010 + block as u16))
                .map_err(attribute_error)?,
            vr: DicomVr::LO,
            value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                creator.clone(),
            ))),
            origin: ValueOrigin::InstanceOverride,
        });
        for element in 0..values_per_block {
            let physical = (((0x10 + block) << 8) | element) as u16;
            attributes.push(ResolvedAttribute {
                address: AttributeAddress::private(Tag(0x7777, physical), creator.clone())
                    .map_err(attribute_error)?,
                vr: DicomVr::UT,
                value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                    value.clone(),
                ))),
                origin: ValueOrigin::InstanceOverride,
            });
        }
    }
    Ok(())
}

fn stress_string(
    tag: &str,
    vr: DicomVr,
    value: &str,
) -> Result<ResolvedAttribute, CuratedPlanError> {
    Ok(ResolvedAttribute {
        address: AttributeAddress::from_normalized_tag(tag).map_err(attribute_error)?,
        vr,
        value: Some(AttributeValue::Primitive(PrimitiveValue::String(
            value.into(),
        ))),
        origin: ValueOrigin::InstanceOverride,
    })
}

fn stress_unsigned(
    tag: &str,
    vr: DicomVr,
    value: u64,
) -> Result<ResolvedAttribute, CuratedPlanError> {
    Ok(ResolvedAttribute {
        address: AttributeAddress::from_normalized_tag(tag).map_err(attribute_error)?,
        vr,
        value: Some(AttributeValue::Primitive(PrimitiveValue::Unsigned(value))),
        origin: ValueOrigin::InstanceOverride,
    })
}

fn attribute_error(error: impl fmt::Display) -> CuratedPlanError {
    CuratedPlanError::Catalog(error.to_string())
}

fn classic_requests(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<
    (
        Vec<ClassicInstanceRequest>,
        Option<Vec<ArtifactResourceEstimate>>,
        Option<crate::recipes::ReducedStressPolicy>,
    ),
    CuratedPlanError,
> {
    if recipe.plan_provider_id == STRESS_CT_PLAN_PROVIDER_ID {
        let output = plan_stress_ct_recipe(recipe, standards_lock_sha256, seed)
            .map_err(|error| CuratedPlanError::ClassicPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            })?
            .ok_or_else(|| CuratedPlanError::ClassicProviderCardinality {
                recipe_id: recipe.recipe_id.clone(),
                matches: 0,
            })?;
        return Ok((output.requests, Some(output.resources), Some(output.policy)));
    }
    if let Some(requests) =
        plan_ct_recipe(recipe, standards_lock_sha256, seed).map_err(|error| {
            CuratedPlanError::ClassicPlan {
                recipe_id: recipe.recipe_id.clone(),
                message: error.to_string(),
            }
        })?
    {
        // CT is selected by its stable provider/template/content/algorithm/
        // projection tuple. Short-circuit before the remaining transitional
        // case-name family matchers so an arbitrary caller name cannot cause a
        // second planner to claim, or reject, a valid CT capability.
        return Ok((requests, None, None));
    }
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
    Ok((
        matched.pop().expect("one classic provider matched"),
        None,
        None,
    ))
}

#[cfg(test)]
mod classic_ct_capability_tests {
    use super::*;
    use crate::recipes::ClassicProjectionFamily;

    const CT_CASE: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le";
    const DX_CASE: &str = "classic/dx/display_shutter_mono2_u16_explicit_le";

    fn recipe(case_id: &str) -> CaseRecipe {
        let catalog = RecipeCatalog::load(
            "cases/recipes",
            "cases/registry.json",
            "templates/catalog.json",
        )
        .unwrap();
        let identity = catalog.binding_for_case(case_id).unwrap();
        catalog.recipes()[identity].clone()
    }

    fn lock_hash() -> String {
        sha256_hex(
            &crate::engine_resources::EngineResources::embedded()
                .bytes("standards.lock.json")
                .unwrap(),
        )
    }

    fn rejected(recipe: &CaseRecipe) {
        assert!(
            classic_requests(recipe, &lock_hash(), 1).is_err(),
            "malformed declared CT tuple must fail closed"
        );
    }

    #[test]
    fn ct_capability_dispatch_is_name_independent_and_fail_closed() {
        let original = recipe(CT_CASE);
        let direct = plan_ct_recipe(&original, &lock_hash(), 1).unwrap().unwrap();
        assert_eq!(
            classic_requests(&original, &lock_hash(), 1).unwrap().0,
            direct
        );

        let mut renamed = original.clone();
        renamed.binding.case_id = "caller/example/signed-ct".into();
        renamed.recipe_id = "caller_signed_ct".into();
        renamed.planning_order = Some(900);
        let renamed_plan = classic_requests(&renamed, &lock_hash(), 1).unwrap().0;
        assert_eq!(renamed_plan.len(), 1);

        let mut misleading = renamed.clone();
        misleading.binding.case_id = "classic/dx/caller-named-ct".into();
        misleading.recipe_id = "caller_named_ct_under_dx".into();
        assert_eq!(
            classic_requests(&misleading, &lock_hash(), 1)
                .unwrap()
                .0
                .len(),
            1
        );

        let mut wrong_algorithm = renamed.clone();
        wrong_algorithm.dicom.as_mut().unwrap().artifacts[0].algorithm_provider_id =
            Some("algorithm.classic_dx_mg".into());
        rejected(&wrong_algorithm);

        let mut wrong_template = renamed.clone();
        wrong_template.dicom.as_mut().unwrap().artifacts[0]
            .template
            .as_mut()
            .unwrap()
            .template_id = "classic/dx/for-presentation".into();
        rejected(&wrong_template);

        let mut wrong_template_version = renamed.clone();
        wrong_template_version.dicom.as_mut().unwrap().artifacts[0]
            .template
            .as_mut()
            .unwrap()
            .template_version = "2.0.0".into();
        rejected(&wrong_template_version);

        let mut wrong_content = renamed.clone();
        wrong_content.dicom.as_mut().unwrap().artifacts[0]
            .content
            .provider_id = "content.case_default".into();
        rejected(&wrong_content);

        let mut wrong_projection = renamed.clone();
        wrong_projection.dicom.as_mut().unwrap().artifacts[0]
            .classic_projection
            .as_mut()
            .unwrap()
            .family = ClassicProjectionFamily::DxMg;
        rejected(&wrong_projection);

        let mut wrong_provider_parameters = renamed.clone();
        wrong_provider_parameters
            .provider_parameters
            .insert("untyped_escape_hatch".into(), Value::Bool(true));
        rejected(&wrong_provider_parameters);

        let mut wrong_plan_provider = renamed.clone();
        wrong_plan_provider.plan_provider_id = "native.sc_plan".into();
        rejected(&wrong_plan_provider);

        let mut wrong_artifact_parameters = renamed.clone();
        wrong_artifact_parameters.dicom.as_mut().unwrap().artifacts[0]
            .parameters
            .insert("untyped_escape_hatch".into(), Value::Bool(true));
        rejected(&wrong_artifact_parameters);

        let mut mixed = recipe("geometry/ct/duplicate_missing_instance_number");
        mixed.dicom.as_mut().unwrap().artifacts[1].algorithm_provider_id =
            Some("algorithm.classic_dx_mg".into());
        rejected(&mixed);

        let unrelated = recipe(DX_CASE);
        assert!(
            crate::recipes::classic_ct::inspect_ct_capability(&unrelated)
                .unwrap()
                .is_none()
        );
        assert!(
            !classic_requests(&unrelated, &lock_hash(), 1)
                .unwrap()
                .0
                .is_empty()
        );
    }
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

fn stress_generation_evidence_plan(policy: &crate::recipes::ReducedStressPolicy) -> EvidencePlan {
    generation_evidence_plan_with_stress_policy(
        &policy.qualification_scale,
        policy.full_scale_available,
        &policy.full_scale_reason,
    )
}

fn reduced_stress_generation_evidence_plan(full_scale_reason: &str) -> EvidencePlan {
    generation_evidence_plan_with_stress_policy("reduced", false, full_scale_reason)
}

fn generation_evidence_plan_with_stress_policy(
    qualification_scale: &str,
    full_scale_available: bool,
    full_scale_reason: &str,
) -> EvidencePlan {
    EvidencePlan {
        obligations: vec![EvidenceObligation {
            obligation_id: "curated_generation_validation".into(),
            route_id: "shared_corpus_executor".into(),
            independence: EvidenceIndependence::SameProject,
            required: true,
            parameters: BTreeMap::from([
                (
                    "qualification_scale".into(),
                    Value::String(qualification_scale.to_owned()),
                ),
                (
                    "full_scale_available".into(),
                    Value::Bool(full_scale_available),
                ),
                (
                    "full_scale_reason".into(),
                    Value::String(full_scale_reason.to_owned()),
                ),
            ]),
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

fn external_sr_validation_plan(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
) -> ValidationPlan {
    external_validation_plan(recipe, artifact, &[])
}

fn external_import_validation_plan(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
    kind: &crate::recipes::ExternalImportKind,
) -> ValidationPlan {
    let wsi_rules: &[&str] =
        if *kind == crate::recipes::ExternalImportKind::WholeSlideTileSegmentation {
            &[
                "wsi_tile_seg_source_file_sha256",
                "wsi_tile_seg_referenced_series_uid",
                "wsi_tile_seg_dimension_index_items",
                "wsi_tile_seg_per_frame_items",
                "wsi_tile_seg_payload_sha256",
                "wsi_tile_seg_reconstructed_matrix_sha256",
            ]
        } else {
            &[]
        };
    external_validation_plan(recipe, artifact, wsi_rules)
}

fn external_validation_plan(
    recipe: &CaseRecipe,
    artifact: &crate::recipes::PlannedArtifactRecipe,
    specialized_rules: &[&str],
) -> ValidationPlan {
    let mut plan = validation_plan(recipe, artifact);
    let mut seen = plan
        .rules
        .iter()
        .map(|rule| rule.rule_id.clone())
        .collect::<BTreeSet<_>>();
    for rule_id in ["curated_composition_plan", "external_backend_contract"]
        .into_iter()
        .chain(specialized_rules.iter().copied())
    {
        if seen.insert(rule_id.into()) {
            plan.rules.push(ValidationRule {
                rule_id: rule_id.into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            });
        }
    }
    plan
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

fn aggregate_resources(
    artifacts: &[PlannedArtifact],
    max_parallelism: u32,
) -> Result<(u64, u64), CuratedPlanError> {
    let mut total = 0_u64;
    let mut working_sets = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        total = total
            .checked_add(artifact.resource_estimate().output_bytes)
            .ok_or(CuratedPlanError::ResourceOverflow)?;
        working_sets.push(artifact.resource_estimate().peak_working_bytes);
    }
    working_sets.sort_unstable_by(|left, right| right.cmp(left));
    let parallelism =
        usize::try_from(max_parallelism).map_err(|_| CuratedPlanError::ResourceOverflow)?;
    let peak =
        working_sets
            .into_iter()
            .take(parallelism)
            .try_fold(0_u64, |total, working_set| {
                total
                    .checked_add(working_set)
                    .ok_or(CuratedPlanError::ResourceOverflow)
            })?;
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

fn retarget_bindings(
    mut bindings: ArtifactExecutionBindings,
    artifact_id: &str,
) -> ArtifactExecutionBindings {
    bindings.artifact_id = artifact_id.to_owned();
    for binding in bindings.slots.values_mut() {
        match binding {
            SlotExecutionBinding::ProviderRequest { request } => {
                request.artifact_id = artifact_id.to_owned();
            }
            SlotExecutionBinding::CodecRequest { request } => {
                request.artifact_id = artifact_id.to_owned();
            }
            SlotExecutionBinding::ProviderCodecPipeline { provider, codec } => {
                provider.artifact_id = artifact_id.to_owned();
                codec.artifact_id = artifact_id.to_owned();
            }
            SlotExecutionBinding::StagedAsset { .. }
            | SlotExecutionBinding::NativeFrames { .. }
            | SlotExecutionBinding::EncodedFrames { .. } => {}
        }
    }
    bindings
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

fn record_unavailable_case(
    registry_case: &RegistryCase,
    recipe: &CaseRecipe,
    kind: CapabilityKind,
    reason_code: &str,
    message: String,
    unavailable: &mut Vec<UnavailableCapability>,
    pending: &mut Vec<PendingCuratedCase>,
) {
    let artifact_ids = recipe
        .dicom
        .as_ref()
        .map(|dicom| {
            dicom
                .artifacts
                .iter()
                .map(|artifact| projection_artifact_id(recipe, &artifact.logical_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let requirements = BTreeMap::from([
        (
            "features".into(),
            registry_case.requirements.features.clone(),
        ),
        (
            "external_codecs".into(),
            registry_case.requirements.external_codecs.clone(),
        ),
        (
            "external_validators".into(),
            registry_case.requirements.external_validators.clone(),
        ),
    ]);
    unavailable.push(UnavailableCapability {
        capability_id: format!("case_{}", registry_case.case_id.replace('/', "_")),
        kind,
        reason_code: reason_code.into(),
        message: message.clone(),
        affected_artifact_ids: artifact_ids.clone(),
        requirements,
    });
    pending.push(PendingCuratedCase {
        case_id: registry_case.case_id.clone(),
        recipe: recipe.identity(),
        reason_code: reason_code.into(),
        message,
        artifact_ids,
        standards_evidence: registry_case.standards_evidence.clone(),
    });
}

fn record_runtime_unavailable_case(
    registry_case: &RegistryCase,
    recipe: &CaseRecipe,
    runtime_unavailable: &[RuntimeUnavailableCapability],
    unavailable: &mut Vec<UnavailableCapability>,
    pending: &mut Vec<PendingCuratedCase>,
) {
    let artifact_ids = recipe
        .dicom
        .as_ref()
        .map(|dicom| {
            dicom
                .artifacts
                .iter()
                .map(|artifact| projection_artifact_id(recipe, &artifact.logical_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let requirements = BTreeMap::from([
        (
            "features".into(),
            registry_case.requirements.features.clone(),
        ),
        (
            "external_codecs".into(),
            registry_case.requirements.external_codecs.clone(),
        ),
        (
            "external_validators".into(),
            registry_case.requirements.external_validators.clone(),
        ),
    ]);
    for (index, item) in runtime_unavailable.iter().enumerate() {
        unavailable.push(UnavailableCapability {
            capability_id: format!(
                "case_{}_runtime_{index}",
                registry_case.case_id.replace('/', "_")
            ),
            kind: match item.kind {
                RuntimeCapabilityKind::CompileTimeFeature => CapabilityKind::Feature,
                RuntimeCapabilityKind::CodecBackend => CapabilityKind::Codec,
                RuntimeCapabilityKind::CodecExecutable => CapabilityKind::ExternalBackend,
                RuntimeCapabilityKind::ExternalValidator => CapabilityKind::Validator,
                RuntimeCapabilityKind::ExternalProvider => CapabilityKind::Provider,
                RuntimeCapabilityKind::RegistryContract => CapabilityKind::Codec,
            },
            reason_code: runtime_reason_code(&item.reason).into(),
            message: runtime_unavailable_message(item),
            affected_artifact_ids: artifact_ids.clone(),
            requirements: requirements.clone(),
        });
    }
    pending.push(PendingCuratedCase {
        case_id: registry_case.case_id.clone(),
        recipe: recipe.identity(),
        reason_code: "feature_gated_case_unavailable".into(),
        message: format!(
            "case requires unavailable build/runtime capabilities: {}",
            registry_case.requirements.summary()
        ),
        artifact_ids,
        standards_evidence: registry_case.standards_evidence.clone(),
    });
}

fn runtime_reason_code(reason: &UnavailableReason) -> &'static str {
    match reason {
        UnavailableReason::FeatureDisabled => "feature_disabled",
        UnavailableReason::CodecBackendUnavailable => "codec_backend_unavailable",
        UnavailableReason::ExecutableUnavailable => "codec_executable_unavailable",
        UnavailableReason::ExternalValidatorUnavailable => "external_validator_unavailable",
        UnavailableReason::ExternalProviderUnavailable => "external_provider_unavailable",
        UnavailableReason::RegistryContractInvalid(_) => "registry_contract_invalid",
    }
}

fn runtime_unavailable_message(item: &RuntimeUnavailableCapability) -> String {
    let detail = match &item.reason {
        UnavailableReason::FeatureDisabled => "compile-time feature is disabled".into(),
        UnavailableReason::CodecBackendUnavailable => {
            "codec backend was not present in the injected inventory".into()
        }
        UnavailableReason::ExecutableUnavailable => {
            "codec executable was not present in the injected inventory".into()
        }
        UnavailableReason::ExternalValidatorUnavailable => {
            "external validator was not present in the injected inventory".into()
        }
        UnavailableReason::ExternalProviderUnavailable => {
            "external provider was not present in the injected inventory".into()
        }
        UnavailableReason::RegistryContractInvalid(message) => message.clone(),
    };
    format!("{}: {detail}", item.capability_id)
}

fn selected_case_ids(
    registry: &RegistryDocument,
    selection: &CuratedScSelection,
) -> Result<BTreeSet<String>, CuratedPlanError> {
    match selection {
        CuratedScSelection::AllFeatureFree => Ok(registry
            .cases
            .iter()
            .filter(|case| {
                case.status == "implemented"
                    && case.requirements.is_feature_free()
                    && !case.profiles.is_empty()
                    && !case
                        .profiles
                        .iter()
                        .any(|profile| matches!(profile.as_str(), "stress" | "negative" | "fuzz"))
            })
            .map(|case| case.case_id.clone())
            .collect()),
        CuratedScSelection::Profile {
            profile,
            include_stress,
        } => {
            if !matches!(
                profile.as_str(),
                "smoke" | "core" | "extended" | "legacy" | "negative" | "fuzz" | "stress" | "all"
            ) {
                return Err(CuratedPlanError::UnknownProfile(profile.clone()));
            }
            Ok(registry
                .cases
                .iter()
                .filter(|case| {
                    case.status == "implemented"
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
            if let Some(mutation) = &recipe.mutation {
                let source_recipe = catalog
                    .recipes()
                    .get(&mutation.source.identity())
                    .ok_or_else(|| CuratedPlanError::MissingMutationSource {
                        recipe_id: recipe.recipe_id.clone(),
                        source: mutation.source.identity(),
                    })?;
                selected.insert(source_recipe.binding.case_id.clone());
            }
        }
        if selected.len() == before {
            return Ok(());
        }
    }
}

fn recipe_dependency_depth(recipe: &CaseRecipe, catalog: &RecipeCatalog) -> u32 {
    fn visit(
        recipe: &CaseRecipe,
        catalog: &RecipeCatalog,
        visiting: &mut BTreeSet<RecipeIdentity>,
    ) -> u32 {
        if !visiting.insert(recipe.identity()) {
            return 0;
        }
        let depth = recipe
            .dependencies
            .iter()
            .filter_map(|dependency| catalog.recipes().get(&dependency.recipe.identity()))
            .map(|dependency| visit(dependency, catalog, visiting).saturating_add(1))
            .max()
            .unwrap_or(0);
        visiting.remove(&recipe.identity());
        depth
    }
    visit(recipe, catalog, &mut BTreeSet::new())
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

    fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.features.is_empty() {
            parts.push(format!("features={}", self.features.join(",")));
        }
        if !self.external_codecs.is_empty() {
            parts.push(format!(
                "external_codecs={}",
                self.external_codecs.join(",")
            ));
        }
        if !self.external_validators.is_empty() {
            parts.push(format!(
                "external_validators={}",
                self.external_validators.join(",")
            ));
        }
        parts.join("; ")
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
    MissingMutation(String),
    MissingMutationSource {
        recipe_id: String,
        source: RecipeIdentity,
    },
    MissingMutationSourceRole {
        recipe_id: String,
        role: String,
    },
    MissingQualificationSource {
        recipe_id: String,
        source: RecipeIdentity,
        role: String,
    },
    NegativePlan {
        recipe_id: String,
        message: String,
    },
    QualificationPlan {
        recipe_id: String,
        message: String,
    },
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
