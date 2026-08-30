//! Neutral composition adapter for recipe-backed advanced defaults.
//!
//! The adapter maps composition-owned instance identities, order, and output
//! paths onto the shared advanced recipe providers. It never creates or opens
//! a DICOM file.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, ImportedDicomProviderPlan, OutputPlan,
    OutputRelativePath, PlannedDicomArtifact, PlannedImportedDicomArtifact, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ProviderOutputExpectation, ProviderRequest, SlotExecutionBinding,
    StagedAssetHandle,
};
use crate::planning::RecipeIdentity;
use crate::recipes::{
    AdvancedArtifactPlanningContext, AdvancedPlanProvider, AdvancedPlanProviderOutput,
    AdvancedPlanProviderRequest, AdvancedProviderFamily, AdvancedProviderLimits,
    AdvancedSourceRole, ContentProviderLimits, ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID,
    EncapsulatedPayloadPlanProvider, EnhancedPlanProvider, PRESENTATION_ADVANCED_PROVIDER_ID,
    PresentationPlanProvider, PresentationSourceInput, QuantitativeArtifactContext,
    QuantitativePlanInput, QuantitativePlanOutput, QuantitativePlanProvider,
    QuantitativeProviderLimits, QuantitativeSourceInput, QuantitativeSourceRole,
    REGISTRATION_PLAN_PROVIDER_ID, RecipeCatalog, RegistrationKindInput, RegistrationPlanProvider,
    RegistrationSourceInput, TypedBulkPlanningContext, WAVEFORM_PLAN_PROVIDER_ID,
    WSI_ADVANCED_PROVIDER_ID, WaveformPlanProvider, WsiAdvancedPlanProvider,
    encapsulated_payload_input_from_recipe, waveform_input_from_recipe,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

use super::executor_adapter::CompositionExternalDicomProvider;
use super::external_quantitative::{
    ExternalQuantitativeSource, ParametricMapExternalProvider, WsiSegExternalProvider,
};
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

#[derive(Clone)]
pub(crate) struct ExternalQuantitativeDefaultOutput {
    pub artifact: PlannedImportedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
    pub dependencies: Vec<ArtifactDependency>,
    pub provider: Arc<dyn CompositionExternalDicomProvider>,
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

pub(crate) fn is_typed_bulk_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.template_id.0.as_str(),
        "non-image/waveform/twelve-lead-ecg"
            | "non-image/waveform/general-ecg"
            | "non-image/encapsulated-document/pdf"
            | "non-image/mesh/stl"
    )
}

pub(crate) fn plan_typed_bulk_default(
    recipes: &RecipeCatalog,
    member: &AdvancedDefaultMember<'_>,
) -> Result<AdvancedDefaultOutput, String> {
    let (recipe_id, recipe_artifact_id) = match member.template.template_id.0.as_str() {
        "non-image/waveform/twelve-lead-ecg" => {
            ("non_image_waveform_twelve_lead_ecg", "artifact_1")
        }
        "non-image/waveform/general-ecg" => ("non_image_waveform_general_ecg", "artifact_1"),
        "non-image/encapsulated-document/pdf" => ("encapsulated_pdf_minimal", "artifact_1"),
        "non-image/mesh/stl" => ("derived_mesh_encapsulated_stl", "artifact_1"),
        other => return Err(format!("unsupported typed-bulk default template {other}")),
    };
    let recipe = recipes
        .recipes()
        .iter()
        .find_map(|(identity, recipe)| (identity.recipe_id == recipe_id).then_some(recipe))
        .ok_or_else(|| format!("missing typed-bulk default recipe {recipe_id}"))?;
    let context = TypedBulkPlanningContext {
        recipe_artifact_logical_id: recipe_artifact_id.into(),
        target_instance_id: member.instance.instance_id.clone(),
        order: member.order,
        output: composition_output(&member.instance.instance_id)?,
        identities: member.identities.clone(),
    };
    let output = match recipe.plan_provider_id.as_str() {
        WAVEFORM_PLAN_PROVIDER_ID => {
            let input = waveform_input_from_recipe(recipe)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "waveform recipe did not produce typed input".to_string())?;
            WaveformPlanProvider
                .plan(&input, &context, ContentProviderLimits::default())
                .map_err(|error| error.to_string())?
        }
        ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID => {
            let input = encapsulated_payload_input_from_recipe(recipe)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "encapsulated payload recipe did not produce typed input".to_string()
                })?;
            EncapsulatedPayloadPlanProvider
                .plan(&input, &context, ContentProviderLimits::default())
                .map_err(|error| error.to_string())?
        }
        other => return Err(format!("unsupported typed-bulk default provider {other}")),
    };
    if output.artifact.instance.template_id != member.template.template_id
        || output.artifact.instance.sop_class_uid != member.template.sop_class_uid
    {
        return Err("typed-bulk provider output differs from composition template".into());
    }
    Ok(AdvancedDefaultOutput {
        artifacts: BTreeMap::from([(member.instance.instance_id.clone(), output.artifact)]),
        bindings: BTreeMap::from([(member.instance.instance_id.clone(), output.bindings)]),
        dependencies: vec![],
    })
}

