use std::error::Error;
use std::fmt;

use crate::PACKAGE_VERSION;

#[cfg(any(
    feature = "charls",
    feature = "deflate",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use std::borrow::Cow;
#[cfg(feature = "jpeg2000")]
use std::os::raw::c_void;
#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(any(
    feature = "htj2k_openjph",
    feature = "legacy_jpeg_dcmtk",
    feature = "jpegxl"
))]
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(any(
    feature = "charls",
    feature = "deflate",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use dicom_core::value::C;
#[cfg(any(
    feature = "charls",
    feature = "deflate",
    feature = "jpeg",
    feature = "jpegxl"
))]
use dicom_encoding::adapters::{EncodeOptions, PixelDataWriter};
#[cfg(any(
    feature = "charls",
    feature = "deflate",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use dicom_encoding::{
    Codec,
    adapters::{PixelDataObject, PixelDataReader, RawPixelData},
};
#[cfg(feature = "deflate")]
use dicom_transfer_syntax_registry::entries::DEFLATED_IMAGE_FRAME_COMPRESSION;
#[cfg(feature = "jpeg2000")]
use dicom_transfer_syntax_registry::entries::JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY;
#[cfg(feature = "jpeg")]
use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;
#[cfg(feature = "charls")]
use dicom_transfer_syntax_registry::entries::JPEG_LS_LOSSLESS_IMAGE_COMPRESSION;
#[cfg(feature = "htj2k_openjph")]
use dicom_transfer_syntax_registry::entries::{
    HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION,
    HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY,
};
#[cfg(feature = "jpegxl")]
use dicom_transfer_syntax_registry::entries::{JPEG_XL, JPEG_XL_LOSSLESS};
#[cfg(feature = "jpeg2000")]
use openjp2::image::opj_image_cmptparm_t;
#[cfg(feature = "jpeg2000")]
use openjp2::openjpeg::*;

pub const JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.90";
pub const JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.50";
pub const JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.80";
pub const JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.57";
pub const JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.70";
pub const JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.110";
pub const JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.112";
pub const HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.201";
pub const HTJ2K_LOSSY_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.4.203";
pub const RLE_LOSSLESS_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.5";
pub const DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.8.1";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossySampleDomain {
    Unsigned8,
    Unsigned16LittleEndian,
}

impl LossySampleDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsigned8 => "unsigned_8_bit",
            Self::Unsigned16LittleEndian => "unsigned_16_bit_little_endian",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LossyChannelMetrics {
    pub channel_index: usize,
    pub sample_count: usize,
    pub max_absolute_error: u64,
    pub rmse: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LossyFrameMetrics {
    pub sample_domain: LossySampleDomain,
    pub sample_count: usize,
    pub channels: Vec<LossyChannelMetrics>,
    pub overall_rmse: f64,
}

pub fn calculate_lossy_frame_metrics(
    reference: &[u8],
    decoded: &[u8],
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
) -> Result<LossyFrameMetrics, CodecError> {
    const BACKEND_ID: &str = "lossy_frame_metric_calculator";
    if samples_per_pixel == 0 {
        return Err(CodecError::unsupported(
            BACKEND_ID,
            "samples_per_pixel must be greater than zero",
        ));
    }
    let domain = match bits_allocated {
        8 => LossySampleDomain::Unsigned8,
        16 => LossySampleDomain::Unsigned16LittleEndian,
        other => {
            return Err(CodecError::unsupported(
                BACKEND_ID,
                format!("only unsigned 8-bit and 16-bit samples are supported, got {other}"),
            ));
        }
    };
    let bytes_per_sample = usize::from(bits_allocated / 8);
    let sample_count = usize::from(rows)
        .checked_mul(usize::from(columns))
        .and_then(|pixels| pixels.checked_mul(usize::from(samples_per_pixel)))
        .ok_or_else(|| CodecError::unsupported(BACKEND_ID, "frame sample count overflowed"))?;
    let expected_bytes = sample_count
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| CodecError::unsupported(BACKEND_ID, "frame byte length overflowed"))?;
    if reference.len() != expected_bytes || decoded.len() != expected_bytes {
        return Err(CodecError::validation_failed(
            BACKEND_ID,
            format!(
                "frame shape requires {expected_bytes} bytes, got reference={} decoded={}",
                reference.len(),
                decoded.len()
            ),
        ));
    }

    let channel_count = usize::from(samples_per_pixel);
    let samples_per_channel = usize::from(rows)
        .checked_mul(usize::from(columns))
        .ok_or_else(|| CodecError::unsupported(BACKEND_ID, "pixel count overflowed"))?;
    let mut maximums = vec![0_u64; channel_count];
    let mut squared_error_sums = vec![0_f64; channel_count];
    let sample = |bytes: &[u8], index: usize| -> u64 {
        match domain {
            LossySampleDomain::Unsigned8 => u64::from(bytes[index]),
            LossySampleDomain::Unsigned16LittleEndian => {
                let offset = index * 2;
                u64::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
            }
        }
    };
    for index in 0..sample_count {
        let error = sample(reference, index).abs_diff(sample(decoded, index));
        let channel = index % channel_count;
        maximums[channel] = maximums[channel].max(error);
        squared_error_sums[channel] += (error * error) as f64;
    }
    let channels = maximums
        .into_iter()
        .zip(squared_error_sums.iter().copied())
        .enumerate()
        .map(
            |(channel_index, (max_absolute_error, squared_error_sum))| LossyChannelMetrics {
                channel_index,
                sample_count: samples_per_channel,
                max_absolute_error,
                rmse: (squared_error_sum / samples_per_channel as f64).sqrt(),
            },
        )
        .collect();
    let overall_squared_error_sum: f64 = squared_error_sums.into_iter().sum();
    Ok(LossyFrameMetrics {
        sample_domain: domain,
        sample_count,
        channels,
        overall_rmse: (overall_squared_error_sum / sample_count as f64).sqrt(),
    })
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(any(
    feature = "htj2k_openjph",
    feature = "legacy_jpeg_dcmtk",
    feature = "jpegxl"
))]
pub struct ExternalCommandBackendIdentity {
    pub command: &'static str,
    pub executable_path: PathBuf,
    pub executable_sha256: String,
    pub version: Option<String>,
    pub version_source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "legacy_jpeg_dcmtk")]
pub struct EncodedDicomFile {
    pub backend_identity: ExternalCommandBackendIdentity,
    pub output_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "legacy_jpeg_dcmtk")]
pub enum DcmtkDcmcjpegLosslessProcess {
    Process14,
    Sv1,
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
impl DcmtkDcmcjpegLosslessProcess {
    pub fn backend_id(self) -> &'static str {
        match self {
            Self::Process14 => "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
            Self::Sv1 => "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Process14 => "DCMTK dcmcjpeg JPEG Lossless Process 14 file writer",
            Self::Sv1 => "DCMTK dcmcjpeg JPEG Lossless SV1 file writer",
        }
    }

