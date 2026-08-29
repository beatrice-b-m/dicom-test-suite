use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::value::{DataSetSequence, PrimitiveValue as DicomPrimitiveValue};
use dicom_core::{DataElement, Tag};
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject, open_file};

use super::{
    AttributeAddress, AttributeItem, AttributeValue, CompositionUidRole, ContentMaterialization,
    DicomVr, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
};
use crate::{IMPLEMENTATION_VERSION_NAME, sha256_hex};

type Dataset = InMemDicomObject<StandardDataDictionary>;

#[derive(Debug, Default, Clone, Copy)]
pub struct Part10Materializer;

impl Part10Materializer {
    pub fn materialize(
        &self,
        plan: &ResolvedInstancePlan,
        path: impl AsRef<Path>,
    ) -> Result<(), MaterializeError> {
        let path = path.as_ref();
        if path.exists() {
            return Err(MaterializeError::OutputExists(path.to_path_buf()));
        }
        let parent = path
            .parent()
            .ok_or_else(|| MaterializeError::MissingParent(path.to_path_buf()))?;
        fs::create_dir_all(parent).map_err(|source| MaterializeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;

        let mut object = build_dataset(plan)?;
        let sop_instance_uid = plan
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .ok_or(MaterializeError::MissingIdentity("sop_instance_uid"))?;
        let implementation_class_uid = plan
            .identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .ok_or(MaterializeError::MissingIdentity(
                "implementation_class_uid",
            ))?;
        ensure_string(
            &mut object,
            tags::SOP_CLASS_UID,
            DicomVr::UI,
            &plan.sop_class_uid,
        )?;
        ensure_string(
            &mut object,
            tags::SOP_INSTANCE_UID,
            DicomVr::UI,
            sop_instance_uid,
        )?;

        object
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(&plan.transfer_syntax_uid)
                    .implementation_class_uid(implementation_class_uid)
                    .implementation_version_name(IMPLEMENTATION_VERSION_NAME),
            )
            .map_err(|error| MaterializeError::Dicom(error.to_string()))?
            .write_to_file(path)
            .map_err(|error| MaterializeError::Dicom(error.to_string()))?;

        let reopened =
            open_file(path).map_err(|error| MaterializeError::Dicom(error.to_string()))?;
        if reopened.meta().transfer_syntax() != plan.transfer_syntax_uid
            || reopened.meta().media_storage_sop_class_uid() != plan.sop_class_uid
            || reopened.meta().media_storage_sop_instance_uid() != sop_instance_uid
        {
            return Err(MaterializeError::IdentityRoundTrip);
        }
        Ok(())
    }
}

fn build_dataset(plan: &ResolvedInstancePlan) -> Result<Dataset, MaterializeError> {
    let mut object = Dataset::new_empty();
    let mut creators = BTreeMap::new();
    let mut element_tags = BTreeSet::new();
    let content_tags = plan
        .content
        .iter()
        .map(|content| content.address.clone())
        .collect::<BTreeSet<_>>();
    for attribute in &plan.attributes {
        if attribute.address.group == 0x0002 {
            return Err(MaterializeError::StructuralConflict(
                attribute.address.normalized_tag(),
            ));
        }
        if !element_tags.insert(attribute.address.clone()) {
            return Err(MaterializeError::DuplicateElement(
                attribute.address.normalized_tag(),
            ));
        }
        if content_tags.contains(&attribute.address) {
            return Err(MaterializeError::DuplicateElement(
                attribute.address.normalized_tag(),
            ));
        }
        put_resolved_attribute(&mut object, attribute, &mut creators)?;
    }
    for content in &plan.content {
        if content.address.group == 0x0002 || !element_tags.insert(content.address.clone()) {
            return Err(MaterializeError::DuplicateElement(
                content.address.normalized_tag(),
            ));
        }
        let bytes = match content.materialization.as_ref() {
            Some(ContentMaterialization::Inline(bytes)) => bytes.clone(),
            Some(ContentMaterialization::StagedFile(path)) => {
                fs::read(path).map_err(|source| MaterializeError::Io {
                    path: path.clone(),
                    source,
                })?
            }
            None => return Err(MaterializeError::MissingContent(content.slot.clone())),
        };
        if bytes.len() as u64 != content.size_bytes {
            return Err(MaterializeError::ContentSize {
                slot: content.slot.clone(),
                expected: content.size_bytes,
                actual: bytes.len() as u64,
            });
        }
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != content.sha256 {
            return Err(MaterializeError::ContentHash {
                slot: content.slot.clone(),
                expected: content.sha256.clone(),
                actual: actual_hash,
            });
        }
        put_private_creator(&mut object, &content.address, &mut creators)?;
        object.put(DataElement::new(
            content.address.tag(),
            content.vr.as_dicom(),
            DicomPrimitiveValue::from(bytes),
        ));
    }
    Ok(object)
}

