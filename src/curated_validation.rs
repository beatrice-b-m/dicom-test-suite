//! Shared, typed validation for plan-first Secondary Capture generation.
//!
//! Validation is deliberately performed against the staged Part 10 object.
//! Manifest projection may serialize this evidence later, but must not infer a
//! successful round trip from the recipe alone.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::{Tag, VR};
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::recipes::{
    MetadataScParameters, PlannedArtifactRecipe, PrivateElementValue, StringValueSource,
};
use crate::sha256_hex;
use crate::validation::{
    PaletteExpectations, Part10Expectations, PixelDataLengthFormula, PixelPaddingExpectations,
    validate_part10_file,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScPixelLengthFormula {
    ContiguousSamples,
    YbrFull422,
    BitPackedContinuousFrames,
    Encapsulated {
        fragments: usize,
        basic_offset_table_offsets: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScPaletteValidation {
    pub descriptor: [u16; 3],
    pub red_data_length: usize,
    pub green_data_length: usize,
    pub blue_data_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScPaddingValidation {
    pub value: i16,
    pub range_limit: Option<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckLayer {
    Internal,
    Standards,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedValidationCheck {
    pub layer: CheckLayer,
    pub name: String,
    pub status: String,
    pub message: String,
}

impl TypedValidationCheck {
    pub fn passed_internal(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            layer: CheckLayer::Internal,
            name: name.into(),
            status: "passed".into(),
            message: message.into(),
        }
    }

    pub fn passed(&self) -> bool {
        self.status == "passed"
    }

    pub fn legacy_json(&self) -> Value {
        json!({
            "name": self.name,
            "status": self.status,
            "message": self.message,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedAttribute {
    pub tag: String,
    pub vr: String,
    pub raw_value_hex: String,
    pub raw_value_sha256: String,
    pub raw_value_byte_length: u64,
    pub decoded_values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MetadataObservation {
    Attributes {
        attributes: Vec<ObservedAttribute>,
    },
    SequenceLengths {
        sequence_tag: String,
        raw_length: u32,
        item_header_matches: bool,
        item_delimiter_present: bool,
        sequence_delimiter_present: bool,
        decoded_item_count: u32,
    },
    NonsquareGeometry {
        variant_id: String,
        pixel_spacing: Option<Vec<String>>,
        nominal_scanned_pixel_spacing: Option<Vec<String>>,
        pixel_aspect_ratio: Option<Vec<String>>,
        patient_space_geometry_present: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonsquareValidationSpec {
    pub variant_id: String,
    pub pixel_spacing: Option<Vec<String>>,
    pub nominal_scanned_pixel_spacing: Option<Vec<String>>,
    pub pixel_aspect_ratio: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedValidationReport {
    pub bytes: Vec<u8>,
    pub checks: Vec<TypedValidationCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_observation: Option<MetadataObservation>,
}

impl TypedValidationReport {
    pub fn legacy_validation_json(&self) -> Value {
        let mut internal = Vec::new();
        let mut standards = Vec::new();
        let mut external = Vec::new();
        for check in &self.checks {
            let value = check.legacy_json();
            match check.layer {
                CheckLayer::Internal => internal.push(value),
                CheckLayer::Standards => standards.push(value),
                CheckLayer::External => external.push(value),
            }
        }
        json!({
            "status": if self.checks.iter().all(TypedValidationCheck::passed) {
                "passed"
            } else {
                "failed"
            },
            "internal": internal,
            "standards": standards,
            "external": external,
        })
    }

    pub fn append(&mut self, check: TypedValidationCheck) {
        self.checks.push(check);
    }
}

pub struct ScPart10ValidationInput<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub rows: u16,
    pub columns: u16,
    pub frames: u16,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: &'a str,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    pub planar_configuration: Option<u16>,
    pub pixel_data_vr: VR,
    pub pixel_data_length_formula: ScPixelLengthFormula,
    pub decoded_frame_hashes: &'a [&'a str],
    pub palette: Option<ScPaletteValidation>,
    pub padding: Option<ScPaddingValidation>,
}

pub fn validate_sc_part10(
    path: &Path,
    input: &ScPart10ValidationInput<'_>,
) -> Result<TypedValidationReport, CuratedValidationError> {
    let validated = validate_part10_file(
        path,
        &Part10Expectations {
            sop_class_uid: input.sop_class_uid,
            sop_instance_uid: input.sop_instance_uid,
            transfer_syntax_uid: input.transfer_syntax_uid,
            implementation_class_uid: input.implementation_class_uid,
            synthetic_data: "YES",
            rows: input.rows,
            columns: input.columns,
            frames: input.frames,
            samples_per_pixel: input.samples_per_pixel,
            photometric_interpretation: input.photometric_interpretation,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            high_bit: input.high_bit,
            pixel_representation: input.pixel_representation,
            planar_configuration: input.planar_configuration,
            pixel_data_vr: input.pixel_data_vr,
            pixel_data_length_formula: match input.pixel_data_length_formula {
                ScPixelLengthFormula::ContiguousSamples => {
                    PixelDataLengthFormula::ContiguousSamples
                }
                ScPixelLengthFormula::YbrFull422 => PixelDataLengthFormula::YbrFull422,
                ScPixelLengthFormula::BitPackedContinuousFrames => {
                    PixelDataLengthFormula::BitPackedContinuousFrames
                }
                ScPixelLengthFormula::Encapsulated {
                    fragments,
                    basic_offset_table_offsets,
                } => PixelDataLengthFormula::Encapsulated {
                    fragments,
                    basic_offset_table_offsets,
                },
            },
            decoded_frame_hashes: input.decoded_frame_hashes,
            palette: input.palette.as_ref().map(|palette| PaletteExpectations {
                descriptor: palette.descriptor,
                red_data_length: palette.red_data_length,
                green_data_length: palette.green_data_length,
                blue_data_length: palette.blue_data_length,
            }),
            padding: input
                .padding
                .as_ref()
                .map(|padding| PixelPaddingExpectations {
                    value: padding.value,
                    range_limit: padding.range_limit,
                }),
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            enhanced_pet_image: None,
            mg_image: None,
            dx_image: None,
            xa_image: None,
            xrf_image: None,
            us_image: None,
            us_multiframe: None,
            nm_image: None,
            pet_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )
    .map_err(|error| CuratedValidationError::Part10(error.to_string()))?;
    Ok(TypedValidationReport {
        bytes: validated.bytes,
        checks: checks_from_legacy(&validated.validation)?,
        metadata_observation: None,
    })
}

pub fn validate_metadata_round_trip(
    path: &Path,
    metadata: &MetadataScParameters,
) -> Result<(TypedValidationCheck, MetadataObservation), CuratedValidationError> {
    let object = open_file(path)
        .map_err(|error| fail(path, format!("reopen metadata SC fixture: {error}")))?;
    match metadata {
        MetadataScParameters::PersonName(expected) => {
            let sets = object
                .element(tags::SPECIFIC_CHARACTER_SET)
                .map_err(|error| fail(path, format!("read Specific Character Set: {error}")))?
                .to_multi_str()
                .map_err(|error| fail(path, format!("decode Specific Character Set: {error}")))?
                .iter()
                .map(|value| value.trim().to_owned())
                .collect::<Vec<_>>();
            if sets != expected.specific_character_sets {
                return Err(fail(path, "Specific Character Set differs from recipe"));
            }
            let pn = observe_attribute(path, &object, tags::PATIENT_NAME, false)?;
            let expected_raw = decode_hex(&expected.patient_name_raw_hex)?;
            if pn.raw_value_hex != hex(&expected_raw)
                || pn.raw_value_sha256 != expected.patient_name_raw_sha256
            {
                return Err(fail(
                    path,
                    "Patient Name encoded bytes changed during round trip",
                ));
            }
            if expected.native_unicode_round_trip
                && pn.decoded_values.first().map(String::as_str)
                    != Some(expected.patient_name_decoded.as_str())
            {
                return Err(fail(path, "Patient Name decoded value differs from recipe"));
            }
            let name = if expected.native_unicode_round_trip {
                "utf8_person_name_round_trip"
            } else {
                "iso2022_person_name_encoded_round_trip"
            };
            let message = if expected.native_unicode_round_trip {
                "The native writer output reopened with the exact declared UTF-8 character set and decoded Person Name."
            } else {
                "The native writer output reopened with the exact declared character-set values and ISO 2022 PN bytes; independent readers prove Unicode semantics."
            };
            let sets_observation =
                observe_attribute(path, &object, tags::SPECIFIC_CHARACTER_SET, true)?;
            Ok((
                TypedValidationCheck::passed_internal(name, message),
                MetadataObservation::Attributes {
                    attributes: vec![sets_observation, pn],
                },
            ))
        }
        MetadataScParameters::TimezoneBoundary(expected) => {
            let mut attributes = Vec::new();
            for (tag, value) in [
                (tags::STUDY_DATE, expected.study_date.as_str()),
                (tags::STUDY_TIME, expected.study_time.as_str()),
                (
                    tags::ACQUISITION_DATE_TIME,
                    expected.acquisition_date_time.as_str(),
                ),
                (
                    tags::TIMEZONE_OFFSET_FROM_UTC,
                    expected.timezone_offset.as_str(),
                ),
            ] {
                let observed = observe_attribute(path, &object, tag, true)?;
                if observed.decoded_values.first().map(String::as_str) != Some(value) {
                    return Err(fail(path, format!("{tag:?} differs from recipe")));
                }
                attributes.push(observed);
            }
            Ok((
                TypedValidationCheck::passed_internal(
                    "timezone_boundary_round_trip",
                    format!(
                        "The {} fixture reopened with exact DA, TM, DT, and Timezone Offset values.",
                        expected.boundary_id
                    ),
                ),
                MetadataObservation::Attributes { attributes },
            ))
        }
        MetadataScParameters::EmptyType2 {
            attributes: expected,
        } => {
            let mut attributes = Vec::new();
            for item in expected {
                let tag = parse_tag(&item.tag)?;
                let observed = observe_attribute(path, &object, tag, true)?;
                if observed.vr != item.vr || observed.raw_value_byte_length != 0 {
                    return Err(fail(
                        path,
                        format!("{} is not empty {}", item.keyword, item.vr),
                    ));
                }
                attributes.push(observed);
            }
            Ok((
                TypedValidationCheck::passed_internal(
                    "empty_type2_round_trip",
                    "The five required Type 2 attributes reopened at their declared VRs with empty values.",
                ),
                MetadataObservation::Attributes { attributes },
            ))
        }
        MetadataScParameters::StringBoundaries { elements } => {
            let mut attributes = Vec::new();
            for item in elements {
                let tag = parse_tag(&item.tag)?;
                let observed = observe_attribute(path, &object, tag, true)?;
                let values = match &item.source {
                    StringValueSource::Repeated {
                        pattern,
                        repetitions,
                    } => {
                        vec![pattern.repeat(*repetitions as usize)]
                    }
                    StringValueSource::Literal { values } => values.clone(),
                };
                if observed.vr != item.vr
                    || observed.decoded_values != values
                    || observed.raw_value_byte_length != u64::from(item.raw_value_byte_length)
                    || observed.raw_value_sha256 != item.raw_value_sha256
                {
                    return Err(fail(
                        path,
                        format!("{} boundary value differs", item.keyword),
                    ));
                }
                attributes.push(observed);
            }
            Ok((
                TypedValidationCheck::passed_internal(
                    "string_boundary_round_trip",
                    "The LT, LO, DS, and IS boundary values reopened with exact VRs and lexical components.",
                ),
                MetadataObservation::Attributes { attributes },
            ))
        }
        MetadataScParameters::PrivateCreators { blocks } => {
            let mut attributes = Vec::new();
            for block in blocks {
                let creator =
                    observe_attribute(path, &object, parse_tag(&block.creator_tag)?, true)?;
                if creator.vr != "LO"
                    || creator.decoded_values.first().map(String::as_str)
                        != Some(block.creator_id.as_str())
                {
                    return Err(fail(
                        path,
                        format!("private creator {} differs", block.creator_tag),
                    ));
                }
                attributes.push(creator);
                for item in &block.elements {
                    let observed = observe_attribute(path, &object, parse_tag(&item.tag)?, true)?;
                    let matches = match &item.value {
                        PrivateElementValue::Lo { text } => {
                            observed.vr == "LO"
                                && observed.decoded_values.first().map(String::as_str)
                                    == Some(text.as_str())
                        }
                        PrivateElementValue::Us { number } => {
                            object
                                .element(parse_tag(&item.tag)?)
                                .ok()
                                .and_then(|e| e.to_int::<u16>().ok())
                                == Some(*number)
                        }
                    };
                    if !matches {
                        return Err(fail(path, format!("private element {} differs", item.tag)));
                    }
                    attributes.push(observed);
                }
            }
            Ok((
                TypedValidationCheck::passed_internal(
                    "private_creator_block_round_trip",
                    "All private creators and typed block elements reopened at their exact tags, VRs, and values.",
                ),
                MetadataObservation::Attributes { attributes },
            ))
        }
        MetadataScParameters::SequenceLengths(expected) => validate_sequence(path, expected),
    }
}

pub fn validate_nonsquare_round_trip(
    path: &Path,
    artifact: &PlannedArtifactRecipe,
) -> Result<(TypedValidationCheck, MetadataObservation), CuratedValidationError> {
    let expected = |tag: &str| -> Option<Vec<String>> {
        artifact.attribute_operations.iter().find_map(|operation| {
            (operation.operation == "set" && operation.tag == tag)
                .then(|| operation.value.as_ref()?.as_str().map(split_values))
                .flatten()
        })
    };
    validate_nonsquare_spec(
        path,
        &NonsquareValidationSpec {
            variant_id: artifact.logical_id.clone(),
            pixel_spacing: expected("0028,0030"),
            nominal_scanned_pixel_spacing: expected("0018,2010"),
            pixel_aspect_ratio: expected("0028,0034"),
        },
    )
}

pub fn validate_nonsquare_spec(
    path: &Path,
    expected: &NonsquareValidationSpec,
) -> Result<(TypedValidationCheck, MetadataObservation), CuratedValidationError> {
    let object = open_file(path)
        .map_err(|error| fail(path, format!("reopen nonsquare SC fixture: {error}")))?;
    let values = |tag: Tag| {
        object
            .element(tag)
            .ok()
            .and_then(|element| element.to_multi_str().ok())
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.trim_end_matches([' ', '\0']).to_owned())
                    .collect::<Vec<_>>()
            })
    };
    let pixel_spacing = values(tags::PIXEL_SPACING);
    let nominal = values(tags::NOMINAL_SCANNED_PIXEL_SPACING);
    let aspect = values(tags::PIXEL_ASPECT_RATIO);
    let patient_space_geometry_present = object.element(tags::IMAGE_POSITION_PATIENT).is_ok()
        || object.element(tags::IMAGE_ORIENTATION_PATIENT).is_ok()
        || object.element(tags::FRAME_OF_REFERENCE_UID).is_ok();
    if pixel_spacing != expected.pixel_spacing
        || nominal != expected.nominal_scanned_pixel_spacing
        || aspect != expected.pixel_aspect_ratio
        || patient_space_geometry_present
    {
        return Err(fail(
            path,
            "nonsquare geometry differs from typed attribute operations",
        ));
    }
    Ok((
        TypedValidationCheck::passed_internal(
            "nonsquare_geometry_round_trip",
            format!(
                "The {} variant preserved exclusive 2:1 row-to-column geometry without patient-space geometry.",
                expected.variant_id
            ),
        ),
        MetadataObservation::NonsquareGeometry {
            variant_id: expected.variant_id.clone(),
            pixel_spacing,
            nominal_scanned_pixel_spacing: nominal,
            pixel_aspect_ratio: aspect,
            patient_space_geometry_present,
        },
    ))
}

fn validate_sequence(
    path: &Path,
    expected: &crate::recipes::SequenceLengthMetadata,
) -> Result<(TypedValidationCheck, MetadataObservation), CuratedValidationError> {
    let object = open_file(path)
        .map_err(|error| fail(path, format!("reopen sequence SC fixture: {error}")))?;
    let tag = parse_tag(&expected.sequence_tag)?;
    let sequence = object
        .element(tag)
        .map_err(|error| fail(path, format!("read sequence: {error}")))?
        .items()
        .ok_or_else(|| fail(path, "sequence does not decode as items"))?;
    if sequence.len() != 1 {
        return Err(fail(path, "sequence item count differs from recipe"));
    }
    for (tag, value) in [
        (tags::CODE_VALUE, expected.code_value.as_str()),
        (
            tags::CODING_SCHEME_DESIGNATOR,
            expected.coding_scheme_designator.as_str(),
        ),
        (tags::CODE_MEANING, expected.code_meaning.as_str()),
    ] {
        let actual = sequence[0]
            .element(tag)
            .ok()
            .and_then(|element| element.to_str().ok());
        if actual.as_deref() != Some(value) {
            return Err(fail(path, format!("sequence code {tag:?} differs")));
        }
    }
    let bytes = fs::read(path).map_err(|error| fail(path, error.to_string()))?;
    let offset = find_top_level_explicit_vr_element(&bytes, tag)
        .ok_or_else(|| fail(path, "raw sequence header is missing"))?;
    let raw_length = u32::from_le_bytes(
        bytes
            .get(offset + 8..offset + 12)
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| fail(path, "truncated SQ length"))?,
    );
    let value_offset = offset + 12;
    let item_data_length = expected.item_dataset_encoded_length as usize;
    let item_delimiter_offset = value_offset + 8 + item_data_length;
    let item_header_matches = bytes.get(value_offset..value_offset + 8)
        == Some(&[0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
    let item_delimiter_present = bytes.get(item_delimiter_offset..item_delimiter_offset + 8)
        == Some(&[0xFE, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0]);
    let sequence_delimiter_present = bytes
        .get(item_delimiter_offset + 8..item_delimiter_offset + 16)
        == Some(&[0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]);
    let expected_raw = u32::from_le_bytes(
        decode_hex(&expected.sequence_length_field_hex)?
            .try_into()
            .map_err(|_| fail(path, "sequence length field is not four bytes"))?,
    );
    if raw_length != expected_raw
        || !item_header_matches
        || item_delimiter_present != expected.item_delimitation_present
        || sequence_delimiter_present != expected.sequence_delimitation_present
    {
        return Err(fail(path, "raw sequence length/delimiter contract differs"));
    }
    Ok((
        TypedValidationCheck::passed_internal(
            "sequence_length_encoding_round_trip",
            format!(
                "The {} SQ length variant preserved exact raw delimiters and decoded code content.",
                expected.variant_id
            ),
        ),
        MetadataObservation::SequenceLengths {
            sequence_tag: expected.sequence_tag.clone(),
            raw_length,
            item_header_matches,
            item_delimiter_present,
            sequence_delimiter_present,
            decoded_item_count: sequence.len() as u32,
        },
    ))
}

fn observe_attribute(
    path: &Path,
    object: &dicom_object::FileDicomObject<dicom_object::mem::InMemDicomObject>,
    tag: Tag,
    decode: bool,
) -> Result<ObservedAttribute, CuratedValidationError> {
    let element = object
        .element(tag)
        .map_err(|error| fail(path, format!("read {tag:?}: {error}")))?;
    let raw = element
        .value()
        .to_bytes()
        .map_err(|error| fail(path, format!("read bytes for {tag:?}: {error}")))?;
    let decoded_values = if decode || element.vr() == VR::PN {
        element
            .to_multi_str()
            .ok()
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.trim_end_matches([' ', '\0']).to_owned())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(ObservedAttribute {
        tag: format!("{:04X},{:04X}", tag.0, tag.1),
        vr: element.vr().to_string().to_owned(),
        raw_value_hex: hex(raw.as_ref()),
        raw_value_sha256: sha256_hex(raw.as_ref()),
        raw_value_byte_length: raw.len() as u64,
        decoded_values,
    })
}

fn checks_from_legacy(value: &Value) -> Result<Vec<TypedValidationCheck>, CuratedValidationError> {
    let mut checks = Vec::new();
    for (field, layer) in [
        ("internal", CheckLayer::Internal),
        ("standards", CheckLayer::Standards),
        ("external", CheckLayer::External),
    ] {
        let values = value
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| CuratedValidationError::LegacyShape(field.into()))?;
        for item in values {
            checks.push(TypedValidationCheck {
                layer,
                name: string_field(item, "name")?,
                status: string_field(item, "status")?,
                message: string_field(item, "message")?,
            });
        }
    }
    Ok(checks)
}

fn string_field(value: &Value, field: &str) -> Result<String, CuratedValidationError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| CuratedValidationError::LegacyShape(field.into()))
}

fn parse_tag(value: &str) -> Result<Tag, CuratedValidationError> {
    if value.len() != 9 || value.as_bytes()[4] != b',' {
        return Err(CuratedValidationError::MalformedTag(value.into()));
    }
    let group = u16::from_str_radix(&value[..4], 16)
        .map_err(|_| CuratedValidationError::MalformedTag(value.into()))?;
    let element = u16::from_str_radix(&value[5..], 16)
        .map_err(|_| CuratedValidationError::MalformedTag(value.into()))?;
    Ok(Tag(group, element))
}

fn split_values(value: &str) -> Vec<String> {
    value.split('\\').map(str::to_owned).collect()
}

fn decode_hex(value: &str) -> Result<Vec<u8>, CuratedValidationError> {
    if value.len() % 2 != 0 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CuratedValidationError::MalformedHex);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or(CuratedValidationError::MalformedHex)
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

fn find_top_level_explicit_vr_element(bytes: &[u8], wanted: Tag) -> Option<usize> {
    if bytes.get(128..132)? != b"DICM" {
        return None;
    }
    let mut offset = 132;
    loop {
        let group = u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?);
        let element = u16::from_le_bytes(bytes.get(offset + 2..offset + 4)?.try_into().ok()?);
        let vr = std::str::from_utf8(bytes.get(offset + 4..offset + 6)?).ok()?;
        let long = matches!(
            vr,
            "OB" | "OD" | "OF" | "OL" | "OV" | "OW" | "SQ" | "UC" | "UR" | "UT" | "UN"
        );
        let (length, value_offset) = if long {
            (
                u32::from_le_bytes(bytes.get(offset + 8..offset + 12)?.try_into().ok()?),
                offset + 12,
            )
        } else {
            (
                u16::from_le_bytes(bytes.get(offset + 6..offset + 8)?.try_into().ok()?) as u32,
                offset + 8,
            )
        };
        if Tag(group, element) == wanted {
            return Some(offset);
        }
        if length == u32::MAX {
            return None;
        }
        offset = value_offset.checked_add(length as usize)?;
    }
}

fn fail(path: &Path, message: impl Into<String>) -> CuratedValidationError {
    CuratedValidationError::RoundTrip {
        path: path.to_owned(),
        message: message.into(),
    }
}

#[derive(Debug)]
pub enum CuratedValidationError {
    Part10(String),
    LegacyShape(String),
    MalformedTag(String),
    MalformedHex,
    RoundTrip { path: PathBuf, message: String },
}

impl fmt::Display for CuratedValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Part10(message) => write!(formatter, "Part 10 validation failed: {message}"),
            Self::LegacyShape(field) => write!(formatter, "legacy validation is missing {field}"),
            Self::MalformedTag(tag) => write!(formatter, "malformed DICOM tag {tag}"),
            Self::MalformedHex => formatter.write_str("malformed hexadecimal value"),
            Self::RoundTrip { path, message } => write!(formatter, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for CuratedValidationError {}
