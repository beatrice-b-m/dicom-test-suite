//! Frontend conversion from recipe strings to the neutral encoding contract.
//!
//! Recipe schemas intentionally remain human-authored string documents. This
//! adapter is the single boundary at which those strings become an immutable,
//! validated `CorpusPlan` encoding policy.

use std::error::Error;
use std::fmt;

use crate::corpus_plan::{
    CorpusPlanError, EncodingPlan, FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan,
    ItemLengthPolicy, OffsetTablePolicy, PreamblePolicy, SequenceLengthPolicy,
};

use super::EncodingPolicy;

const EXPLICIT_VR_BIG_ENDIAN: &str = "1.2.840.10008.1.2.2";
const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
const BIG_ENDIAN_BACKEND: &str = "encoding.native.explicit_vr_big_endian";
const RLE_BACKEND: &str = "encoding.native.rle_lossless";
const DEFAULT_PART10_BACKEND: &str = "dicom-rs.part10";

pub(crate) fn qualifies_non_template_transfer_syntax(
    transfer_syntax_uid: &str,
    backend_id: &str,
) -> bool {
    matches!(
        (transfer_syntax_uid, backend_id),
        (RLE_LOSSLESS, RLE_BACKEND) | (EXPLICIT_VR_BIG_ENDIAN, BIG_ENDIAN_BACKEND)
    )
}

pub fn encoding_plan_from_recipe(
    policy: &EncodingPolicy,
    implementation: ImplementationIdentityPlan,
) -> Result<EncodingPlan, RecipeEncodingError> {
    reject_provider("sequence_length_policy", &policy.sequence_length_policy)?;
    reject_provider("item_length_policy", &policy.item_length_policy)?;
    reject_provider("offset_table_policy", &policy.offset_table_policy)?;
    reject_provider("fragmentation_policy", &policy.fragmentation_policy)?;

    let sequence_length = match policy.sequence_length_policy.as_str() {
        "default" => SequenceLengthPolicy::WriterDefault,
        "defined" => SequenceLengthPolicy::Defined,
        "undefined" => SequenceLengthPolicy::Undefined,
        value => return Err(unknown("sequence_length_policy", value)),
    };
    let item_length = match policy.item_length_policy.as_str() {
        "default" => ItemLengthPolicy::WriterDefault,
        "defined" => ItemLengthPolicy::Defined,
        "undefined" => ItemLengthPolicy::Undefined,
        value => return Err(unknown("item_length_policy", value)),
    };
    let offset_table = match policy.offset_table_policy.as_str() {
        "none" => OffsetTablePolicy::NotApplicable,
        "empty_basic" => OffsetTablePolicy::EmptyBasic,
        "populated_basic" => OffsetTablePolicy::PopulatedBasic,
        "extended" => OffsetTablePolicy::Extended,
        value => return Err(unknown("offset_table_policy", value)),
    };
    let fragmentation = match policy.fragmentation_policy.as_str() {
        "native" => FragmentationPolicy::Native,
        "one_per_frame" => FragmentationPolicy::OneFragmentPerFrame,
        "bounded_fragments" => return Err(RecipeEncodingError::MissingFragmentMaximum),
        value => return Err(unknown("fragmentation_policy", value)),
    };
    let preamble = match required("preamble_policy", policy.preamble_policy.as_deref())? {
        "zero_filled" => PreamblePolicy::ZeroFilled,
        "deterministic_nonzero" => PreamblePolicy::DeterministicNonZero,
        "provider" => return Err(unresolved("preamble_policy")),
        value => return Err(unknown("preamble_policy", value)),
    };
    let file_meta = match required("file_meta_policy", policy.file_meta_policy.as_deref())? {
        "standard" => FileMetaPolicy::Standard,
        "provider" => return Err(unresolved("file_meta_policy")),
        value => return Err(unknown("file_meta_policy", value)),
    };

    let backend_id = match policy.non_template_encoding_provider_id.as_deref() {
        Some("provider") => return Err(unresolved("non_template_encoding_provider_id")),
        Some(value) => value.to_owned(),
        None => DEFAULT_PART10_BACKEND.to_owned(),
    };
    let expected_backend = match policy.transfer_syntax_uid.as_str() {
        RLE_LOSSLESS => Some(RLE_BACKEND),
        EXPLICIT_VR_BIG_ENDIAN => Some(BIG_ENDIAN_BACKEND),
        _ => None,
    };
    if let Some(expected) = expected_backend {
        if backend_id != expected {
            return Err(RecipeEncodingError::BackendMismatch {
                transfer_syntax_uid: policy.transfer_syntax_uid.clone(),
                expected: expected.to_owned(),
                actual: backend_id,
            });
        }
    }

    let encoding = EncodingPlan {
        transfer_syntax_uid: policy.transfer_syntax_uid.clone(),
        sequence_length,
        item_length,
        fragmentation,
        offset_table,
        preamble,
        file_meta,
        implementation,
        backend_id,
    };
    encoding.validate().map_err(RecipeEncodingError::Plan)?;
    Ok(encoding)
}

fn required<'a>(
    field: &'static str,
    value: Option<&'a str>,
) -> Result<&'a str, RecipeEncodingError> {
    value.ok_or(RecipeEncodingError::MissingPolicy(field))
}

fn reject_provider(field: &'static str, value: &str) -> Result<(), RecipeEncodingError> {
    if value == "provider" {
        Err(unresolved(field))
    } else {
        Ok(())
    }
}

fn unresolved(field: &'static str) -> RecipeEncodingError {
    RecipeEncodingError::UnresolvedProviderPolicy(field)
}

fn unknown(field: &'static str, value: &str) -> RecipeEncodingError {
    RecipeEncodingError::UnknownPolicy {
        field,
        value: value.to_owned(),
    }
}

#[derive(Debug)]
pub enum RecipeEncodingError {
    MissingPolicy(&'static str),
    UnresolvedProviderPolicy(&'static str),
    UnknownPolicy {
        field: &'static str,
        value: String,
    },
    MissingFragmentMaximum,
    BackendMismatch {
        transfer_syntax_uid: String,
        expected: String,
        actual: String,
    },
    Plan(CorpusPlanError),
}

impl fmt::Display for RecipeEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPolicy(field) => write!(formatter, "recipe encoding omits {field}"),
            Self::UnresolvedProviderPolicy(field) => {
                write!(formatter, "recipe encoding leaves {field} as provider")
            }
            Self::UnknownPolicy { field, value } => {
                write!(formatter, "unknown recipe encoding {field} value {value:?}")
            }
            Self::MissingFragmentMaximum => formatter.write_str(
                "bounded_fragments requires a numeric maximum before CorpusPlan construction",
            ),
            Self::BackendMismatch {
                transfer_syntax_uid,
                expected,
                actual,
            } => write!(
                formatter,
                "transfer syntax {transfer_syntax_uid} requires backend {expected}, not {actual}"
            ),
            Self::Plan(error) => write!(formatter, "invalid corpus encoding plan: {error}"),
        }
    }
}

impl Error for RecipeEncodingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            _ => None,
        }
    }
}
