use super::classic_vl_projection::{inspect_vl_photo_capability, inspect_xa_xrf_capability};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::classic_ct::inspect_ct_capability;
use super::classic_dx_mg::inspect_dx_mg_capability;
use super::classic_mr_cr::{inspect_cr_capability, inspect_mr_capability};
use super::classic_nuclear::{
    inspect_pet_capability, inspect_us_capability, inspect_us_multiframe_capability,
};
use super::codec_registry::{
    BACKENDS, TransferSyntaxBackendRegistry, encoding_provider_matches, recipe_encoding_provider_id,
};
use super::enhanced::{
    ENHANCED_ALGORITHM_PROVIDER_ID, ENHANCED_PLAN_PROVIDER_ID, EnhancedProviderInput,
    enhanced_input_from_recipe,
};
use super::error::RecipeCatalogError;
use super::model::{CaseRecipe, MetadataScParameters, RecipeKind, StringValueSource};
use super::presentation::{
    PRESENTATION_ADVANCED_PROVIDER_ID, PRESENTATION_ALGORITHM_PROVIDER_ID, PresentationPlanInput,
    PresentationSourceInput, presentation_input_from_recipe, validate_presentation_recipe,
};
use super::registration::{
    REGISTRATION_ALGORITHM_PROVIDER_ID, REGISTRATION_PLAN_PROVIDER_ID, RegistrationProviderInput,
    RegistrationSourceInput, registration_input_from_recipe, validate_registration_recipe,
};
use super::wsi::{
    WSI_ADVANCED_PROVIDER_ID, WSI_ALGORITHM_PROVIDER_ID, WsiPlanRecipe, wsi_input_from_recipe,
};
use crate::planning::RecipeIdentity;

#[path = "robustness.rs"]
mod robustness;
pub use robustness::{
    EOT_ARITHMETIC_PLAN_PROVIDER_ID, FUZZ_PLAN_PROVIDER_ID, FuzzBudgetContract, FuzzSource,
    PayloadPolicy, QualificationParameters, RobustnessProviderParameters, qualification_parameters,
};

const CASE_RECIPE_SCHEMA: &str = include_str!("../../schemas/case-recipe.schema.json");

