//! Neutral composition adapter for recipe-backed advanced defaults.
//!
//! The adapter maps composition-owned instance identities, order, and output
//! paths onto the shared advanced recipe providers. It never creates or opens
//! a DICOM file.

use std::collections::{BTreeMap, BTreeSet};

use crate::corpus_plan::{
    ArtifactDependency, OutputPlan, OutputRelativePath, PlannedDicomArtifact,
};
use crate::executor::services::ArtifactExecutionBindings;
use crate::planning::RecipeIdentity;
use crate::recipes::{
    AdvancedArtifactPlanningContext, AdvancedPlanProvider, AdvancedPlanProviderOutput,
    AdvancedPlanProviderRequest, AdvancedProviderFamily, AdvancedProviderLimits,
    AdvancedSourceRole, EnhancedPlanProvider, PRESENTATION_ADVANCED_PROVIDER_ID,
    PresentationPlanProvider, PresentationSourceInput, REGISTRATION_PLAN_PROVIDER_ID,
    RecipeCatalog, RegistrationKindInput, RegistrationPlanProvider, RegistrationSourceInput,
    WSI_ADVANCED_PROVIDER_ID, WsiAdvancedPlanProvider,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

use super::{
    CompositionUidRole, IdentityPlan, MaterializedReference, SpecInstance, TemplateDescriptor,
};

#[derive(Debug, Clone)]
pub(crate) struct AdvancedDefaultMember<'a> {
    pub instance: &'a SpecInstance,
    pub template: &'a TemplateDescriptor,
    pub identities: IdentityPlan,
    pub order: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct AdvancedSourceMember {
    pub artifact: PlannedDicomArtifact,
    pub binding: ArtifactExecutionBindings,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AdvancedDefaultOutput {
    pub artifacts: BTreeMap<String, PlannedDicomArtifact>,
    pub bindings: BTreeMap<String, ArtifactExecutionBindings>,
    pub dependencies: Vec<ArtifactDependency>,
}

pub(crate) fn is_direct_advanced_default(template: &TemplateDescriptor) -> bool {
    template.default_recipe.is_some()
        && matches!(
            template.artifact_kind.as_str(),
            "enhanced_image" | "whole_slide_image" | "registration" | "presentation_state"
        )
}

pub(crate) fn is_reference_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.artifact_kind.as_str(),
        "registration" | "presentation_state"
    ) && template.default_recipe.is_some()
}

pub(crate) fn group_identity(
    member: &AdvancedDefaultMember<'_>,
    bundle_root: &str,
) -> Result<(String, String, String), String> {
    let binding = member
        .template
        .default_recipe
        .as_ref()
        .ok_or_else(|| format!("{} has no default recipe", member.template.template_id))?;
    Ok((
        bundle_root.to_owned(),
        binding.recipe_id.clone(),
        binding.recipe_version.clone(),
    ))
}

