//! Direct, typed planning for presentation-state reference graphs.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use dicom_dictionary_std::tags;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AdvancedArtifactPlanningContext, AdvancedArtifactProvenance, AdvancedArtifactRole,
    AdvancedPlanProvider, AdvancedPlanProviderOutput, AdvancedPlanProviderRequest,
    AdvancedPlannedArtifact, AdvancedProviderContractError, AdvancedProviderFamily,
    AdvancedSourceConsumer, AdvancedSourceReference, AdvancedSourceRole, CaseRecipe,
    RecipeReference,
};
use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CompositionUidRole,
    DicomVr, IdentityPlan, MaterializedReference, PrimitiveValue, ResolvedAttribute,
    ResolvedInstancePlan, TemplateId, TemplateVersion, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan,
    EvidencePlan, FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan,
    ItemLengthPolicy, OffsetTablePolicy, OutputPlan, OutputRelativePath, PlannedDicomArtifact,
    PreamblePolicy, SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::ArtifactExecutionBindings;
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

pub const PRESENTATION_ADVANCED_PROVIDER_ID: &str = "native.presentation_state_plan";
pub const PRESENTATION_ALGORITHM_PROVIDER_ID: &str = "algorithm.presentation_state";
const TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
const ICC_COLOR_SPACE: &str = "SRGB";
const ICC_PROFILE_SIZE: usize = 736;
const PROFILE_HEX: &[u8] = include_bytes!("../generator/native/dcmtk_srgb_input_profile.hex");

#[derive(Debug, Clone)]
pub struct PresentationPlanProvider {
    standards_lock_sha256: String,
}

impl PresentationPlanProvider {
    pub fn new(standards_lock_sha256: impl Into<String>) -> Self {
        Self {
            standards_lock_sha256: standards_lock_sha256.into(),
        }
    }

