use std::error::Error;
use std::fmt;

use crate::PACKAGE_VERSION;

#[cfg(feature = "jpeg")]
use std::borrow::Cow;

#[cfg(feature = "jpeg")]
use dicom_core::value::C;
#[cfg(feature = "jpeg")]
use dicom_encoding::{
    Codec,
    adapters::{EncodeOptions, PixelDataObject, PixelDataReader, PixelDataWriter, RawPixelData},
};
#[cfg(feature = "jpeg")]
use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;

pub const JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.50";
pub const RLE_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.5";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecBackendKind {
    Native,
    DicomRsFeature,
    ExternalCommand,
}

impl CodecBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::DicomRsFeature => "dicom_rs_feature",
            Self::ExternalCommand => "external_command",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecDeterminism {
    ByteStable,
    SemanticStable,
    Unstable,
}

impl CodecDeterminism {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ByteStable => "byte_stable",
            Self::SemanticStable => "semantic_stable",
            Self::Unstable => "unstable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecBackendInfo {
    pub backend_id: &'static str,
    pub backend_kind: CodecBackendKind,
    pub display_name: &'static str,
    pub version: &'static str,
    pub transfer_syntax_uid: &'static str,
    pub feature_gate: Option<&'static str>,
    pub determinism: CodecDeterminism,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameEncodeInput<'a> {
    pub native_frame: &'a [u8],
    pub rows: u16,
    pub columns: u16,
    pub samples_per_pixel: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub photometric_interpretation: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub bytes: Vec<u8>,
}

pub trait FrameEncoder {
    fn backend(&self) -> CodecBackendInfo;

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDecodeInput<'a> {
    pub encoded_frame: &'a [u8],
    pub rows: u16,
    pub columns: u16,
    pub samples_per_pixel: u16,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub photometric_interpretation: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub native_bytes: Vec<u8>,
}

pub trait FrameDecoder {
    fn backend(&self) -> CodecBackendInfo;

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Unavailable {
        backend_id: &'static str,
        reason: String,
    },
    Unsupported {
        backend_id: &'static str,
        message: String,
    },
    EncodeFailed {
        backend_id: &'static str,
        message: String,
    },
    ValidationFailed {
        backend_id: &'static str,
        message: String,
    },
}

impl CodecError {
    pub fn unavailable(backend_id: &'static str, reason: impl Into<String>) -> Self {
        Self::Unavailable {
            backend_id,
            reason: reason.into(),
        }
    }

    pub fn unsupported(backend_id: &'static str, message: impl Into<String>) -> Self {
        Self::Unsupported {
            backend_id,
            message: message.into(),
        }
    }

    pub fn encode_failed(backend_id: &'static str, message: impl Into<String>) -> Self {
        Self::EncodeFailed {
            backend_id,
            message: message.into(),
        }
    }

    pub fn validation_failed(backend_id: &'static str, message: impl Into<String>) -> Self {
        Self::ValidationFailed {
            backend_id,
            message: message.into(),
        }
    }

    pub fn backend_id(&self) -> &'static str {
        match self {
            Self::Unavailable { backend_id, .. }
            | Self::Unsupported { backend_id, .. }
            | Self::EncodeFailed { backend_id, .. }
            | Self::ValidationFailed { backend_id, .. } => backend_id,
        }
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { backend_id, reason } => {
                write!(f, "codec backend {backend_id} is unavailable: {reason}")
            }
            Self::Unsupported {
                backend_id,
                message,
            } => write!(
                f,
                "codec backend {backend_id} does not support input: {message}"
            ),
            Self::EncodeFailed {
                backend_id,
                message,
            } => write!(f, "codec backend {backend_id} failed to encode: {message}"),
            Self::ValidationFailed {
                backend_id,
                message,
            } => write!(
                f,
                "codec backend {backend_id} failed encoded output validation: {message}"
            ),
        }
    }
}

impl Error for CodecError {}

#[derive(Debug, Clone, Copy, Default)]
#[cfg(feature = "jpeg")]
pub struct DicomRsJpegBaselineEncoder;

#[cfg(feature = "jpeg")]
impl DicomRsJpegBaselineEncoder {
    pub const BACKEND_ID: &'static str = "dicom_rs_jpeg_baseline_writer";

    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "jpeg")]
