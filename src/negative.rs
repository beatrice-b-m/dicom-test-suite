//! Stable mapping from Phase 7 negative case IDs to checked byte mutations.

#[cfg(test)]
#[path = "mutation.rs"]
mod mutation;
#[cfg(test)]
#[path = "part10_locator.rs"]
mod part10_locator;

#[cfg(test)]
use self::mutation::{
    AcceptableOutcome, ByteRange as MutationRange, ChangedByteRange, FailureLayer, LengthWidth,
    MutationError, MutationParameters, MutationRequest, TruncationTarget, apply_named_mutation,
};
#[cfg(test)]
use self::part10_locator::{
    BITS_STORED, ByteRange as LocatorRange, EXPLICIT_VR_LITTLE_ENDIAN_UID, ElementLocation,
    HIGH_BIT, LocatedPart10, LocatorError, LocatorLimits, PIXEL_DATA, RLE_LOSSLESS_UID,
    SOP_CLASS_UID, SOP_INSTANCE_UID, SPECIFIC_CHARACTER_SET, Tag, locate_explicit_vr_le_part10,
};
#[cfg(not(test))]
use crate::mutation::{
    AcceptableOutcome, ByteRange as MutationRange, ChangedByteRange, FailureLayer, LengthWidth,
    MutationError, MutationParameters, MutationRequest, TruncationTarget, apply_named_mutation,
};
#[cfg(not(test))]
use crate::part10_locator::{
    BITS_STORED, ByteRange as LocatorRange, EXPLICIT_VR_LITTLE_ENDIAN_UID, ElementLocation,
    HIGH_BIT, LocatedPart10, LocatorError, LocatorLimits, PIXEL_DATA, RLE_LOSSLESS_UID,
    SOP_CLASS_UID, SOP_INSTANCE_UID, SPECIFIC_CHARACTER_SET, Tag, locate_explicit_vr_le_part10,
};
use std::error::Error;
use std::fmt;

pub const NEGATIVE_RECIPE_VERSION: &str = "0.1.0";

const PATIENT_NAME: Tag = Tag(0x0010, 0x0010);
const MODALITY: Tag = Tag(0x0008, 0x0060);

const NATIVE_SOURCE: &str = "classic/sc/mono2_u8_explicit_le";
const NESTED_SOURCE: &str = "metadata/sc/defined_undefined_sequence_lengths";
const CHARSET_SOURCE: &str = "metadata/sc/utf8_person_name";
const RLE_SOURCE: &str = "classic/sc/mono1_u8_rle_lossless";
const EOT_SOURCE: &str = "encapsulation/sc/eot_single_fragment_multiframe";

