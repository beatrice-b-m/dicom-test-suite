//! Direct plan-only translation for typed Secondary Capture metadata recipes.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
    ResolvedAttribute, ResolvedInstancePlan, TemplateDescriptor, ValueOrigin,
};
use crate::sha256_hex;

use super::{
    CaseRecipe, MetadataScParameters, PlannedArtifactRecipe, PrivateElementValue,
    SecondaryCapturePlanInput, StringValueSource, sc::resolved_secondary_capture_base_plan,
};

/// A complete, name-independent caller declaration for the two legal DICOM
/// timezone extrema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimezoneBoundaryCapability {
    pub artifact_count: usize,
}

pub(crate) fn inspect_timezone_boundary_capability(
    recipe: &CaseRecipe,
) -> Result<Option<TimezoneBoundaryCapability>, String> {
    if recipe.plan_provider_id != "native.metadata_sc_plan" {
        return Ok(None);
    }
    let Some(dicom) = recipe.dicom.as_ref() else {
        return Ok(None);
    };
    if !dicom.artifacts.iter().any(|artifact| {
        matches!(
            artifact.metadata_sc,
            Some(MetadataScParameters::TimezoneBoundary(_))
        )
    }) {
        return Ok(None);
    }
    if dicom.artifacts.len() != 2
        || !recipe.provider_parameters.is_empty()
        || !recipe.dependencies.is_empty()
        || !recipe
            .validation_rule_ids
            .iter()
            .any(|rule| rule == "validation.sc.pixel")
        || !recipe
            .validation_rule_ids
            .iter()
            .any(|rule| rule == "validation.metadata.timezone")
        || !recipe
            .projection_rule_ids
            .iter()
            .any(|rule| rule == "projection.curated")
    {
        return Err("timezone capability requires one complete two-artifact pair contract".into());
    }
    let mut boundaries = BTreeSet::new();
    for artifact in &dicom.artifacts {
        let template = artifact.template.as_ref();
        let pixels = artifact.secondary_capture.as_ref();
        if !template.is_some_and(|value| {
            value.template_id == "classic/secondary-capture/monochrome"
                && value.template_version == "1.0.0"
        }) || artifact.output.path.is_none()
            || artifact.output.provider_derived == Some(true)
            || artifact.encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || artifact.encoding.sequence_length_policy != "default"
            || artifact.encoding.item_length_policy != "default"
            || artifact.encoding.offset_table_policy != "none"
            || artifact.encoding.fragmentation_policy != "native"
            || artifact.encoding.preamble_policy.as_deref() != Some("zero_filled")
            || artifact.encoding.file_meta_policy.as_deref() != Some("standard")
            || artifact
                .encoding
                .non_template_encoding_provider_id
                .is_some()
            || !artifact.parameters.is_empty()
            || !artifact.content.parameters.is_empty()
            || artifact.content.provider_id != "content.metadata.timezone_boundary"
            || artifact.algorithm_provider_id.is_some()
            || !artifact.attribute_operations.is_empty()
            || artifact.classic_projection.is_some()
            || artifact.nonsquare_geometry.is_some()
            || !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.sc.pixel")
            || !artifact
                .validation_rule_ids
                .iter()
                .any(|rule| rule == "validation.metadata.timezone")
            || !artifact
                .projection_rule_ids
                .iter()
                .any(|rule| rule == "projection.curated")
        {
            return Err(
                "timezone artifact must use the complete native monochrome SC contract".into(),
            );
        }
        let Some(pixels) = pixels else {
            return Err("timezone artifact requires native 2x2 U8 pixels".into());
        };
        if pixels.rows != 2
            || pixels.columns != 2
            || pixels.frames != 1
            || pixels.samples_per_pixel != 1
            || pixels.photometric_interpretation != "MONOCHROME2"
            || pixels.bits_allocated != 8
            || pixels.bits_stored != 8
            || pixels.high_bit != 7
            || pixels.pixel_representation != 0
            || pixels.pixel_data_vr != "OB"
            || pixels.stored_value_type != "u8"
            || pixels.stored_values.len() != 4
            || pixels.frame_sha256.len() != 1
            || pixels.padding.is_some()
            || pixels.palette.is_some()
            || pixels.color.is_some()
            || pixels.bit_packing.is_some()
            || pixels.integer_word.is_some()
            || pixels.encapsulation_projection.is_some()
            || pixels
                .stored_values
                .iter()
                .any(|value| !(0..=255).contains(value))
            || pixels.pixel_min != *pixels.stored_values.iter().min().unwrap()
            || pixels.pixel_max != *pixels.stored_values.iter().max().unwrap()
        {
            return Err(
                "timezone artifact requires an internally consistent native 2x2 U8 SC tuple".into(),
            );
        }
        let bytes = pixels
            .stored_values
            .iter()
            .map(|value| *value as u8)
            .collect::<Vec<_>>();
        if pixels.frame_sha256[0] != crate::sha256_hex(&bytes) {
            return Err("timezone artifact frame hash contradicts its caller-owned pixels".into());
        }
        let Some(MetadataScParameters::TimezoneBoundary(boundary)) = artifact.metadata_sc.as_ref()
        else {
            return Err("timezone pair cannot mix metadata variants".into());
        };
        crate::metadata::validate_timezone_boundary_definition(boundary)?;
        if !boundaries.insert(boundary.boundary_id.as_str()) {
            return Err("timezone pair boundary IDs must be unique".into());
        }
    }
    if boundaries != BTreeSet::from(["negative_min", "positive_max"]) {
        return Err("timezone pair requires exactly one negative_min and one positive_max".into());
    }
    Ok(Some(TimezoneBoundaryCapability { artifact_count: 2 }))
}