impl FrameEncoder for DicomRsJpegBaselineEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::DicomRsFeature,
            display_name: "DICOM-rs JPEG Baseline writer",
            version: "dicom-transfer-syntax-registry 0.9.1",
            transfer_syntax_uid: JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID,
            feature_gate: Some("jpeg"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 8 || input.bits_stored != 8 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG Baseline case support is limited to 8-bit source frames",
            ));
        }
        if input.samples_per_pixel != 3 || input.photometric_interpretation != "RGB" {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG Baseline case support currently requires RGB input",
            ));
        }

        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            rows: input.rows,
            columns: input.columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.native_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(_, Some(writer)) = JPEG_BASELINE.codec() else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG Baseline writer is not available",
            ));
        };

        let mut options = EncodeOptions::default();
        options.quality = Some(95);
        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, options, &mut encoded)
            .map_err(|err| CodecError::encode_failed(Self::BACKEND_ID, err.to_string()))?;

        if encoded.len() < 4
            || encoded[..2] != [0xff, 0xd8]
            || encoded[encoded.len() - 2..] != [0xff, 0xd9]
        {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "JPEG codestream is missing SOI or EOI markers",
            ));
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "jpeg")]
impl FrameDecoder for DicomRsJpegBaselineEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID,
            rows: input.rows,
            columns: input.columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.encoded_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) = JPEG_BASELINE.codec() else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG Baseline reader is not available",
            ));
        };

        let mut decoded = Vec::new();
        reader
            .decode_frame(&obj, 0, &mut decoded)
            .map_err(|err| CodecError::validation_failed(Self::BACKEND_ID, err.to_string()))?;

        Ok(DecodedFrame {
            native_bytes: decoded,
        })
    }
}

#[cfg(feature = "jpeg")]
struct DicomRsPixelDataObject<'a> {
    transfer_syntax_uid: &'a str,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &'a str,
    fragments: Vec<Vec<u8>>,
    offset_table: Vec<u32>,
}

