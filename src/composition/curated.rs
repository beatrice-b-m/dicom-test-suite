use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use dicom_core::header::Header;
use dicom_core::value::{PrimitiveValue as DicomPrimitive, Value as DicomValue};
use dicom_core::{Tag, VR};
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::InMemDicomObject;

use super::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    CompositionUidRole, ContentMaterialization, DicomVr, IdentityError, IdentityPlan,
    PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan, TemplateId, TemplateVersion,
    ValueOrigin,
};
use crate::sha256_hex;

type Dataset = InMemDicomObject<StandardDataDictionary>;

#[derive(Debug)]
pub struct CuratedPlanInput<'a> {
    pub instance_id: &'a str,
    pub template_id: TemplateId,
    pub template_version: TemplateVersion,
    pub sop_class_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub study_instance_uid: Option<&'a str>,
    pub series_instance_uid: Option<&'a str>,
    pub sop_instance_uid: &'a str,
    pub implementation_class_uid: &'a str,
}

pub fn resolved_plan_from_curated_dataset(
    object: &Dataset,
    input: CuratedPlanInput<'_>,
) -> Result<ResolvedInstancePlan, CuratedPlanError> {
    let mut identities = vec![
        (
            CompositionUidRole::SopInstance,
            0,
            input.sop_instance_uid.to_string(),
        ),
        (
            CompositionUidRole::ImplementationClass,
            0,
            input.implementation_class_uid.to_string(),
        ),
    ];
    if let Some(value) = input.study_instance_uid {
        identities.push((CompositionUidRole::StudyInstance, 0, value.to_string()));
    }
    if let Some(value) = input.series_instance_uid {
        identities.push((CompositionUidRole::SeriesInstance, 0, value.to_string()));
    }
    let identities = IdentityPlan::from_exact_values(input.instance_id, identities)?;

    let private_creators = private_creator_map(object)?;
    let mut attributes = Vec::new();
    let mut content = Vec::new();
    for element in object.iter() {
        let tag = element.tag();
        if tag == tags::PIXEL_DATA
            || tag == tags::FLOAT_PIXEL_DATA
            || tag == tags::DOUBLE_FLOAT_PIXEL_DATA
        {
            content.push(content_from_element(element)?);
            continue;
        }
        if tag.group() & 1 == 1 && (0x0010..=0x00ff).contains(&tag.element()) {
            continue;
        }
        let address = if tag.group() & 1 == 1 && tag.element() >= 0x1000 {
            let creator = private_creators
                .get(&(tag.group(), tag.element() >> 8))
                .ok_or(CuratedPlanError::PrivateCreatorRequired(tag))?;
            AttributeAddress::private(tag, creator.clone())
        } else {
            AttributeAddress::standard(tag)
        }
        .map_err(|error| CuratedPlanError::Attribute(error.to_string()))?;
        let vr = DicomVr::from_str(&element.vr().to_string())
            .map_err(|error| CuratedPlanError::Attribute(error.to_string()))?;
        attributes.push(ResolvedAttribute {
            address,
            vr,
            value: attribute_value(element.value(), element.vr())?,
            origin: ValueOrigin::InstanceOverride,
        });
    }

    Ok(ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: input.instance_id.into(),
        template_id: input.template_id,
        template_version: input.template_version,
        sop_class_uid: input.sop_class_uid.into(),
        transfer_syntax_uid: input.transfer_syntax_uid.into(),
        identities,
        attributes,
        content,
        references: Vec::new(),
    })
}

fn private_creator_map(object: &Dataset) -> Result<BTreeMap<(u16, u16), String>, CuratedPlanError> {
    let mut creators = BTreeMap::new();
    for element in object.iter() {
        let tag = element.tag();
        if tag.group() & 1 == 1 && (0x0010..=0x00ff).contains(&tag.element()) {
            let creator = element
                .to_str()
                .map_err(|error| CuratedPlanError::Value(error.to_string()))?
                .trim_end_matches([' ', '\0'])
                .to_string();
            creators.insert((tag.group(), tag.element()), creator);
        }
    }
    Ok(creators)
}

fn content_from_element(
    element: &dicom_core::DataElement<Dataset, Vec<u8>>,
) -> Result<CanonicalContent, CuratedPlanError> {
    let address = AttributeAddress::standard(element.tag())
        .map_err(|error| CuratedPlanError::Attribute(error.to_string()))?;
    let vr = DicomVr::from_str(&element.vr().to_string())
        .map_err(|error| CuratedPlanError::Attribute(error.to_string()))?;
    let (kind, bytes, materialization) = match element.value() {
        DicomValue::Primitive(value) => {
            let bytes = value.to_bytes().into_owned();
            (
                "native_pixels",
                bytes.clone(),
                ContentMaterialization::Inline(bytes),
            )
        }
        DicomValue::PixelSequence(sequence) => {
            let fragments = sequence.fragments().to_vec();
            let bytes = fragments.concat();
            (
                "encapsulated_pixels",
                bytes,
                ContentMaterialization::Encapsulated {
                    basic_offset_table: sequence.offset_table().to_vec(),
                    fragments,
                },
            )
        }
        DicomValue::Sequence(_) => return Err(CuratedPlanError::PixelDataSequence),
    };
    Ok(CanonicalContent {
        slot: "pixels".into(),
        kind: kind.into(),
        address,
        vr,
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        properties: Default::default(),
        placement: super::ContentPlacement::TopLevel,
        materialization: Some(materialization),
    })
}

