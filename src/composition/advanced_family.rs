use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use dicom_object::open_file;

use super::{
    AttributeOperation, AttributeValue, ContentSource, IdentityPlan, LocalContentResolver,
    PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan, SpecInstance, TemplateDescriptor,
    ValueOrigin, resolve_raw_native_pixels,
};
use crate::generator::write_composition_default_artifacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedFamilyKind {
    EnhancedCt,
    EnhancedMr,
    EnhancedPet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedFamilyProfile {
    pub kind: AdvancedFamilyKind,
    pub include_frame_of_reference: bool,
}

impl AdvancedFamilyProfile {
    pub fn for_template(template_id: &str) -> Option<Self> {
        let kind = match template_id {
            "enhanced/ct" => AdvancedFamilyKind::EnhancedCt,
            "enhanced/mr" => AdvancedFamilyKind::EnhancedMr,
            "enhanced/pet" => AdvancedFamilyKind::EnhancedPet,
            _ => return None,
        };
        Some(Self {
            kind,
            include_frame_of_reference: true,
        })
    }

    pub fn resolve_plan(
        &self,
        instance: &SpecInstance,
        template: &TemplateDescriptor,
        identities: IdentityPlan,
        seed: u64,
        private_root: &Path,
        content_resolver: &mut LocalContentResolver,
    ) -> Result<ResolvedInstancePlan, AdvancedFamilyError> {
        if instance.parameters.contains_key("variant") {
            return Err(AdvancedFamilyError::InvalidVariant(
                instance.instance_id.clone(),
            ));
        }
        let artifacts =
            write_composition_default_artifacts(private_root, seed, &template.template_id.0, None)
                .map_err(|error| AdvancedFamilyError::DefaultArtifact(error.to_string()))?;
        if artifacts.len() != 1 {
            return Err(AdvancedFamilyError::DefaultArtifact(format!(
                "{} emitted {} artifacts where one instance was required",
                template.template_id,
                artifacts.len()
            )));
        }
        let artifact = open_file(&artifacts[0].path)
            .map_err(|error| AdvancedFamilyError::DefaultArtifact(error.to_string()))?;
        let transfer_syntax_uid = artifact.meta().transfer_syntax().to_string();
        if transfer_syntax_uid != template.transfer_syntaxes[0].uid {
            return Err(AdvancedFamilyError::DefaultArtifact(
                "qualified default transfer syntax differs from the descriptor".into(),
            ));
        }
        let object = artifact.into_inner();
        let sop_instance_uid = identities
            .get(&super::CompositionUidRole::SopInstance, 0)
            .ok_or(AdvancedFamilyError::MissingIdentity("sop_instance"))?;
        let implementation_class_uid = identities
            .get(&super::CompositionUidRole::ImplementationClass, 0)
            .ok_or(AdvancedFamilyError::MissingIdentity("implementation_class"))?;
        let study_instance_uid = identities
            .get(&super::CompositionUidRole::StudyInstance, 0)
            .ok_or(AdvancedFamilyError::MissingIdentity("study"))?;
        let series_instance_uid = identities
            .get(&super::CompositionUidRole::SeriesInstance, 0)
            .ok_or(AdvancedFamilyError::MissingIdentity("series"))?;
        let mut plan = super::resolved_plan_from_curated_dataset(
            &object,
            super::CuratedPlanInput {
                instance_id: &instance.instance_id,
                template_id: template.template_id.clone(),
                template_version: template.template_version,
                sop_class_uid: &template.sop_class_uid,
                transfer_syntax_uid: &transfer_syntax_uid,
                study_instance_uid: Some(study_instance_uid),
                series_instance_uid: Some(series_instance_uid),
                sop_instance_uid,
                implementation_class_uid,
            },
        )
        .map_err(|error| AdvancedFamilyError::DefaultArtifact(error.to_string()))?;
        plan.identities = identities;
        rewrite_plan_identities(&mut plan)?;
        if self.kind == AdvancedFamilyKind::EnhancedCt {
            normalize_enhanced_ct_defined_terms(&mut plan);
        }
        validate_multiframe_structure(&plan)?;
        apply_caller_content(instance, &mut plan, content_resolver)?;
        apply_overrides(instance, &mut plan)?;
        Ok(plan)
    }
}

fn normalize_enhanced_ct_defined_terms(plan: &mut ResolvedInstancePlan) {
    fn normalize(value: &mut AttributeValue) {
        let normalize_primitive = |primitive: &mut PrimitiveValue| {
            if let PrimitiveValue::String(value) = primitive {
                match value.trim() {
                    "AXIAL" => *value = "VOLUME".into(),
                    "SRT" => *value = "SCT".into(),
                    _ => {}
                }
            }
        };
        match value {
            AttributeValue::Primitive(value) => normalize_primitive(value),
            AttributeValue::Multi(values) => values.iter_mut().for_each(normalize_primitive),
            AttributeValue::Sequence(items) => {
                for item in items {
                    for operation in &mut item.attributes {
                        if let AttributeOperation::Set { value, .. } = operation {
                            normalize(value);
                        }
                    }
                }
            }
            AttributeValue::Binary(_) => {}
        }
    }
    for attribute in &mut plan.attributes {
        if let Some(value) = &mut attribute.value {
            normalize(value);
        }
    }
}

fn rewrite_plan_identities(plan: &mut ResolvedInstancePlan) -> Result<(), AdvancedFamilyError> {
    let roles = [
        ("0008,0018", super::CompositionUidRole::SopInstance),
        ("0020,000D", super::CompositionUidRole::StudyInstance),
        ("0020,000E", super::CompositionUidRole::SeriesInstance),
        ("0020,0052", super::CompositionUidRole::FrameOfReference),
    ];
    let mut replacements = BTreeMap::new();
    for (tag, role) in &roles {
        let Some(new) = plan.identities.get(role, 0) else {
            continue;
        };
        let old_value = plan
            .attributes
            .iter()
            .find(|attribute| attribute.address.normalized_tag() == *tag)
            .and_then(|attribute| attribute.value.as_ref());
        if let Some(AttributeValue::Primitive(PrimitiveValue::String(old))) = old_value {
            replacements.insert(old.clone(), new.to_string());
        }
    }
    for attribute in &mut plan.attributes {
        if let Some(value) = &mut attribute.value {
            rewrite_uids(value, &replacements);
        }
    }
    for (tag, role) in &roles {
        let Some(value) = plan.identities.get(role, 0) else {
            continue;
        };
        if let Some(attribute) = plan
            .attributes
            .iter_mut()
            .find(|attribute| attribute.address.normalized_tag() == *tag)
        {
            attribute.value = Some(AttributeValue::Primitive(PrimitiveValue::String(
                value.to_string(),
            )));
            attribute.origin = ValueOrigin::DerivedStructural;
        }
    }
    Ok(())
}

fn rewrite_uids(value: &mut AttributeValue, replacements: &BTreeMap<String, String>) {
    let rewrite = |primitive: &mut PrimitiveValue| {
        if let PrimitiveValue::String(value) = primitive {
            if let Some(replacement) = replacements.get(value) {
                *value = replacement.clone();
            }
        }
    };
    match value {
        AttributeValue::Primitive(primitive) => rewrite(primitive),
        AttributeValue::Multi(values) => values.iter_mut().for_each(rewrite),
        AttributeValue::Sequence(items) => {
            for item in items {
                for operation in &mut item.attributes {
                    if let AttributeOperation::Set { value, .. } = operation {
                        rewrite_uids(value, replacements);
                    }
                }
            }
        }
        AttributeValue::Binary(_) => {}
    }
}

fn validate_multiframe_structure(plan: &ResolvedInstancePlan) -> Result<(), AdvancedFamilyError> {
    let frames = numeric_attribute(plan, "0028,0008")?;
    let shared = sequence_attribute(plan, "5200,9229")?;
    let per_frame = sequence_attribute(plan, "5200,9230")?;
    let organizations = sequence_attribute(plan, "0020,9221")?;
    let indices = sequence_attribute(plan, "0020,9222")?;
    if frames == 0
        || shared.len() != 1
        || per_frame.len() != frames as usize
        || organizations.is_empty()
        || indices.is_empty()
    {
        return Err(AdvancedFamilyError::FunctionalGroupCardinality {
            frames,
            shared: shared.len(),
            per_frame: per_frame.len(),
            organizations: organizations.len(),
            indices: indices.len(),
        });
    }
    for (frame_index, item) in per_frame.iter().enumerate() {
        let frame_content = item
            .attributes
            .iter()
            .find_map(|operation| match operation {
                AttributeOperation::Set {
                    address,
                    value: AttributeValue::Sequence(items),
                    ..
                } if address.normalized_tag() == "0020,9111" => Some(items),
                _ => None,
            });
        let Some(frame_content) = frame_content else {
            return Err(AdvancedFamilyError::MissingFrameContent(
                frame_index as u32 + 1,
            ));
        };
        if frame_content.len() != 1 {
            return Err(AdvancedFamilyError::MissingFrameContent(
                frame_index as u32 + 1,
            ));
        }
        let values = frame_content[0]
            .attributes
            .iter()
            .find_map(|operation| match operation {
                AttributeOperation::Set {
                    address,
                    value: AttributeValue::Multi(values),
                    ..
                } if address.normalized_tag() == "0020,9157" => Some(values.len()),
                AttributeOperation::Set {
                    address,
                    value: AttributeValue::Primitive(_),
                    ..
                } if address.normalized_tag() == "0020,9157" => Some(1),
                _ => None,
            });
        if values != Some(indices.len()) {
            return Err(AdvancedFamilyError::DimensionCardinality {
                frame: frame_index as u32 + 1,
                expected: indices.len(),
                actual: values.unwrap_or(0),
            });
        }
    }
    Ok(())
}

fn apply_caller_content(
    instance: &SpecInstance,
    plan: &mut ResolvedInstancePlan,
    resolver: &mut LocalContentResolver,
) -> Result<(), AdvancedFamilyError> {
    if instance.content.is_empty() || matches!(instance.content[0].source, ContentSource::Default) {
        if instance.content.len() <= 1 {
            return Ok(());
        }
    }
    if instance.content.len() != 1 || instance.content[0].slot != "pixels" {
        return Err(AdvancedFamilyError::ContentCardinality(
            instance.instance_id.clone(),
        ));
    }
    let ContentSource::LocalFile {
        path,
        sha256,
        pixel: Some(declaration),
        ..
    } = &instance.content[0].source
    else {
        return Err(AdvancedFamilyError::UnsupportedContent(
            instance.instance_id.clone(),
        ));
    };
    let declared = declaration
        .shape()
        .map_err(|error| AdvancedFamilyError::UnsupportedContent(error.to_string()))?;
    let expected = pixel_shape(plan)?;
    if declared != expected {
        return Err(AdvancedFamilyError::PixelShapeMismatch {
            instance_id: instance.instance_id.clone(),
            expected: format!("{expected:?}"),
            actual: format!("{declared:?}"),
        });
    }
    let mut output = resolve_raw_native_pixels(resolver, path, sha256.as_deref(), declared)
        .map_err(|error| AdvancedFamilyError::UnsupportedContent(error.to_string()))?;
    output.content.slot = "pixels".into();
    plan.content = vec![output.content];
    Ok(())
}

fn pixel_shape(plan: &ResolvedInstancePlan) -> Result<super::PixelShape, AdvancedFamilyError> {
    let photometric = string_attribute(plan, "0028,0004")?;
    Ok(super::PixelShape {
        rows: numeric_attribute(plan, "0028,0010")?,
        columns: numeric_attribute(plan, "0028,0011")?,
        frames: numeric_attribute(plan, "0028,0008")?,
        samples_per_pixel: numeric_attribute(plan, "0028,0002")? as u8,
        photometric_interpretation: match photometric.as_str() {
            "MONOCHROME1" => super::PhotometricInterpretation::Monochrome1,
            "MONOCHROME2" => super::PhotometricInterpretation::Monochrome2,
            "RGB" => super::PhotometricInterpretation::Rgb,
            other => return Err(AdvancedFamilyError::UnsupportedPhotometric(other.into())),
        },
        sample_type: if numeric_attribute(plan, "0028,0103")? == 0 {
            super::SampleType::UnsignedInteger
        } else {
            super::SampleType::SignedInteger
        },
        bits_allocated: numeric_attribute(plan, "0028,0100")? as u8,
        bits_stored: numeric_attribute(plan, "0028,0101")? as u8,
        high_bit: numeric_attribute(plan, "0028,0102")? as u8,
        byte_order: super::ByteOrder::Little,
        planar_configuration: plan
            .attributes
            .iter()
            .find(|attribute| attribute.address.normalized_tag() == "0028,0006")
            .map(|_| numeric_attribute(plan, "0028,0006"))
            .transpose()?
            .map(|value| {
                if value == 0 {
                    super::PlanarConfiguration::Interleaved
                } else {
                    super::PlanarConfiguration::Planar
                }
            }),
    })
}

fn numeric_attribute(plan: &ResolvedInstancePlan, tag: &str) -> Result<u32, AdvancedFamilyError> {
    let value = attribute_value(plan, tag)?;
    match value {
        AttributeValue::Primitive(PrimitiveValue::Unsigned(value)) => u32::try_from(*value).ok(),
        AttributeValue::Primitive(PrimitiveValue::Signed(value)) => u32::try_from(*value).ok(),
        AttributeValue::Primitive(PrimitiveValue::String(value)) => value.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(|| AdvancedFamilyError::InvalidAttribute(format!("{tag} {value:?}")))
}

fn string_attribute(plan: &ResolvedInstancePlan, tag: &str) -> Result<String, AdvancedFamilyError> {
    match attribute_value(plan, tag)? {
        AttributeValue::Primitive(PrimitiveValue::String(value)) => Ok(value.trim().to_string()),
        _ => Err(AdvancedFamilyError::InvalidAttribute(tag.into())),
    }
}

fn sequence_attribute<'a>(
    plan: &'a ResolvedInstancePlan,
    tag: &str,
) -> Result<&'a [super::AttributeItem], AdvancedFamilyError> {
    match attribute_value(plan, tag)? {
        AttributeValue::Sequence(items) => Ok(items),
        _ => Err(AdvancedFamilyError::InvalidAttribute(tag.into())),
    }
}