    pub fn transfer_syntax_uid(self) -> &'static str {
        match self {
            Self::Process14 => JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID,
            Self::Sv1 => JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID,
        }
    }

    pub fn mode_label(self) -> &'static str {
        match self {
            Self::Process14 => "lossless_process_14",
            Self::Sv1 => "lossless_sv1",
        }
    }

    fn dcmcjpeg_encode_arg(self) -> &'static str {
        match self {
            Self::Process14 => "--encode-lossless",
            Self::Sv1 => "--encode-lossless-sv1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "legacy_jpeg_dcmtk")]
pub struct DcmtkDcmcjpegLosslessSv1Encoder {
    command: PathBuf,
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
impl Default for DcmtkDcmcjpegLosslessSv1Encoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
impl DcmtkDcmcjpegLosslessSv1Encoder {
    pub const BACKEND_ID: &'static str = "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer";
    pub const COMMAND: &'static str = "dcmcjpeg";

    pub fn new() -> Self {
        Self {
            command: PathBuf::from(Self::COMMAND),
        }
    }

    pub fn with_command(command: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn backend(&self) -> CodecBackendInfo {
        self.backend_for(DcmtkDcmcjpegLosslessProcess::Sv1)
    }

    pub fn backend_for(&self, process: DcmtkDcmcjpegLosslessProcess) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: process.backend_id(),
            backend_kind: CodecBackendKind::ExternalCommand,
            display_name: process.display_name(),
            version: "DCMTK dcmcjpeg version and executable SHA-256 recorded at runtime",
            transfer_syntax_uid: process.transfer_syntax_uid(),
            feature_gate: Some("legacy_jpeg_dcmtk"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    pub fn discover_backend_identity(&self) -> Result<ExternalCommandBackendIdentity, CodecError> {
        let executable_path =
            resolve_command_path(&self.command, Self::BACKEND_ID, "DCMTK dcmcjpeg")?;
        let executable_bytes = fs::read(&executable_path).map_err(|err| {
            CodecError::unavailable(
                Self::BACKEND_ID,
                format!(
                    "failed to read DCMTK dcmcjpeg executable {} for fingerprinting: {err}",
                    executable_path.display()
                ),
            )
        })?;
        let version = Command::new(&executable_path)
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|version| !version.is_empty());

        Ok(ExternalCommandBackendIdentity {
            command: Self::COMMAND,
            executable_path,
            executable_sha256: crate::sha256_hex(&executable_bytes),
            version,
            version_source: "command_stdout",
        })
    }

    pub fn encode_file(
        &self,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<EncodedDicomFile, CodecError> {
        self.encode_file_with_process(DcmtkDcmcjpegLosslessProcess::Sv1, input_path, output_path)
    }

    pub fn encode_file_with_process(
        &self,
        process: DcmtkDcmcjpegLosslessProcess,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<EncodedDicomFile, CodecError> {
        self.encode_file_with_process_cancellable(process, input_path, output_path, &|| false)
    }

    pub fn encode_file_with_process_cancellable(
        &self,
        process: DcmtkDcmcjpegLosslessProcess,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<EncodedDicomFile, CodecError> {
        let input_path = input_path.as_ref();
        let output_path = output_path.as_ref();
        let identity = self.discover_backend_identity()?;
        if is_cancelled() {
            return Err(CodecError::encode_failed(
                process.backend_id(),
                "execution cancelled",
            ));
        }
        let mut child = Command::new(&identity.executable_path)
            .arg(process.dcmcjpeg_encode_arg())
            .args([
                "--true-lossless",
                "--fragment-per-frame",
                "--offset-table-create",
                "--uid-never",
            ])
            .arg(input_path)
            .arg(output_path)
            .spawn()
            .map_err(|err| {
                CodecError::encode_failed(
                    process.backend_id(),
                    format!("failed to run dcmcjpeg: {err}"),
                )
            })?;
        loop {
            if is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(output_path);
                return Err(CodecError::encode_failed(
                    process.backend_id(),
                    "execution cancelled",
                ));
            }
            match child.try_wait().map_err(|err| {
                CodecError::encode_failed(
                    process.backend_id(),
                    format!("failed waiting for dcmcjpeg: {err}"),
                )
            })? {
                Some(status) => {
                    if !status.success() {
                        return Err(CodecError::encode_failed(
                            process.backend_id(),
                            format!("dcmcjpeg failed with status {:?}", status.code()),
                        ));
                    }
                    break;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }

        let output_bytes = fs::read(output_path).map_err(|err| {
            CodecError::encode_failed(
                process.backend_id(),
                format!(
                    "failed to read DCMTK dcmcjpeg output {}: {err}",
                    output_path.display()
                ),
            )
        })?;
        if output_bytes.len() < 132 || &output_bytes[128..132] != b"DICM" {
            return Err(CodecError::validation_failed(
                process.backend_id(),
                "dcmcjpeg output is not a DICOM Part 10 file",
            ));
        }

        Ok(EncodedDicomFile {
            backend_identity: identity,
            output_bytes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "htj2k_openjph")]
pub struct OpenJphHtj2kLosslessEncoder {
    command: PathBuf,
}

#[cfg(feature = "htj2k_openjph")]
impl Default for OpenJphHtj2kLosslessEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "htj2k_openjph")]
impl OpenJphHtj2kLosslessEncoder {
    pub const BACKEND_ID: &'static str = "openjph_htj2k_lossless_command_writer";
    pub const COMMAND: &'static str = "ojph_compress";

    pub fn new() -> Self {
        Self {
            command: PathBuf::from(Self::COMMAND),
        }
    }

    pub fn with_command(command: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn discover_backend_identity(&self) -> Result<ExternalCommandBackendIdentity, CodecError> {
        let executable_path = resolve_command_path(&self.command, Self::BACKEND_ID, "OpenJPH")?;
        let executable_bytes = fs::read(&executable_path).map_err(|err| {
            CodecError::unavailable(
                Self::BACKEND_ID,
                format!(
                    "failed to read OpenJPH executable {} for fingerprinting: {err}",
                    executable_path.display()
                ),
            )
        })?;

        Ok(ExternalCommandBackendIdentity {
            command: Self::COMMAND,
            executable_path,
            executable_sha256: crate::sha256_hex(&executable_bytes),
            version: None,
            version_source: "executable_sha256",
        })
    }
}

#[cfg(feature = "htj2k_openjph")]
impl FrameEncoder for OpenJphHtj2kLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::ExternalCommand,
            display_name: "OpenJPH HTJ2K Lossless external command writer",
            version: "OpenJPH ojph_compress executable SHA-256 fingerprint recorded at runtime",
            transfer_syntax_uid: HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID,
            feature_gate: Some("htj2k_openjph"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 16 || input.bits_stored != 16 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "HTJ2K Lossless first-case support is limited to 16-bit source frames",
            ));
        }
        if input.samples_per_pixel != 1 || input.photometric_interpretation != "MONOCHROME2" {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "HTJ2K Lossless first-case support currently requires MONOCHROME2 input",
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

        let identity = self.discover_backend_identity()?;
        let dir = unique_openjph_temp_dir();
        let encode_result = (|| {
            fs::create_dir_all(&dir).map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to create OpenJPH temporary directory {}: {err}",
                        dir.display()
                    ),
                )
            })?;
            let input_path = dir.join("frame.pgm");
            let output_path = dir.join("frame_htj2k.j2c");
            fs::write(
                &input_path,
                pgm_u16_mono2_from_native_le(
                    Self::BACKEND_ID,
                    input.columns,
                    input.rows,
                    input.native_frame,
                )?,
            )
            .map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to write OpenJPH PGM input {}: {err}",
                        input_path.display()
                    ),
                )
            })?;

            let output = Command::new(&identity.executable_path)
                .arg("-i")
                .arg(&input_path)
                .arg("-o")
                .arg(&output_path)
                .arg("-reversible")
                .arg("true")
                .arg("-num_decomps")
                .arg("1")
                .output()
                .map_err(|err| {
                    CodecError::encode_failed(
                        Self::BACKEND_ID,
                        format!("failed to run OpenJPH command: {err}"),
                    )
                })?;
            if !output.status.success() {
                return Err(CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "ojph_compress failed with status {:?}: stdout={}, stderr={}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    ),
                ));
            }

            fs::read(&output_path).map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to read OpenJPH codestream {}: {err}",
                        output_path.display()
                    ),
                )
            })
        })();
        let _ = fs::remove_dir_all(&dir);

        let encoded = encode_result?;
        if encoded.len() < 4 || encoded[..2] != [0xff, 0x4f] {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "HTJ2K codestream is missing the SOC marker",
            ));
        }
        if encoded[encoded.len() - 2..] != [0xff, 0xd9] {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "HTJ2K codestream is missing the EOC marker",
            ));
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "htj2k_openjph")]
impl FrameDecoder for OpenJphHtj2kLosslessEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID,
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
            HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs HTJ2K Lossless reader is not available",
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "htj2k_openjph")]
pub struct OpenJphHtj2kLossyEncoder {
    command: PathBuf,
}