fn put_resolved_attribute(
    object: &mut Dataset,
    attribute: &ResolvedAttribute,
    creators: &mut BTreeMap<Tag, String>,
) -> Result<(), MaterializeError> {
    put_private_creator(object, &attribute.address, creators)?;
    let value: dicom_core::value::Value<Dataset, Vec<u8>> = match &attribute.value {
        None => DicomPrimitiveValue::Empty.into(),
        Some(AttributeValue::Primitive(value)) => primitive(value, attribute.vr)?.into(),
        Some(AttributeValue::Multi(values)) => multi(values, attribute.vr)?.into(),
        Some(AttributeValue::Binary(bytes)) => DicomPrimitiveValue::from(bytes.clone()).into(),
        Some(AttributeValue::Sequence(items)) => {
            let items = items
                .iter()
                .map(build_item)
                .collect::<Result<Vec<_>, _>>()?;
            DataSetSequence::from(items).into()
        }
    };
    object.put(DataElement::new(
        attribute.address.tag(),
        attribute.vr.as_dicom(),
        value,
    ));
    Ok(())
}

fn build_item(item: &AttributeItem) -> Result<Dataset, MaterializeError> {
    let mut object = Dataset::new_empty();
    let mut creators = BTreeMap::new();
    for operation in &item.attributes {
        let super::AttributeOperation::Set { address, vr, value } = operation else {
            return Err(MaterializeError::UnresolvedNestedOperation);
        };
        put_private_creator(&mut object, address, &mut creators)?;
        let attribute = ResolvedAttribute {
            address: address.clone(),
            vr: *vr,
            value: Some(value.clone()),
            origin: super::ValueOrigin::InstanceOverride,
        };
        put_resolved_attribute(&mut object, &attribute, &mut creators)?;
    }
    Ok(object)
}

fn put_private_creator(
    object: &mut Dataset,
    address: &AttributeAddress,
    creators: &mut BTreeMap<Tag, String>,
) -> Result<(), MaterializeError> {
    let Some(creator) = &address.private_creator else {
        return Ok(());
    };
    let creator_tag = Tag(address.group, address.element >> 8);
    if let Some(previous) = creators.insert(creator_tag, creator.clone()) {
        if previous != *creator {
            return Err(MaterializeError::PrivateCreatorConflict {
                tag: format!("{:04X},{:04X}", creator_tag.group(), creator_tag.element()),
            });
        }
        return Ok(());
    }
    object.put(DataElement::new(
        creator_tag,
        dicom_core::VR::LO,
        creator.as_str(),
    ));
    Ok(())
}