pub(crate) fn plan_image_group(
    recipes: &RecipeCatalog,
    standards_lock_sha256: &str,
    seed: u64,
    limits: AdvancedProviderLimits,
    members: &[AdvancedDefaultMember<'_>],
) -> Result<AdvancedDefaultOutput, String> {
    let first = members
        .first()
        .ok_or_else(|| "advanced default group is empty".to_string())?;
    let binding = first
        .template
        .default_recipe
        .as_ref()
        .ok_or_else(|| "advanced default group lacks a recipe binding".to_string())?;
    let identity = RecipeIdentity {
        recipe_id: binding.recipe_id.clone(),
        recipe_version: binding.recipe_version.clone(),
    };
    let recipe = recipes
        .recipes()
        .get(&identity)
        .ok_or_else(|| format!("missing advanced default recipe {identity}"))?;
    let selected = members
        .iter()
        .map(|member| {
            let binding =
                member.template.default_recipe.as_ref().ok_or_else(|| {
                    format!("{} lacks a default recipe", member.template.template_id)
                })?;
            if binding.recipe_id != identity.recipe_id
                || binding.recipe_version != identity.recipe_version
            {
                return Err("advanced group crosses recipe identities".into());
            }
            Ok((binding.artifact_logical_id.clone(), member))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;

    let output = match recipe.plan_provider_id.as_str() {
        "native.enhanced_plan" => {
            let input = recipes
                .enhanced_input_for_case(&recipe.binding.case_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "enhanced recipe did not produce typed input".to_string())?;
            let provider = EnhancedPlanProvider::new(standards_lock_sha256)
                .map_err(|error| error.to_string())?;
            let contexts = overlay_contexts(
                provider
                    .recipe_default_contexts(&input, seed)
                    .map_err(|error| error.to_string())?,
                &selected,
                false,
            )?;
            let request = request(
                recipe,
                AdvancedProviderFamily::Enhanced,
                seed,
                limits,
                contexts,
            );
            provider
                .plan(&request, &input)
                .map_err(|error| error.to_string())?
        }
        WSI_ADVANCED_PROVIDER_ID => {
            let input = recipes
                .wsi_input_for_case(&recipe.binding.case_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "WSI recipe did not produce typed input".to_string())?;
            let provider = WsiAdvancedPlanProvider::new(standards_lock_sha256);
            let contexts = overlay_contexts(
                provider
                    .recipe_default_contexts(&input, seed)
                    .map_err(|error| error.to_string())?,
                &selected,
                true,
            )?;
            let request = request(
                recipe,
                AdvancedProviderFamily::WholeSlide,
                seed,
                limits,
                contexts,
            );
            provider
                .plan(&request, &input)
                .map_err(|error| error.to_string())?
        }
        other => return Err(format!("unsupported image default provider {other}")),
    };
    select_output(output, members, false)
}

pub(crate) fn plan_reference_default(
    recipes: &RecipeCatalog,
    standards_lock_sha256: &str,
    seed: u64,
    limits: AdvancedProviderLimits,
    target: &AdvancedDefaultMember<'_>,
    sources: &[AdvancedSourceMember],
) -> Result<AdvancedDefaultOutput, String> {
    let binding = target
        .template
        .default_recipe
        .as_ref()
        .ok_or_else(|| "reference default lacks a recipe binding".to_string())?;
    let identity = RecipeIdentity {
        recipe_id: binding.recipe_id.clone(),
        recipe_version: binding.recipe_version.clone(),
    };
    let recipe = recipes
        .recipes()
        .get(&identity)
        .ok_or_else(|| format!("missing reference default recipe {identity}"))?;
    let target_id = target.instance.instance_id.as_str();
    let output = match recipe.plan_provider_id.as_str() {
        REGISTRATION_PLAN_PROVIDER_ID => {
            let aliased = alias_registration_sources(
                recipes,
                recipe,
                target_id,
                &target.identities,
                sources,
            )?;
            let mut input = recipes
                .registration_input_for_case(&recipe.binding.case_id, aliased)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "registration recipe did not produce typed input".to_string())?;
            if matches!(&input.registration, RegistrationKindInput::Deformable(_)) {
                let moving = input
                    .sources
                    .get_mut(1)
                    .ok_or_else(|| "deformable registration lacks its moving source".to_string())?;
                let binding = moving.artifact.case_binding.as_ref().ok_or_else(|| {
                    "deformable registration moving source lacks recipe provenance".to_string()
                })?;
                let frame_of_reference = deterministic_uid(&DeterministicUidInput {
                    standards_lock_sha256,
                    case_id: &binding.case_id,
                    recipe_version: &binding.recipe_version,
                    run_seed: seed,
                    file_index: 0,
                    frame_index: None,
                    referenced_object_index: None,
                    role: UidRole::FrameOfReference,
                });
                moving
                    .artifact
                    .instance
                    .identities
                    .identities
                    .insert("frame_of_reference_uid#0".into(), frame_of_reference);
            }
            for (source, actual) in input.sources.iter_mut().zip(sources) {
                source.artifact.order = actual.artifact.order;
            }
            let provider = RegistrationPlanProvider::new(standards_lock_sha256)
                .map_err(|error| error.to_string())?;
            let defaults = provider
                .recipe_default_contexts(&input, &recipe.binding.case_id, &identity, seed)
                .map_err(|error| error.to_string())?;
            let contexts = overlay_contexts(
                defaults,
                &BTreeMap::from([(binding.artifact_logical_id.clone(), target)]),
                false,
            )?;
            let request = request(
                recipe,
                AdvancedProviderFamily::Registration,
                seed,
                limits,
                contexts,
            );
            let mut output = provider
                .plan(&request, &input)
                .map_err(|error| error.to_string())?;
            remap_source_ids(&mut output, &input.sources, sources);
            output
        }
        PRESENTATION_ADVANCED_PROVIDER_ID => {
            let aliased = alias_presentation_sources(recipes, recipe, sources)?;
            let mut input = recipes
                .presentation_input_for_case(&recipe.binding.case_id, aliased)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "presentation recipe did not produce typed input".to_string())?;
            for (source, actual) in input.sources.iter_mut().zip(sources) {
                source.artifact.order = actual.artifact.order;
            }
            let provider = PresentationPlanProvider::new(standards_lock_sha256);
            let defaults = provider
                .recipe_default_contexts(&input, seed)
                .map_err(|error| error.to_string())?;
            let contexts = overlay_contexts(
                defaults,
                &BTreeMap::from([(binding.artifact_logical_id.clone(), target)]),
                false,
            )?;
            let request = request(
                recipe,
                AdvancedProviderFamily::PresentationState,
                seed,
                limits,
                contexts,
            );
            let mut output = provider
                .plan(&request, &input)
                .map_err(|error| error.to_string())?;
            remap_presentation_source_ids(&mut output, &input.sources, sources);
            output
        }
        other => return Err(format!("unsupported reference default provider {other}")),
    };
    select_output(output, &[target.clone()], true)
}

fn request(
    recipe: &crate::recipes::CaseRecipe,
    family: AdvancedProviderFamily,
    seed: u64,
    limits: AdvancedProviderLimits,
    artifact_contexts: Vec<AdvancedArtifactPlanningContext>,
) -> AdvancedPlanProviderRequest {
    AdvancedPlanProviderRequest {
        provider_id: recipe.plan_provider_id.clone(),
        family,
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        seed,
        artifact_contexts,
        limits,
    }
}

fn overlay_contexts(
    contexts: Vec<AdvancedArtifactPlanningContext>,
    selected: &BTreeMap<String, &AdvancedDefaultMember<'_>>,
    share_group_identities: bool,
) -> Result<Vec<AdvancedArtifactPlanningContext>, String> {
    let shared_implementation = selected
        .values()
        .next()
        .and_then(|member| {
            member
                .identities
                .get(&CompositionUidRole::ImplementationClass, 0)
        })
        .map(str::to_owned);
    let mut next_unselected_order = selected
        .values()
        .map(|member| member.order)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "advanced composition order overflow".to_string())?;
    let shared_group_identities = selected.values().next().map(|member| {
        [
            CompositionUidRole::StudyInstance,
            CompositionUidRole::SeriesInstance,
            CompositionUidRole::FrameOfReference,
        ]
        .into_iter()
        .filter_map(|role| {
            member
                .identities
                .get(&role, 0)
                .map(|value| (format!("{}#0", role.as_str()), value.to_owned()))
        })
        .collect::<BTreeMap<_, _>>()
    });
    contexts
        .into_iter()
        .map(|mut context| {
            if share_group_identities {
                if let Some(identities) = &shared_group_identities {
                    context.identities.identities.extend(identities.clone());
                }
            }
            if let Some(value) = &shared_implementation {
                context
                    .identities
                    .identities
                    .insert("implementation_class_uid#0".into(), value.clone());
            }
            let Some(member) = selected.get(&context.recipe_artifact_logical_id) else {
                context.order = next_unselected_order;
                next_unselected_order = next_unselected_order
                    .checked_add(1)
                    .ok_or_else(|| "advanced composition order overflow".to_string())?;
                return Ok(context);
            };
            context.target_instance_id = member.instance.instance_id.clone();
            context.order = member.order;
            context.output = composition_output(&member.instance.instance_id)?;
            context.identities.logical_instance_id = member.instance.instance_id.clone();
            for (key, value) in &member.identities.identities {
                if key == "implementation_class_uid#0" {
                    continue;
                }
                context
                    .identities
                    .identities
                    .insert(key.clone(), value.clone());
            }
            Ok(context)
        })
        .collect()
}