#[cfg(feature = "htj2k_openjph")]
impl Default for OpenJphHtj2kLossyEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "htj2k_openjph")]
impl OpenJphHtj2kLossyEncoder {
    pub const BACKEND_ID: &'static str = "openjph_htj2k_lossy_command_writer";
    pub const COMMAND: &'static str = "ojph_compress";
    pub const QSTEP: &'static str = "0.00025";
    pub const NUM_DECOMPOSITIONS: &'static str = "2";
    pub const PROGRESSION_ORDER: &'static str = "LRCP";
    pub const LOSSY_IMAGE_COMPRESSION_METHOD: &'static str = "ISO_15444_15";
    pub const DECODER_ID: &'static str = "dicom_rs_openjpeg_htj2k_decoder";
    pub const DECODER_VERSION: &'static str =
        "dicom-transfer-syntax-registry 0.9.1 + jpeg2k 0.10.1 + openjp2 0.6.1";
    pub const DECODER_INDEPENDENCE: &'static str = "independent";

    pub fn fixed_option_arguments() -> Vec<String> {
        vec![
            "-reversible".to_string(),
            "false".to_string(),
            "-qstep".to_string(),
            Self::QSTEP.to_string(),
            "-num_decomps".to_string(),
            Self::NUM_DECOMPOSITIONS.to_string(),
            "-colour_trans".to_string(),
            "false".to_string(),
            "-prog_order".to_string(),
            Self::PROGRESSION_ORDER.to_string(),
        ]
    }

    pub fn new() -> Self {
        Self {
            command: PathBuf::from(Self::COMMAND),
        }
    }

    pub fn with_command(command: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn discover_backend_identity(&self) -> Result<ExternalCommandBackendIdentity, CodecError> {
        let executable_path = resolve_command_path(&self.command, Self::BACKEND_ID, "OpenJPH")?;
        let executable_bytes = fs::read(&executable_path).map_err(|err| {
            CodecError::unavailable(
                Self::BACKEND_ID,
                format!(
                    "failed to read OpenJPH executable {} for fingerprinting: {err}",
                    executable_path.display()
                ),
            )
        })?;
        Ok(ExternalCommandBackendIdentity {
            command: Self::COMMAND,
            executable_path,
            executable_sha256: crate::sha256_hex(&executable_bytes),
            version: None,
            version_source: "executable_sha256",
        })
    }
}

#[cfg(feature = "htj2k_openjph")]
impl FrameEncoder for OpenJphHtj2kLossyEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::ExternalCommand,
            display_name: "OpenJPH HTJ2K lossy external command writer",
            version: "OpenJPH 0.27.3 executable SHA-256 recorded at runtime",
            transfer_syntax_uid: HTJ2K_LOSSY_TRANSFER_SYNTAX_UID,
            feature_gate: Some("htj2k_openjph"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 16 || input.bits_stored != 16 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "HTJ2K lossy support requires unsigned 16-bit samples",
            ));
        }
        if input.samples_per_pixel != 1 || input.photometric_interpretation != "MONOCHROME2" {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "HTJ2K lossy support requires MONOCHROME2 input",
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

        let identity = self.discover_backend_identity()?;
        let dir = unique_codec_temp_dir("openjph-htj2k-lossy");
        let encode_result = (|| {
            fs::create_dir_all(&dir).map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to create OpenJPH temporary directory {}: {err}",
                        dir.display()
                    ),
                )
            })?;
            let input_path = dir.join("frame.pgm");
            let output_path = dir.join("frame_htj2k.j2c");
            fs::write(
                &input_path,
                pgm_u16_mono2_from_native_le(
                    Self::BACKEND_ID,
                    input.columns,
                    input.rows,
                    input.native_frame,
                )?,
            )
            .map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to write OpenJPH PGM input {}: {err}",
                        input_path.display()
                    ),
                )
            })?;
            let result = Command::new(&identity.executable_path)
                .arg("-i")
                .arg(&input_path)
                .arg("-o")
                .arg(&output_path)
                .args(Self::fixed_option_arguments())
                .output()
                .map_err(|err| {
                    CodecError::encode_failed(
                        Self::BACKEND_ID,
                        format!("failed to run OpenJPH command: {err}"),
                    )
                })?;
            if !result.status.success() {
                return Err(CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "ojph_compress failed with status {:?}: stdout={}, stderr={}",
                        result.status.code(),
                        String::from_utf8_lossy(&result.stdout),
                        String::from_utf8_lossy(&result.stderr)
                    ),
                ));
            }
            fs::read(&output_path).map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to read OpenJPH codestream {}: {err}",
                        output_path.display()
                    ),
                )
            })
        })();
        let _ = fs::remove_dir_all(&dir);

        let encoded = encode_result?;
        if encoded.len() < 4 || encoded[..2] != [0xff, 0x4f] {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "HTJ2K codestream is missing the SOC marker",
            ));
        }
        if encoded[encoded.len() - 2..] != [0xff, 0xd9] {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "HTJ2K codestream is missing the EOC marker",
            ));
        }
        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "htj2k_openjph")]