fn attribute_value(
    value: &DicomValue<Dataset, Vec<u8>>,
    vr: VR,
) -> Result<Option<AttributeValue>, CuratedPlanError> {
    match value {
        DicomValue::Primitive(DicomPrimitive::Empty) => Ok(None),
        DicomValue::Primitive(value) => primitive_value(value, vr).map(Some),
        DicomValue::Sequence(sequence) => sequence
            .items()
            .iter()
            .map(|item| {
                let attributes = item
                    .iter()
                    .map(|element| {
                        let address = AttributeAddress::standard(element.tag())
                            .map_err(|error| CuratedPlanError::Attribute(error.to_string()))?;
                        let vr = DicomVr::from_str(&element.vr().to_string())
                            .map_err(|error| CuratedPlanError::Attribute(error.to_string()))?;
                        let value = attribute_value(element.value(), element.vr())?
                            .unwrap_or_else(|| AttributeValue::Binary(vec![]));
                        Ok(AttributeOperation::Set { address, vr, value })
                    })
                    .collect::<Result<Vec<_>, CuratedPlanError>>()?;
                Ok(AttributeItem { attributes })
            })
            .collect::<Result<Vec<_>, CuratedPlanError>>()
            .map(AttributeValue::Sequence)
            .map(Some),
        DicomValue::PixelSequence(_) => Err(CuratedPlanError::UnexpectedPixelSequence),
    }
}

fn primitive_value(value: &DicomPrimitive, vr: VR) -> Result<AttributeValue, CuratedPlanError> {
    let strings = |values: Vec<String>| {
        if values.len() == 1 {
            AttributeValue::Primitive(PrimitiveValue::String(values[0].clone()))
        } else {
            AttributeValue::Multi(values.into_iter().map(PrimitiveValue::String).collect())
        }
    };
    Ok(match value {
        DicomPrimitive::Empty => return Err(CuratedPlanError::UnexpectedEmpty),
        DicomPrimitive::Str(value) => {
            AttributeValue::Primitive(PrimitiveValue::String(value.clone()))
        }
        DicomPrimitive::Strs(values) => strings(values.to_vec()),
        DicomPrimitive::Tags(values) => multi_or_one(
            values
                .iter()
                .map(|tag| {
                    AttributeAddress::standard(*tag)
                        .map(PrimitiveValue::Tag)
                        .map_err(|error| CuratedPlanError::Attribute(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        DicomPrimitive::U8(values) => AttributeValue::Binary(values.to_vec()),
        DicomPrimitive::I16(values) if vr == VR::IS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::U16(values) if vr == VR::IS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::I32(values) if vr == VR::IS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::U32(values) if vr == VR::IS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::I64(values) if vr == VR::IS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::U64(values) if vr == VR::IS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::F32(values) if vr == VR::DS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::F64(values) if vr == VR::DS => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::I16(values) if vr == VR::SS => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Signed(i64::from(*value)))
                .collect(),
        ),
        DicomPrimitive::U16(values) if vr == VR::US => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Unsigned(u64::from(*value)))
                .collect(),
        ),
        DicomPrimitive::I32(values) if vr == VR::SL => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Signed(i64::from(*value)))
                .collect(),
        ),
        DicomPrimitive::U32(values) if vr == VR::UL => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Unsigned(u64::from(*value)))
                .collect(),
        ),
        DicomPrimitive::I64(values) if vr == VR::SV => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Signed(*value))
                .collect(),
        ),
        DicomPrimitive::U64(values) if vr == VR::UV => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Unsigned(*value))
                .collect(),
        ),
        DicomPrimitive::F32(values) if vr == VR::FL => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Float32Bits(value.to_bits()))
                .collect(),
        ),
        DicomPrimitive::F64(values) if vr == VR::FD => multi_or_one(
            values
                .iter()
                .map(|value| PrimitiveValue::Float64Bits(value.to_bits()))
                .collect(),
        ),
        DicomPrimitive::U16(values) => binary_u16(values),
        DicomPrimitive::U32(values) => binary_u32(values),
        DicomPrimitive::U64(values) => binary_u64(values),
        DicomPrimitive::I16(values) => {
            binary_u16(&values.iter().map(|v| *v as u16).collect::<Vec<_>>())
        }
        DicomPrimitive::I32(values) => {
            binary_u32(&values.iter().map(|v| *v as u32).collect::<Vec<_>>())
        }
        DicomPrimitive::I64(values) => {
            binary_u64(&values.iter().map(|v| *v as u64).collect::<Vec<_>>())
        }
        DicomPrimitive::F32(values) => {
            binary_u32(&values.iter().map(|v| v.to_bits()).collect::<Vec<_>>())
        }
        DicomPrimitive::F64(values) => {
            binary_u64(&values.iter().map(|v| v.to_bits()).collect::<Vec<_>>())
        }
        DicomPrimitive::Date(values) => strings(values.iter().map(ToString::to_string).collect()),
        DicomPrimitive::DateTime(values) => {
            strings(values.iter().map(ToString::to_string).collect())
        }
        DicomPrimitive::Time(values) => strings(values.iter().map(ToString::to_string).collect()),
    })
}

