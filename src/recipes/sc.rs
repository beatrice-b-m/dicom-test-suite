//! Plan-only adapters for data-first Secondary Capture recipes.
//!
//! This boundary has no output root, staging path, CLI spec, or materializer.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use dicom_core::Tag;
use serde_json::Value;

use crate::BYTE_STABLE_OUTPUT_VERSION;
use crate::composition::{
    AttributeAddress, AttributeValue, ByteOrder as CompositionByteOrder, CompositionUidRole,
    DicomVr, IdentityPlan, NativePixelPlan as CompositionPixelPlan,
    PhotometricInterpretation as CompositionPhotometric, PixelShape as CompositionPixelShape,
    PlanarConfiguration, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
    SampleType as CompositionSampleType, TemplateDescriptor, TemplateStatus, ValueOrigin,
    canonical_native_pixels,
};
use crate::native_pixel::{
    ByteOrder, ChromaSubsampling, ColorOrganization, NativePixelContent, NativePixelFactory,
    NativePixelRequest, Palette, PhotometricInterpretation, PixelDataVr, PixelPadding, PixelShape,
    StoredValueType,
};
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};

use super::{
    AttributeOperation as RecipeAttributeOperation, CaseRecipe, PlannedArtifactRecipe,
    SecondaryCaptureParameters,
};

const ORDINARY_SC: &str = "1.2.840.10008.5.1.4.1.1.7";
const MULTIFRAME_SINGLE_BIT_SC: &str = "1.2.840.10008.5.1.4.1.1.7.1";
const MULTIFRAME_GRAYSCALE_BYTE_SC: &str = "1.2.840.10008.5.1.4.1.1.7.2";

/// Convert typed recipe pixels into the registry-independent native contract.
pub fn native_pixel_request_from_recipe(
    sc: &SecondaryCaptureParameters,
) -> Result<NativePixelRequest, ScPlanError> {
    let photometric_interpretation = match sc.photometric_interpretation.as_str() {
        "MONOCHROME1" => PhotometricInterpretation::Monochrome1,
        "MONOCHROME2" => PhotometricInterpretation::Monochrome2,
        "PALETTE COLOR" => PhotometricInterpretation::PaletteColor,
        "RGB" => PhotometricInterpretation::Rgb,
        "YBR_FULL" => PhotometricInterpretation::YbrFull,
        "YBR_FULL_422" => PhotometricInterpretation::YbrFull422,
        value => return Err(ScPlanError::UnsupportedPhotometric(value.into())),
    };
    let stored_value_type = match sc.stored_value_type.as_str() {
        "u1" => StoredValueType::U1,
        "u8" => StoredValueType::U8,
        "i8" => StoredValueType::I8,
        "u16" => StoredValueType::U16,
        "i16" => StoredValueType::I16,
        "u32" => StoredValueType::U32,
        "i32" => StoredValueType::I32,
        value => return Err(ScPlanError::UnsupportedStoredValueType(value.into())),
    };
    let pixel_data_vr = match sc.pixel_data_vr.as_str() {
        "OB" => PixelDataVr::Ob,
        "OW" => PixelDataVr::Ow,
        value => return Err(ScPlanError::UnsupportedPixelDataVr(value.into())),
    };
    let color = sc
        .color
        .as_ref()
        .map(|color| {
            let planar_configuration = color.planar_configuration.ok_or_else(|| {
                ScPlanError::InvalidColor("Planar Configuration is required".into())
            })?;
            let chroma_subsampling = match color.chroma_subsampling.as_str() {
                "none" => ChromaSubsampling::None,
                "horizontal_2_to_1" => ChromaSubsampling::Horizontal2To1,
                value => return Err(ScPlanError::UnsupportedChromaSubsampling(value.into())),
            };
            Ok(ColorOrganization {
                planar_configuration,
                chroma_subsampling,
            })
        })
        .transpose()?;

    Ok(NativePixelRequest {
        shape: PixelShape {
            rows: sc.rows,
            columns: sc.columns,
            frames: sc.frames,
            samples_per_pixel: sc.samples_per_pixel,
            photometric_interpretation,
            bits_allocated: sc.bits_allocated,
            bits_stored: sc.bits_stored,
            high_bit: sc.high_bit,
            pixel_representation: sc.pixel_representation,
            stored_value_type,
            // Target byte order is an encoding-plan concern.
            byte_order: ByteOrder::Little,
            pixel_data_vr,
            color,
        },
        stored_values: sc.stored_values.clone(),
        declared_pixel_min: sc.pixel_min,
        declared_pixel_max: sc.pixel_max,
        expected_frame_sha256: sc.frame_sha256.clone(),
        signed_stored_bits: Default::default(),
        padding: sc.padding.as_ref().map(|padding| PixelPadding {
            value: padding.value,
            range_limit: padding.range_limit,
        }),
        palette: sc.palette.as_ref().map(|palette| Palette {
            descriptor: palette.descriptor,
            red: palette.red.clone(),
            green: palette.green.clone(),
            blue: palette.blue.clone(),
        }),
    })
}

