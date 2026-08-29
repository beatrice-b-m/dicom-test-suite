//! Direct plan-only translation for typed Secondary Capture metadata recipes.

use std::collections::BTreeMap;
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
                let values = match &element.source {
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
            if !supported_sequence_contract(sequence, &input.artifact.encoding) {
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