fn multi_or_one(values: Vec<PrimitiveValue>) -> AttributeValue {
    if values.len() == 1 {
        AttributeValue::Primitive(values[0].clone())
    } else {
        AttributeValue::Multi(values)
    }
}

fn binary_u16(values: &[u16]) -> AttributeValue {
    AttributeValue::Binary(
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    )
}

fn binary_u32(values: &[u32]) -> AttributeValue {
    AttributeValue::Binary(
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    )
}

fn binary_u64(values: &[u64]) -> AttributeValue {
    AttributeValue::Binary(
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    )
}

#[derive(Debug)]
pub enum CuratedPlanError {
    Attribute(String),
    Identity(IdentityError),
    Value(String),
    PrivateCreatorRequired(Tag),
    PixelDataSequence,
    UnexpectedPixelSequence,
    UnexpectedEmpty,
}

impl From<IdentityError> for CuratedPlanError {
    fn from(value: IdentityError) -> Self {
        Self::Identity(value)
    }
}

impl fmt::Display for CuratedPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CuratedPlanError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use dicom_core::DataElement;
    use dicom_core::value::{PixelFragmentSequence, PrimitiveValue};
    use dicom_object::FileMetaTableBuilder;

    use super::*;
    use crate::composition::Part10Materializer;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn output(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "dts-curated-plan-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn dataset(pixel_value: DicomValue<Dataset, Vec<u8>>, pixel_vr: VR) -> Dataset {
        let mut object = Dataset::new_empty();
        object.put(DataElement::new(
            tags::SOP_CLASS_UID,
            VR::UI,
            "1.2.840.10008.5.1.4.1.1.7",
        ));
        object.put(DataElement::new(tags::SOP_INSTANCE_UID, VR::UI, "2.25.3"));
        object.put(DataElement::new(tags::STUDY_INSTANCE_UID, VR::UI, "2.25.1"));
        object.put(DataElement::new(
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            "2.25.2",
        ));
        object.put(DataElement::new(tags::PATIENT_NAME, VR::PN, "DTS^CURATED"));
        object.put(DataElement::new(
            tags::SAMPLES_PER_PIXEL,
            VR::US,
            PrimitiveValue::from(1_u16),
        ));
        object.put(DataElement::new(tags::PIXEL_DATA, pixel_vr, pixel_value));
        object
    }

    fn assert_exact_bridge(object: Dataset, transfer_syntax_uid: &str) {
        let root = output("exact");
        fs::create_dir(&root).unwrap();
        let legacy = root.join("legacy.dcm");
        let composed = root.join("composed.dcm");
        object
            .clone()
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(transfer_syntax_uid)
                    .implementation_class_uid("2.25.4")
                    .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
            )
            .unwrap()
            .write_to_file(&legacy)
            .unwrap();
        let plan = resolved_plan_from_curated_dataset(
            &object,
            CuratedPlanInput {
                instance_id: "curated",
                template_id: TemplateId("classic/secondary-capture/monochrome".into()),
                template_version: "1.0.0".parse().unwrap(),
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.7",
                transfer_syntax_uid,
                study_instance_uid: Some("2.25.1"),
                series_instance_uid: Some("2.25.2"),
                sop_instance_uid: "2.25.3",
                implementation_class_uid: "2.25.4",
            },
        )
        .unwrap();
        Part10Materializer.materialize(&plan, &composed).unwrap();
        assert_eq!(fs::read(legacy).unwrap(), fs::read(composed).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_curated_dataset_round_trips_through_the_shared_plan_byte_exactly() {
        assert_exact_bridge(
            dataset(
                PrimitiveValue::U8(vec![0_u8, 1, 2, 3].into()).into(),
                VR::OB,
            ),
            "1.2.840.10008.1.2.1",
        );
    }

    #[test]
    fn encapsulated_curated_dataset_round_trips_through_the_shared_plan_byte_exactly() {
        assert_exact_bridge(
            dataset(
                PixelFragmentSequence::new(vec![0], vec![vec![1_u8, 2, 3, 4]]).into(),
                VR::OB,
            ),
            "1.2.840.10008.1.2.5",
        );
    }
}