pub const NEGATIVE_CASE_IDS: &[&str] = &[
    "negative/charset/malformed_encoded_text",
    "negative/dataset/invalid_nested_item_length",
    "negative/dataset/truncated_dataset",
    "negative/dataset/truncated_sequence_item",
    "negative/dataset/undefined_length_without_delimitation",
    "negative/encapsulation/broken_offset_table",
    "negative/encapsulation/truncated_fragment",
    "negative/encoding/explicit_vr_length_mismatch",
    "negative/encoding/illegal_vr_bytes",
    "negative/encoding/transfer_syntax_mismatch",
    "negative/identity/meta_dataset_uid_mismatch",
    "negative/iod/missing_type1_attribute",
    "negative/part10/truncated_file_meta",
    "negative/pixels/invalid_bits_and_length",
    "negative/pixels/truncated_pixel_value",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub expected_case_id: &'static str,
    pub sha256: String,
    pub transfer_syntax_uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationStepEvidence {
    pub mutation_id: &'static str,
    pub parameters: MutationParameters,
    pub changed_byte_ranges: Vec<ChangedByteRange>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub expected_failure_layer: FailureLayer,
    pub acceptable_outcomes: Vec<AcceptableOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeEvidence {
    pub case_id: &'static str,
    pub recipe_version: &'static str,
    pub source: SourceIdentity,
    pub source_shape: &'static str,
    pub steps: Vec<MutationStepEvidence>,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeOutput {
    pub bytes: Vec<u8>,
    pub evidence: NegativeEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegativeError {
    UnknownCaseId {
        case_id: String,
    },
    Locate(LocatorError),
    Mutate(MutationError),
    SourceTransferSyntax {
        case_id: &'static str,
        expected: &'static str,
        actual: String,
    },
    MissingElement {
        case_id: &'static str,
        tag: Tag,
    },
    MissingStructure {
        case_id: &'static str,
        requirement: &'static str,
    },
    Capability {
        case_id: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for NegativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCaseId { case_id } => write!(f, "unknown negative case {case_id}"),
            Self::Locate(error) => write!(f, "cannot locate mutation source: {error}"),
            Self::Mutate(error) => write!(f, "cannot apply negative mutation: {error}"),
            Self::SourceTransferSyntax {
                case_id,
                expected,
                actual,
            } => write!(
                f,
                "{case_id} requires source transfer syntax {expected}, got {actual}"
            ),
            Self::MissingElement { case_id, tag } => write!(
                f,
                "{case_id} source lacks element ({:04x},{:04x})",
                tag.0, tag.1
            ),
            Self::MissingStructure {
                case_id,
                requirement,
            } => write!(f, "{case_id} source lacks {requirement}"),
            Self::Capability { case_id, reason } => {
                write!(f, "{case_id} is not safely producible: {reason}")
            }
        }
    }
}

impl Error for NegativeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Locate(error) => Some(error),
            Self::Mutate(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LocatorError> for NegativeError {
    fn from(value: LocatorError) -> Self {
        Self::Locate(value)
    }
}

impl From<MutationError> for NegativeError {
    fn from(value: MutationError) -> Self {
        Self::Mutate(value)
    }
}

struct RecipePlan {
    case_id: &'static str,
    expected_source_case_id: &'static str,
    expected_transfer_syntax: &'static str,
    source_shape: &'static str,
    requests: Vec<MutationRequest>,
}

pub fn build_negative_case(
    case_id: &str,
    valid_source: &[u8],
) -> Result<NegativeOutput, NegativeError> {
    if !NEGATIVE_CASE_IDS.contains(&case_id) {
        return Err(NegativeError::UnknownCaseId {
            case_id: case_id.to_string(),
        });
    }
    let located = locate_explicit_vr_le_part10(valid_source, LocatorLimits::default())?;
    let plan = recipe_plan(case_id, valid_source, &located)?;
    let actual_transfer_syntax = text_value(valid_source, located.transfer_syntax_uid.value);
    if actual_transfer_syntax != plan.expected_transfer_syntax {
        return Err(NegativeError::SourceTransferSyntax {
            case_id: plan.case_id,
            expected: plan.expected_transfer_syntax,
            actual: actual_transfer_syntax,
        });
    }

    let mut bytes = valid_source.to_vec();
    let mut steps = Vec::with_capacity(plan.requests.len());
    for request in plan.requests {
        let result = apply_named_mutation(&bytes, request)?;
        steps.push(MutationStepEvidence {
            mutation_id: result.mutation_id,
            parameters: result.parameters,
            changed_byte_ranges: result.changed_byte_ranges,
            source_sha256: result.source_sha256,
            output_sha256: result.output_sha256.clone(),
            expected_failure_layer: result.expected_failure_layer,
            acceptable_outcomes: result.acceptable_outcomes,
        });
        bytes = result.bytes;
    }
    let source_sha256 = steps
        .first()
        .expect("every producible negative recipe has a mutation step")
        .source_sha256
        .clone();
    let output_sha256 = steps.last().unwrap().output_sha256.clone();
    Ok(NegativeOutput {
        bytes,
        evidence: NegativeEvidence {
            case_id: plan.case_id,
            recipe_version: NEGATIVE_RECIPE_VERSION,
            source: SourceIdentity {
                expected_case_id: plan.expected_source_case_id,
                sha256: source_sha256,
                transfer_syntax_uid: actual_transfer_syntax,
            },
            source_shape: plan.source_shape,
            steps,
            output_sha256,
        },
    })
}

fn recipe_plan(
    case_id: &str,
    source: &[u8],
    located: &LocatedPart10,
) -> Result<RecipePlan, NegativeError> {
    let clean_reject = || {
        vec![
            AcceptableOutcome::CleanRejection,
            AcceptableOutcome::ParseFailure,
        ]
    };
    let validation = || {
        vec![
            AcceptableOutcome::CleanRejection,
            AcceptableOutcome::ValidationFailure,
            AcceptableOutcome::AcceptedWithBoundedWarning,
        ]
    };
    let decode = || {
        vec![
            AcceptableOutcome::CleanRejection,
            AcceptableOutcome::DecodeFailure,
            AcceptableOutcome::ValidationFailure,
        ]
    };
    let request = |parameters, layer, outcomes| MutationRequest::new(parameters, layer, outcomes);

    let plan = match case_id {
        "negative/charset/malformed_encoded_text" => {
            let charset = require_element(located, SPECIFIC_CHARACTER_SET, case_id)?;
            let text = require_element(located, PATIENT_NAME, case_id)?;
            let charset_replacement = repeated_replacement(charset.value, b'X');
            let mut text_replacement = source[text.value.start..text.value.end].to_vec();
            if text_replacement.is_empty() {
                return Err(missing(case_id, "a non-empty Person Name value"));
            }
            text_replacement[0] = 0xff;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[0],
                expected_source_case_id: CHARSET_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with Specific Character Set and non-empty Person Name",
                requests: vec![
                    request(
                        MutationParameters::InvalidCharacterSetDeclaration {
                            value: mutation_range(charset.value),
                            replacement: charset_replacement,
                        },
                        FailureLayer::TextDecoding,
                        decode(),
                    ),
                    request(
                        MutationParameters::MalformedEncodedText {
                            value: mutation_range(text.value),
                            replacement: text_replacement,
                        },
                        FailureLayer::TextDecoding,
                        decode(),
                    ),
                ],
            }
        }
        "negative/dataset/invalid_nested_item_length" => {
            let item = located
                .items
                .first()
                .ok_or_else(|| missing(case_id, "a nested Sequence Item"))?;
            let declared_length = item.value.len().saturating_add(3).min(u32::MAX as usize) as u32;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[1],
                expected_source_case_id: NESTED_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with at least one nested Sequence Item",
                requests: vec![request(
                    MutationParameters::InvalidNestedItemLength {
                        length_field: MutationRange::new(item.header.start + 4, item.header.end),
                        declared_length,
                    },
                    FailureLayer::DatasetParser,
                    clean_reject(),
                )],
            }
        }
        "negative/dataset/truncated_dataset" => {
            let pixel = require_element(located, PIXEL_DATA, case_id)?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[2],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "native Explicit VR LE SC with top-level Pixel Data",
                requests: vec![request(
                    MutationParameters::Truncate {
                        target: TruncationTarget::Dataset,
                        offset: pixel.header.start,
                    },
                    FailureLayer::SemanticValidation,
                    validation(),
                )],
            }
        }
        "negative/dataset/truncated_sequence_item" => {
            let item = located
                .items
                .first()
                .ok_or_else(|| missing(case_id, "a nested Sequence Item"))?;
            if item.value.len() == 0 {
                return Err(missing(case_id, "a non-empty nested Sequence Item"));
            }
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[3],
                expected_source_case_id: NESTED_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with a non-empty nested Sequence Item",
                requests: vec![request(
                    MutationParameters::Truncate {
                        target: TruncationTarget::Item,
                        offset: item.value.start + item.value.len() / 2,
                    },
                    FailureLayer::DatasetParser,
                    clean_reject(),
                )],
            }
        }
        "negative/dataset/undefined_length_without_delimitation" => {
            return Err(NegativeError::Capability {
                case_id: NEGATIVE_CASE_IDS[4],
                reason: "the current mutation primitive rewrites a defined length and removes a delimiter, but a valid source cannot contain both on the same Sequence or Item",
            });
        }
        "negative/encapsulation/broken_offset_table" => {
            let entry = located
                .extended_offset_table_entries
                .first()
                .copied()
                .ok_or_else(|| missing(case_id, "a non-empty Extended Offset Table"))?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[5],
                expected_source_case_id: EOT_SOURCE,
                expected_transfer_syntax: RLE_LOSSLESS_UID,
                source_shape: "RLE SC with a non-empty Extended Offset Table",
                requests: vec![request(
                    MutationParameters::BrokenExtendedOffsetTable {
                        entry: mutation_range(entry),
                        offset: u64::MAX,
                    },
                    FailureLayer::Encapsulation,
                    decode(),
                )],
            }
        }
        "negative/encapsulation/truncated_fragment" => {
            let fragment = located
                .encapsulated_pixel_data
                .as_ref()
                .and_then(|pixel| pixel.fragment_items.first())
                .ok_or_else(|| missing(case_id, "an encapsulated Pixel Data fragment"))?;
            if fragment.value.len() == 0 {
                return Err(missing(case_id, "a non-empty Pixel Data fragment"));
            }
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[6],
                expected_source_case_id: RLE_SOURCE,
                expected_transfer_syntax: RLE_LOSSLESS_UID,
                source_shape: "RLE SC with at least one non-empty Pixel Data fragment",
                requests: vec![request(
                    MutationParameters::Truncate {
                        target: TruncationTarget::Fragment,
                        offset: fragment.value.start + fragment.value.len() / 2,
                    },
                    FailureLayer::Encapsulation,
                    decode(),
                )],
            }
        }
        "negative/encoding/explicit_vr_length_mismatch" => {
            let element = require_element(located, PATIENT_NAME, case_id)?;
            let width = length_width(element.length_field, case_id)?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[7],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with Person Name",
                requests: vec![request(
                    MutationParameters::IncorrectExplicitVrLength {
                        length_field: mutation_range(element.length_field),
                        width,
                        declared_length: element.value.len() as u64 + 2,
                    },
                    FailureLayer::DatasetParser,
                    clean_reject(),
                )],
            }
        }
        "negative/encoding/illegal_vr_bytes" => {
            let element = require_element(located, PATIENT_NAME, case_id)?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[8],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with Person Name",
                requests: vec![request(
                    MutationParameters::IllegalVr {
                        vr_field: MutationRange::new(
                            element.header.start + 4,
                            element.header.start + 6,
                        ),
                        replacement: *b"??",
                    },
                    FailureLayer::DatasetParser,
                    clean_reject(),
                )],
            }
        }
        "negative/encoding/transfer_syntax_mismatch" => RecipePlan {
            case_id: NEGATIVE_CASE_IDS[9],
            expected_source_case_id: NATIVE_SOURCE,
            expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
            source_shape: "native Explicit VR LE SC whose TS UID value can hold the RLE UID",
            requests: vec![request(
                MutationParameters::TransferSyntaxMismatch {
                    file_meta_uid_value: mutation_range(located.transfer_syntax_uid.value),
                    replacement: padded_uid(
                        RLE_LOSSLESS_UID,
                        located.transfer_syntax_uid.value.len(),
                        case_id,
                    )?,
                },
                FailureLayer::DatasetParser,
                clean_reject(),
            )],
        },
        "negative/identity/meta_dataset_uid_mismatch" => {
            let class = require_element(located, SOP_CLASS_UID, case_id)?;
            let instance = require_element(located, SOP_INSTANCE_UID, case_id)?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[10],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with dataset SOP Class and Instance UIDs",
                requests: vec![
                    request(
                        MutationParameters::UidMismatch {
                            dataset_uid_value: mutation_range(class.value),
                            replacement: changed_uid(source, class.value, case_id)?,
                        },
                        FailureLayer::SemanticValidation,
                        validation(),
                    ),
                    request(
                        MutationParameters::UidMismatch {
                            dataset_uid_value: mutation_range(instance.value),
                            replacement: changed_uid(source, instance.value, case_id)?,
                        },
                        FailureLayer::SemanticValidation,
                        validation(),
                    ),
                ],
            }
        }
        "negative/iod/missing_type1_attribute" => {
            let modality = require_element(located, MODALITY, case_id)?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[11],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with top-level Type 1 Modality",
                requests: vec![request(
                    MutationParameters::MissingType1Element {
                        element: MutationRange::new(modality.header.start, modality.value.end),
                    },
                    FailureLayer::SemanticValidation,
                    validation(),
                )],
            }
        }
        "negative/part10/truncated_file_meta" => {
            let value = located.transfer_syntax_uid.value;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[12],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with complete Transfer Syntax UID file meta",
                requests: vec![request(
                    MutationParameters::Truncate {
                        target: TruncationTarget::FileMeta,
                        offset: value.start + value.len() / 2,
                    },
                    FailureLayer::FileMeta,
                    clean_reject(),
                )],
            }
        }
        "negative/pixels/invalid_bits_and_length" => {
            let bits = require_element(located, BITS_STORED, case_id)?;
            let high = require_element(located, HIGH_BIT, case_id)?;
            let pixel = require_element(located, PIXEL_DATA, case_id)?;
            if pixel.declared_length.is_none() {
                return Err(missing(case_id, "defined-length native Pixel Data"));
            }
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[13],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "native Explicit VR LE SC with Bits Stored, High Bit, and defined Pixel Data",
                requests: vec![
                    request(
                        MutationParameters::InvalidBitsStoredHighBit {
                            bits_stored_value: mutation_range(bits.value),
                            high_bit_value: mutation_range(high.value),
                            bits_stored: 17,
                            high_bit: 3,
                        },
                        FailureLayer::PixelDecoding,
                        decode(),
                    ),
                    request(
                        MutationParameters::InvalidPixelByteLength {
                            length_field: mutation_range(pixel.length_field),
                            width: length_width(pixel.length_field, case_id)?,
                            declared_length: pixel.value.len() as u64 + 2,
                        },
                        FailureLayer::PixelDecoding,
                        decode(),
                    ),
                ],
            }
        }
        "negative/pixels/truncated_pixel_value" => {
            let pixel = require_element(located, PIXEL_DATA, case_id)?;
            if pixel.declared_length.is_none() || pixel.value.len() == 0 {
                return Err(missing(
                    case_id,
                    "non-empty defined-length native Pixel Data",
                ));
            }
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[14],
                expected_source_case_id: NATIVE_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "native Explicit VR LE SC with non-empty defined Pixel Data",
                requests: vec![request(
                    MutationParameters::Truncate {
                        target: TruncationTarget::PixelValue,
                        offset: pixel.value.start + pixel.value.len() / 2,
                    },
                    FailureLayer::PixelDecoding,
                    decode(),
                )],
            }
        }
        _ => {
            return Err(NegativeError::UnknownCaseId {
                case_id: case_id.to_string(),
            });
        }
    };
    Ok(plan)
}