/// Recipe 0.2 encoded metadata carries the complete caller identity tuple.
pub(crate) fn inspect_encoded_metadata_capability(
    recipe: &CaseRecipe,
) -> Result<Option<&'static str>, String> {
    if recipe.plan_provider_id != "native.metadata_sc_plan"
        || recipe.case_recipe_schema_version != "0.2.0"
    {
        return Ok(None);
    }
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or("encoded metadata requires artifacts")?;
    let kind = match dicom.artifacts.first().and_then(|a| a.metadata_sc.as_ref()) {
        Some(MetadataScParameters::PersonName(_)) => "person_name",
        Some(MetadataScParameters::StringBoundaries { .. }) => "string_boundaries",
        Some(MetadataScParameters::SequenceLengths(_)) => "sequence_lengths",
        _ => return Ok(None),
    };
    if recipe.planning_order.is_none()
        || recipe.projection_order.is_none()
        || !recipe.dependencies.is_empty()
    {
        return Err("encoded metadata requires explicit independent ordering".into());
    }
    let mut variants = BTreeSet::new();
    for artifact in &dicom.artifacts {
        let pixels = artifact
            .secondary_capture
            .as_ref()
            .ok_or("missing metadata pixels")?;
        if pixels.frames != 1
            || pixels.samples_per_pixel != 1
            || pixels.photometric_interpretation != "MONOCHROME2"
            || pixels.stored_value_type != "u8"
            || pixels.bits_allocated != 8
            || pixels.bits_stored != 8
            || pixels.high_bit != 7
            || pixels.pixel_representation != 0
            || pixels.pixel_data_vr != "OB"
            || pixels.color.is_some()
            || pixels.palette.is_some()
            || pixels.padding.is_some()
            || pixels.bit_packing.is_some()
            || pixels.integer_word.is_some()
            || pixels.encapsulation_projection.is_some()
            || artifact.public_profile_membership.is_some()
        {
            return Err(
                "encoded metadata requires native single-frame unsigned-byte monochrome pixels"
                    .into(),
            );
        }
        let pn = matches!(
            artifact.metadata_sc,
            Some(MetadataScParameters::PersonName(_))
        );
        let mut tags = BTreeSet::new();
        for operation in &artifact.attribute_operations {
            let vr = match operation.tag.as_str() {
                "0010,0010" if !pn => "PN",
                "0010,0020" | "0008,0070" => "LO",
                "0010,0030" | "0008,0020" | "0008,0023" => "DA",
                "0010,0040" => "CS",
                "0008,0030" | "0008,0033" => "TM",
                "0020,0010" => "SH",
                _ => {
                    return Err(
                        "encoded metadata override conflicts with typed or structural metadata"
                            .into(),
                    );
                }
            };
            let value = operation
                .value
                .as_ref()
                .and_then(serde_json::Value::as_str)
                .ok_or("metadata override must be text")?;
            if operation.operation != "set"
                || operation.vr.as_deref() != Some(vr)
                || !tags.insert(operation.tag.as_str())
                || !value.is_ascii()
                || value.contains('\\')
            {
                return Err(
                    "metadata overrides require unique singleton ASCII set operations".into(),
                );
            }
            nested_string(
                &operation.tag,
                DicomVr::from_str(vr).map_err(|e| e.to_string())?,
                value,
            )
            .map_err(|e| e.to_string())?
            .validate_declared_vr()
            .map_err(|e| e.to_string())?;
        }
        for tag in [
            "0010,0010",
            "0010,0020",
            "0010,0030",
            "0010,0040",
            "0008,0020",
            "0008,0030",
            "0008,0023",
            "0008,0033",
            "0008,0070",
            "0020,0010",
        ] {
            if !(pn && tag == "0010,0010") && !tags.contains(tag) {
                return Err("encoded metadata requires complete caller patient/study tuple".into());
            }
        }
        if let Some(MetadataScParameters::SequenceLengths(sequence)) = &artifact.metadata_sc {
            if !variants.insert(sequence.variant_id.as_str()) {
                return Err("duplicate sequence variant".into());
            }
        }
    }
    if kind == "sequence_lengths" && variants != BTreeSet::from(["defined", "undefined"]) {
        return Err("encoded sequence capability requires defined and undefined variants".into());
    }
    Ok(Some(kind))
}