    pub fn recipe_default_contexts(
        &self,
        input: &PresentationPlanInput,
        seed: u64,
    ) -> Result<Vec<AdvancedArtifactPlanningContext>, AdvancedProviderContractError> {
        let sources = validate_sources(input)?;
        let target = input.recipe.logical_id.clone();
        let series_uid = allocated_uid(
            &self.standards_lock_sha256,
            &input.recipe,
            seed,
            UidRole::SeriesInstance,
        );
        let sop_uid = allocated_uid(
            &self.standards_lock_sha256,
            &input.recipe,
            seed,
            UidRole::SopInstance,
        );
        let implementation = implementation_uid(&self.standards_lock_sha256);
        let mut identities = vec![
            (
                CompositionUidRole::StudyInstance,
                0,
                sources[0].study_uid.clone(),
            ),
            (CompositionUidRole::SeriesInstance, 0, series_uid),
            (CompositionUidRole::SopInstance, 0, sop_uid),
            (CompositionUidRole::ImplementationClass, 0, implementation),
        ];
        if matches!(input.recipe.kind, PresentationKind::AdvancedBlending(_)) {
            identities.push((
                CompositionUidRole::FrameOfReference,
                0,
                sources[0]
                    .frame_of_reference_uid
                    .clone()
                    .ok_or_else(|| invalid("source_identity", "frame_of_reference"))?,
            ));
        }
        Ok(vec![AdvancedArtifactPlanningContext {
            recipe_artifact_logical_id: input.recipe.logical_id.clone(),
            target_instance_id: target.clone(),
            order: sources.len() as u64,
            output: OutputPlan {
                relative_path: OutputRelativePath::new(&input.recipe.output_relative_path)
                    .map_err(AdvancedProviderContractError::CorpusPlan)?,
                role: "presentation_state".into(),
                publish: true,
            },
            identities: IdentityPlan::from_exact_values(target, identities)
                .map_err(|error| invalid("identities", &error.to_string()))?,
        }])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationPlanInput {
    pub recipe: PresentationRecipe,
    pub sources: Vec<PresentationSourceInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationSourceInput {
    pub ordinal: u32,
    pub role: AdvancedSourceRole,
    /// One-based frames referenced from this source, empty for whole-instance references.
    pub referenced_frames: Vec<u32>,
    pub artifact: PlannedDicomArtifact,
    pub binding: ArtifactExecutionBindings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationRecipe {
    pub case_id: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub output_relative_path: String,
    pub logical_id: String,
    pub uid_reference_index: Option<u32>,
    pub kind: PresentationKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresentationKind {
    Grayscale(GrayscalePresentationParameters),
    Color(ColorPresentationParameters),
    Blending(BlendingPresentationParameters),
    AdvancedBlending(AdvancedBlendingPresentationParameters),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayedAreaParameters {
    pub top_left: [i32; 2],
    pub bottom_right: [i32; 2],
    pub size_mode: String,
    pub pixel_aspect_ratio: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrayscalePresentationParameters {
    pub expected_source_sop_class_uid: String,
    pub content_label: String,
    pub content_description: String,
    pub displayed_area: DisplayedAreaParameters,
    pub window_center: String,
    pub window_width: String,
    pub window_explanation: String,
    pub presentation_lut_shape: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorPresentationParameters {
    pub expected_source_sop_class_uid: String,
    pub content_label: String,
    pub content_description: String,
    pub displayed_area: DisplayedAreaParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendingPresentationParameters {
    pub expected_source_sop_class_uid: String,
    pub content_label: String,
    pub content_description: String,
    pub displayed_area: DisplayedAreaParameters,
    pub positions: [String; 2],
    pub relative_opacity: f32,
    pub rescale_intercept: String,
    pub rescale_slope: String,
    pub rescale_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedBlendingPresentationParameters {
    pub expected_source_sop_class_uid: String,
    pub content_label: String,
    pub content_description: String,
    pub input_numbers: [u16; 2],
    pub geometry_input_number: u16,
    pub blending_mode: String,
    pub pixel_presentation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationDocumentParameters {
    #[serde(default)]
    uid_reference_index: Option<u32>,
    presentation: PresentationKind,
    sources: Vec<PresentationSourceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationSourceDeclaration {
    recipe: RecipeReference,
    artifact_logical_id: String,
    role: AdvancedSourceRole,
    referenced_frames: Vec<u32>,
}

pub(crate) fn presentation_input_from_recipe(
    recipe: &CaseRecipe,
    sources: Vec<PresentationSourceInput>,
) -> Result<Option<PresentationPlanInput>, String> {
    if recipe.plan_provider_id != PRESENTATION_ADVANCED_PROVIDER_ID {
        return Ok(None);
    }
    validate_presentation_recipe(recipe)?;
    let parameters: PresentationDocumentParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| format!("presentation provider_parameters: {error}"))?;
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| "presentation provider requires DICOM artifacts".to_string())?;
    let [target] = dicom.artifacts.as_slice() else {
        return Err("presentation provider requires exactly one public artifact".into());
    };
    if !target.parameters.is_empty() {
        return Err("presentation target stores static facts in provider_parameters".into());
    }
    let template = target
        .template
        .as_ref()
        .ok_or_else(|| "presentation target requires a template".to_string())?;
    let output = target
        .output
        .path
        .as_ref()
        .ok_or_else(|| "presentation target requires an exact output path".to_string())?;
    if template.template_id != template_id(&parameters.presentation) {
        return Err("presentation template does not match presentation kind".into());
    }
    if parameters.sources.len() != sources.len() {
        return Err("presentation source declaration cardinality mismatch".into());
    }
    let declared_dependencies = recipe
        .dependencies
        .iter()
        .map(|dependency| dependency.recipe.identity())
        .collect::<BTreeSet<_>>();
    let mut remaining = sources;
    let mut ordered = Vec::with_capacity(parameters.sources.len());
    for (index, declaration) in parameters.sources.iter().enumerate() {
        if !declared_dependencies.contains(&declaration.recipe.identity()) {
            return Err("presentation source lacks an outer recipe dependency".into());
        }
        let position = remaining
            .iter()
            .position(|source| {
                source.artifact.logical_id == declaration.artifact_logical_id
                    && source
                        .artifact
                        .case_binding
                        .as_ref()
                        .is_some_and(|binding| {
                            binding.recipe_id == declaration.recipe.recipe_id
                                && binding.recipe_version == declaration.recipe.recipe_version
                        })
            })
            .ok_or_else(|| {
                format!(
                    "missing presentation source {}",
                    declaration.artifact_logical_id
                )
            })?;
        let mut source = remaining.remove(position);
        if source.role != declaration.role {
            return Err(format!(
                "wrong role for presentation source {}",
                declaration.artifact_logical_id
            ));
        }
        source.ordinal = u32::try_from(index + 1).map_err(|_| "source ordinal overflow")?;
        source.referenced_frames = declaration.referenced_frames.clone();
        ordered.push(source);
    }
    if !remaining.is_empty() {
        return Err("presentation contains undeclared sources".into());
    }
    Ok(Some(PresentationPlanInput {
        recipe: PresentationRecipe {
            case_id: recipe.binding.case_id.clone(),
            recipe_id: recipe.recipe_id.clone(),
            recipe_version: recipe.recipe_version.clone(),
            output_relative_path: output.clone(),
            logical_id: target.logical_id.clone(),
            uid_reference_index: parameters.uid_reference_index,
            kind: parameters.presentation,
        },
        sources: ordered,
    }))
}

pub(crate) fn validate_presentation_recipe(recipe: &CaseRecipe) -> Result<(), String> {
    let parameters: PresentationDocumentParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| format!("presentation provider_parameters: {error}"))?;
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| "presentation provider requires DICOM artifacts".to_string())?;
    let [target] = dicom.artifacts.as_slice() else {
        return Err("presentation provider requires exactly one public artifact".into());
    };
    if !target.parameters.is_empty() || parameters.sources.is_empty() {
        return Err("presentation requires one empty target and declared sources".into());
    }
    let dependency_roles = recipe
        .dependencies
        .iter()
        .map(|dependency| (dependency.recipe.identity(), dependency.role.as_str()))
        .collect::<BTreeMap<_, _>>();
    let declared_recipes = parameters
        .sources
        .iter()
        .map(|source| source.recipe.identity())
        .collect::<BTreeSet<_>>();
    if dependency_roles.len() != recipe.dependencies.len()
        || dependency_roles.keys().cloned().collect::<BTreeSet<_>>() != declared_recipes
    {
        return Err("presentation dependencies do not cover source declarations exactly".into());
    }
    let blending = matches!(
        parameters.presentation,
        PresentationKind::Blending(_) | PresentationKind::AdvancedBlending(_)
    );
    let grayscale = matches!(parameters.presentation, PresentationKind::Grayscale(_));
    let expected_count = if blending { 4 } else { 1 };
    if parameters.sources.len() != expected_count {
        return Err("presentation source cardinality does not match its kind".into());
    }
    let mut artifacts = BTreeSet::new();
    for source in &parameters.sources {
        if !artifacts.insert((
            source.recipe.identity(),
            source.artifact_logical_id.as_str(),
        )) {
            return Err("presentation source declarations must be unique".into());
        }
        let expected_dependency_role = if blending {
            "presentation_blending_inputs"
        } else {
            "presentation_source_image"
        };
        if dependency_roles.get(&source.recipe.identity()).copied()
            != Some(expected_dependency_role)
            || (grayscale && source.referenced_frames.is_empty())
            || (!grayscale && !source.referenced_frames.is_empty())
            || (blending
                && !matches!(
                    source.role,
                    AdvancedSourceRole::PresentationBlendingInput { .. }
                ))
            || (!blending && source.role != AdvancedSourceRole::PresentationSourceImage)
        {
            return Err("presentation source role or frame declaration is incompatible".into());
        }
    }
    Ok(())
}

impl AdvancedPlanProvider for PresentationPlanProvider {
    type ProviderInput = PresentationPlanInput;

    fn provider_id(&self) -> &str {
        PRESENTATION_ADVANCED_PROVIDER_ID
    }

    fn plan(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &PresentationPlanInput,
    ) -> Result<AdvancedPlanProviderOutput, AdvancedProviderContractError> {
        request.validate()?;
        validate_request(request, input, self.provider_id())?;
        let sources = validate_sources(input)?;
        let context = request.artifact_context(&input.recipe.logical_id)?;
        let series_uid = context
            .identities
            .get(&CompositionUidRole::SeriesInstance, 0)
            .ok_or_else(|| invalid("identities", "series"))?
            .to_owned();
        let sop_uid = context
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .ok_or_else(|| invalid("identities", "sop"))?
            .to_owned();
        if sources
            .iter()
            .any(|source| source.series_uid == series_uid || source.sop_instance_uid == sop_uid)
        {
            return Err(invalid("presentation_identity", &sop_uid));
        }
        let implementation_uid = context
            .identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .ok_or_else(|| invalid("identities", "implementation"))?
            .to_owned();
        let presentation_id = context.target_instance_id.clone();

        let mut artifacts = Vec::with_capacity(sources.len() + 1);
        let mut bindings = Vec::with_capacity(sources.len() + 1);
        let mut dependencies = Vec::with_capacity(sources.len());
        let mut references = Vec::with_capacity(sources.len());
        let mut materialized_references = Vec::with_capacity(sources.len());
        for (source_input, source) in input.sources.iter().zip(&sources) {
            let mut planned = source_input.artifact.clone();
            planned.provenance = ArtifactProvenance::PrivateSource {
                consumed_by: vec![presentation_id.clone()],
            };
            planned.output.publish = false;
            let consumer = AdvancedSourceConsumer {
                artifact_id: presentation_id.clone(),
                role: source_input.role.clone(),
            };
            artifacts.push(AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::PresentationState {
                    ordinal: source_input.ordinal,
                },
                planned,
                provenance: AdvancedArtifactProvenance::PrivateSource {
                    consumed_by: vec![consumer],
                },
            });
            bindings.push(source_input.binding.clone());
            let relationship = source_input.role.dependency_relationship().to_owned();
            dependencies.push(ArtifactDependency {
                artifact_id: presentation_id.clone(),
                depends_on: source_input.artifact.logical_id.clone(),
                relationship,
                frame_numbers: source.referenced_frames.clone(),
            });
            let reference = MaterializedReference {
                source_instance_id: presentation_id.clone(),
                target_instance_id: source_input.artifact.instance.instance_id.clone(),
                role: reference_role(&input.recipe.kind, &source_input.role).into(),
                frame_role: None,
                referenced_sop_class_uid: source.sop_class_uid.clone(),
                referenced_sop_instance_uid: source.sop_instance_uid.clone(),
                referenced_frames: source.referenced_frames.clone(),
            };
            materialized_references.push(reference.clone());
            references.push(AdvancedSourceReference {
                owner_artifact_id: presentation_id.clone(),
                source_artifact_id: source_input.artifact.logical_id.clone(),
                source_role: source_input.role.clone(),
                reference,
            });
        }

        let attributes = presentation_attributes(input, &sources, &series_uid, &sop_uid);
        if context
            .identities
            .get(&CompositionUidRole::StudyInstance, 0)
            != Some(sources[0].study_uid.as_str())
        {
            return Err(invalid("presentation_identity", "study"));
        }
        let sop_class_uid = sop_class_uid(&input.recipe.kind).to_owned();
        let presentation = PlannedDicomArtifact {
            logical_id: presentation_id.clone(),
            order: context.order,
            provenance: ArtifactProvenance::Requested,
            case_binding: Some(CaseBinding {
                case_id: input.recipe.case_id.clone(),
                recipe_id: input.recipe.recipe_id.clone(),
                recipe_version: input.recipe.recipe_version.clone(),
            }),
            instance: ResolvedInstancePlan {
                plan_schema_version: "0.1.0".into(),
                instance_id: presentation_id.clone(),
                template_id: TemplateId(template_id(&input.recipe.kind).into()),
                template_version: TemplateVersion::from_str("1.0.0").expect("valid version"),
                sop_class_uid: sop_class_uid.clone(),
                transfer_syntax_uid: TRANSFER_SYNTAX_UID.into(),
                identities: context.identities.clone(),
                attributes,
                content: Vec::new(),
                references: materialized_references,
            },
            output: context.output.clone(),
            encoding: encoding(&implementation_uid),
            validation: ValidationPlan {
                rules: vec![ValidationRule {
                    rule_id: "validation.shared".into(),
                    requirement: ValidationRequirement::Required,
                    parameters: BTreeMap::new(),
                }],
            },
            evidence: EvidencePlan {
                obligations: Vec::new(),
            },
            resources: ArtifactResourceEstimate {
                output_bytes: 8 * 1024,
                peak_working_bytes: 16 * 1024,
            },
        };
        artifacts.push(AdvancedPlannedArtifact {
            role: AdvancedArtifactRole::PresentationState {
                ordinal: u32::try_from(sources.len() + 1).expect("bounded sources"),
            },
            planned: presentation,
            provenance: AdvancedArtifactProvenance::Requested,
        });
        bindings.push(ArtifactExecutionBindings {
            artifact_id: presentation_id,
            slots: BTreeMap::new(),
        });
        artifacts.sort_by_key(|artifact| artifact.planned.order);
        let output = AdvancedPlanProviderOutput {
            artifacts,
            dependencies,
            references,
            bindings,
        };
        output.validate(request)?;
        Ok(output)
    }
}

#[derive(Debug)]
struct SourceFacts {
    study_uid: String,
    series_uid: String,
    sop_class_uid: String,
    sop_instance_uid: String,
    frame_of_reference_uid: Option<String>,
    referenced_frames: Vec<u32>,
}

fn validate_request(
    request: &AdvancedPlanProviderRequest,
    input: &PresentationPlanInput,
    provider_id: &str,
) -> Result<(), AdvancedProviderContractError> {
    if request.provider_id != provider_id {
        return Err(invalid("provider_id", &request.provider_id));
    }
    if request.family != AdvancedProviderFamily::PresentationState {
        return Err(AdvancedProviderContractError::FamilyRoleMismatch);
    }
    if request.case_id != input.recipe.case_id
        || request.recipe.recipe_id != input.recipe.recipe_id
        || request.recipe.recipe_version != input.recipe.recipe_version
    {
        return Err(invalid("recipe_id", &request.recipe.recipe_id));
    }
    Ok(())
}

fn validate_sources(
    input: &PresentationPlanInput,
) -> Result<Vec<SourceFacts>, AdvancedProviderContractError> {
    let expected_count = match input.recipe.kind {
        PresentationKind::Grayscale(_) | PresentationKind::Color(_) => 1,
        PresentationKind::Blending(_) | PresentationKind::AdvancedBlending(_) => 4,
    };
    if input.sources.len() != expected_count {
        return Err(AdvancedProviderContractError::UnknownReferenceSource(
            "source_count".into(),
        ));
    }
    let expected_sop = expected_source_sop(&input.recipe.kind);
    if expected_sop.is_empty() {
        return Err(invalid("expected_source_sop_class_uid", expected_sop));
    }
    if let PresentationKind::AdvancedBlending(value) = &input.recipe.kind {
        if value.input_numbers[0] == 0
            || value.input_numbers[1] == 0
            || value.input_numbers[0] == value.input_numbers[1]
            || !value.input_numbers.contains(&value.geometry_input_number)
        {
            return Err(invalid(
                "geometry_input_number",
                &value.geometry_input_number.to_string(),
            ));
        }
    }
    let mut facts = Vec::with_capacity(expected_count);
    let mut logical_ids = BTreeSet::new();
    let mut sop_uids = BTreeSet::new();
    for (index, source) in input.sources.iter().enumerate() {
        if source.ordinal != index as u32 + 1
            || !expected_role(&input.recipe.kind, index, &source.role)
            || source.binding.artifact_id != source.artifact.logical_id
            || source.artifact.instance.instance_id != source.artifact.logical_id
        {
            return Err(AdvancedProviderContractError::ReferenceOwnershipMismatch);
        }
        let is_grayscale = matches!(input.recipe.kind, PresentationKind::Grayscale(_));
        if source.referenced_frames.iter().any(|frame| *frame == 0)
            || source
                .referenced_frames
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || (is_grayscale && source.referenced_frames.is_empty())
            || (!is_grayscale && !source.referenced_frames.is_empty())
        {
            return Err(AdvancedProviderContractError::ReferenceOwnershipMismatch);
        }
        if !logical_ids.insert(source.artifact.logical_id.clone()) {
            return Err(AdvancedProviderContractError::DuplicateArtifact(
                source.artifact.logical_id.clone(),
            ));
        }
        if source.artifact.instance.sop_class_uid != expected_sop {
            return Err(invalid(
                "source_sop_class_uid",
                &source.artifact.instance.sop_class_uid,
            ));
        }
        let identities = &source.artifact.instance.identities;
        let study_uid = identity(identities, CompositionUidRole::StudyInstance)?;
        let series_uid = identity(identities, CompositionUidRole::SeriesInstance)?;
        let sop_instance_uid = identity(identities, CompositionUidRole::SopInstance)?;
        if !sop_uids.insert(sop_instance_uid.clone()) {
            return Err(AdvancedProviderContractError::DuplicateInstance(
                sop_instance_uid,
            ));
        }
        let frame_of_reference_uid = identities
            .get(&CompositionUidRole::FrameOfReference, 0)
            .map(str::to_owned);
        facts.push(SourceFacts {
            study_uid,
            series_uid,
            sop_class_uid: source.artifact.instance.sop_class_uid.clone(),
            sop_instance_uid,
            frame_of_reference_uid,
            referenced_frames: source.referenced_frames.clone(),
        });
    }
    if facts
        .iter()
        .any(|source| source.study_uid != facts[0].study_uid)
    {
        return Err(AdvancedProviderContractError::ReferenceOwnershipMismatch);
    }
    if facts.len() == 4 {
        if facts[0].series_uid != facts[1].series_uid
            || facts[2].series_uid != facts[3].series_uid
            || facts[0].series_uid == facts[2].series_uid
        {
            return Err(AdvancedProviderContractError::ReferenceOwnershipMismatch);
        }
        if facts.iter().any(|source| {
            source.frame_of_reference_uid.is_none()
                || source.frame_of_reference_uid != facts[0].frame_of_reference_uid
        }) {
            return Err(AdvancedProviderContractError::ReferenceOwnershipMismatch);
        }
    }
    Ok(facts)
}

fn identity(
    identities: &IdentityPlan,
    role: CompositionUidRole,
) -> Result<String, AdvancedProviderContractError> {
    identities
        .get(&role, 0)
        .map(str::to_owned)
        .ok_or_else(|| invalid("source_identity", role.as_str()))
}

fn expected_role(kind: &PresentationKind, index: usize, role: &AdvancedSourceRole) -> bool {
    match kind {
        PresentationKind::Grayscale(_) | PresentationKind::Color(_) => {
            *role == AdvancedSourceRole::PresentationSourceImage
        }
        PresentationKind::Blending(_) | PresentationKind::AdvancedBlending(_) => {
            *role
                == AdvancedSourceRole::PresentationBlendingInput {
                    input_number: if index < 2 { 1 } else { 2 },
                }
        }
    }
}

fn expected_source_sop(kind: &PresentationKind) -> &str {
    match kind {
        PresentationKind::Grayscale(value) => &value.expected_source_sop_class_uid,
        PresentationKind::Color(value) => &value.expected_source_sop_class_uid,
        PresentationKind::Blending(value) => &value.expected_source_sop_class_uid,
        PresentationKind::AdvancedBlending(value) => &value.expected_source_sop_class_uid,
    }
}

fn presentation_attributes(
    input: &PresentationPlanInput,
    sources: &[SourceFacts],
    series_uid: &str,
    sop_uid: &str,
) -> Vec<ResolvedAttribute> {
    match &input.recipe.kind {
        PresentationKind::Grayscale(value) => {
            grayscale_attributes(&input.recipe, value, &sources[0], series_uid, sop_uid)
        }
        PresentationKind::Color(value) => color_attributes(value, &sources[0], series_uid, sop_uid),
        PresentationKind::Blending(value) => {
            blending_attributes(value, sources, series_uid, sop_uid)
        }
        PresentationKind::AdvancedBlending(value) => {
            advanced_blending_attributes(value, sources, series_uid, sop_uid)
        }
    }
}

fn grayscale_attributes(
    recipe: &PresentationRecipe,
    value: &GrayscalePresentationParameters,
    source: &SourceFacts,
    series_uid: &str,
    sop_uid: &str,
) -> Vec<ResolvedAttribute> {
    let mut values = common_attributes(
        sop_class_uid(&recipe.kind),
        sop_uid,
        source,
        series_uid,
        "61",
        &recipe.recipe_id,
        "DTS-GSPS-0001",
        false,
        "DTS-GSPS",
    );
    values.extend([
        set_string(tags::CONTENT_DATE, DicomVr::DA, "20260101"),
        set_string(tags::CONTENT_TIME, DicomVr::TM, "000000"),
    ]);
    values.extend(content_attributes(
        &value.content_label,
        &value.content_description,
    ));
    values.extend(referenced_series(std::slice::from_ref(source), false));
    values.extend(displayed_area(&value.displayed_area));
    values.extend([
        sequence(
            tags::SOFTCOPY_VOILUT_SEQUENCE,
            vec![item(vec![
                set_string(tags::WINDOW_CENTER, DicomVr::DS, &value.window_center),
                set_string(tags::WINDOW_WIDTH, DicomVr::DS, &value.window_width),
                set_string(
                    tags::WINDOW_CENTER_WIDTH_EXPLANATION,
                    DicomVr::LO,
                    &value.window_explanation,
                ),
            ])],
        ),
        set_string(
            tags::PRESENTATION_LUT_SHAPE,
            DicomVr::CS,
            &value.presentation_lut_shape,
        ),
    ]);
    resolved(values)
}

fn color_attributes(
    value: &ColorPresentationParameters,
    source: &SourceFacts,
    series_uid: &str,
    sop_uid: &str,
) -> Vec<ResolvedAttribute> {
    let mut values = common_attributes(
        "1.2.840.10008.5.1.4.1.1.11.2",
        sop_uid,
        source,
        series_uid,
        "62",
        "Native Color Softcopy Presentation State",
        "DTS-COLOR-PR-0001",
        true,
        "SMOKE",
    );
    values.extend([
        set_string(tags::BODY_PART_EXAMINED, DicomVr::CS, "HAND"),
        set_string(tags::LATERALITY, DicomVr::CS, "R"),
    ]);
    values.extend(content_attributes(
        &value.content_label,
        &value.content_description,
    ));
    values.extend(referenced_series(std::slice::from_ref(source), false));
    values.extend(displayed_area(&value.displayed_area));
    values.extend([
        set_binary(tags::ICC_PROFILE, DicomVr::OB, icc_profile()),
        set_string(tags::COLOR_SPACE, DicomVr::CS, ICC_COLOR_SPACE),
    ]);
    resolved(values)
}

fn blending_attributes(
    value: &BlendingPresentationParameters,
    sources: &[SourceFacts],
    series_uid: &str,
    sop_uid: &str,
) -> Vec<ResolvedAttribute> {
    let mut values = common_attributes(
        "1.2.840.10008.5.1.4.1.1.11.4",
        sop_uid,
        &sources[0],
        series_uid,
        "81",
        "Native Blending Softcopy Presentation State",
        "DTS-BLEND-001",
        false,
        "DTS-CT",
    );
    values.extend([
        set_string(tags::LATERALITY, DicomVr::CS, "R"),
        empty(tags::INSTITUTION_NAME),
        empty(tags::INSTITUTION_ADDRESS),
    ]);
    values.extend(content_attributes(
        &value.content_label,
        &value.content_description,
    ));
    values.extend([
        sequence(
            tags::BLENDING_SEQUENCE,
            vec![
                blending_item(&value.positions[0], &sources[0..2], value),
                blending_item(&value.positions[1], &sources[2..4], value),
            ],
        ),
        set_f32(tags::RELATIVE_OPACITY, value.relative_opacity),
    ]);
    values.extend(displayed_area(&value.displayed_area));
    values.extend([
        palette_descriptor(tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR),
        palette_descriptor(tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR),
        palette_descriptor(tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR),
        set_binary(
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            DicomVr::OW,
            palette(),
        ),
        set_binary(
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            DicomVr::OW,
            palette(),
        ),
        set_binary(
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            DicomVr::OW,
            palette(),
        ),
        set_binary(tags::ICC_PROFILE, DicomVr::OB, icc_profile()),
        set_string(tags::COLOR_SPACE, DicomVr::CS, ICC_COLOR_SPACE),
    ]);
    resolved(values)
}

fn advanced_blending_attributes(
    value: &AdvancedBlendingPresentationParameters,
    sources: &[SourceFacts],
    series_uid: &str,
    sop_uid: &str,
) -> Vec<ResolvedAttribute> {
    let mut values = common_attributes(
        "1.2.840.10008.5.1.4.1.1.11.8",
        sop_uid,
        &sources[0],
        series_uid,
        "80",
        "Native Advanced Blending Presentation State",
        "DTS-ADVBLEND-001",
        false,
        "DTS-CT",
    );
    values.extend([
        set_string(tags::LATERALITY, DicomVr::CS, "R"),
        empty(tags::INSTITUTION_NAME),
        empty(tags::INSTITUTION_ADDRESS),
    ]);
    values.extend(content_attributes(
        &value.content_label,
        &value.content_description,
    ));
    values.extend([
        set_string(
            tags::FRAME_OF_REFERENCE_UID,
            DicomVr::UI,
            sources[0].frame_of_reference_uid.as_deref().unwrap(),
        ),
        empty(tags::POSITION_REFERENCE_INDICATOR),
        sequence(
            tags::ADVANCED_BLENDING_SEQUENCE,
            vec![
                advanced_blending_item(
                    value.input_numbers[0],
                    &sources[0..2],
                    value.input_numbers[0] == value.geometry_input_number,
                ),
                advanced_blending_item(
                    value.input_numbers[1],
                    &sources[2..4],
                    value.input_numbers[1] == value.geometry_input_number,
                ),
            ],
        ),
        set_string(
            tags::PIXEL_PRESENTATION,
            DicomVr::CS,
            &value.pixel_presentation,
        ),
        sequence(
            tags::BLENDING_DISPLAY_SEQUENCE,
            vec![item(vec![
                sequence(
                    tags::BLENDING_DISPLAY_INPUT_SEQUENCE,
                    value
                        .input_numbers
                        .iter()
                        .map(|number| {
                            item(vec![set_unsigned(
                                tags::BLENDING_INPUT_NUMBER,
                                DicomVr::US,
                                u64::from(*number),
                            )])
                        })
                        .collect(),
                ),
                set_string(tags::BLENDING_MODE, DicomVr::CS, &value.blending_mode),
            ])],
        ),
        set_binary(tags::ICC_PROFILE, DicomVr::OB, icc_profile()),
        set_string(tags::COLOR_SPACE, DicomVr::CS, ICC_COLOR_SPACE),
    ]);
    values.extend(referenced_series(sources, true));
    resolved(values)
}

fn common_attributes(
    sop_class_uid: &str,
    sop_uid: &str,
    source: &SourceFacts,
    series_uid: &str,
    series_number: &str,
    model: &str,
    serial: &str,
    color_source: bool,
    study_id: &str,
) -> Vec<AttributeOperation> {
    let patient_name = if color_source {
        "DICOMTEST^SMOKE"
    } else {
        "DTS^Synthetic^Patient001"
    };
    let patient_id = if color_source {
        "DICOMTEST-SMOKE-001"
    } else {
        "DTS-PATIENT-001"
    };
    vec![
        set_string(tags::SOP_CLASS_UID, DicomVr::UI, sop_class_uid),
        set_string(tags::SOP_INSTANCE_UID, DicomVr::UI, sop_uid),
        set_string(tags::SYNTHETIC_DATA, DicomVr::CS, "YES"),
        set_string(tags::PATIENT_NAME, DicomVr::PN, patient_name),
        set_string(tags::PATIENT_ID, DicomVr::LO, patient_id),
        set_string(tags::PATIENT_BIRTH_DATE, DicomVr::DA, "19700101"),
        set_string(tags::PATIENT_SEX, DicomVr::CS, "O"),
        set_string(tags::STUDY_INSTANCE_UID, DicomVr::UI, &source.study_uid),
        set_string(tags::STUDY_DATE, DicomVr::DA, "20260101"),
        set_string(tags::STUDY_TIME, DicomVr::TM, "000000"),
        empty(tags::REFERRING_PHYSICIAN_NAME),
        set_string(tags::STUDY_ID, DicomVr::SH, study_id),
        empty(tags::ACCESSION_NUMBER),
        set_string(tags::MODALITY, DicomVr::CS, "PR"),
        set_string(tags::SERIES_INSTANCE_UID, DicomVr::UI, series_uid),
        set_string(tags::SERIES_NUMBER, DicomVr::IS, series_number),
        set_string(tags::MANUFACTURER, DicomVr::LO, "dicom-test-suite"),
        set_string(tags::MANUFACTURER_MODEL_NAME, DicomVr::LO, model),
        set_string(tags::DEVICE_SERIAL_NUMBER, DicomVr::LO, serial),
        set_string(tags::SOFTWARE_VERSIONS, DicomVr::LO, crate::PACKAGE_VERSION),
        set_string(tags::INSTANCE_NUMBER, DicomVr::IS, "1"),
    ]
}

fn content_attributes(label: &str, description: &str) -> Vec<AttributeOperation> {
    vec![
        set_string(tags::PRESENTATION_CREATION_DATE, DicomVr::DA, "20260101"),
        set_string(tags::PRESENTATION_CREATION_TIME, DicomVr::TM, "000000"),
        set_string(tags::CONTENT_LABEL, DicomVr::CS, label),
        set_string(tags::CONTENT_DESCRIPTION, DicomVr::LO, description),
        set_string(tags::CONTENT_CREATOR_NAME, DicomVr::PN, "DTS^Generator"),
    ]
}

fn referenced_series(sources: &[SourceFacts], instances: bool) -> Vec<AttributeOperation> {
    let mut series = Vec::new();
    for group in sources.chunks(2.min(sources.len())) {
        series.push(item(vec![
            set_string(tags::SERIES_INSTANCE_UID, DicomVr::UI, &group[0].series_uid),
            sequence(
                if instances {
                    tags::REFERENCED_INSTANCE_SEQUENCE
                } else {
                    tags::REFERENCED_IMAGE_SEQUENCE
                },
                group.iter().map(referenced_sop_item).collect(),
            ),
        ]));
    }
    vec![sequence(tags::REFERENCED_SERIES_SEQUENCE, series)]
}

fn referenced_sop_item(source: &SourceFacts) -> AttributeItem {
    item(vec![
        set_string(
            tags::REFERENCED_SOP_CLASS_UID,
            DicomVr::UI,
            &source.sop_class_uid,
        ),
        set_string(
            tags::REFERENCED_SOP_INSTANCE_UID,
            DicomVr::UI,
            &source.sop_instance_uid,
        ),
    ])
}

fn displayed_area(value: &DisplayedAreaParameters) -> Vec<AttributeOperation> {
    vec![sequence(
        tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
        vec![item(vec![
            set_multi_signed(
                tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
                DicomVr::SL,
                value.top_left.map(i64::from).to_vec(),
            ),
            set_multi_signed(
                tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
                DicomVr::SL,
                value.bottom_right.map(i64::from).to_vec(),
            ),
            set_string(tags::PRESENTATION_SIZE_MODE, DicomVr::CS, &value.size_mode),
            set_multi_string(
                tags::PRESENTATION_PIXEL_ASPECT_RATIO,
                DicomVr::IS,
                value
                    .pixel_aspect_ratio
                    .map(|value| value.to_string())
                    .to_vec(),
            ),
        ])],
    )]
}

fn blending_item(
    position: &str,
    sources: &[SourceFacts],
    value: &BlendingPresentationParameters,
) -> AttributeItem {
    item(vec![
        set_string(tags::BLENDING_POSITION, DicomVr::CS, position),
        set_string(tags::STUDY_INSTANCE_UID, DicomVr::UI, &sources[0].study_uid),
        referenced_series(sources, false).remove(0),
        set_string(
            tags::RESCALE_INTERCEPT,
            DicomVr::DS,
            &value.rescale_intercept,
        ),
        set_string(tags::RESCALE_SLOPE, DicomVr::DS, &value.rescale_slope),
        set_string(tags::RESCALE_TYPE, DicomVr::LO, &value.rescale_type),
    ])
}

fn advanced_blending_item(number: u16, sources: &[SourceFacts], geometry: bool) -> AttributeItem {
    item(vec![
        set_unsigned(tags::BLENDING_INPUT_NUMBER, DicomVr::US, u64::from(number)),
        set_string(tags::STUDY_INSTANCE_UID, DicomVr::UI, &sources[0].study_uid),
        set_string(
            tags::SERIES_INSTANCE_UID,
            DicomVr::UI,
            &sources[0].series_uid,
        ),
        sequence(
            tags::REFERENCED_IMAGE_SEQUENCE,
            sources.iter().map(referenced_sop_item).collect(),
        ),
        set_string(tags::TIME_SERIES_BLENDING, DicomVr::CS, "FALSE"),
        set_string(
            tags::GEOMETRY_FOR_DISPLAY,
            DicomVr::CS,
            if geometry { "TRUE" } else { "FALSE" },
        ),
    ])
}

fn palette_descriptor(tag: dicom_core::Tag) -> AttributeOperation {
    set_multi_unsigned(tag, DicomVr::US, vec![256, 0, 16])
}

fn palette() -> Vec<u8> {
    (0_u16..=255)
        .flat_map(|entry| (entry * 0x0101).to_le_bytes())
        .collect()
}

fn item(mut attributes: Vec<AttributeOperation>) -> AttributeItem {
    attributes.sort_by_key(|operation| operation.address().clone());
    AttributeItem { attributes }
}

fn sequence(tag: dicom_core::Tag, items: Vec<AttributeItem>) -> AttributeOperation {
    set_value(tag, DicomVr::SQ, AttributeValue::Sequence(items))
}

fn set_string(tag: dicom_core::Tag, vr: DicomVr, value: &str) -> AttributeOperation {
    set_multi_string(tag, vr, value.split('\\').map(str::to_owned).collect())
}

fn set_multi_string(tag: dicom_core::Tag, vr: DicomVr, values: Vec<String>) -> AttributeOperation {
    let values = values
        .into_iter()
        .map(PrimitiveValue::String)
        .collect::<Vec<_>>();
    set_value(
        tag,
        vr,
        if values.len() == 1 {
            AttributeValue::Primitive(values.into_iter().next().unwrap())
        } else {
            AttributeValue::Multi(values)
        },
    )
}

fn set_unsigned(tag: dicom_core::Tag, vr: DicomVr, value: u64) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    )
}

fn set_multi_unsigned(tag: dicom_core::Tag, vr: DicomVr, values: Vec<u64>) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Multi(values.into_iter().map(PrimitiveValue::Unsigned).collect()),
    )
}

fn set_multi_signed(tag: dicom_core::Tag, vr: DicomVr, values: Vec<i64>) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Multi(values.into_iter().map(PrimitiveValue::Signed).collect()),
    )
}

fn set_f32(tag: dicom_core::Tag, value: f32) -> AttributeOperation {
    set_value(
        tag,
        DicomVr::FL,
        AttributeValue::Primitive(PrimitiveValue::Float32Bits(value.to_bits())),
    )
}

fn set_binary(tag: dicom_core::Tag, vr: DicomVr, bytes: Vec<u8>) -> AttributeOperation {
    set_value(tag, vr, AttributeValue::Binary(bytes))
}

fn empty(tag: dicom_core::Tag) -> AttributeOperation {
    AttributeOperation::Empty {
        address: AttributeAddress::standard(tag).expect("standard tag"),
    }
}

fn set_value(tag: dicom_core::Tag, vr: DicomVr, value: AttributeValue) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::standard(tag).expect("standard tag"),
        vr,
        value,
    }
}

