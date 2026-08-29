//! Frontend-neutral quantitative DICOM planning contracts.
//!
//! Native SEG and RWVM are completely resolved before staging. External
//! Parametric Map and WSI-tile SEG cases stop at a bounded, typed import
//! request whose dependencies and semantic evidence are explicit.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use serde::{Deserialize, Serialize};

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    CompositionUidRole, ContentMaterialization, DicomVr, IdentityPlan, MaterializedReference,
    PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan, TemplateId, TemplateVersion,
    ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan,
    EvidencePlan, FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan,
    ItemLengthPolicy, OffsetTablePolicy, OutputPlan, PlannedDicomArtifact, PreamblePolicy,
    SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, NativeFrameBinding, SlotExecutionBinding,
};
use crate::planning::RecipeIdentity;

use super::{
    CaseRecipe, ContentByteOrder, ContentProviderLimits, ContentProviderRequest, ContentTarget,
    IntegerPixelsContract, IntegerSamples, NeutralContentProvider, RecipeReference,
};

pub const QUANTITATIVE_NATIVE_PROVIDER_ID: &str = "native.quantitative_plan";
pub const QUANTITATIVE_EXTERNAL_PROVIDER_ID: &str = "external.quantitative_import_plan";
const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const DEFLATED_IMAGE_FRAME: &str = "1.2.840.10008.1.2.8.1";
const SEGMENTATION_SOP: &str = "1.2.840.10008.5.1.4.1.1.66.4";
const LABELMAP_SOP: &str = "1.2.840.10008.5.1.4.1.1.66.7";
const RWVM_SOP: &str = "1.2.840.10008.5.1.4.1.1.67";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantitativeProviderLimits {
    pub max_sources: u32,
    pub max_frames: u32,
    pub max_elements: u64,
    pub max_output_bytes: u64,
    pub max_external_seconds: u32,
}