/// Resolve the exact canonical native bytes and frame identities in a recipe.
pub fn native_pixel_content_from_recipe(
    sc: &SecondaryCaptureParameters,
) -> Result<NativePixelContent, ScPlanError> {
    NativePixelFactory
        .create(native_pixel_request_from_recipe(sc)?)
        .map_err(ScPlanError::NativePixel)
}

/// Stable inputs available before any transaction staging exists.
pub struct SecondaryCapturePlanInput<'a> {
    pub recipe: &'a CaseRecipe,
    pub artifact: &'a PlannedArtifactRecipe,
    pub template: &'a TemplateDescriptor,
    /// Globally unique ID selected by the corpus planner.
    pub instance_id: &'a str,
    pub standards_lock_sha256: &'a str,
    pub seed: u64,
}

/// Build the complete pre-encoding plan for an ordinary SC recipe artifact.
pub fn resolved_secondary_capture_plan(
    input: SecondaryCapturePlanInput<'_>,
) -> Result<ResolvedInstancePlan, ScPlanError> {
    validate_ordinary_input(&input)?;
    resolved_secondary_capture_base_plan(input)
}

/// Build the provider-neutral SC structure after the caller has established
/// its own provider-specific contract.
pub(super) fn resolved_secondary_capture_base_plan(
    input: SecondaryCapturePlanInput<'_>,
) -> Result<ResolvedInstancePlan, ScPlanError> {
    validate_structural_input(&input)?;
    let sc = input
        .artifact
        .secondary_capture
        .as_ref()
        .ok_or(ScPlanError::MissingSecondaryCapture)?;
    let neutral = native_pixel_content_from_recipe(sc)?;
    let content = composition_pixel_content(&neutral, sc)?;

    let identities = exact_legacy_identities(&input)?;
    let mut attributes = BTreeMap::new();
    add_common_attributes(&mut attributes, &input, &identities)?;
    add_pixel_attributes(
        &mut attributes,
        sc,
        &identities,
        &input.template.sop_class_uid,
    )?;
    add_padding_and_palette(&mut attributes, sc)?;
    add_recipe_attribute_operations(&mut attributes, &input.artifact.attribute_operations)?;
    add_bounded_multiframe_sc_attributes(
        &mut attributes,
        &input.template.sop_class_uid,
        sc.frames,
    )?;
    if input
        .artifact
        .validation_rule_ids
        .iter()
        .any(|rule| rule == "validation.sc.geometry")
    {
        set_string(
            &mut attributes,
            "0020,0060",
            DicomVr::CS,
            "R",
            ValueOrigin::RunDefault,
        )?;
    }

    Ok(ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: input.instance_id.into(),
        template_id: input.template.template_id.clone(),
        template_version: input.template.template_version,
        sop_class_uid: input.template.sop_class_uid.clone(),
        transfer_syntax_uid: input.artifact.encoding.transfer_syntax_uid.clone(),
        identities,
        attributes: attributes.into_values().collect(),
        content: vec![content],
        references: Vec::new(),
    })
}

fn validate_ordinary_input(input: &SecondaryCapturePlanInput<'_>) -> Result<(), ScPlanError> {
    if input.recipe.plan_provider_id != "native.sc_plan" {
        return Err(ScPlanError::WrongPlanProvider(
            input.recipe.plan_provider_id.clone(),
        ));
    }
    if input.artifact.metadata_sc.is_some() {
        return Err(ScPlanError::MetadataOverrideDeferred);
    }
    Ok(())
}

