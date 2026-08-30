//! Composition-owned adapters for neutral semantic recipe providers.

use std::collections::BTreeMap;

use crate::corpus_plan::{
    ArtifactDependency, ArtifactResourceEstimate, ImplementationIdentityPlan, OutputPlan,
    OutputRelativePath,
};
use crate::planning::RecipeIdentity;
use crate::recipes::{
    RecipeCatalog, RtPlanProvider, SemanticPlanContext, SemanticSource, SrPlanProvider,
    encoding_plan_from_recipe, rt_input_from_recipe, sr_input_from_recipe,
};

use super::advanced_defaults::{
    AdvancedDefaultMember, AdvancedDefaultOutput, AdvancedSourceMember,
};
use super::{CompositionUidRole, MaterializedReference, TemplateDescriptor};

pub(crate) fn is_native_sr_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.template_id.0.as_str(),
        "derived/structured-report/basic-text"
            | "derived/structured-report/comprehensive"
            | "derived/structured-report/key-object"
    )
}

pub(crate) fn is_native_rt_default(template: &TemplateDescriptor) -> bool {
    template.template_id.0.starts_with("non-image/rt/")
}

pub(crate) fn semantic_identity_roles(
    recipes: &RecipeCatalog,
    template: &TemplateDescriptor,
) -> Result<Vec<CompositionUidRole>, String> {
    if !is_native_rt_default(template) {
        return Ok(vec![]);
    }
    let recipe_id = match template.template_id.0.as_str() {
        "non-image/rt/structure-set" => "rt_structure_set_single_roi",
        "non-image/rt/dose" => "rt_dose_grid_u16",
        "non-image/rt/plan" => "non_image_rt_plan_linked",
        "non-image/rt/image" => "non_image_rt_image_linked",
        "non-image/rt/c-arm-photon-electron-radiation" => {
            "non_image_rt_carm_photon_electron_radiation_minimal"
        }
        "non-image/rt/radiation-set" => "non_image_rt_radiation_set_minimal",
        other => return Err(format!("unsupported native RT template {other}")),
    };
    let recipe = recipes
        .recipes()
        .iter()
        .find_map(|(identity, recipe)| (identity.recipe_id == recipe_id).then_some(recipe))
        .ok_or_else(|| format!("missing RT recipe {recipe_id}"))?;
    Ok(recipe
        .provider_parameters
        .get("object")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| {
            (key.ends_with("_uid_role"))
                .then(|| value.as_str())
                .flatten()
                .map(|role| CompositionUidRole::TemplateDefined(role.into()))
        })
        .collect())
}

