use std::error::Error;
use std::fmt;

use crate::PACKAGE_VERSION;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub bytes: Vec<u8>,
}

pub trait FrameEncoder {
    fn backend(&self) -> CodecBackendInfo;

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError>;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_rle_backend_reports_identity_and_determinism() {
        let encoder = NativeRleLosslessEncoder::new();

        let backend = encoder.backend();

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
            })
            .expect("RLE should encode a tiny 8-bit frame");

        assert_eq!(&encoded.bytes[0..4], &1u32.to_le_bytes());
        assert_eq!(&encoded.bytes[4..8], &64u32.to_le_bytes());
        assert_eq!(&encoded.bytes[8..64], &[0u8; 56]);
        assert_eq!(&encoded.bytes[64..], &[3, 0, 85, 170, 255]);
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
            })
            .expect("RLE should encode repeated 8-bit samples");

        assert_eq!(&encoded.bytes[64..], &[253, 7]);
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
            })
            .expect("RLE should encode 16-bit byte planes");

        assert_eq!(&encoded.bytes[0..4], &2u32.to_le_bytes());
        assert_eq!(&encoded.bytes[4..8], &64u32.to_le_bytes());
        assert_eq!(&encoded.bytes[8..12], &67u32.to_le_bytes());
        assert_eq!(&encoded.bytes[64..67], &[1, 0x34, 0xcd]);
        assert_eq!(&encoded.bytes[67..70], &[1, 0x12, 0xab]);
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
            })
            .expect_err("RLE should reject truncated input");

        assert!(matches!(error, CodecError::Unsupported { .. }));
        assert!(error.to_string().contains("native frame length is 3"));
    }
}