/// Inspect an integrity-captured caller recipe using the same schema,
/// registered engine IDs, and shape rules as the embedded recipe catalog.
pub(crate) fn inspect_corpus_recipe(
    logical_path: &str,
    value: Value,
) -> Result<Vec<(String, String)>, RecipeCatalogError> {
    let path = Path::new(logical_path);
    let schema: Value = serde_json::from_str(CASE_RECIPE_SCHEMA).expect("embedded recipe schema");
    let validator = jsonschema::validator_for(&schema).expect("case recipe schema compiles");
    let errors = validator
        .iter_errors(&value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(RecipeCatalogError::Schema {
            path: path.to_path_buf(),
            errors,
        });
    }
    let recipe: CaseRecipe =
        serde_json::from_value(value).map_err(|error| RecipeCatalogError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    validate_shape(path, &recipe)?;
    Ok(recipe
        .dependencies
        .iter()
        .map(|dependency| {
            (
                dependency.recipe.recipe_id.clone(),
                dependency.recipe.recipe_version.clone(),
            )
        })
        .collect())
}

#[derive(Debug)]
pub struct RecipeCatalog {
    recipes: BTreeMap<RecipeIdentity, CaseRecipe>,
    bindings: BTreeMap<String, RecipeIdentity>,
    ordered: Vec<RecipeIdentity>,
}

#[derive(Debug, Deserialize)]
struct RegistryDocument {
    cases: Vec<RegistryCase>,
}

#[derive(Debug, Deserialize)]
struct RegistryCase {
    modality: Option<String>,
    case_id: String,
    status: String,
    profiles: Vec<String>,
    recipe_id: String,
    recipe_version: String,
    sop_class_uid: Option<String>,
    transfer_syntax_uid: Option<String>,
    artifact_kind: String,
    determinism: String,
    provider: RegistryProvider,
    requirements: RegistryRequirements,
}

#[derive(Debug, Deserialize)]
struct RegistryProvider {
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct RegistryRequirements {
    features: Vec<String>,
    external_codecs: Vec<String>,
    external_validators: Vec<String>,
}

#[derive(Debug, Clone)]
struct TemplateContract {
    status: String,
    sop_class_uid: String,
    transfer_syntax_uids: BTreeSet<String>,
    default_recipe: Option<TemplateDefaultRecipe>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateDefaultRecipe {
    recipe_id: String,
    recipe_version: String,
    artifact_logical_id: String,
}

impl RecipeCatalog {
    pub fn load(
        recipes_root: impl AsRef<Path>,
        registry_path: impl AsRef<Path>,
        template_catalog_path: impl AsRef<Path>,
    ) -> Result<Self, RecipeCatalogError> {
        let registry_path = registry_path.as_ref();
        let template_catalog_path = template_catalog_path.as_ref();
        let registry: RegistryDocument = read_typed(registry_path)?;
        let templates = load_templates(template_catalog_path)?;
        let codec_registry = TransferSyntaxBackendRegistry::load_committed().map_err(|error| {
            RecipeCatalogError::Completeness {
                message: error.to_string(),
            }
        })?;
        let paths = sorted_json_files(recipes_root.as_ref())?;
        let documents = paths
            .into_iter()
            .map(|path| {
                fs::read(&path)
                    .map(|bytes| (path.clone(), bytes))
                    .map_err(|error| RecipeCatalogError::Read {
                        path,
                        message: error.to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::assemble(
            registry,
            templates,
            codec_registry,
            documents,
            Some(template_catalog_path),
        )
    }

    /// Caller recipes are compatible with installed templates but need not own
    /// the installed catalog's unrelated default-recipe closure.
    pub(crate) fn from_verified_bundle(
        bundle: &crate::corpus_definition::CorpusDefinitionBundle,
        engine_root: &Path,
    ) -> Result<Self, RecipeCatalogError> {
        let registry_path = &bundle.manifest().registry.path;
        let registry = serde_json::from_slice(
            bundle
                .bytes(registry_path)
                .expect("verified registry capture"),
        )
        .map_err(|error| RecipeCatalogError::Parse {
            path: registry_path.into(),
            message: error.to_string(),
        })?;
        let templates = load_templates(&engine_root.join("templates/catalog.json"))?;
        let matrix_path = engine_root.join("transfer-syntax/capability-matrix.json");
        let matrix =
            fs::read_to_string(&matrix_path).map_err(|error| RecipeCatalogError::Read {
                path: matrix_path,
                message: error.to_string(),
            })?;
        let codecs =
            TransferSyntaxBackendRegistry::from_capability_matrix(&matrix).map_err(|error| {
                RecipeCatalogError::Completeness {
                    message: error.to_string(),
                }
            })?;
        let documents = bundle
            .manifest()
            .cases
            .iter()
            .map(|case| {
                (
                    PathBuf::from(&case.recipe.path),
                    bundle
                        .bytes(&case.recipe.path)
                        .expect("verified recipe capture")
                        .to_vec(),
                )
            })
            .collect();
        Self::assemble(registry, templates, codecs, documents, None)
    }

    fn assemble(
        registry: RegistryDocument,
        templates: BTreeMap<(String, String), TemplateContract>,
        codec_registry: TransferSyntaxBackendRegistry,
        documents: Vec<(PathBuf, Vec<u8>)>,
        engine_default_closure: Option<&Path>,
    ) -> Result<Self, RecipeCatalogError> {
        let schema: Value =
            serde_json::from_str(CASE_RECIPE_SCHEMA).expect("embedded recipe schema");
        let validator = jsonschema::validator_for(&schema).expect("case recipe schema compiles");
        let mut recipes = BTreeMap::new();
        let mut bindings = BTreeMap::new();

        for (path, bytes) in documents {
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|error| RecipeCatalogError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                })?;
            let errors = validator
                .iter_errors(&value)
                .map(|error| error.to_string())
                .collect::<Vec<_>>();
            if !errors.is_empty() {
                return Err(RecipeCatalogError::Schema { path, errors });
            }
            let recipe: CaseRecipe =
                serde_json::from_value(value).map_err(|error| RecipeCatalogError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                })?;
            validate_shape(&path, &recipe)?;
            let identity = recipe.identity();
            if recipes.insert(identity.clone(), recipe.clone()).is_some() {
                return Err(semantic(
                    &path,
                    format!("duplicate recipe identity {identity}"),
                ));
            }
            if let Some(previous) = bindings.insert(recipe.binding.case_id.clone(), identity) {
                return Err(semantic(
                    &path,
                    format!("duplicate case binding previously owned by {previous}"),
                ));
            }
        }

        validate_registry_bindings(&registry, &recipes, &bindings, &templates, &codec_registry)?;
        if let Some(template_catalog_path) = engine_default_closure {
            validate_template_default_recipes(template_catalog_path, &templates, &recipes)?;
        }
        validate_migrated_planning_orders(&recipes)?;
        validate_dependencies(&registry, &recipes)?;
        let ordered = topological_order(&recipes)?;
        Ok(Self {
            recipes,
            bindings,
            ordered,
        })
    }

    pub fn recipes(&self) -> &BTreeMap<RecipeIdentity, CaseRecipe> {
        &self.recipes
    }

    pub fn binding_for_case(&self, case_id: &str) -> Option<&RecipeIdentity> {
        self.bindings.get(case_id)
    }

    pub fn ordered_identities(&self) -> &[RecipeIdentity] {
        &self.ordered
    }

    pub fn enhanced_input_for_case(
        &self,
        case_id: &str,
    ) -> Result<Option<EnhancedProviderInput>, RecipeCatalogError> {
        let Some(identity) = self.bindings.get(case_id) else {
            return Ok(None);
        };
        enhanced_input_from_recipe(&self.recipes[identity]).map_err(|message| {
            RecipeCatalogError::Completeness {
                message: format!("{case_id}: {message}"),
            }
        })
    }

    pub fn wsi_input_for_case(
        &self,
        case_id: &str,
    ) -> Result<Option<WsiPlanRecipe>, RecipeCatalogError> {
        let Some(identity) = self.bindings.get(case_id) else {
            return Ok(None);
        };
        wsi_input_from_recipe(&self.recipes[identity]).map_err(|message| {
            RecipeCatalogError::Completeness {
                message: format!("{case_id}: {message}"),
            }
        })
    }

    pub fn registration_input_for_case(
        &self,
        case_id: &str,
        sources: Vec<RegistrationSourceInput>,
    ) -> Result<Option<RegistrationProviderInput>, RecipeCatalogError> {
        let Some(identity) = self.bindings.get(case_id) else {
            return Ok(None);
        };
        registration_input_from_recipe(&self.recipes[identity], sources).map_err(|message| {
            RecipeCatalogError::Completeness {
                message: format!("{case_id}: {message}"),
            }
        })
    }

    pub fn presentation_input_for_case(
        &self,
        case_id: &str,
        sources: Vec<PresentationSourceInput>,
    ) -> Result<Option<PresentationPlanInput>, RecipeCatalogError> {
        let Some(identity) = self.bindings.get(case_id) else {
            return Ok(None);
        };
        presentation_input_from_recipe(&self.recipes[identity], sources).map_err(|message| {
            RecipeCatalogError::Completeness {
                message: format!("{case_id}: {message}"),
            }
        })
    }
}

fn read_typed<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, RecipeCatalogError> {
    let bytes = fs::read(path).map_err(|error| RecipeCatalogError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| RecipeCatalogError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn sorted_json_files(root: &Path) -> Result<Vec<PathBuf>, RecipeCatalogError> {
    fn visit(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), RecipeCatalogError> {
        let entries = fs::read_dir(root).map_err(|error| RecipeCatalogError::Read {
            path: root.to_path_buf(),
            message: error.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| RecipeCatalogError::Read {
                path: root.to_path_buf(),
                message: error.to_string(),
            })?;
            let file_type = entry
                .file_type()
                .map_err(|error| RecipeCatalogError::Read {
                    path: entry.path(),
                    message: error.to_string(),
                })?;
            if file_type.is_symlink() {
                return Err(semantic(
                    &entry.path(),
                    "recipe catalog cannot contain symlinks",
                ));
            }
            if file_type.is_dir() {
                visit(&entry.path(), paths)?;
            } else if file_type.is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            {
                paths.push(entry.path());
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    visit(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn load_templates(
    path: &Path,
) -> Result<BTreeMap<(String, String), TemplateContract>, RecipeCatalogError> {
    let catalog: Value = read_typed(path)?;
    let mut contracts = BTreeMap::new();
    for section in [
        "templates",
        "classic_family_templates",
        "advanced_family_templates",
    ] {
        for descriptor in catalog[section].as_array().into_iter().flatten() {
            let id = required_string(path, descriptor, "template_id")?;
            let version = required_string(path, descriptor, "template_version")?;
            let status = required_string(path, descriptor, "status")?;
            let sop_class_uid = required_string(path, descriptor, "sop_class_uid")?;
            let transfer_syntax_uids = descriptor["transfer_syntaxes"]
                .as_array()
                .ok_or_else(|| semantic(path, format!("template {id} lacks transfer syntaxes")))?
                .iter()
                .map(|item| required_string(path, item, "uid"))
                .collect::<Result<BTreeSet<_>, _>>()?;
            let default_recipe = descriptor
                .get("default_recipe")
                .map(|value| {
                    serde_json::from_value(value.clone()).map_err(|error| {
                        semantic(
                            path,
                            format!("template {id} has invalid default_recipe: {error}"),
                        )
                    })
                })
                .transpose()?;
            if contracts
                .insert(
                    (id.clone(), version.clone()),
                    TemplateContract {
                        status,
                        sop_class_uid,
                        transfer_syntax_uids,
                        default_recipe,
                    },
                )
                .is_some()
            {
                return Err(semantic(path, format!("duplicate template {id}@{version}")));
            }
        }
    }
    Ok(contracts)
}

fn validate_template_default_recipes(
    path: &Path,
    templates: &BTreeMap<(String, String), TemplateContract>,
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
) -> Result<(), RecipeCatalogError> {
    let mut owners = BTreeMap::new();
    for ((template_id, template_version), template) in templates {
        let Some(binding) = &template.default_recipe else {
            continue;
        };
        let identity = RecipeIdentity {
            recipe_id: binding.recipe_id.clone(),
            recipe_version: binding.recipe_version.clone(),
        };
        let recipe = recipes.get(&identity).ok_or_else(|| {
            semantic(
                path,
                format!(
                    "template {template_id}@{template_version} default_recipe references unknown {identity}"
                ),
            )
        })?;
        let artifact = recipe
            .dicom
            .as_ref()
            .and_then(|dicom| {
                dicom
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.logical_id == binding.artifact_logical_id)
            })
            .ok_or_else(|| {
                semantic(
                    path,
                    format!(
                        "template {template_id}@{template_version} default_recipe artifact {} is missing from {identity}",
                        binding.artifact_logical_id
                    ),
                )
            })?;
        let Some(artifact_template) = &artifact.template else {
            return Err(semantic(
                path,
                format!(
                    "template {template_id}@{template_version} default_recipe artifact {} has no template identity",
                    binding.artifact_logical_id
                ),
            ));
        };
        if artifact_template.template_id != *template_id
            || artifact_template.template_version != *template_version
        {
            return Err(semantic(
                path,
                format!(
                    "template {template_id}@{template_version} default_recipe artifact {} resolves as {}@{}",
                    binding.artifact_logical_id,
                    artifact_template.template_id,
                    artifact_template.template_version
                ),
            ));
        }
        let key = (
            binding.recipe_id.as_str(),
            binding.recipe_version.as_str(),
            binding.artifact_logical_id.as_str(),
        );
        if let Some(previous) = owners.insert(key, (template_id, template_version)) {
            return Err(semantic(
                path,
                format!(
                    "default_recipe {}@{} artifact {} is shared by {}@{} and {template_id}@{template_version}",
                    binding.recipe_id,
                    binding.recipe_version,
                    binding.artifact_logical_id,
                    previous.0,
                    previous.1
                ),
            ));
        }
    }
    Ok(())
}

fn required_string(path: &Path, value: &Value, key: &str) -> Result<String, RecipeCatalogError> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| semantic(path, format!("{key} must be a string")))
}

fn validate_shape(path: &Path, recipe: &CaseRecipe) -> Result<(), RecipeCatalogError> {
    let shape_matches = matches!(
        (
            recipe.kind,
            recipe.dicom.is_some(),
            recipe.mutation.is_some(),
            recipe.qualification.is_some()
        ),
        (RecipeKind::Dicom, true, false, false)
            | (RecipeKind::Mutation, false, true, false)
            | (RecipeKind::Qualification, false, false, true)
    );
    if !shape_matches {
        return Err(semantic(path, "recipe kind and payload do not agree"));
    }
    if let Some(dicom) = &recipe.dicom {
        let mut logical_ids = BTreeSet::new();
        let mut orders = BTreeSet::new();
        for artifact in &dicom.artifacts {
            if !logical_ids.insert(&artifact.logical_id) {
                return Err(semantic(path, "artifact logical_id must be unique"));
            }
            if !orders.insert(artifact.order) {
                return Err(semantic(path, "artifact order must be unique"));
            }
            if let Some(output) = &artifact.output.path {
                let candidate = Path::new(output);
                if candidate.is_absolute()
                    || candidate
                        .components()
                        .any(|component| !matches!(component, Component::Normal(_)))
                {
                    return Err(semantic(path, format!("unsafe output path {output}")));
                }
            } else if artifact.output.provider_derived != Some(true) {
                return Err(semantic(
                    path,
                    "artifact output needs a safe path or provider_derived=true",
                ));
            }
        }
        if orders
            .iter()
            .copied()
            .ne(0..u32::try_from(orders.len()).unwrap_or(u32::MAX))
        {
            return Err(semantic(
                path,
                "artifact order must be contiguous and zero-based",
            ));
        }
    }
    validate_registered_ids(path, recipe)?;
    validate_secondary_capture_contract(path, recipe)?;
    validate_metadata_sc_contract(path, recipe)?;
    validate_classic_capability_contract(path, recipe)?;
    validate_advanced_contract(path, recipe)?;
    Ok(())
}

fn validate_classic_capability_contract(
    path: &Path,
    recipe: &CaseRecipe,
) -> Result<(), RecipeCatalogError> {
    if recipe.plan_provider_id != "native.classic_plan" {
        return Ok(());
    }
    let ct = inspect_ct_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let dx_mg =
        inspect_dx_mg_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let cr = inspect_cr_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let mr = inspect_mr_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let us = inspect_us_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let us_multiframe = inspect_us_multiframe_capability(recipe)
        .map_err(|error| semantic(path, error.to_string()))?;
    let pet = inspect_pet_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let xa_xrf =
        inspect_xa_xrf_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    let photo =
        inspect_vl_photo_capability(recipe).map_err(|error| semantic(path, error.to_string()))?;
    match ct.is_some()
        || dx_mg.is_some()
        || cr.is_some()
        || mr.is_some()
        || us.is_some()
        || us_multiframe.is_some()
        || pet.is_some()
        || xa_xrf.is_some()
        || photo.is_some()
    {
        true => recipe
            .planning_order
            .map(|_| ())
            .ok_or_else(|| semantic(path, "declared classic recipe requires planning_order")),
        false => {
            let case_id = recipe.binding.case_id.as_str();
            if case_id.starts_with("classic/")
                || case_id.starts_with("geometry/ct/")
                || (case_id.starts_with("vl/") && !case_id.starts_with("vl/wsi/"))
            {
                Ok(())
            } else {
                Err(semantic(
                    path,
                    "native.classic_plan has an unsupported case binding without a qualified tuple",
                ))
            }
        }
    }
}

fn validate_registered_ids(path: &Path, recipe: &CaseRecipe) -> Result<(), RecipeCatalogError> {
    const PLAN_PROVIDERS: &[&str] = &[
        "native.case_plan",
        "native.classic_plan",
        "native.sc_plan",
        "native.exceptional_sc_plan",
        "native.metadata_sc_plan",
        ENHANCED_PLAN_PROVIDER_ID,
        WSI_ADVANCED_PROVIDER_ID,
        REGISTRATION_PLAN_PROVIDER_ID,
        PRESENTATION_ADVANCED_PROVIDER_ID,
        "native.quantitative_plan",
        "external.quantitative_import_plan",
        "native.sr_plan",
        "external.highdicom_sr_import_plan",
        "native.rt_plan",
        "native.waveform_plan",
        "native.encapsulated_payload_plan",
        "native.stress_sc_plan",
        "native.stress_ct_plan",
        "external.import_plan",
        "mutation.named_plan",
        "qualification.bounded_plan",
        FUZZ_PLAN_PROVIDER_ID,
        EOT_ARITHMETIC_PLAN_PROVIDER_ID,
    ];
    const CONTENT_PROVIDERS: &[&str] = &[
        "content.case_default",
        "content.native_pixels",
        "content.empty_dataset",
        "content.sc.pixel_pattern",
        "content.metadata.person_name",
        "content.metadata.timezone_boundary",
        "content.metadata.empty_type2",
        "content.metadata.string_boundaries",
        "content.metadata.private_creators",
        "content.metadata.sequence_lengths",
        "content.neutral",
        "content.sr_semantics",
        "content.external_import",
        "content.rt_semantics",
        "content.waveform_samples",
        "content.declared_byte_payload",
        "content.stress.synthetic",
    ];
    const ALGORITHM_PROVIDERS: &[&str] = &[
        "algorithm.case_provider",
        "algorithm.classic_ct",
        "algorithm.classic_dx_mg",
        "algorithm.classic_mr_cr",
        "algorithm.classic_nuclear",
        "algorithm.classic_vl_projection",
        ENHANCED_ALGORITHM_PROVIDER_ID,
        WSI_ALGORITHM_PROVIDER_ID,
        REGISTRATION_ALGORITHM_PROVIDER_ID,
        PRESENTATION_ALGORITHM_PROVIDER_ID,
        "algorithm.quantitative",
        "algorithm.sr_content_tree",
        "algorithm.highdicom_sr",
        "algorithm.rt_semantics",
        "algorithm.waveform_deterministic_multiplex",
        "algorithm.encapsulated_pdf_minimal",
        "algorithm.binary_stl_tetrahedron",
        "algorithm.stress_sc",
        "algorithm.stress_ct",
    ];
    const ENCODING_PROVIDERS: &[&str] = &[
        "encoding.transfer_syntax_plan",
        "encoding.native.explicit_vr_big_endian",
        "encoding.native.rle_lossless",
    ];
    const VALIDATION_RULES: &[&str] = &[
        "validation.shared",
        "validation.independent_decode",
        "validation.mutation",
        "validation.qualification",
        "validation.sc.pixel",
        "validation.sc.palette",
        "validation.sc.padding",
        "validation.sc.color",
        "validation.sc.encapsulation",
        "validation.sc.eot",
        "validation.sc.geometry",
        "validation.classic.dx",
        "validation.classic.mammography",
        "validation.metadata.person_name",
        "validation.metadata.timezone",
        "validation.metadata.empty_type2",
        "validation.metadata.string_boundaries",
        "validation.metadata.private_creators",
        "validation.metadata.sequence_lengths",
        "validation.quantitative.seg",
        "validation.quantitative.rwvm",
        "validation.quantitative.external_import",
        "validation.sr",
        "validation.sr_external_import",
        "validation.rt",
        "validation.waveform.topology",
        "validation.waveform.samples",
        "validation.content.integrity",
        "validation.encapsulated_document",
        "validation.pdf.structure",
        "validation.manufacturing_model",
        "validation.stl.structure",
    ];
    const PROJECTION_RULES: &[&str] = &[
        "projection.curated",
        "projection.mutation",
        "projection.qualification",
        "projection.quantitative",
        "projection.sr",
        "projection.rt",
        "projection.waveform",
        "projection.encapsulated_document",
        "projection.encapsulated_mesh",
    ];
    let known = |id: &str, values: &[&str], kind: &str| {
        if values.contains(&id) {
            Ok(())
        } else {
            Err(semantic(path, format!("unknown {kind} id {id}")))
        }
    };
    known(&recipe.plan_provider_id, PLAN_PROVIDERS, "plan provider")?;
    for id in &recipe.validation_rule_ids {
        known(id, VALIDATION_RULES, "validation rule")?;
    }
    for id in &recipe.projection_rule_ids {
        known(id, PROJECTION_RULES, "projection rule")?;
    }
    if let Some(dicom) = &recipe.dicom {
        for artifact in &dicom.artifacts {
            known(
                &artifact.content.provider_id,
                CONTENT_PROVIDERS,
                "content provider",
            )?;
            if let Some(id) = &artifact.algorithm_provider_id {
                known(id, ALGORITHM_PROVIDERS, "algorithm provider")?;
            }
            if let Some(id) = &artifact.encoding.non_template_encoding_provider_id {
                let executable = TransferSyntaxBackendRegistry::load_committed()
                    .map_err(|error| semantic(path, error.to_string()))?;
                if !ENCODING_PROVIDERS.contains(&id.as_str())
                    && executable.for_backend_id(id).is_empty()
                    && !BACKENDS.iter().any(|backend| {
                        recipe_encoding_provider_id(backend.backend_id) == Some(id.as_str())
                    })
                {
                    return Err(semantic(path, format!("unknown encoding provider id {id}")));
                }
            }
            for id in &artifact.validation_rule_ids {
                known(id, VALIDATION_RULES, "validation rule")?;
            }
            for id in &artifact.projection_rule_ids {
                known(id, PROJECTION_RULES, "projection rule")?;
            }
        }
    }
    if recipe.mutation.is_some() {
        robustness::validate_mutation_contract(recipe)
            .map_err(|message| semantic(path, message))?;
    }
    if matches!(
        recipe.plan_provider_id.as_str(),
        FUZZ_PLAN_PROVIDER_ID | EOT_ARITHMETIC_PLAN_PROVIDER_ID
    ) {
        robustness::validate_qualification_contract(recipe)
            .map_err(|message| semantic(path, message))?;
    }
    Ok(())
}

fn validate_metadata_sc_contract(
    path: &Path,
    recipe: &CaseRecipe,
) -> Result<bool, RecipeCatalogError> {
    if recipe.plan_provider_id != "native.metadata_sc_plan" {
        if recipe.dicom.as_ref().is_some_and(|dicom| {
            dicom
                .artifacts
                .iter()
                .any(|artifact| artifact.metadata_sc.is_some())
        }) {
            return Err(semantic(
                path,
                "metadata_sc parameters require native.metadata_sc_plan",
            ));
        }
        return Ok(false);
    }
    if !recipe.provider_parameters.is_empty() {
        return Err(semantic(
            path,
            "native.metadata_sc_plan stores static values in typed artifact contracts",
        ));
    }
    let declared_kind = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .and_then(|artifact| artifact.metadata_sc.as_ref())
        .and_then(|metadata| match metadata {
            MetadataScParameters::PersonName(pn)
                if pn.specific_character_sets == ["ISO_IR 192"] =>
            {
                Some("person_name")
            }
            MetadataScParameters::EmptyType2 { .. } => Some("empty_type2"),
            MetadataScParameters::PrivateCreators { .. } => Some("private_creators"),
            _ => None,
        });
    let expected_kind = if let Some(kind) = declared_kind {
        kind
    } else {
        match recipe.binding.case_id.as_str() {
            "metadata/sc/utf8_person_name" | "metadata/sc/iso2022_person_name_component_groups" => {
                "person_name"
            }
            "metadata/sc/timezone_boundaries" => "timezone_boundary",
            "metadata/sc/empty_type2_attributes" => "empty_type2",
            "metadata/sc/long_multivalue_text_numeric_strings" => "string_boundaries",
            "metadata/sc/private_creator_blocks" => "private_creators",
            "metadata/sc/defined_undefined_sequence_lengths" => "sequence_lengths",
            _ => {
                return Err(semantic(
                    path,
                    "native.metadata_sc_plan has an unsupported case binding",
                ));
            }
        }
    };
    let expected_artifact_count =
        if matches!(expected_kind, "timezone_boundary" | "sequence_lengths") {
            2
        } else {
            1
        };
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| semantic(path, "native.metadata_sc_plan requires DICOM artifacts"))?;
    if dicom.artifacts.len() != expected_artifact_count {
        return Err(semantic(
            path,
            "metadata SC artifact count differs from its typed variant set",
        ));
    }
    for artifact in &dicom.artifacts {
        if declared_kind.is_some()
            && (!artifact.template.as_ref().is_some_and(|template| {
                template.template_id == "classic/secondary-capture/monochrome"
                    && template.template_version == "1.0.0"
            }) || !artifact.attribute_operations.is_empty()
                || artifact.classic_projection.is_some()
                || artifact.nonsquare_geometry.is_some()
                || artifact.encoding.sequence_length_policy != "default"
                || artifact.encoding.item_length_policy != "default")
        {
            return Err(semantic(
                path,
                "metadata3 requires the complete monochrome SC template@1 contract",
            ));
        }
        if artifact.output.path.is_none() || artifact.output.provider_derived == Some(true) {
            return Err(semantic(
                path,
                "metadata SC artifacts require exact output paths",
            ));
        }
        if !artifact.parameters.is_empty() || !artifact.content.parameters.is_empty() {
            return Err(semantic(
                path,
                "metadata SC cannot hide static values in untyped parameter maps",
            ));
        }
        if artifact.algorithm_provider_id.is_some() {
            return Err(semantic(
                path,
                "data-first metadata SC artifacts cannot name an algorithm provider",
            ));
        }
        if artifact.encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || artifact.encoding.offset_table_policy != "none"
            || artifact.encoding.fragmentation_policy != "native"
            || artifact.encoding.preamble_policy.as_deref() != Some("zero_filled")
            || artifact.encoding.file_meta_policy.as_deref() != Some("standard")
            || artifact
                .encoding
                .non_template_encoding_provider_id
                .is_some()
        {
            return Err(semantic(path, "metadata SC encoding policy is not exact"));
        }
        let pixels = artifact.secondary_capture.as_ref().ok_or_else(|| {
            semantic(
                path,
                "metadata SC requires a typed Secondary Capture pixel contract",
            )
        })?;
        if pixels.rows == 0
            || pixels.columns == 0
            || pixels.frames == 0
            || pixels.stored_values.is_empty()
            || pixels.frame_sha256.len() != pixels.frames as usize
        {
            return Err(semantic(path, "metadata SC pixel contract is incomplete"));
        }
        validate_metadata_pixel_contract(path, pixels)?;
        let metadata = artifact
            .metadata_sc
            .as_ref()
            .ok_or_else(|| semantic(path, "metadata SC requires typed metadata parameters"))?;
        let (actual_kind, content_provider, validation_rule) = match metadata {
            MetadataScParameters::PersonName(person_name) => {
                validate_person_name_metadata(path, person_name)?;
                if declared_kind.is_some()
                    || recipe.binding.case_id != "metadata/sc/iso2022_person_name_component_groups"
                {
                    let raw = decode_upper_hex(&person_name.patient_name_raw_hex)
                        .ok_or_else(|| semantic(path, "UTF-8 Person Name raw hex is invalid"))?;
                    if person_name.specific_character_sets != ["ISO_IR 192"]
                        || !person_name.native_unicode_round_trip
                        || raw != person_name.patient_name_decoded.as_bytes()
                        || person_name.patient_name_raw_hex.len() % 2 != 0
                    {
                        return Err(semantic(
                            path,
                            "UTF-8 Person Name must exactly round-trip its declared raw bytes",
                        ));
                    }
                }
                (
                    "person_name",
                    "content.metadata.person_name",
                    "validation.metadata.person_name",
                )
            }
            MetadataScParameters::TimezoneBoundary(boundary) => {
                if !boundary
                    .acquisition_date_time
                    .ends_with(&boundary.timezone_offset)
                {
                    return Err(semantic(
                        path,
                        "timezone boundary DT and Timezone Offset disagree",
                    ));
                }
                (
                    "timezone_boundary",
                    "content.metadata.timezone_boundary",
                    "validation.metadata.timezone",
                )
            }
            MetadataScParameters::EmptyType2 { attributes } => {
                let tags = attributes
                    .iter()
                    .map(|item| &item.tag)
                    .collect::<BTreeSet<_>>();
                let qualified = attributes.iter().all(|item| {
                    matches!(
                        (item.tag.as_str(), item.keyword.as_str(), item.vr.as_str()),
                        ("0010,0010", "PatientName", "PN")
                            | ("0010,0030", "PatientBirthDate", "DA")
                            | ("0010,0040", "PatientSex", "CS")
                            | ("0008,0090", "ReferringPhysicianName", "PN")
                            | ("0008,0050", "AccessionNumber", "SH")
                    )
                });
                if attributes.is_empty() || tags.len() != attributes.len() || !qualified {
                    return Err(semantic(
                        path,
                        "empty Type 2 attributes must be a nonempty unique subset of qualified tag/keyword/VR tuples",
                    ));
                }
                (
                    "empty_type2",
                    "content.metadata.empty_type2",
                    "validation.metadata.empty_type2",
                )
            }
            MetadataScParameters::StringBoundaries { elements } => {
                validate_string_boundaries(path, elements)?;
                (
                    "string_boundaries",
                    "content.metadata.string_boundaries",
                    "validation.metadata.string_boundaries",
                )
            }
            MetadataScParameters::PrivateCreators { blocks } => {
                validate_private_creators(path, blocks)?;
                (
                    "private_creators",
                    "content.metadata.private_creators",
                    "validation.metadata.private_creators",
                )
            }
            MetadataScParameters::SequenceLengths(sequence) => {
                let defined = sequence.variant_id == "defined";
                if sequence.item_length_field_hex != "FFFFFFFF"
                    || !sequence.item_delimitation_present
                    || (defined
                        && (sequence.sequence_length_field_hex != "38000000"
                            || sequence.sequence_delimitation_present))
                    || (!defined
                        && (sequence.sequence_length_field_hex != "FFFFFFFF"
                            || !sequence.sequence_delimitation_present))
                    || artifact.encoding.sequence_length_policy != sequence.variant_id
                    || artifact.encoding.item_length_policy != "undefined"
                {
                    return Err(semantic(
                        path,
                        "sequence-length metadata policy is inconsistent",
                    ));
                }
                (
                    "sequence_lengths",
                    "content.metadata.sequence_lengths",
                    "validation.metadata.sequence_lengths",
                )
            }
        };
        if actual_kind != expected_kind
            || artifact.content.provider_id != content_provider
            || !recipe
                .validation_rule_ids
                .iter()
                .any(|rule| rule == validation_rule)
            || !recipe
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.pixel")
            || !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == validation_rule)
            || !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.pixel")
        {
            return Err(semantic(
                path,
                "metadata SC kind, provider, or specialized validation rule disagrees",
            ));
        }
    }
    Ok(true)
}

fn validate_metadata_pixel_contract(
    path: &Path,
    pixels: &super::model::SecondaryCaptureParameters,
) -> Result<(), RecipeCatalogError> {
    if pixels.bits_stored == 0
        || pixels.bits_stored > pixels.bits_allocated
        || pixels.high_bit.checked_add(1) != Some(pixels.bits_stored)
    {
        return Err(semantic(path, "invalid metadata SC bit contract"));
    }
    let expected_type = match (pixels.bits_allocated, pixels.pixel_representation) {
        (1, 0) => "u1",
        (8, 0) => "u8",
        (16, 0) => "u16",
        (16, 1) => "i16",
        (32, 0) => "u32",
        _ => return Err(semantic(path, "unsupported metadata SC stored type")),
    };
    let samples = if pixels.photometric_interpretation == "YBR_FULL_422" {
        2_u64
    } else {
        u64::from(pixels.samples_per_pixel)
    };
    let expected_values = u64::from(pixels.rows)
        .checked_mul(u64::from(pixels.columns))
        .and_then(|value| value.checked_mul(u64::from(pixels.frames)))
        .and_then(|value| value.checked_mul(samples))
        .ok_or_else(|| semantic(path, "metadata SC value count overflow"))?;
    if pixels.stored_value_type != expected_type
        || u64::try_from(pixels.stored_values.len()).ok() != Some(expected_values)
        || pixels.pixel_min > pixels.pixel_max
        || pixels
            .stored_values
            .iter()
            .any(|value| *value < pixels.pixel_min || *value > pixels.pixel_max)
    {
        return Err(semantic(
            path,
            "metadata SC stored value contract is inconsistent",
        ));
    }
    Ok(())
}

fn validate_person_name_metadata(
    path: &Path,
    person_name: &super::model::PersonNameMetadata,
) -> Result<(), RecipeCatalogError> {
    let reconstructed = person_name
        .component_groups
        .iter()
        .map(|group| group.decoded_value.as_str())
        .collect::<Vec<_>>()
        .join("=");
    if reconstructed != person_name.patient_name_decoded {
        return Err(semantic(
            path,
            "Person Name component groups do not reconstruct PN",
        ));
    }
    if person_name
        .component_groups
        .iter()
        .any(|group| group.components.join("^").trim_end_matches('^') != group.decoded_value)
    {
        return Err(semantic(
            path,
            "Person Name components do not reconstruct their group",
        ));
    }
    let raw = decode_upper_hex(&person_name.patient_name_raw_hex)
        .ok_or_else(|| semantic(path, "Person Name raw hex is invalid"))?;
    if crate::sha256_hex(&raw) != person_name.patient_name_raw_sha256 {
        return Err(semantic(
            path,
            "Person Name raw hash differs from raw bytes",
        ));
    }
    Ok(())
}

fn decode_upper_hex(value: &str) -> Option<Vec<u8>> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let nibble = |byte: u8| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            };
            Some((nibble(pair[0])? << 4) | nibble(pair[1])?)
        })
        .collect()
}

fn validate_string_boundaries(
    path: &Path,
    elements: &[super::model::StringBoundaryElementMetadata],
) -> Result<(), RecipeCatalogError> {
    let tags = elements
        .iter()
        .map(|item| &item.tag)
        .collect::<BTreeSet<_>>();
    if elements.is_empty() || tags.len() != elements.len() {
        return Err(semantic(
            path,
            "string-boundary tags must be nonempty and unique",
        ));
    }
    for element in elements {
        let mut raw = match &element.source {
            StringValueSource::Repeated {
                pattern,
                repetitions,
            } => pattern.repeat(*repetitions as usize).into_bytes(),
            StringValueSource::Literal { values } => values.join("\\").into_bytes(),
        };
        if element.padding == "space" && raw.len() % 2 == 1 {
            raw.push(b' ');
        }
        if raw.len() != element.raw_value_byte_length as usize
            || crate::sha256_hex(&raw) != element.raw_value_sha256
        {
            return Err(semantic(
                path,
                "string-boundary source differs from declared raw length or hash",
            ));
        }
    }
    Ok(())
}

fn validate_private_creators(
    path: &Path,
    blocks: &[super::model::PrivateCreatorBlockMetadata],
) -> Result<(), RecipeCatalogError> {
    let creator_tags = blocks
        .iter()
        .map(|block| &block.creator_tag)
        .collect::<BTreeSet<_>>();
    if blocks.is_empty() || creator_tags.len() != blocks.len() {
        return Err(semantic(
            path,
            "private creator tags must be nonempty and unique",
        ));
    }
    for block in blocks {
        let element_tags = block
            .elements
            .iter()
            .map(|element| &element.tag)
            .collect::<BTreeSet<_>>();
        if element_tags.len() != block.elements.len()
            || block.elements.iter().any(|element| {
                element.tag < block.block_start_tag || element.tag > block.block_end_tag
            })
        {
            return Err(semantic(
                path,
                "private elements are duplicated or outside their creator block",
            ));
        }
    }
    Ok(())
}

fn validate_secondary_capture_contract(
    path: &Path,
    recipe: &CaseRecipe,
) -> Result<bool, RecipeCatalogError> {
    if !matches!(
        recipe.plan_provider_id.as_str(),
        "native.sc_plan" | "native.exceptional_sc_plan"
    ) {
        return Ok(false);
    }
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| semantic(path, "native.sc_plan requires DICOM artifacts"))?;
    if !recipe
        .validation_rule_ids
        .iter()
        .any(|rule| rule == "validation.sc.pixel")
    {
        return Err(semantic(
            path,
            "native.sc_plan requires validation.sc.pixel",
        ));
    }
    if !recipe.provider_parameters.is_empty() {
        return Err(semantic(
            path,
            "native.sc_plan stores its complete static contract on artifacts",
        ));
    }
    for artifact in &dicom.artifacts {
        if artifact.output.path.is_none() || artifact.output.provider_derived == Some(true) {
            return Err(semantic(
                path,
                "native.sc_plan artifacts require an exact output path",
            ));
        }
        if artifact.content.provider_id != "content.sc.pixel_pattern" {
            return Err(semantic(
                path,
                "native.sc_plan requires content.sc.pixel_pattern",
            ));
        }
        if !artifact.content.parameters.is_empty()
            || (recipe.plan_provider_id == "native.sc_plan" && !artifact.parameters.is_empty())
        {
            return Err(semantic(
                path,
                "native.sc_plan cannot hide static values in untyped parameter maps",
            ));
        }
        if artifact.algorithm_provider_id.is_some() {
            return Err(semantic(
                path,
                "data-first native.sc_plan artifacts cannot name an algorithm provider",
            ));
        }
        if artifact.encoding.preamble_policy.as_deref() != Some("zero_filled")
            || artifact.encoding.file_meta_policy.as_deref() != Some("standard")
        {
            return Err(semantic(
                path,
                "native.sc_plan requires exact preamble and file-meta policies",
            ));
        }
        if artifact.stressors.is_empty() {
            return Err(semantic(path, "native.sc_plan requires explicit stressors"));
        }
        let transfer_syntax_uid = artifact.encoding.transfer_syntax_uid.as_str();
        let declared_provider = artifact
            .encoding
            .non_template_encoding_provider_id
            .as_deref();
        let encoding_matches = if matches!(
            transfer_syntax_uid,
            "1.2.840.10008.1.2" | "1.2.840.10008.1.2.1"
        ) {
            declared_provider.is_none()
        } else {
            let registry = TransferSyntaxBackendRegistry::load_committed()
                .map_err(|error| semantic(path, error.to_string()))?;
            registry
                .for_transfer_syntax(transfer_syntax_uid)
                .zip(declared_provider)
                .is_some_and(|(backend, provider)| encoding_provider_matches(backend, provider))
        };
        if !encoding_matches {
            return Err(semantic(
                path,
                "native.sc_plan encoding provider differs from transfer syntax",
            ));
        }
        let sc = artifact.secondary_capture.as_ref().ok_or_else(|| {
            semantic(path, "native.sc_plan requires secondary_capture parameters")
        })?;
        if sc.bits_stored == 0
            || sc.bits_stored > sc.bits_allocated
            || sc.high_bit.checked_add(1) != Some(sc.bits_stored)
        {
            return Err(semantic(path, "invalid Secondary Capture bit contract"));
        }
        let expected_type = match (sc.bits_allocated, sc.pixel_representation) {
            (1, 0) => "u1",
            (8, 0) => "u8",
            (16, 0) => "u16",
            (16, 1) => "i16",
            (32, 0) => "u32",
            _ => return Err(semantic(path, "unsupported Secondary Capture stored type")),
        };
        if sc.stored_value_type != expected_type {
            return Err(semantic(
                path,
                "stored_value_type differs from the pixel bit contract",
            ));
        }
        let samples_per_pixel = if sc.photometric_interpretation == "YBR_FULL_422" {
            2_u64
        } else {
            u64::from(sc.samples_per_pixel)
        };
        let expected_values = u64::from(sc.rows)
            .checked_mul(u64::from(sc.columns))
            .and_then(|value| value.checked_mul(u64::from(sc.frames)))
            .and_then(|value| value.checked_mul(samples_per_pixel))
            .ok_or_else(|| semantic(path, "Secondary Capture value count overflow"))?;
        if u64::try_from(sc.stored_values.len()).ok() != Some(expected_values)
            || sc.frame_sha256.len() != sc.frames as usize
        {
            return Err(semantic(
                path,
                "Secondary Capture values or frame hashes differ from declared shape",
            ));
        }
        if sc.pixel_min > sc.pixel_max
            || sc
                .stored_values
                .iter()
                .any(|value| *value < sc.pixel_min || *value > sc.pixel_max)
        {
            return Err(semantic(
                path,
                "Secondary Capture stored values exceed declared range",
            ));
        }
        match (&sc.palette, sc.photometric_interpretation.as_str()) {
            (Some(palette), "PALETTE COLOR") => {
                if !artifact
                    .validation_rule_ids
                    .iter()
                    .any(|rule| rule == "validation.sc.palette")
                {
                    return Err(semantic(path, "palette contract lacks its validation rule"));
                }
                let entries = usize::try_from(palette.descriptor[0]).unwrap_or(usize::MAX);
                if palette.red.len() != entries
                    || palette.green.len() != entries
                    || palette.blue.len() != entries
                {
                    return Err(semantic(
                        path,
                        "palette channel length differs from descriptor",
                    ));
                }
            }
            (None, "PALETTE COLOR") | (Some(_), _) => {
                return Err(semantic(
                    path,
                    "palette parameters disagree with photometric mode",
                ));
            }
            _ => {}
        }
        let is_color = sc.samples_per_pixel > 1;
        if is_color != sc.color.is_some() {
            return Err(semantic(
                path,
                "color parameters disagree with Samples per Pixel",
            ));
        }
        if is_color
            && !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.color")
        {
            return Err(semantic(path, "color contract lacks its validation rule"));
        }
        if sc.padding.is_some()
            && !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.padding")
        {
            return Err(semantic(path, "padding contract lacks its validation rule"));
        }
        if artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.5"
            && !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.encapsulation")
        {
            return Err(semantic(
                path,
                "RLE Lossless contract lacks its encapsulation validation rule",
            ));
        }
        if artifact.encoding.offset_table_policy == "extended"
            && !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.eot")
        {
            return Err(semantic(
                path,
                "extended offset contract lacks its validation rule",
            ));
        }
        if artifact.attribute_operations.iter().any(|operation| {
            matches!(
                operation.tag.as_str(),
                "0018,1164" | "0020,0032" | "0020,0037" | "0028,0030" | "0028,0034"
            )
        }) && !artifact
            .validation_rule_ids
            .iter()
            .any(|rule| rule == "validation.sc.geometry")
        {
            return Err(semantic(
                path,
                "SC attribute operations lack their geometry validation rule",
            ));
        }
    }
    let bounded = is_name_independent_sc(recipe);
    if recipe.plan_provider_id == "native.sc_plan"
        && !bounded
        && !recipe.binding.case_id.starts_with("classic/sc/")
        && recipe.binding.case_id != "encapsulation/sc/eot_single_fragment_multiframe"
    {
        return Err(semantic(
            path,
            "native.sc_plan requires a qualified caller tuple or historical specialized binding",
        ));
    }
    Ok(bounded)
}

/// The already-validated, bounded native single-frame SC capability tuple.
fn is_name_independent_sc(recipe: &CaseRecipe) -> bool {
    if recipe.plan_provider_id != "native.sc_plan" || recipe.planning_order.is_none() {
        return false;
    }
    let Some(dicom) = &recipe.dicom else {
        return false;
    };
    let [artifact] = dicom.artifacts.as_slice() else {
        return false;
    };
    let Some(sc) = &artifact.secondary_capture else {
        return false;
    };
    let has_rule = |rule: &str| {
        recipe.validation_rule_ids.iter().any(|id| id == rule)
            && artifact.validation_rule_ids.iter().any(|id| id == rule)
    };
    let color = |planar: &[u8], subsampling: &str| {
        sc.color.as_ref().is_some_and(|color| {
            color
                .planar_configuration
                .is_some_and(|value| planar.contains(&value))
                && color.chroma_subsampling == subsampling
        })
    };
    let photometric = sc.photometric_interpretation.as_str();
    let mono = matches!(photometric, "MONOCHROME1" | "MONOCHROME2");
    let shape = match (
        photometric,
        sc.stored_value_type.as_str(),
        sc.samples_per_pixel,
    ) {
        ("MONOCHROME1", "u8", 1) | ("MONOCHROME2", "u8" | "u16" | "i16", 1) => {
            sc.color.is_none() && sc.palette.is_none()
        }
        ("PALETTE COLOR", "u8", 1) => {
            sc.color.is_none() && sc.palette.is_some() && sc.padding.is_none()
        }
        ("RGB", "u8", 3) => color(&[0, 1], "none") && sc.palette.is_none() && sc.padding.is_none(),
        ("YBR_FULL", "u8", 3) => {
            color(&[0], "none") && sc.palette.is_none() && sc.padding.is_none()
        }
        ("YBR_FULL_422", "u8", 3) => {
            color(&[0], "horizontal_2_to_1")
                && sc.columns % 2 == 0
                && sc.palette.is_none()
                && sc.padding.is_none()
        }
        _ => false,
    };
    let template_id = if mono {
        "classic/secondary-capture/monochrome"
    } else {
        "classic/secondary-capture/rgb"
    };
    shape
        && sc.rows > 0
        && sc.columns > 0
        && sc.frames == 1
        && sc.bit_packing.is_none()
        && sc.integer_word.is_none()
        && sc.encapsulation_projection.is_none()
        && sc.pixel_data_vr == if sc.bits_allocated == 8 { "OB" } else { "OW" }
        && artifact.template.as_ref().is_some_and(|template| {
            template.template_id == template_id && template.template_version == "1.0.0"
        })
        && artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.1"
        && artifact.encoding.sequence_length_policy == "default"
        && artifact.encoding.item_length_policy == "default"
        && artifact.encoding.offset_table_policy == "none"
        && artifact.encoding.fragmentation_policy == "native"
        && artifact.encoding.fragments_per_frame.is_none()
        && artifact
            .encoding
            .non_template_encoding_provider_id
            .is_none()
        && artifact.metadata_sc.is_none()
        && artifact.classic_projection.is_none()
        && artifact.nonsquare_geometry.is_none()
        && artifact.attribute_operations.is_empty()
        && has_rule("validation.sc.pixel")
        && (sc.padding.is_none() || has_rule("validation.sc.padding"))
        && (sc.palette.is_none() || has_rule("validation.sc.palette"))
        && (sc.color.is_none() || has_rule("validation.sc.color"))
        && recipe
            .projection_rule_ids
            .iter()
            .any(|id| id == "projection.curated")
        && artifact
            .projection_rule_ids
            .iter()
            .any(|id| id == "projection.curated")
}

fn validate_registry_bindings(
    registry: &RegistryDocument,
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
    bindings: &BTreeMap<String, RecipeIdentity>,
    templates: &BTreeMap<(String, String), TemplateContract>,
    codec_registry: &TransferSyntaxBackendRegistry,
) -> Result<(), RecipeCatalogError> {
    let implemented = registry
        .cases
        .iter()
        .filter(|case| case.status == "implemented")
        .collect::<Vec<_>>();
    let expected = implemented
        .iter()
        .map(|case| RecipeIdentity {
            recipe_id: case.recipe_id.clone(),
            recipe_version: case.recipe_version.clone(),
        })
        .collect::<BTreeSet<_>>();
    let actual = recipes.keys().cloned().collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(RecipeCatalogError::Completeness {
            message: format!(
                "implemented registry recipe set differs from recipe catalog; missing={:?}; extra={:?}",
                expected.difference(&actual).collect::<Vec<_>>(),
                actual.difference(&expected).collect::<Vec<_>>()
            ),
        });
    }
    for case in implemented {
        let identity = RecipeIdentity {
            recipe_id: case.recipe_id.clone(),
            recipe_version: case.recipe_version.clone(),
        };
        if bindings.get(&case.case_id) != Some(&identity) {
            return Err(RecipeCatalogError::Completeness {
                message: format!("{} does not bind exactly to {identity}", case.case_id),
            });
        }
        let recipe = &recipes[&identity];
        let negative = case.profiles.iter().any(|profile| profile == "negative");
        let fuzz = case.profiles.iter().any(|profile| profile == "fuzz");
        let expected_kind = if negative {
            RecipeKind::Mutation
        } else if fuzz || case.artifact_kind != "dicom_instance" {
            RecipeKind::Qualification
        } else {
            RecipeKind::Dicom
        };
        if recipe.kind != expected_kind {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "{} has {:?} recipe but registry boundary requires {:?}",
                    case.case_id, recipe.kind, expected_kind
                ),
            });
        }
        let expected_provider = match (case.provider.kind.as_str(), case.provider.id.as_str()) {
            (
                "external_backend",
                "highdicom_pydicom"
                | "cjxl_jpegxl_lossy_command_writer"
                | "openjph_htj2k_lossy_command_writer",
            ) => "external.import_plan",
            ("mutation_layer", "mutation_layer") if negative => "mutation.named_plan",
            ("mutation_layer", "bounded_deterministic_fuzz") => FUZZ_PLAN_PROVIDER_ID,
            ("rust_native", "checked_eot_arithmetic")
                if expected_kind == RecipeKind::Qualification =>
            {
                EOT_ARITHMETIC_PLAN_PROVIDER_ID
            }
            ("rust_native", "rust_native") => "native.case_plan",
            (kind, id) => {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{} has unsupported registry provider {kind}:{id}",
                        case.case_id
                    ),
                });
            }
        };
        let migrated_secondary_capture = recipe.plan_provider_id == "native.sc_plan"
            && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && expected_kind == RecipeKind::Dicom
            && case.requirements.features.is_empty()
            && case.requirements.external_codecs.is_empty()
            && (validate_secondary_capture_contract(Path::new(&recipe.recipe_id), recipe)?
                || case.case_id.starts_with("classic/sc/")
                || case.case_id == "encapsulation/sc/eot_single_fragment_multiframe");
        let migrated_exceptional_sc = recipe.plan_provider_id == "native.exceptional_sc_plan"
            && expected_kind == RecipeKind::Dicom
            && case.case_id.starts_with("classic/sc/")
            && ((case.provider.kind == "rust_native" && case.provider.id == "rust_native")
                || (case.provider.kind == "external_backend"
                    && matches!(
                        case.provider.id.as_str(),
                        "cjxl_jpegxl_lossy_command_writer" | "openjph_htj2k_lossy_command_writer"
                    )));
        let migrated_metadata_sc = recipe.plan_provider_id == "native.metadata_sc_plan"
            && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && expected_kind == RecipeKind::Dicom
            && case.requirements.features.is_empty()
            && case.requirements.external_codecs.is_empty()
            && validate_metadata_sc_contract(Path::new(&recipe.recipe_id), recipe)?;
        let name_independent_ct = recipe.plan_provider_id == "native.classic_plan"
            && inspect_ct_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid CT capability: {error}", case.case_id),
                })?
                .is_some();
        let name_independent_dx_mg = recipe.plan_provider_id == "native.classic_plan"
            && inspect_dx_mg_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid DX/MG capability: {error}", case.case_id),
                })?
                .is_some();
        let name_independent_cr = recipe.plan_provider_id == "native.classic_plan"
            && inspect_cr_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid CR capability: {error}", case.case_id),
                })?
                .is_some();
        let name_independent_mr = recipe.plan_provider_id == "native.classic_plan"
            && inspect_mr_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid MR capability: {error}", case.case_id),
                })?
                .is_some();
        if name_independent_mr && case.modality.as_deref() != Some("MR") {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "{} registry modality contradicts MR capability",
                    case.case_id
                ),
            });
        }
        let name_independent_us = recipe.plan_provider_id == "native.classic_plan"
            && inspect_us_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid US capability: {error}", case.case_id),
                })?
                .is_some();
        let name_independent_us_multiframe = recipe.plan_provider_id == "native.classic_plan"
            && inspect_us_multiframe_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!(
                        "{} has invalid US multi-frame capability: {error}",
                        case.case_id
                    ),
                })?
                .is_some();
        if (name_independent_us || name_independent_us_multiframe)
            && case.modality.as_deref() != Some("US")
        {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "{} registry modality contradicts US capability",
                    case.case_id
                ),
            });
        }
        let name_independent_pet = recipe.plan_provider_id == "native.classic_plan"
            && inspect_pet_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid PET capability: {error}", case.case_id),
                })?
                .is_some();
        let name_independent_xa_xrf = recipe.plan_provider_id == "native.classic_plan"
            && inspect_xa_xrf_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!("{} has invalid XA/XRF capability: {error}", case.case_id),
                })?
                .is_some();
        if name_independent_xa_xrf {
            let parameters = inspect_xa_xrf_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: error.to_string(),
                })?
                .expect("inspected XA/XRF");
            if case.modality.as_deref() != Some(parameters.modality.as_str()) {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{} registry modality contradicts XA/XRF capability",
                        case.case_id
                    ),
                });
            }
        }
        let name_independent_photo = recipe.plan_provider_id == "native.classic_plan"
            && inspect_vl_photo_capability(recipe)
                .map_err(|error| RecipeCatalogError::Completeness {
                    message: format!(
                        "{} has invalid photographic capability: {error}",
                        case.case_id
                    ),
                })?
                .is_some();
        if name_independent_photo && case.modality.as_deref() != Some("XC") {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "{} registry modality contradicts photographic capability",
                    case.case_id
                ),
            });
        }
        let migrated_classic = recipe.plan_provider_id == "native.classic_plan"
            && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && expected_kind == RecipeKind::Dicom
            && case.requirements.features.is_empty()
            && case.requirements.external_codecs.is_empty()
            && (name_independent_ct
                || name_independent_dx_mg
                || name_independent_cr
                || name_independent_mr
                || name_independent_us
                || name_independent_us_multiframe
                || name_independent_pet
                || name_independent_xa_xrf
                || name_independent_photo
                || case.case_id.starts_with("classic/")
                || case.case_id.starts_with("geometry/ct/")
                || (case.case_id.starts_with("vl/") && !case.case_id.starts_with("vl/wsi/")));
        let migrated_advanced = matches!(
            recipe.plan_provider_id.as_str(),
            ENHANCED_PLAN_PROVIDER_ID
                | WSI_ADVANCED_PROVIDER_ID
                | REGISTRATION_PLAN_PROVIDER_ID
                | PRESENTATION_ADVANCED_PROVIDER_ID
        ) && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && expected_kind == RecipeKind::Dicom
            && case.requirements.features.is_empty()
            && case.requirements.external_codecs.is_empty();
        let migrated_u6_native = matches!(
            recipe.plan_provider_id.as_str(),
            "native.quantitative_plan"
                | "native.sr_plan"
                | "native.rt_plan"
                | "native.waveform_plan"
                | "native.encapsulated_payload_plan"
        ) && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && expected_kind == RecipeKind::Dicom;
        let migrated_u6_external = matches!(
            recipe.plan_provider_id.as_str(),
            "external.quantitative_import_plan" | "external.highdicom_sr_import_plan"
        ) && case.provider.kind == "external_backend"
            && case.provider.id == "highdicom_pydicom"
            && expected_kind == RecipeKind::Dicom;
        let migrated_stress_sc = recipe.plan_provider_id == "native.stress_sc_plan"
            && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && case.case_id.starts_with("stress/sc/")
            && expected_kind == RecipeKind::Dicom;
        let migrated_stress_ct = recipe.plan_provider_id == "native.stress_ct_plan"
            && case.provider.kind == "rust_native"
            && case.provider.id == "rust_native"
            && case.case_id == "stress/study/high_instance_count_ct"
            && expected_kind == RecipeKind::Dicom;
        if recipe.plan_provider_id != expected_provider
            && !migrated_secondary_capture
            && !migrated_exceptional_sc
            && !migrated_metadata_sc
            && !migrated_classic
            && !migrated_advanced
            && !migrated_u6_native
            && !migrated_u6_external
            && !migrated_stress_sc
            && !migrated_stress_ct
        {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "{} plan provider {} is incompatible with registry provider {}:{}",
                    case.case_id, recipe.plan_provider_id, case.provider.kind, case.provider.id
                ),
            });
        }
        if let Some(dicom) = &recipe.dicom {
            let registry_sop =
                case.sop_class_uid
                    .as_deref()
                    .ok_or_else(|| RecipeCatalogError::Completeness {
                        message: format!("{} lacks registry SOP Class UID", case.case_id),
                    })?;
            let registry_ts = case.transfer_syntax_uid.as_deref().ok_or_else(|| {
                RecipeCatalogError::Completeness {
                    message: format!("{} lacks registry transfer syntax UID", case.case_id),
                }
            })?;
            if case.provider.kind == "rust_native"
                || !case.requirements.features.is_empty()
                || !case.requirements.external_codecs.is_empty()
            {
                codec_registry
                    .validate_registry_requirements(
                        registry_ts,
                        &case.determinism,
                        &case.requirements.features,
                        &case.requirements.external_codecs,
                    )
                    .map_err(|error| RecipeCatalogError::Completeness {
                        message: format!("{}: {error}", case.case_id),
                    })?;
            }
            for artifact in &dicom.artifacts {
                if artifact.encoding.transfer_syntax_uid != registry_ts {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!("{} transfer syntax differs from registry", case.case_id),
                    });
                }
                if artifact.determinism != case.determinism {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!("{} determinism differs from registry", case.case_id),
                    });
                }
                let Some(template_ref) = &artifact.template else {
                    if artifact
                        .encoding
                        .non_template_encoding_provider_id
                        .is_none()
                    {
                        return Err(RecipeCatalogError::Completeness {
                            message: format!(
                                "{} non-template artifact lacks encoding provider",
                                case.case_id
                            ),
                        });
                    }
                    continue;
                };
                let key = (
                    template_ref.template_id.clone(),
                    template_ref.template_version.clone(),
                );
                let contract =
                    templates
                        .get(&key)
                        .ok_or_else(|| RecipeCatalogError::Completeness {
                            message: format!(
                                "{} references unknown template {}@{}",
                                case.case_id, key.0, key.1
                            ),
                        })?;
                if contract.sop_class_uid != registry_sop {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!(
                            "{} template SOP Class differs from registry",
                            case.case_id
                        ),
                    });
                }
                if contract.status != "qualified" {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!(
                            "{} references template {}@{} with non-qualified status {}",
                            case.case_id, key.0, key.1, contract.status
                        ),
                    });
                }
                if !contract.transfer_syntax_uids.contains(registry_ts)
                    && artifact
                        .encoding
                        .non_template_encoding_provider_id
                        .is_none()
                {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!(
                            "{} template does not qualify registry transfer syntax",
                            case.case_id
                        ),
                    });
                }
                if contract.transfer_syntax_uids.contains(registry_ts)
                    && artifact
                        .encoding
                        .non_template_encoding_provider_id
                        .is_some()
                {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!("{} declares unnecessary encoding provider", case.case_id),
                    });
                }
            }
            let has_runtime_requirement = !case.requirements.features.is_empty()
                || !case.requirements.external_codecs.is_empty();
            if has_runtime_requirement
                && dicom.artifacts.iter().all(|artifact| {
                    artifact
                        .encoding
                        .non_template_encoding_provider_id
                        .is_none()
                })
            {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{} requirements are absent from encoding dispatch",
                        case.case_id
                    ),
                });
            }
            if !case.requirements.external_validators.is_empty()
                && !recipe
                    .validation_rule_ids
                    .iter()
                    .any(|rule| rule == "validation.independent_decode")
            {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{} independent validator requirement is not attached",
                        case.case_id
                    ),
                });
            }
        }
    }
    Ok(())
}