pub(crate) fn is_native_quantitative_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.template_id.0.as_str(),
        "derived/segmentation/binary"
            | "derived/segmentation/fractional-probability"
            | "derived/segmentation/labelmap"
            | "derived/real-world-value-mapping/linear"
    )
}

pub(crate) fn is_external_quantitative_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.template_id.0.as_str(),
        "derived/segmentation/wsi-tile"
            | "derived/parametric-map/float32"
            | "derived/parametric-map/float64"
    )
}

pub(crate) fn plan_external_quantitative_default(
    recipes: &RecipeCatalog,
    repository_root: &std::path::Path,
    standards_lock_sha256: &str,
    seed: u64,
    member: &AdvancedDefaultMember<'_>,
    sources: &[AdvancedSourceMember],
) -> Result<ExternalQuantitativeDefaultOutput, String> {
    let (recipe_id, artifact_id, roles) = match member.template.template_id.0.as_str() {
        "derived/segmentation/wsi-tile" => (
            "derived_seg_wsi_tile_reference",
            "segmentation",
            vec![QuantitativeSourceRole::WholeSlideSourceImage],
        ),
        "derived/parametric-map/float32" => (
            "derived_parametric_map_float32_ct_derived_explicit_le",
            "parametric_map",
            vec![QuantitativeSourceRole::ParametricMapSourceImage; 3],
        ),
        "derived/parametric-map/float64" => (
            "derived_parametric_map_float64_ct_derived_explicit_le",
            "parametric_map",
            vec![QuantitativeSourceRole::ParametricMapSourceImage; 3],
        ),
        other => {
            return Err(format!(
                "unsupported external quantitative template {other}"
            ));
        }
    };
    if sources.len() != roles.len() {
        return Err("external quantitative source cardinality differs from recipe".into());
    }
    let recipe = recipe_by_id(recipes, recipe_id)?;
    let artifact_recipe = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no artifact"))?;
    let recipe_path = artifact_recipe
        .output
        .path
        .as_deref()
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no output path"))?;
    let mut parser_identities = member.identities.clone();
    parser_identities.logical_instance_id = artifact_id.into();
    let parser_context = QuantitativeArtifactContext {
        recipe_artifact_logical_id: artifact_id.into(),
        target_instance_id: artifact_id.into(),
        order: u64::from(artifact_recipe.order),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(recipe_path)
                .map_err(|error| error.to_string())?,
            role: artifact_recipe.output.role.clone(),
            publish: true,
        },
        identities: parser_identities,
    };
    let declarations = recipe
        .provider_parameters
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no sources"))?;
    let parser_sources = sources
        .iter()
        .zip(&roles)
        .zip(declarations)
        .map(|((source, role), declaration)| {
            let logical_id = declaration["artifact_logical_id"]
                .as_str()
                .ok_or_else(|| "external source declaration lacks artifact ID".to_string())?;
            let referenced_frames = declaration
                .get("referenced_frames")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            let mut artifact = source.artifact.clone();
            artifact.logical_id = logical_id.into();
            artifact.case_binding = Some(CaseBinding {
                case_id: "composition_parser_source".into(),
                recipe_id: declaration
                    .pointer("/recipe/recipe_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "external source declaration lacks recipe ID".to_string())?
                    .into(),
                recipe_version: declaration
                    .pointer("/recipe/recipe_version")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "external source declaration lacks recipe version".to_string())?
                    .into(),
            });
            let mut bindings = source.binding.clone();
            bindings.artifact_id = logical_id.into();
            Ok(QuantitativeSourceInput {
                role: *role,
                artifact,
                bindings,
                referenced_frames,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut input =
        crate::recipes::quantitative_input_from_recipe(recipe, parser_context, parser_sources)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "quantitative recipe did not produce typed input".to_string())?;
    let QuantitativePlanInput::ExternalImport {
        artifact: context,
        sources: bound_sources,
        ..
    } = &mut input
    else {
        return Err("external quantitative recipe produced native input".into());
    };
    context.target_instance_id = member.instance.instance_id.clone();
    context.order = member.order;
    context.output = composition_output(&member.instance.instance_id)?;
    context.identities = member.identities.clone();
    *bound_sources = sources
        .iter()
        .zip(&roles)
        .zip(declarations)
        .map(|((source, role), declaration)| {
            Ok(QuantitativeSourceInput {
                role: *role,
                artifact: source.artifact.clone(),
                bindings: source.binding.clone(),
                referenced_frames: declaration
                    .get("referenced_frames")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let QuantitativePlanOutput::ExternalImport {
        recipe,
        case_id,
        artifact,
        import,
        sources,
        dependencies,
        references,
    } = QuantitativePlanProvider
        .plan(&input, QuantitativeProviderLimits::default())
        .map_err(|error| error.to_string())?
    else {
        return Err("external quantitative recipe produced native output".into());
    };
    let provider_sources = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let role = if sources.len() == 1 {
                "source_image".to_string()
            } else {
                format!("source_{}", index + 1)
            };
            (role, source)
        })
        .collect::<Vec<_>>();
    let source_assets = provider_sources
        .iter()
        .map(|(role, source)| (role.clone(), source.artifact.logical_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let input_assets = provider_sources
        .iter()
        .map(|(role, source)| {
            Ok((
                role.clone(),
                StagedAssetHandle::new(format!("output:{}", source.artifact.logical_id))
                    .map_err(|error| error.to_string())?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let parameters = BTreeMap::from([
        (
            "import_kind".into(),
            serde_json::to_value(import.kind).unwrap(),
        ),
        (
            "timeout_seconds".into(),
            serde_json::json!(import.timeout_seconds),
        ),
        (
            "dependency_lock_sha256".into(),
            serde_json::json!(import.dependency.dependency_lock_sha256),
        ),
        (
            "semantic_evidence".into(),
            serde_json::to_value(&import.semantic_evidence).unwrap(),
        ),
    ]);
    let provider_plan = ImportedDicomProviderPlan {
        request_id: format!("{}-{}", import.request_id, member.instance.instance_id),
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
    let declared_instance = super::ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: artifact.target_instance_id.clone(),
        template_id: member.template.template_id.clone(),
        template_version: member.template.template_version,
        sop_class_uid: import.semantic_evidence.sop_class_uid.clone(),
        transfer_syntax_uid: import.semantic_evidence.transfer_syntax_uid.clone(),
        identities: artifact.identities.clone(),
        attributes: vec![],
        content: vec![],
        references,
    };
    let recipe_case_id = case_id.clone();
    let recipe_version = recipe.recipe_version.clone();
    let planned = PlannedImportedDicomArtifact {
        logical_id: artifact.target_instance_id.clone(),
        order: artifact.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: None,
        provider: provider_plan.clone(),
        declared_instance,
        output: artifact.output,
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "composition_imported_quantitative".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![
                EvidenceObligation {
                    obligation_id: "composition_manifest".into(),
                    route_id: "composition_manifest".into(),
                    independence: EvidenceIndependence::SameProject,
                    required: true,
                    parameters: BTreeMap::new(),
                },
                EvidenceObligation {
                    obligation_id: "external_quantitative_provider".into(),
                    route_id: import.dependency.executable_provider_id.clone(),
                    independence: EvidenceIndependence::ExternalProvider,
                    required: true,
                    parameters: BTreeMap::new(),
                },
            ],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: import.maximum_output_bytes,
            peak_working_bytes: import.maximum_output_bytes,
        },
    };
    let request = ProviderRequest {
        request_id: provider_plan.request_id.clone(),
        artifact_id: planned.logical_id.clone(),
        provider_id: provider_plan.provider_id.clone(),
        required_version: provider_plan.required_version.clone(),
        parameters,
        input_assets,
        expected_outputs: vec![ProviderOutputExpectation {
            slot: provider_plan.output_slot.clone(),
            media_type: provider_plan.media_type.clone(),
            maximum_size_bytes: provider_plan.maximum_size_bytes,
            expected_sha256: None,
        }],
    };
    let required_uid = |role: CompositionUidRole| {
        artifact
            .identities
            .get(&role, 0)
            .map(str::to_owned)
            .ok_or_else(|| format!("import target lacks {}", role.as_str()))
    };
    let external_sources = provider_sources
        .iter()
        .map(|(role, source)| {
            Ok(ExternalQuantitativeSource {
                role: role.clone(),
                case_id: source
                    .artifact
                    .case_binding
                    .as_ref()
                    .map(|binding| binding.case_id.clone())
                    .unwrap_or_else(|| source.artifact.logical_id.clone()),
                sop_class_uid: source.artifact.instance.sop_class_uid.clone(),
                sop_instance_uid: source
                    .artifact
                    .instance
                    .identities
                    .get(&CompositionUidRole::SopInstance, 0)
                    .ok_or_else(|| "quantitative source lacks SOP Instance UID".to_string())?
                    .into(),
                series_instance_uid: source
                    .artifact
                    .instance
                    .identities
                    .get(&CompositionUidRole::SeriesInstance, 0)
                    .map(str::to_owned),
                frame_numbers: source.referenced_frames.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let study = required_uid(CompositionUidRole::StudyInstance)?;
    let series = required_uid(CompositionUidRole::SeriesInstance)?;
    let frame_of_reference = required_uid(CompositionUidRole::FrameOfReference)?;
    let sop = required_uid(CompositionUidRole::SopInstance)?;
    let dimension = artifact
        .identities
        .get(&CompositionUidRole::DimensionOrganization, 0)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            crate::deterministic_uid(&crate::DeterministicUidInput {
                standards_lock_sha256,
                case_id: &recipe_case_id,
                recipe_version: &recipe_version,
                run_seed: seed,
                file_index: 0,
                frame_index: None,
                referenced_object_index: None,
                role: crate::UidRole::DimensionOrganization,
            })
        });
    let provider: Arc<dyn CompositionExternalDicomProvider> = match import.kind {
        crate::recipes::ExternalImportKind::WholeSlideTileSegmentation => {
            Arc::new(WsiSegExternalProvider {
                repository_root: repository_root.to_owned(),
                standards_lock_path: repository_root.join("standards.lock.json"),
                seed,
                standards_lock_sha256: standards_lock_sha256.into(),
                study_instance_uid: study,
                series_instance_uid: series,
                frame_of_reference_uid: frame_of_reference,
                sop_instance_uid: sop,
                dimension_organization_uid: dimension,
                source: external_sources
                    .into_iter()
                    .next()
                    .ok_or_else(|| "WSI import has no source".to_string())?,
            })
        }
        kind => Arc::new(ParametricMapExternalProvider {
            repository_root: repository_root.to_owned(),
            standards_lock_path: repository_root.join("standards.lock.json"),
            seed,
            standards_lock_sha256: standards_lock_sha256.into(),
            study_instance_uid: study,
            series_instance_uid: series,
            frame_of_reference_uid: frame_of_reference,
            sop_instance_uid: sop,
            dimension_organization_uid: dimension,
            sources: external_sources,
            float64: matches!(
                kind,
                crate::recipes::ExternalImportKind::ParametricMapFloat64
            ),
            // These values are part of the pinned highdicom request protocol,
            // not independently generated composition content.
            stored_value_scale: 0.25,
            spatial_rank_increment: if matches!(
                kind,
                crate::recipes::ExternalImportKind::ParametricMapFloat64
            ) {
                9.313_226e-10
            } else {
                0.25
            },
        }),
    };
    Ok(ExternalQuantitativeDefaultOutput {
        artifact: planned,
        bindings: ArtifactExecutionBindings {
            artifact_id: member.instance.instance_id.clone(),
            slots: BTreeMap::from([(
                provider_plan.output_slot.clone(),
                SlotExecutionBinding::ProviderRequest { request },
            )]),
        },
        dependencies,
        provider,
    })
}

pub(crate) fn plan_native_quantitative_default(
    recipes: &RecipeCatalog,
    member: &AdvancedDefaultMember<'_>,
    source: &AdvancedSourceMember,
) -> Result<AdvancedDefaultOutput, String> {
    let (recipe_id, artifact_id, role) = match member.template.template_id.0.as_str() {
        "derived/segmentation/binary" => (
            "seg_binary_multiframe",
            "segmentation",
            QuantitativeSourceRole::SegmentationSourceImage,
        ),
        "derived/segmentation/fractional-probability" => (
            "seg_fractional_probability_multiframe",
            "segmentation",
            QuantitativeSourceRole::SegmentationSourceImage,
        ),
        "derived/segmentation/labelmap" => (
            "seg_labelmap_multiframe",
            "segmentation",
            QuantitativeSourceRole::SegmentationSourceImage,
        ),
        "derived/real-world-value-mapping/linear" => (
            "rwvm_linear_ct_mapping",
            "mapping",
            QuantitativeSourceRole::RealWorldValueSourceImage,
        ),
        other => return Err(format!("unsupported native quantitative template {other}")),
    };
    let recipe = recipe_by_id(recipes, recipe_id)?;
    let artifact_recipe = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no artifact"))?;
    let recipe_path = artifact_recipe
        .output
        .path
        .as_deref()
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no output path"))?;
    let mut parser_identities = member.identities.clone();
    parser_identities.logical_instance_id = artifact_id.into();
    let parser_context = QuantitativeArtifactContext {
        recipe_artifact_logical_id: artifact_id.into(),
        target_instance_id: artifact_id.into(),
        order: u64::from(artifact_recipe.order),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(recipe_path).map_err(|e| e.to_string())?,
            role: artifact_recipe.output.role.clone(),
            publish: true,
        },
        identities: parser_identities,
    };
    let source_declaration = recipe
        .provider_parameters
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .and_then(|sources| sources.first())
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no source declaration"))?;
    let source_artifact_id = source_declaration
        .get("artifact_logical_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("quantitative recipe {recipe_id} has no source artifact ID"))?;
    let referenced_frames: Vec<u32> = source_declaration
        .get("referenced_frames")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let mut parser_artifact = source.artifact.clone();
    parser_artifact.logical_id = source_artifact_id.into();
    let mut parser_binding = source.binding.clone();
    parser_binding.artifact_id = source_artifact_id.into();
    let parser_source = QuantitativeSourceInput {
        role,
        artifact: parser_artifact,
        bindings: parser_binding,
        referenced_frames: referenced_frames.clone(),
    };
    let mut input =
        crate::recipes::quantitative_input_from_recipe(recipe, parser_context, vec![parser_source])
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "quantitative recipe did not produce typed input".to_string())?;
    let (context, sources) = match &mut input {
        QuantitativePlanInput::NativeSeg {
            artifact, sources, ..
        }
        | QuantitativePlanInput::NativeRwvm {
            artifact, sources, ..
        }
        | QuantitativePlanInput::ExternalImport {
            artifact, sources, ..
        } => (artifact, sources),
    };
    context.target_instance_id = member.instance.instance_id.clone();
    context.order = member.order;
    context.output = composition_output(&member.instance.instance_id)?;
    context.identities = member.identities.clone();
    *sources = vec![QuantitativeSourceInput {
        role,
        artifact: source.artifact.clone(),
        bindings: source.binding.clone(),
        referenced_frames,
    }];
    let QuantitativePlanOutput::Native {
        artifact,
        bindings,
        dependencies,
    } = QuantitativePlanProvider
        .plan(&input, QuantitativeProviderLimits::default())
        .map_err(|error| error.to_string())?
    else {
        return Err("native quantitative recipe produced an import boundary".into());
    };
    Ok(AdvancedDefaultOutput {
        artifacts: BTreeMap::from([(member.instance.instance_id.clone(), artifact)]),
        bindings: BTreeMap::from([(member.instance.instance_id.clone(), bindings)]),
        dependencies,
    })
}

fn recipe_by_id<'a>(
    recipes: &'a RecipeCatalog,
    recipe_id: &str,
) -> Result<&'a crate::recipes::CaseRecipe, String> {
    recipes
        .recipes()
        .iter()
        .find_map(|(identity, recipe)| (identity.recipe_id == recipe_id).then_some(recipe))
        .ok_or_else(|| format!("missing recipe {recipe_id}"))
}

pub(crate) fn group_identity(
    recipes: &RecipeCatalog,
    member: &AdvancedDefaultMember<'_>,
    bundle_root: &str,
) -> Result<(String, String, String, Option<String>), String> {
    let binding = member
        .template
        .default_recipe
        .as_ref()
        .ok_or_else(|| format!("{} has no default recipe", member.template.template_id))?;
    let identity = RecipeIdentity {
        recipe_id: binding.recipe_id.clone(),
        recipe_version: binding.recipe_version.clone(),
    };
    let artifact_count = recipes
        .recipes()
        .get(&identity)
        .and_then(|recipe| recipe.dicom.as_ref())
        .map_or(0, |dicom| dicom.artifacts.len());
    Ok((
        bundle_root.to_owned(),
        binding.recipe_id.clone(),
        binding.recipe_version.clone(),
        (artifact_count <= 1).then(|| member.instance.instance_id.clone()),
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