/// Stable metadata planning inputs available before staging exists.
pub struct MetadataScPlanInput<'a> {
    pub recipe: &'a CaseRecipe,
    pub artifact: &'a PlannedArtifactRecipe,
    pub template: &'a TemplateDescriptor,
    pub instance_id: &'a str,
    pub standards_lock_sha256: &'a str,
    pub seed: u64,
}

/// Resolve one typed metadata-SC artifact without reading or writing files.
pub fn resolved_metadata_sc_plan(
    input: MetadataScPlanInput<'_>,
) -> Result<ResolvedInstancePlan, MetadataScPlannerError> {
    if input.recipe.plan_provider_id != "native.metadata_sc_plan" {
        return Err(MetadataScPlannerError::WrongPlanProvider(
            input.recipe.plan_provider_id.clone(),
        ));
    }
    let metadata = input
        .artifact
        .metadata_sc
        .as_ref()
        .ok_or(MetadataScPlannerError::MissingMetadata)?;

    let mut plan = resolved_secondary_capture_base_plan(SecondaryCapturePlanInput {
        recipe: input.recipe,
        artifact: input.artifact,
        template: input.template,
        instance_id: input.instance_id,
        standards_lock_sha256: input.standards_lock_sha256,
        seed: input.seed,
    })
    .map_err(MetadataScPlannerError::SecondaryCapture)?;

    let mut attributes = plan
        .attributes
        .into_iter()
        .map(|attribute| (attribute.address.clone(), attribute))
        .collect::<BTreeMap<_, _>>();
    match metadata {
        MetadataScParameters::PersonName(person_name) => {
            if input.recipe.case_recipe_schema_version == "0.2.0" {
                crate::metadata::validate_caller_person_name(person_name)
                    .map_err(MetadataScPlannerError::DeclaredHashMismatch)?;
            }
            let raw = decode_hex(&person_name.patient_name_raw_hex)?;
            if sha256_hex(&raw) != person_name.patient_name_raw_sha256 {
                return Err(MetadataScPlannerError::DeclaredHashMismatch(
                    "Patient Name".into(),
                ));
            }
            set_value(
                &mut attributes,
                standard("0008,0005")?,
                DicomVr::CS,
                AttributeValue::Multi(
                    person_name
                        .specific_character_sets
                        .iter()
                        .cloned()
                        .map(PrimitiveValue::String)
                        .collect(),
                ),
            );
            // Raw encoded text is authoritative. This preserves ISO 2022
            // escape sequences and UTF-8 bytes without transcoding.
            set_value(
                &mut attributes,
                standard("0010,0010")?,
                DicomVr::PN,
                AttributeValue::EncodedText(raw),
            );
            set_laterality(&mut attributes)?;
        }
        MetadataScParameters::TimezoneBoundary(boundary) => {
            for (tag, vr, value) in [
                ("0008,0020", DicomVr::DA, boundary.study_date.as_str()),
                ("0008,0030", DicomVr::TM, boundary.study_time.as_str()),
                (
                    "0008,002A",
                    DicomVr::DT,
                    boundary.acquisition_date_time.as_str(),
                ),
                ("0008,0201", DicomVr::SH, boundary.timezone_offset.as_str()),
            ] {
                set_string(&mut attributes, tag, vr, value)?;
            }
            set_laterality(&mut attributes)?;
        }
        MetadataScParameters::EmptyType2 { attributes: empty } => {
            for item in empty {
                let address = standard(&item.tag)?;
                let vr = DicomVr::from_str(&item.vr).map_err(MetadataScPlannerError::Attribute)?;
                attributes.insert(
                    address.clone(),
                    ResolvedAttribute {
                        address,
                        vr,
                        value: None,
                        origin: ValueOrigin::InstanceOverride,
                    },
                );
            }
            set_laterality(&mut attributes)?;
        }
        MetadataScParameters::StringBoundaries { elements } => {
            for element in elements {
                let vr =
                    DicomVr::from_str(&element.vr).map_err(MetadataScPlannerError::Attribute)?;
                let values = if input.recipe.case_recipe_schema_version == "0.2.0" {
                    caller_string_values(element)
                        .map_err(MetadataScPlannerError::DeclaredHashMismatch)?
                } else {
                    match &element.source {
                        StringValueSource::Repeated {
                            pattern,
                            repetitions,
                        } => vec![
                            pattern.repeat(
                                usize::try_from(*repetitions)
                                    .map_err(|_| MetadataScPlannerError::SizeOverflow)?,
                            ),
                        ],
                        StringValueSource::Literal { values } => values.clone(),
                    }
                };
                validate_string_oracle(element, &values)?;
                let value = if values.len() == 1 {
                    AttributeValue::Primitive(PrimitiveValue::String(values[0].clone()))
                } else {
                    AttributeValue::Multi(values.into_iter().map(PrimitiveValue::String).collect())
                };
                set_value(&mut attributes, standard(&element.tag)?, vr, value);
            }
            set_laterality(&mut attributes)?;
        }
        MetadataScParameters::PrivateCreators { blocks } => {
            for block in blocks {
                validate_private_block(block)?;
                for element in &block.elements {
                    let tag = parse_tag(&element.tag)?;
                    let start = parse_tag(&block.block_start_tag)?;
                    let end = parse_tag(&block.block_end_tag)?;
                    if tag.group() != start.group()
                        || tag.element() < start.element()
                        || tag.element() > end.element()
                    {
                        return Err(MetadataScPlannerError::UnsupportedPrivateElement(
                            element.tag.clone(),
                        ));
                    }
                    let address = AttributeAddress::private(tag, block.creator_id.clone())
                        .map_err(MetadataScPlannerError::Attribute)?;
                    let (vr, value) = match &element.value {
                        PrivateElementValue::Lo { text } => (
                            DicomVr::LO,
                            AttributeValue::Primitive(PrimitiveValue::String(text.clone())),
                        ),
                        PrivateElementValue::Us { number } => (
                            DicomVr::US,
                            AttributeValue::Primitive(PrimitiveValue::Unsigned((*number).into())),
                        ),
                    };
                    set_value(&mut attributes, address, vr, value);
                }
            }
            set_laterality(&mut attributes)?;
        }
        MetadataScParameters::SequenceLengths(sequence) => {
            if !(if input.recipe.case_recipe_schema_version == "0.2.0" {
                caller_sequence_contract(sequence, &input.artifact.encoding)
            } else {
                supported_sequence_contract(sequence, &input.artifact.encoding)
            }) {
                return Err(MetadataScPlannerError::UnsupportedSequence(
                    sequence.variant_id.clone(),
                ));
            }
            let item = AttributeItem {
                attributes: vec![
                    nested_string("0008,0100", DicomVr::SH, &sequence.code_value)?,
                    nested_string("0008,0102", DicomVr::SH, &sequence.coding_scheme_designator)?,
                    nested_string("0008,0104", DicomVr::LO, &sequence.code_meaning)?,
                ],
            };
            set_value(
                &mut attributes,
                standard(&sequence.sequence_tag)?,
                DicomVr::SQ,
                AttributeValue::Sequence(vec![item]),
            );
            // Sequence metadata intentionally has no Laterality in the legacy
            // fixture; the encoding plan owns defined/undefined SQ/item forms.
        }
    }
    plan.attributes = attributes.into_values().collect();
    Ok(plan)
}