fn validate_structural_input(input: &SecondaryCapturePlanInput<'_>) -> Result<(), ScPlanError> {
    input
        .artifact
        .secondary_capture
        .as_ref()
        .ok_or(ScPlanError::MissingSecondaryCapture)?;
    if input.template.status != TemplateStatus::Qualified {
        return Err(ScPlanError::TemplateNotQualified(
            input.template.template_id.0.clone(),
        ));
    }
    let reference = input
        .artifact
        .template
        .as_ref()
        .ok_or(ScPlanError::MissingTemplate)?;
    if reference.template_id != input.template.template_id.0
        || reference.template_version != input.template.template_version.to_string()
    {
        return Err(ScPlanError::TemplateIdentityMismatch);
    }
    if !matches!(
        input.template.sop_class_uid.as_str(),
        ORDINARY_SC | MULTIFRAME_SINGLE_BIT_SC | MULTIFRAME_GRAYSCALE_BYTE_SC
    ) {
        return Err(ScPlanError::UnsupportedSopClass(
            input.template.sop_class_uid.clone(),
        ));
    }
    Ok(())
}

fn exact_legacy_identities(
    input: &SecondaryCapturePlanInput<'_>,
) -> Result<IdentityPlan, ScPlanError> {
    let allocate = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256: input.standards_lock_sha256,
            case_id: &input.recipe.binding.case_id,
            recipe_version: &input.recipe.recipe_version,
            run_seed: input.seed,
            file_index: input.artifact.order,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let implementation = deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: input.standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: BYTE_STABLE_OUTPUT_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    });
    IdentityPlan::from_exact_values(
        input.instance_id,
        [
            (
                CompositionUidRole::StudyInstance,
                0,
                allocate(UidRole::StudyInstance),
            ),
            (
                CompositionUidRole::SeriesInstance,
                0,
                allocate(UidRole::SeriesInstance),
            ),
            (
                CompositionUidRole::SopInstance,
                0,
                allocate(UidRole::SopInstance),
            ),
            (CompositionUidRole::ImplementationClass, 0, implementation),
        ],
    )
    .map_err(ScPlanError::Identity)
}

fn add_common_attributes(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    input: &SecondaryCapturePlanInput<'_>,
    identities: &IdentityPlan,
) -> Result<(), ScPlanError> {
    for (tag, vr, value) in [
        ("0008,001C", DicomVr::CS, "YES"),
        ("0008,0020", DicomVr::DA, "20260101"),
        ("0008,0023", DicomVr::DA, "20260101"),
        ("0008,0030", DicomVr::TM, "000000"),
        ("0008,0033", DicomVr::TM, "000000"),
        ("0008,0050", DicomVr::SH, ""),
        ("0008,0060", DicomVr::CS, "OT"),
        ("0008,0064", DicomVr::CS, "SYN"),
        ("0008,0070", DicomVr::LO, "dicom-test-suite"),
        ("0008,0090", DicomVr::PN, ""),
        ("0010,0010", DicomVr::PN, "DICOMTEST^SMOKE"),
        ("0010,0020", DicomVr::LO, "DICOMTEST-SMOKE-001"),
        ("0010,0030", DicomVr::DA, "19700101"),
        ("0010,0040", DicomVr::CS, "O"),
        ("0018,1020", DicomVr::LO, BYTE_STABLE_OUTPUT_VERSION),
        ("0020,0010", DicomVr::SH, "SMOKE"),
        ("0020,0011", DicomVr::IS, "1"),
        ("0020,0013", DicomVr::IS, "1"),
        ("0020,0020", DicomVr::CS, ""),
    ] {
        set_string(attributes, tag, vr, value, ValueOrigin::RunDefault)?;
    }
    set_string(
        attributes,
        "0008,1090",
        DicomVr::LO,
        &input.recipe.recipe_id,
        ValueOrigin::RunDefault,
    )?;
    for (tag, role) in [
        ("0020,000D", CompositionUidRole::StudyInstance),
        ("0020,000E", CompositionUidRole::SeriesInstance),
    ] {
        let uid = identities
            .get(&role, 0)
            .ok_or_else(|| ScPlanError::MissingIdentity(role.as_str().into()))?;
        set_string(
            attributes,
            tag,
            DicomVr::UI,
            uid,
            ValueOrigin::DerivedStructural,
        )?;
    }
    Ok(())
}