impl FrameDecoder for OpenJphHtj2kLossyEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: HTJ2K_LOSSY_TRANSFER_SYNTAX_UID,
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
            HIGH_THROUGHPUT_JPEG_2000_IMAGE_COMPRESSION.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs OpenJPEG HTJ2K reader is unavailable",
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "jpegxl")]
pub struct CjxlJpegXlLossyEncoder {
    command: PathBuf,
}

#[cfg(feature = "jpegxl")]
impl Default for CjxlJpegXlLossyEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "jpegxl")]
impl CjxlJpegXlLossyEncoder {
    pub const BACKEND_ID: &'static str = "cjxl_jpegxl_lossy_command_writer";
    pub const COMMAND: &'static str = "cjxl";
    pub const DISTANCE: &'static str = "0.05";
    pub const EFFORT: &'static str = "7";
    pub const NUM_THREADS: &'static str = "0";
    pub const LOSSY_IMAGE_COMPRESSION_METHOD: &'static str = "ISO_18181_1";
    pub const DECODER_ID: &'static str = "dicom_rs_jxl_oxide_decoder";
    pub const DECODER_VERSION: &'static str =
        "dicom-transfer-syntax-registry 0.9.1 + jxl-oxide 0.10.2";
    pub const DECODER_INDEPENDENCE: &'static str = "independent";

    pub fn new() -> Self {
        Self {
            command: PathBuf::from(Self::COMMAND),
        }
    }

    pub fn with_command(command: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn fixed_option_arguments() -> Vec<String> {
        vec![
            format!("--distance={}", Self::DISTANCE),
            format!("--effort={}", Self::EFFORT),
            format!("--num_threads={}", Self::NUM_THREADS),
            "--container=0".to_string(),
            "--modular=0".to_string(),
            "--quiet".to_string(),
        ]
    }

    pub fn discover_backend_identity(&self) -> Result<ExternalCommandBackendIdentity, CodecError> {
        let executable_path =
            resolve_command_path(&self.command, Self::BACKEND_ID, "JPEG XL cjxl")?;
        let executable_bytes = fs::read(&executable_path).map_err(|err| {
            CodecError::unavailable(
                Self::BACKEND_ID,
                format!(
                    "failed to read cjxl executable {} for fingerprinting: {err}",
                    executable_path.display()
                ),
            )
        })?;
        let version_output = Command::new(&executable_path)
            .arg("--version")
            .output()
            .map_err(|err| {
                CodecError::unavailable(
                    Self::BACKEND_ID,
                    format!("failed to query cjxl version: {err}"),
                )
            })?;
        if !version_output.status.success() {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                format!(
                    "cjxl --version failed with status {:?}",
                    version_output.status.code()
                ),
            ));
        }
        let version = String::from_utf8_lossy(&version_output.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        if version.is_empty() {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "cjxl --version returned no version text",
            ));
        }

        Ok(ExternalCommandBackendIdentity {
            command: Self::COMMAND,
            executable_path,
            executable_sha256: crate::sha256_hex(&executable_bytes),
            version: Some(version),
            version_source: "command_version_and_executable_sha256",
        })
    }
}

#[cfg(feature = "jpegxl")]
impl FrameEncoder for CjxlJpegXlLossyEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::ExternalCommand,
            display_name: "cjxl JPEG XL lossy external command writer",
            version: "cjxl 0.11.2 version and executable SHA-256 recorded at runtime",
            transfer_syntax_uid: JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID,
            feature_gate: Some("jpegxl"),
            determinism: CodecDeterminism::SemanticStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        if input.bits_allocated != 8 || input.bits_stored != 8 {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG XL lossy support requires unsigned 8-bit samples",
            ));
        }
        if input.samples_per_pixel != 3 || input.photometric_interpretation != "RGB" {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                "JPEG XL lossy support requires interleaved RGB input",
            ));
        }
        let expected_len = usize::from(input.rows)
            .checked_mul(usize::from(input.columns))
            .and_then(|pixels| pixels.checked_mul(3))
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

        let identity = self.discover_backend_identity()?;
        let dir = unique_codec_temp_dir("cjxl-jpegxl-lossy");
        let encode_result = (|| {
            fs::create_dir_all(&dir).map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to create cjxl temporary directory {}: {err}",
                        dir.display()
                    ),
                )
            })?;
            let input_path = dir.join("frame.ppm");
            let output_path = dir.join("frame.jxl");
            fs::write(
                &input_path,
                ppm_rgb8(input.columns, input.rows, input.native_frame)?,
            )
            .map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to write cjxl PPM input {}: {err}",
                        input_path.display()
                    ),
                )
            })?;
            let result = Command::new(&identity.executable_path)
                .arg(&input_path)
                .arg(&output_path)
                .args(Self::fixed_option_arguments())
                .output()
                .map_err(|err| {
                    CodecError::encode_failed(
                        Self::BACKEND_ID,
                        format!("failed to run cjxl command: {err}"),
                    )
                })?;
            if !result.status.success() {
                return Err(CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "cjxl failed with status {:?}: stdout={}, stderr={}",
                        result.status.code(),
                        String::from_utf8_lossy(&result.stdout),
                        String::from_utf8_lossy(&result.stderr)
                    ),
                ));
            }
            fs::read(&output_path).map_err(|err| {
                CodecError::encode_failed(
                    Self::BACKEND_ID,
                    format!(
                        "failed to read cjxl codestream {}: {err}",
                        output_path.display()
                    ),
                )
            })
        })();
        let _ = fs::remove_dir_all(&dir);

        let encoded = encode_result?;
        if !encoded.starts_with(&[0xff, 0x0a]) {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "JPEG XL output is not a raw codestream",
            ));
        }
        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "jpegxl")]
impl FrameDecoder for CjxlJpegXlLossyEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID,
            rows: input.rows,
            columns: input.columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: input.bits_allocated,
            bits_stored: input.bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.encoded_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) = JPEG_XL.codec() else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs jxl-oxide JPEG XL reader is unavailable",
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
    feature = "deflate",
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
    feature = "deflate",
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
#[cfg(feature = "deflate")]
pub struct DicomRsDeflatedImageFrameEncoder;

#[cfg(feature = "deflate")]
impl DicomRsDeflatedImageFrameEncoder {
    pub const BACKEND_ID: &'static str = "dicom_rs_deflated_image_frame_writer";

    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "deflate")]
impl FrameEncoder for DicomRsDeflatedImageFrameEncoder {
    fn backend(&self) -> CodecBackendInfo {
        CodecBackendInfo {
            backend_id: Self::BACKEND_ID,
            backend_kind: CodecBackendKind::DicomRsFeature,
            display_name: "DICOM-rs Deflated Image Frame writer",
            version: "dicom-transfer-syntax-registry 0.9.1 + flate2",
            transfer_syntax_uid: DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID,
            feature_gate: Some("deflate"),
            determinism: CodecDeterminism::ByteStable,
        }
    }

