//! Stable mapping from Phase 7 negative case IDs to checked byte mutations.

use crate::mutation::{
    AcceptableOutcome, ByteRange as MutationRange, ChangedByteRange, FailureLayer, LengthWidth,
    MutationError, MutationParameters, MutationRequest, TruncationTarget, apply_named_mutation,
};
use crate::part10_locator::{
    BITS_STORED, ByteRange as LocatorRange, EXPLICIT_VR_LITTLE_ENDIAN_UID, ElementLocation,
    HIGH_BIT, LocatedPart10, LocatorError, LocatorLimits, PIXEL_DATA, RLE_LOSSLESS_UID,
    SOP_CLASS_UID, SOP_INSTANCE_UID, SPECIFIC_CHARACTER_SET, Tag, locate_explicit_vr_le_part10,
};
use serde_json::Value;
use std::error::Error;
use std::fmt;

use crate::recipes::{MutationEdit, MutationRecipe};

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
pub struct NegativeParserProbeEvidence {
    pub kind: &'static str,
    pub independence: &'static str,
    pub outcome: &'static str,
    pub detail: &'static str,
}

/// Apply the historical bounded parser classifier without reopening a path or
/// treating expected-invalid bytes as an ordinary valid DICOM instance.
pub fn classify_negative_parser_probe(
    case_id: &str,
    bytes: &[u8],
    expected_layer: &str,
) -> NegativeParserProbeEvidence {
    let (outcome, detail) = match locate_explicit_vr_le_part10(
        bytes,
        LocatorLimits {
            max_elements: 100_000,
            max_depth: 32,
            max_items: 100_000,
            max_fragments: 100_000,
        },
    ) {
        Err(_) => (
            match expected_layer {
                "file_meta" => "clean_rejection",
                "pixel_decoding" | "encapsulation" => "decode_failure",
                "semantic_validation" | "text_decoding" => "validation_failure",
                _ => "parse_failure",
            },
            "same-project bounded Explicit VR LE/RLE parser rejected the mutated byte stream",
        ),
        Ok(_) => (
            match expected_layer {
                "pixel_decoding" | "encapsulation" | "text_decoding" => "decode_failure",
                "dataset_parser" if case_id == "negative/encoding/transfer_syntax_mismatch" => {
                    "parse_failure"
                }
                _ => "accepted_with_bounded_warning",
            },
            "same-project bounded parser accepted structure; the registered bounded semantic/decode classifier supplied the outcome",
        ),
    };
    NegativeParserProbeEvidence {
        kind: "same_project_bounded_parser_classifier",
        independence: "same_project",
        outcome,
        detail,
    }
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
    InvalidContract {
        edit_id: String,
        message: String,
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
            Self::InvalidContract { edit_id, message } => {
                write!(f, "invalid mutation edit {edit_id}: {message}")
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

/// Resolve and apply an explicit recipe-owned mutation contract. The recipe,
/// rather than the case identifier, is authoritative for operation selection,
/// ordering, parameters, failure layers, and acceptable outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeNegativeSourceIdentity {
    pub expected_case_id: String,
    pub sha256: String,
    pub transfer_syntax_uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeNegativeEvidence {
    pub case_id: String,
    pub recipe_version: String,
    pub source: RecipeNegativeSourceIdentity,
    pub source_shape: &'static str,
    pub steps: Vec<MutationStepEvidence>,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeNegativeOutput {
    pub bytes: Vec<u8>,
    pub evidence: RecipeNegativeEvidence,
}

pub fn build_negative_from_recipe(
    case_id: &str,
    recipe_version: &str,
    recipe: &MutationRecipe,
    source_case_id: &str,
    valid_source: &[u8],
) -> Result<RecipeNegativeOutput, NegativeError> {
    let located = locate_explicit_vr_le_part10(valid_source, LocatorLimits::default())?;
    if recipe.edits.is_empty() {
        return Err(invalid_contract("mutation", "edits must not be empty"));
    }
    if recipe.failure_layers.is_empty() || recipe.acceptable_outcomes.is_empty() {
        return Err(invalid_contract(
            "mutation",
            "failure layers and acceptable outcomes must not be empty",
        ));
    }
    let outcomes = recipe
        .acceptable_outcomes
        .iter()
        .map(|value| contract_outcome("mutation", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut requests = Vec::with_capacity(recipe.edits.len());
    for (index, edit) in recipe.edits.iter().enumerate() {
        let layer = recipe
            .failure_layers
            .get(index)
            .or_else(|| recipe.failure_layers.first())
            .ok_or_else(|| invalid_contract(&edit.edit_id, "missing failure layer"))?;
        requests.push(MutationRequest::new(
            resolve_recipe_edit(edit, valid_source, &located)?,
            contract_failure_layer(&edit.edit_id, layer)?,
            outcomes.clone(),
        ));
    }
    let actual_transfer_syntax = text_value(valid_source, located.transfer_syntax_uid.value);
    let mut bytes = valid_source.to_vec();
    let mut steps = Vec::with_capacity(requests.len());
    for request in requests {
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
    let source_sha256 = steps.first().unwrap().source_sha256.clone();
    let output_sha256 = steps.last().unwrap().output_sha256.clone();
    Ok(RecipeNegativeOutput {
        bytes,
        evidence: RecipeNegativeEvidence {
            case_id: case_id.to_owned(),
            recipe_version: recipe_version.to_owned(),
            source: RecipeNegativeSourceIdentity {
                expected_case_id: source_case_id.to_owned(),
                sha256: source_sha256,
                transfer_syntax_uid: actual_transfer_syntax,
            },
            source_shape: source_shape_for_edits(&recipe.edits),
            steps,
            output_sha256,
        },
    })
}

fn resolve_recipe_edit(
    edit: &MutationEdit,
    source: &[u8],
    located: &LocatedPart10,
) -> Result<MutationParameters, NegativeError> {
    let id = edit.mutation_id.strip_prefix("mutation.").ok_or_else(|| {
        invalid_contract(
            &edit.edit_id,
            "mutation_id must use the mutation.* namespace",
        )
    })?;
    let tag = |name: &str| -> Result<Tag, NegativeError> {
        let value = contract_string(edit, name)?;
        parse_contract_tag(&edit.edit_id, value)
    };
    let element = |name: &str| -> Result<&ElementLocation, NegativeError> {
        let tag = tag(name)?;
        located.first(tag).ok_or_else(|| {
            invalid_contract(
                &edit.edit_id,
                &format!("source lacks element ({:04x},{:04x})", tag.0, tag.1),
            )
        })
    };
    let fraction = || -> Result<(usize, usize), NegativeError> {
        let numerator = contract_u64(edit, "numerator")?;
        let denominator = contract_u64(edit, "denominator")?;
        if denominator == 0 || numerator == 0 || numerator >= denominator {
            return Err(invalid_contract(
                &edit.edit_id,
                "fraction must satisfy 0 < numerator < denominator",
            ));
        }
        Ok((numerator as usize, denominator as usize))
    };
    Ok(match id {
        "invalid_character_set_declaration" => {
            let element = element("tag")?;
            let replacement = u8::try_from(contract_u64(edit, "replacement_byte")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "replacement_byte exceeds u8"))?;
            MutationParameters::InvalidCharacterSetDeclaration {
                value: mutation_range(element.value),
                replacement: repeated_replacement(element.value, replacement),
            }
        }
        "malformed_encoded_text" => {
            let element = element("tag")?;
            let offset = usize::try_from(contract_u64(edit, "byte_offset")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "byte_offset exceeds usize"))?;
            let mut replacement = source[element.value.start..element.value.end].to_vec();
            if offset >= replacement.len() {
                return Err(invalid_contract(
                    &edit.edit_id,
                    "byte_offset is out of range",
                ));
            }
            replacement[offset] = u8::try_from(contract_u64(edit, "replacement_byte")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "replacement_byte exceeds u8"))?;
            MutationParameters::MalformedEncodedText {
                value: mutation_range(element.value),
                replacement,
            }
        }
        "invalid_nested_item_length" => {
            require_literal(edit, "selector", "first_sequence_item")?;
            let item = located
                .items
                .first()
                .ok_or_else(|| invalid_contract(&edit.edit_id, "source has no Sequence Item"))?;
            let adjustment = usize::try_from(contract_u64(edit, "declared_length_adjustment")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "length adjustment overflow"))?;
            MutationParameters::InvalidNestedItemLength {
                length_field: MutationRange::new(item.header.start + 4, item.header.end),
                declared_length: u32::try_from(item.value.len().saturating_add(adjustment))
                    .map_err(|_| invalid_contract(&edit.edit_id, "declared length exceeds u32"))?,
            }
        }
        "truncate_dataset" => {
            require_literal(edit, "selector", "pixel_data_header_start")?;
            MutationParameters::Truncate {
                target: TruncationTarget::Dataset,
                offset: located
                    .first(PIXEL_DATA)
                    .ok_or_else(|| invalid_contract(&edit.edit_id, "source lacks Pixel Data"))?
                    .header
                    .start,
            }
        }
        "truncate_sequence_item" => {
            require_literal(edit, "selector", "first_nonempty_sequence_item")?;
            let item = located
                .items
                .iter()
                .find(|item| item.value.len() != 0)
                .ok_or_else(|| invalid_contract(&edit.edit_id, "source has no non-empty item"))?;
            let (numerator, denominator) = fraction()?;
            MutationParameters::Truncate {
                target: TruncationTarget::Item,
                offset: item.value.start + item.value.len() * numerator / denominator,
            }
        }
        "undefined_length_without_delimitation" => {
            require_literal(edit, "selector", "first_undefined_length_sequence")?;
            require_literal(edit, "remove", "sequence_delimitation_item")?;
            let sequence = located
                .sequences
                .iter()
                .find(|sequence| sequence.delimitation.is_some())
                .ok_or_else(|| {
                    invalid_contract(&edit.edit_id, "source has no delimited sequence")
                })?;
            MutationParameters::UndefinedLengthWithoutDelimitation {
                length_field: None,
                delimitation_item: mutation_range(sequence.delimitation.unwrap()),
            }
        }
        "broken_extended_offset_table" => {
            if contract_u64_or_max(edit, "offset")? != u64::MAX {
                return Err(invalid_contract(&edit.edit_id, "offset must be u64_max"));
            }
            let index = usize::try_from(contract_u64(edit, "entry_index")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "entry_index overflow"))?;
            let entry = located
                .extended_offset_table_entries
                .get(index)
                .copied()
                .ok_or_else(|| invalid_contract(&edit.edit_id, "EOT entry is unavailable"))?;
            MutationParameters::BrokenExtendedOffsetTable {
                entry: mutation_range(entry),
                offset: u64::MAX,
            }
        }
        "truncate_fragment" => {
            require_literal(edit, "selector", "first_fragment")?;
            let fragment = located
                .encapsulated_pixel_data
                .as_ref()
                .and_then(|pixel| pixel.fragment_items.first())
                .ok_or_else(|| invalid_contract(&edit.edit_id, "source has no fragment"))?;
            let (numerator, denominator) = fraction()?;
            MutationParameters::Truncate {
                target: TruncationTarget::Fragment,
                offset: fragment.value.start + fragment.value.len() * numerator / denominator,
            }
        }
        "incorrect_explicit_vr_length" => {
            require_literal(edit, "width_policy", "from_vr")?;
            let element = element("tag")?;
            MutationParameters::IncorrectExplicitVrLength {
                length_field: mutation_range(element.length_field),
                width: length_width(element.length_field, &edit.edit_id)?,
                declared_length: (element.value.len() as u64)
                    .checked_add(contract_u64(edit, "declared_length_adjustment")?)
                    .ok_or_else(|| invalid_contract(&edit.edit_id, "declared length overflow"))?,
            }
        }
        "illegal_vr_bytes" => {
            let element = element("tag")?;
            let replacement = contract_string(edit, "replacement_ascii")?.as_bytes();
            MutationParameters::IllegalVr {
                vr_field: MutationRange::new(element.header.start + 4, element.header.start + 6),
                replacement: replacement.try_into().map_err(|_| {
                    invalid_contract(&edit.edit_id, "replacement must be two bytes")
                })?,
            }
        }
        "transfer_syntax_mismatch" => {
            require_literal(edit, "padding_policy", "preserve_value_length")?;
            MutationParameters::TransferSyntaxMismatch {
                file_meta_uid_value: mutation_range(located.transfer_syntax_uid.value),
                replacement: padded_uid(
                    contract_string(edit, "replacement_uid")?,
                    located.transfer_syntax_uid.value.len(),
                    &edit.edit_id,
                )?,
            }
        }
        "dataset_uid_mismatch" => {
            require_literal(
                edit,
                "replacement_policy",
                "increment_last_decimal_digit_preserve_length",
            )?;
            let element = element("dataset_tag")?;
            MutationParameters::UidMismatch {
                dataset_uid_value: mutation_range(element.value),
                replacement: changed_uid(source, element.value, &edit.edit_id)?,
            }
        }
        "missing_type1_attribute" => {
            let element = element("tag")?;
            MutationParameters::MissingType1Element {
                element: MutationRange::new(element.header.start, element.value.end),
            }
        }
        "truncate_file_meta_value" => {
            let element = element("tag")?;
            let (numerator, denominator) = fraction()?;
            MutationParameters::Truncate {
                target: TruncationTarget::FileMeta,
                offset: element.value.start + element.value.len() * numerator / denominator,
            }
        }
        "invalid_bits_stored_high_bit" => MutationParameters::InvalidBitsStoredHighBit {
            bits_stored_value: mutation_range(element("bits_stored_tag")?.value),
            high_bit_value: mutation_range(element("high_bit_tag")?.value),
            bits_stored: u16::try_from(contract_u64(edit, "bits_stored")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "bits_stored exceeds u16"))?,
            high_bit: u16::try_from(contract_u64(edit, "high_bit")?)
                .map_err(|_| invalid_contract(&edit.edit_id, "high_bit exceeds u16"))?,
        },
        "invalid_pixel_byte_length" => {
            require_literal(edit, "width_policy", "from_vr")?;
            let element = element("tag")?;
            MutationParameters::InvalidPixelByteLength {
                length_field: mutation_range(element.length_field),
                width: length_width(element.length_field, &edit.edit_id)?,
                declared_length: (element.value.len() as u64)
                    .checked_add(contract_u64(edit, "declared_length_adjustment")?)
                    .ok_or_else(|| invalid_contract(&edit.edit_id, "declared length overflow"))?,
            }
        }
        "truncate_pixel_value" => {
            let element = element("tag")?;
            let (numerator, denominator) = fraction()?;
            MutationParameters::Truncate {
                target: TruncationTarget::PixelValue,
                offset: element.value.start + element.value.len() * numerator / denominator,
            }
        }
        _ => {
            return Err(invalid_contract(
                &edit.edit_id,
                format!("unsupported mutation_id {}", edit.mutation_id),
            ));
        }
    })
}

fn source_shape_for_edits(edits: &[MutationEdit]) -> &'static str {
    match edits.first().map(|edit| edit.mutation_id.as_str()) {
        Some("mutation.invalid_character_set_declaration") => {
            "Explicit VR LE SC with Specific Character Set and non-empty Person Name"
        }
        Some("mutation.invalid_nested_item_length") | Some("mutation.truncate_sequence_item") => {
            "Explicit VR LE SC with at least one nested Sequence Item"
        }
        Some("mutation.undefined_length_without_delimitation") => {
            "Explicit VR LE SC with a delimited undefined-length Sequence"
        }
        Some("mutation.broken_extended_offset_table") => {
            "RLE SC with a non-empty Extended Offset Table"
        }
        Some("mutation.truncate_fragment") => {
            "RLE SC with at least one non-empty Pixel Data fragment"
        }
        Some("mutation.illegal_vr_bytes") | Some("mutation.incorrect_explicit_vr_length") => {
            "Explicit VR LE SC with Person Name"
        }
        Some("mutation.dataset_uid_mismatch") => {
            "Explicit VR LE SC with dataset SOP Class and Instance UIDs"
        }
        Some("mutation.missing_type1_attribute") => {
            "Explicit VR LE SC with top-level Type 1 Modality"
        }
        Some("mutation.truncate_file_meta_value") => {
            "Explicit VR LE SC with complete Transfer Syntax UID file meta"
        }
        Some("mutation.invalid_bits_stored_high_bit") => {
            "native Explicit VR LE SC with Bits Stored, High Bit, and defined Pixel Data"
        }
        Some("mutation.truncate_pixel_value") => {
            "native Explicit VR LE SC with non-empty defined Pixel Data"
        }
        Some("mutation.transfer_syntax_mismatch") => {
            "native Explicit VR LE SC whose TS UID value can hold the RLE UID"
        }
        _ => "native Explicit VR LE SC with top-level Pixel Data",
    }
}

