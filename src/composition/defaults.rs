use std::collections::BTreeMap;
use std::fmt;

use super::{
    AttributeAddress, AttributeLayer, AttributeOperation, AttributeResolver, AttributeValue,
    CanonicalContent, CompositionUidRole, ContentMaterialization, DicomVr, IdentityPlan,
    NativePixelPlan, PhotometricInterpretation, PixelShape, PlanarConfiguration, PrimitiveValue,
    ResolvedInstancePlan, SampleType, TemplateDescriptor, TemplateId, ValueOrigin,
};
use crate::native_pixel::{
    NativePixelContent as NeutralPixelContent, NativePixelFactory, NativePixelPatternRequest,
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
    let pattern = match template_id.0.as_str() {
        "classic/secondary-capture/monochrome" => {
            NativePixelPatternRequest::MonochromeHorizontalRamp {
                rows: 64,
                columns: 64,
                frames: 1,
                column_step: 4,
            }
        }
        "classic/secondary-capture/rgb" => NativePixelPatternRequest::RgbCoordinates {
            rows: 32,
            columns: 32,
            frames: 1,
        },
        other => return Err(DefaultError::UnsupportedTemplate(other.to_string())),
    };
    let neutral = NativePixelFactory.create_pattern(pattern)?;
    adapt_neutral_default(neutral)
}

fn adapt_neutral_default(neutral: NeutralPixelContent) -> Result<DefaultPixelOutput, DefaultError> {
    let neutral_shape = &neutral.plan.shape;
    let shape = PixelShape {
        rows: neutral_shape.rows,
        columns: neutral_shape.columns,
        frames: neutral_shape.frames,
        samples_per_pixel: u8::try_from(neutral_shape.samples_per_pixel)
            .map_err(|_| DefaultError::NeutralShape("Samples per Pixel exceeds u8".into()))?,
        photometric_interpretation: match neutral_shape.photometric_interpretation {
            crate::native_pixel::PhotometricInterpretation::Monochrome1 => {
                PhotometricInterpretation::Monochrome1
            }
            crate::native_pixel::PhotometricInterpretation::Monochrome2 => {
                PhotometricInterpretation::Monochrome2
            }
            crate::native_pixel::PhotometricInterpretation::PaletteColor => {
                PhotometricInterpretation::PaletteColor
            }
            crate::native_pixel::PhotometricInterpretation::Rgb => PhotometricInterpretation::Rgb,
            crate::native_pixel::PhotometricInterpretation::YbrFull => {
                PhotometricInterpretation::YbrFull
            }
            crate::native_pixel::PhotometricInterpretation::YbrFull422 => {
                PhotometricInterpretation::YbrFull422
            }
        },
        sample_type: match neutral_shape.stored_value_type {
            crate::native_pixel::StoredValueType::U1 => SampleType::Bit1,
            crate::native_pixel::StoredValueType::U8
            | crate::native_pixel::StoredValueType::U16
            | crate::native_pixel::StoredValueType::U32 => SampleType::UnsignedInteger,
            crate::native_pixel::StoredValueType::I8
            | crate::native_pixel::StoredValueType::I16
            | crate::native_pixel::StoredValueType::I32 => SampleType::SignedInteger,
        },
        bits_allocated: u8::try_from(neutral_shape.bits_allocated)
            .map_err(|_| DefaultError::NeutralShape("Bits Allocated exceeds u8".into()))?,
        bits_stored: u8::try_from(neutral_shape.bits_stored)
            .map_err(|_| DefaultError::NeutralShape("Bits Stored exceeds u8".into()))?,
        high_bit: u8::try_from(neutral_shape.high_bit)
            .map_err(|_| DefaultError::NeutralShape("High Bit exceeds u8".into()))?,
        byte_order: match neutral_shape.byte_order {
            crate::native_pixel::ByteOrder::Little => super::ByteOrder::Little,
            crate::native_pixel::ByteOrder::Big => super::ByteOrder::Big,
        },
        planar_configuration: neutral_shape
            .color
            .as_ref()
            .map(|color| match color.planar_configuration {
                0 => Ok(PlanarConfiguration::Interleaved),
                1 => Ok(PlanarConfiguration::Planar),
                other => Err(DefaultError::NeutralShape(format!(
                    "unsupported Planar Configuration {other}"
                ))),
            })
            .transpose()?,
    };
    let plan = NativePixelPlan::plan(shape)?;
    if plan.unpadded_value_bytes != neutral.plan.unpadded_value_bytes
        || plan.padded_value_bytes != neutral.plan.padded_value_bytes
        || plan.padding_bytes != neutral.plan.padding_bytes
    {
        return Err(DefaultError::NeutralShape(
            "neutral and composition pixel plans disagree".into(),
        ));
    }
    let content = canonical_native_pixels(&plan, neutral.unpadded_bytes, BTreeMap::new());
    debug_assert_eq!(content.sha256, neutral.unpadded_sha256);
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
        placement: super::ContentPlacement::TopLevel,
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
    NativePixel(crate::native_pixel::NativePixelError),
    NeutralShape(String),
    Resolve(super::ResolveError),
}

impl From<super::PixelError> for DefaultError {
    fn from(error: super::PixelError) -> Self {
        Self::Pixel(error)
    }
}

impl From<crate::native_pixel::NativePixelError> for DefaultError {
    fn from(error: crate::native_pixel::NativePixelError) -> Self {
        Self::NativePixel(error)
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
        let expected_mono = (0_u32..64 * 64)
            .map(|index| ((index % 64) * 4) as u8)
            .collect::<Vec<_>>();
        let mut expected_rgb = Vec::with_capacity(32 * 32 * 3);
        for row in 0_u8..32 {
            for column in 0_u8..32 {
                expected_rgb.extend([column * 8, row * 8, column.wrapping_add(row) * 4]);
            }
        }
        assert_eq!(mono.content.size_bytes, 4096);
        assert_eq!(rgb.content.size_bytes, 3072);
        assert_eq!(
            mono.content.sha256,
            "fc79e707a60d7602732e7b610a0191cf3eb205264589af81571471727db68099"
        );
        assert_eq!(
            rgb.content.sha256,
            "56699dcfac1f1f988529c223f70bb5bad5c1879dc0ed4842ceecb82817cf0e02"
        );
        assert_eq!(
            mono.content.materialization,
            Some(ContentMaterialization::Inline(expected_mono.clone()))
        );
        assert_eq!(
            rgb.content.materialization,
            Some(ContentMaterialization::Inline(expected_rgb.clone()))
        );
        assert_eq!(sha256_hex(&expected_mono), mono.content.sha256);
        assert_eq!(sha256_hex(&expected_rgb), rgb.content.sha256);
        assert!(mono.content.properties.is_empty());
        assert!(rgb.content.properties.is_empty());
        assert_eq!(mono.plan.frame_spans.len(), 1);
        assert_eq!(rgb.plan.frame_spans.len(), 1);
        assert_eq!(mono.plan.unpadded_value_bytes, mono.content.size_bytes);
        assert_eq!(rgb.plan.unpadded_value_bytes, rgb.content.size_bytes);
        assert!(
            mono.plan.unpadded_value_bytes
                <= crate::native_pixel::NativePixelLimits::DEFAULT_MAX_VALUE_BYTES
        );
        assert!(
            rgb.plan.unpadded_value_bytes
                <= crate::native_pixel::NativePixelLimits::DEFAULT_MAX_VALUE_BYTES
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