#[cfg(feature = "jpeg")]
impl PixelDataObject for DicomRsPixelDataObject<'_> {
    fn transfer_syntax_uid(&self) -> &str {
        self.transfer_syntax_uid
    }

    fn rows(&self) -> Option<u16> {
        Some(self.rows)
    }

    fn cols(&self) -> Option<u16> {
        Some(self.columns)
    }

    fn samples_per_pixel(&self) -> Option<u16> {
        Some(self.samples_per_pixel)
    }

    fn bits_allocated(&self) -> Option<u16> {
        Some(self.bits_allocated)
    }

    fn bits_stored(&self) -> Option<u16> {
        Some(self.bits_stored)
    }

    fn photometric_interpretation(&self) -> Option<&str> {
        Some(self.photometric_interpretation)
    }

    fn number_of_frames(&self) -> Option<u32> {
        Some(u32::try_from(self.fragments.len()).unwrap_or(u32::MAX))
    }

    fn number_of_fragments(&self) -> Option<u32> {
        Some(u32::try_from(self.fragments.len()).unwrap_or(u32::MAX))
    }

    fn fragment(&self, fragment: usize) -> Option<Cow<'_, [u8]>> {
        self.fragments
            .get(fragment)
            .map(|fragment| Cow::Borrowed(fragment.as_slice()))
    }

    fn offset_table(&self) -> Option<Cow<'_, [u32]>> {
        Some(Cow::Borrowed(&self.offset_table))
    }

    fn raw_pixel_data(&self) -> Option<RawPixelData> {
        Some(RawPixelData {
            fragments: C::from_vec(self.fragments.clone()),
            offset_table: C::from_vec(self.offset_table.clone()),
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRleLosslessEncoder;

impl NativeRleLosslessEncoder {
    pub const BACKEND_ID: &'static str = "native_project_rle_encoder";

    pub fn new() -> Self {
        Self
    }
}

impl FrameEncoder for NativeRleLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::Native,
            display_name: "Native project RLE Lossless encoder",
            version: PACKAGE_VERSION,
            transfer_syntax_uid: RLE_LOSSLESS_TRANSFER_SYNTAX_UID,
            feature_gate: None,
            determinism: CodecDeterminism::ByteStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        let bytes_per_sample = checked_bytes_per_sample(Self::BACKEND_ID, input.bits_allocated)?;
        let pixels = usize::from(input.rows) * usize::from(input.columns);
        let samples_per_pixel = usize::from(input.samples_per_pixel);
        let segments = samples_per_pixel
            .checked_mul(bytes_per_sample)
            .ok_or_else(|| {
                CodecError::unsupported(Self::BACKEND_ID, "sample byte segment count overflowed")
            })?;
        if segments == 0 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "at least one RLE segment is required",
            ));
        }
        if segments > 15 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                format!("RLE allows at most 15 segments, got {segments}"),
            ));
        }

        let expected_len = pixels
            .checked_mul(samples_per_pixel)
            .and_then(|len| len.checked_mul(bytes_per_sample))
            .ok_or_else(|| {
                CodecError::unsupported(Self::BACKEND_ID, "native frame length overflowed")
            })?;
        if input.native_frame.len() != expected_len {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                format!(
                    "native frame length is {}, expected {expected_len}",
                    input.native_frame.len()
                ),
            ));
        }

        let mut segment_payloads = Vec::with_capacity(segments);
        for sample in 0..samples_per_pixel {
            for byte_plane in 0..bytes_per_sample {
                let mut segment = Vec::with_capacity(pixels);
                for pixel in 0..pixels {
                    let offset =
                        ((pixel * samples_per_pixel + sample) * bytes_per_sample) + byte_plane;
                    segment.push(input.native_frame[offset]);
                }
                segment_payloads.push(encode_packbits_segment(&segment));
            }
        }

        let mut encoded = vec![0u8; 64];
        encoded[0..4].copy_from_slice(&(segments as u32).to_le_bytes());
        let mut next_offset = 64usize;
        for (index, segment) in segment_payloads.iter().enumerate() {
            let offset_bytes = u32::try_from(next_offset)
                .map_err(|_| {
                    CodecError::encode_failed(Self::BACKEND_ID, "RLE segment offset exceeded u32")
                })?
                .to_le_bytes();
            let header_start = 4 + (index * 4);
            encoded[header_start..header_start + 4].copy_from_slice(&offset_bytes);
            next_offset = next_offset.checked_add(segment.len()).ok_or_else(|| {
                CodecError::encode_failed(Self::BACKEND_ID, "RLE payload length overflowed")
            })?;
        }
        for segment in segment_payloads {
            encoded.extend_from_slice(&segment);
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

impl FrameDecoder for NativeRleLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let bytes_per_sample = checked_bytes_per_sample(Self::BACKEND_ID, input.bits_allocated)?;
        let pixels = usize::from(input.rows) * usize::from(input.columns);
        let samples_per_pixel = usize::from(input.samples_per_pixel);
        let expected_segments =
            samples_per_pixel
                .checked_mul(bytes_per_sample)
                .ok_or_else(|| {
                    CodecError::unsupported(
                        Self::BACKEND_ID,
                        "sample byte segment count overflowed",
                    )
                })?;
        if input.encoded_frame.len() < 64 {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                format!(
                    "RLE frame header is {} bytes, expected at least 64",
                    input.encoded_frame.len()
                ),
            ));
        }

        let segment_count = read_u32_le(input.encoded_frame, 0) as usize;
        if segment_count != expected_segments {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                format!("RLE frame has {segment_count} segments, expected {expected_segments}"),
            ));
        }
        if segment_count == 0 || segment_count > 15 {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                format!("RLE segment count {segment_count} is outside 1..=15"),
            ));
        }

        let mut segment_offsets = Vec::with_capacity(segment_count);
        for index in 0..segment_count {
            let offset = read_u32_le(input.encoded_frame, 4 + index * 4) as usize;
            if offset < 64 || offset > input.encoded_frame.len() {
                return Err(CodecError::validation_failed(
                    Self::BACKEND_ID,
                    format!("RLE segment {index} offset {offset} is outside the frame payload"),
                ));
            }
            if index > 0 && offset < segment_offsets[index - 1] {
                return Err(CodecError::validation_failed(
                    Self::BACKEND_ID,
                    format!("RLE segment {index} offset {offset} is before the previous segment"),
                ));
            }
            segment_offsets.push(offset);
        }

        let mut decoded_segments = Vec::with_capacity(segment_count);
        for index in 0..segment_count {
            let start = segment_offsets[index];
            let end = segment_offsets
                .get(index + 1)
                .copied()
                .unwrap_or(input.encoded_frame.len());
            let segment = decode_packbits_segment(&input.encoded_frame[start..end], pixels)?;
            decoded_segments.push(segment);
        }

        let native_len = pixels
            .checked_mul(samples_per_pixel)
            .and_then(|len| len.checked_mul(bytes_per_sample))
            .ok_or_else(|| {
                CodecError::validation_failed(Self::BACKEND_ID, "native frame length overflowed")
            })?;
        let mut native_bytes = vec![0u8; native_len];
        for sample in 0..samples_per_pixel {
            for byte_plane in 0..bytes_per_sample {
                let segment_index = sample * bytes_per_sample + byte_plane;
                for pixel in 0..pixels {
                    let offset =
                        ((pixel * samples_per_pixel + sample) * bytes_per_sample) + byte_plane;
                    native_bytes[offset] = decoded_segments[segment_index][pixel];
                }
            }
        }

        Ok(DecodedFrame { native_bytes })
    }
}

