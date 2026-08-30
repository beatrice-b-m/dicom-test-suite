//! Composition-owned adapters for neutral semantic recipe providers.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ImportedDicomProviderPlan, ItemLengthPolicy, OffsetTablePolicy,
    OutputPlan, OutputRelativePath, PlannedImportedDicomArtifact, PreamblePolicy,
    SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ProviderOutputExpectation, ProviderRequest, SlotExecutionBinding,
    StagedAssetHandle,
};
use crate::planning::RecipeIdentity;
use crate::recipes::{
    RecipeCatalog, RtPlanProvider, SemanticPlanContext, SemanticSource, SrPlanProvider,
    encoding_plan_from_recipe, rt_input_from_recipe, sr_input_from_recipe,
};

use super::advanced_defaults::{
    AdvancedDefaultMember, AdvancedDefaultOutput, AdvancedSourceMember,
};
use super::executor_adapter::CompositionExternalDicomProvider;
use super::external_sr::{ExternalSrKind, ExternalSrProvider, ExternalSrSource};
use super::{CompositionUidRole, MaterializedReference, TemplateDescriptor};

pub(crate) fn align_external_sr_source_graph(
    spec: &mut super::CompositionSpec,
) -> Result<(), String> {
    let tid_targets = spec
        .instances
        .iter()
        .filter(|instance| instance.template.id.0 == "derived/structured-report/tid1500")
        .map(|instance| {
            let image = instance
                .references
                .iter()
                .find(|reference| reference.role == "image_source")
                .ok_or_else(|| "TID 1500 bundle lacks image source".to_string())?;
            let segmentation = instance
                .references
                .iter()
                .find(|reference| reference.role == "segmentation_source")
                .ok_or_else(|| "TID 1500 bundle lacks segmentation source".to_string())?;
            Ok((
                image.target_instance_id.clone(),
                segmentation.target_instance_id.clone(),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    for (image_id, segmentation_id) in tid_targets {
        let segmentation = spec
            .instances
            .iter_mut()
            .find(|instance| instance.instance_id == segmentation_id)
            .ok_or_else(|| "TID 1500 segmentation bundle member is absent".to_string())?;
        if segmentation.references.len() != 1 {
            return Err("TID 1500 segmentation must have exactly one image source".into());
        }
        let source = &mut segmentation.references[0];
        if source.role != "source_image" || source.frames != [1, 2] {
            return Err("TID 1500 segmentation source edge has an unexpected shape".into());
        }
        source.target_instance_id = image_id;
    }
    Ok(())
}

pub(crate) fn qualify_tid1500_segmentation_source(
    plan: &mut super::ResolvedInstancePlan,
) -> Result<(), String> {
    fn patch_frame(operations: &mut [super::AttributeOperation], frame: u32, matches: &mut usize) {
        for operation in operations {
            if operation.address().normalized_tag() == "0008,1160" {
                if let super::AttributeOperation::Set { value, .. } = operation {
                    *value = super::AttributeValue::Primitive(super::PrimitiveValue::String(
                        frame.to_string(),
                    ));
                    *matches += 1;
                }
            }
            if let super::AttributeOperation::Set {
                value: super::AttributeValue::Sequence(items),
                ..
            } = operation
            {
                for item in items {
                    patch_frame(&mut item.attributes, frame, matches);
                }
            }
        }
    }
    let per_frame = plan
        .attributes
        .iter_mut()
        .find(|attribute| attribute.address.normalized_tag() == "5200,9230")
        .ok_or_else(|| "TID 1500 SEG lacks Per-frame Functional Groups".to_string())?;
    let Some(super::AttributeValue::Sequence(items)) = per_frame.value.as_mut() else {
        return Err("TID 1500 SEG Per-frame Functional Groups is not a sequence".into());
    };
    if items.len() != 2 {
        return Err("TID 1500 SEG must contain exactly two per-frame items".into());
    }
    for (index, item) in items.iter_mut().enumerate() {
        let mut matches = 0;
        patch_frame(&mut item.attributes, index as u32 + 1, &mut matches);
        if matches != 1 {
            return Err(format!(
                "TID 1500 SEG frame {} has {matches} source frame declarations",
                index + 1
            ));
        }
    }
    Ok(())
}

pub(crate) fn qualify_tid1500_segmentation_sources(
    spec: &super::CompositionSpec,
    plans: &mut [super::ResolvedInstancePlan],
) -> Result<(), String> {
    for target in spec
        .instances
        .iter()
        .filter(|instance| instance.template.id.0 == "derived/structured-report/tid1500")
    {
        let segmentation_id = target
            .references
            .iter()
            .find(|reference| reference.role == "segmentation_source")
            .map(|reference| reference.target_instance_id.as_str())
            .ok_or_else(|| "TID 1500 target lacks segmentation source".to_string())?;
        let plan = plans
            .iter_mut()
            .find(|plan| plan.instance_id == segmentation_id)
            .ok_or_else(|| "TID 1500 segmentation is not planned".to_string())?;
        qualify_tid1500_segmentation_source(plan)?;
    }
    Ok(())
}

pub(crate) fn is_native_sr_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.template_id.0.as_str(),
        "derived/structured-report/basic-text"
            | "derived/structured-report/comprehensive"
            | "derived/structured-report/key-object"
    )
}

pub(crate) fn is_external_sr_default(template: &TemplateDescriptor) -> bool {
    matches!(
        template.template_id.0.as_str(),
        "derived/structured-report/comprehensive-3d" | "derived/structured-report/tid1500"
    )
}

pub(crate) struct ExternalSrDefaultOutput {
    pub artifact: PlannedImportedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
    pub dependencies: Vec<ArtifactDependency>,
    pub provider: Arc<dyn CompositionExternalDicomProvider>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_external_sr_default(
    recipes: &RecipeCatalog,
    repository_root: &std::path::Path,
    standards_lock_sha256: &str,
    seed: u64,
    member: &AdvancedDefaultMember<'_>,
    sources: &[AdvancedSourceMember],
) -> Result<ExternalSrDefaultOutput, String> {
    let recipe_id = match member.template.template_id.0.as_str() {
        "derived/structured-report/comprehensive-3d" => "derived_sr_comprehensive3d_scoord3d",
        "derived/structured-report/tid1500" => "derived_sr_tid1500_ct_measurement_report",
        other => return Err(format!("unsupported external SR template {other}")),
    };
    let recipe = recipes
        .recipes()
        .iter()
        .find_map(|(identity, recipe)| (identity.recipe_id == recipe_id).then_some(recipe))
        .ok_or_else(|| format!("missing external SR recipe {recipe_id}"))?;
    let artifact_recipe = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .ok_or_else(|| format!("external SR recipe {recipe_id} has no artifact"))?;
    let declarations = recipe
        .provider_parameters
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("external SR recipe {recipe_id} has no sources"))?;
    if declarations.len() != sources.len() {
        return Err(format!(
            "external SR recipe {recipe_id} source cardinality differs from bundle"
        ));
    }
    let semantic_sources = declarations
        .iter()
        .zip(sources)
        .map(|(declaration, source)| {
            semantic_source(declaration, source, &member.instance.instance_id)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let implementation_uid = member
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or_else(|| "external SR target lacks implementation identity".to_string())?;
    let encoding = crate::corpus_plan::EncodingPlan {
        transfer_syntax_uid: artifact_recipe.encoding.transfer_syntax_uid.clone(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: FragmentationPolicy::Native,
        offset_table: OffsetTablePolicy::NotApplicable,
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: ImplementationIdentityPlan {
            class_uid: implementation_uid.into(),
            version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
        },
        backend_id: "external.highdicom_sr".into(),
    };
    let artifact_template = artifact_recipe
        .template
        .as_ref()
        .ok_or_else(|| "external SR artifact lacks template".to_string())?;
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
                    .ok_or_else(|| "external SR artifact lacks output path".to_string())?,
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
            peak_working_bytes: 1 << 20,
        },
    };
    let mut input = sr_input_from_recipe(recipe, context)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "external SR recipe did not produce typed input".to_string())?;
    input.context.logical_id = member.instance.instance_id.clone();
    input.context.order = member.order;
    input.context.output = composition_output(&member.instance.instance_id)?;
    input.context.identities = member.identities.clone();
    for source in &mut input.context.sources {
        source.reference.source_instance_id = member.instance.instance_id.clone();
    }
    let import = SrPlanProvider
        .external_import(&input)
        .map_err(|error| error.to_string())?;
    for (source, composition_reference) in input
        .context
        .sources
        .iter_mut()
        .zip(&member.instance.references)
    {
        source.reference.referenced_frames = composition_reference.frames.clone();
    }
    let mut input_assets = BTreeMap::new();
    let mut source_assets = BTreeMap::new();
    let mut dependencies = Vec::new();
    let mut provider_sources = Vec::new();
    for (index, (semantic, source)) in input.context.sources.iter().zip(sources).enumerate() {
        let binding_role = format!("source_{index}");
        let handle = StagedAssetHandle::new(format!("output:{}", source.artifact.logical_id))
            .map_err(|error| error.to_string())?;
        input_assets.insert(binding_role.clone(), handle.clone());
        source_assets.insert(binding_role.clone(), source.artifact.logical_id.clone());
        dependencies.push(ArtifactDependency {
            artifact_id: member.instance.instance_id.clone(),
            depends_on: source.artifact.logical_id.clone(),
            relationship: semantic.role.clone(),
            frame_numbers: semantic.reference.referenced_frames.clone(),
        });
        let identity = |role| {
            source
                .artifact
                .instance
                .identities
                .get(&role, 0)
                .map(str::to_owned)
                .ok_or_else(|| format!("external SR source lacks {}", role.as_str()))
        };
        provider_sources.push(ExternalSrSource {
            binding_role,
            backend_role: if index == 0 {
                "source_image".into()
            } else {
                "segmentation".into()
            },
            case_id: if index == 0 {
                "enhanced/ct/multiframe_shared_perframe_explicit_le".into()
            } else {
                "derived/seg/binary_multiframe_explicit_le".into()
            },
            sop_class_uid: source.artifact.instance.sop_class_uid.clone(),
            sop_instance_uid: identity(CompositionUidRole::SopInstance)?,
            series_instance_uid: identity(CompositionUidRole::SeriesInstance)?,
            frame_numbers: (!semantic.reference.referenced_frames.is_empty()).then(|| {
                semantic
                    .reference
                    .referenced_frames
                    .iter()
                    .copied()
                    .map(u64::from)
                    .collect()
            }),
        });
    }
    let required_uid = |role| {
        member
            .identities
            .get(&role, 0)
            .map(str::to_owned)
            .ok_or_else(|| format!("external SR target lacks {}", role.as_str()))
    };
    let derived_uid = |index| {
        crate::deterministic_uid(&crate::DeterministicUidInput {
            standards_lock_sha256,
            case_id: &recipe.binding.case_id,
            recipe_version: &recipe.recipe_version,
            run_seed: seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: Some(index),
            role: crate::UidRole::DerivedReference,
        })
    };
    let kind = if member.template.template_id.0.ends_with("tid1500") {
        ExternalSrKind::Tid1500 {
            tracking_uid: derived_uid(0),
            observer_uid: derived_uid(1),
            parameters: crate::generation_backends::Tid1500Parameters {
                measurement_value_mm3: member
                    .instance
                    .parameters
                    .get("measurement_value_mm3")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(crate::generation_backends::TID1500_MEASUREMENT_VALUE),
            },
        }
    } else {
        let graphic_data = member
            .instance
            .parameters
            .get("graphic_data_patient_mm")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                let values = values
                    .iter()
                    .map(serde_json::Value::as_f64)
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| "SCOORD3D graphic data must contain numbers".to_string())?;
                let values: [f64; 6] = values.try_into().map_err(|_| {
                    "SCOORD3D graphic data must contain exactly six numbers".to_string()
                })?;
                Ok::<[[f64; 3]; 2], String>([
                    [values[0], values[1], values[2]],
                    [values[3], values[4], values[5]],
                ])
            })
            .transpose()?
            .unwrap_or(crate::generation_backends::SCOORD3D_GRAPHIC_DATA_PATIENT_MM);
        ExternalSrKind::Comprehensive3d {
            tracking_uid: derived_uid(0),
            observer_uid: derived_uid(1),
            fiducial_uid: derived_uid(2),
            parameters: crate::generation_backends::Scoord3dParameters {
                graphic_data_patient_mm: graphic_data,
                measurement_value_mm: member
                    .instance
                    .parameters
                    .get("measurement_value_mm")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(crate::generation_backends::SCOORD3D_MEASUREMENT_VALUE_MM),
            },
        }
    };
    let caller_parameters = match &kind {
        ExternalSrKind::Comprehensive3d { parameters, .. } => serde_json::json!({
            "graphic_data_patient_mm": parameters.graphic_data_patient_mm,
            "measurement_value_mm": parameters.measurement_value_mm,
        }),
        ExternalSrKind::Tid1500 { parameters, .. } => serde_json::json!({
            "measurement_value_mm3": parameters.measurement_value_mm3,
        }),
    };
    let mut parameters = BTreeMap::from([
        (
            "parameters_sha256".into(),
            serde_json::json!(import.parameters_sha256),
        ),
        (
            "semantic_evidence".into(),
            serde_json::to_value(&import.semantic_evidence).unwrap(),
        ),
    ]);
    if !member.instance.parameters.is_empty() {
        parameters.insert("caller_parameters".into(), caller_parameters);
    }
    let provider_plan = ImportedDicomProviderPlan {
        request_id: format!("{}-{}", import.request_id, member.instance.instance_id),
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
    let mut declared_identities = member.identities.clone();
    declared_identities
        .identities
        .remove("frame_of_reference_uid#0");
    let declared_instance = super::ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: member.instance.instance_id.clone(),
        template_id: member.template.template_id.clone(),
        template_version: member.template.template_version,
        sop_class_uid: member.template.sop_class_uid.clone(),
        transfer_syntax_uid: input.context.encoding.transfer_syntax_uid.clone(),
        identities: declared_identities,
        attributes: vec![],
        content: vec![],
        references: input
            .context
            .sources
            .iter()
            .map(|source| source.reference.clone())
            .collect(),
    };
    let artifact = PlannedImportedDicomArtifact {
        logical_id: member.instance.instance_id.clone(),
        order: member.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: None,
        provider: provider_plan.clone(),
        declared_instance,
        output: composition_output(&member.instance.instance_id)?,
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "composition_imported_sr".into(),
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
                    obligation_id: "external_sr_provider".into(),
                    route_id: import.provider_id.clone(),
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
        artifact_id: artifact.logical_id.clone(),
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
    let provider = Arc::new(ExternalSrProvider {
        repository_root: repository_root.to_owned(),
        standards_lock_path: repository_root.join("standards.lock.json"),
        seed,
        standards_lock_sha256: standards_lock_sha256.into(),
        study_instance_uid: required_uid(CompositionUidRole::StudyInstance)?,
        series_instance_uid: required_uid(CompositionUidRole::SeriesInstance)?,
        frame_of_reference_uid: required_uid(CompositionUidRole::FrameOfReference)?,
        sop_instance_uid: required_uid(CompositionUidRole::SopInstance)?,
        sources: provider_sources,
        kind,
    });
    Ok(ExternalSrDefaultOutput {
        artifact,
        bindings: ArtifactExecutionBindings {
            artifact_id: member.instance.instance_id.clone(),
            slots: BTreeMap::from([(
                provider_plan.output_slot,
                SlotExecutionBinding::ProviderRequest { request },
            )]),
        },
        dependencies,
        provider,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tid_source_graph(
        segmentation_references: serde_json::Value,
    ) -> super::super::CompositionSpec {
        serde_json::from_value(json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                {"instance_id": "ct", "template": {"id": "enhanced/ct/multiframe"}},
                {
                    "instance_id": "seg",
                    "template": {"id": "derived/segmentation/binary"},
                    "references": segmentation_references
                },
                {
                    "instance_id": "report",
                    "template": {"id": "derived/structured-report/tid1500"},
                    "references": [
                        {"role": "image_source", "target_instance_id": "ct", "frames": [1, 2]},
                        {"role": "segmentation_source", "target_instance_id": "seg"}
                    ]
                }
            ]
        }))
        .expect("test source graph")
    }

    #[test]
    fn tid1500_source_graph_requires_one_exact_seg_to_image_edge() {
        let exact = json!([
            {"role": "source_image", "target_instance_id": "bundle-ct", "frames": [1, 2]}
        ]);
        let mut spec = tid_source_graph(exact);
        align_external_sr_source_graph(&mut spec).expect("exact graph");
        let segment = spec
            .instances
            .iter()
            .find(|instance| instance.instance_id == "seg")
            .unwrap();
        assert_eq!(segment.references[0].target_instance_id, "ct");

        for malformed in [
            json!([]),
            json!([
                {"role": "source_image", "target_instance_id": "ct", "frames": [1, 2]},
                {"role": "source_image", "target_instance_id": "ct", "frames": [1, 2]}
            ]),
            json!([{"role": "wrong", "target_instance_id": "ct", "frames": [1, 2]}]),
            json!([{"role": "source_image", "target_instance_id": "ct", "frames": [2, 1]}]),
        ] {
            assert!(align_external_sr_source_graph(&mut tid_source_graph(malformed)).is_err());
        }
    }
}