fn attribute_value<'a>(
    plan: &'a ResolvedInstancePlan,
    tag: &str,
) -> Result<&'a AttributeValue, AdvancedFamilyError> {
    plan.attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == tag)
        .and_then(|attribute| attribute.value.as_ref())
        .ok_or_else(|| AdvancedFamilyError::MissingAttribute(tag.into()))
}

fn apply_overrides(
    instance: &SpecInstance,
    plan: &mut ResolvedInstancePlan,
) -> Result<(), AdvancedFamilyError> {
    let protected = [
        "0008,0016",
        "0008,0018",
        "0020,000D",
        "0020,000E",
        "0020,0052",
        "0028,0002",
        "0028,0004",
        "0028,0006",
        "0028,0008",
        "0028,0010",
        "0028,0011",
        "0028,0100",
        "0028,0101",
        "0028,0102",
        "0028,0103",
        "0020,9221",
        "0020,9222",
        "5200,9229",
        "5200,9230",
        "7FE0,0010",
    ];
    for operation in instance
        .typed_attributes()
        .map_err(|error| AdvancedFamilyError::Override(error.to_string()))?
    {
        operation
            .validate()
            .map_err(|error| AdvancedFamilyError::Override(error.to_string()))?;
        let tag = operation.address().normalized_tag();
        if protected.contains(&tag.as_str()) {
            return Err(AdvancedFamilyError::ProtectedOverride(tag));
        }
        match operation {
            AttributeOperation::Set { address, vr, value } => upsert(
                &mut plan.attributes,
                ResolvedAttribute {
                    address,
                    vr,
                    value: Some(value),
                    origin: ValueOrigin::InstanceOverride,
                },
            ),
            AttributeOperation::Empty { address } => {
                let Some(existing) = plan
                    .attributes
                    .iter()
                    .find(|attribute| attribute.address == address)
                else {
                    return Err(AdvancedFamilyError::Override(format!(
                        "cannot empty unknown attribute {}",
                        address.normalized_tag()
                    )));
                };
                let vr = existing.vr;
                upsert(
                    &mut plan.attributes,
                    ResolvedAttribute {
                        address,
                        vr,
                        value: None,
                        origin: ValueOrigin::InstanceOverride,
                    },
                );
            }
            AttributeOperation::Remove { address } => {
                plan.attributes
                    .retain(|attribute| attribute.address != address);
            }
        }
    }
    plan.attributes
        .sort_by(|left, right| left.address.cmp(&right.address));
    Ok(())
}

fn upsert(attributes: &mut Vec<ResolvedAttribute>, replacement: ResolvedAttribute) {
    if let Some(existing) = attributes
        .iter_mut()
        .find(|attribute| attribute.address == replacement.address)
    {
        *existing = replacement;
    } else {
        attributes.push(replacement);
    }
}

#[derive(Debug)]
pub enum AdvancedFamilyError {
    DefaultArtifact(String),
    InvalidVariant(String),
    MissingIdentity(&'static str),
    MissingAttribute(String),
    InvalidAttribute(String),
    FunctionalGroupCardinality {
        frames: u32,
        shared: usize,
        per_frame: usize,
        organizations: usize,
        indices: usize,
    },
    MissingFrameContent(u32),
    DimensionCardinality {
        frame: u32,
        expected: usize,
        actual: usize,
    },
    ContentCardinality(String),
    UnsupportedContent(String),
    PixelShapeMismatch {
        instance_id: String,
        expected: String,
        actual: String,
    },
    UnsupportedPhotometric(String),
    Override(String),
    ProtectedOverride(String),
}

impl fmt::Display for AdvancedFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "advanced family composition failed: {self:?}")
    }
}

impl std::error::Error for AdvancedFamilyError {}