fn checked_bytes_per_sample(
    backend_id: &'static str,
    bits_allocated: u16,
) -> Result<usize, CodecError> {
    if bits_allocated == 0 || bits_allocated % 8 != 0 {
        return Err(CodecError::unsupported(
            backend_id,
            format!("bits_allocated must be a non-zero multiple of 8, got {bits_allocated}"),
        ));
    }
    Ok(usize::from(bits_allocated / 8))
}

fn encode_packbits_segment(segment: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    let mut literal = Vec::new();
    let mut index = 0usize;

    while index < segment.len() {
        let run_len = repeated_run_len(segment, index);
        if run_len >= 3 {
            flush_literal(&mut encoded, &mut literal);
            let mut remaining = run_len;
            while remaining > 0 {
                let packet_len = remaining.min(128);
                encoded.push((257u16 - packet_len as u16) as u8);
                encoded.push(segment[index]);
                remaining -= packet_len;
            }
            index += run_len;
        } else {
            literal.push(segment[index]);
            if literal.len() == 128 {
                flush_literal(&mut encoded, &mut literal);
            }
            index += 1;
        }
    }
    flush_literal(&mut encoded, &mut literal);

    encoded
}

fn repeated_run_len(segment: &[u8], start: usize) -> usize {
    let value = segment[start];
    let mut len = 1usize;
    while start + len < segment.len() && segment[start + len] == value && len < 128 {
        len += 1;
    }
    len
}

fn flush_literal(encoded: &mut Vec<u8>, literal: &mut Vec<u8>) {
    if literal.is_empty() {
        return;
    }
    encoded.push((literal.len() - 1) as u8);
    encoded.extend_from_slice(literal);
    literal.clear();
}

