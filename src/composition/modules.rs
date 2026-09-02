use std::collections::BTreeSet;

use super::{
    AttributeAddress, AttributeOperation, AttributeValue, CompositionUidRole, DicomVr,
    IdentityPlan, PrimitiveValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePlan {
    pub name: &'static str,
    pub operations: Vec<AttributeOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonModulePlans {
    pub patient: ModulePlan,
    pub study: ModulePlan,
    pub series: ModulePlan,
    pub frame_of_reference: Option<ModulePlan>,
    pub equipment: ModulePlan,
    pub general_image: ModulePlan,
    pub content_date_time: ModulePlan,
}

impl CommonModulePlans {
    pub fn synthetic(
        modality: &str,
        identities: &IdentityPlan,
        include_frame_of_reference: bool,
    ) -> Result<Self, ModuleError> {
        let uid = |role| {
            identities
                .get(role, 0)
                .map(str::to_string)
                .ok_or_else(|| ModuleError::MissingIdentity(role.as_str().into()))
        };
        let frame_of_reference = if include_frame_of_reference {
            Some(ModulePlan {
                name: "Frame of Reference",
                operations: vec![
                    set_string(
                        "0020,0052",
                        DicomVr::UI,
                        uid(&CompositionUidRole::FrameOfReference)?,
                    ),
                    empty("0020,1040"),
                ],
            })
        } else {
            None
        };
        Ok(Self {
            patient: ModulePlan {
                name: "Patient",
                operations: vec![
                    set_string("0008,0005", DicomVr::CS, "ISO_IR 192"),
                    set_string("0010,0010", DicomVr::PN, "DTS^Synthetic"),
                    set_string("0010,0020", DicomVr::LO, "DTS-COMPOSE"),
                    empty("0010,0030"),
                    empty("0010,0040"),
                ],
            },
            study: ModulePlan {
                name: "General Study",
                operations: vec![
                    set_string("0008,0020", DicomVr::DA, "20000101"),
                    set_string("0008,0030", DicomVr::TM, "000000"),
                    empty("0008,0050"),
                    empty("0008,0090"),
                    set_string(
                        "0020,000D",
                        DicomVr::UI,
                        uid(&CompositionUidRole::StudyInstance)?,
                    ),
                    set_string("0020,0010", DicomVr::SH, "DTS-STUDY"),
                ],
            },
            series: ModulePlan {
                name: "General Series",
                operations: vec![
                    set_string("0008,0060", DicomVr::CS, modality),
                    set_string(
                        "0020,000E",
                        DicomVr::UI,
                        uid(&CompositionUidRole::SeriesInstance)?,
                    ),
                    set_string("0020,0011", DicomVr::IS, "1"),
                ],
            },
            frame_of_reference,
            equipment: ModulePlan {
                name: "General Equipment",
                operations: vec![
                    set_string("0008,0070", DicomVr::LO, "OpenAI"),
                    set_string("0008,1090", DicomVr::LO, "DICOM Test Suite"),
                    set_string("0018,1020", DicomVr::LO, crate::BYTE_STABLE_OUTPUT_VERSION),
                ],
            },
            general_image: ModulePlan {
                name: "General Image",
                operations: vec![
                    set_string("0020,0013", DicomVr::IS, "1"),
                    empty("0020,0020"),
                ],
            },
            content_date_time: ModulePlan {
                name: "Content Date and Time",
                operations: vec![
                    set_string("0008,0023", DicomVr::DA, "20000101"),
                    set_string("0008,0033", DicomVr::TM, "000000"),
                ],
            },
        })
    }

    pub fn operations(&self) -> Vec<AttributeOperation> {
        let mut operations = Vec::new();
        for module in [
            Some(&self.patient),
            Some(&self.study),
            Some(&self.series),
            self.frame_of_reference.as_ref(),
            Some(&self.equipment),
            Some(&self.general_image),
            Some(&self.content_date_time),
        ]
        .into_iter()
        .flatten()
        {
            operations.extend(module.operations.clone());
        }
        operations
    }

    pub fn validate_unique(&self) -> Result<(), ModuleError> {
        let mut tags = BTreeSet::new();
        for operation in self.operations() {
            operation.validate_trusted()?;
            let tag = operation.address().normalized_tag();
            if !tags.insert(tag.clone()) {
                return Err(ModuleError::DuplicateAttribute(tag));
            }
        }
        Ok(())
    }
}

pub fn sop_common_operations(
    sop_class_uid: &str,
    identities: &IdentityPlan,
) -> Result<Vec<AttributeOperation>, ModuleError> {
    let sop_instance_uid = identities
        .get(&CompositionUidRole::SopInstance, 0)
        .ok_or_else(|| ModuleError::MissingIdentity("sop_instance_uid".into()))?;
    Ok(vec![
        set_string("0008,0016", DicomVr::UI, sop_class_uid),
        set_string("0008,0018", DicomVr::UI, sop_instance_uid),
    ])
}

fn set_string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).expect("module tag is valid"),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn empty(tag: &str) -> AttributeOperation {
    AttributeOperation::Empty {
        address: AttributeAddress::from_normalized_tag(tag).expect("module tag is valid"),
    }
}

#[derive(Debug)]
pub enum ModuleError {
    MissingIdentity(String),
    DuplicateAttribute(String),
    Attribute(super::AttributeError),
}

impl From<super::AttributeError> for ModuleError {
    fn from(error: super::AttributeError) -> Self {
        Self::Attribute(error)
    }
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ModuleError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{IdentityAllocator, TemplateId};

    #[test]
    fn shared_modules_are_typed_unique_and_non_phi() {
        let template_id = TemplateId("classic/ct".into());
        let identities = IdentityAllocator::new(
            "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559",
            template_id,
            "1.0.0".parse().unwrap(),
            1,
        )
        .unwrap()
        .allocate_plan(
            "primary",
            [
                (CompositionUidRole::StudyInstance, 0),
                (CompositionUidRole::SeriesInstance, 0),
                (CompositionUidRole::SopInstance, 0),
                (CompositionUidRole::FrameOfReference, 0),
            ],
        )
        .unwrap();
        let modules = CommonModulePlans::synthetic("CT", &identities, true).unwrap();
        modules.validate_unique().unwrap();
        assert_eq!(modules.patient.name, "Patient");
        assert!(modules.frame_of_reference.is_some());
        let serialized = serde_json::to_string(&modules.operations()).unwrap();
        assert!(serialized.contains("DTS^Synthetic"));
        assert!(!serialized.to_ascii_lowercase().contains("birthdate"));
    }
}
