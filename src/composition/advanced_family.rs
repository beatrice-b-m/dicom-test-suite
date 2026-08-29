use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use dicom_object::open_file;

use super::{
    AttributeOperation, AttributeValue, BulkDataBounds, BulkDataPlan, BulkDataSource,
    ContentSource, DoubleFloatPixelDataSlot, EncapsulatedDocumentSlot, FloatPixelDataSlot,
    IdentityPlan, LocalContentResolver, MeshSlot, PixelDataSlot, PrimitiveValue, ResolvedAttribute,
    ResolvedInstancePlan, SequenceItemPlacement, SpecInstance, TemplateDescriptor, ValueOrigin,
    WaveformSamplesSlot, resolve_raw_native_pixels,
};
use crate::generator::write_composition_default_artifacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedFamilyKind {
    EnhancedCt,
    EnhancedMr,
    EnhancedPet,
    WholeSlide,
    DerivedReference,
    TypedBulk(TypedBulkFamily),
    Quantitative(QuantitativeFamily),
    StructuredReport(StructuredReportFamily),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedBulkFamily {
    TwelveLeadEcg,
    GeneralEcg,
    EncapsulatedPdf,
    EncapsulatedStl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantitativeFamily {
    Segmentation,
    ParametricMap,
    RealWorldValueMapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredReportFamily {
    BasicText,
    Comprehensive,
    Comprehensive3d,
    Tid1500,
    KeyObject,
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
            "enhanced/ct/concatenation-part-1" | "enhanced/ct/concatenation-part-2" => {
                AdvancedFamilyKind::EnhancedCt
            }
            "enhanced/mr" => AdvancedFamilyKind::EnhancedMr,
            "enhanced/pet" => AdvancedFamilyKind::EnhancedPet,
            "vl/wsi/tiled-full"
            | "vl/wsi/tiled-sparse"
            | "vl/wsi/multiple-optical-paths"
            | "vl/wsi/pyramid-volume"
            | "vl/wsi/pyramid-thumbnail"
            | "vl/wsi/pyramid-label" => AdvancedFamilyKind::WholeSlide,
            "derived/registration/spatial"
            | "derived/registration/deformable"
            | "derived/presentation-state/grayscale"
            | "derived/presentation-state/color"
            | "derived/presentation-state/blending"
            | "derived/presentation-state/advanced-blending" => {
                AdvancedFamilyKind::DerivedReference
            }
            "non-image/waveform/twelve-lead-ecg" => {
                AdvancedFamilyKind::TypedBulk(TypedBulkFamily::TwelveLeadEcg)
            }
            "non-image/waveform/general-ecg" => {
                AdvancedFamilyKind::TypedBulk(TypedBulkFamily::GeneralEcg)
            }
            "non-image/encapsulated-document/pdf" => {
                AdvancedFamilyKind::TypedBulk(TypedBulkFamily::EncapsulatedPdf)
            }
            "non-image/mesh/stl" => AdvancedFamilyKind::TypedBulk(TypedBulkFamily::EncapsulatedStl),
            "derived/segmentation/binary"
            | "derived/segmentation/fractional-probability"
            | "derived/segmentation/labelmap"
            | "derived/segmentation/wsi-tile" => {
                AdvancedFamilyKind::Quantitative(QuantitativeFamily::Segmentation)
            }
            "derived/parametric-map/float32" | "derived/parametric-map/float64" => {
                AdvancedFamilyKind::Quantitative(QuantitativeFamily::ParametricMap)
            }
            "derived/real-world-value-mapping/linear" => {
                AdvancedFamilyKind::Quantitative(QuantitativeFamily::RealWorldValueMapping)
            }
            "derived/structured-report/basic-text" => {
                AdvancedFamilyKind::StructuredReport(StructuredReportFamily::BasicText)
            }
            "derived/structured-report/comprehensive" => {
                AdvancedFamilyKind::StructuredReport(StructuredReportFamily::Comprehensive)
            }
            "derived/structured-report/comprehensive-3d" => {
                AdvancedFamilyKind::StructuredReport(StructuredReportFamily::Comprehensive3d)
            }
            "derived/structured-report/tid1500" => {
                AdvancedFamilyKind::StructuredReport(StructuredReportFamily::Tid1500)
            }
            "derived/structured-report/key-object" => {
                AdvancedFamilyKind::StructuredReport(StructuredReportFamily::KeyObject)
            }
            _ => return None,
        };
        Some(Self {
            kind,
            include_frame_of_reference: !matches!(
                kind,
                AdvancedFamilyKind::TypedBulk(
                    TypedBulkFamily::TwelveLeadEcg
                        | TypedBulkFamily::GeneralEcg
                        | TypedBulkFamily::EncapsulatedPdf
                )
            ),
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
            if template
                .template_id
                .0
                .starts_with("enhanced/ct/concatenation-")
            {
                qualify_enhanced_ct_concatenation(&mut plan)?;
            }
        }
        normalize_derived_reference_defaults(&template.template_id.0, &mut plan)?;
        if let AdvancedFamilyKind::TypedBulk(family) = self.kind {
            extract_typed_bulk_defaults(family, &mut plan)?;
        } else if let AdvancedFamilyKind::Quantitative(family) = self.kind {
            normalize_quantitative_content(family, &mut plan)?;
        } else if let AdvancedFamilyKind::StructuredReport(_) = self.kind {
            if !plan.content.is_empty() {
                return Err(AdvancedFamilyError::TypedBulk(
                    "structured reports cannot carry bulk content".into(),
                ));
            }
        } else if self.kind == AdvancedFamilyKind::WholeSlide {
            validate_wsi_structure(&plan)?;
        } else if self.kind != AdvancedFamilyKind::DerivedReference {
            validate_multiframe_structure(&plan)?;
        }
        if self.kind == AdvancedFamilyKind::DerivedReference {
            if !instance.content.is_empty() {
                return Err(AdvancedFamilyError::UnsupportedContent(
                    instance.instance_id.clone(),
                ));
            }
        } else if let AdvancedFamilyKind::TypedBulk(family) = self.kind {
            apply_typed_bulk_content(family, instance, &mut plan, content_resolver)?;
        } else if let AdvancedFamilyKind::Quantitative(family) = self.kind {
            apply_quantitative_content(family, instance, &mut plan, content_resolver)?;
        } else if let AdvancedFamilyKind::StructuredReport(family) = self.kind {
            if !instance.content.is_empty() {
                return Err(AdvancedFamilyError::UnsupportedContent(
                    instance.instance_id.clone(),
                ));
            }
            apply_structured_report_parameters(family, instance, &mut plan)?;
        } else {
            apply_caller_content(instance, &mut plan, content_resolver)?;
        }
        apply_overrides(instance, &mut plan)?;
        Ok(plan)
    }
}

fn normalize_derived_reference_defaults(
    template_id: &str,
    plan: &mut ResolvedInstancePlan,
) -> Result<(), AdvancedFamilyError> {
    if template_id == "derived/presentation-state/grayscale" {
        plan.attributes.retain(|attribute| {
            !matches!(
                attribute.address.normalized_tag().as_str(),
                "0008,0023" | "0008,0033"
            )
        });
        upsert(
            &mut plan.attributes,
            ResolvedAttribute {
                address: super::AttributeAddress::from_normalized_tag("0020,0060")
                    .map_err(|error| AdvancedFamilyError::DefaultArtifact(error.to_string()))?,
                vr: super::DicomVr::CS,
                value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                    "R".into(),
                ))),
                origin: ValueOrigin::TemplateDefault,
            },
        );
    }
    if template_id == "derived/presentation-state/advanced-blending" {
        plan.attributes.retain(|attribute| {
            !matches!(
                attribute.address.normalized_tag().as_str(),
                "0020,0052" | "0020,1040"
            )
        });
    }
    plan.attributes
        .sort_by(|left, right| left.address.cmp(&right.address));
    Ok(())
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