fn composition_output(instance_id: &str) -> Result<OutputPlan, String> {
    Ok(OutputPlan {
        relative_path: OutputRelativePath::new(format!("instances/{instance_id}.dcm"))
            .map_err(|error| error.to_string())?,
        role: "composition_instance".into(),
        publish: true,
    })
}

fn select_output(
    output: AdvancedPlanProviderOutput,
    members: &[AdvancedDefaultMember<'_>],
    retain_external_dependencies: bool,
) -> Result<AdvancedDefaultOutput, String> {
    let selected = members
        .iter()
        .map(|member| member.instance.instance_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = AdvancedDefaultOutput::default();
    for planned in output.artifacts {
        if selected.contains(planned.planned.logical_id.as_str()) {
            result
                .artifacts
                .insert(planned.planned.logical_id.clone(), planned.planned);
        }
    }
    for binding in output.bindings {
        if selected.contains(binding.artifact_id.as_str()) {
            result.bindings.insert(binding.artifact_id.clone(), binding);
        }
    }
    result.dependencies = output
        .dependencies
        .into_iter()
        .filter(|dependency| {
            selected.contains(dependency.artifact_id.as_str())
                && (selected.contains(dependency.depends_on.as_str())
                    || retain_external_dependencies)
        })
        .collect();
    if result.artifacts.len() != selected.len() || result.bindings.len() != selected.len() {
        return Err("advanced provider omitted a selected composition artifact".into());
    }
    Ok(result)
}

fn alias_registration_sources(
    recipes: &RecipeCatalog,
    recipe: &crate::recipes::CaseRecipe,
    target_id: &str,
    target_identities: &IdentityPlan,
    sources: &[AdvancedSourceMember],
) -> Result<Vec<RegistrationSourceInput>, String> {
    if sources.len() != 2 || recipe.dependencies.len() != 2 {
        return Err("registration source cardinality differs from its recipe".into());
    }
    sources
        .iter()
        .zip(&recipe.dependencies)
        .enumerate()
        .map(|(index, (source, dependency))| {
            let dependency_recipe = dependency.recipe.identity();
            let dependency_document = recipes
                .recipes()
                .get(&dependency_recipe)
                .ok_or_else(|| format!("missing source recipe {dependency_recipe}"))?;
            let source_recipe = dependency_document
                .dicom
                .as_ref()
                .and_then(|dicom| {
                    dicom
                        .artifacts
                        .get(index.min(dicom.artifacts.len().saturating_sub(1)))
                })
                .ok_or_else(|| {
                    format!("source recipe {dependency_recipe} has no DICOM artifact")
                })?;
            let mut artifact = source.artifact.clone();
            artifact.logical_id = source_recipe.logical_id.clone();
            artifact.instance.instance_id = artifact.logical_id.clone();
            if index == 0 {
                for role in [
                    CompositionUidRole::StudyInstance,
                    CompositionUidRole::FrameOfReference,
                ] {
                    let value = target_identities
                        .get(&role, 0)
                        .ok_or_else(|| format!("registration target lacks {role:?} identity"))?
                        .to_owned();
                    let key = match role {
                        CompositionUidRole::StudyInstance => "study_instance_uid#0",
                        CompositionUidRole::FrameOfReference => "frame_of_reference_uid#0",
                        _ => unreachable!(),
                    };
                    artifact
                        .instance
                        .identities
                        .identities
                        .insert(key.into(), value);
                }
            } else {
                let distinct_study = artifact
                    .instance
                    .identities
                    .get(&CompositionUidRole::SeriesInstance, 0)
                    .ok_or_else(|| "moving registration source lacks series identity".to_string())?
                    .to_owned();
                artifact
                    .instance
                    .identities
                    .identities
                    .insert("study_instance_uid#0".into(), distinct_study);
            }
            artifact.case_binding = Some(crate::corpus_plan::CaseBinding {
                case_id: dependency_document.binding.case_id.clone(),
                recipe_id: dependency_recipe.recipe_id,
                recipe_version: dependency_recipe.recipe_version,
            });
            let mut bindings = source.binding.clone();
            bindings.artifact_id = artifact.logical_id.clone();
            let role = if index == 0 {
                AdvancedSourceRole::RegistrationFixed
            } else {
                AdvancedSourceRole::RegistrationMoving
            };
            Ok(RegistrationSourceInput {
                role,
                reference: reference(target_id, &artifact, &source.artifact, Vec::new())?,
                artifact,
                bindings,
            })
        })
        .collect()
}

fn alias_presentation_sources(
    recipes: &RecipeCatalog,
    recipe: &crate::recipes::CaseRecipe,
    sources: &[AdvancedSourceMember],
) -> Result<Vec<PresentationSourceInput>, String> {
    let dependency = recipe
        .dependencies
        .first()
        .ok_or_else(|| "presentation recipe lacks a source dependency".to_string())?
        .recipe
        .identity();
    let dependency_document = recipes
        .recipes()
        .get(&dependency)
        .ok_or_else(|| format!("missing source recipe {dependency}"))?;
    let dependency_artifacts = dependency_document
        .dicom
        .as_ref()
        .ok_or_else(|| format!("source recipe {dependency} has no DICOM artifacts"))?
        .artifacts
        .as_slice();
    if dependency_artifacts.len() != sources.len() {
        return Err("presentation source count differs from dependency artifacts".into());
    }
    sources
        .iter()
        .zip(dependency_artifacts)
        .enumerate()
        .map(|(index, (source, declared))| {
            let mut artifact = source.artifact.clone();
            artifact.logical_id = declared.logical_id.clone();
            artifact.instance.instance_id = artifact.logical_id.clone();
            artifact.case_binding = Some(crate::corpus_plan::CaseBinding {
                case_id: dependency_document.binding.case_id.clone(),
                recipe_id: dependency.recipe_id.clone(),
                recipe_version: dependency.recipe_version.clone(),
            });
            let mut binding = source.binding.clone();
            binding.artifact_id = artifact.logical_id.clone();
            let role = if sources.len() == 1 {
                AdvancedSourceRole::PresentationSourceImage
            } else {
                AdvancedSourceRole::PresentationBlendingInput {
                    input_number: if index < 2 { 1 } else { 2 },
                }
            };
            Ok(PresentationSourceInput {
                ordinal: (index + 1) as u32,
                role,
                referenced_frames: Vec::new(),
                artifact,
                binding,
            })
        })
        .collect()
}

fn reference(
    source_instance_id: &str,
    alias: &PlannedDicomArtifact,
    actual: &PlannedDicomArtifact,
    referenced_frames: Vec<u32>,
) -> Result<MaterializedReference, String> {
    let sop = actual
        .instance
        .identities
        .get(&CompositionUidRole::SopInstance, 0)
        .ok_or_else(|| format!("{} lacks SOP identity", actual.logical_id))?;
    Ok(MaterializedReference {
        source_instance_id: source_instance_id.into(),
        target_instance_id: alias.instance.instance_id.clone(),
        role: "source_image".into(),
        frame_role: None,
        referenced_sop_class_uid: actual.instance.sop_class_uid.clone(),
        referenced_sop_instance_uid: sop.into(),
        referenced_frames,
    })
}

fn remap_source_ids(
    output: &mut AdvancedPlanProviderOutput,
    aliases: &[RegistrationSourceInput],
    actual: &[AdvancedSourceMember],
) {
    remap(
        output,
        aliases
            .iter()
            .map(|source| source.artifact.logical_id.as_str()),
        actual,
    );
}

fn remap_presentation_source_ids(
    output: &mut AdvancedPlanProviderOutput,
    aliases: &[PresentationSourceInput],
    actual: &[AdvancedSourceMember],
) {
    remap(
        output,
        aliases
            .iter()
            .map(|source| source.artifact.logical_id.as_str()),
        actual,
    );
}

fn remap<'a>(
    output: &mut AdvancedPlanProviderOutput,
    aliases: impl Iterator<Item = &'a str>,
    actual: &[AdvancedSourceMember],
) {
    let mapping = aliases
        .zip(actual)
        .map(|(alias, source)| {
            (
                alias.to_owned(),
                (
                    source.artifact.logical_id.clone(),
                    source.artifact.instance.instance_id.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for dependency in &mut output.dependencies {
        if let Some((id, _)) = mapping.get(&dependency.depends_on) {
            dependency.depends_on.clone_from(id);
        }
    }
    for reference in &mut output.references {
        if let Some((id, instance_id)) = mapping.get(&reference.source_artifact_id) {
            reference.source_artifact_id.clone_from(id);
            reference
                .reference
                .target_instance_id
                .clone_from(instance_id);
        }
    }
    for artifact in &mut output.artifacts {
        for reference in &mut artifact.planned.instance.references {
            if let Some((_, instance_id)) = mapping.get(&reference.target_instance_id) {
                reference.target_instance_id.clone_from(instance_id);
            }
        }
    }
}