fn validate_migrated_planning_orders(
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
) -> Result<(), RecipeCatalogError> {
    let mut owners = BTreeMap::new();
    let mut projection_owners = BTreeMap::new();
    for recipe in recipes.values().filter(|recipe| {
        matches!(
            recipe.plan_provider_id.as_str(),
            "native.sc_plan"
                | "native.exceptional_sc_plan"
                | "native.metadata_sc_plan"
                | "native.classic_plan"
                | ENHANCED_PLAN_PROVIDER_ID
                | WSI_ADVANCED_PROVIDER_ID
                | REGISTRATION_PLAN_PROVIDER_ID
                | PRESENTATION_ADVANCED_PROVIDER_ID
                | "native.quantitative_plan"
                | "external.quantitative_import_plan"
                | "native.sr_plan"
                | "external.highdicom_sr_import_plan"
                | "native.rt_plan"
                | "native.waveform_plan"
                | "native.encapsulated_payload_plan"
                | "native.stress_sc_plan"
                | "native.stress_ct_plan"
        )
    }) {
        let order = recipe
            .planning_order
            .ok_or_else(|| RecipeCatalogError::Completeness {
                message: format!(
                    "{} requires planning_order for migrated provider {}",
                    recipe.binding.case_id, recipe.plan_provider_id
                ),
            })?;
        if let Some(previous) = owners.insert(order, recipe.binding.case_id.as_str()) {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "planning_order {order} is shared by migrated cases {previous} and {}",
                    recipe.binding.case_id
                ),
            });
        }
        if let Some(projection_order) = recipe.projection_order {
            if let Some(previous) =
                projection_owners.insert(projection_order, recipe.binding.case_id.as_str())
            {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "projection_order {projection_order} is shared by migrated cases {previous} and {}",
                        recipe.binding.case_id
                    ),
                });
            }
        }
    }
    Ok(())
}

