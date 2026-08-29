use std::collections::BTreeMap;
use std::fmt;

use super::{
    AttributeAddress, AttributeLayer, AttributeOperation, AttributeResolver, AttributeValue,
    CanonicalContent, CompositionUidRole, ContentMaterialization, DicomVr, IdentityPlan,
    NativePixelPlan, PhotometricInterpretation, PixelShape, PlanarConfiguration, PrimitiveValue,
    ResolvedInstancePlan, SampleType, TemplateDescriptor, TemplateId, ValueOrigin,
};
use crate::sha256_hex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultPixelOutput {
    pub plan: NativePixelPlan,
    pub content: CanonicalContent,
}

pub fn sc_template_default_layer(
    template: &TemplateDescriptor,
) -> Result<AttributeLayer, DefaultError> {
    Ok(
        AttributeResolver::from_descriptor_attributes(&template.attributes)?
            .template_default_layer()?,
    )
}

pub fn sc_default_pixels(template_id: &TemplateId) -> Result<DefaultPixelOutput, DefaultError> {
    let (shape, bytes) = match template_id.0.as_str() {
        "classic/secondary-capture/monochrome" => {
            let shape = PixelShape {
                rows: 64,
                columns: 64,
                frames: 1,
                samples_per_pixel: 1,
                photometric_interpretation: PhotometricInterpretation::Monochrome2,
                sample_type: SampleType::UnsignedInteger,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                byte_order: super::ByteOrder::Little,
                planar_configuration: None,
            };
            let bytes = (0_u32..64 * 64)
                .map(|index| ((index % 64) * 4) as u8)
                .collect();
            (shape, bytes)
        }
        "classic/secondary-capture/rgb" => {
            let shape = PixelShape {
                rows: 32,
                columns: 32,
                frames: 1,
                samples_per_pixel: 3,
                photometric_interpretation: PhotometricInterpretation::Rgb,
                sample_type: SampleType::UnsignedInteger,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                byte_order: super::ByteOrder::Little,
                planar_configuration: Some(PlanarConfiguration::Interleaved),
            };
            let mut bytes = Vec::with_capacity(32 * 32 * 3);
            for row in 0_u8..32 {
                for column in 0_u8..32 {
                    bytes.extend([column * 8, row * 8, column.wrapping_add(row) * 4]);
                }
            }
            (shape, bytes)
        }
        other => return Err(DefaultError::UnsupportedTemplate(other.to_string())),
    };
    let plan = NativePixelPlan::plan(shape)?;
    debug_assert_eq!(plan.unpadded_value_bytes, bytes.len() as u64);
    let content = canonical_native_pixels(&plan, bytes, BTreeMap::new());
    Ok(DefaultPixelOutput { plan, content })
}

pub fn canonical_native_pixels(
    pixel_plan: &NativePixelPlan,
    bytes: Vec<u8>,
    properties: BTreeMap<String, String>,
) -> CanonicalContent {
    CanonicalContent {
        slot: "pixels".into(),
        kind: "native_pixels".into(),
        address: AttributeAddress::from_normalized_tag("7FE0,0010")
            .expect("Pixel Data is a known tag"),
        vr: if pixel_plan.shape.bits_allocated <= 8 {
            DicomVr::OB
        } else {
            DicomVr::OW
        },
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        properties,
        materialization: Some(ContentMaterialization::Inline(bytes)),
    }
}

pub fn sc_derived_layer(
    identities: &IdentityPlan,
    pixel_plan: &NativePixelPlan,
) -> Result<AttributeLayer, DefaultError> {
    let uid = |role| {
        identities
            .get(role, 0)
            .map(str::to_string)
            .ok_or_else(|| DefaultError::MissingIdentity(role.as_str().to_string()))
    };
    let shape = &pixel_plan.shape;
    let mut operations = vec![
        set_string("0008,0016", DicomVr::UI, "1.2.840.10008.5.1.4.1.1.7"),
        set_string(
            "0008,0018",
            DicomVr::UI,
            uid(&CompositionUidRole::SopInstance)?,
        ),
        set_string(
            "0020,000D",
            DicomVr::UI,
            uid(&CompositionUidRole::StudyInstance)?,
        ),
        set_string(
            "0020,000E",
            DicomVr::UI,
            uid(&CompositionUidRole::SeriesInstance)?,
        ),
        set_unsigned("0028,0002", shape.samples_per_pixel as u64),
        set_string(
            "0028,0004",
            DicomVr::CS,
            photometric_name(shape.photometric_interpretation),
        ),
        set_unsigned("0028,0010", shape.rows as u64),
        set_unsigned("0028,0011", shape.columns as u64),
        set_unsigned("0028,0100", shape.bits_allocated as u64),
        set_unsigned("0028,0101", shape.bits_stored as u64),
        set_unsigned("0028,0102", shape.high_bit as u64),
        set_unsigned(
            "0028,0103",
            u64::from(shape.sample_type == SampleType::SignedInteger),
        ),
    ];
    if shape.frames > 1 {
        operations.push(set_string(
            "0028,0008",
            DicomVr::IS,
            shape.frames.to_string(),
        ));
    }
    if let Some(planar_configuration) = shape.planar_configuration {
        operations.push(set_unsigned("0028,0006", planar_configuration as u64));
    }
    Ok(AttributeLayer {
        origin: ValueOrigin::DerivedStructural,
        operations,
    })
}