fn decode_packbits_segment(encoded: &[u8], expected_len: usize) -> Result<Vec<u8>, CodecError> {
    let mut decoded = Vec::with_capacity(expected_len);
    let mut index = 0usize;
    while index < encoded.len() && decoded.len() < expected_len {
        let header = encoded[index] as i8;
        index += 1;
        if header >= 0 {
            let literal_len = usize::from(header as u8) + 1;
            let end = index.checked_add(literal_len).ok_or_else(|| {
                CodecError::validation_failed(
                    NativeRleLosslessEncoder::BACKEND_ID,
                    "RLE literal packet length overflowed",
                )
            })?;
            if end > encoded.len() {
                return Err(CodecError::validation_failed(
                    NativeRleLosslessEncoder::BACKEND_ID,
                    "RLE literal packet is truncated",
                ));
            }
            decoded.extend_from_slice(&encoded[index..end]);
            index = end;
        } else if header >= -127 {
            if index >= encoded.len() {
                return Err(CodecError::validation_failed(
                    NativeRleLosslessEncoder::BACKEND_ID,
                    "RLE repeat packet is missing its sample byte",
                ));
            }
            let repeat_len = usize::try_from(1i16 - i16::from(header)).map_err(|_| {
                CodecError::validation_failed(
                    NativeRleLosslessEncoder::BACKEND_ID,
                    "RLE repeat packet length overflowed",
                )
            })?;
            let value = encoded[index];
            index += 1;
            decoded.extend(std::iter::repeat_n(value, repeat_len));
        }
    }

    if decoded.len() != expected_len {
        return Err(CodecError::validation_failed(
            NativeRleLosslessEncoder::BACKEND_ID,
            format!(
                "RLE segment decoded to {} bytes, expected {expected_len}",
                decoded.len()
            ),
        ));
    }
    Ok(decoded)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "jpeg")]
    use std::borrow::Cow;

    #[cfg(feature = "jpeg")]
    use dicom_core::value::C;
    #[cfg(feature = "jpeg")]
    use dicom_encoding::{
        Codec,
        adapters::{EncodeOptions, PixelDataObject, PixelDataWriter, RawPixelData},
    };
    #[cfg(feature = "jpeg")]
    use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;

    #[test]
    fn native_rle_backend_reports_identity_and_determinism() {
        let encoder = NativeRleLosslessEncoder::new();

        let backend = FrameEncoder::backend(&encoder);

        assert_eq!(backend.backend_id, "native_project_rle_encoder");
        assert_eq!(backend.backend_kind.as_str(), "native");
        assert_eq!(backend.transfer_syntax_uid, "1.2.840.10008.1.2.5");
        assert_eq!(backend.feature_gate, None);
        assert_eq!(backend.determinism.as_str(), "byte_stable");
    }

    #[test]
    fn native_rle_encodes_single_segment_literal_frame() {
        let encoder = NativeRleLosslessEncoder::new();

        let encoded = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &[0, 85, 170, 255],
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should encode a tiny 8-bit frame");

        assert_eq!(&encoded.bytes[0..4], &1u32.to_le_bytes());
        assert_eq!(&encoded.bytes[4..8], &64u32.to_le_bytes());
        assert_eq!(&encoded.bytes[8..64], &[0u8; 56]);
        assert_eq!(&encoded.bytes[64..], &[3, 0, 85, 170, 255]);
    }

    #[test]
    fn native_rle_decodes_single_segment_literal_frame() {
        let codec = NativeRleLosslessEncoder::new();
        let encoded = [1, 0, 0, 0, 64, 0, 0, 0]
            .iter()
            .copied()
            .chain([0; 56])
            .chain([3, 0, 85, 170, 255])
            .collect::<Vec<_>>();

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded,
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("literal RLE frame should decode");

        assert_eq!(decoded.native_bytes, vec![0, 85, 170, 255]);
    }

    #[test]
    fn native_rle_encodes_repeated_runs_deterministically() {
        let encoder = NativeRleLosslessEncoder::new();

        let encoded = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &[7, 7, 7, 7],
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should encode repeated 8-bit samples");

        assert_eq!(&encoded.bytes[64..], &[253, 7]);
    }

    #[test]
    fn native_rle_decodes_repeated_runs() {
        let codec = NativeRleLosslessEncoder::new();
        let mut encoded = vec![0u8; 64];
        encoded[0..4].copy_from_slice(&1u32.to_le_bytes());
        encoded[4..8].copy_from_slice(&64u32.to_le_bytes());
        encoded.extend_from_slice(&[253, 7]);

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded,
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("repeat RLE frame should decode");

        assert_eq!(decoded.native_bytes, vec![7, 7, 7, 7]);
    }

    #[test]
    fn native_rle_splits_sample_byte_planes_into_segments() {
        let encoder = NativeRleLosslessEncoder::new();

        let encoded = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &[0x34, 0x12, 0xcd, 0xab],
                rows: 1,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should encode 16-bit byte planes");

        assert_eq!(&encoded.bytes[0..4], &2u32.to_le_bytes());
        assert_eq!(&encoded.bytes[4..8], &64u32.to_le_bytes());
        assert_eq!(&encoded.bytes[8..12], &67u32.to_le_bytes());
        assert_eq!(&encoded.bytes[64..67], &[1, 0x34, 0xcd]);
        assert_eq!(&encoded.bytes[67..70], &[1, 0x12, 0xab]);
    }

    #[test]
    fn native_rle_decodes_byte_planes_into_native_sample_order() {
        let codec = NativeRleLosslessEncoder::new();
        let encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &[0x34, 0x12, 0xcd, 0xab],
                rows: 1,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should encode 16-bit byte planes");

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 1,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should decode 16-bit byte planes");

        assert_eq!(decoded.native_bytes, vec![0x34, 0x12, 0xcd, 0xab]);
    }

    #[test]
    fn native_rle_rejects_unsupported_frame_shape() {
        let encoder = NativeRleLosslessEncoder::new();

        let error = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &[0; 16],
                rows: 1,
                columns: 1,
                samples_per_pixel: 16,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect_err("RLE should reject more than 15 segments");

        assert_eq!(error.backend_id(), "native_project_rle_encoder");
        assert!(matches!(error, CodecError::Unsupported { .. }));
        assert!(error.to_string().contains("RLE allows at most 15 segments"));
    }

    #[test]
    fn native_rle_rejects_length_mismatch() {
        let encoder = NativeRleLosslessEncoder::new();

        let error = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &[0, 1, 2],
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect_err("RLE should reject truncated input");

        assert!(matches!(error, CodecError::Unsupported { .. }));
        assert!(error.to_string().contains("native frame length is 3"));
    }

    #[test]
    fn native_rle_rejects_corrupt_segment_count_on_decode() {
        let codec = NativeRleLosslessEncoder::new();
        let mut encoded = vec![0u8; 64];
        encoded[0..4].copy_from_slice(&2u32.to_le_bytes());
        encoded[4..8].copy_from_slice(&64u32.to_le_bytes());
        encoded[8..12].copy_from_slice(&66u32.to_le_bytes());
        encoded.extend_from_slice(&[3, 0, 1, 2, 3]);

        let error = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded,
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect_err("decode should reject an unexpected segment count");

        assert!(matches!(error, CodecError::ValidationFailed { .. }));
        assert!(error.to_string().contains("has 2 segments, expected 1"));
    }

    #[cfg(feature = "jpeg")]
    #[test]
    fn dicom_rs_jpeg_baseline_feature_encodes_rgb_frame() {
        let obj = NativePixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            rows: 2,
            columns: 2,
            samples_per_pixel: 3,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "RGB",
            pixels: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        };
        let Codec::EncapsulatedPixelData(_, Some(writer)) = JPEG_BASELINE.codec() else {
            panic!("JPEG Baseline transfer syntax must expose a pixel writer with the jpeg feature")
        };

        let mut options = EncodeOptions::default();
        options.quality = Some(95);
        let mut encoded = Vec::new();
        let ops = writer
            .encode_frame(&obj, 0, options, &mut encoded)
            .expect("DICOM-rs JPEG Baseline writer should encode a tiny RGB frame");

        assert!(
            !ops.is_empty(),
            "JPEG writer should return attribute updates"
        );
        assert!(encoded.len() > 4, "JPEG codestream should not be empty");
        assert_eq!(
            &encoded[..2],
            &[0xff, 0xd8],
            "JPEG codestream must start with SOI"
        );
        assert_eq!(
            &encoded[encoded.len() - 2..],
            &[0xff, 0xd9],
            "JPEG codestream must end with EOI"
        );
    }

    #[cfg(feature = "jpeg")]
    struct NativePixelTestObject<'a> {
        transfer_syntax_uid: &'a str,
        rows: u16,
        columns: u16,
        samples_per_pixel: u16,
        bits_allocated: u16,
        bits_stored: u16,
        photometric_interpretation: &'a str,
        pixels: &'a [u8],
    }

    #[cfg(feature = "jpeg")]
    impl PixelDataObject for NativePixelTestObject<'_> {
        fn transfer_syntax_uid(&self) -> &str {
            self.transfer_syntax_uid
        }

        fn rows(&self) -> Option<u16> {
            Some(self.rows)
        }

        fn cols(&self) -> Option<u16> {
            Some(self.columns)
        }

        fn samples_per_pixel(&self) -> Option<u16> {
            Some(self.samples_per_pixel)
        }

        fn bits_allocated(&self) -> Option<u16> {
            Some(self.bits_allocated)
        }

        fn bits_stored(&self) -> Option<u16> {
            Some(self.bits_stored)
        }

        fn photometric_interpretation(&self) -> Option<&str> {
            Some(self.photometric_interpretation)
        }

        fn number_of_frames(&self) -> Option<u32> {
            Some(1)
        }

        fn number_of_fragments(&self) -> Option<u32> {
            None
        }

        fn fragment(&self, fragment: usize) -> Option<Cow<'_, [u8]>> {
            (fragment == 0).then(|| Cow::Borrowed(self.pixels))
        }

        fn offset_table(&self) -> Option<Cow<'_, [u32]>> {
            None
        }

        fn raw_pixel_data(&self) -> Option<RawPixelData> {
            Some(RawPixelData {
                fragments: C::from_vec(vec![self.pixels.to_vec()]),
                offset_table: C::new(),
            })
        }
    }
}