fn set_laterality(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
) -> Result<(), MetadataScPlannerError> {
    set_string(attributes, "0020,0060", DicomVr::CS, "R")
}

fn nested_string(
    tag: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, MetadataScPlannerError> {
    Ok(AttributeOperation::Set {
        address: standard(tag)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    })
}

fn set_string(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    tag: &str,
    vr: DicomVr,
    value: &str,
) -> Result<(), MetadataScPlannerError> {
    set_value(
        attributes,
        standard(tag)?,
        vr,
        AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    );
    Ok(())
}

fn set_value(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    address: AttributeAddress,
    vr: DicomVr,
    value: AttributeValue,
) {
    attributes.insert(
        address.clone(),
        ResolvedAttribute {
            address,
            vr,
            value: Some(value),
            origin: ValueOrigin::InstanceOverride,
        },
    );
}

fn validate_string_oracle(
    element: &super::StringBoundaryElementMetadata,
    values: &[String],
) -> Result<(), MetadataScPlannerError> {
    let mut raw = values.join("\\").into_bytes();
    let actual_padding = if raw.len() % 2 == 1 {
        raw.push(b' ');
        "space"
    } else {
        "none"
    };
    let declared_length = usize::try_from(element.raw_value_byte_length)
        .map_err(|_| MetadataScPlannerError::SizeOverflow)?;
    if raw.len() != declared_length
        || sha256_hex(&raw) != element.raw_value_sha256
        || actual_padding != element.padding
    {
        return Err(MetadataScPlannerError::DeclaredHashMismatch(
            element.tag.clone(),
        ));
    }
    Ok(())
}

fn validate_private_block(
    block: &super::PrivateCreatorBlockMetadata,
) -> Result<(), MetadataScPlannerError> {
    let creator = parse_tag(&block.creator_tag)?;
    let start = parse_tag(&block.block_start_tag)?;
    let end = parse_tag(&block.block_end_tag)?;
    if creator.group() % 2 == 0
        || creator.group() != start.group()
        || start.group() != end.group()
        || creator.element() < 0x0010
        || creator.element() > 0x00ff
        || start.element() != creator.element() << 8
        || end.element() != (creator.element() << 8) | 0x00ff
    {
        return Err(MetadataScPlannerError::UnsupportedPrivateBlock(
            block.creator_tag.clone(),
        ));
    }
    Ok(())
}

/// Bound declarative expansion before allocating; encoded short-VR values
/// cannot exceed 65534 bytes, while LT is bounded to 10240 characters.
pub(crate) fn caller_string_values(
    element: &super::StringBoundaryElementMetadata,
) -> Result<Vec<String>, String> {
    let limit = if element.vr == "LT" {
        10240_u64
    } else {
        65534_u64
    };
    let values = match &element.source {
        StringValueSource::Repeated {
            pattern,
            repetitions,
        } => {
            let size = (pattern.len() as u64)
                .checked_mul(u64::from(*repetitions))
                .filter(|size| *size <= limit)
                .ok_or("metadata string expansion exceeds VR limit")?;
            if size == 0 {
                return Err("metadata string expansion must be nonempty".into());
            }
            vec![pattern.repeat(usize::try_from(*repetitions).map_err(|_| "string size overflow")?)]
        }
        StringValueSource::Literal { values } => {
            let size = values
                .iter()
                .try_fold(0_u64, |n, value| {
                    n.checked_add(value.len() as u64)
                        .and_then(|n| n.checked_add(1))
                })
                .ok_or("string size overflow")?;
            if size.saturating_sub(1) > limit {
                return Err("metadata string values exceed VR limit".into());
            }
            values.clone()
        }
    };
    let raw = crate::metadata::validate_caller_string_values(
        &element.tag,
        &element.keyword,
        &element.vr,
        &values,
    )?;
    let unpadded = values.join("\\").len();
    if raw.len() as u64 != u64::from(element.raw_value_byte_length)
        || sha256_hex(&raw) != element.raw_value_sha256
        || element.padding != if unpadded % 2 == 1 { "space" } else { "none" }
    {
        return Err("caller string oracle differs from encoded values".into());
    }
    Ok(values)
}

/// Explicit-VR LE code item: three short-VR headers, padded values, and
/// the undefined item header/delimiter. Caller text owns the resulting lengths.
pub(crate) fn caller_sequence_contract(
    sequence: &super::SequenceLengthMetadata,
    encoding: &super::EncodingPolicy,
) -> bool {
    crate::metadata::caller_sequence_bytes(sequence).is_ok()
        && encoding.item_length_policy == "undefined"
        && encoding.sequence_length_policy == sequence.variant_id
}

fn supported_sequence_contract(
    sequence: &super::SequenceLengthMetadata,
    encoding: &super::EncodingPolicy,
) -> bool {
    let common = sequence.sequence_tag == "0008,2218"
        && sequence.sequence_vr == "SQ"
        && sequence.item_dataset_encoded_length == 40
        && sequence.undefined_item_encoded_length == 56
        && sequence.item_length_field_hex == "FFFFFFFF"
        && sequence.item_delimitation_present
        && encoding.item_length_policy == "undefined";
    common
        && match sequence.variant_id.as_str() {
            "defined" => {
                encoding.sequence_length_policy == "defined"
                    && sequence.sequence_length_field_hex == "38000000"
                    && !sequence.sequence_delimitation_present
            }
            "undefined" => {
                encoding.sequence_length_policy == "undefined"
                    && sequence.sequence_length_field_hex == "FFFFFFFF"
                    && sequence.sequence_delimitation_present
            }
            _ => false,
        }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, MetadataScPlannerError> {
    if value.len() % 2 != 0 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MetadataScPlannerError::MalformedHex);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or(MetadataScPlannerError::MalformedHex)
        })
        .collect()
}