pub(crate) fn plan_native_rt_default(
    recipes: &RecipeCatalog,
    member: &AdvancedDefaultMember<'_>,
    sources: &[AdvancedSourceMember],
) -> Result<AdvancedDefaultOutput, String> {
    let recipe_id = match member.template.template_id.0.as_str() {
        "non-image/rt/structure-set" => "rt_structure_set_single_roi",
        "non-image/rt/dose" => "rt_dose_grid_u16",
        "non-image/rt/plan" => "non_image_rt_plan_linked",
        "non-image/rt/image" => "non_image_rt_image_linked",
        "non-image/rt/c-arm-photon-electron-radiation" => {
            "non_image_rt_carm_photon_electron_radiation_minimal"
        }
        "non-image/rt/radiation-set" => "non_image_rt_radiation_set_minimal",
        other => return Err(format!("unsupported native RT template {other}")),
    };
    let recipe = recipes
        .recipes()
        .iter()
        .find_map(|(identity, recipe)| (identity.recipe_id == recipe_id).then_some(recipe))
        .ok_or_else(|| format!("missing RT recipe {recipe_id}"))?;
    let artifact_recipe = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .ok_or_else(|| format!("RT recipe {recipe_id} has no artifact"))?;
    let declarations = recipe
        .provider_parameters
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("RT recipe {recipe_id} has no sources"))?;
    if declarations.len() != sources.len() {
        return Err(format!(
            "RT recipe {recipe_id} source cardinality differs from bundle"
        ));
    }
    let semantic_sources = declarations
        .iter()
        .zip(sources)
        .map(|(declaration, source)| {
            semantic_source(declaration, source, &member.instance.instance_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let implementation_uid = member
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or_else(|| "RT target lacks implementation identity".to_string())?;
    let encoding = encoding_plan_from_recipe(
        &artifact_recipe.encoding,
        ImplementationIdentityPlan {
            class_uid: implementation_uid.into(),
            version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
        },
    )
    .map_err(|error| error.to_string())?;
    let artifact_template = artifact_recipe
        .template
        .as_ref()
        .ok_or_else(|| "RT artifact lacks template".to_string())?;
    let mut parser_identities = member.identities.clone();
    parser_identities.logical_instance_id = artifact_recipe.logical_id.clone();
    let context = SemanticPlanContext {
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        logical_id: artifact_recipe.logical_id.clone(),
        order: u64::from(artifact_recipe.order),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(
                artifact_recipe
                    .output
                    .path
                    .as_deref()
                    .ok_or_else(|| "RT artifact lacks output path".to_string())?,
            )
            .map_err(|error| error.to_string())?,
            role: artifact_recipe.output.role.clone(),
            publish: true,
        },
        template_id: artifact_template.template_id.clone(),
        template_version: artifact_template.template_version.clone(),
        identities: parser_identities,
        encoding,
        base_attributes: vec![],
        sources: semantic_sources,
        resources: ArtifactResourceEstimate {
            output_bytes: 1 << 20,
            peak_working_bytes: 2 << 20,
        },
    };
    let mut input = rt_input_from_recipe(recipe, context)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "RT recipe did not produce typed input".to_string())?;
    input.context.logical_id = member.instance.instance_id.clone();
    input.context.order = member.order;
    input.context.output = composition_output(&member.instance.instance_id)?;
    input.context.identities = member.identities.clone();
    for source in &mut input.context.sources {
        source.reference.source_instance_id = member.instance.instance_id.clone();
    }
    let output = RtPlanProvider
        .plan(&input)
        .map_err(|error| format!("RT recipe {recipe_id}: {error}"))?;
    let dependencies = input
        .context
        .sources
        .iter()
        .map(|source| ArtifactDependency {
            artifact_id: member.instance.instance_id.clone(),
            depends_on: source.artifact_id.clone(),
            relationship: source.role.clone(),
            frame_numbers: source.reference.referenced_frames.clone(),
        })
        .collect();
    Ok(AdvancedDefaultOutput {
        artifacts: BTreeMap::from([(member.instance.instance_id.clone(), output.artifact)]),
        bindings: BTreeMap::from([(member.instance.instance_id.clone(), output.bindings)]),
        dependencies,
    })
}

