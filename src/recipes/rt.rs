//! Typed, plan-first radiotherapy object recipes.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};

use super::semantic::{
    SemanticPlanContext, SemanticPlanError, SemanticPlanOutput, SemanticSource, build_semantic_plan,
};
use super::{
    CaseRecipe, ContentByteOrder, ContentProviderLimits, ContentProviderRequest, ContentTarget,
    IntegerPixelsContract, IntegerSamples, NeutralContentProvider, RecipeReference, RtObjectKind,
    RtSemanticContract, SemanticReference, SemanticReferenceRole,
};

pub const RT_PLAN_PROVIDER_ID: &str = "native.rt_plan";
pub const RT_CONTENT_PROVIDER_ID: &str = "content.rt_semantics";
pub const RT_ALGORITHM_PROVIDER_ID: &str = "algorithm.rt_semantics";

const RT_STRUCTURE_SET_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.3";
const RT_DOSE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.2";
const RT_PLAN_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.5";
const RT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.1";
const CARM_RADIATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.13";
const RADIATION_SET_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.12";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RtSourceDeclaration {
    pub recipe: RecipeReference,
    pub artifact_logical_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureSetParameters {
    pub structure_set_name: String,
    pub roi_number: u32,
    pub roi_name: String,
    pub generation_algorithm: String,
    pub generation_description: String,
    pub display_color: [u16; 3],
    pub contour_number: u32,
    pub contour_geometric_type: String,
    pub contour_points: u32,
    pub contour_data: Vec<String>,
    pub interpreted_type: String,
    pub interpreter: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoseParameters {
    pub rows: u32,
    pub columns: u32,
    pub frames: u32,
    pub stored_values: Vec<u64>,
    pub pixel_spacing: [String; 2],
    pub image_orientation_patient: [String; 6],
    pub image_position_patient: [String; 3],
    pub slice_thickness: String,
    pub grid_frame_offset_vector: Vec<String>,
    pub dose_units: String,
    pub dose_type: String,
    pub dose_summation_type: String,
    pub dose_grid_scaling: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanParameters {
    pub plan_name: String,
    pub plan_geometry: String,
    pub fraction_group_number: u32,
    pub fractions_planned: u32,
    pub beam_number: u32,
    pub beam_name: String,
    pub beam_type: String,
    pub radiation_type: String,
    pub treatment_machine_name: String,
    pub control_point_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageParameters {
    pub rows: u32,
    pub columns: u32,
    pub stored_values: Vec<u64>,
    pub referenced_beam_number: u32,
    pub referenced_fraction_group_number: u32,
    pub image_plane: String,
    pub image_position: [String; 2],
    pub image_orientation: [String; 6],
    pub image_sid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadiationParameters {
    pub radiation_name: String,
    pub radiation_type: String,
    pub radiation_mode: String,
    pub treatment_delivery_type: String,
    pub machine_name: String,
    pub control_point_count: u32,
    pub rt_record_flag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadiationSetParameters {
    pub radiation_set_label: String,
    pub radiation_set_name: String,
    pub treatment_position_group_uid_role: String,
    pub treatment_session_uid_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RtObjectParameters {
    StructureSet(StructureSetParameters),
    Dose(DoseParameters),
    Plan(PlanParameters),
    Image(ImageParameters),
    CarmRadiation(RadiationParameters),
    RadiationSet(RadiationSetParameters),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RtDocumentParameters {
    pub series_number: String,
    pub instance_number: u32,
    pub label: String,
    pub object: RtObjectParameters,
    pub sources: Vec<RtSourceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RtPlanInput {
    pub parameters: RtDocumentParameters,
    pub context: SemanticPlanContext,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RtPlanProvider;

impl RtPlanProvider {
    pub fn plan(&self, input: &RtPlanInput) -> Result<SemanticPlanOutput, RtPlanError> {
        validate_input(input)?;
        let (sop_class, neutral_kind, validation_rule) = match input.parameters.object {
            RtObjectParameters::StructureSet(_) => (
                RT_STRUCTURE_SET_STORAGE,
                Some(RtObjectKind::StructureSet),
                "rt.structure_set",
            ),
            RtObjectParameters::Dose(_) => (RT_DOSE_STORAGE, Some(RtObjectKind::Dose), "rt.dose"),
            RtObjectParameters::Plan(_) => (RT_PLAN_STORAGE, Some(RtObjectKind::Plan), "rt.plan"),
            RtObjectParameters::Image(_) => {
                (RT_IMAGE_STORAGE, Some(RtObjectKind::Image), "rt.image")
            }
            RtObjectParameters::CarmRadiation(_) => {
                (CARM_RADIATION_STORAGE, None, "rt.carm_radiation")
            }
            RtObjectParameters::RadiationSet(_) => {
                (RADIATION_SET_STORAGE, None, "rt.radiation_set")
            }
        };
        let mut operations = if let Some(object_kind) = neutral_kind {
            NeutralContentProvider
                .expand(
                    &ContentProviderRequest::RtObject(RtSemanticContract {
                        object_kind,
                        label: input.parameters.label.clone(),
                        instance_number: input.parameters.instance_number,
                        references: neutral_references(&input.context.sources)?,
                    }),
                    ContentProviderLimits::default(),
                )
                .map_err(|error| RtPlanError::Content(error.to_string()))?
                .attribute_operations
        } else {
            vec![
                string_op(
                    "0008,0060",
                    DicomVr::CS,
                    match input.parameters.object {
                        RtObjectParameters::CarmRadiation(_) => "RTRAD",
                        RtObjectParameters::RadiationSet(_) => "RTSET",
                        _ => unreachable!(),
                    },
                )?,
                string_op(
                    "0020,0013",
                    DicomVr::IS,
                    &input.parameters.instance_number.to_string(),
                )?,
            ]
        };
        operations.push(string_op(
            "0020,0011",
            DicomVr::IS,
            &input.parameters.series_number,
        )?);
        operations.extend(object_operations(
            &input.parameters.object,
            &input.context.sources,
        )?);
        let contents = object_content(&input.parameters.object)?;
        build_semantic_plan(
            &input.context,
            sop_class,
            operations,
            contents,
            &[validation_rule, "rt.reference_graph"],
            "builtin.strict_rt",
        )
        .map_err(RtPlanError::Plan)
    }
}

pub fn rt_input_from_recipe(
    recipe: &CaseRecipe,
    context: SemanticPlanContext,
) -> Result<Option<RtPlanInput>, RtPlanError> {
    if recipe.plan_provider_id != RT_PLAN_PROVIDER_ID {
        return Ok(None);
    }
    let parameters = serde_json::from_value::<RtDocumentParameters>(Value::Object(
        recipe.provider_parameters.clone(),
    ))
    .map_err(|error| RtPlanError::Recipe(error.to_string()))?;
    let dicom = recipe.dicom.as_ref().ok_or(RtPlanError::RecipeShape)?;
    let [artifact] = dicom.artifacts.as_slice() else {
        return Err(RtPlanError::RecipeShape);
    };
    if artifact.logical_id != context.logical_id
        || artifact.order != 0
        || artifact
            .output
            .path
            .as_deref()
            .is_none_or(|path| path != context.output.relative_path.to_string())
        || artifact
            .template
            .as_ref()
            .is_none_or(|template| template.template_id != context.template_id)
        || artifact.content.provider_id != RT_CONTENT_PROVIDER_ID
        || artifact.algorithm_provider_id.as_deref() != Some(RT_ALGORITHM_PROVIDER_ID)
    {
        return Err(RtPlanError::RecipeShape);
    }
    Ok(Some(RtPlanInput {
        parameters,
        context,
    }))
}

fn validate_input(input: &RtPlanInput) -> Result<(), RtPlanError> {
    if input.parameters.series_number.is_empty()
        || input.parameters.instance_number == 0
        || input.parameters.label.is_empty()
        || input.parameters.label.len() > 64
    {
        return Err(RtPlanError::InvalidParameters);
    }
    let declarations = input
        .parameters
        .sources
        .iter()
        .map(|source| {
            (
                &source.recipe,
                source.artifact_logical_id.as_str(),
                source.role.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual_owned = input
        .context
        .sources
        .iter()
        .map(|source| {
            (
                RecipeReference {
                    recipe_id: source.recipe.recipe_id.clone(),
                    recipe_version: source.recipe.recipe_version.clone(),
                },
                source.artifact_id.as_str(),
                source.role.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual = actual_owned
        .iter()
        .map(|(recipe, artifact, role)| (recipe, *artifact, *role))
        .collect::<BTreeSet<_>>();
    if declarations != actual || !valid_role_set(&input.parameters.object, &input.context.sources) {
        return Err(RtPlanError::SourceGraph);
    }
    Ok(())
}

fn valid_role_set(object: &RtObjectParameters, sources: &[SemanticSource]) -> bool {
    let actual = sources
        .iter()
        .map(|source| source.role.as_str())
        .collect::<BTreeSet<_>>();
    let expected = match object {
        RtObjectParameters::StructureSet(_) => ["source_image"].as_slice(),
        RtObjectParameters::Dose(_) => ["source_image", "referenced_structure_set"].as_slice(),
        RtObjectParameters::Plan(_) => ["referenced_structure_set", "referenced_dose"].as_slice(),
        RtObjectParameters::Image(_) | RtObjectParameters::CarmRadiation(_) => {
            ["referenced_plan"].as_slice()
        }
        RtObjectParameters::RadiationSet(_) => {
            ["referenced_plan", "referenced_radiation"].as_slice()
        }
    };
    actual == expected.iter().copied().collect()
}

fn neutral_references(sources: &[SemanticSource]) -> Result<Vec<SemanticReference>, RtPlanError> {
    sources
        .iter()
        .map(|source| {
            let role = match source.role.as_str() {
                "source_image" => SemanticReferenceRole::SourceImage,
                "referenced_structure_set" => SemanticReferenceRole::ReferencedStructureSet,
                "referenced_dose" => SemanticReferenceRole::ReferencedDose,
                "referenced_plan" => SemanticReferenceRole::ReferencedPlan,
                _ => return Err(RtPlanError::SourceGraph),
            };
            Ok(SemanticReference {
                role,
                sop_class_uid: source.reference.referenced_sop_class_uid.clone(),
                sop_instance_uid: source.reference.referenced_sop_instance_uid.clone(),
                frames: source.reference.referenced_frames.clone(),
            })
        })
        .collect()
}

fn object_content(
    object: &RtObjectParameters,
) -> Result<Vec<crate::composition::CanonicalContent>, RtPlanError> {
    let (values, dimensions, bits, vr) = match object {
        RtObjectParameters::Dose(value) => (
            &value.stored_values,
            vec![value.frames, value.rows, value.columns],
            16,
            DicomVr::OW,
        ),
        RtObjectParameters::Image(value) => (
            &value.stored_values,
            vec![value.rows, value.columns],
            8,
            DicomVr::OB,
        ),
        _ => return Ok(vec![]),
    };
    NeutralContentProvider
        .expand(
            &ContentProviderRequest::IntegerPixels(IntegerPixelsContract {
                target: ContentTarget {
                    slot: "pixels".into(),
                    content_kind: "native_pixels".into(),
                    address: address("7FE0,0010")?,
                    vr,
                },
                dimensions,
                bits_allocated: bits,
                byte_order: ContentByteOrder::LittleEndian,
                samples: IntegerSamples::Unsigned {
                    values: values.clone(),
                },
            }),
            ContentProviderLimits::default(),
        )
        .map(|output| output.contents)
        .map_err(|error| RtPlanError::Content(error.to_string()))
}

fn object_operations(
    object: &RtObjectParameters,
    sources: &[SemanticSource],
) -> Result<Vec<AttributeOperation>, RtPlanError> {
    match object {
        RtObjectParameters::StructureSet(value) => structure_set_operations(value, sources),
        RtObjectParameters::Dose(value) => dose_operations(value),
        RtObjectParameters::Plan(value) => plan_operations(value),
        RtObjectParameters::Image(value) => image_operations(value),
        RtObjectParameters::CarmRadiation(value) => radiation_operations(value, sources),
        RtObjectParameters::RadiationSet(value) => radiation_set_operations(value, sources),
    }
}

fn structure_set_operations(
    value: &StructureSetParameters,
    _sources: &[SemanticSource],
) -> Result<Vec<AttributeOperation>, RtPlanError> {
    if value.roi_number == 0
        || value.contour_number == 0
        || value.contour_points < 3
        || value.contour_data.len() != value.contour_points as usize * 3
        || value.display_color.iter().any(|component| *component > 255)
    {
        return Err(RtPlanError::InvalidParameters);
    }
    Ok(vec![
        string_op("3006,0004", DicomVr::LO, &value.structure_set_name)?,
        sequence_op(
            "3006,0020",
            vec![
                string_op("3006,0022", DicomVr::IS, &value.roi_number.to_string())?,
                string_op("3006,0026", DicomVr::LO, &value.roi_name)?,
                string_op("3006,0036", DicomVr::CS, &value.generation_algorithm)?,
                string_op("3006,0038", DicomVr::LO, &value.generation_description)?,
            ],
        )?,
        sequence_op(
            "3006,0039",
            vec![
                string_op("3006,0084", DicomVr::IS, &value.roi_number.to_string())?,
                multi_string_op(
                    "3006,002A",
                    DicomVr::IS,
                    value.display_color.iter().map(u16::to_string),
                )?,
                sequence_op(
                    "3006,0040",
                    vec![
                        string_op("3006,0048", DicomVr::IS, &value.contour_number.to_string())?,
                        string_op("3006,0042", DicomVr::CS, &value.contour_geometric_type)?,
                        string_op("3006,0046", DicomVr::IS, &value.contour_points.to_string())?,
                        multi_string_op(
                            "3006,0050",
                            DicomVr::DS,
                            value.contour_data.iter().cloned(),
                        )?,
                    ],
                )?,
            ],
        )?,
        sequence_op(
            "3006,0080",
            vec![
                string_op("3006,0082", DicomVr::IS, &value.roi_number.to_string())?,
                string_op("3006,0084", DicomVr::IS, &value.roi_number.to_string())?,
                string_op("3006,00A4", DicomVr::CS, &value.interpreted_type)?,
                string_allow_empty_op("3006,00A6", DicomVr::PN, &value.interpreter)?,
            ],
        )?,
    ])
}

fn dose_operations(value: &DoseParameters) -> Result<Vec<AttributeOperation>, RtPlanError> {
    if value.rows == 0
        || value.columns == 0
        || value.frames == 0
        || value.grid_frame_offset_vector.len() != value.frames as usize
    {
        return Err(RtPlanError::InvalidParameters);
    }
    Ok(vec![
        unsigned_op("0028,0010", DicomVr::US, value.rows.into())?,
        unsigned_op("0028,0011", DicomVr::US, value.columns.into())?,
        string_op("0028,0008", DicomVr::IS, &value.frames.to_string())?,
        multi_string_op(
            "0028,0030",
            DicomVr::DS,
            value.pixel_spacing.iter().cloned(),
        )?,
        multi_string_op(
            "0020,0037",
            DicomVr::DS,
            value.image_orientation_patient.iter().cloned(),
        )?,
        multi_string_op(
            "0020,0032",
            DicomVr::DS,
            value.image_position_patient.iter().cloned(),
        )?,
        string_op("0018,0050", DicomVr::DS, &value.slice_thickness)?,
        multi_string_op(
            "3004,000C",
            DicomVr::DS,
            value.grid_frame_offset_vector.iter().cloned(),
        )?,
        string_op("3004,0002", DicomVr::CS, &value.dose_units)?,
        string_op("3004,0004", DicomVr::CS, &value.dose_type)?,
        string_op("3004,000A", DicomVr::CS, &value.dose_summation_type)?,
        string_op("3004,000E", DicomVr::DS, &value.dose_grid_scaling)?,
        unsigned_op("0028,0100", DicomVr::US, 16)?,
        unsigned_op("0028,0101", DicomVr::US, 16)?,
        unsigned_op("0028,0102", DicomVr::US, 15)?,
        unsigned_op("0028,0103", DicomVr::US, 0)?,
    ])
}

fn plan_operations(value: &PlanParameters) -> Result<Vec<AttributeOperation>, RtPlanError> {
    if value.fraction_group_number == 0
        || value.fractions_planned == 0
        || value.beam_number == 0
        || value.control_point_count == 0
    {
        return Err(RtPlanError::InvalidParameters);
    }
    Ok(vec![
        string_op("300A,0003", DicomVr::LO, &value.plan_name)?,
        string_op("300A,000C", DicomVr::CS, &value.plan_geometry)?,
        sequence_op(
            "300A,0070",
            vec![
                string_op(
                    "300A,0071",
                    DicomVr::IS,
                    &value.fraction_group_number.to_string(),
                )?,
                string_op(
                    "300A,0078",
                    DicomVr::IS,
                    &value.fractions_planned.to_string(),
                )?,
            ],
        )?,
        sequence_op(
            "300A,00B0",
            vec![
                string_op("300A,00C0", DicomVr::IS, &value.beam_number.to_string())?,
                string_op("300A,00C2", DicomVr::LO, &value.beam_name)?,
                string_op("300A,00C4", DicomVr::CS, &value.beam_type)?,
                string_op("300A,00C6", DicomVr::CS, &value.radiation_type)?,
                string_op("300A,00B2", DicomVr::SH, &value.treatment_machine_name)?,
                string_op(
                    "300A,0110",
                    DicomVr::IS,
                    &value.control_point_count.to_string(),
                )?,
            ],
        )?,
    ])
}

fn image_operations(value: &ImageParameters) -> Result<Vec<AttributeOperation>, RtPlanError> {
    if value.rows == 0 || value.columns == 0 || value.referenced_beam_number == 0 {
        return Err(RtPlanError::InvalidParameters);
    }
    Ok(vec![
        unsigned_op("0028,0010", DicomVr::US, value.rows.into())?,
        unsigned_op("0028,0011", DicomVr::US, value.columns.into())?,
        unsigned_op("0028,0100", DicomVr::US, 8)?,
        unsigned_op("0028,0101", DicomVr::US, 8)?,
        unsigned_op("0028,0102", DicomVr::US, 7)?,
        unsigned_op("0028,0103", DicomVr::US, 0)?,
        string_op("3002,000C", DicomVr::CS, &value.image_plane)?,
        multi_string_op(
            "3002,0012",
            DicomVr::DS,
            value.image_position.iter().cloned(),
        )?,
        multi_string_op(
            "3002,0010",
            DicomVr::DS,
            value.image_orientation.iter().cloned(),
        )?,
        string_op("3002,0026", DicomVr::DS, &value.image_sid)?,
        string_op(
            "300C,0006",
            DicomVr::IS,
            &value.referenced_beam_number.to_string(),
        )?,
        string_op(
            "300C,0022",
            DicomVr::IS,
            &value.referenced_fraction_group_number.to_string(),
        )?,
    ])
}

fn radiation_operations(
    value: &RadiationParameters,
    sources: &[SemanticSource],
) -> Result<Vec<AttributeOperation>, RtPlanError> {
    if value.control_point_count == 0 || sources.len() != 1 {
        return Err(RtPlanError::InvalidParameters);
    }
    let source = &sources[0];
    Ok(vec![
        string_op("3010,0033", DicomVr::SH, &value.radiation_name)?,
        string_op("300A,00C6", DicomVr::CS, &value.radiation_type)?,
        string_op("0018,115A", DicomVr::CS, &value.radiation_mode)?,
        string_op("300A,00CE", DicomVr::CS, &value.treatment_delivery_type)?,
        string_op("300A,00B2", DicomVr::SH, &value.machine_name)?,
        unsigned_op("300A,0604", DicomVr::US, value.control_point_count.into())?,
        string_op("300A,0638", DicomVr::CS, "IDENT_ONLY")?,
        string_op("300A,0639", DicomVr::CS, &value.rt_record_flag)?,
        reference_sequence("300C,0002", source)?,
    ])
}

fn radiation_set_operations(
    value: &RadiationSetParameters,
    sources: &[SemanticSource],
) -> Result<Vec<AttributeOperation>, RtPlanError> {
    if sources.len() != 2
        || value.treatment_position_group_uid_role.is_empty()
        || value.treatment_session_uid_role.is_empty()
    {
        return Err(RtPlanError::InvalidParameters);
    }
    let mut operations = vec![
        string_op("3010,0033", DicomVr::SH, &value.radiation_set_label)?,
        string_op("3010,0034", DicomVr::LO, &value.radiation_set_name)?,
        unsigned_op("300A,0636", DicomVr::US, 1)?,
        string_op("300A,0637", DicomVr::CS, "TREATMENT")?,
    ];
    for source in sources {
        operations.push(reference_sequence(
            if source.role == "referenced_plan" {
                "300C,0002"
            } else {
                "300A,0630"
            },
            source,
        )?);
    }
    Ok(operations)
}

fn reference_sequence(
    tag: &str,
    source: &SemanticSource,
) -> Result<AttributeOperation, RtPlanError> {
    sequence_op(
        tag,
        vec![
            string_op(
                "0008,1150",
                DicomVr::UI,
                &source.reference.referenced_sop_class_uid,
            )?,
            string_op(
                "0008,1155",
                DicomVr::UI,
                &source.reference.referenced_sop_instance_uid,
            )?,
        ],
    )
}

fn sequence_op(
    tag: &str,
    attributes: Vec<AttributeOperation>,
) -> Result<AttributeOperation, RtPlanError> {
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(vec![AttributeItem { attributes }]),
    })
}

fn string_op(tag: &str, vr: DicomVr, value: &str) -> Result<AttributeOperation, RtPlanError> {
    if value.is_empty() || value.len() > 1024 * 1024 || value.chars().any(char::is_control) {
        return Err(RtPlanError::InvalidParameters);
    }
    string_allow_empty_op(tag, vr, value)
}

fn string_allow_empty_op(
    tag: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, RtPlanError> {
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    })
}

fn multi_string_op(
    tag: &str,
    vr: DicomVr,
    values: impl IntoIterator<Item = String>,
) -> Result<AttributeOperation, RtPlanError> {
    let values = values
        .into_iter()
        .map(PrimitiveValue::String)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(RtPlanError::InvalidParameters);
    }
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr,
        value: AttributeValue::Multi(values),
    })
}

fn unsigned_op(tag: &str, vr: DicomVr, value: u64) -> Result<AttributeOperation, RtPlanError> {
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    })
}

fn address(tag: &str) -> Result<AttributeAddress, RtPlanError> {
    AttributeAddress::from_normalized_tag(tag)
        .map_err(|error| RtPlanError::Attribute(error.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtPlanError {
    Recipe(String),
    RecipeShape,
    InvalidParameters,
    SourceGraph,
    Content(String),
    Attribute(String),
    Plan(SemanticPlanError),
}

impl fmt::Display for RtPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RtPlanError {}