fn contract_string<'a>(edit: &'a MutationEdit, name: &str) -> Result<&'a str, NegativeError> {
    edit.parameters
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_contract(&edit.edit_id, format!("{name} must be a string")))
}

fn contract_u64(edit: &MutationEdit, name: &str) -> Result<u64, NegativeError> {
    edit.parameters
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid_contract(&edit.edit_id, format!("{name} must be an unsigned integer"))
        })
}

fn contract_u64_or_max(edit: &MutationEdit, name: &str) -> Result<u64, NegativeError> {
    match edit.parameters.get(name) {
        Some(Value::String(value)) if value == "u64_max" => Ok(u64::MAX),
        Some(value) => value.as_u64().ok_or_else(|| {
            invalid_contract(&edit.edit_id, format!("{name} must be u64_max or u64"))
        }),
        None => Err(invalid_contract(&edit.edit_id, format!("missing {name}"))),
    }
}

fn require_literal(edit: &MutationEdit, name: &str, expected: &str) -> Result<(), NegativeError> {
    let actual = contract_string(edit, name)?;
    if actual != expected {
        return Err(invalid_contract(
            &edit.edit_id,
            format!("{name} must be {expected}, got {actual}"),
        ));
    }
    Ok(())
}

fn parse_contract_tag(edit_id: &str, value: &str) -> Result<Tag, NegativeError> {
    let (group, element) = value
        .split_once(',')
        .ok_or_else(|| invalid_contract(edit_id, "tag must be GGGG,EEEE"))?;
    Ok(Tag(
        u16::from_str_radix(group, 16)
            .map_err(|_| invalid_contract(edit_id, "tag group is invalid"))?,
        u16::from_str_radix(element, 16)
            .map_err(|_| invalid_contract(edit_id, "tag element is invalid"))?,
    ))
}