fn require_element<'a>(
    located: &'a LocatedPart10,
    tag: Tag,
    case_id: &str,
) -> Result<&'a ElementLocation, NegativeError> {
    located
        .first(tag)
        .ok_or_else(|| NegativeError::MissingElement {
            case_id: stable_case_id(case_id),
            tag,
        })
}

fn stable_case_id(case_id: &str) -> &'static str {
    NEGATIVE_CASE_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == case_id)
        .expect("known case ID has a stable entry")
}

fn missing(case_id: &str, requirement: &'static str) -> NegativeError {
    NegativeError::MissingStructure {
        case_id: stable_case_id(case_id),
        requirement,
    }
}

fn mutation_range(range: LocatorRange) -> MutationRange {
    MutationRange::new(range.start, range.end)
}

fn length_width(range: LocatorRange, case_id: &str) -> Result<LengthWidth, NegativeError> {
    match range.len() {
        2 => Ok(LengthWidth::U16),
        4 => Ok(LengthWidth::U32),
        _ => Err(missing(
            case_id,
            "a 2-byte or 4-byte explicit-VR length field",
        )),
    }
}

fn text_value(source: &[u8], range: LocatorRange) -> String {
    String::from_utf8_lossy(&source[range.start..range.end])
        .trim_end_matches(['\0', ' '])
        .to_string()
}

fn repeated_replacement(range: LocatorRange, byte: u8) -> Vec<u8> {
    vec![byte; range.len()]
}

fn padded_uid(uid: &str, length: usize, case_id: &str) -> Result<Vec<u8>, NegativeError> {
    if uid.len() > length {
        return Err(missing(
            case_id,
            "a Transfer Syntax UID value field wide enough for the replacement",
        ));
    }
    let mut bytes = uid.as_bytes().to_vec();
    bytes.resize(length, 0);
    Ok(bytes)
}

fn changed_uid(
    source: &[u8],
    range: LocatorRange,
    case_id: &str,
) -> Result<Vec<u8>, NegativeError> {
    let mut replacement = source[range.start..range.end].to_vec();
    let index = replacement
        .iter()
        .rposition(u8::is_ascii_digit)
        .ok_or_else(|| missing(case_id, "a UID containing a decimal digit"))?;
    replacement[index] = if replacement[index] == b'9' {
        b'8'
    } else {
        replacement[index] + 1
    };
    Ok(replacement)
}
