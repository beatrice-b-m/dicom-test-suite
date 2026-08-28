//! Deterministic, artifact-free mutations over known-good DICOM Part 10 bytes.
//!
//! Locators use half-open byte ranges in the original source. The eventual
//! generator integration is responsible for deriving them from an independently
//! parsed, valid object; this module deliberately does not rediscover elements.

use std::error::Error;
use std::fmt;

pub const MUTATION_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthWidth {
    U16,
    U32,
    U64,
}

impl LengthWidth {
    const fn bytes(self) -> usize {
        match self {
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationTarget {
    FileMeta,
    Dataset,
    Sequence,
    Item,
    Fragment,
    PixelValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationParameters {
    Truncate {
        target: TruncationTarget,
        offset: usize,
    },
    IncorrectExplicitVrLength {
        length_field: ByteRange,
        width: LengthWidth,
        declared_length: u64,
    },
    IllegalVr {
        vr_field: ByteRange,
        replacement: [u8; 2],
    },
    TransferSyntaxMismatch {
        file_meta_uid_value: ByteRange,
        replacement: Vec<u8>,
    },
    UidMismatch {
        dataset_uid_value: ByteRange,
        replacement: Vec<u8>,
    },
    MissingType1Element {
        element: ByteRange,
    },
    InvalidBitsStoredHighBit {
        bits_stored_value: ByteRange,
        high_bit_value: ByteRange,
        bits_stored: u16,
        high_bit: u16,
    },
    InvalidPixelByteLength {
        length_field: ByteRange,
        width: LengthWidth,
        declared_length: u64,
    },
    BrokenBasicOffsetTable {
        entry: ByteRange,
        offset: u32,
    },
    BrokenExtendedOffsetTable {
        entry: ByteRange,
        offset: u64,
    },
    UndefinedLengthWithoutDelimitation {
        /// `None` when the valid source is already undefined length.
        length_field: Option<ByteRange>,
        delimitation_item: ByteRange,
    },
    InvalidNestedItemLength {
        length_field: ByteRange,
        declared_length: u32,
    },
    InvalidCharacterSetDeclaration {
        value: ByteRange,
        replacement: Vec<u8>,
    },
    MalformedEncodedText {
        value: ByteRange,
        replacement: Vec<u8>,
    },
}

impl MutationParameters {
    pub const fn mutation_id(&self) -> &'static str {
        match self {
            Self::Truncate { target, .. } => match target {
                TruncationTarget::FileMeta => "truncate_file_meta",
                TruncationTarget::Dataset => "truncate_dataset",
                TruncationTarget::Sequence => "truncate_sequence",
                TruncationTarget::Item => "truncate_item",
                TruncationTarget::Fragment => "truncate_fragment",
                TruncationTarget::PixelValue => "truncate_pixel_value",
            },
            Self::IncorrectExplicitVrLength { .. } => "incorrect_explicit_vr_length",
            Self::IllegalVr { .. } => "illegal_vr_bytes",
            Self::TransferSyntaxMismatch { .. } => "transfer_syntax_mismatch",
            Self::UidMismatch { .. } => "file_meta_dataset_uid_mismatch",
            Self::MissingType1Element { .. } => "missing_type_1_element",
            Self::InvalidBitsStoredHighBit { .. } => "invalid_bits_stored_high_bit",
            Self::InvalidPixelByteLength { .. } => "invalid_pixel_byte_length",
            Self::BrokenBasicOffsetTable { .. } => "broken_basic_offset_table",
            Self::BrokenExtendedOffsetTable { .. } => "broken_extended_offset_table",
            Self::UndefinedLengthWithoutDelimitation { .. } => {
                "undefined_length_without_delimitation"
            }
            Self::InvalidNestedItemLength { .. } => "invalid_nested_item_length",
            Self::InvalidCharacterSetDeclaration { .. } => "invalid_character_set_declaration",
            Self::MalformedEncodedText { .. } => "malformed_encoded_text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureLayer {
    FileMeta,
    DatasetParser,
    ValueDecoding,
    SemanticValidation,
    PixelDecoding,
    Encapsulation,
    TextDecoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AcceptableOutcome {
    CleanRejection,
    ParseFailure,
    ValidationFailure,
    DecodeFailure,
    AcceptedWithBoundedWarning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationRequest {
    pub contract_version: &'static str,
    pub parameters: MutationParameters,
    pub expected_failure_layer: FailureLayer,
    pub acceptable_outcomes: Vec<AcceptableOutcome>,
}

impl MutationRequest {
    pub fn new(
        parameters: MutationParameters,
        expected_failure_layer: FailureLayer,
        acceptable_outcomes: Vec<AcceptableOutcome>,
    ) -> Self {
        Self {
            contract_version: MUTATION_CONTRACT_VERSION,
            parameters,
            expected_failure_layer,
            acceptable_outcomes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedByteRange {
    /// Half-open range in the valid source. Deleted bytes have a non-empty range.
    pub source: ByteRange,
    /// Half-open range in the mutated output. Deletions have an empty range.
    pub output: ByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationResult {
    pub bytes: Vec<u8>,
    pub contract_version: &'static str,
    pub mutation_id: &'static str,
    pub parameters: MutationParameters,
    pub changed_byte_ranges: Vec<ChangedByteRange>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub expected_failure_layer: FailureLayer,
    pub acceptable_outcomes: Vec<AcceptableOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationError {
    UnsupportedContractVersion {
        actual: String,
    },
    NotPart10,
    EmptyAcceptableOutcomes,
    OutOfBounds {
        range: ByteRange,
        source_len: usize,
    },
    InvalidLocatorLength {
        name: &'static str,
        expected: usize,
        actual: usize,
    },
    ReplacementLength {
        range: ByteRange,
        replacement_len: usize,
    },
    OverlappingEdits {
        first: ByteRange,
        second: ByteRange,
    },
    NoChange {
        range: ByteRange,
    },
    ValueDoesNotFit {
        width: LengthWidth,
        value: u64,
    },
}

impl fmt::Display for MutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedContractVersion { actual } => {
                write!(f, "unsupported mutation contract version {actual}")
            }
            Self::NotPart10 => write!(f, "source is not a DICOM Part 10 byte stream"),
            Self::EmptyAcceptableOutcomes => write!(f, "acceptable outcomes must not be empty"),
            Self::OutOfBounds { range, source_len } => write!(
                f,
                "byte range {}..{} is outside source length {source_len}",
                range.start, range.end
            ),
            Self::InvalidLocatorLength {
                name,
                expected,
                actual,
            } => write!(f, "{name} locator must be {expected} bytes, got {actual}"),
            Self::ReplacementLength {
                range,
                replacement_len,
            } => write!(
                f,
                "replacement for {}..{} must preserve its {}-byte length, got {replacement_len}",
                range.start,
                range.end,
                range.len()
            ),
            Self::OverlappingEdits { first, second } => write!(
                f,
                "edits {}..{} and {}..{} overlap",
                first.start, first.end, second.start, second.end
            ),
            Self::NoChange { range } => write!(
                f,
                "mutation leaves byte range {}..{} unchanged",
                range.start, range.end
            ),
            Self::ValueDoesNotFit { width, value } => {
                write!(f, "value {value} does not fit {width:?}")
            }
        }
    }
}

impl Error for MutationError {}

#[derive(Debug)]
struct Edit {
    range: ByteRange,
    replacement: Vec<u8>,
}

pub fn apply_named_mutation(
    source: &[u8],
    request: MutationRequest,
) -> Result<MutationResult, MutationError> {
    if request.contract_version != MUTATION_CONTRACT_VERSION {
        return Err(MutationError::UnsupportedContractVersion {
            actual: request.contract_version.to_string(),
        });
    }
    if source.len() < 132 || &source[128..132] != b"DICM" {
        return Err(MutationError::NotPart10);
    }
    if request.acceptable_outcomes.is_empty() {
        return Err(MutationError::EmptyAcceptableOutcomes);
    }

    let mut edits = edits_for(source, &request.parameters)?;
    validate_edits(source, &mut edits)?;
    let changed_byte_ranges = changed_ranges(&edits);
    let mut bytes = source.to_vec();
    for edit in edits.iter().rev() {
        bytes.splice(
            edit.range.start..edit.range.end,
            edit.replacement.iter().copied(),
        );
    }

    Ok(MutationResult {
        source_sha256: sha256_hex(source),
        output_sha256: sha256_hex(&bytes),
        bytes,
        contract_version: MUTATION_CONTRACT_VERSION,
        mutation_id: request.parameters.mutation_id(),
        parameters: request.parameters,
        changed_byte_ranges,
        expected_failure_layer: request.expected_failure_layer,
        acceptable_outcomes: request.acceptable_outcomes,
    })
}

fn edits_for(source: &[u8], parameters: &MutationParameters) -> Result<Vec<Edit>, MutationError> {
    let edits = match parameters {
        MutationParameters::Truncate { offset, .. } => vec![Edit {
            range: ByteRange::new(*offset, source.len()),
            replacement: Vec::new(),
        }],
        MutationParameters::IncorrectExplicitVrLength {
            length_field,
            width,
            declared_length,
        }
        | MutationParameters::InvalidPixelByteLength {
            length_field,
            width,
            declared_length,
        } => vec![length_edit(*length_field, *width, *declared_length)?],
        MutationParameters::IllegalVr {
            vr_field,
            replacement,
        } => vec![Edit {
            range: *vr_field,
            replacement: replacement.to_vec(),
        }],
        MutationParameters::TransferSyntaxMismatch {
            file_meta_uid_value,
            replacement,
        } => vec![Edit {
            range: *file_meta_uid_value,
            replacement: replacement.clone(),
        }],
        MutationParameters::UidMismatch {
            dataset_uid_value,
            replacement,
        } => vec![Edit {
            range: *dataset_uid_value,
            replacement: replacement.clone(),
        }],
        MutationParameters::MissingType1Element { element } => vec![Edit {
            range: *element,
            replacement: Vec::new(),
        }],
        MutationParameters::InvalidBitsStoredHighBit {
            bits_stored_value,
            high_bit_value,
            bits_stored,
            high_bit,
        } => vec![
            exact_width_edit(
                *bits_stored_value,
                "Bits Stored",
                &bits_stored.to_le_bytes(),
            )?,
            exact_width_edit(*high_bit_value, "High Bit", &high_bit.to_le_bytes())?,
        ],
        MutationParameters::BrokenBasicOffsetTable { entry, offset } => vec![exact_width_edit(
            *entry,
            "Basic Offset Table entry",
            &offset.to_le_bytes(),
        )?],
        MutationParameters::BrokenExtendedOffsetTable { entry, offset } => vec![exact_width_edit(
            *entry,
            "Extended Offset Table entry",
            &offset.to_le_bytes(),
        )?],
        MutationParameters::UndefinedLengthWithoutDelimitation {
            length_field,
            delimitation_item,
        } => {
            let mut edits = Vec::with_capacity(2);
            if let Some(length_field) = length_field {
                edits.push(exact_width_edit(
                    *length_field,
                    "undefined length field",
                    &u32::MAX.to_le_bytes(),
                )?);
            }
            edits.push(Edit {
                range: *delimitation_item,
                replacement: Vec::new(),
            });
            edits
        }
        MutationParameters::InvalidNestedItemLength {
            length_field,
            declared_length,
        } => vec![exact_width_edit(
            *length_field,
            "nested item length field",
            &declared_length.to_le_bytes(),
        )?],
        MutationParameters::InvalidCharacterSetDeclaration { value, replacement }
        | MutationParameters::MalformedEncodedText { value, replacement } => vec![Edit {
            range: *value,
            replacement: replacement.clone(),
        }],
    };
    Ok(edits)
}

fn length_edit(range: ByteRange, width: LengthWidth, value: u64) -> Result<Edit, MutationError> {
    if range.len() != width.bytes() {
        return Err(MutationError::InvalidLocatorLength {
            name: "length field",
            expected: width.bytes(),
            actual: range.len(),
        });
    }
    let replacement = match width {
        LengthWidth::U16 => u16::try_from(value)
            .map(u16::to_le_bytes)
            .map(Vec::from)
            .map_err(|_| MutationError::ValueDoesNotFit { width, value })?,
        LengthWidth::U32 => u32::try_from(value)
            .map(u32::to_le_bytes)
            .map(Vec::from)
            .map_err(|_| MutationError::ValueDoesNotFit { width, value })?,
        LengthWidth::U64 => value.to_le_bytes().to_vec(),
    };
    Ok(Edit { range, replacement })
}

fn exact_width_edit(
    range: ByteRange,
    name: &'static str,
    replacement: &[u8],
) -> Result<Edit, MutationError> {
    if range.len() != replacement.len() {
        return Err(MutationError::InvalidLocatorLength {
            name,
            expected: replacement.len(),
            actual: range.len(),
        });
    }
    Ok(Edit {
        range,
        replacement: replacement.to_vec(),
    })
}

fn validate_edits(source: &[u8], edits: &mut [Edit]) -> Result<(), MutationError> {
    edits.sort_by_key(|edit| edit.range.start);
    for edit in edits.iter() {
        if edit.range.start > edit.range.end || edit.range.end > source.len() {
            return Err(MutationError::OutOfBounds {
                range: edit.range,
                source_len: source.len(),
            });
        }
        if !edit.replacement.is_empty() && edit.replacement.len() != edit.range.len() {
            return Err(MutationError::ReplacementLength {
                range: edit.range,
                replacement_len: edit.replacement.len(),
            });
        }
        if source.get(edit.range.start..edit.range.end) == Some(edit.replacement.as_slice()) {
            return Err(MutationError::NoChange { range: edit.range });
        }
    }
    for pair in edits.windows(2) {
        if pair[0].range.end > pair[1].range.start {
            return Err(MutationError::OverlappingEdits {
                first: pair[0].range,
                second: pair[1].range,
            });
        }
    }
    Ok(())
}

fn changed_ranges(edits: &[Edit]) -> Vec<ChangedByteRange> {
    let mut delta = 0isize;
    edits
        .iter()
        .map(|edit| {
            let output_start = edit.range.start.saturating_add_signed(delta);
            let output = ByteRange::new(output_start, output_start + edit.replacement.len());
            delta += edit.replacement.len() as isize - edit.range.len() as isize;
            ChangedByteRange {
                source: edit.range,
                output,
            }
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut h = H0;
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut z) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = z
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            (z, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, z]) {
            *slot = slot.wrapping_add(value);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part10() -> Vec<u8> {
        let mut bytes = (0..320)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        bytes[128..132].copy_from_slice(b"DICM");
        bytes
    }

    fn request(parameters: MutationParameters, layer: FailureLayer) -> MutationRequest {
        MutationRequest::new(
            parameters,
            layer,
            vec![
                AcceptableOutcome::CleanRejection,
                AcceptableOutcome::ValidationFailure,
            ],
        )
    }

    #[test]
    fn sha256_implementation_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn every_named_primitive_is_deterministic_and_records_hashes_and_ranges() {
        let source = part10();
        let cases = vec![
            request(
                MutationParameters::IncorrectExplicitVrLength {
                    length_field: ByteRange::new(140, 142),
                    width: LengthWidth::U16,
                    declared_length: 33,
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::IllegalVr {
                    vr_field: ByteRange::new(142, 144),
                    replacement: *b"??",
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::TransferSyntaxMismatch {
                    file_meta_uid_value: ByteRange::new(144, 152),
                    replacement: b"1.2.840\0".to_vec(),
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::UidMismatch {
                    dataset_uid_value: ByteRange::new(152, 160),
                    replacement: b"9.9.999\0".to_vec(),
                },
                FailureLayer::SemanticValidation,
            ),
            request(
                MutationParameters::MissingType1Element {
                    element: ByteRange::new(160, 172),
                },
                FailureLayer::SemanticValidation,
            ),
            request(
                MutationParameters::InvalidBitsStoredHighBit {
                    bits_stored_value: ByteRange::new(172, 174),
                    high_bit_value: ByteRange::new(174, 176),
                    bits_stored: 17,
                    high_bit: 3,
                },
                FailureLayer::PixelDecoding,
            ),
            request(
                MutationParameters::InvalidPixelByteLength {
                    length_field: ByteRange::new(176, 180),
                    width: LengthWidth::U32,
                    declared_length: 1,
                },
                FailureLayer::PixelDecoding,
            ),
            request(
                MutationParameters::BrokenBasicOffsetTable {
                    entry: ByteRange::new(180, 184),
                    offset: u32::MAX,
                },
                FailureLayer::Encapsulation,
            ),
            request(
                MutationParameters::BrokenExtendedOffsetTable {
                    entry: ByteRange::new(184, 192),
                    offset: u64::MAX,
                },
                FailureLayer::Encapsulation,
            ),
            request(
                MutationParameters::UndefinedLengthWithoutDelimitation {
                    length_field: Some(ByteRange::new(192, 196)),
                    delimitation_item: ByteRange::new(220, 228),
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::InvalidNestedItemLength {
                    length_field: ByteRange::new(196, 200),
                    declared_length: u32::MAX - 1,
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::InvalidCharacterSetDeclaration {
                    value: ByteRange::new(200, 208),
                    replacement: b"INVALID!".to_vec(),
                },
                FailureLayer::TextDecoding,
            ),
            request(
                MutationParameters::MalformedEncodedText {
                    value: ByteRange::new(208, 212),
                    replacement: vec![0xff, 0xfe, 0xff, 0xfe],
                },
                FailureLayer::TextDecoding,
            ),
        ];

        for case in cases {
            let first =
                apply_named_mutation(&source, case.clone()).expect(case.parameters.mutation_id());
            let second = apply_named_mutation(&source, case).expect("repeat mutation");
            assert_eq!(first, second);
            assert_eq!(first.contract_version, MUTATION_CONTRACT_VERSION);
            assert_eq!(first.source_sha256, sha256_hex(&source));
            assert_eq!(first.output_sha256, sha256_hex(&first.bytes));
            assert_ne!(first.source_sha256, first.output_sha256);
            assert!(!first.changed_byte_ranges.is_empty());
            assert!(!first.acceptable_outcomes.is_empty());
        }
    }

    #[test]
    fn every_truncation_target_uses_the_exact_removed_suffix() {
        let source = part10();
        for target in [
            TruncationTarget::FileMeta,
            TruncationTarget::Dataset,
            TruncationTarget::Sequence,
            TruncationTarget::Item,
            TruncationTarget::Fragment,
            TruncationTarget::PixelValue,
        ] {
            let result = apply_named_mutation(
                &source,
                request(
                    MutationParameters::Truncate {
                        target,
                        offset: 240,
                    },
                    FailureLayer::DatasetParser,
                ),
            )
            .expect("checked truncation");
            assert_eq!(result.bytes, source[..240]);
            assert_eq!(
                result.changed_byte_ranges,
                vec![ChangedByteRange {
                    source: ByteRange::new(240, 320),
                    output: ByteRange::new(240, 240),
                }]
            );
            assert!(result.mutation_id.starts_with("truncate_"));
        }
    }

    #[test]
    fn multi_edit_ranges_remain_in_source_coordinates() {
        let source = part10();
        let result = apply_named_mutation(
            &source,
            request(
                MutationParameters::UndefinedLengthWithoutDelimitation {
                    length_field: Some(ByteRange::new(180, 184)),
                    delimitation_item: ByteRange::new(240, 248),
                },
                FailureLayer::DatasetParser,
            ),
        )
        .expect("undefined length mutation");
        assert_eq!(
            result.changed_byte_ranges,
            vec![
                ChangedByteRange {
                    source: ByteRange::new(180, 184),
                    output: ByteRange::new(180, 184),
                },
                ChangedByteRange {
                    source: ByteRange::new(240, 248),
                    output: ByteRange::new(240, 240),
                },
            ]
        );
        assert_eq!(result.bytes.len(), source.len() - 8);
    }

    #[test]
    fn checked_errors_reject_unsafe_or_ambiguous_requests() {
        let source = part10();
        assert_eq!(
            apply_named_mutation(
                b"not part 10",
                request(
                    MutationParameters::Truncate {
                        target: TruncationTarget::Dataset,
                        offset: 2,
                    },
                    FailureLayer::DatasetParser,
                )
            ),
            Err(MutationError::NotPart10)
        );

        let mut unsupported = request(
            MutationParameters::IllegalVr {
                vr_field: ByteRange::new(140, 142),
                replacement: *b"??",
            },
            FailureLayer::DatasetParser,
        );
        unsupported.contract_version = "9.9.9";
        assert!(matches!(
            apply_named_mutation(&source, unsupported),
            Err(MutationError::UnsupportedContractVersion { .. })
        ));

        let empty_outcomes = MutationRequest::new(
            MutationParameters::IllegalVr {
                vr_field: ByteRange::new(140, 142),
                replacement: *b"??",
            },
            FailureLayer::DatasetParser,
            Vec::new(),
        );
        assert_eq!(
            apply_named_mutation(&source, empty_outcomes),
            Err(MutationError::EmptyAcceptableOutcomes)
        );

        let errors = [
            request(
                MutationParameters::Truncate {
                    target: TruncationTarget::Dataset,
                    offset: source.len() + 1,
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::IncorrectExplicitVrLength {
                    length_field: ByteRange::new(140, 143),
                    width: LengthWidth::U16,
                    declared_length: 2,
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::IncorrectExplicitVrLength {
                    length_field: ByteRange::new(140, 142),
                    width: LengthWidth::U16,
                    declared_length: u16::MAX as u64 + 1,
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::TransferSyntaxMismatch {
                    file_meta_uid_value: ByteRange::new(140, 148),
                    replacement: b"short".to_vec(),
                },
                FailureLayer::DatasetParser,
            ),
            request(
                MutationParameters::InvalidBitsStoredHighBit {
                    bits_stored_value: ByteRange::new(140, 142),
                    high_bit_value: ByteRange::new(141, 143),
                    bits_stored: 17,
                    high_bit: 3,
                },
                FailureLayer::PixelDecoding,
            ),
        ];
        for bad_request in errors {
            assert!(apply_named_mutation(&source, bad_request).is_err());
        }

        let unchanged = MutationRequest::new(
            MutationParameters::MalformedEncodedText {
                value: ByteRange::new(140, 144),
                replacement: source[140..144].to_vec(),
            },
            FailureLayer::TextDecoding,
            vec![AcceptableOutcome::DecodeFailure],
        );
        assert!(matches!(
            apply_named_mutation(&source, unchanged),
            Err(MutationError::NoChange { .. })
        ));
    }
}
