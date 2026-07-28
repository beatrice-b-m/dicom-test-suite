use std::error::Error;
use std::fmt;

use crate::sha256_hex;

const ITEM_TAG_BYTES: [u8; 4] = [0xfe, 0xff, 0x00, 0xe0];
const SEQUENCE_DELIMITATION_ITEM_BYTES: [u8; 8] = [0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicOffsetTablePolicy {
    Empty,
    Populated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncapsulatedPixelData {
    pub basic_offset_table: BasicOffsetTable,
    pub fragments: Vec<EncapsulatedFragment>,
    pub fragment_payloads: Vec<Vec<u8>>,
    pub fragments_per_frame: Vec<usize>,
    pub compressed_frame_hashes: Vec<String>,
    pub value_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicOffsetTable {
    pub offsets: Vec<u32>,
}

impl BasicOffsetTable {
    pub fn is_populated(&self) -> bool {
        !self.offsets.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncapsulatedFragment {
    pub frame_index: usize,
    pub item_start_offset: u32,
    pub compressed_length: usize,
    pub padded_length: usize,
}

impl EncapsulatedPixelData {
    pub fn one_fragment_per_frame(
        frames: &[Vec<u8>],
        basic_offset_table_policy: BasicOffsetTablePolicy,
    ) -> Result<Self, EncapsulationError> {
        let fragments_per_frame = vec![1; frames.len()];
        encapsulate_frames(frames, &fragments_per_frame, basic_offset_table_policy)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncapsulationError {
    NoFrames,
    FragmentLayoutFrameMismatch {
        frames: usize,
        layout_entries: usize,
    },
    ZeroFragments {
        frame_index: usize,
    },
    InsufficientFrameBytes {
        frame_index: usize,
        frame_length: usize,
        fragment_count: usize,
    },
    OffsetOverflow,
    ItemLengthOverflow {
        length: usize,
    },
}

impl fmt::Display for EncapsulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => write!(f, "encapsulated Pixel Data requires at least one frame"),
            Self::FragmentLayoutFrameMismatch {
                frames,
                layout_entries,
            } => write!(
                f,
                "fragment layout has {layout_entries} entries for {frames} compressed frames"
            ),
            Self::ZeroFragments { frame_index } => {
                write!(f, "frame {frame_index} must contain at least one fragment")
            }
            Self::InsufficientFrameBytes {
                frame_index,
                frame_length,
                fragment_count,
            } => write!(
                f,
                "frame {frame_index} has {frame_length} bytes, insufficient for {fragment_count} even-length fragments"
            ),
            Self::OffsetOverflow => write!(f, "encapsulated Pixel Data item offset exceeded u32"),
            Self::ItemLengthOverflow { length } => write!(
                f,
                "encapsulated Pixel Data item length {length} exceeded u32"
            ),
        }
    }
}

impl Error for EncapsulationError {}

pub fn encapsulate_frames(
    frames: &[Vec<u8>],
    fragments_per_frame: &[usize],
    basic_offset_table_policy: BasicOffsetTablePolicy,
) -> Result<EncapsulatedPixelData, EncapsulationError> {
    if frames.is_empty() {
        return Err(EncapsulationError::NoFrames);
    }
    if frames.len() != fragments_per_frame.len() {
        return Err(EncapsulationError::FragmentLayoutFrameMismatch {
            frames: frames.len(),
            layout_entries: fragments_per_frame.len(),
        });
    }

    let mut frame_fragments = Vec::with_capacity(frames.len());
    for (frame_index, (frame, fragment_count)) in frames.iter().zip(fragments_per_frame).enumerate()
    {
        if *fragment_count == 0 {
            return Err(EncapsulationError::ZeroFragments { frame_index });
        }
        frame_fragments.push(split_frame_at_even_boundaries(
            frame,
            *fragment_count,
            frame_index,
        )?);
    }

    let bot_payload_len = match basic_offset_table_policy {
        BasicOffsetTablePolicy::Empty => 0usize,
        BasicOffsetTablePolicy::Populated => frames
            .len()
            .checked_mul(4)
            .ok_or(EncapsulationError::OffsetOverflow)?,
    };

    let mut value_bytes = Vec::new();
    append_item(&mut value_bytes, &vec![0u8; bot_payload_len])?;
    let first_fragment_item_start = value_bytes.len();

    let mut basic_offsets = Vec::new();
    let mut fragments = Vec::new();
    let mut fragment_payloads = Vec::new();
    let compressed_frame_hashes = frames.iter().map(|frame| sha256_hex(frame)).collect();

    for (frame_index, frame_parts) in frame_fragments.iter().enumerate() {
        let first_fragment_offset =
            u32::try_from(value_bytes.len()).map_err(|_| EncapsulationError::OffsetOverflow)?;
        if matches!(basic_offset_table_policy, BasicOffsetTablePolicy::Populated) {
            let bot_offset = first_fragment_offset
                .checked_sub(
                    u32::try_from(first_fragment_item_start)
                        .map_err(|_| EncapsulationError::OffsetOverflow)?,
                )
                .ok_or(EncapsulationError::OffsetOverflow)?;
            basic_offsets.push(bot_offset);
        }

        for fragment in frame_parts {
            let item_start_offset =
                u32::try_from(value_bytes.len()).map_err(|_| EncapsulationError::OffsetOverflow)?;
            let compressed_length = fragment.len();
            append_item(&mut value_bytes, fragment)?;
            let padded_length = padded_item_value_length(compressed_length);
            fragments.push(EncapsulatedFragment {
                frame_index,
                item_start_offset,
                compressed_length,
                padded_length,
            });
            fragment_payloads.push(fragment.to_vec());
        }
    }
    value_bytes.extend_from_slice(&SEQUENCE_DELIMITATION_ITEM_BYTES);

    if matches!(basic_offset_table_policy, BasicOffsetTablePolicy::Populated) {
        write_basic_offset_table(&mut value_bytes, &basic_offsets);
    }

    Ok(EncapsulatedPixelData {
        basic_offset_table: BasicOffsetTable {
            offsets: basic_offsets,
        },
        fragments,
        fragment_payloads,
        fragments_per_frame: fragments_per_frame.to_vec(),
        compressed_frame_hashes,
        value_bytes,
    })
}

fn split_frame_at_even_boundaries(
    frame: &[u8],
    fragment_count: usize,
    frame_index: usize,
) -> Result<Vec<&[u8]>, EncapsulationError> {
    let even_units = frame.len() / 2;
    if even_units < fragment_count {
        return Err(EncapsulationError::InsufficientFrameBytes {
            frame_index,
            frame_length: frame.len(),
            fragment_count,
        });
    }

    let base_units = even_units / fragment_count;
    let remainder_units = even_units % fragment_count;
    let mut offset = 0usize;
    let mut fragments = Vec::with_capacity(fragment_count);
    for index in 0..fragment_count {
        let mut len = (base_units + usize::from(index < remainder_units)) * 2;
        if index + 1 == fragment_count {
            len += frame.len() % 2;
        }
        fragments.push(&frame[offset..offset + len]);
        offset += len;
    }
    Ok(fragments)
}

fn append_item(value_bytes: &mut Vec<u8>, payload: &[u8]) -> Result<(), EncapsulationError> {
    let padded_len = padded_item_value_length(payload.len());
    let padded_len_u32 = u32::try_from(padded_len)
        .map_err(|_| EncapsulationError::ItemLengthOverflow { length: padded_len })?;

    value_bytes.extend_from_slice(&ITEM_TAG_BYTES);
    value_bytes.extend_from_slice(&padded_len_u32.to_le_bytes());
    value_bytes.extend_from_slice(payload);
    if payload.len() != padded_len {
        value_bytes.push(0);
    }
    Ok(())
}

fn padded_item_value_length(length: usize) -> usize {
    length + (length % 2)
}

fn write_basic_offset_table(value_bytes: &mut [u8], offsets: &[u32]) {
    let mut cursor = 8usize;
    for offset in offsets {
        value_bytes[cursor..cursor + 4].copy_from_slice(&offset.to_le_bytes());
        cursor += 4;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};

    #[test]
    fn one_fragment_per_frame_with_empty_basic_offset_table_pads_odd_frame_items() {
        let frame = vec![0xaa, 0xbb, 0xcc];

        let encoded = EncapsulatedPixelData::one_fragment_per_frame(
            &[frame.clone()],
            BasicOffsetTablePolicy::Empty,
        )
        .expect("single odd-length frame should encapsulate");

        assert!(!encoded.basic_offset_table.is_populated());
        assert_eq!(encoded.fragments_per_frame, vec![1]);
        assert_eq!(encoded.compressed_frame_hashes, vec![sha256_hex(&frame)]);
        assert_eq!(encoded.fragments[0].compressed_length, 3);
        assert_eq!(encoded.fragments[0].padded_length, 4);
        assert_eq!(
            encoded.value_bytes,
            vec![
                0xfe, 0xff, 0x00, 0xe0, 0, 0, 0, 0, 0xfe, 0xff, 0x00, 0xe0, 4, 0, 0, 0, 0xaa, 0xbb,
                0xcc, 0, 0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn populated_basic_offset_table_points_to_first_fragment_item_headers() {
        let frames = vec![vec![1, 2], vec![3, 4, 5, 6]];

        let encoded = EncapsulatedPixelData::one_fragment_per_frame(
            &frames,
            BasicOffsetTablePolicy::Populated,
        )
        .expect("frames should encapsulate with populated offsets");

        assert_eq!(encoded.basic_offset_table.offsets, vec![0, 10]);
        assert_eq!(encoded.fragments[0].item_start_offset, 16);
        assert_eq!(encoded.fragments[1].item_start_offset, 26);
        assert_eq!(&encoded.value_bytes[8..12], &0u32.to_le_bytes());
        assert_eq!(&encoded.value_bytes[12..16], &10u32.to_le_bytes());
    }

    #[test]
    fn multi_fragment_layout_tracks_frame_fragment_counts() {
        let frames = vec![vec![1, 2, 3, 4, 5], vec![6, 7, 8, 9]];

        let encoded = encapsulate_frames(&frames, &[2, 2], BasicOffsetTablePolicy::Empty)
            .expect("multi-fragment frames should encapsulate");

        assert_eq!(encoded.fragments_per_frame, vec![2, 2]);
        assert_eq!(encoded.fragments.len(), 4);
        assert_eq!(
            encoded
                .fragments
                .iter()
                .map(|fragment| fragment.frame_index)
                .collect::<Vec<_>>(),
            vec![0, 0, 1, 1]
        );
        assert_eq!(
            encoded
                .fragments
                .iter()
                .map(|fragment| fragment.compressed_length)
                .collect::<Vec<_>>(),
            vec![2, 3, 2, 2]
        );
        assert_eq!(
            encoded.fragment_payloads,
            vec![vec![1, 2], vec![3, 4, 5], vec![6, 7], vec![8, 9]]
        );
    }

    #[test]
    fn rle_encoded_frame_can_be_wrapped_as_one_fragment() {
        let rle_frame = NativeRleLosslessEncoder::new()
            .encode_frame(FrameEncodeInput {
                native_frame: &[7, 7, 7, 7],
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should encode the native frame");

        let encoded = EncapsulatedPixelData::one_fragment_per_frame(
            &[rle_frame.bytes.clone()],
            BasicOffsetTablePolicy::Populated,
        )
        .expect("RLE frame should encapsulate");

        assert_eq!(encoded.basic_offset_table.offsets, vec![0]);
        assert_eq!(encoded.fragments_per_frame, vec![1]);
        assert_eq!(
            encoded.compressed_frame_hashes,
            vec![sha256_hex(&rle_frame.bytes)]
        );
        assert_eq!(
            encoded.fragments[0].compressed_length,
            rle_frame.bytes.len()
        );
        assert_eq!(encoded.value_bytes[12..16], ITEM_TAG_BYTES);
    }

    #[test]
    fn rejects_empty_frame_list_and_invalid_fragment_layouts() {
        assert_eq!(
            EncapsulatedPixelData::one_fragment_per_frame(&[], BasicOffsetTablePolicy::Empty)
                .expect_err("empty frame list should be rejected"),
            EncapsulationError::NoFrames
        );
        assert_eq!(
            encapsulate_frames(&[vec![1]], &[], BasicOffsetTablePolicy::Empty)
                .expect_err("layout length mismatch should be rejected"),
            EncapsulationError::FragmentLayoutFrameMismatch {
                frames: 1,
                layout_entries: 0,
            }
        );
        assert_eq!(
            encapsulate_frames(&[vec![1]], &[0], BasicOffsetTablePolicy::Empty)
                .expect_err("zero fragments should be rejected"),
            EncapsulationError::ZeroFragments { frame_index: 0 }
        );
        assert_eq!(
            encapsulate_frames(&[vec![1, 2, 3]], &[2], BasicOffsetTablePolicy::Empty)
                .expect_err("each fragment needs at least two source bytes"),
            EncapsulationError::InsufficientFrameBytes {
                frame_index: 0,
                frame_length: 3,
                fragment_count: 2,
            }
        );
    }
}