fn resolved(mut operations: Vec<AttributeOperation>) -> Vec<ResolvedAttribute> {
    operations.sort_by_key(|operation| operation.address().clone());
    operations
        .into_iter()
        .map(|operation| match operation {
            AttributeOperation::Set { address, vr, value } => ResolvedAttribute {
                address,
                vr,
                value: Some(value),
                origin: ValueOrigin::InstanceOverride,
            },
            AttributeOperation::Empty { address } => ResolvedAttribute {
                vr: vr_for(&address),
                address,
                value: None,
                origin: ValueOrigin::InstanceOverride,
            },
            AttributeOperation::Remove { .. } => unreachable!(),
        })
        .collect()
}

fn vr_for(address: &AttributeAddress) -> DicomVr {
    use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
    let entry = dicom_dictionary_std::StandardDataDictionary
        .by_tag(address.tag())
        .expect("standard tag");
    let vr = match entry.vr() {
        VirtualVr::Exact(value) => value,
        other => other.exact().expect("exact VR"),
    };
    DicomVr::from_str(&vr.to_string()).expect("supported VR")
}

fn encoding(implementation_uid: &str) -> EncodingPlan {
    EncodingPlan {
        transfer_syntax_uid: TRANSFER_SYNTAX_UID.into(),
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
        backend_id: "dicom-rs.part10".into(),
    }
}

