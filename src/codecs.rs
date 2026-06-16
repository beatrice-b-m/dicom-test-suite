use std::error::Error;
use std::fmt;

use crate::PACKAGE_VERSION;

#[cfg(any(
    feature = "charls",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use std::borrow::Cow;
#[cfg(feature = "jpeg2000")]
use std::os::raw::c_void;

#[cfg(any(
    feature = "charls",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use dicom_core::value::C;
#[cfg(any(feature = "charls", feature = "jpeg", feature = "jpegxl"))]
use dicom_encoding::adapters::{EncodeOptions, PixelDataWriter};
#[cfg(any(
    feature = "charls",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use dicom_encoding::{
    Codec,
    adapters::{PixelDataObject, PixelDataReader, RawPixelData},
};
#[cfg(feature = "jpeg2000")]
use dicom_transfer_syntax_registry::entries::JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY;
#[cfg(feature = "jpeg")]
use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;
#[cfg(feature = "charls")]
use dicom_transfer_syntax_registry::entries::JPEG_LS_LOSSLESS_IMAGE_COMPRESSION;
#[cfg(feature = "jpegxl")]
use dicom_transfer_syntax_registry::entries::JPEG_XL_LOSSLESS;
#[cfg(feature = "jpeg2000")]
use openjp2::image::opj_image_cmptparm_t;
#[cfg(feature = "jpeg2000")]
use openjp2::openjpeg::*;

pub const JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.90";
pub const JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.50";
pub const JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.80";
pub const JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.110";
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
#[cfg(feature = "jpeg2000")]
pub struct OpenJp2Jpeg2000LosslessEncoder;

#[cfg(feature = "jpeg2000")]
impl OpenJp2Jpeg2000LosslessEncoder {
    pub const BACKEND_ID: &'static str = "project_openjp2_jpeg2000_lossless_writer";

    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "jpeg2000")]
impl FrameEncoder for OpenJp2Jpeg2000LosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::DicomRsFeature,
            display_name: "Project OpenJPEG-rs JPEG 2000 Lossless writer",
            version: "dicom-transfer-syntax-registry 0.9.1 + jpeg2k 0.10.1 + openjp2 0.6.1",
            transfer_syntax_uid: JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID,
            feature_gate: Some("jpeg2000"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 16 || input.bits_stored != 16 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG 2000 Lossless first-case support is limited to 16-bit source frames",
            ));
        }
        if input.samples_per_pixel != 1 || input.photometric_interpretation != "MONOCHROME2" {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG 2000 Lossless first-case support currently requires MONOCHROME2 input",
            ));
        }

        let expected_len = usize::from(input.rows)
            .checked_mul(usize::from(input.columns))
            .and_then(|pixels| pixels.checked_mul(2))
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

        let mut samples = Vec::with_capacity(expected_len / 2);
        for chunk in input.native_frame.chunks_exact(2) {
            samples.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }

        let encoded = encode_jpeg2000_lossless_u16_mono2(input.rows, input.columns, &samples)?;
        if encoded.len() < 4 || encoded[..2] != [0xff, 0x4f] {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "JPEG 2000 codestream is missing the SOC marker",
            ));
        }
        if encoded[encoded.len() - 2..] != [0xff, 0xd9] {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "JPEG 2000 codestream is missing the EOC marker",
            ));
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "jpeg2000")]
impl FrameDecoder for OpenJp2Jpeg2000LosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID,
            rows: input.rows,
            columns: input.columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.encoded_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) =
            JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG 2000 Lossless reader is not available",
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

#[derive(Debug, Clone, Copy, Default)]
#[cfg(feature = "charls")]
pub struct DicomRsJpegLsLosslessEncoder;

#[cfg(feature = "charls")]
impl DicomRsJpegLsLosslessEncoder {
    pub const BACKEND_ID: &'static str = "dicom_rs_charls_jpeg_ls_lossless_writer";

    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "charls")]