pub fn resolved_sc_plan(
    mut plan: ResolvedInstancePlan,
    template: &TemplateDescriptor,
    run_defaults: &[AttributeOperation],
    instance_overrides: &[AttributeOperation],
    pixel_output: DefaultPixelOutput,
) -> Result<ResolvedInstancePlan, DefaultError> {
    let resolver = AttributeResolver::from_descriptor_attributes(&template.attributes)?;
    let context = super::ResolveContext {
        content_slots: ["pixels".to_string()].into_iter().collect(),
        parameters: BTreeMap::from([
            (
                "multiframe".into(),
                (pixel_output.plan.shape.frames > 1).into(),
            ),
            (
                "color".into(),
                (pixel_output.plan.shape.samples_per_pixel == 3).into(),
            ),
        ]),
    };
    let layers = [
        resolver.template_default_layer()?,
        AttributeLayer {
            origin: ValueOrigin::RunDefault,
            operations: run_defaults.to_vec(),
        },
        AttributeLayer {
            origin: ValueOrigin::InstanceOverride,
            operations: instance_overrides.to_vec(),
        },
        sc_derived_layer(&plan.identities, &pixel_output.plan)?,
    ];
    plan.attributes = resolver.resolve(&layers, &context)?;
    plan.content = vec![pixel_output.content];
    Ok(plan)
}

fn set_string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).expect("known SC tag"),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn set_unsigned(tag: &str, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).expect("known SC tag"),
        vr: DicomVr::US,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn photometric_name(value: PhotometricInterpretation) -> &'static str {
    match value {
        PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        PhotometricInterpretation::Rgb => "RGB",
        PhotometricInterpretation::YbrFull => "YBR_FULL",
        PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    }
}

#[derive(Debug)]
pub enum DefaultError {
    UnsupportedTemplate(String),
    MissingIdentity(String),
    Pixel(super::PixelError),
    Resolve(super::ResolveError),
}

impl From<super::PixelError> for DefaultError {
    fn from(error: super::PixelError) -> Self {
        Self::Pixel(error)
    }
}

impl From<super::ResolveError> for DefaultError {
    fn from(error: super::ResolveError) -> Self {
        Self::Resolve(error)
    }
}

impl fmt::Display for DefaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DefaultError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{IdentityAllocator, TemplateCatalog};

    const LOCK_HASH: &str = "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559";

    #[test]
    fn default_pixel_providers_are_stable_and_separate() {
        let mono =
            sc_default_pixels(&TemplateId("classic/secondary-capture/monochrome".into())).unwrap();
        let rgb = sc_default_pixels(&TemplateId("classic/secondary-capture/rgb".into())).unwrap();
        assert_eq!(mono.content.size_bytes, 4096);
        assert_eq!(rgb.content.size_bytes, 3072);
        assert_ne!(mono.content.sha256, rgb.content.sha256);
        assert_eq!(
            mono.content.sha256,
            sc_default_pixels(&TemplateId("classic/secondary-capture/monochrome".into()))
                .unwrap()
                .content
                .sha256
        );
    }

    #[test]
    fn module_and_structural_defaults_resolve_without_caller_input() {
        let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
        let template_id = TemplateId("classic/secondary-capture/monochrome".into());
        let template = catalog.resolve_qualified(&template_id, None).unwrap();
        let identities =
            IdentityAllocator::new(LOCK_HASH, template_id, template.template_version, 1)
                .unwrap()
                .allocate_plan(
                    "primary",
                    [
                        (CompositionUidRole::StudyInstance, 0),
                        (CompositionUidRole::SeriesInstance, 0),
                        (CompositionUidRole::SopInstance, 0),
                        (CompositionUidRole::ImplementationClass, 0),
                    ],
                )
                .unwrap();
        let pixel = sc_default_pixels(&template.template_id).unwrap();
        let resolver = AttributeResolver::from_descriptor_attributes(&template.attributes).unwrap();
        let attributes = resolver
            .resolve(
                &[
                    sc_template_default_layer(template).unwrap(),
                    sc_derived_layer(&identities, &pixel.plan).unwrap(),
                ],
                &super::super::ResolveContext {
                    content_slots: ["pixels".into()].into_iter().collect(),
                    parameters: BTreeMap::from([
                        ("multiframe".into(), false.into()),
                        ("color".into(), false.into()),
                    ]),
                },
            )
            .unwrap();
        assert!(
            attributes
                .iter()
                .any(|attribute| attribute.address.normalized_tag() == "0010,0010")
        );
        assert!(
            attributes
                .iter()
                .any(|attribute| attribute.address.normalized_tag() == "0028,0010")
        );
    }
}