fn allocated_uid(lock: &str, recipe: &PresentationRecipe, seed: u64, role: UidRole) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: &recipe.case_id,
        recipe_version: &recipe.recipe_version,
        run_seed: seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: recipe.uid_reference_index,
        role,
    })
}

fn implementation_uid(lock: &str) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    })
}

fn reference_role(kind: &PresentationKind, role: &AdvancedSourceRole) -> &'static str {
    match role {
        AdvancedSourceRole::PresentationSourceImage => "source_image",
        AdvancedSourceRole::PresentationBlendingInput { .. }
            if matches!(kind, PresentationKind::Blending(_)) =>
        {
            "blending_source"
        }
        AdvancedSourceRole::PresentationBlendingInput { .. } => "blending_input",
        _ => "invalid_presentation_role",
    }
}

fn sop_class_uid(kind: &PresentationKind) -> &'static str {
    match kind {
        PresentationKind::Grayscale(_) => "1.2.840.10008.5.1.4.1.1.11.1",
        PresentationKind::Color(_) => "1.2.840.10008.5.1.4.1.1.11.2",
        PresentationKind::Blending(_) => "1.2.840.10008.5.1.4.1.1.11.4",
        PresentationKind::AdvancedBlending(_) => "1.2.840.10008.5.1.4.1.1.11.8",
    }
}

fn template_id(kind: &PresentationKind) -> &'static str {
    match kind {
        PresentationKind::Grayscale(_) => "derived/presentation-state/grayscale",
        PresentationKind::Color(_) => "derived/presentation-state/color",
        PresentationKind::Blending(_) => "derived/presentation-state/blending",
        PresentationKind::AdvancedBlending(_) => "derived/presentation-state/advanced-blending",
    }
}

fn icc_profile() -> Vec<u8> {
    let mut output = Vec::with_capacity(ICC_PROFILE_SIZE);
    let mut high = None;
    for byte in PROFILE_HEX
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
    {
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => unreachable!("locked ICC source is hexadecimal"),
        };
        if let Some(high) = high.take() {
            output.push((high << 4) | nibble);
        } else {
            high = Some(nibble);
        }
    }
    assert_eq!(output.len(), ICC_PROFILE_SIZE);
    output
}

fn invalid(field: &'static str, value: &str) -> AdvancedProviderContractError {
    AdvancedProviderContractError::InvalidIdentifier {
        field,
        value: value.into(),
    }
}