impl FrameEncoder for DicomRsJpegLsLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::DicomRsFeature,
            display_name: "DICOM-rs CharLS JPEG-LS Lossless writer",
            version: "dicom-transfer-syntax-registry 0.9.1 + charls 0.4.2",
            transfer_syntax_uid: JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID,
            feature_gate: Some("charls"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 8 && input.bits_allocated != 16 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG-LS Lossless case support is limited to 8-bit and 16-bit source frames",
            ));
        }
        if input.bits_stored == 1 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG-LS Lossless case support does not include 1-bit samples",
            ));
        }
        if input.samples_per_pixel != 1 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG-LS Lossless first-case support currently requires single-sample input",
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
        let Codec::EncapsulatedPixelData(_, Some(writer)) =
            JPEG_LS_LOSSLESS_IMAGE_COMPRESSION.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG-LS Lossless writer is not available",
            ));
        };

        let mut options = EncodeOptions::default();
        options.quality = Some(100);
        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, options, &mut encoded)
            .map_err(|err| CodecError::encode_failed(Self::BACKEND_ID, err.to_string()))?;

        if encoded.is_empty() {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "JPEG-LS codestream is empty",
            ));
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "charls")]
impl FrameDecoder for DicomRsJpegLsLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID,
            rows: input.rows,
            columns: input.columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.encoded_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) =
            JPEG_LS_LOSSLESS_IMAGE_COMPRESSION.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG-LS Lossless reader is not available",
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

#[derive(Debug, Clone, Copy, Default)]
#[cfg(feature = "jpegxl")]
pub struct DicomRsJpegXlLosslessEncoder;

#[cfg(feature = "jpegxl")]
impl DicomRsJpegXlLosslessEncoder {
    pub const BACKEND_ID: &'static str = "dicom_rs_jpegxl_lossless_writer";

    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "jpegxl")]
impl FrameEncoder for DicomRsJpegXlLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::DicomRsFeature,
            display_name: "DICOM-rs JPEG XL Lossless writer",
            version: "dicom-transfer-syntax-registry 0.9.1 + jxl-oxide 0.10.2 + zune-jpegxl 0.4.0",
            transfer_syntax_uid: JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID,
            feature_gate: Some("jpegxl"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 8 && input.bits_allocated != 16 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG XL Lossless case support is limited to 8-bit and 16-bit source frames",
            ));
        }
        if input.bits_stored == 1 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG XL Lossless case support does not include 1-bit samples",
            ));
        }
        if input.samples_per_pixel != 1 && input.samples_per_pixel != 3 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG XL Lossless first-case support currently requires MONOCHROME2 or RGB input",
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
        let Codec::EncapsulatedPixelData(_, Some(writer)) = JPEG_XL_LOSSLESS.codec() else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG XL Lossless writer is not available",
            ));
        };

        let mut options = EncodeOptions::default();
        options.quality = Some(100);
        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, options, &mut encoded)
            .map_err(|err| CodecError::encode_failed(Self::BACKEND_ID, err.to_string()))?;

        if encoded.is_empty() {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "JPEG XL codestream is empty",
            ));
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "jpegxl")]
impl FrameDecoder for DicomRsJpegXlLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID,
            rows: input.rows,
            columns: input.columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.encoded_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) = JPEG_XL_LOSSLESS.codec() else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs JPEG XL Lossless reader is not available",
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

#[cfg(any(
    feature = "charls",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
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

#[cfg(any(
    feature = "charls",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
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

#[cfg(feature = "jpeg2000")]
struct OpenJp2Output {
    bytes: Vec<u8>,
    offset: usize,
}

#[cfg(feature = "jpeg2000")]
impl OpenJp2Output {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            offset: 0,
        }
    }
}

#[cfg(feature = "jpeg2000")]
unsafe extern "C" fn openjp2_write_stream(
    buffer: *mut c_void,
    byte_count: usize,
    user_data: *mut c_void,
) -> usize {
    if buffer.is_null() || user_data.is_null() {
        return usize::MAX;
    }
    let output = unsafe { &mut *(user_data as *mut OpenJp2Output) };
    let input = unsafe { std::slice::from_raw_parts(buffer as *const u8, byte_count) };
    let end = match output.offset.checked_add(byte_count) {
        Some(end) => end,
        None => return usize::MAX,
    };
    if end > output.bytes.len() {
        output.bytes.resize(end, 0);
    }
    output.bytes[output.offset..end].copy_from_slice(input);
    output.offset = end;
    byte_count
}

