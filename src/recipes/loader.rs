use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::error::RecipeCatalogError;
use super::model::{CaseRecipe, RecipeKind};
use crate::planning::RecipeIdentity;

const CASE_RECIPE_SCHEMA: &str = include_str!("../../schemas/case-recipe.schema.json");

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
        let schema: Value =
            serde_json::from_str(CASE_RECIPE_SCHEMA).expect("embedded recipe schema");
        let validator = jsonschema::validator_for(&schema).expect("case recipe schema compiles");
        let paths = sorted_json_files(recipes_root.as_ref())?;
        let mut recipes = BTreeMap::new();
        let mut bindings = BTreeMap::new();

        for path in paths {
            let bytes = fs::read(&path).map_err(|error| RecipeCatalogError::Read {
                path: path.clone(),
                message: error.to_string(),
            })?;
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

        validate_registry_bindings(&registry, &recipes, &bindings, &templates)?;
        validate_dependencies(&recipes)?;
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
            if contracts
                .insert(
                    (id.clone(), version.clone()),
                    TemplateContract {
                        status,
                        sop_class_uid,
                        transfer_syntax_uids,
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
    Ok(())
}

fn validate_registered_ids(path: &Path, recipe: &CaseRecipe) -> Result<(), RecipeCatalogError> {
    const PLAN_PROVIDERS: &[&str] = &[
        "native.case_plan",
        "external.import_plan",
        "mutation.named_plan",
        "qualification.bounded_plan",
    ];
    const CONTENT_PROVIDERS: &[&str] = &["content.case_default"];
    const ALGORITHM_PROVIDERS: &[&str] = &["algorithm.case_provider"];
    const ENCODING_PROVIDERS: &[&str] = &["encoding.transfer_syntax_plan"];
    const VALIDATION_RULES: &[&str] = &[
        "validation.shared",
        "validation.independent_decode",
        "validation.mutation",
        "validation.qualification",
    ];
    const PROJECTION_RULES: &[&str] = &[
        "projection.curated",
        "projection.mutation",
        "projection.qualification",
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
                known(id, ENCODING_PROVIDERS, "encoding provider")?;
            }
            for id in &artifact.validation_rule_ids {
                known(id, VALIDATION_RULES, "validation rule")?;
            }
            for id in &artifact.projection_rule_ids {
                known(id, PROJECTION_RULES, "projection rule")?;
            }
        }
    }
    if let Some(mutation) = &recipe.mutation {
        for edit in &mutation.edits {
            if edit.mutation_id != "mutation.registry_named" {
                return Err(semantic(
                    path,
                    format!("unknown mutation id {}", edit.mutation_id),
                ));
            }
        }
    }
    Ok(())
}

fn validate_registry_bindings(
    registry: &RegistryDocument,
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
    bindings: &BTreeMap<String, RecipeIdentity>,
    templates: &BTreeMap<(String, String), TemplateContract>,
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
            ("mutation_layer", "bounded_deterministic_fuzz") => "qualification.bounded_plan",
            ("rust_native", "checked_eot_arithmetic")
                if expected_kind == RecipeKind::Qualification =>
            {
                "qualification.bounded_plan"
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
        if recipe.plan_provider_id != expected_provider {
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

fn validate_dependencies(
    recipes: &BTreeMap<RecipeIdentity, CaseRecipe>,
) -> Result<(), RecipeCatalogError> {
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
        }
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
}