fn standard(value: &str) -> Result<AttributeAddress, MetadataScPlannerError> {
    AttributeAddress::from_normalized_tag(value).map_err(MetadataScPlannerError::Attribute)
}

fn parse_tag(value: &str) -> Result<dicom_core::Tag, MetadataScPlannerError> {
    if value.len() != 9 || value.as_bytes()[4] != b',' {
        return Err(MetadataScPlannerError::MalformedTag(value.into()));
    }
    let group = u16::from_str_radix(&value[..4], 16)
        .map_err(|_| MetadataScPlannerError::MalformedTag(value.into()))?;
    let element = u16::from_str_radix(&value[5..], 16)
        .map_err(|_| MetadataScPlannerError::MalformedTag(value.into()))?;
    if value != format!("{group:04X},{element:04X}") {
        return Err(MetadataScPlannerError::MalformedTag(value.into()));
    }
    Ok(dicom_core::Tag(group, element))
}

#[derive(Debug)]
pub enum MetadataScPlannerError {
    WrongPlanProvider(String),
    MissingMetadata,
    MalformedHex,
    MalformedTag(String),
    DeclaredHashMismatch(String),
    SizeOverflow,
    UnsupportedPrivateBlock(String),
    UnsupportedPrivateElement(String),
    UnsupportedSequence(String),
    SecondaryCapture(super::ScPlanError),
    Attribute(crate::composition::AttributeError),
}

impl fmt::Display for MetadataScPlannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for MetadataScPlannerError {}
