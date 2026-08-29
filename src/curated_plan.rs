//! Plan-only assembly for the feature-free curated Secondary Capture slice.
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
use crate::composition::{CompositionUidRole, ContentMaterialization, TemplateCatalog, TemplateId};
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
use crate::recipes::{
    CaseRecipe, MetadataScPlanInput, RecipeCatalog, SecondaryCapturePlanInput,
    encoding_plan_from_recipe, native_pixel_request_from_recipe, resolved_metadata_sc_plan,
    resolved_secondary_capture_plan,
};
use crate::sha256_hex;

const SC_PLAN_PROVIDER: &str = "native.sc_plan";
const METADATA_SC_PLAN_PROVIDER: &str = "native.metadata_sc_plan";
const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const EXPLICIT_VR_BE: &str = "1.2.840.10008.1.2.2";
const ARTIFACT_OVERHEAD_BYTES: u64 = 16 * 1024;

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
    /// Every implemented, feature-free recipe currently owned by the SC
    /// planner, including legacy-profile entries.
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
    pub pending: Vec<PendingCuratedCase>,
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
        let selected_ids = selected_case_ids(&self.registry, &request.selection)?;
        let mut artifacts = Vec::new();
        let mut bindings = BTreeMap::new();
        let mut native_content_requests = Vec::new();
        let pending = Vec::new();
        let mut artifact_by_recipe_role = BTreeMap::new();
        let mut selected_recipes = Vec::new();

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
                    evidence: EvidencePlan {
                        obligations: vec![EvidenceObligation {
                            obligation_id: "curated_generation_validation".into(),
                            route_id: "shared_corpus_executor".into(),
                            independence: EvidenceIndependence::SameProject,
                            required: true,
                            parameters: BTreeMap::new(),
                        }],
                    },
                    resources,
                }));
                artifact_by_recipe_role.insert(
                    (recipe.identity(), artifact_recipe.output.role.clone()),
                    global_id.clone(),
                );
                native_content_requests.push(NativeContentServiceRequest {
                    artifact_id: global_id.clone(),
                    slot: "pixels".into(),
                    factory_id: artifact_recipe.content.provider_id.clone(),
                    request: native_request,
                    unpadded_size_bytes: native.plan.unpadded_value_bytes,
                    unpadded_sha256: native.unpadded_sha256.clone(),
                    frame_sha256: native
                        .frames
                        .iter()
                        .map(|frame| frame.decoded_sha256.clone())
                        .collect(),
                });
                let execution_binding = if artifact_recipe.encoding.transfer_syntax_uid
                    == RLE_LOSSLESS_TRANSFER_SYNTAX_UID
                {
                    rle_execution_binding(&global_id, sc, &native)
                } else {
                    ArtifactExecutionBindings {
                        artifact_id: global_id.clone(),
                        slots: BTreeMap::from([(
                            "pixels".into(),
                            SlotExecutionBinding::NativeFrames {
                                frames: native_frame_bindings(sc, &native)?,
                            },
                        )]),
                    }
                };
                bindings.insert(global_id, execution_binding);
            }
        }

        let dependencies = dependencies(&selected_recipes, &artifact_by_recipe_role)?;
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
        Ok(CuratedScCorpusPlan {
            plan,
            bindings,
            native_content_requests,
            pending,
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
        .find(|content| content.slot == "pixels")
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
    sc: &crate::recipes::SecondaryCaptureParameters,
    native: &crate::native_pixel::NativePixelContent,
) -> ArtifactExecutionBindings {
    let frames = native
        .frames
        .iter()
        .map(|frame| NativeFrameBinding {
            frame_number: frame.frame_number,
            bytes: ByteBinding::Inline {
                bytes: frame.decoded_bytes.clone(),
                sha256: frame.decoded_sha256.clone(),
            },
            rows: sc.rows,
            columns: sc.columns,
            samples_per_pixel: sc.samples_per_pixel,
            bits_allocated: sc.bits_allocated,
            photometric_interpretation: sc.photometric_interpretation.clone(),
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
                        Value::from(sc.bits_stored),
                    )]),
                },
            },
        )]),
    }
}

fn native_frame_bindings(
    sc: &crate::recipes::SecondaryCaptureParameters,
    native: &crate::native_pixel::NativePixelContent,
) -> Result<Vec<NativeFrameBinding>, CuratedPlanError> {
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
                rows: sc.rows,
                columns: sc.columns,
                samples_per_pixel: sc.samples_per_pixel,
                bits_allocated: sc.bits_allocated,
                photometric_interpretation: sc.photometric_interpretation.clone(),
            })
        })
        .collect()
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

#[derive(Debug, Deserialize)]
struct RegistryCase {
    case_id: String,
    status: String,
    profiles: Vec<String>,
    requirements: RegistryRequirements,
}

#[derive(Debug, Deserialize)]
struct RegistryRequirements {
    features: Vec<String>,
    external_codecs: Vec<String>,
    external_validators: Vec<String>,
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
    Encoding {
        artifact_id: String,
        message: String,
    },
    MissingDependency {
        recipe: RecipeIdentity,
        dependency: RecipeIdentity,
        role: String,
    },
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