fn primitive(value: &PrimitiveValue, vr: DicomVr) -> Result<DicomPrimitiveValue, MaterializeError> {
    Ok(match value {
        PrimitiveValue::String(value) => DicomPrimitiveValue::from(value.clone()),
        PrimitiveValue::Signed(value) => match vr {
            DicomVr::SS => DicomPrimitiveValue::from(
                i16::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::SL => DicomPrimitiveValue::from(
                i32::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::SV => DicomPrimitiveValue::from(*value),
            _ => return Err(MaterializeError::NumericRange),
        },
        PrimitiveValue::Unsigned(value) => match vr {
            DicomVr::US => DicomPrimitiveValue::from(
                u16::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::UL => DicomPrimitiveValue::from(
                u32::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::UV => DicomPrimitiveValue::from(*value),
            _ => return Err(MaterializeError::NumericRange),
        },
        PrimitiveValue::Float32Bits(value) => DicomPrimitiveValue::from(f32::from_bits(*value)),
        PrimitiveValue::Float64Bits(value) => DicomPrimitiveValue::from(f64::from_bits(*value)),
        PrimitiveValue::Tag(value) => DicomPrimitiveValue::from(value.tag()),
    })
}

fn multi(values: &[PrimitiveValue], vr: DicomVr) -> Result<DicomPrimitiveValue, MaterializeError> {
    if values
        .iter()
        .all(|value| matches!(value, PrimitiveValue::String(_)))
    {
        let joined = values
            .iter()
            .map(|value| match value {
                PrimitiveValue::String(value) => value.as_str(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("\\");
        return Ok(DicomPrimitiveValue::from(joined));
    }
    macro_rules! numeric_vec {
        ($variant:ident, $source:ident, $type:ty) => {{
            let converted = values
                .iter()
                .map(|value| match value {
                    PrimitiveValue::$source(value) => {
                        <$type>::try_from(*value).map_err(|_| MaterializeError::NumericRange)
                    }
                    _ => Err(MaterializeError::NumericRange),
                })
                .collect::<Result<Vec<$type>, _>>()?;
            DicomPrimitiveValue::$variant(converted.into())
        }};
    }
    Ok(match vr {
        DicomVr::AT => {
            let tags = values
                .iter()
                .map(|value| match value {
                    PrimitiveValue::Tag(value) => Ok(value.tag()),
                    _ => Err(MaterializeError::NumericRange),
                })
                .collect::<Result<Vec<_>, _>>()?;
            DicomPrimitiveValue::Tags(tags.into())
        }
        DicomVr::SS => numeric_vec!(I16, Signed, i16),
        DicomVr::SL => numeric_vec!(I32, Signed, i32),
        DicomVr::SV => numeric_vec!(I64, Signed, i64),
        DicomVr::US => numeric_vec!(U16, Unsigned, u16),
        DicomVr::UL => numeric_vec!(U32, Unsigned, u32),
        DicomVr::UV => numeric_vec!(U64, Unsigned, u64),
        _ => return Err(MaterializeError::NumericRange),
    })
}

fn ensure_string(
    object: &mut Dataset,
    tag: Tag,
    vr: DicomVr,
    expected: &str,
) -> Result<(), MaterializeError> {
    if let Ok(element) = object.element(tag) {
        if element.vr() != vr.as_dicom() || element.to_str().ok().as_deref() != Some(expected) {
            return Err(MaterializeError::StructuralConflict(format!(
                "{:04X},{:04X}",
                tag.group(),
                tag.element()
            )));
        }
    } else {
        object.put(DataElement::new(tag, vr.as_dicom(), expected));
    }
    Ok(())
}

#[derive(Debug)]
pub enum MaterializeError {
    OutputExists(PathBuf),
    MissingParent(PathBuf),
    MissingIdentity(&'static str),
    MissingContent(String),
    DuplicateElement(String),
    ContentSize {
        slot: String,
        expected: u64,
        actual: u64,
    },
    ContentHash {
        slot: String,
        expected: String,
        actual: String,
    },
    PrivateCreatorConflict {
        tag: String,
    },
    UnresolvedNestedOperation,
    NumericRange,
    StructuralConflict(String),
    IdentityRoundTrip,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Dicom(String),
}

impl fmt::Display for MaterializeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MaterializeError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::composition::{AttributeItem, IdentityAllocator, TemplateId, ValueOrigin};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn plan(pixel_bytes: Vec<u8>) -> ResolvedInstancePlan {
        let template_id = TemplateId("classic/secondary-capture/monochrome".into());
        let version = "1.0.0".parse().unwrap();
        let identities = IdentityAllocator::new(LOCK_HASH, template_id.clone(), version, 1)
            .unwrap()
            .allocate_plan(
                "primary",
                [
                    (CompositionUidRole::SopInstance, 0),
                    (CompositionUidRole::ImplementationClass, 0),
                ],
            )
            .unwrap();
        let attr = |tag: &str, vr, value: &str| ResolvedAttribute {
            address: AttributeAddress::from_normalized_tag(tag).unwrap(),
            vr,
            value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                value.to_string(),
            ))),
            origin: ValueOrigin::TemplateDefault,
        };
        ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: "primary".into(),
            template_id,
            template_version: version,
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities,
            attributes: vec![
                attr("0008,001C", DicomVr::CS, "YES"),
                attr("0010,0010", DicomVr::PN, "DTS^Synthetic"),
                ResolvedAttribute {
                    address: AttributeAddress::from_normalized_tag("0008,1115").unwrap(),
                    vr: DicomVr::SQ,
                    value: Some(AttributeValue::Sequence(vec![AttributeItem {
                        attributes: vec![super::super::AttributeOperation::Set {
                            address: AttributeAddress::from_normalized_tag("0020,000E").unwrap(),
                            vr: DicomVr::UI,
                            value: AttributeValue::Primitive(PrimitiveValue::String(
                                "2.25.99".into(),
                            )),
                        }],
                    }])),
                    origin: ValueOrigin::InstanceOverride,
                },
            ],
            content: vec![super::super::CanonicalContent {
                slot: "pixels".into(),
                kind: "native_pixels".into(),
                address: AttributeAddress::from_normalized_tag("7FE0,0010").unwrap(),
                vr: DicomVr::OB,
                size_bytes: pixel_bytes.len() as u64,
                sha256: sha256_hex(&pixel_bytes),
                properties: BTreeMap::new(),
                materialization: Some(ContentMaterialization::Inline(pixel_bytes)),
            }],
            references: vec![],
        }
    }

    #[test]
    fn writes_reopenable_part10_only_from_resolved_plan() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-materializer-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Part10Materializer
            .materialize(&plan(vec![0, 1, 2, 3]), &path)
            .unwrap();
        let object = open_file(&path).unwrap();
        assert_eq!(
            object
                .element(tags::SYNTHETIC_DATA)
                .unwrap()
                .to_str()
                .unwrap(),
            "YES"
        );
        assert_eq!(
            object
                .element(tags::PIXEL_DATA)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            &[0, 1, 2, 3]
        );
        assert_eq!(
            object
                .element(tags::REFERENCED_SERIES_SEQUENCE)
                .unwrap()
                .items()
                .unwrap()
                .len(),
            1
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn materializes_multi_valued_attribute_tags_without_string_coercion() {
        let value = multi(
            &[
                PrimitiveValue::Tag(AttributeAddress::from_normalized_tag("0054,0010").unwrap()),
                PrimitiveValue::Tag(AttributeAddress::from_normalized_tag("0054,0020").unwrap()),
            ],
            DicomVr::AT,
        )
        .unwrap();
        assert_eq!(
            value,
            DicomPrimitiveValue::Tags(
                vec![
                    AttributeAddress::from_normalized_tag("0054,0010")
                        .unwrap()
                        .tag(),
                    AttributeAddress::from_normalized_tag("0054,0020")
                        .unwrap()
                        .tag(),
                ]
                .into()
            )
        );
    }

    #[test]
    fn rejects_content_hash_drift_before_writing() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-materializer-bad-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut plan = plan(vec![0, 1]);
        plan.content[0].sha256 = "0".repeat(64);
        assert!(matches!(
            Part10Materializer.materialize(&plan, &path),
            Err(MaterializeError::ContentHash { .. })
        ));
        assert!(!path.exists());
    }
}