fn contract_failure_layer(edit_id: &str, value: &str) -> Result<FailureLayer, NegativeError> {
    match value {
        "file_meta" => Ok(FailureLayer::FileMeta),
        "dataset_parser" => Ok(FailureLayer::DatasetParser),
        "value_decoding" => Ok(FailureLayer::ValueDecoding),
        "semantic_validation" => Ok(FailureLayer::SemanticValidation),
        "pixel_decoding" => Ok(FailureLayer::PixelDecoding),
        "encapsulation" => Ok(FailureLayer::Encapsulation),
        "text_decoding" => Ok(FailureLayer::TextDecoding),
        _ => Err(invalid_contract(edit_id, "unknown failure layer")),
    }
}

fn contract_outcome(edit_id: &str, value: &str) -> Result<AcceptableOutcome, NegativeError> {
    match value {
        "clean_rejection" => Ok(AcceptableOutcome::CleanRejection),
        "parse_failure" => Ok(AcceptableOutcome::ParseFailure),
        "validation_failure" => Ok(AcceptableOutcome::ValidationFailure),
        "decode_failure" => Ok(AcceptableOutcome::DecodeFailure),
        "accepted_with_bounded_warning" => Ok(AcceptableOutcome::AcceptedWithBoundedWarning),
        _ => Err(invalid_contract(edit_id, "unknown acceptable outcome")),
    }
}