    fn encode_frame(&self, input: FrameEncodeInput<'_>) -> Result<EncodedFrame, CodecError> {
        let expected_len = deflated_frame_byte_len(input)?;
        if input.native_frame.len() != expected_len {
            return Err(CodecError::unsupported(
                Self::BACKEND_ID,
                format!(
                    "native frame length is {}, expected {expected_len}",
                    input.native_frame.len()
                ),
            ));
        }

        let (adapter_rows, adapter_columns, adapter_bits_allocated, adapter_bits_stored) =
            if input.bits_allocated == 1 {
                (
                    1,
                    u16::try_from(input.native_frame.len()).map_err(|_| {
                        CodecError::unsupported(
                            Self::BACKEND_ID,
                            "bit-packed frame byte length exceeds u16 columns",
                        )
                    })?,
                    8,
                    8,
                )
            } else {
                (
                    input.rows,
                    input.columns,
                    input.bits_allocated,
                    input.bits_stored,
                )
            };

        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID,
            rows: adapter_rows,
            columns: adapter_columns,
            samples_per_pixel: input.samples_per_pixel,
            bits_allocated: adapter_bits_allocated,
            bits_stored: adapter_bits_stored,
            photometric_interpretation: input.photometric_interpretation,
            fragments: vec![input.native_frame.to_vec()],
            offset_table: Vec::new(),
        };
        let Codec::EncapsulatedPixelData(_, Some(writer)) =
            DEFLATED_IMAGE_FRAME_COMPRESSION.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs Deflated Image Frame writer is not available",
            ));
        };

        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, EncodeOptions::default(), &mut encoded)
            .map_err(|err| CodecError::encode_failed(Self::BACKEND_ID, err.to_string()))?;
        if encoded.is_empty() {
            return Err(CodecError::validation_failed(
                Self::BACKEND_ID,
                "Deflated Image Frame encoder produced an empty fragment",
            ));
        }

        Ok(EncodedFrame { bytes: encoded })
    }
}

#[cfg(feature = "deflate")]
impl FrameDecoder for DicomRsDeflatedImageFrameEncoder {
    fn backend(&self) -> CodecBackendInfo {
        <Self as FrameEncoder>::backend(self)
    }

    fn decode_frame(&self, input: FrameDecodeInput<'_>) -> Result<DecodedFrame, CodecError> {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID,
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
            DEFLATED_IMAGE_FRAME_COMPRESSION.codec()
        else {
            return Err(CodecError::unavailable(
                Self::BACKEND_ID,
                "DICOM-rs Deflated Image Frame reader is not available",
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
                // Annex G orders each sample's byte planes from most to least
                // significant. Native frames use DICOM's little-endian sample
                // representation, so the corresponding native byte index runs
                // in the opposite direction.
                let native_byte = bytes_per_sample - 1 - byte_plane;
                let mut segment = Vec::with_capacity(pixels);
                for pixel in 0..pixels {
                    let offset =
                        ((pixel * samples_per_pixel + sample) * bytes_per_sample) + native_byte;
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
                let native_byte = bytes_per_sample - 1 - byte_plane;
                for pixel in 0..pixels {
                    let offset =
                        ((pixel * samples_per_pixel + sample) * bytes_per_sample) + native_byte;
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

#[cfg(feature = "deflate")]
fn deflated_frame_byte_len(input: FrameEncodeInput<'_>) -> Result<usize, CodecError> {
    let samples = usize::from(input.rows)
        .checked_mul(usize::from(input.columns))
        .and_then(|pixels| pixels.checked_mul(usize::from(input.samples_per_pixel)))
        .ok_or_else(|| {
            CodecError::unsupported(
                DicomRsDeflatedImageFrameEncoder::BACKEND_ID,
                "frame sample count overflowed",
            )
        })?;
    if input.bits_allocated == 1 {
        if input.samples_per_pixel != 1 {
            return Err(CodecError::unsupported(
                DicomRsDeflatedImageFrameEncoder::BACKEND_ID,
                "1-bit Deflated Image Frame support requires one sample per pixel",
            ));
        }
        Ok(samples.div_ceil(8))
    } else {
        let bytes_per_sample = checked_bytes_per_sample(
            DicomRsDeflatedImageFrameEncoder::BACKEND_ID,
            input.bits_allocated,
        )?;
        samples.checked_mul(bytes_per_sample).ok_or_else(|| {
            CodecError::unsupported(
                DicomRsDeflatedImageFrameEncoder::BACKEND_ID,
                "native frame length overflowed",
            )
        })
    }
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

#[cfg(any(
    feature = "htj2k_openjph",
    feature = "legacy_jpeg_dcmtk",
    feature = "jpegxl"
))]
fn resolve_command_path(
    command: &Path,
    backend_id: &'static str,
    display_name: &str,
) -> Result<PathBuf, CodecError> {
    if command.is_absolute() || command.components().count() > 1 {
        return canonical_existing_command(command, backend_id, display_name);
    }

    let path_var = env::var_os("PATH").ok_or_else(|| {
        CodecError::unavailable(
            backend_id,
            format!(
                "PATH is not set, so {} cannot be discovered",
                command.display()
            ),
        )
    })?;

    for dir in env::split_paths(&path_var) {
        for candidate in command_candidates(&dir, command) {
            if candidate.is_file() {
                return candidate.canonicalize().map_err(|err| {
                    CodecError::unavailable(
                        backend_id,
                        format!(
                            "failed to canonicalize {display_name} executable {}: {err}",
                            candidate.display()
                        ),
                    )
                });
            }
        }
    }

    Err(CodecError::unavailable(
        backend_id,
        format!("{} was not found on PATH", command.display()),
    ))
}

#[cfg(any(
    feature = "htj2k_openjph",
    feature = "legacy_jpeg_dcmtk",
    feature = "jpegxl"
))]
fn canonical_existing_command(
    command: &Path,
    backend_id: &'static str,
    display_name: &str,
) -> Result<PathBuf, CodecError> {
    if !command.is_file() {
        return Err(CodecError::unavailable(
            backend_id,
            format!(
                "{display_name} executable {} does not exist",
                command.display()
            ),
        ));
    }
    command.canonicalize().map_err(|err| {
        CodecError::unavailable(
            backend_id,
            format!(
                "failed to canonicalize {display_name} executable {}: {err}",
                command.display()
            ),
        )
    })
}

#[cfg(all(
    any(
        feature = "htj2k_openjph",
        feature = "legacy_jpeg_dcmtk",
        feature = "jpegxl"
    ),
    not(windows)
))]
fn command_candidates(dir: &Path, command: &Path) -> Vec<PathBuf> {
    vec![dir.join(command)]
}

#[cfg(all(
    any(
        feature = "htj2k_openjph",
        feature = "legacy_jpeg_dcmtk",
        feature = "jpegxl"
    ),
    windows
))]
fn command_candidates(dir: &Path, command: &Path) -> Vec<PathBuf> {
    if command.extension().is_some() {
        return vec![dir.join(command)];
    }

    let pathext = env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    env::split_paths(&pathext)
        .map(|extension| {
            let extension = extension.to_string_lossy();
            dir.join(format!("{}{}", command.display(), extension))
        })
        .chain(std::iter::once(dir.join(command)))
        .collect()
}