fn add_pixel_attributes(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    sc: &SecondaryCaptureParameters,
    identities: &IdentityPlan,
    sop_class_uid: &str,
) -> Result<(), ScPlanError> {
    set_string(
        attributes,
        "0008,0016",
        DicomVr::UI,
        sop_class_uid,
        ValueOrigin::DerivedStructural,
    )?;
    set_string(
        attributes,
        "0008,0018",
        DicomVr::UI,
        identities
            .get(&CompositionUidRole::SopInstance, 0)
            .ok_or_else(|| ScPlanError::MissingIdentity("sop_instance_uid".into()))?,
        ValueOrigin::DerivedStructural,
    )?;
    set_unsigned(attributes, "0028,0002", sc.samples_per_pixel.into())?;
    set_string(
        attributes,
        "0028,0004",
        DicomVr::CS,
        &sc.photometric_interpretation,
        ValueOrigin::DerivedStructural,
    )?;
    if let Some(planar) = sc
        .color
        .as_ref()
        .and_then(|color| color.planar_configuration)
    {
        set_unsigned(attributes, "0028,0006", planar.into())?;
    }
    if sc.frames > 1 {
        set_string(
            attributes,
            "0028,0008",
            DicomVr::IS,
            &sc.frames.to_string(),
            ValueOrigin::DerivedStructural,
        )?;
    }
    for (tag, value) in [
        ("0028,0010", u64::from(sc.rows)),
        ("0028,0011", u64::from(sc.columns)),
        ("0028,0100", u64::from(sc.bits_allocated)),
        ("0028,0101", u64::from(sc.bits_stored)),
        ("0028,0102", u64::from(sc.high_bit)),
        ("0028,0103", u64::from(sc.pixel_representation)),
    ] {
        set_unsigned(attributes, tag, value)?;
    }
    Ok(())
}

fn add_padding_and_palette(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    sc: &SecondaryCaptureParameters,
) -> Result<(), ScPlanError> {
    if let Some(padding) = &sc.padding {
        set_padding(
            attributes,
            "0028,0120",
            padding.value,
            sc.pixel_representation,
        )?;
        if let Some(limit) = padding.range_limit {
            set_padding(attributes, "0028,0121", limit, sc.pixel_representation)?;
        }
    }
    if let Some(palette) = &sc.palette {
        for tag in ["0028,1101", "0028,1102", "0028,1103"] {
            set_value(
                attributes,
                tag,
                DicomVr::US,
                AttributeValue::Multi(
                    palette
                        .descriptor
                        .iter()
                        .map(|value| PrimitiveValue::Unsigned(u64::from(*value)))
                        .collect(),
                ),
                ValueOrigin::InstanceOverride,
            )?;
        }
        for (tag, channel) in [
            ("0028,1201", &palette.red),
            ("0028,1202", &palette.green),
            ("0028,1203", &palette.blue),
        ] {
            set_value(
                attributes,
                tag,
                DicomVr::OW,
                AttributeValue::Binary(
                    channel
                        .iter()
                        .flat_map(|value| value.to_le_bytes())
                        .collect(),
                ),
                ValueOrigin::InstanceOverride,
            )?;
        }
    }
    Ok(())
}