fn invalid_contract(edit_id: impl Into<String>, message: impl Into<String>) -> NegativeError {
    NegativeError::InvalidContract {
        edit_id: edit_id.into(),
        message: message.into(),
    }
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
            let sequence = located
                .sequences
                .iter()
                .find(|sequence| sequence.delimitation.is_some())
                .ok_or_else(|| {
                    missing(case_id, "an undefined-length Sequence with delimitation")
                })?;
            RecipePlan {
                case_id: NEGATIVE_CASE_IDS[4],
                expected_source_case_id: NESTED_SOURCE,
                expected_transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN_UID,
                source_shape: "Explicit VR LE SC with a delimited undefined-length Sequence",
                requests: vec![request(
                    MutationParameters::UndefinedLengthWithoutDelimitation {
                        length_field: None,
                        delimitation_item: mutation_range(
                            sequence
                                .delimitation
                                .expect("selected Sequence has delimitation"),
                        ),
                    },
                    FailureLayer::DatasetParser,
                    clean_reject(),
                )],
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn padded(value: &str, pad: u8) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 != 0 {
            bytes.push(pad);
        }
        bytes
    }

    fn short(tag: Tag, vr: &[u8; 2], value: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&tag.0.to_le_bytes());
        bytes.extend_from_slice(&tag.1.to_le_bytes());
        bytes.extend_from_slice(vr);
        bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
        bytes.extend_from_slice(value);
        bytes
    }

    fn long(tag: Tag, vr: &[u8; 2], value: &[u8], undefined: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&tag.0.to_le_bytes());
        bytes.extend_from_slice(&tag.1.to_le_bytes());
        bytes.extend_from_slice(vr);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(
            &if undefined {
                u32::MAX
            } else {
                value.len() as u32
            }
            .to_le_bytes(),
        );
        bytes.extend_from_slice(value);
        bytes
    }

    fn control(tag: Tag, length: u32, value: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&tag.0.to_le_bytes());
        bytes.extend_from_slice(&tag.1.to_le_bytes());
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(value);
        bytes
    }

    fn wrap(transfer_syntax: &str, dataset: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; 128];
        bytes.extend_from_slice(b"DICM");
        bytes.extend(short(
            Tag(0x0002, 0x0002),
            b"UI",
            &padded("1.2.840.10008.5.1.4.1.1.7", 0),
        ));
        bytes.extend(short(
            Tag(0x0002, 0x0003),
            b"UI",
            &padded("1.2.826.0.1.3680043.10.543.900", 0),
        ));
        bytes.extend(short(
            crate::part10_locator::TRANSFER_SYNTAX_UID,
            b"UI",
            &padded(transfer_syntax, 0),
        ));
        bytes.extend_from_slice(dataset);
        bytes
    }

    fn common_dataset() -> Vec<u8> {
        let mut dataset = Vec::new();
        dataset.extend(short(
            SPECIFIC_CHARACTER_SET,
            b"CS",
            &padded("ISO_IR 192", b' '),
        ));
        dataset.extend(short(
            SOP_CLASS_UID,
            b"UI",
            &padded("1.2.840.10008.5.1.4.1.1.7", 0),
        ));
        dataset.extend(short(
            SOP_INSTANCE_UID,
            b"UI",
            &padded("1.2.826.0.1.3680043.10.543.901", 0),
        ));
        dataset.extend(short(MODALITY, b"CS", b"OT"));
        dataset.extend(short(PATIENT_NAME, b"PN", &padded("Negative^Source", b' ')));
        dataset.extend(short(BITS_STORED, b"US", &8u16.to_le_bytes()));
        dataset.extend(short(HIGH_BIT, b"US", &7u16.to_le_bytes()));
        dataset
    }

    fn native_source() -> Vec<u8> {
        let mut dataset = common_dataset();
        dataset.extend(long(PIXEL_DATA, b"OB", &[1, 2, 3, 4], false));
        wrap(EXPLICIT_VR_LITTLE_ENDIAN_UID, &dataset)
    }

    fn nested_source() -> Vec<u8> {
        let mut dataset = common_dataset();
        let nested = short(
            Tag(0x0008, 0x1155),
            b"UI",
            &padded("1.2.826.0.1.3680043.10.543.902", 0),
        );
        let item = control(Tag(0xfffe, 0xe000), nested.len() as u32, &nested);
        dataset.extend(long(Tag(0x0008, 0x1115), b"SQ", &item, false));
        let mut undefined_value = item;
        undefined_value.extend(control(Tag(0xfffe, 0xe0dd), 0, &[]));
        dataset.extend(long(Tag(0x0008, 0x1140), b"SQ", &undefined_value, true));
        dataset.extend(long(PIXEL_DATA, b"OB", &[1, 2, 3, 4], false));
        wrap(EXPLICIT_VR_LITTLE_ENDIAN_UID, &dataset)
    }

    fn rle_source(with_eot: bool) -> Vec<u8> {
        let mut dataset = common_dataset();
        if with_eot {
            dataset.extend(long(
                crate::part10_locator::EXTENDED_OFFSET_TABLE,
                b"OV",
                &0u64.to_le_bytes(),
                false,
            ));
            dataset.extend(long(
                crate::part10_locator::EXTENDED_OFFSET_TABLE_LENGTHS,
                b"OV",
                &8u64.to_le_bytes(),
                false,
            ));
        }
        let mut pixel = control(Tag(0xfffe, 0xe000), 4, &[0, 0, 0, 0]);
        pixel.extend(control(Tag(0xfffe, 0xe000), 8, &[1, 2, 3, 4, 5, 6, 7, 8]));
        pixel.extend(control(Tag(0xfffe, 0xe0dd), 0, &[]));
        dataset.extend(long(PIXEL_DATA, b"OB", &pixel, true));
        wrap(RLE_LOSSLESS_UID, &dataset)
    }

    fn source_for(case_id: &str) -> Vec<u8> {
        match case_id {
            "negative/dataset/invalid_nested_item_length"
            | "negative/dataset/truncated_sequence_item"
            | "negative/dataset/undefined_length_without_delimitation" => nested_source(),
            "negative/encapsulation/broken_offset_table" => rle_source(true),
            "negative/encapsulation/truncated_fragment" => rle_source(false),
            _ => native_source(),
        }
    }

    #[test]
    fn registry_negative_rows_have_stable_mapping_entries() {
        let registry = include_str!("../cases/registry.json");
        assert_eq!(registry.matches("\"case_id\": \"negative/").count(), 15);
        assert_eq!(NEGATIVE_CASE_IDS.len(), 15);
        for case_id in NEGATIVE_CASE_IDS {
            assert!(registry.contains(&format!("\"case_id\": \"{case_id}\"")));
        }
    }

    #[test]
    fn every_current_producible_recipe_is_deterministic_and_evidence_complete() {
        for case_id in NEGATIVE_CASE_IDS {
            let source = source_for(case_id);
            let first = build_negative_case(case_id, &source).expect(case_id);
            let second = build_negative_case(case_id, &source).expect("repeat recipe");
            assert_eq!(first, second, "{case_id}");
            assert_ne!(first.bytes, source, "{case_id}");
            assert_eq!(first.evidence.case_id, *case_id);
            assert_eq!(first.evidence.recipe_version, NEGATIVE_RECIPE_VERSION);
            assert!(!first.evidence.source.expected_case_id.is_empty());
            assert_eq!(first.evidence.source.sha256.len(), 64);
            assert_eq!(first.evidence.output_sha256.len(), 64);
            assert!(!first.evidence.source_shape.is_empty());
            assert!(!first.evidence.steps.is_empty());
            assert_eq!(
                first.evidence.steps.first().unwrap().source_sha256,
                first.evidence.source.sha256
            );
            assert_eq!(
                first.evidence.steps.last().unwrap().output_sha256,
                first.evidence.output_sha256
            );
            for step in &first.evidence.steps {
                assert!(!step.changed_byte_ranges.is_empty());
                assert!(!step.acceptable_outcomes.is_empty());
                assert_ne!(step.source_sha256, step.output_sha256);
            }
            for pair in first.evidence.steps.windows(2) {
                assert_eq!(pair[0].output_sha256, pair[1].source_sha256);
            }
        }
    }

    #[test]
    fn grouped_rows_use_multi_step_contracts_where_the_registry_groups_failures() {
        for (case_id, expected_steps) in [
            ("negative/charset/malformed_encoded_text", 2),
            ("negative/identity/meta_dataset_uid_mismatch", 2),
            ("negative/pixels/invalid_bits_and_length", 2),
        ] {
            let output = build_negative_case(case_id, &native_source()).expect(case_id);
            assert_eq!(output.evidence.steps.len(), expected_steps);
        }
    }

    #[test]
    fn undefined_length_row_removes_the_existing_sequence_delimiter() {
        let source = nested_source();
        let output = build_negative_case(
            "negative/dataset/undefined_length_without_delimitation",
            &source,
        )
        .expect("undefined-length source should be mutated");
        assert_eq!(output.evidence.steps.len(), 1);
        assert_eq!(
            output.evidence.steps[0].mutation_id,
            "undefined_length_without_delimitation"
        );
        assert_eq!(output.bytes.len() + 8, source.len());
    }

    #[test]
    fn missing_shapes_wrong_syntax_and_unknown_ids_are_typed() {
        let dataset_without_name = long(PIXEL_DATA, b"OB", &[1, 2], false);
        let missing_name = wrap(EXPLICIT_VR_LITTLE_ENDIAN_UID, &dataset_without_name);
        assert!(matches!(
            build_negative_case("negative/encoding/illegal_vr_bytes", &missing_name),
            Err(NegativeError::MissingElement {
                tag: PATIENT_NAME,
                ..
            })
        ));

        assert!(matches!(
            build_negative_case("negative/pixels/truncated_pixel_value", &rle_source(false)),
            Err(NegativeError::MissingStructure { .. })
                | Err(NegativeError::SourceTransferSyntax { .. })
        ));
        assert!(matches!(
            build_negative_case("negative/not_registered", b"bad"),
            Err(NegativeError::UnknownCaseId { .. })
        ));
    }
}