#[cfg(feature = "jpegxl")]
fn ppm_rgb8(columns: u16, rows: u16, native_frame: &[u8]) -> Result<Vec<u8>, CodecError> {
    let expected_len = usize::from(columns)
        .checked_mul(usize::from(rows))
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| {
            CodecError::unsupported(
                CjxlJpegXlLossyEncoder::BACKEND_ID,
                "PPM input length overflowed",
            )
        })?;
    if native_frame.len() != expected_len {
        return Err(CodecError::unsupported(
            CjxlJpegXlLossyEncoder::BACKEND_ID,
            format!(
                "native frame length is {}, expected {expected_len}",
                native_frame.len()
            ),
        ));
    }
    let mut bytes = format!("P6\n{columns} {rows}\n255\n").into_bytes();
    bytes.extend_from_slice(native_frame);
    Ok(bytes)
}

#[cfg(feature = "htj2k_openjph")]
fn pgm_u16_mono2_from_native_le(
    backend_id: &'static str,
    columns: u16,
    rows: u16,
    native_frame: &[u8],
) -> Result<Vec<u8>, CodecError> {
    let expected_len = usize::from(columns)
        .checked_mul(usize::from(rows))
        .and_then(|samples| samples.checked_mul(2))
        .ok_or_else(|| CodecError::unsupported(backend_id, "PGM input length overflowed"))?;
    if native_frame.len() != expected_len {
        return Err(CodecError::unsupported(
            backend_id,
            format!(
                "native frame length is {}, expected {expected_len}",
                native_frame.len()
            ),
        ));
    }

    let mut bytes = format!("P5\n{columns} {rows}\n65535\n").into_bytes();
    for sample in native_frame.chunks_exact(2) {
        let sample = u16::from_le_bytes([sample[0], sample[1]]);
        bytes.extend_from_slice(&sample.to_be_bytes());
    }
    Ok(bytes)
}

#[cfg(feature = "htj2k_openjph")]
fn unique_openjph_temp_dir() -> PathBuf {
    unique_codec_temp_dir("openjph-htj2k")
}