fn validate_advanced_contract(path: &Path, recipe: &CaseRecipe) -> Result<(), RecipeCatalogError> {
    let (content_provider, algorithm_provider) = match recipe.plan_provider_id.as_str() {
        ENHANCED_PLAN_PROVIDER_ID => ("content.native_pixels", ENHANCED_ALGORITHM_PROVIDER_ID),
        WSI_ADVANCED_PROVIDER_ID => ("content.native_pixels", WSI_ALGORITHM_PROVIDER_ID),
        REGISTRATION_PLAN_PROVIDER_ID => {
            ("content.empty_dataset", REGISTRATION_ALGORITHM_PROVIDER_ID)
        }
        PRESENTATION_ADVANCED_PROVIDER_ID => {
            ("content.empty_dataset", PRESENTATION_ALGORITHM_PROVIDER_ID)
        }
        _ => return Ok(()),
    };
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| semantic(path, "advanced plan provider requires DICOM artifacts"))?;
    let mut paths = BTreeSet::new();
    for artifact in &dicom.artifacts {
        let output = artifact.output.path.as_ref().ok_or_else(|| {
            semantic(
                path,
                format!("{} requires an exact output path", artifact.logical_id),
            )
        })?;
        if artifact.output.provider_derived == Some(true) || !paths.insert(output) {
            return Err(semantic(
                path,
                "advanced output paths must be explicit and unique",
            ));
        }
        if artifact.template.is_none()
            || artifact.content.provider_id != content_provider
            || artifact.algorithm_provider_id.as_deref() != Some(algorithm_provider)
        {
            return Err(semantic(
                path,
                format!(
                    "{} has mismatched advanced provider bindings",
                    artifact.logical_id
                ),
            ));
        }
        let encoding = &artifact.encoding;
        if encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || encoding.sequence_length_policy != "default"
            || encoding.item_length_policy != "default"
            || encoding.offset_table_policy != "none"
            || encoding.fragmentation_policy != "native"
            || encoding.preamble_policy.as_deref() != Some("zero_filled")
            || encoding.file_meta_policy.as_deref() != Some("standard")
            || encoding.non_template_encoding_provider_id.is_some()
        {
            return Err(semantic(
                path,
                format!(
                    "{} has unresolved or incompatible encoding",
                    artifact.logical_id
                ),
            ));
        }
    }
    match recipe.plan_provider_id.as_str() {
        ENHANCED_PLAN_PROVIDER_ID => {
            enhanced_input_from_recipe(recipe).map_err(|message| semantic(path, message))?;
        }
        WSI_ADVANCED_PROVIDER_ID => {
            let input = wsi_input_from_recipe(recipe).map_err(|message| semantic(path, message))?;
            let input = input.expect("provider ID selected WSI input");
            let file_indices = input
                .artifacts
                .iter()
                .map(|artifact| artifact.file_index)
                .collect::<BTreeSet<_>>();
            let levels = input
                .artifacts
                .iter()
                .map(|artifact| artifact.level)
                .collect::<BTreeSet<_>>();
            if file_indices.len() != input.artifacts.len() || levels.len() != input.artifacts.len()
            {
                return Err(semantic(path, "WSI file indices and levels must be unique"));
            }
            if input.dependency_mode != super::wsi::WsiDependencyMode::None
                && input.artifacts.len() < 2
            {
                return Err(semantic(
                    path,
                    "WSI dependency mode requires multiple artifacts",
                ));
            }
        }
        REGISTRATION_PLAN_PROVIDER_ID => {
            validate_registration_recipe(recipe).map_err(|message| semantic(path, message))?;
        }
        PRESENTATION_ADVANCED_PROVIDER_ID => {
            validate_presentation_recipe(recipe).map_err(|message| semantic(path, message))?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn validate_dependencies(
    registry: &RegistryDocument,
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
) -> Result<(), RecipeCatalogError> {
    let registry_by_case = registry
        .cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    for (identity, recipe) in recipes {
        let dependencies = recipe
            .dependencies
            .iter()
            .map(|dependency| dependency.recipe.identity())
            .collect::<BTreeSet<_>>();
        if dependencies.len() != recipe.dependencies.len() {
            return Err(RecipeCatalogError::Completeness {
                message: format!("{identity} has duplicate dependency recipes"),
            });
        }
        for dependency in &dependencies {
            if dependency == identity || !recipes.contains_key(dependency) {
                return Err(RecipeCatalogError::Completeness {
                    message: format!("{identity} has invalid dependency {dependency}"),
                });
            }
        }
        if let Some(mutation) = &recipe.mutation {
            let source = mutation.source.identity();
            if !dependencies.contains(&source) {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{identity} mutation source {source} is not a declared dependency"
                    ),
                });
            }
            if recipes.get(&source).map(|source| source.kind) != Some(RecipeKind::Dicom) {
                return Err(RecipeCatalogError::Completeness {
                    message: format!("{identity} mutation source {source} is not valid DICOM"),
                });
            }
            if dependencies.len() != 1 {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{identity} mutation must declare exactly its source dependency"
                    ),
                });
            }
            let source_recipe = &recipes[&source];
            let logical_ids = source_recipe
                .dicom
                .as_ref()
                .into_iter()
                .flat_map(|dicom| &dicom.artifacts)
                .map(|artifact| artifact.logical_id.as_str())
                .collect::<BTreeSet<_>>();
            if !logical_ids.contains(mutation.source_logical_role.as_str()) {
                return Err(RecipeCatalogError::Completeness {
                    message: format!(
                        "{identity} mutation source {source} has no artifact logical role {}",
                        mutation.source_logical_role
                    ),
                });
            }
            validate_robustness_source_profile(identity, source_recipe, &registry_by_case)?;
        }
        if recipe.plan_provider_id == FUZZ_PLAN_PROVIDER_ID {
            let parameters = robustness::qualification_parameters(recipe).map_err(|message| {
                RecipeCatalogError::Completeness {
                    message: format!("{identity}: {message}"),
                }
            })?;
            let QualificationParameters::BoundedDeterministicFuzz { sources, .. } = parameters
            else {
                unreachable!("shape validation matched fuzz provider")
            };
            for declared in sources {
                let source = declared.recipe.identity();
                let source_recipe = &recipes[&source];
                let exists = source_recipe.dicom.as_ref().is_some_and(|dicom| {
                    dicom
                        .artifacts
                        .iter()
                        .any(|artifact| artifact.logical_id == declared.artifact_logical_id)
                });
                if !exists {
                    return Err(RecipeCatalogError::Completeness {
                        message: format!(
                            "{identity} fuzz source {source} lacks artifact {}",
                            declared.artifact_logical_id
                        ),
                    });
                }
                validate_robustness_source_profile(identity, source_recipe, &registry_by_case)?;
            }
        }
    }
    Ok(())
}