impl Default for QuantitativeProviderLimits {
    fn default() -> Self {
        Self {
            max_sources: 16,
            max_frames: 256,
            max_elements: 16 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_external_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantitativeArtifactContext {
    pub recipe_artifact_logical_id: String,
    pub target_instance_id: String,
    pub order: u64,
    pub output: OutputPlan,
    pub identities: IdentityPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantitativeSourceRole {
    SegmentationSourceImage,
    RealWorldValueSourceImage,
    ParametricMapSourceImage,
    WholeSlideSourceImage,
}

impl QuantitativeSourceRole {
    fn relationship(self) -> &'static str {
        match self {
            Self::SegmentationSourceImage => "source_image_for_segmentation",
            Self::RealWorldValueSourceImage => "source_image",
            Self::ParametricMapSourceImage => "source_image_for_parametric_map",
            Self::WholeSlideSourceImage => "source_image_for_segmentation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantitativeSourceInput {
    pub role: QuantitativeSourceRole,
    pub artifact: PlannedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
    #[serde(default)]
    pub referenced_frames: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentationKind {
    Binary,
    FractionalProbability,
    Labelmap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentationInput {
    pub kind: SegmentationKind,
    pub rows: u16,
    pub columns: u16,
    pub frames: u16,
    pub transfer_syntax_uid: String,
    pub segment_label: String,
    pub segment_number: u16,
    pub stored_values: Vec<u8>,
    pub visual_pattern: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealWorldValueMappingInput {
    pub content_label: String,
    pub content_description: String,
    pub lut_label: String,
    pub first_value_mapped: u16,
    pub last_value_mapped: u16,
    pub intercept: f64,
    pub slope: f64,
    pub unit_code_value: String,
    pub unit_coding_scheme_designator: String,
    pub unit_code_meaning: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportKind {
    ParametricMapFloat32,
    ParametricMapFloat64,
    WholeSlideTileSegmentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalDependencyContract {
    pub executable_provider_id: String,
    pub required_tool_version: String,
    pub dependency_lock_sha256: String,
    pub protocol_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalSemanticEvidence {
    pub sop_class_uid: String,
    pub transfer_syntax_uid: String,
    pub pixel_vr: String,
    pub frame_count: u32,
    pub rows: u32,
    pub columns: u32,
    pub source_frame_numbers: Vec<u32>,
    pub required_validation_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalImportBoundary {
    pub kind: ExternalImportKind,
    pub request_id: String,
    pub output_media_type: String,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u32,
    pub dependency: ExternalDependencyContract,
    pub semantic_evidence: ExternalSemanticEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantitativePlanInput {
    NativeSeg {
        recipe: RecipeIdentity,
        case_id: String,
        artifact: QuantitativeArtifactContext,
        segmentation: SegmentationInput,
        sources: Vec<QuantitativeSourceInput>,
    },
    NativeRwvm {
        recipe: RecipeIdentity,
        case_id: String,
        artifact: QuantitativeArtifactContext,
        mapping: RealWorldValueMappingInput,
        sources: Vec<QuantitativeSourceInput>,
    },
    ExternalImport {
        recipe: RecipeIdentity,
        case_id: String,
        artifact: QuantitativeArtifactContext,
        import: ExternalImportBoundary,
        sources: Vec<QuantitativeSourceInput>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuantitativeSourceDeclaration {
    recipe: RecipeReference,
    artifact_logical_id: String,
    role: QuantitativeSourceRole,
    #[serde(default)]
    referenced_frames: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSegDocumentParameters {
    segmentation: SegmentationInput,
    sources: Vec<QuantitativeSourceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRwvmDocumentParameters {
    mapping: RealWorldValueMappingInput,
    sources: Vec<QuantitativeSourceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalDocumentParameters {
    import: ExternalImportBoundary,
    sources: Vec<QuantitativeSourceDeclaration>,
}

pub fn quantitative_input_from_recipe(
    recipe: &CaseRecipe,
    artifact: QuantitativeArtifactContext,
    sources: Vec<QuantitativeSourceInput>,
) -> Result<Option<QuantitativePlanInput>, QuantitativePlanError> {
    let expected = match recipe.plan_provider_id.as_str() {
        QUANTITATIVE_NATIVE_PROVIDER_ID | QUANTITATIVE_EXTERNAL_PROVIDER_ID => recipe
            .dicom
            .as_ref()
            .and_then(|dicom| dicom.artifacts.first())
            .ok_or(QuantitativePlanError::Document(
                "quantitative recipe requires one DICOM artifact".into(),
            ))?,
        _ => return Ok(None),
    };
    if recipe
        .dicom
        .as_ref()
        .is_none_or(|dicom| dicom.artifacts.len() != 1)
        || expected.logical_id != artifact.recipe_artifact_logical_id
        || expected.output.path.as_deref() != Some(artifact.output.relative_path.as_str())
        || expected.order as u64 != artifact.order
    {
        return Err(QuantitativePlanError::Document(
            "quantitative artifact context differs from recipe".into(),
        ));
    }
    let (declarations, result) = if recipe.plan_provider_id == QUANTITATIVE_EXTERNAL_PROVIDER_ID {
        let parameters: ExternalDocumentParameters = serde_json::from_value(
            serde_json::Value::Object(recipe.provider_parameters.clone()),
        )
        .map_err(|error| QuantitativePlanError::Document(error.to_string()))?;
        let declarations = parameters.sources.clone();
        (
            declarations,
            QuantitativePlanInput::ExternalImport {
                recipe: recipe.identity(),
                case_id: recipe.binding.case_id.clone(),
                artifact,
                import: parameters.import,
                sources: Vec::new(),
            },
        )
    } else if recipe.provider_parameters.contains_key("segmentation") {
        let parameters: NativeSegDocumentParameters = serde_json::from_value(
            serde_json::Value::Object(recipe.provider_parameters.clone()),
        )
        .map_err(|error| QuantitativePlanError::Document(error.to_string()))?;
        let declarations = parameters.sources.clone();
        (
            declarations,
            QuantitativePlanInput::NativeSeg {
                recipe: recipe.identity(),
                case_id: recipe.binding.case_id.clone(),
                artifact,
                segmentation: parameters.segmentation,
                sources: Vec::new(),
            },
        )
    } else {
        let parameters: NativeRwvmDocumentParameters = serde_json::from_value(
            serde_json::Value::Object(recipe.provider_parameters.clone()),
        )
        .map_err(|error| QuantitativePlanError::Document(error.to_string()))?;
        let declarations = parameters.sources.clone();
        (
            declarations,
            QuantitativePlanInput::NativeRwvm {
                recipe: recipe.identity(),
                case_id: recipe.binding.case_id.clone(),
                artifact,
                mapping: parameters.mapping,
                sources: Vec::new(),
            },
        )
    };
    let ordered = bind_declared_sources(recipe, &declarations, sources)?;
    Ok(Some(match result {
        QuantitativePlanInput::NativeSeg {
            recipe,
            case_id,
            artifact,
            segmentation,
            ..
        } => QuantitativePlanInput::NativeSeg {
            recipe,
            case_id,
            artifact,
            segmentation,
            sources: ordered,
        },
        QuantitativePlanInput::NativeRwvm {
            recipe,
            case_id,
            artifact,
            mapping,
            ..
        } => QuantitativePlanInput::NativeRwvm {
            recipe,
            case_id,
            artifact,
            mapping,
            sources: ordered,
        },
        QuantitativePlanInput::ExternalImport {
            recipe,
            case_id,
            artifact,
            import,
            ..
        } => QuantitativePlanInput::ExternalImport {
            recipe,
            case_id,
            artifact,
            import,
            sources: ordered,
        },
    }))
}

fn bind_declared_sources(
    recipe: &CaseRecipe,
    declarations: &[QuantitativeSourceDeclaration],
    mut sources: Vec<QuantitativeSourceInput>,
) -> Result<Vec<QuantitativeSourceInput>, QuantitativePlanError> {
    if declarations.len() != sources.len() {
        return Err(QuantitativePlanError::Document(
            "quantitative source cardinality mismatch".into(),
        ));
    }
    let dependencies = recipe
        .dependencies
        .iter()
        .map(|dependency| dependency.recipe.identity())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(sources.len());
    for declaration in declarations {
        if !dependencies.contains(&declaration.recipe.identity()) {
            return Err(QuantitativePlanError::Document(
                "quantitative source lacks recipe dependency".into(),
            ));
        }
        let position = sources
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
                QuantitativePlanError::Document(format!(
                    "missing quantitative source {}",
                    declaration.artifact_logical_id
                ))
            })?;
        let mut source = sources.remove(position);
        if source.role != declaration.role {
            return Err(QuantitativePlanError::Document(
                "quantitative source role mismatch".into(),
            ));
        }
        source.referenced_frames = declaration.referenced_frames.clone();
        ordered.push(source);
    }
    if !sources.is_empty() {
        return Err(QuantitativePlanError::Document(
            "undeclared quantitative source".into(),
        ));
    }
    Ok(ordered)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantitativePlanOutput {
    Native {
        artifact: PlannedDicomArtifact,
        bindings: ArtifactExecutionBindings,
        dependencies: Vec<ArtifactDependency>,
    },
    ExternalImport {
        recipe: RecipeIdentity,
        case_id: String,
        artifact: QuantitativeArtifactContext,
        import: ExternalImportBoundary,
        sources: Vec<QuantitativeSourceInput>,
        dependencies: Vec<ArtifactDependency>,
        references: Vec<MaterializedReference>,
    },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct QuantitativePlanProvider;

impl QuantitativePlanProvider {
    pub fn plan(
        &self,
        input: &QuantitativePlanInput,
        limits: QuantitativeProviderLimits,
    ) -> Result<QuantitativePlanOutput, QuantitativePlanError> {
        validate_limits(limits)?;
        match input {
            QuantitativePlanInput::NativeSeg {
                recipe,
                case_id,
                artifact,
                segmentation,
                sources,
            } => plan_seg(recipe, case_id, artifact, segmentation, sources, limits),
            QuantitativePlanInput::NativeRwvm {
                recipe,
                case_id,
                artifact,
                mapping,
                sources,
            } => plan_rwvm(recipe, case_id, artifact, mapping, sources, limits),
            QuantitativePlanInput::ExternalImport {
                recipe,
                case_id,
                artifact,
                import,
                sources,
            } => plan_external(recipe, case_id, artifact, import, sources, limits),
        }
    }
}

fn plan_seg(
    recipe: &RecipeIdentity,
    case_id: &str,
    context: &QuantitativeArtifactContext,
    input: &SegmentationInput,
    sources: &[QuantitativeSourceInput],
    limits: QuantitativeProviderLimits,
) -> Result<QuantitativePlanOutput, QuantitativePlanError> {
    validate_context(context)?;
    validate_sources(
        sources,
        QuantitativeSourceRole::SegmentationSourceImage,
        limits,
    )?;
    if sources.len() != 1
        || input.rows == 0
        || input.columns == 0
        || input.frames == 0
        || u32::from(input.frames) > limits.max_frames
        || input.segment_number == 0
    {
        return Err(QuantitativePlanError::InvalidSegmentation);
    }
    let count = usize::from(input.rows)
        .checked_mul(usize::from(input.columns))
        .and_then(|value| value.checked_mul(usize::from(input.frames)))
        .ok_or(QuantitativePlanError::ResourceOverflow)?;
    if count != input.stored_values.len() || count as u64 > limits.max_elements {
        return Err(QuantitativePlanError::InvalidSegmentation);
    }
    let (sop, bits, content) = match input.kind {
        SegmentationKind::Binary => {
            if input.stored_values.iter().any(|value| *value > 1) {
                return Err(QuantitativePlanError::InvalidSegmentation);
            }
            let bytes = pack_bits(&input.stored_values);
            (
                SEGMENTATION_SOP,
                1_u16,
                canonical_pixels(bytes, DicomVr::OB),
            )
        }
        SegmentationKind::FractionalProbability => (
            SEGMENTATION_SOP,
            8,
            neutral_u8_pixels(&input.stored_values, input, limits)?,
        ),
        SegmentationKind::Labelmap => (
            LABELMAP_SOP,
            8,
            neutral_u8_pixels(&input.stored_values, input, limits)?,
        ),
    };
    if !matches!(
        input.transfer_syntax_uid.as_str(),
        EXPLICIT_VR_LE | DEFLATED_IMAGE_FRAME
    ) || (input.transfer_syntax_uid == DEFLATED_IMAGE_FRAME
        && input.kind != SegmentationKind::Binary)
    {
        return Err(QuantitativePlanError::InvalidEncoding);
    }
    let source = &sources[0].artifact;
    let reference = source_reference(context, &sources[0])?;
    let mut attributes = common_attributes(context, sop, "SEG", "63")?;
    attributes.extend([
        set_u16(tags::ROWS, input.rows),
        set_u16(tags::COLUMNS, input.columns),
        set_string(
            tags::NUMBER_OF_FRAMES,
            DicomVr::IS,
            &input.frames.to_string(),
        ),
        set_u16(tags::SAMPLES_PER_PIXEL, 1),
        set_string(tags::PHOTOMETRIC_INTERPRETATION, DicomVr::CS, "MONOCHROME2"),
        set_u16(tags::BITS_ALLOCATED, bits),
        set_u16(tags::BITS_STORED, bits),
        set_u16(tags::HIGH_BIT, bits - 1),
        set_u16(tags::PIXEL_REPRESENTATION, 0),
        set_string(
            Tag(0x0062, 0x0001),
            DicomVr::CS,
            match input.kind {
                SegmentationKind::Binary => "BINARY",
                SegmentationKind::FractionalProbability => "FRACTIONAL",
                SegmentationKind::Labelmap => "LABELMAP",
            },
        ),
        segment_sequence(input),
        referenced_series(source, &sources[0]),
    ]);
    if input.kind == SegmentationKind::FractionalProbability {
        attributes.extend([
            set_string(Tag(0x0062, 0x0010), DicomVr::CS, "PROBABILITY"),
            set_u16(Tag(0x0062, 0x000E), 255),
        ]);
    }
    finish_native(
        recipe,
        case_id,
        context,
        match input.kind {
            SegmentationKind::Binary => "derived/segmentation/binary",
            SegmentationKind::FractionalProbability => {
                "derived/segmentation/fractional-probability"
            }
            SegmentationKind::Labelmap => "derived/segmentation/labelmap",
        },
        sop,
        &input.transfer_syntax_uid,
        attributes,
        vec![content],
        vec![reference],
        sources,
        limits,
    )
}

fn plan_rwvm(
    recipe: &RecipeIdentity,
    case_id: &str,
    context: &QuantitativeArtifactContext,
    input: &RealWorldValueMappingInput,
    sources: &[QuantitativeSourceInput],
    limits: QuantitativeProviderLimits,
) -> Result<QuantitativePlanOutput, QuantitativePlanError> {
    validate_context(context)?;
    validate_sources(
        sources,
        QuantitativeSourceRole::RealWorldValueSourceImage,
        limits,
    )?;
    let [source] = sources else {
        return Err(QuantitativePlanError::InvalidSources);
    };
    if !input.intercept.is_finite()
        || !input.slope.is_finite()
        || input.first_value_mapped > input.last_value_mapped
    {
        return Err(QuantitativePlanError::InvalidMapping);
    }
    let reference = source_reference(context, source)?;
    let mut attributes = common_attributes(context, RWVM_SOP, "RWV", "62")?;
    attributes.extend([
        set_string(Tag(0x0070, 0x0080), DicomVr::CS, &input.content_label),
        set_string(Tag(0x0070, 0x0081), DicomVr::LO, &input.content_description),
        rwvm_sequence(input, source),
        referenced_series(&source.artifact, source),
    ]);
    finish_native(
        recipe,
        case_id,
        context,
        "derived/real-world-value-mapping/linear",
        RWVM_SOP,
        EXPLICIT_VR_LE,
        attributes,
        Vec::new(),
        vec![reference],
        sources,
        limits,
    )
}

fn plan_external(
    recipe: &RecipeIdentity,
    case_id: &str,
    context: &QuantitativeArtifactContext,
    import: &ExternalImportBoundary,
    sources: &[QuantitativeSourceInput],
    limits: QuantitativeProviderLimits,
) -> Result<QuantitativePlanOutput, QuantitativePlanError> {
    validate_context(context)?;
    let role = match import.kind {
        ExternalImportKind::ParametricMapFloat32 | ExternalImportKind::ParametricMapFloat64 => {
            QuantitativeSourceRole::ParametricMapSourceImage
        }
        ExternalImportKind::WholeSlideTileSegmentation => {
            QuantitativeSourceRole::WholeSlideSourceImage
        }
    };
    validate_sources(sources, role, limits)?;
    let expected_sources = if role == QuantitativeSourceRole::ParametricMapSourceImage {
        3
    } else {
        1
    };
    if sources.len() != expected_sources
        || import.dependency.executable_provider_id != "highdicom_pydicom"
        || import.dependency.required_tool_version.is_empty()
        || import.dependency.protocol_version.is_empty()
        || !is_sha256(&import.dependency.dependency_lock_sha256)
        || import.output_media_type != "application/dicom"
        || import.maximum_output_bytes == 0
        || import.maximum_output_bytes > limits.max_output_bytes
        || import.timeout_seconds == 0
        || import.timeout_seconds > limits.max_external_seconds
        || import.semantic_evidence.frame_count == 0
        || import.semantic_evidence.frame_count > limits.max_frames
        || import
            .semantic_evidence
            .required_validation_names
            .is_empty()
    {
        return Err(QuantitativePlanError::InvalidExternalBoundary);
    }
    let references = sources
        .iter()
        .map(|source| source_reference(context, source))
        .collect::<Result<Vec<_>, _>>()?;
    let dependencies = source_dependencies(context, sources);
    Ok(QuantitativePlanOutput::ExternalImport {
        recipe: recipe.clone(),
        case_id: case_id.into(),
        artifact: context.clone(),
        import: import.clone(),
        sources: sources.to_vec(),
        dependencies,
        references,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_native(
    recipe: &RecipeIdentity,
    case_id: &str,
    context: &QuantitativeArtifactContext,
    template_id: &str,
    sop_class_uid: &str,
    transfer_syntax_uid: &str,
    attributes: Vec<AttributeOperation>,
    content: Vec<CanonicalContent>,
    references: Vec<MaterializedReference>,
    sources: &[QuantitativeSourceInput],
    limits: QuantitativeProviderLimits,
) -> Result<QuantitativePlanOutput, QuantitativePlanError> {
    let content_bytes = content.iter().try_fold(0_u64, |sum, item| {
        sum.checked_add(item.size_bytes)
            .ok_or(QuantitativePlanError::ResourceOverflow)
    })?;
    let output_bytes = content_bytes
        .checked_add(16 * 1024)
        .ok_or(QuantitativePlanError::ResourceOverflow)?;
    if output_bytes > limits.max_output_bytes {
        return Err(QuantitativePlanError::ResourceLimitExceeded);
    }
    let implementation = context
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or(QuantitativePlanError::MissingIdentity(
            "implementation_class_uid",
        ))?;
    let slots = content
        .iter()
        .map(|item| {
            let Some(ContentMaterialization::Inline(bytes)) = &item.materialization else {
                return Err(QuantitativePlanError::MissingContent);
            };
            Ok((
                item.slot.clone(),
                SlotExecutionBinding::NativeFrames {
                    frames: vec![NativeFrameBinding {
                        frame_number: 1,
                        bytes: ByteBinding::Inline {
                            sha256: item.sha256.clone(),
                            bytes: bytes.clone(),
                        },
                        rows: 1,
                        columns: u32::try_from(bytes.len())
                            .map_err(|_| QuantitativePlanError::ResourceOverflow)?,
                        samples_per_pixel: 1,
                        bits_allocated: 8,
                        photometric_interpretation: "MONOCHROME2".into(),
                    }],
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let planned = PlannedDicomArtifact {
        logical_id: context.target_instance_id.clone(),
        order: context.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: case_id.into(),
            recipe_id: recipe.recipe_id.clone(),
            recipe_version: recipe.recipe_version.clone(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: context.target_instance_id.clone(),
            template_id: TemplateId(template_id.into()),
            template_version: TemplateVersion::from_str("1.0.0").expect("valid version"),
            sop_class_uid: sop_class_uid.into(),
            transfer_syntax_uid: transfer_syntax_uid.into(),
            identities: context.identities.clone(),
            attributes: resolved(attributes),
            content,
            references,
        },
        output: context.output.clone(),
        encoding: encoding(transfer_syntax_uid, implementation),
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
            output_bytes,
            peak_working_bytes: output_bytes
                .checked_mul(2)
                .ok_or(QuantitativePlanError::ResourceOverflow)?,
        },
    };
    Ok(QuantitativePlanOutput::Native {
        bindings: ArtifactExecutionBindings {
            artifact_id: planned.logical_id.clone(),
            slots,
        },
        dependencies: source_dependencies(context, sources),
        artifact: planned,
    })
}

fn validate_limits(limits: QuantitativeProviderLimits) -> Result<(), QuantitativePlanError> {
    if limits.max_sources == 0
        || limits.max_frames == 0
        || limits.max_elements == 0
        || limits.max_output_bytes == 0
        || limits.max_external_seconds == 0
    {
        return Err(QuantitativePlanError::InvalidLimits);
    }
    Ok(())
}

fn validate_context(context: &QuantitativeArtifactContext) -> Result<(), QuantitativePlanError> {
    if context.recipe_artifact_logical_id.is_empty()
        || context.target_instance_id.is_empty()
        || context.identities.logical_instance_id != context.target_instance_id
        || !context.output.publish
        || context
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .is_none()
        || context
            .identities
            .get(&CompositionUidRole::StudyInstance, 0)
            .is_none()
        || context
            .identities
            .get(&CompositionUidRole::SeriesInstance, 0)
            .is_none()
        || context
            .identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .is_none()
    {
        return Err(QuantitativePlanError::InvalidContext);
    }
    Ok(())
}

fn validate_sources(
    sources: &[QuantitativeSourceInput],
    role: QuantitativeSourceRole,
    limits: QuantitativeProviderLimits,
) -> Result<(), QuantitativePlanError> {
    if sources.is_empty() || sources.len() > limits.max_sources as usize {
        return Err(QuantitativePlanError::InvalidSources);
    }
    let mut ids = BTreeSet::new();
    for source in sources {
        if source.role != role
            || source.bindings.artifact_id != source.artifact.logical_id
            || !ids.insert(source.artifact.logical_id.as_str())
            || source.referenced_frames.len() > limits.max_frames as usize
            || source.referenced_frames.iter().any(|frame| *frame == 0)
        {
            return Err(QuantitativePlanError::InvalidSources);
        }
    }
    Ok(())
}

fn neutral_u8_pixels(
    values: &[u8],
    input: &SegmentationInput,
    limits: QuantitativeProviderLimits,
) -> Result<CanonicalContent, QuantitativePlanError> {
    let output = NeutralContentProvider.expand(
        &ContentProviderRequest::IntegerPixels(IntegerPixelsContract {
            target: ContentTarget {
                slot: "pixels".into(),
                content_kind: "native_pixels".into(),
                address: AttributeAddress::standard(tags::PIXEL_DATA).expect("standard tag"),
                vr: DicomVr::OB,
            },
            dimensions: vec![
                u32::from(input.frames),
                u32::from(input.rows),
                u32::from(input.columns),
            ],
            bits_allocated: 8,
            byte_order: ContentByteOrder::LittleEndian,
            samples: IntegerSamples::Unsigned {
                values: values.iter().map(|value| u64::from(*value)).collect(),
            },
        }),
        ContentProviderLimits {
            max_elements: limits.max_elements,
            max_output_bytes: limits.max_output_bytes,
            ..ContentProviderLimits::default()
        },
    )?;
    output
        .contents
        .into_iter()
        .next()
        .ok_or(QuantitativePlanError::MissingContent)
}

fn canonical_pixels(bytes: Vec<u8>, vr: DicomVr) -> CanonicalContent {
    CanonicalContent {
        slot: "pixels".into(),
        kind: "native_pixels".into(),
        address: AttributeAddress::standard(tags::PIXEL_DATA).expect("standard tag"),
        vr,
        size_bytes: bytes.len() as u64,
        sha256: crate::sha256_hex(&bytes),
        properties: BTreeMap::new(),
        placement: Default::default(),
        materialization: Some(ContentMaterialization::Inline(bytes)),
    }
}

fn pack_bits(values: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; values.len().div_ceil(8)];
    for (index, value) in values.iter().enumerate() {
        bytes[index / 8] |= value << (index % 8);
    }
    if bytes.len() % 2 != 0 {
        bytes.push(0);
    }
    bytes
}

fn source_reference(
    context: &QuantitativeArtifactContext,
    source: &QuantitativeSourceInput,
) -> Result<MaterializedReference, QuantitativePlanError> {
    Ok(MaterializedReference {
        source_instance_id: context.target_instance_id.clone(),
        target_instance_id: source.artifact.instance.instance_id.clone(),
        role: source.role.relationship().into(),
        frame_role: None,
        referenced_sop_class_uid: source.artifact.instance.sop_class_uid.clone(),
        referenced_sop_instance_uid: identity(&source.artifact, CompositionUidRole::SopInstance)?,
        referenced_frames: source.referenced_frames.clone(),
    })
}

fn source_dependencies(
    context: &QuantitativeArtifactContext,
    sources: &[QuantitativeSourceInput],
) -> Vec<ArtifactDependency> {
    sources
        .iter()
        .map(|source| ArtifactDependency {
            artifact_id: context.target_instance_id.clone(),
            depends_on: source.artifact.logical_id.clone(),
            relationship: source.role.relationship().into(),
            frame_numbers: source.referenced_frames.clone(),
        })
        .collect()
}

fn identity(
    artifact: &PlannedDicomArtifact,
    role: CompositionUidRole,
) -> Result<String, QuantitativePlanError> {
    artifact
        .instance
        .identities
        .get(&role, 0)
        .map(str::to_owned)
        .ok_or(QuantitativePlanError::MissingIdentity("source identity"))
}

fn common_attributes(
    context: &QuantitativeArtifactContext,
    sop: &str,
    modality: &str,
    series_number: &str,
) -> Result<Vec<AttributeOperation>, QuantitativePlanError> {
    Ok(vec![
        set_string(tags::SOP_CLASS_UID, DicomVr::UI, sop),
        set_string(
            tags::SOP_INSTANCE_UID,
            DicomVr::UI,
            context
                .identities
                .get(&CompositionUidRole::SopInstance, 0)
                .ok_or(QuantitativePlanError::MissingIdentity("sop_instance_uid"))?,
        ),
        set_string(tags::SYNTHETIC_DATA, DicomVr::CS, "YES"),
        set_string(tags::PATIENT_NAME, DicomVr::PN, "DTS^Synthetic^Patient001"),
        set_string(tags::PATIENT_ID, DicomVr::LO, "DTS-PATIENT-001"),
        set_string(
            tags::STUDY_INSTANCE_UID,
            DicomVr::UI,
            context
                .identities
                .get(&CompositionUidRole::StudyInstance, 0)
                .ok_or(QuantitativePlanError::MissingIdentity("study_instance_uid"))?,
        ),
        set_string(tags::STUDY_DATE, DicomVr::DA, "20260101"),
        set_string(tags::STUDY_TIME, DicomVr::TM, "000000"),
        set_string(tags::MODALITY, DicomVr::CS, modality),
        set_string(
            tags::SERIES_INSTANCE_UID,
            DicomVr::UI,
            context
                .identities
                .get(&CompositionUidRole::SeriesInstance, 0)
                .ok_or(QuantitativePlanError::MissingIdentity(
                    "series_instance_uid",
                ))?,
        ),
        set_string(tags::SERIES_NUMBER, DicomVr::IS, series_number),
        set_string(tags::INSTANCE_NUMBER, DicomVr::IS, "1"),
        set_string(tags::MANUFACTURER, DicomVr::LO, "dicom-test-suite"),
        set_string(tags::SOFTWARE_VERSIONS, DicomVr::LO, crate::PACKAGE_VERSION),
    ])
}

fn segment_sequence(input: &SegmentationInput) -> AttributeOperation {
    sequence(
        Tag(0x0062, 0x0002),
        vec![item(vec![
            set_u16(Tag(0x0062, 0x0004), input.segment_number),
            set_string(Tag(0x0062, 0x0005), DicomVr::LO, &input.segment_label),
            set_string(Tag(0x0062, 0x0008), DicomVr::CS, "MANUAL"),
        ])],
    )
}

fn referenced_series(
    source: &PlannedDicomArtifact,
    declaration: &QuantitativeSourceInput,
) -> AttributeOperation {
    sequence(
        tags::REFERENCED_SERIES_SEQUENCE,
        vec![item(vec![
            set_string(
                tags::SERIES_INSTANCE_UID,
                DicomVr::UI,
                source
                    .instance
                    .identities
                    .get(&CompositionUidRole::SeriesInstance, 0)
                    .unwrap(),
            ),
            sequence(
                tags::REFERENCED_INSTANCE_SEQUENCE,
                vec![item(vec![
                    set_string(
                        tags::REFERENCED_SOP_CLASS_UID,
                        DicomVr::UI,
                        &source.instance.sop_class_uid,
                    ),
                    set_string(
                        tags::REFERENCED_SOP_INSTANCE_UID,
                        DicomVr::UI,
                        source
                            .instance
                            .identities
                            .get(&CompositionUidRole::SopInstance, 0)
                            .unwrap(),
                    ),
                    set_unsigned_multi(
                        tags::REFERENCED_FRAME_NUMBER,
                        DicomVr::IS,
                        &declaration.referenced_frames,
                    ),
                ])],
            ),
        ])],
    )
}

fn rwvm_sequence(
    value: &RealWorldValueMappingInput,
    source: &QuantitativeSourceInput,
) -> AttributeOperation {
    sequence(
        Tag(0x0040, 0x9096),
        vec![item(vec![
            set_u16(Tag(0x0040, 0x9216), value.first_value_mapped),
            set_u16(Tag(0x0040, 0x9211), value.last_value_mapped),
            set_f64(Tag(0x0040, 0x9224), value.intercept),
            set_f64(Tag(0x0040, 0x9225), value.slope),
            set_string(Tag(0x0040, 0x9210), DicomVr::SH, &value.lut_label),
            sequence(
                Tag(0x0040, 0x08EA),
                vec![item(vec![
                    set_string(tags::CODE_VALUE, DicomVr::SH, &value.unit_code_value),
                    set_string(
                        tags::CODING_SCHEME_DESIGNATOR,
                        DicomVr::SH,
                        &value.unit_coding_scheme_designator,
                    ),
                    set_string(tags::CODE_MEANING, DicomVr::LO, &value.unit_code_meaning),
                ])],
            ),
            set_unsigned_multi(
                tags::REFERENCED_FRAME_NUMBER,
                DicomVr::IS,
                &source.referenced_frames,
            ),
        ])],
    )
}

fn set_string(tag: Tag, vr: DicomVr, value: &str) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    )
}

fn set_u16(tag: Tag, value: u16) -> AttributeOperation {
    set_value(
        tag,
        DicomVr::US,
        AttributeValue::Primitive(PrimitiveValue::Unsigned(u64::from(value))),
    )
}

fn set_f64(tag: Tag, value: f64) -> AttributeOperation {
    set_value(
        tag,
        DicomVr::FD,
        AttributeValue::Primitive(PrimitiveValue::Float64Bits(value.to_bits())),
    )
}

fn set_unsigned_multi(tag: Tag, vr: DicomVr, values: &[u32]) -> AttributeOperation {
    let values = values
        .iter()
        .map(|value| {
            if vr == DicomVr::IS {
                PrimitiveValue::String(value.to_string())
            } else {
                PrimitiveValue::Unsigned(u64::from(*value))
            }
        })
        .collect();
    set_value(tag, vr, AttributeValue::Multi(values))
}

fn set_value(tag: Tag, vr: DicomVr, value: AttributeValue) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::standard(tag).expect("standard tag"),
        vr,
        value,
    }
}

fn sequence(tag: Tag, items: Vec<AttributeItem>) -> AttributeOperation {
    set_value(tag, DicomVr::SQ, AttributeValue::Sequence(items))
}

fn item(operations: Vec<AttributeOperation>) -> AttributeItem {
    AttributeItem {
        attributes: operations,
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
                address,
                vr: DicomVr::UN,
                value: None,
                origin: ValueOrigin::InstanceOverride,
            },
            AttributeOperation::Remove { .. } => unreachable!(),
        })
        .collect()
}

fn encoding(transfer_syntax_uid: &str, implementation: &str) -> EncodingPlan {
    EncodingPlan {
        transfer_syntax_uid: transfer_syntax_uid.into(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: if transfer_syntax_uid == DEFLATED_IMAGE_FRAME {
            FragmentationPolicy::OneFragmentPerFrame
        } else {
            FragmentationPolicy::Native
        },
        offset_table: if transfer_syntax_uid == DEFLATED_IMAGE_FRAME {
            OffsetTablePolicy::EmptyBasic
        } else {
            OffsetTablePolicy::NotApplicable
        },
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: ImplementationIdentityPlan {
            class_uid: implementation.into(),
            version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
        },
        backend_id: if transfer_syntax_uid == DEFLATED_IMAGE_FRAME {
            "encoding.deflated_image_frame".into()
        } else {
            "dicom-rs.part10".into()
        },
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug)]
pub enum QuantitativePlanError {
    InvalidLimits,
    InvalidContext,
    InvalidSources,
    InvalidSegmentation,
    InvalidMapping,
    InvalidEncoding,
    InvalidExternalBoundary,
    MissingIdentity(&'static str),
    MissingContent,
    ResourceOverflow,
    ResourceLimitExceeded,
    Content(super::ContentProviderError),
    Document(String),
}

impl fmt::Display for QuantitativePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for QuantitativePlanError {}

impl From<super::ContentProviderError> for QuantitativePlanError {
    fn from(value: super::ContentProviderError) -> Self {
        Self::Content(value)
    }
}