#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
fn unique_codec_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!("dts-{label}-{}-{nonce}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(
        feature = "charls",
        feature = "deflate",
        feature = "jpeg",
        feature = "jpegxl"
    ))]
    use std::borrow::Cow;

    #[cfg(any(
        feature = "charls",
        feature = "deflate",
        feature = "jpeg",
        feature = "jpegxl"
    ))]
    use dicom_core::value::C;
    #[cfg(any(
        feature = "charls",
        feature = "deflate",
        feature = "jpeg",
        feature = "jpegxl",
        feature = "jpeg2000"
    ))]
    use dicom_encoding::Codec;
    #[cfg(any(
        feature = "charls",
        feature = "deflate",
        feature = "jpeg",
        feature = "jpegxl"
    ))]
    use dicom_encoding::adapters::{EncodeOptions, PixelDataObject, PixelDataWriter, RawPixelData};
    #[cfg(feature = "deflate")]
    use dicom_transfer_syntax_registry::entries::DEFLATED_IMAGE_FRAME_COMPRESSION;
    #[cfg(feature = "jpeg2000")]
    use dicom_transfer_syntax_registry::entries::JPEG_2000_IMAGE_COMPRESSION_LOSSLESS_ONLY;
    #[cfg(feature = "jpeg")]
    use dicom_transfer_syntax_registry::entries::JPEG_BASELINE;
    #[cfg(feature = "charls")]
    use dicom_transfer_syntax_registry::entries::JPEG_LS_LOSSLESS_IMAGE_COMPRESSION;
    #[cfg(feature = "jpegxl")]
    use dicom_transfer_syntax_registry::entries::JPEG_XL_LOSSLESS;

    #[test]
    fn lossy_metrics_cover_every_interleaved_channel_and_sample() {
        let metrics = calculate_lossy_frame_metrics(
            &[10, 20, 30, 40, 50, 60],
            &[12, 19, 34, 40, 47, 54],
            1,
            2,
            3,
            8,
        )
        .expect("RGB metrics should calculate");

        assert_eq!(metrics.sample_domain.as_str(), "unsigned_8_bit");
        assert_eq!(metrics.sample_count, 6);
        assert_eq!(metrics.channels.len(), 3);
        assert_eq!(metrics.channels[0].max_absolute_error, 2);
        assert_eq!(metrics.channels[1].max_absolute_error, 3);
        assert_eq!(metrics.channels[2].max_absolute_error, 6);
        assert!((metrics.channels[0].rmse - 2_f64.sqrt()).abs() < 1e-12);
        assert!((metrics.channels[1].rmse - 5_f64.sqrt()).abs() < 1e-12);
        assert!((metrics.channels[2].rmse - (26_f64).sqrt()).abs() < 1e-12);
        assert!((metrics.overall_rmse - (66_f64 / 6_f64).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn lossy_metrics_decode_unsigned_u16_little_endian() {
        let reference = [0x00, 0x01, 0xff, 0xff];
        let decoded = [0x40, 0x01, 0xbf, 0xff];
        let metrics = calculate_lossy_frame_metrics(&reference, &decoded, 1, 2, 1, 16)
            .expect("u16 metrics should calculate");

        assert_eq!(
            metrics.sample_domain,
            LossySampleDomain::Unsigned16LittleEndian
        );
        assert_eq!(metrics.channels[0].max_absolute_error, 64);
        assert_eq!(metrics.channels[0].rmse, 64.0);
        assert_eq!(metrics.overall_rmse, 64.0);
    }

    #[test]
    fn lossy_metrics_reject_shape_and_domain_mismatches() {
        let shape = calculate_lossy_frame_metrics(&[0; 3], &[0; 4], 2, 2, 1, 8)
            .expect_err("short reference must fail");
        assert_eq!(shape.backend_id(), "lossy_frame_metric_calculator");
        assert!(shape.to_string().contains("requires 4 bytes"));

        let domain = calculate_lossy_frame_metrics(&[], &[], 1, 1, 1, 12)
            .expect_err("unsupported sample domain must fail");
        assert!(domain.to_string().contains("8-bit and 16-bit"));
    }

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
    fn native_rle_can_emit_odd_length_encoded_frames() {
        let encoder = NativeRleLosslessEncoder::new();

        let encoded = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &[0, 255],
                rows: 1,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("RLE should encode a two-sample frame");

        assert_eq!(&encoded.bytes[0..4], &1u32.to_le_bytes());
        assert_eq!(&encoded.bytes[4..8], &64u32.to_le_bytes());
        assert_eq!(&encoded.bytes[64..], &[1, 0, 255]);
        assert_eq!(encoded.bytes.len() % 2, 1);
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
        assert_eq!(&encoded.bytes[64..67], &[1, 0x12, 0xab]);
        assert_eq!(&encoded.bytes[67..70], &[1, 0x34, 0xcd]);
    }

    #[test]
    fn native_rle_decodes_byte_planes_into_native_sample_order() {
        let codec = NativeRleLosslessEncoder::new();
        let encoded = [2, 0, 0, 0, 64, 0, 0, 0, 67, 0, 0, 0]
            .iter()
            .copied()
            .chain([0; 52])
            .chain([1, 0x12, 0xab])
            .chain([1, 0x34, 0xcd])
            .collect::<Vec<_>>();

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded,
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
    fn native_rle_round_trips_rgb_planar0_as_three_segments() {
        let codec = NativeRleLosslessEncoder::new();
        let native = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];

        let encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows: 2,
                columns: 2,
                samples_per_pixel: 3,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "RGB",
            })
            .expect("RLE should encode a tiny 8-bit RGB frame");

        assert_eq!(&encoded.bytes[0..4], &3u32.to_le_bytes());
        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 2,
                columns: 2,
                samples_per_pixel: 3,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "RGB",
            })
            .expect("RGB RLE frame should decode");

        assert_eq!(decoded.native_bytes, native);
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

    #[cfg(feature = "deflate")]
    #[test]
    fn dicom_rs_deflated_image_frame_backend_reports_identity() {
        let encoder = DicomRsDeflatedImageFrameEncoder::new();

        let backend = FrameEncoder::backend(&encoder);

        assert_eq!(backend.backend_id, "dicom_rs_deflated_image_frame_writer");
        assert_eq!(backend.backend_kind.as_str(), "dicom_rs_feature");
        assert_eq!(backend.transfer_syntax_uid, "1.2.840.10008.1.2.8.1");
        assert_eq!(backend.feature_gate, Some("deflate"));
        assert_eq!(backend.determinism.as_str(), "byte_stable");
    }

    #[cfg(feature = "deflate")]
    #[test]
    fn dicom_rs_deflated_image_frame_round_trips_bit_packed_seg_frame() {
        let codec = DicomRsDeflatedImageFrameEncoder::new();
        let native = [0b0000_1001];

        let encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 1,
                bits_stored: 1,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("Deflated Image Frame should encode a bit-packed SEG frame");
        assert!(
            !encoded.bytes.is_empty(),
            "deflated fragment should not be empty"
        );

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 2,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 1,
                bits_stored: 1,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("Deflated Image Frame should decode the bit-packed SEG frame");

        assert_eq!(decoded.native_bytes, native);
    }

    #[cfg(feature = "deflate")]
    #[test]
    fn dicom_rs_deflate_feature_exposes_deflated_image_frame_writer() {
        let Codec::EncapsulatedPixelData(Some(reader), Some(writer)) =
            DEFLATED_IMAGE_FRAME_COMPRESSION.codec()
        else {
            panic!(
                "Deflated Image Frame transfer syntax must expose reader and writer with the deflate feature"
            );
        };

        let obj = NativePixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            rows: 1,
            columns: 1,
            samples_per_pixel: 1,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "MONOCHROME2",
            pixels: &[0b0000_1001],
        };
        let mut encoded = Vec::new();
        writer
            .encode_frame(&obj, 0, EncodeOptions::default(), &mut encoded)
            .expect("DICOM-rs Deflated Image Frame writer should encode a tiny byte frame");
        assert!(!encoded.is_empty());

        let encoded_obj = EncodedPixelTestObject {
            transfer_syntax_uid: "1.2.840.10008.1.2.8.1",
            rows: 1,
            columns: 1,
            samples_per_pixel: 1,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "MONOCHROME2",
            fragments: vec![encoded],
        };
        let mut decoded = Vec::new();
        reader
            .decode_frame(&encoded_obj, 0, &mut decoded)
            .expect("DICOM-rs Deflated Image Frame reader should decode its own fragment");
        assert_eq!(decoded, vec![0b0000_1001]);
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
    fn cjxl_lossy_rgb_meets_policy_and_is_reproducible() {
        let codec = CjxlJpegXlLossyEncoder::new();
        let backend = FrameEncoder::backend(&codec);
        assert_eq!(
            backend.transfer_syntax_uid,
            JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID
        );
        assert_eq!(backend.backend_kind, CodecBackendKind::ExternalCommand);
        assert_eq!(CjxlJpegXlLossyEncoder::DECODER_INDEPENDENCE, "independent");
        let identity = match codec.discover_backend_identity() {
            Ok(identity) => identity,
            Err(CodecError::Unavailable { reason, .. }) if reason.contains("not found") => {
                eprintln!("skipping cjxl lossy proof because {reason}");
                return;
            }
            Err(error) => panic!("cjxl discovery should succeed: {error}"),
        };
        assert!(
            identity
                .version
                .as_deref()
                .is_some_and(|version| version.contains("0.11.2"))
        );
        assert_eq!(identity.executable_sha256.len(), 64);

        let mut native = Vec::with_capacity(32 * 32 * 3);
        for row in 0..32_u16 {
            for column in 0..32_u16 {
                let bar = (column / 4) as u8;
                native.extend_from_slice(&[
                    if row < 16 {
                        (column * 8) as u8
                    } else {
                        bar * 32
                    },
                    if column < 16 {
                        (row * 8) as u8
                    } else {
                        255 - bar * 32
                    },
                    if (row / 4 + column / 4) % 2 == 0 {
                        16
                    } else {
                        240
                    },
                ]);
            }
        }
        let input = FrameEncodeInput {
            native_frame: &native,
            rows: 32,
            columns: 32,
            samples_per_pixel: 3,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation: "RGB",
        };
        let encoded = codec
            .encode_frame(input)
            .expect("cjxl should encode RGB diagnostic frame");
        let repeated = codec
            .encode_frame(input)
            .expect("cjxl should reproduce fixed options");
        assert_eq!(encoded.bytes, repeated.bytes);
        assert!(encoded.bytes.starts_with(&[0xff, 0x0a]));

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 32,
                columns: 32,
                samples_per_pixel: 3,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "RGB",
            })
            .expect("independent jxl-oxide adapter should decode cjxl output");
        let metrics = calculate_lossy_frame_metrics(&native, &decoded.native_bytes, 32, 32, 3, 8)
            .expect("JPEG XL metrics should calculate");
        eprintln!(
            "cjxl distance={} effort={} bytes={} metrics={metrics:?}",
            CjxlJpegXlLossyEncoder::DISTANCE,
            CjxlJpegXlLossyEncoder::EFFORT,
            encoded.bytes.len()
        );
        assert!(
            metrics
                .channels
                .iter()
                .all(|channel| channel.max_absolute_error <= 8)
        );
        assert!(metrics.overall_rmse <= 3.0);
    }

    #[cfg(feature = "jpegxl")]
    #[test]
    fn cjxl_lossy_reports_controlled_unavailable_command() {
        let error = CjxlJpegXlLossyEncoder::with_command("/definitely/missing/cjxl")
            .discover_backend_identity()
            .expect_err("missing cjxl should be unavailable");
        assert!(matches!(error, CodecError::Unavailable { .. }));
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

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn openjph_htj2k_lossy_meets_policy_and_is_reproducible() {
        let codec = OpenJphHtj2kLossyEncoder::new();
        let backend = FrameEncoder::backend(&codec);
        assert_eq!(backend.transfer_syntax_uid, HTJ2K_LOSSY_TRANSFER_SYNTAX_UID);
        assert_eq!(backend.backend_kind, CodecBackendKind::ExternalCommand);
        assert_eq!(
            OpenJphHtj2kLossyEncoder::DECODER_INDEPENDENCE,
            "independent"
        );
        if let Err(CodecError::Unavailable { reason, .. }) = codec.discover_backend_identity() {
            if reason.contains("not found") {
                eprintln!("skipping OpenJPH HTJ2K lossy proof because {reason}");
                return;
            }
        }
        let mut native = Vec::with_capacity(32 * 32 * 2);
        for row in 0..32_u32 {
            for column in 0..32_u32 {
                let sample = if row < 8 {
                    column * 2048
                } else if row < 16 {
                    if column < 16 { 0 } else { 65535 }
                } else if (row / 4 + column / 4) % 2 == 0 {
                    4096
                } else {
                    61440
                };
                native.extend_from_slice(&(sample.min(65535) as u16).to_le_bytes());
            }
        }
        let input = FrameEncodeInput {
            native_frame: &native,
            rows: 32,
            columns: 32,
            samples_per_pixel: 1,
            bits_allocated: 16,
            bits_stored: 16,
            photometric_interpretation: "MONOCHROME2",
        };
        let encoded = codec
            .encode_frame(input)
            .expect("OpenJPH should encode HTJ2K lossy diagnostic frame");
        let repeated = codec
            .encode_frame(input)
            .expect("OpenJPH should reproduce fixed lossy options");
        assert_eq!(encoded.bytes, repeated.bytes);
        assert_eq!(&encoded.bytes[..2], &[0xff, 0x4f]);
        assert_eq!(&encoded.bytes[encoded.bytes.len() - 2..], &[0xff, 0xd9]);

        let decoded = codec
            .decode_frame(FrameDecodeInput {
                encoded_frame: &encoded.bytes,
                rows: 32,
                columns: 32,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("independent OpenJPEG adapter should decode OpenJPH output");
        let metrics = calculate_lossy_frame_metrics(&native, &decoded.native_bytes, 32, 32, 1, 16)
            .expect("HTJ2K metrics should calculate");
        eprintln!(
            "OpenJPH qstep={} decompositions={} bytes={} metrics={metrics:?}",
            OpenJphHtj2kLossyEncoder::QSTEP,
            OpenJphHtj2kLossyEncoder::NUM_DECOMPOSITIONS,
            encoded.bytes.len()
        );
        assert!(metrics.channels[0].max_absolute_error <= 64);
        assert!(metrics.overall_rmse <= 16.0);
    }

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn openjph_htj2k_lossy_reports_controlled_unavailable_command() {
        let error = OpenJphHtj2kLossyEncoder::with_command("/definitely/missing/ojph_compress")
            .discover_backend_identity()
            .expect_err("missing OpenJPH should be unavailable");
        assert!(matches!(error, CodecError::Unavailable { .. }));
    }

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn openjph_htj2k_lossless_backend_reports_identity() {
        let encoder = OpenJphHtj2kLosslessEncoder::new();

        let backend = FrameEncoder::backend(&encoder);

        assert_eq!(backend.backend_id, "openjph_htj2k_lossless_command_writer");
        assert_eq!(backend.backend_kind.as_str(), "external_command");
        assert_eq!(backend.transfer_syntax_uid, "1.2.840.10008.1.2.4.201");
        assert_eq!(backend.feature_gate, Some("htj2k_openjph"));
        assert_eq!(backend.determinism.as_str(), "semantic_stable");
        assert!(backend.version.contains("executable SHA-256 fingerprint"));
    }

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn openjph_htj2k_lossless_discovers_executable_fingerprint() {
        let encoder = OpenJphHtj2kLosslessEncoder::new();

        let identity = match encoder.discover_backend_identity() {
            Ok(identity) => identity,
            Err(CodecError::Unavailable { reason, .. }) if reason.contains("not found") => {
                eprintln!("skipping OpenJPH HTJ2K fingerprint proof because {reason}");
                return;
            }
            Err(error) => panic!("OpenJPH backend discovery should not fail unexpectedly: {error}"),
        };

        assert_eq!(identity.command, "ojph_compress");
        assert!(
            identity.executable_path.is_file(),
            "resolved OpenJPH executable path should point to a file"
        );
        assert_eq!(identity.version, None);
        assert_eq!(identity.version_source, "executable_sha256");
        assert_eq!(identity.executable_sha256.len(), 64);
        assert!(
            identity
                .executable_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit()),
            "OpenJPH executable fingerprint should be hex SHA-256"
        );
    }

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn openjph_htj2k_lossless_wrapper_round_trips_u16_edges() {
        let codec = OpenJphHtj2kLosslessEncoder::new();
        if let Err(CodecError::Unavailable { reason, .. }) = codec.discover_backend_identity() {
            if reason.contains("not found") {
                eprintln!("skipping OpenJPH HTJ2K wrapper proof because {reason}");
                return;
            }
        }

        let samples = [0u16, 1, 32767, 32768, 65535, 0x1234, 0xabcd, 2];
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
            .expect("OpenJPH HTJ2K Lossless should encode a tiny 16-bit frame");
        let repeated_encoded = codec
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows: 2,
                columns: 4,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 16,
                photometric_interpretation: "MONOCHROME2",
            })
            .expect("OpenJPH HTJ2K Lossless should encode reproducibly");

        assert_eq!(
            crate::sha256_hex(&encoded.bytes),
            crate::sha256_hex(&repeated_encoded.bytes),
            "OpenJPH should produce byte-identical HTJ2K codestreams for fixed PGM input and options"
        );
        assert!(
            encoded.bytes.len() >= 4,
            "OpenJPH HTJ2K codestream should not be empty"
        );
        assert_eq!(
            &encoded.bytes[..2],
            &[0xff, 0x4f],
            "HTJ2K codestream must start with SOC"
        );
        assert_eq!(
            &encoded.bytes[encoded.bytes.len() - 2..],
            &[0xff, 0xd9],
            "HTJ2K codestream must end with EOC"
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
            .expect("DICOM-rs OpenJPEG-backed HTJ2K reader should decode OpenJPH output");

        assert_eq!(decoded.native_bytes, native);
    }

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn dicom_rs_htj2k_feature_exposes_lossless_reader_without_writer() {
        let obj = DicomRsPixelDataObject {
            transfer_syntax_uid: HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID,
            rows: 2,
            columns: 4,
            samples_per_pixel: 1,
            bits_allocated: 16,
            bits_stored: 16,
            photometric_interpretation: "MONOCHROME2",
            fragments: vec![vec![0xff, 0x4f, 0xff, 0xd9]],
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
        assert_eq!(obj.transfer_syntax_uid, HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID);
        let _ = reader;
    }

    #[cfg(any(
        feature = "charls",
        feature = "deflate",
        feature = "jpeg",
        feature = "jpegxl"
    ))]
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

    #[cfg(any(
        feature = "charls",
        feature = "deflate",
        feature = "jpeg",
        feature = "jpegxl"
    ))]
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

    #[cfg(any(feature = "charls", feature = "deflate", feature = "jpegxl"))]
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

    #[cfg(any(feature = "charls", feature = "deflate", feature = "jpegxl"))]
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