fn validate_robustness_source_profile(
    owner: &RecipeIdentity,
    source: &CaseRecipe,
    registry_by_case: &BTreeMap<&str, &RegistryCase>,
) -> Result<(), RecipeCatalogError> {
    let Some(registry_case) = registry_by_case.get(source.binding.case_id.as_str()) else {
        return Err(RecipeCatalogError::Completeness {
            message: format!(
                "{owner} source {} is absent from the registry",
                source.identity()
            ),
        });
    };
    if registry_case.artifact_kind != "dicom_instance"
        || registry_case
            .profiles
            .iter()
            .any(|profile| matches!(profile.as_str(), "negative" | "fuzz" | "stress"))
    {
        return Err(RecipeCatalogError::Completeness {
            message: format!(
                "{owner} source {} crosses an invalid robustness profile boundary",
                source.identity()
            ),
        });
    }
    Ok(())
}

fn topological_order(
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
) -> Result<Vec<RecipeIdentity>, RecipeCatalogError> {
    let mut remaining = recipes.keys().cloned().collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|identity| {
                recipes[*identity]
                    .dependencies
                    .iter()
                    .all(|dependency| !remaining.contains(&dependency.recipe.identity()))
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(RecipeCatalogError::Completeness {
                message: format!(
                    "recipe dependency graph contains a cycle involving {remaining:?}"
                ),
            });
        }
        for identity in ready {
            remaining.remove(&identity);
            ordered.push(identity);
        }
    }
    Ok(ordered)
}