fn qualify_enhanced_ct_concatenation(
    plan: &mut ResolvedInstancePlan,
) -> Result<(), AdvancedFamilyError> {
    for (tag, vr, value) in [
        ("0018,5100", super::DicomVr::CS, None),
        ("0018,9004", super::DicomVr::CS, Some("RESEARCH")),
        ("0028,0301", super::DicomVr::CS, Some("NO")),
        ("0028,2110", super::DicomVr::CS, Some("00")),
        ("2050,0020", super::DicomVr::CS, Some("IDENTITY")),
    ] {
        let address = super::AttributeAddress::from_normalized_tag(tag)
            .map_err(|error| AdvancedFamilyError::DefaultArtifact(error.to_string()))?;
        upsert(
            &mut plan.attributes,
            ResolvedAttribute {
                address,
                vr,
                value: value.map(|value| {
                    AttributeValue::Primitive(PrimitiveValue::String(value.to_string()))
                }),
                origin: ValueOrigin::TemplateDefault,
            },
        );
    }
    plan.attributes
        .sort_by(|left, right| left.address.cmp(&right.address));
    Ok(())
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

pub(crate) fn validate_concatenation_closure(
    plans: &[ResolvedInstancePlan],
    members: &BTreeMap<String, super::BundleMemberProvenance>,
) -> Result<(), AdvancedFamilyError> {
    let mut groups = BTreeMap::<String, Vec<&ResolvedInstancePlan>>::new();
    for plan in plans {
        if plan
            .attributes
            .iter()
            .any(|attribute| attribute.address.normalized_tag() == "0020,9161")
        {
            let bundle_root = &members
                .get(&plan.instance_id)
                .ok_or_else(|| {
                    AdvancedFamilyError::ConcatenationClosure(format!(
                        "{} has no bundle provenance",
                        plan.instance_id
                    ))
                })?
                .bundle_root_instance_id;
            groups.entry(bundle_root.clone()).or_default().push(plan);
        }
    }
    for (root, group) in groups {
        let summaries = group
            .into_iter()
            .map(|plan| {
                Ok(ConcatenationPartSummary {
                    instance_id: plan.instance_id.clone(),
                    concatenation_uid: string_attribute(plan, "0020,9161")?,
                    source_uid: string_attribute(plan, "0020,0242")?,
                    number: numeric_attribute(plan, "0020,9162")?,
                    total: numeric_attribute(plan, "0020,9163")?,
                    offset: numeric_attribute(plan, "0020,9228")?,
                    frames: numeric_attribute(plan, "0028,0008")?,
                })
            })
            .collect::<Result<Vec<_>, AdvancedFamilyError>>()?;
        validate_concatenation_summaries(&root, summaries)?;
    }
    Ok(())
}

pub(crate) fn rewrite_materialized_dicom_references(
    plans: &mut [ResolvedInstancePlan],
) -> Result<(), AdvancedFamilyError> {
    #[derive(Clone)]
    struct TargetIdentity {
        sop: String,
        series: Option<String>,
        study: Option<String>,
        frame_of_reference: Option<String>,
    }
    let targets = plans
        .iter()
        .map(|plan| {
            (
                plan.instance_id.clone(),
                TargetIdentity {
                    sop: plan
                        .identities
                        .get(&super::CompositionUidRole::SopInstance, 0)
                        .expect("resolved plans allocate SOP identity")
                        .to_string(),
                    series: plan
                        .identities
                        .get(&super::CompositionUidRole::SeriesInstance, 0)
                        .map(str::to_string),
                    study: plan
                        .identities
                        .get(&super::CompositionUidRole::StudyInstance, 0)
                        .map(str::to_string),
                    frame_of_reference: plan
                        .identities
                        .get(&super::CompositionUidRole::FrameOfReference, 0)
                        .map(str::to_string),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for plan in plans {
        if plan.references.is_empty()
            || !(plan.template_id.0.starts_with("derived/registration/")
                || plan
                    .template_id
                    .0
                    .starts_with("derived/presentation-state/")
                || plan.template_id.0.starts_with("derived/segmentation/")
                || plan.template_id.0.starts_with("derived/parametric-map/")
                || plan
                    .template_id
                    .0
                    .starts_with("derived/real-world-value-mapping/")
                || plan.template_id.0.starts_with("derived/structured-report/"))
        {
            continue;
        }
        let ordered = plan
            .references
            .iter()
            .map(|reference| {
                targets.get(&reference.target_instance_id).ok_or_else(|| {
                    AdvancedFamilyError::DicomReferenceClosure(format!(
                        "{} targets unknown {}",
                        plan.instance_id, reference.target_instance_id
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame_replacements = plan
            .references
            .iter()
            .map(|reference| {
                (!reference.referenced_frames.is_empty()).then(|| {
                    reference
                        .referenced_frames
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join("\\")
                })
            })
            .collect::<Vec<_>>();
        rewrite_nested_identity_axis(
            plan,
            "0008,1155",
            ordered.iter().map(|identity| Some(identity.sop.clone())),
            true,
        )?;
        rewrite_nested_identity_axis(
            plan,
            "0020,000E",
            ordered.iter().map(|identity| identity.series.clone()),
            false,
        )?;
        if frame_replacements.iter().any(Option::is_some) {
            rewrite_nested_identity_axis(plan, "0008,1160", frame_replacements, false)?;
        }
        rewrite_nested_identity_axis(
            plan,
            "0020,000D",
            ordered.iter().map(|identity| identity.study.clone()),
            false,
        )?;
        rewrite_nested_identity_axis(
            plan,
            "0020,0052",
            ordered
                .iter()
                .map(|identity| identity.frame_of_reference.clone()),
            false,
        )?;
    }
    Ok(())
}

fn rewrite_nested_identity_axis(
    plan: &mut ResolvedInstancePlan,
    tag: &str,
    replacements: impl IntoIterator<Item = Option<String>>,
    required: bool,
) -> Result<(), AdvancedFamilyError> {
    let mut old = Vec::new();
    for attribute in &plan.attributes {
        if let Some(value) = &attribute.value {
            collect_nested_tag_strings(value, tag, &mut old);
        }
    }
    old.dedup();
    let mut new = replacements.into_iter().flatten().collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    new.retain(|value| seen.insert(value.clone()));
    if old.is_empty() {
        if required {
            return Err(AdvancedFamilyError::DicomReferenceClosure(format!(
                "{} has logical references but no nested {tag}",
                plan.instance_id
            )));
        }
        return Ok(());
    }
    if tag == "0008,1160" && new.len() == 1 {
        let replacement = new.pop().expect("one frame replacement");
        let mapping = old
            .into_iter()
            .map(|value| (value, replacement.clone()))
            .collect::<BTreeMap<_, _>>();
        for attribute in &mut plan.attributes {
            if let Some(value) = &mut attribute.value {
                rewrite_nested_tag_strings(value, tag, &mapping);
            }
        }
        return Ok(());
    }
    if old.len() != new.len() {
        return Err(AdvancedFamilyError::DicomReferenceClosure(format!(
            "{} {tag} identity cardinality differs: DICOM={}, graph={}",
            plan.instance_id,
            old.len(),
            new.len()
        )));
    }
    let mapping = old.into_iter().zip(new).collect::<BTreeMap<_, _>>();
    for attribute in &mut plan.attributes {
        if let Some(value) = &mut attribute.value {
            rewrite_nested_tag_strings(value, tag, &mapping);
        }
    }
    Ok(())
}

fn collect_nested_tag_strings(value: &AttributeValue, tag: &str, output: &mut Vec<String>) {
    let AttributeValue::Sequence(items) = value else {
        return;
    };
    for item in items {
        for operation in &item.attributes {
            if let AttributeOperation::Set {
                address,
                value: nested,
                ..
            } = operation
            {
                if address.normalized_tag() == tag {
                    collect_primitive_strings(nested, output);
                }
                collect_nested_tag_strings(nested, tag, output);
            }
        }
    }
}

fn collect_primitive_strings(value: &AttributeValue, output: &mut Vec<String>) {
    match value {
        AttributeValue::Primitive(PrimitiveValue::String(value)) => {
            let value = value.trim().to_string();
            if !output.contains(&value) {
                output.push(value);
            }
        }
        AttributeValue::Multi(values) => {
            for value in values {
                if let PrimitiveValue::String(value) = value {
                    let value = value.trim().to_string();
                    if !output.contains(&value) {
                        output.push(value);
                    }
                }
            }
        }
        _ => {}
    }
}

fn rewrite_nested_tag_strings(
    value: &mut AttributeValue,
    tag: &str,
    mapping: &BTreeMap<String, String>,
) {
    let AttributeValue::Sequence(items) = value else {
        return;
    };
    for item in items {
        for operation in &mut item.attributes {
            if let AttributeOperation::Set {
                address,
                value: nested,
                ..
            } = operation
            {
                if address.normalized_tag() == tag {
                    rewrite_primitive_strings(nested, mapping);
                }
                rewrite_nested_tag_strings(nested, tag, mapping);
            }
        }
    }
}

fn rewrite_primitive_strings(value: &mut AttributeValue, mapping: &BTreeMap<String, String>) {
    let rewrite = |value: &mut PrimitiveValue| {
        if let PrimitiveValue::String(current) = value {
            if let Some(replacement) = mapping.get(current.trim()) {
                *current = replacement.clone();
            }
        }
    };
    match value {
        AttributeValue::Primitive(value) => rewrite(value),
        AttributeValue::Multi(values) => values.iter_mut().for_each(rewrite),
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConcatenationPartSummary {
    instance_id: String,
    concatenation_uid: String,
    source_uid: String,
    number: u32,
    total: u32,
    offset: u32,
    frames: u32,
}

fn validate_concatenation_summaries(
    root: &str,
    mut group: Vec<ConcatenationPartSummary>,
) -> Result<(), AdvancedFamilyError> {
    group.sort_by_key(|part| part.number);
    let expected_total = u32::try_from(group.len()).map_err(|_| {
        AdvancedFamilyError::ConcatenationClosure(format!("{root} has too many parts"))
    })?;
    let Some(first) = group.first() else {
        return Ok(());
    };
    let concatenation_uid = &first.concatenation_uid;
    let source_uid = &first.source_uid;
    let mut expected_offset = 0_u32;
    for (index, part) in group.iter().enumerate() {
        let expected_number = index as u32 + 1;
        if part.number != expected_number
            || part.total != expected_total
            || part.offset != expected_offset
            || &part.concatenation_uid != concatenation_uid
            || &part.source_uid != source_uid
        {
            return Err(AdvancedFamilyError::ConcatenationClosure(format!(
                "{root} part {} breaks UID, numbering, total, or frame-offset continuity",
                part.instance_id
            )));
        }
        expected_offset = expected_offset.checked_add(part.frames).ok_or_else(|| {
            AdvancedFamilyError::ConcatenationClosure(format!("{root} frame offsets overflow"))
        })?;
    }
    Ok(())
}

fn validate_wsi_structure(plan: &ResolvedInstancePlan) -> Result<(), AdvancedFamilyError> {
    let frames = numeric_attribute(plan, "0028,0008")?;
    let rows = numeric_attribute(plan, "0028,0010")?;
    let columns = numeric_attribute(plan, "0028,0011")?;
    let total_rows = numeric_attribute(plan, "0048,0006")?;
    let total_columns = numeric_attribute(plan, "0048,0007")?;
    let organizations = sequence_attribute(plan, "0020,9221")?;
    let indices = optional_sequence_attribute(plan, "0020,9222")?;
    if frames == 0
        || rows == 0
        || columns == 0
        || total_rows < rows
        || total_columns < columns
        || organizations.is_empty()
    {
        return Err(AdvancedFamilyError::InvalidTiling(format!(
            "frames={frames}, tile={rows}x{columns}, matrix={total_rows}x{total_columns}, organizations={}, indices={}",
            organizations.len(),
            indices.map_or(0, <[super::AttributeItem]>::len)
        )));
    }
    let organization_type = plan
        .attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == "0020,9311")
        .map(|_| string_attribute(plan, "0020,9311"))
        .transpose()?;
    let per_frame = optional_sequence_attribute(plan, "5200,9230")?;
    if organization_type.as_deref() == Some("TILED_FULL") {
        let optical_paths =
            optional_sequence_attribute(plan, "0048,0105")?.map_or(1, |items| items.len().max(1));
        let tile_rows = total_rows.div_ceil(rows);
        let tile_columns = total_columns.div_ceil(columns);
        let expected = u32::try_from(optical_paths)
            .ok()
            .and_then(|paths| tile_rows.checked_mul(tile_columns)?.checked_mul(paths))
            .ok_or_else(|| AdvancedFamilyError::InvalidTiling("frame-count overflow".into()))?;
        if frames != expected
            || per_frame.is_some_and(|items| !items.is_empty())
            || indices.is_some_and(|items| !items.is_empty())
        {
            return Err(AdvancedFamilyError::InvalidTiling(format!(
                "TILED_FULL expected {expected} implicit frames but found {frames}"
            )));
        }
    } else {
        if indices.is_none_or(<[super::AttributeItem]>::is_empty) {
            return Err(AdvancedFamilyError::InvalidTiling(
                "sparse WSI requires dimension indices".into(),
            ));
        }
        let per_frame = per_frame.ok_or_else(|| {
            AdvancedFamilyError::InvalidTiling("sparse WSI requires per-frame positions".into())
        })?;
        if per_frame.len() != frames as usize
            || per_frame.iter().any(|item| {
                !item.attributes.iter().any(|operation| {
                    matches!(operation, AttributeOperation::Set { address, value: AttributeValue::Sequence(items), .. }
                        if address.normalized_tag() == "0048,021A" && items.len() == 1)
                })
            })
        {
            return Err(AdvancedFamilyError::InvalidTiling(format!(
                "sparse WSI requires one slide position for each of {frames} frames"
            )));
        }
    }
    Ok(())
}

fn apply_structured_report_parameters(
    family: StructuredReportFamily,
    instance: &SpecInstance,
    plan: &mut ResolvedInstancePlan,
) -> Result<(), AdvancedFamilyError> {
    match family {
        StructuredReportFamily::BasicText => {
            if let Some(value) = instance
                .parameters
                .get("observation_text")
                .and_then(serde_json::Value::as_str)
            {
                replace_first_sr_value(
                    plan,
                    "0040,A160",
                    AttributeValue::Primitive(PrimitiveValue::String(value.into())),
                )?;
            }
        }
        StructuredReportFamily::Comprehensive => {
            if let Some(value) = instance
                .parameters
                .get("measurement_value_mm")
                .and_then(serde_json::Value::as_f64)
            {
                replace_first_sr_value(
                    plan,
                    "0040,A30A",
                    AttributeValue::Primitive(PrimitiveValue::String(value.to_string())),
                )?;
            }
        }
        StructuredReportFamily::Comprehensive3d => {
            if let Some(values) = instance
                .parameters
                .get("graphic_data_patient_mm")
                .and_then(serde_json::Value::as_array)
            {
                replace_first_sr_value(
                    plan,
                    "0070,0022",
                    AttributeValue::Multi(
                        values
                            .iter()
                            .map(|value| {
                                value.as_f64().map(|value| {
                                    PrimitiveValue::Float32Bits((value as f32).to_bits())
                                })
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(|| {
                                AdvancedFamilyError::StructuredParameter(
                                    "graphic_data_patient_mm must contain numbers".into(),
                                )
                            })?,
                    ),
                )?;
            }
            if let Some(value) = instance
                .parameters
                .get("measurement_value_mm")
                .and_then(serde_json::Value::as_f64)
            {
                replace_first_sr_value(
                    plan,
                    "0040,A30A",
                    AttributeValue::Primitive(PrimitiveValue::String(value.to_string())),
                )?;
            }
        }
        StructuredReportFamily::Tid1500 => {
            if let Some(value) = instance
                .parameters
                .get("measurement_value_mm3")
                .and_then(serde_json::Value::as_f64)
            {
                replace_first_sr_value(
                    plan,
                    "0040,A30A",
                    AttributeValue::Primitive(PrimitiveValue::String(value.to_string())),
                )?;
            }
        }
        StructuredReportFamily::KeyObject => {}
    }
    Ok(())
}

fn replace_first_sr_value(
    plan: &mut ResolvedInstancePlan,
    tag: &str,
    replacement: AttributeValue,
) -> Result<(), AdvancedFamilyError> {
    fn in_value(value: &mut AttributeValue, tag: &str, replacement: &AttributeValue) -> bool {
        let AttributeValue::Sequence(items) = value else {
            return false;
        };
        for item in items {
            for operation in &mut item.attributes {
                let AttributeOperation::Set { address, value, .. } = operation else {
                    continue;
                };
                if address.normalized_tag() == tag {
                    *value = replacement.clone();
                    return true;
                }
                if in_value(value, tag, replacement) {
                    return true;
                }
            }
        }
        false
    }

    for attribute in &mut plan.attributes {
        if attribute.address.normalized_tag() == tag {
            attribute.value = Some(replacement);
            attribute.origin = ValueOrigin::InstanceOverride;
            return Ok(());
        }
        if attribute
            .value
            .as_mut()
            .is_some_and(|value| in_value(value, tag, &replacement))
        {
            return Ok(());
        }
    }
    Err(AdvancedFamilyError::StructuredParameter(format!(
        "qualified content tree has no {tag} parameter target"
    )))
}

fn normalize_quantitative_content(
    family: QuantitativeFamily,
    plan: &mut ResolvedInstancePlan,
) -> Result<(), AdvancedFamilyError> {
    if family == QuantitativeFamily::RealWorldValueMapping {
        if !plan.content.is_empty() {
            return Err(AdvancedFamilyError::TypedBulk(
                "RWVM must not contain a bulk payload".into(),
            ));
        }
        return Ok(());
    }
    if plan.content.len() != 1 {
        return Err(AdvancedFamilyError::TypedBulk(format!(
            "{} must contain exactly one quantitative pixel value",
            plan.template_id
        )));
    }
    let original = plan.content.remove(0);
    let bytes = match original.materialization {
        Some(super::ContentMaterialization::Inline(bytes)) => bytes,
        _ => {
            return Err(AdvancedFamilyError::TypedBulk(
                "qualified quantitative defaults must use native inline pixels".into(),
            ));
        }
    };
    validate_quantitative_bytes(&plan.template_id.0, &bytes)?;
    let bounds = BulkDataBounds::exact(bytes.len() as u64);
    let properties = BTreeMap::from([(
        "semantic_validator".into(),
        match family {
            QuantitativeFamily::Segmentation => "segmentation_pixels",
            QuantitativeFamily::ParametricMap => "finite_parametric_values",
            QuantitativeFamily::RealWorldValueMapping => unreachable!(),
        }
        .into(),
    )]);
    let mut content = match original.address.normalized_tag().as_str() {
        "7FE0,0010" => BulkDataPlan::from_bytes::<PixelDataSlot>(
            bytes,
            original.vr,
            bounds,
            BulkDataSource::DefaultSynthetic,
            properties,
        ),
        "7FE0,0008" => BulkDataPlan::from_bytes::<FloatPixelDataSlot>(
            bytes,
            original.vr,
            bounds,
            BulkDataSource::DefaultSynthetic,
            properties,
        ),
        "7FE0,0009" => BulkDataPlan::from_bytes::<DoubleFloatPixelDataSlot>(
            bytes,
            original.vr,
            bounds,
            BulkDataSource::DefaultSynthetic,
            properties,
        ),
        tag => {
            return Err(AdvancedFamilyError::TypedBulk(format!(
                "unsupported quantitative pixel element {tag}"
            )));
        }
    }
    .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?
    .into_canonical_content();
    content.placement = original.placement;
    plan.content.push(content);
    Ok(())
}

fn apply_quantitative_content(
    family: QuantitativeFamily,
    instance: &SpecInstance,
    plan: &mut ResolvedInstancePlan,
    resolver: &mut LocalContentResolver,
) -> Result<(), AdvancedFamilyError> {
    if family == QuantitativeFamily::RealWorldValueMapping {
        if instance.content.is_empty() {
            return Ok(());
        }
        return Err(AdvancedFamilyError::UnsupportedContent(
            instance.instance_id.clone(),
        ));
    }
    if instance.content.is_empty()
        || (instance.content.len() == 1
            && matches!(instance.content[0].source, ContentSource::Default))
    {
        return Ok(());
    }
    if instance.content.len() != 1 || instance.content[0].slot != "pixels" {
        return Err(AdvancedFamilyError::ContentCardinality(
            instance.instance_id.clone(),
        ));
    }
    let ContentSource::LocalFile {
        path,
        sha256,
        pixel: None,
        ..
    } = &instance.content[0].source
    else {
        return Err(AdvancedFamilyError::UnsupportedContent(
            instance.instance_id.clone(),
        ));
    };
    let expected = &plan.content[0];
    let asset = resolver
        .resolve(
            "pixels",
            "quantitative_pixels",
            Path::new(path),
            sha256.as_deref(),
        )
        .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?;
    let bytes = std::fs::read(&asset.staged_path)
        .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?;
    validate_quantitative_bytes(&plan.template_id.0, &bytes)?;
    let bounds = BulkDataBounds::exact(expected.size_bytes);
    let properties = BTreeMap::from([(
        "semantic_validator".into(),
        match family {
            QuantitativeFamily::Segmentation => "segmentation_pixels",
            QuantitativeFamily::ParametricMap => "finite_parametric_values",
            QuantitativeFamily::RealWorldValueMapping => unreachable!(),
        }
        .into(),
    )]);
    let mut replacement = match expected.address.normalized_tag().as_str() {
        "7FE0,0010" => {
            BulkDataPlan::from_staged::<PixelDataSlot>(asset, expected.vr, bounds, properties)
        }
        "7FE0,0008" => {
            BulkDataPlan::from_staged::<FloatPixelDataSlot>(asset, expected.vr, bounds, properties)
        }
        "7FE0,0009" => BulkDataPlan::from_staged::<DoubleFloatPixelDataSlot>(
            asset,
            expected.vr,
            bounds,
            properties,
        ),
        tag => {
            return Err(AdvancedFamilyError::TypedBulk(format!(
                "unsupported quantitative pixel element {tag}"
            )));
        }
    }
    .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?
    .into_canonical_content();
    replacement.placement = expected.placement.clone();
    plan.content[0] = replacement;
    Ok(())
}

fn validate_quantitative_bytes(template_id: &str, bytes: &[u8]) -> Result<(), AdvancedFamilyError> {
    if template_id == "derived/parametric-map/float32" {
        if bytes.len() % 4 != 0
            || bytes.chunks_exact(4).any(|chunk| {
                !f32::from_le_bytes(chunk.try_into().expect("four-byte chunk")).is_finite()
            })
        {
            return Err(AdvancedFamilyError::TypedBulk(
                "Float Pixel Data must contain only finite little-endian f32 values".into(),
            ));
        }
    } else if template_id == "derived/parametric-map/float64"
        && (bytes.len() % 8 != 0
            || bytes.chunks_exact(8).any(|chunk| {
                !f64::from_le_bytes(chunk.try_into().expect("eight-byte chunk")).is_finite()
            }))
    {
        return Err(AdvancedFamilyError::TypedBulk(
            "Double Float Pixel Data must contain only finite little-endian f64 values".into(),
        ));
    }
    Ok(())
}

const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;

fn extract_typed_bulk_defaults(
    family: TypedBulkFamily,
    plan: &mut ResolvedInstancePlan,
) -> Result<(), AdvancedFamilyError> {
    match family {
        TypedBulkFamily::TwelveLeadEcg | TypedBulkFamily::GeneralEcg => {
            let sequence = super::AttributeAddress::from_normalized_tag("5400,0100")
                .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?;
            let data = super::AttributeAddress::from_normalized_tag("5400,1010")
                .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?;
            let attribute = plan
                .attributes
                .iter_mut()
                .find(|attribute| attribute.address == sequence)
                .ok_or_else(|| {
                    AdvancedFamilyError::TypedBulk("missing Waveform Sequence".into())
                })?;
            let Some(AttributeValue::Sequence(items)) = attribute.value.as_mut() else {
                return Err(AdvancedFamilyError::TypedBulk(
                    "Waveform Sequence is not a sequence".into(),
                ));
            };
            let expected_groups = if family == TypedBulkFamily::TwelveLeadEcg {
                1
            } else {
                2
            };
            if items.len() != expected_groups {
                return Err(AdvancedFamilyError::TypedBulk(format!(
                    "expected {expected_groups} waveform groups but found {}",
                    items.len()
                )));
            }
            for (item_index, item) in items.iter_mut().enumerate() {
                let operation_index = item
                    .attributes
                    .iter()
                    .position(|operation| {
                        matches!(operation, AttributeOperation::Set { address, .. } if address == &data)
                    })
                    .ok_or_else(|| {
                        AdvancedFamilyError::TypedBulk(format!(
                            "waveform group {} has no Waveform Data",
                            item_index + 1
                        ))
                    })?;
                let AttributeOperation::Set { vr, value, .. } =
                    item.attributes.remove(operation_index)
                else {
                    unreachable!("position selected a Set operation")
                };
                let AttributeValue::Binary(bytes) = value else {
                    return Err(AdvancedFamilyError::TypedBulk(format!(
                        "waveform group {} data was not a binary value",
                        item_index + 1
                    )));
                };
                let mut bulk = BulkDataPlan::from_bytes::<WaveformSamplesSlot>(
                    bytes,
                    vr,
                    BulkDataBounds::bounded(2, MAX_DOCUMENT_BYTES, 2),
                    BulkDataSource::DefaultSynthetic,
                    BTreeMap::from([("semantic_validator".into(), "waveform_samples".into())]),
                )
                .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?
                .into_canonical_content();
                bulk.slot = waveform_slot(item_index, expected_groups).into();
                bulk.placement = super::ContentPlacement::Nested {
                    sequence_path: vec![SequenceItemPlacement {
                        sequence: sequence.clone(),
                        item_index,
                    }],
                };
                plan.content.push(bulk);
            }
        }
        TypedBulkFamily::EncapsulatedPdf | TypedBulkFamily::EncapsulatedStl => {
            let address = super::AttributeAddress::from_normalized_tag("0042,0011")
                .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?;
            let attribute_index = plan
                .attributes
                .iter()
                .position(|attribute| attribute.address == address)
                .ok_or_else(|| {
                    AdvancedFamilyError::TypedBulk("missing Encapsulated Document".into())
                })?;
            let attribute = plan.attributes.remove(attribute_index);
            let Some(AttributeValue::Binary(mut bytes)) = attribute.value else {
                return Err(AdvancedFamilyError::TypedBulk(
                    "Encapsulated Document was not a binary value".into(),
                ));
            };
            let declared_length = numeric_attribute(plan, "0042,0015")? as usize;
            if declared_length > bytes.len() {
                return Err(AdvancedFamilyError::TypedBulk(
                    "Encapsulated Document Length exceeds the stored value".into(),
                ));
            }
            bytes.truncate(declared_length);
            validate_document_payload(family, &bytes)?;
            let properties = BTreeMap::from([(
                "semantic_validator".into(),
                match family {
                    TypedBulkFamily::EncapsulatedPdf => "pdf_structure",
                    TypedBulkFamily::EncapsulatedStl => "binary_stl_structure",
                    _ => unreachable!(),
                }
                .into(),
            )]);
            let mut content = match family {
                TypedBulkFamily::EncapsulatedPdf => {
                    BulkDataPlan::from_bytes::<EncapsulatedDocumentSlot>(
                        bytes,
                        attribute.vr,
                        BulkDataBounds::bounded(8, MAX_DOCUMENT_BYTES, 1),
                        BulkDataSource::DefaultSynthetic,
                        properties,
                    )
                }
                TypedBulkFamily::EncapsulatedStl => BulkDataPlan::from_bytes::<MeshSlot>(
                    bytes,
                    attribute.vr,
                    BulkDataBounds::bounded(84, MAX_DOCUMENT_BYTES, 1),
                    BulkDataSource::DefaultSynthetic,
                    properties,
                ),
                _ => unreachable!(),
            }
            .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?
            .into_canonical_content();
            content.slot = document_slot(family).into();
            plan.content.push(content);
        }
    }
    plan.content
        .sort_by(|left, right| left.slot.cmp(&right.slot));
    Ok(())
}

fn apply_typed_bulk_content(
    family: TypedBulkFamily,
    instance: &SpecInstance,
    plan: &mut ResolvedInstancePlan,
    resolver: &mut LocalContentResolver,
) -> Result<(), AdvancedFamilyError> {
    if instance.content.is_empty()
        || instance
            .content
            .iter()
            .all(|assignment| matches!(assignment.source, ContentSource::Default))
    {
        return Ok(());
    }
    if instance.content.len() != plan.content.len() {
        return Err(AdvancedFamilyError::ContentCardinality(
            instance.instance_id.clone(),
        ));
    }
    for assignment in &instance.content {
        let position = plan
            .content
            .iter()
            .position(|content| content.slot == assignment.slot)
            .ok_or_else(|| AdvancedFamilyError::UnsupportedContent(assignment.slot.clone()))?;
        let ContentSource::LocalFile {
            path,
            sha256,
            media_type,
            pixel: None,
        } = &assignment.source
        else {
            return Err(AdvancedFamilyError::UnsupportedContent(
                instance.instance_id.clone(),
            ));
        };
        validate_media_type(family, media_type.as_deref())?;
        let expected = &plan.content[position];
        let asset = resolver
            .resolve(
                &assignment.slot,
                document_slot(family),
                Path::new(path),
                sha256.as_deref(),
            )
            .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?;
        let bounds = match family {
            TypedBulkFamily::TwelveLeadEcg | TypedBulkFamily::GeneralEcg => {
                BulkDataBounds::exact(expected.size_bytes)
            }
            TypedBulkFamily::EncapsulatedPdf => BulkDataBounds::bounded(8, MAX_DOCUMENT_BYTES, 1),
            TypedBulkFamily::EncapsulatedStl => BulkDataBounds::bounded(84, MAX_DOCUMENT_BYTES, 1),
        };
        let mut replacement = match family {
            TypedBulkFamily::TwelveLeadEcg | TypedBulkFamily::GeneralEcg => {
                BulkDataPlan::from_staged::<WaveformSamplesSlot>(
                    asset,
                    expected.vr,
                    bounds,
                    BTreeMap::from([("semantic_validator".into(), "waveform_samples".into())]),
                )
            }
            TypedBulkFamily::EncapsulatedPdf => {
                BulkDataPlan::from_staged::<EncapsulatedDocumentSlot>(
                    asset,
                    expected.vr,
                    bounds,
                    BTreeMap::from([("semantic_validator".into(), "pdf_structure".into())]),
                )
            }
            TypedBulkFamily::EncapsulatedStl => BulkDataPlan::from_staged::<MeshSlot>(
                asset,
                expected.vr,
                bounds,
                BTreeMap::from([("semantic_validator".into(), "binary_stl_structure".into())]),
            ),
        }
        .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?
        .into_canonical_content();
        replacement.slot = assignment.slot.clone();
        replacement.placement = expected.placement.clone();
        let bytes = match replacement.materialization.as_ref() {
            Some(super::ContentMaterialization::StagedFile(path)) => std::fs::read(path)
                .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?,
            _ => unreachable!("local content resolves to a staged file"),
        };
        validate_document_payload(family, &bytes)?;
        if matches!(
            family,
            TypedBulkFamily::EncapsulatedPdf | TypedBulkFamily::EncapsulatedStl
        ) {
            upsert(
                &mut plan.attributes,
                ResolvedAttribute {
                    address: super::AttributeAddress::from_normalized_tag("0042,0015")
                        .map_err(|error| AdvancedFamilyError::TypedBulk(error.to_string()))?,
                    vr: super::DicomVr::UL,
                    value: Some(AttributeValue::Primitive(PrimitiveValue::Unsigned(
                        replacement.size_bytes,
                    ))),
                    origin: ValueOrigin::DerivedStructural,
                },
            );
        }
        plan.content[position] = replacement;
    }
    Ok(())
}

fn waveform_slot(index: usize, groups: usize) -> &'static str {
    match (groups, index) {
        (1, 0) => "waveform_samples",
        (2, 0) => "waveform_samples_1",
        (2, 1) => "waveform_samples_2",
        _ => unreachable!("qualified waveform group count"),
    }
}

fn document_slot(family: TypedBulkFamily) -> &'static str {
    match family {
        TypedBulkFamily::TwelveLeadEcg | TypedBulkFamily::GeneralEcg => "waveform_samples",
        TypedBulkFamily::EncapsulatedPdf => "document",
        TypedBulkFamily::EncapsulatedStl => "mesh",
    }
}

fn validate_media_type(
    family: TypedBulkFamily,
    media_type: Option<&str>,
) -> Result<(), AdvancedFamilyError> {
    let valid = match (family, media_type) {
        (_, None) => true,
        (TypedBulkFamily::EncapsulatedPdf, Some("application/pdf")) => true,
        (TypedBulkFamily::EncapsulatedStl, Some("model/stl" | "application/sla")) => true,
        (
            TypedBulkFamily::TwelveLeadEcg | TypedBulkFamily::GeneralEcg,
            Some("application/octet-stream"),
        ) => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AdvancedFamilyError::TypedBulk(format!(
            "media type {media_type:?} is incompatible with {family:?}"
        )))
    }
}

fn validate_document_payload(
    family: TypedBulkFamily,
    bytes: &[u8],
) -> Result<(), AdvancedFamilyError> {
    match family {
        TypedBulkFamily::TwelveLeadEcg | TypedBulkFamily::GeneralEcg => Ok(()),
        TypedBulkFamily::EncapsulatedPdf => {
            if bytes.starts_with(b"%PDF-")
                && bytes
                    .windows(5)
                    .rev()
                    .take(32)
                    .any(|window| window == b"%%EOF")
            {
                Ok(())
            } else {
                Err(AdvancedFamilyError::TypedBulk(
                    "supplied document is not a bounded PDF payload".into(),
                ))
            }
        }
        TypedBulkFamily::EncapsulatedStl => {
            if bytes.len() < 84 {
                return Err(AdvancedFamilyError::TypedBulk(
                    "binary STL payload is shorter than its header".into(),
                ));
            }
            let triangles = u32::from_le_bytes(bytes[80..84].try_into().expect("four bytes"));
            let expected = 84_usize
                .checked_add(
                    usize::try_from(triangles)
                        .ok()
                        .and_then(|count| count.checked_mul(50))
                        .ok_or_else(|| {
                            AdvancedFamilyError::TypedBulk(
                                "binary STL triangle count overflows".into(),
                            )
                        })?,
                )
                .ok_or_else(|| {
                    AdvancedFamilyError::TypedBulk("binary STL length overflows".into())
                })?;
            if bytes.len() == expected {
                Ok(())
            } else {
                Err(AdvancedFamilyError::TypedBulk(format!(
                    "binary STL declares {triangles} triangles but has {} bytes",
                    bytes.len()
                )))
            }
        }
    }
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

fn optional_sequence_attribute<'a>(
    plan: &'a ResolvedInstancePlan,
    tag: &str,
) -> Result<Option<&'a [super::AttributeItem]>, AdvancedFamilyError> {
    let Some(value) = plan
        .attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == tag)
        .and_then(|attribute| attribute.value.as_ref())
    else {
        return Ok(None);
    };
    match value {
        AttributeValue::Sequence(items) => Ok(Some(items)),
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
        "0020,9311",
        "0020,9161",
        "0020,9162",
        "0020,9163",
        "0020,9228",
        "0020,0242",
        "0048,0006",
        "0048,0007",
        "0048,0105",
        "0008,0019",
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
        let existing_sequence = plan.attributes.iter().any(|attribute| {
            attribute.address == *operation.address() && attribute.vr == super::DicomVr::SQ
        });
        if protected.contains(&tag.as_str()) || existing_sequence {
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
    InvalidTiling(String),
    ConcatenationClosure(String),
    DicomReferenceClosure(String),
    TypedBulk(String),
    StructuredParameter(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn concatenation() -> Vec<ConcatenationPartSummary> {
        (0..2)
            .map(|index| ConcatenationPartSummary {
                instance_id: format!("part_{}", index + 1),
                concatenation_uid: "2.25.1".into(),
                source_uid: "2.25.2".into(),
                number: index + 1,
                total: 2,
                offset: index,
                frames: 1,
            })
            .collect()
    }

    #[test]
    fn concatenation_closure_rejects_gaps_totals_and_identity_drift() {
        validate_concatenation_summaries("root", concatenation()).unwrap();

        let mut invalid = concatenation();
        invalid[1].offset = 2;
        assert!(validate_concatenation_summaries("root", invalid).is_err());

        let mut invalid = concatenation();
        invalid[1].total = 3;
        assert!(validate_concatenation_summaries("root", invalid).is_err());

        let mut invalid = concatenation();
        invalid[1].concatenation_uid = "2.25.3".into();
        assert!(validate_concatenation_summaries("root", invalid).is_err());
    }
}