#[cfg(feature = "jpeg2000")]
unsafe extern "C" fn openjp2_skip_stream(byte_count: i64, user_data: *mut c_void) -> i64 {
    if user_data.is_null() {
        return -1;
    }
    let output = unsafe { &mut *(user_data as *mut OpenJp2Output) };
    let next_offset = if byte_count >= 0 {
        output.offset.checked_add(byte_count as usize)
    } else {
        output
            .offset
            .checked_sub(byte_count.unsigned_abs() as usize)
    };
    let Some(next_offset) = next_offset else {
        return -1;
    };
    if next_offset > output.bytes.len() {
        output.bytes.resize(next_offset, 0);
    }
    output.offset = next_offset;
    byte_count
}

#[cfg(feature = "jpeg2000")]
unsafe extern "C" fn openjp2_seek_stream(offset: i64, user_data: *mut c_void) -> i32 {
    if user_data.is_null() || offset < 0 {
        return 0;
    }
    let output = unsafe { &mut *(user_data as *mut OpenJp2Output) };
    let offset = offset as usize;
    if offset > output.bytes.len() {
        output.bytes.resize(offset, 0);
    }
    output.offset = offset;
    1
}

#[cfg(feature = "jpeg2000")]
fn encode_jpeg2000_lossless_u16_mono2(
    rows: u16,
    columns: u16,
    samples: &[u16],
) -> Result<Vec<u8>, CodecError> {
    let expected_samples = usize::from(rows)
        .checked_mul(usize::from(columns))
        .ok_or_else(|| {
            CodecError::unsupported(
                OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID,
                "sample count overflowed",
            )
        })?;
    if samples.len() != expected_samples {
        return Err(CodecError::unsupported(
            OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID,
            format!(
                "sample count is {}, expected {expected_samples}",
                samples.len()
            ),
        ));
    }

    let mut component = opj_image_cmptparm_t {
        dx: 1,
        dy: 1,
        w: u32::from(columns),
        h: u32::from(rows),
        x0: 0,
        y0: 0,
        prec: 16,
        bpp: 16,
        sgnd: 0,
    };

    let image = opj_image_create(1, &mut component, OPJ_CLRSPC_GRAY);
    if image.is_null() {
        return Err(CodecError::encode_failed(
            OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID,
            "OpenJPEG failed to allocate an image",
        ));
    }

    let encode_result = unsafe {
        (*image).x0 = 0;
        (*image).y0 = 0;
        (*image).x1 = u32::from(columns);
        (*image).y1 = u32::from(rows);

        if (*image).comps.is_null() {
            opj_image_destroy(image);
            return Err(CodecError::encode_failed(
                OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID,
                "OpenJPEG image has no component storage",
            ));
        }
        let comps = std::slice::from_raw_parts_mut((*image).comps, 1);
        if comps[0].data.is_null() {
            opj_image_destroy(image);
            return Err(CodecError::encode_failed(
                OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID,
                "OpenJPEG image component has no sample storage",
            ));
        }
        let data = std::slice::from_raw_parts_mut(comps[0].data, samples.len());
        for (target, sample) in data.iter_mut().zip(samples) {
            *target = i32::from(*sample);
        }

        let result = encode_openjp2_image_to_j2k(image);
        opj_image_destroy(image);
        result
    };

    encode_result
}