fn semantic(path: &Path, message: impl Into<String>) -> RecipeCatalogError {
    RecipeCatalogError::Semantic {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::model::{
        CaseBinding, DependencyBinding, QualificationRecipe, RecipeReference, ResourcePolicy,
    };

    fn qualification(id: &str, dependency: Option<&str>) -> CaseRecipe {
        CaseRecipe {
            case_recipe_schema_version: "0.1.0".into(),
            recipe_id: id.into(),
            recipe_version: "0.1.0".into(),
            binding: CaseBinding {
                case_id: format!("fixture/{id}"),
            },
            kind: RecipeKind::Qualification,
            plan_provider_id: "qualification.bounded_plan".into(),
            planning_order: None,
            projection_order: None,
            provider_parameters: Default::default(),
            dependencies: dependency
                .map(|dependency| DependencyBinding {
                    recipe: RecipeReference {
                        recipe_id: dependency.into(),
                        recipe_version: "0.1.0".into(),
                    },
                    role: "source".into(),
                })
                .into_iter()
                .collect(),
            validation_rule_ids: vec!["validation.qualification".into()],
            projection_rule_ids: vec!["projection.qualification".into()],
            dicom: None,
            mutation: None,
            qualification: Some(QualificationRecipe {
                parameters: Default::default(),
                resource_policy: ResourcePolicy {
                    max_input_bytes: 0,
                    max_output_bytes: 0,
                    max_operations: 1,
                },
                retention: "evidence_only".into(),
            }),
        }
    }

    #[test]
    fn dependency_cycle_is_rejected() {
        let recipes = [qualification("a", Some("b")), qualification("b", Some("a"))]
            .into_iter()
            .map(|recipe| (recipe.identity(), recipe))
            .collect::<BTreeMap<_, _>>();
        let error = topological_order(&recipes).unwrap_err();
        assert!(error.to_string().contains("cycle"));
    }

    #[test]
    fn lexical_identity_breaks_topological_ties() {
        let recipes = [qualification("b", None), qualification("a", None)]
            .into_iter()
            .map(|recipe| (recipe.identity(), recipe))
            .collect::<BTreeMap<_, _>>();
        let order = topological_order(&recipes).unwrap();
        assert_eq!(order[0].recipe_id, "a");
        assert_eq!(order[1].recipe_id, "b");
    }

    #[test]
    fn unknown_plan_provider_is_rejected() {
        let mut recipe = qualification("provider", None);
        recipe.plan_provider_id = "unknown.provider".into();
        let error = validate_registered_ids(Path::new("fixture.json"), &recipe).unwrap_err();
        assert!(error.to_string().contains("unknown plan provider"));
    }

    fn robustness_fixture() -> (RegistryDocument, BTreeMap<RecipeIdentity, CaseRecipe>) {
        let source: CaseRecipe = serde_json::from_str(include_str!(
            "../../cases/recipes/classic/sc/sc_mono2_u8.json"
        ))
        .unwrap();
        let mutation: CaseRecipe = serde_json::from_str(include_str!(
            "../../cases/recipes/negative/iod/negative_iod_missing_type1_attribute.json"
        ))
        .unwrap();
        let registry: RegistryDocument = serde_json::from_value(serde_json::json!({
            "cases": [
                {
                    "case_id": source.binding.case_id,
                    "status": "implemented",
                    "profiles": ["core"],
                    "recipe_id": source.recipe_id,
                    "recipe_version": source.recipe_version,
                    "sop_class_uid": null,
                    "transfer_syntax_uid": null,
                    "artifact_kind": "dicom_instance",
                    "determinism": "byte_stable",
                    "provider": {"kind": "rust_native", "id": "rust_native"},
                    "requirements": {"features": [], "external_codecs": [], "external_validators": []}
                },
                {
                    "case_id": mutation.binding.case_id,
                    "status": "implemented",
                    "profiles": ["negative"],
                    "recipe_id": mutation.recipe_id,
                    "recipe_version": mutation.recipe_version,
                    "sop_class_uid": null,
                    "transfer_syntax_uid": null,
                    "artifact_kind": "negative_instance",
                    "determinism": "byte_stable",
                    "provider": {"kind": "mutation_layer", "id": "mutation_layer"},
                    "requirements": {"features": [], "external_codecs": [], "external_validators": []}
                }
            ]
        }))
        .unwrap();
        let recipes = [source, mutation]
            .into_iter()
            .map(|recipe| (recipe.identity(), recipe))
            .collect();
        (registry, recipes)
    }

    #[test]
    fn robustness_source_requires_exact_version_and_artifact_role() {
        let (registry, recipes) = robustness_fixture();
        validate_dependencies(&registry, &recipes).unwrap();

        let mut wrong_version = recipes.clone();
        let mutation_id = wrong_version
            .keys()
            .find(|identity| identity.recipe_id.starts_with("negative_"))
            .unwrap()
            .clone();
        wrong_version
            .get_mut(&mutation_id)
            .unwrap()
            .mutation
            .as_mut()
            .unwrap()
            .source
            .recipe_version = "9.9.9".into();
        assert!(
            validate_dependencies(&registry, &wrong_version)
                .unwrap_err()
                .to_string()
                .contains("not a declared dependency")
        );

        let mut missing_role = recipes.clone();
        missing_role
            .get_mut(&mutation_id)
            .unwrap()
            .mutation
            .as_mut()
            .unwrap()
            .source_logical_role = "missing".into();
        assert!(
            validate_dependencies(&registry, &missing_role)
                .unwrap_err()
                .to_string()
                .contains("no artifact logical role")
        );
    }

    #[test]
    fn robustness_source_rejects_invalid_profile_boundary() {
        let (mut registry, recipes) = robustness_fixture();
        registry.cases[0].profiles = vec!["negative".into()];
        assert!(
            validate_dependencies(&registry, &recipes)
                .unwrap_err()
                .to_string()
                .contains("invalid robustness profile boundary")
        );
    }

    #[test]
    fn robustness_source_rejects_self_reference() {
        let (registry, mut recipes) = robustness_fixture();
        let mutation_id = recipes
            .keys()
            .find(|identity| identity.recipe_id.starts_with("negative_"))
            .unwrap()
            .clone();
        let mutation = recipes.get_mut(&mutation_id).unwrap();
        mutation.dependencies[0].recipe.recipe_id = mutation_id.recipe_id.clone();
        mutation.dependencies[0].recipe.recipe_version = mutation_id.recipe_version.clone();
        mutation.mutation.as_mut().unwrap().source = mutation.dependencies[0].recipe.clone();
        assert!(
            validate_dependencies(&registry, &recipes)
                .unwrap_err()
                .to_string()
                .contains("invalid dependency")
        );
    }
}