fn add_recipe_attribute_operations(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    operations: &[RecipeAttributeOperation],
) -> Result<(), ScPlanError> {
    for operation in operations {
        let address = AttributeAddress::from_normalized_tag(&operation.tag)
            .map_err(ScPlanError::Attribute)?;
        match operation.operation.as_str() {
            "remove" => {
                attributes.remove(&address);
            }
            "empty" => {
                let vr = parse_operation_vr(operation)?;
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
            "set" => {
                let vr = parse_operation_vr(operation)?;
                let value = recipe_attribute_value(
                    vr,
                    operation.value.as_ref().ok_or_else(|| {
                        ScPlanError::InvalidAttributeOperation(operation.tag.clone())
                    })?,
                )?;
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
            _ => {
                return Err(ScPlanError::InvalidAttributeOperation(
                    operation.tag.clone(),
                ));
            }
        }
    }
    Ok(())
}

fn parse_operation_vr(operation: &RecipeAttributeOperation) -> Result<DicomVr, ScPlanError> {
    DicomVr::from_str(
        operation
            .vr
            .as_deref()
            .ok_or_else(|| ScPlanError::InvalidAttributeOperation(operation.tag.clone()))?,
    )
    .map_err(ScPlanError::Attribute)
}

fn recipe_attribute_value(vr: DicomVr, value: &Value) -> Result<AttributeValue, ScPlanError> {
    let string = value
        .as_str()
        .ok_or_else(|| ScPlanError::UnsupportedAttributeValue(value.clone()))?;
    let values = string
        .split('\\')
        .map(|part| PrimitiveValue::String(part.into()))
        .collect::<Vec<_>>();
    if values.len() == 1 {
        Ok(AttributeValue::Primitive(
            values.into_iter().next().unwrap(),
        ))
    } else if matches!(vr, DicomVr::DS | DicomVr::IS | DicomVr::LO | DicomVr::CS) {
        Ok(AttributeValue::Multi(values))
    } else {
        Err(ScPlanError::UnsupportedAttributeValue(value.clone()))
    }
}

fn add_bounded_multiframe_sc_attributes(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    sop_class_uid: &str,
    frames: u32,
) -> Result<(), ScPlanError> {
    let page_numbers = match (sop_class_uid, frames) {
        (MULTIFRAME_SINGLE_BIT_SC, 2) => vec!["1", "2"],
        (MULTIFRAME_GRAYSCALE_BYTE_SC, 3) => vec!["1", "2", "3"],
        (ORDINARY_SC, _) => return Ok(()),
        _ => return Err(ScPlanError::UnsupportedMultiframeShape { frames }),
    };
    for (tag, vr, value) in [
        ("0008,002A", DicomVr::DT, "20260101000000"),
        ("0020,0012", DicomVr::IS, "1"),
        ("0018,0015", DicomVr::CS, "CHEST"),
        ("0028,0301", DicomVr::CS, "NO"),
        ("0028,2110", DicomVr::CS, "00"),
    ] {
        set_string(attributes, tag, vr, value, ValueOrigin::RunDefault)?;
    }
    set_value(
        attributes,
        "0028,0009",
        DicomVr::AT,
        AttributeValue::Primitive(PrimitiveValue::Tag(
            AttributeAddress::standard(Tag(0x0018, 0x2001)).map_err(ScPlanError::Attribute)?,
        )),
        ValueOrigin::DerivedStructural,
    )?;
    set_value(
        attributes,
        "0018,2001",
        DicomVr::IS,
        AttributeValue::Multi(
            page_numbers
                .into_iter()
                .map(|value| PrimitiveValue::String(value.into()))
                .collect(),
        ),
        ValueOrigin::DerivedStructural,
    )?;
    if sop_class_uid == MULTIFRAME_GRAYSCALE_BYTE_SC {
        for (tag, vr, value) in [
            ("0028,1052", DicomVr::DS, "0"),
            ("0028,1053", DicomVr::DS, "1"),
            ("0028,1054", DicomVr::LO, "US"),
            ("2050,0020", DicomVr::CS, "IDENTITY"),
        ] {
            set_string(attributes, tag, vr, value, ValueOrigin::RunDefault)?;
        }
    }
    Ok(())
}

fn composition_pixel_content(
    neutral: &NativePixelContent,
    sc: &SecondaryCaptureParameters,
) -> Result<crate::composition::CanonicalContent, ScPlanError> {
    let shape = &neutral.plan.shape;
    let composition_shape = CompositionPixelShape {
        rows: shape.rows,
        columns: shape.columns,
        frames: shape.frames,
        samples_per_pixel: u8::try_from(shape.samples_per_pixel)
            .map_err(|_| ScPlanError::NumericRange)?,
        photometric_interpretation: match shape.photometric_interpretation {
            PhotometricInterpretation::Monochrome1 => CompositionPhotometric::Monochrome1,
            PhotometricInterpretation::Monochrome2 => CompositionPhotometric::Monochrome2,
            PhotometricInterpretation::PaletteColor => CompositionPhotometric::PaletteColor,
            PhotometricInterpretation::Rgb => CompositionPhotometric::Rgb,
            PhotometricInterpretation::YbrFull => CompositionPhotometric::YbrFull,
            PhotometricInterpretation::YbrFull422 => CompositionPhotometric::YbrFull422,
        },
        sample_type: match shape.stored_value_type {
            StoredValueType::U1 => CompositionSampleType::Bit1,
            StoredValueType::U8 | StoredValueType::U16 | StoredValueType::U32 => {
                CompositionSampleType::UnsignedInteger
            }
            StoredValueType::I8 | StoredValueType::I16 | StoredValueType::I32 => {
                CompositionSampleType::SignedInteger
            }
        },
        bits_allocated: u8::try_from(shape.bits_allocated)
            .map_err(|_| ScPlanError::NumericRange)?,
        bits_stored: u8::try_from(shape.bits_stored).map_err(|_| ScPlanError::NumericRange)?,
        high_bit: u8::try_from(shape.high_bit).map_err(|_| ScPlanError::NumericRange)?,
        byte_order: match shape.byte_order {
            ByteOrder::Little => CompositionByteOrder::Little,
            ByteOrder::Big => CompositionByteOrder::Big,
        },
        planar_configuration: shape
            .color
            .as_ref()
            .map(|color| match color.planar_configuration {
                0 => Ok(PlanarConfiguration::Interleaved),
                1 => Ok(PlanarConfiguration::Planar),
                _ => Err(ScPlanError::InvalidColor(
                    "Planar Configuration must be zero or one".into(),
                )),
            })
            .transpose()?,
    };
    let plan = CompositionPixelPlan::plan(composition_shape).map_err(ScPlanError::PixelPlan)?;
    if plan.unpadded_value_bytes != neutral.plan.unpadded_value_bytes
        || plan.padded_value_bytes != neutral.plan.padded_value_bytes
        || plan.padding_bytes != neutral.plan.padding_bytes
    {
        return Err(ScPlanError::NeutralCompositionMismatch);
    }
    let mut content =
        canonical_native_pixels(&plan, neutral.unpadded_bytes.clone(), BTreeMap::new());
    content.vr = pixel_vr(sc)?;
    Ok(content)
}

fn pixel_vr(sc: &SecondaryCaptureParameters) -> Result<DicomVr, ScPlanError> {
    match sc.pixel_data_vr.as_str() {
        "OB" => Ok(DicomVr::OB),
        "OW" => Ok(DicomVr::OW),
        value => Err(ScPlanError::UnsupportedPixelDataVr(value.into())),
    }
}

fn set_padding(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    tag: &str,
    value: i64,
    representation: u16,
) -> Result<(), ScPlanError> {
    let (vr, value) = if representation == 1 {
        let value = i16::try_from(value).map_err(|_| ScPlanError::NumericRange)?;
        (DicomVr::SS, PrimitiveValue::Signed(value.into()))
    } else {
        let value = u16::try_from(value).map_err(|_| ScPlanError::NumericRange)?;
        (DicomVr::US, PrimitiveValue::Unsigned(value.into()))
    };
    set_value(
        attributes,
        tag,
        vr,
        AttributeValue::Primitive(value),
        ValueOrigin::InstanceOverride,
    )
}

fn set_string(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    tag: &str,
    vr: DicomVr,
    value: &str,
    origin: ValueOrigin,
) -> Result<(), ScPlanError> {
    set_value(
        attributes,
        tag,
        vr,
        AttributeValue::Primitive(PrimitiveValue::String(value.into())),
        origin,
    )
}

fn set_unsigned(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    tag: &str,
    value: u64,
) -> Result<(), ScPlanError> {
    set_value(
        attributes,
        tag,
        DicomVr::US,
        AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
        ValueOrigin::DerivedStructural,
    )
}

fn set_value(
    attributes: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    tag: &str,
    vr: DicomVr,
    value: AttributeValue,
    origin: ValueOrigin,
) -> Result<(), ScPlanError> {
    let address = AttributeAddress::from_normalized_tag(tag).map_err(ScPlanError::Attribute)?;
    attributes.insert(
        address.clone(),
        ResolvedAttribute {
            address,
            vr,
            value: Some(value),
            origin,
        },
    );
    Ok(())
}

#[derive(Debug)]
pub enum ScPlanError {
    MissingSecondaryCapture,
    MissingTemplate,
    MetadataOverrideDeferred,
    WrongPlanProvider(String),
    TemplateNotQualified(String),
    TemplateIdentityMismatch,
    UnsupportedSopClass(String),
    UnsupportedPhotometric(String),
    UnsupportedStoredValueType(String),
    UnsupportedPixelDataVr(String),
    UnsupportedChromaSubsampling(String),
    InvalidColor(String),
    InvalidAttributeOperation(String),
    UnsupportedAttributeValue(Value),
    UnsupportedMultiframeShape { frames: u32 },
    MissingIdentity(String),
    NumericRange,
    NativePixel(crate::native_pixel::NativePixelError),
    PixelPlan(crate::composition::PixelError),
    NeutralCompositionMismatch,
    Attribute(crate::composition::AttributeError),
    Identity(crate::composition::IdentityError),
}

impl fmt::Display for ScPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for ScPlanError {}