pub(crate) fn plan_native_sr_default(
    recipes: &RecipeCatalog,
    member: &AdvancedDefaultMember<'_>,
    sources: &[AdvancedSourceMember],
) -> Result<AdvancedDefaultOutput, String> {
    let recipe_id = match member.template.template_id.0.as_str() {
        "derived/structured-report/basic-text" => "sr_basic_text_observation",
        "derived/structured-report/comprehensive" => "sr_comprehensive_measurement",
        "derived/structured-report/key-object" => "sr_key_object_selection",
        other => return Err(format!("unsupported native SR template {other}")),
    };
    let recipe = recipes
        .recipes()
        .iter()
        .find_map(|(identity, recipe)| (identity.recipe_id == recipe_id).then_some(recipe))
        .ok_or_else(|| format!("missing SR recipe {recipe_id}"))?;
    let artifact_recipe = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .ok_or_else(|| format!("SR recipe {recipe_id} has no artifact"))?;
    let declarations = recipe
        .provider_parameters
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("SR recipe {recipe_id} has no sources"))?;
    if declarations.len() != sources.len() {
        return Err(format!(
            "SR recipe {recipe_id} source cardinality differs from bundle"
        ));
    }
    let semantic_sources = declarations
        .iter()
        .zip(sources)
        .map(|(declaration, source)| {
            semantic_source(declaration, source, &member.instance.instance_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let implementation_uid = member
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or_else(|| "SR target lacks implementation identity".to_string())?;
    let encoding = encoding_plan_from_recipe(
        &artifact_recipe.encoding,
        ImplementationIdentityPlan {
            class_uid: implementation_uid.into(),
            version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
        },
    )
    .map_err(|error| error.to_string())?;
    let artifact_template = artifact_recipe
        .template
        .as_ref()
        .ok_or_else(|| "SR artifact lacks template".to_string())?;
    let mut parser_identities = member.identities.clone();
    parser_identities.logical_instance_id = artifact_recipe.logical_id.clone();
    let context = SemanticPlanContext {
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        logical_id: artifact_recipe.logical_id.clone(),
        order: u64::from(artifact_recipe.order),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(
                artifact_recipe
                    .output
                    .path
                    .as_deref()
                    .ok_or_else(|| "SR artifact lacks output path".to_string())?,
            )
            .map_err(|error| error.to_string())?,
            role: artifact_recipe.output.role.clone(),
            publish: true,
        },
        template_id: artifact_template.template_id.clone(),
        template_version: artifact_template.template_version.clone(),
        identities: parser_identities,
        encoding,
        base_attributes: vec![],
        sources: semantic_sources,
        resources: ArtifactResourceEstimate {
            output_bytes: 1 << 20,
            peak_working_bytes: 2 << 20,
        },
    };
    let mut input = sr_input_from_recipe(recipe, context)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "SR recipe did not produce typed input".to_string())?;
    input.context.logical_id = member.instance.instance_id.clone();
    input.context.order = member.order;
    input.context.output = composition_output(&member.instance.instance_id)?;
    input.context.identities = member.identities.clone();
    for source in &mut input.context.sources {
        source.reference.source_instance_id = member.instance.instance_id.clone();
    }
    let output = SrPlanProvider
        .plan_native(&input)
        .map_err(|error| error.to_string())?;
    let dependencies = input
        .context
        .sources
        .iter()
        .map(|source| ArtifactDependency {
            artifact_id: member.instance.instance_id.clone(),
            depends_on: source.artifact_id.clone(),
            relationship: source.role.clone(),
            frame_numbers: source.reference.referenced_frames.clone(),
        })
        .collect();
    Ok(AdvancedDefaultOutput {
        artifacts: BTreeMap::from([(member.instance.instance_id.clone(), output.artifact)]),
        bindings: BTreeMap::from([(member.instance.instance_id.clone(), output.bindings)]),
        dependencies,
    })
}

fn semantic_source(
    declaration: &serde_json::Value,
    source: &AdvancedSourceMember,
    target_id: &str,
) -> Result<SemanticSource, String> {
    let recipe = declaration
        .get("recipe")
        .ok_or_else(|| "semantic source lacks recipe".to_string())?;
    let role = string(declaration, "role")?;
    let frames = declaration
        .get("referenced_frames")
        .cloned()
        .map(serde_json::from_value::<Vec<u32>>)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let identity = |role| {
        source
            .artifact
            .instance
            .identities
            .get(&role, 0)
            .map(str::to_owned)
            .ok_or_else(|| format!("semantic source lacks {role:?} identity"))
    };
    Ok(SemanticSource {
        recipe: RecipeIdentity {
            recipe_id: string(recipe, "recipe_id")?,
            recipe_version: string(recipe, "recipe_version")?,
        },
        recipe_artifact_logical_id: string(declaration, "artifact_logical_id")?,
        artifact_id: source.artifact.logical_id.clone(),
        role: role.clone(),
        study_instance_uid: identity(CompositionUidRole::StudyInstance)?,
        series_instance_uid: identity(CompositionUidRole::SeriesInstance)?,
        reference: MaterializedReference {
            source_instance_id: target_id.into(),
            target_instance_id: source.artifact.logical_id.clone(),
            role,
            referenced_frames: frames,
            frame_role: None,
            referenced_sop_class_uid: source.artifact.instance.sop_class_uid.clone(),
            referenced_sop_instance_uid: identity(CompositionUidRole::SopInstance)?,
        },
    })
}

fn composition_output(instance_id: &str) -> Result<OutputPlan, String> {
    Ok(OutputPlan {
        relative_path: OutputRelativePath::new(format!("instances/{instance_id}.dcm"))
            .map_err(|error| error.to_string())?,
        role: "dicom_instance".into(),
        publish: true,
    })
}

fn string(value: &serde_json::Value, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("semantic source lacks {field}"))
}