#[cfg(feature = "jpeg2000")]
unsafe fn encode_openjp2_image_to_j2k(image: *mut opj_image_t) -> Result<Vec<u8>, CodecError> {
    let backend_id = OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID;
    let mut params = opj_cparameters_t::default();
    unsafe {
        opj_set_default_encoder_parameters(&mut params);
    }
    params.tcp_numlayers = 1;
    params.cp_disto_alloc = 1;
    params.tcp_rates[0] = 0.0;
    params.irreversible = 0;
    params.numresolution = 1;

    let codec = unsafe { opj_create_compress(OPJ_CODEC_J2K) };
    if codec.is_null() {
        return Err(CodecError::encode_failed(
            backend_id,
            "OpenJPEG failed to create a J2K compressor",
        ));
    }

    let stream = unsafe { opj_stream_default_create(0) };
    if stream.is_null() {
        unsafe {
            opj_destroy_codec(codec);
        }
        return Err(CodecError::encode_failed(
            backend_id,
            "OpenJPEG failed to create an output stream",
        ));
    }

    let mut output = OpenJp2Output::new();
    unsafe {
        opj_stream_set_write_function(stream, Some(openjp2_write_stream));
        opj_stream_set_skip_function(stream, Some(openjp2_skip_stream));
        opj_stream_set_seek_function(stream, Some(openjp2_seek_stream));
        opj_stream_set_user_data(
            stream,
            (&mut output as *mut OpenJp2Output).cast::<c_void>(),
            None,
        );
    }

    let ok = unsafe {
        opj_setup_encoder(codec, &mut params, image) == 1
            && opj_start_compress(codec, image, stream) == 1
            && opj_encode(codec, stream) == 1
            && opj_end_compress(codec, stream) == 1
    };

    unsafe {
        opj_stream_destroy(stream);
        opj_destroy_codec(codec);
    }

    if !ok {
        return Err(CodecError::encode_failed(
            backend_id,
            "OpenJPEG failed to encode the image",
        ));
    }
    if output.bytes.is_empty() {
        return Err(CodecError::validation_failed(
            backend_id,
            "OpenJPEG produced an empty J2K codestream",
        ));
    }

    Ok(output.bytes)
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
    #[cfg(any(feature = "charls", feature = "jpeg", feature = "jpegxl"))]
    use std::borrow::Cow;

    #[cfg(any(feature = "charls", feature = "jpeg", feature = "jpegxl"))]
    use dicom_core::value::C;
    #[cfg(any(
        feature = "charls",
        feature = "jpeg",
        feature = "jpegxl",
        feature = "jpeg2000"
    ))]
    use dicom_encoding::Codec;
    #[cfg(any(feature = "charls", feature = "jpeg", feature = "jpegxl"))]
    use dicom_encoding::adapters::{EncodeOptions, PixelDataObject, PixelDataWriter, RawPixelData};
    #[cfg(feature = "jpeg2000")]
    use dicom_transfer_syntax_registry::entries::JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY;
    #[cfg(feature = "jpeg")]
    use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;
    #[cfg(feature = "charls")]
    use dicom_transfer_syntax_registry::entries::JPEG_LS_LOSSLESS_IMAGE_COMPRESSION;
    #[cfg(feature = "jpegxl")]
    use dicom_transfer_syntax_registry::entries::JPEG_XL_LOSSLESS;

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

    #[cfg(feature = "charls")]
    #[test]
    fn dicom_rs_jpeg_ls_lossless_backend_reports_identity() {
        let encoder = DicomRsJpegLsLosslessEncoder::new();

        let backend = FrameEncoder::backend(&encoder);

        assert_eq!(
            backend.backend_id,
            "dicom_rs_charls_jpeg_ls_lossless_writer"
        );
        assert_eq!(backend.backend_kind.as_str(), "dicom_rs_feature");
        assert_eq!(backend.transfer_syntax_uid, "1.2.840.10008.1.2.4.80");
        assert_eq!(backend.feature_gate, Some("charls"));
        assert_eq!(backend.determinism.as_str(), "semantic_stable");
    }

    #[cfg(feature = "charls")]
    #[test]
    fn dicom_rs_jpeg_ls_lossless_feature_round_trips_mono_frame() {
        let codec = DicomRsJpegLsLosslessEncoder::new();
        let native = [0, 32, 64, 96, 128, 160, 192, 255];

        let encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows: 2,
                columns: 4,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("JPEG-LS Lossless should encode a tiny monochrome frame");
        assert!(
            !encoded.bytes.is_empty(),
            "JPEG-LS codestream should not be empty"
        );

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 2,
                columns: 4,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("JPEG-LS Lossless should decode its own codestream");

        assert_eq!(decoded.native_bytes, native);
    }

    #[cfg(feature = "charls")]
    #[test]
    fn dicom_rs_charls_feature_exposes_jpeg_ls_lossless_writer() {
        let obj = NativePixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            rows: 2,
            columns: 4,
            samples_per_pixel: 1,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "MONOCHROME2",
            pixels: &[0, 32, 64, 96, 128, 160, 192, 255],
        };
        let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) =
            JPEG_LS_LOSSLESS_IMAGE_COMPRESSION.codec()
        else {
            panic!(
                "JPEG-LS Lossless transfer syntax must expose reader and writer with the charls feature"
            )
        };

        let mut options = EncodeOptions::default();
        options.quality = Some(100);
        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, options, &mut encoded)
            .expect("DICOM-rs JPEG-LS Lossless writer should encode a tiny monochrome frame");

        let encoded_obj = EncodedPixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.4.80",
            rows: 2,
            columns: 4,
            samples_per_pixel: 1,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "MONOCHROME2",
            fragments: vec![encoded],
        };
        let mut decoded = Vec::new();
        reader
            .decode_frame(&encoded_obj, 0, &mut decoded)
            .expect("DICOM-rs JPEG-LS Lossless reader should decode the generated frame");

        assert_eq!(decoded, obj.pixels);
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

    #[cfg(feature = "jpegxl")]
    #[test]
    fn dicom_rs_jpeg_xl_lossless_backend_reports_identity() {
        let encoder = DicomRsJpegXlLosslessEncoder::new();

        let backend = FrameEncoder::backend(&encoder);

        assert_eq!(backend.backend_id, "dicom_rs_jpegxl_lossless_writer");
        assert_eq!(backend.backend_kind.as_str(), "dicom_rs_feature");
        assert_eq!(backend.transfer_syntax_uid, "1.2.840.10008.1.2.4.110");
        assert_eq!(backend.feature_gate, Some("jpegxl"));
        assert_eq!(backend.determinism.as_str(), "semantic_stable");
        assert!(backend.version.contains("jxl-oxide 0.10.2"));
        assert!(backend.version.contains("zune-jpegxl 0.4.0"));
    }

    #[cfg(feature = "jpegxl")]
    #[test]
    fn dicom_rs_jpeg_xl_lossless_feature_round_trips_rgb_frame() {
        let codec = DicomRsJpegXlLosslessEncoder::new();
        let native = [
            255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 7, 19, 31, 240, 128, 64, 10, 20, 30,
            40, 50, 60,
        ];

        let encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows: 2,
                columns: 4,
                samples_per_pixel: 3,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "RGB",
            })
            .expect("JPEG XL Lossless should encode a tiny RGB frame");
        assert!(
            !encoded.bytes.is_empty(),
            "JPEG XL codestream should not be empty"
        );

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 2,
                columns: 4,
                samples_per_pixel: 3,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "RGB",
            })
            .expect("JPEG XL Lossless should decode its own codestream");

        assert_eq!(decoded.native_bytes, native);
    }

    #[cfg(feature = "jpegxl")]
    #[test]
    fn dicom_rs_jpegxl_feature_exposes_lossless_reader_and_writer() {
        let obj = NativePixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            rows: 2,
            columns: 4,
            samples_per_pixel: 3,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "RGB",
            pixels: &[
                255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 7, 19, 31, 240, 128, 64, 10, 20,
                30, 40, 50, 60,
            ],
        };
        let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) = JPEG_XL_LOSSLESS.codec()
        else {
            panic!(
                "JPEG XL Lossless transfer syntax must expose reader and writer with the jpegxl feature"
            )
        };

        let mut options = EncodeOptions::default();
        options.quality = Some(100);
        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, options, &mut encoded)
            .expect("DICOM-rs JPEG XL Lossless writer should encode a tiny RGB frame");
        assert!(
            !encoded.is_empty(),
            "JPEG XL codestream should not be empty"
        );

        let encoded_obj = EncodedPixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.4.110",
            rows: 2,
            columns: 4,
            samples_per_pixel: 3,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "RGB",
            fragments: vec![encoded],
        };
        let mut decoded = Vec::new();
        reader
            .decode_frame(&encoded_obj, 0, &mut decoded)
            .expect("DICOM-rs JPEG XL Lossless reader should decode the generated frame");

        assert_eq!(decoded, obj.pixels);
    }

    #[cfg(feature = "jpeg2000")]
    #[test]
    fn openjp2_jpeg2000_lossless_backend_reports_identity() {
        let encoder = OpenJp2Jpeg2000LosslessEncoder::new();

        let backend = FrameEncoder::backend(&encoder);

        assert_eq!(
            backend.backend_id,
            "project_openjp2_jpeg2000_lossless_writer"
        );
        assert_eq!(backend.backend_kind.as_str(), "dicom_rs_feature");
        assert_eq!(backend.transfer_syntax_uid, "1.2.840.10008.1.2.4.90");
        assert_eq!(backend.feature_gate, Some("jpeg2000"));
        assert_eq!(backend.determinism.as_str(), "semantic_stable");
        assert!(backend.version.contains("jpeg2k 0.10.1"));
        assert!(backend.version.contains("openjp2 0.6.1"));
    }

    #[cfg(feature = "jpeg2000")]
    #[test]
    fn openjp2_jpeg2000_lossless_round_trips_u16_mono2_frame() {
        let codec = OpenJp2Jpeg2000LosslessEncoder::new();
        let samples = [0u16, 17, 1024, 4096, 8192, 32768, 49152, 65535];
        let native = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();

        let encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows: 2,
                columns: 4,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("JPEG 2000 Lossless should encode a tiny 16-bit frame");

        assert_eq!(
            &encoded.bytes[..2],
            &[0xff, 0x4f],
            "J2K codestream must start with SOC"
        );
        assert_eq!(
            &encoded.bytes[encoded.bytes.len() - 2..],
            &[0xff, 0xd9],
            "J2K codestream must end with EOC"
        );

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 2,
                columns: 4,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("DICOM-rs JPEG 2000 reader should decode the generated codestream");

        assert_eq!(decoded.native_bytes, native);
    }

    #[cfg(feature = "jpeg2000")]
    #[test]
    fn dicom_rs_jpeg2000_feature_exposes_lossless_reader_without_writer() {
        let Codec::EncapsulatedPixelData(Some(_reader), writer) =
            JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.codec()
        else {
            panic!("JPEG 2000 Lossless transfer syntax must expose a reader")
        };

        assert!(
            writer.is_none(),
            "DICOM-rs JPEG 2000 Lossless support remains decode-only"
        );
    }

    #[cfg(feature = "jpeg2000")]
    #[test]
    fn openjph_htj2k_lossless_codestream_is_reproducible_for_sampled_u16_values() {
        use dicom_transfer_syntax_registry::entries::HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY;
        use std::fs;
        use std::process::Command;
        use std::time::{SystemTime, UNIX_EPOCH};

        if Command::new("ojph_compress").arg("-v").output().is_err() {
            eprintln!("skipping OpenJPH HTJ2K spike proof because ojph_compress is not on PATH");
            return;
        }

        let samples = [1u16, 17, 1024, 4096, 8192, 16384, 24576, 32767];
        let native = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("dts-openjph-htj2k-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&dir).expect("temporary OpenJPH spike directory should be writable");
        let input_path = dir.join("mono2_u16.raw");
        let first_codestream_path = dir.join("mono2_u16_htj2k_first.j2c");
        let second_codestream_path = dir.join("mono2_u16_htj2k_second.j2c");
        fs::write(&input_path, &native).expect("temporary raw input should be writable");

        run_openjph_htj2k_lossless_encode(&input_path, &first_codestream_path);
        run_openjph_htj2k_lossless_encode(&input_path, &second_codestream_path);

        let codestream =
            fs::read(&first_codestream_path).expect("OpenJPH HTJ2K codestream should be readable");
        let repeated_codestream = fs::read(&second_codestream_path)
            .expect("second OpenJPH HTJ2K codestream should be readable");
        assert_eq!(
            crate::sha256_hex(&codestream),
            crate::sha256_hex(&repeated_codestream),
            "OpenJPH should produce byte-identical HTJ2K codestreams for fixed raw input and options"
        );
        assert!(
            codestream.len() >= 4,
            "OpenJPH HTJ2K codestream should not be empty"
        );
        assert_eq!(
            &codestream[..2],
            &[0xff, 0x4f],
            "HTJ2K codestream must start with SOC"
        );
        assert_eq!(
            &codestream[codestream.len() - 2..],
            &[0xff, 0xd9],
            "HTJ2K codestream must end with EOC"
        );

        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.4.201",
            rows: 2,
            columns: 4,
            samples_per_pixel: 1,
            bits_allocated: 16,
            bits_stored: 16,
            photometric_interpretation: "MONOCHROME2",
            fragments: vec![codestream],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(Some(reader), writer) =
            HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.codec()
        else {
            panic!("DICOM-rs HTJ2K Lossless transfer syntax must expose a reader")
        };
        assert!(
            writer.is_none(),
            "DICOM-rs HTJ2K Lossless support should remain decode-only"
        );

        let mut decoded = Vec::new();
        reader
            .decode_frame(&obj, 0, &mut decoded)
            .expect("DICOM-rs OpenJPEG-backed HTJ2K reader should decode OpenJPH output");
        assert_eq!(decoded, native);

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(feature = "jpeg2000")]
    fn run_openjph_htj2k_lossless_encode(
        input_path: &std::path::Path,
        output_path: &std::path::Path,
    ) {
        use std::process::Command;

        let output = Command::new("ojph_compress")
            .arg("-i")
            .arg(input_path)
            .arg("-o")
            .arg(output_path)
            .arg("-reversible")
            .arg("true")
            .arg("-num_decomps")
            .arg("0")
            .arg("-dims")
            .arg("{4,2}")
            .arg("-num_comps")
            .arg("1")
            .arg("-signed")
            .arg("false")
            .arg("-bit_depth")
            .arg("16")
            .arg("-downsamp")
            .arg("{1,1}")
            .output()
            .expect("ojph_compress should run for the HTJ2K spike");
        assert!(
            output.status.success(),
            "ojph_compress should encode the tiny HTJ2K frame: status={:?}, stdout={}, stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(any(feature = "charls", feature = "jpeg", feature = "jpegxl"))]
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

    #[cfg(any(feature = "charls", feature = "jpeg", feature = "jpegxl"))]
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

    #[cfg(any(feature = "charls", feature = "jpegxl"))]
    struct EncodedPixelTestObject<'a> {
        transfer_syntax_uid: &'a str,
        rows: u16,
        columns: u16,
        samples_per_pixel: u16,
        bits_allocated: u16,
        bits_stored: u16,
        photometric_interpretation: &'a str,
        fragments: Vec<Vec<u8>>,
    }

    #[cfg(any(feature = "charls", feature = "jpegxl"))]
    impl PixelDataObject for EncodedPixelTestObject<'_> {
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
            Some(u32::try_from(self.fragments.len()).unwrap_or(u32::MAX))
        }

        fn fragment(&self, fragment: usize) -> Option<Cow<'_, [u8]>> {
            self.fragments
                .get(fragment)
                .map(|fragment| Cow::Borrowed(fragment.as_slice()))
        }

        fn offset_table(&self) -> Option<Cow<'_, [u32]>> {
            None
        }

        fn raw_pixel_data(&self) -> Option<RawPixelData> {
            Some(RawPixelData {
                fragments: C::from_vec(self.fragments.clone()),
                offset_table: C::new(),
            })
        }
    }
}
