//! Registry-backed encoded-frame codec execution.
//!
//! This layer consumes only verified native frame bindings. It never receives
//! an IOD dataset and cannot perform a full-file transform.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};

#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
use crate::codecs::CodecBackendInfo;
#[cfg(feature = "deflate")]
use crate::codecs::DicomRsDeflatedImageFrameEncoder;
#[cfg(feature = "jpeg")]
use crate::codecs::DicomRsJpegBaselineEncoder;
#[cfg(feature = "charls")]
use crate::codecs::DicomRsJpegLsLosslessEncoder;
#[cfg(feature = "jpeg2000")]
use crate::codecs::OpenJp2Jpeg2000LosslessEncoder;
#[cfg(feature = "jpegxl")]
use crate::codecs::{CjxlJpegXlLossyEncoder, DicomRsJpegXlLosslessEncoder};
use crate::codecs::{
    EncodedFrame, FrameDecodeInput, FrameDecoder, FrameEncodeInput, FrameEncoder,
    NativeRleLosslessEncoder, calculate_lossy_frame_metrics,
};
#[cfg(feature = "htj2k_openjph")]
use crate::codecs::{OpenJphHtj2kLosslessEncoder, OpenJphHtj2kLossyEncoder};
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::{CodecServiceOutcome, ServiceInvocationError};
use crate::executor::services::{
    ByteBinding, CodecRequest, CodecResult, EncodedFrameResult, ServiceEvidence, ToolIdentity,
};
use crate::recipes::{
    BackendBoundary, BackendDeterminism, CodecDispatchRequest, CodecEvidenceRequirement,
    CodecSourceRequest, TransferSyntaxBackendRegistry,
};
use crate::runtime_capabilities::{CapabilityInventory, QualifiedExecutableIdentity};
use crate::sha256_hex;

const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameCodecLimits {
    pub max_frames: usize,
    pub max_native_frame_bytes: usize,
    pub max_encoded_frame_bytes: usize,
    pub max_total_encoded_bytes: usize,
}

impl Default for FrameCodecLimits {
    fn default() -> Self {
        Self {
            max_frames: 4_096,
            max_native_frame_bytes: 256 * 1024 * 1024,
            max_encoded_frame_bytes: 256 * 1024 * 1024,
            max_total_encoded_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalFrameCodecCommands {
    pub openjph: Option<PathBuf>,
    pub cjxl: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RegisteredFrameCodecService {
    limits: FrameCodecLimits,
    external: ExternalFrameCodecCommands,
    expected_external: BTreeMap<String, QualifiedExecutableIdentity>,
}

impl Default for RegisteredFrameCodecService {
    fn default() -> Self {
        Self::new(
            FrameCodecLimits::default(),
            ExternalFrameCodecCommands::default(),
        )
        .expect("default frame codec limits are valid")
    }
}

impl RegisteredFrameCodecService {
    pub fn new(
        limits: FrameCodecLimits,
        external: ExternalFrameCodecCommands,
    ) -> Result<Self, ServiceInvocationError> {
        if limits.max_frames == 0
            || limits.max_native_frame_bytes == 0
            || limits.max_encoded_frame_bytes == 0
            || limits.max_total_encoded_bytes == 0
        {
            return Err(service_error("all codec limits must be greater than zero"));
        }
        Ok(Self {
            limits,
            external,
            expected_external: BTreeMap::new(),
        })
    }

    /// Construct a service whose explicitly supplied external commands must
    /// match the identities qualified during planning. Identity discovery is
    /// performed here, before any codec request or source binding is consumed.
    pub fn new_qualified(
        limits: FrameCodecLimits,
        external: ExternalFrameCodecCommands,
        expected_external: BTreeMap<String, QualifiedExecutableIdentity>,
    ) -> Result<Self, ServiceInvocationError> {
        let service = Self::new(limits, external)?;
        service.validate_external_identities(&expected_external)?;
        Ok(Self {
            expected_external,
            ..service
        })
    }

    /// Reconcile the command identities carried by this execution service
    /// with the exact inventory snapshot used by planning.
    pub fn validate_capability_inventory(
        &self,
        inventory: &CapabilityInventory,
    ) -> Result<(), ServiceInvocationError> {
        let configured = self.available_tools();
        for executable in &configured {
            if !inventory.available_executables.contains(executable) {
                return Err(service_error(format!(
                    "injected command {executable} was not available during planning"
                )));
            }
            let planned = inventory
                .executable_identities
                .get(executable)
                .ok_or_else(|| {
                    service_error(format!(
                        "injected command {executable} has no planning-qualified identity"
                    ))
                })?;
            let bound = self.expected_external.get(executable).ok_or_else(|| {
                service_error(format!(
                    "injected command {executable} has no execution-bound identity"
                ))
            })?;
            if planned != bound {
                return Err(service_error(format!(
                    "injected command {executable} identity differs from planning inventory"
                )));
            }
        }
        self.validate_external_identities(&self.expected_external)
    }

    pub fn encode(
        &self,
        request: &CodecRequest,
        cancellation: &CancellationToken,
        resolve: impl Fn(&ByteBinding) -> Result<Vec<u8>, ServiceInvocationError>,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        // Recheck immediately before resolving any source bytes so a replaced
        // executable cannot bypass the identity qualified at construction.
        self.validate_external_identities(&self.expected_external)?;
        if request.source_transfer_syntax_uid != EXPLICIT_VR_LITTLE_ENDIAN {
            return Err(service_error(format!(
                "encoded-frame service requires native Explicit VR Little Endian input, got {}",
                request.source_transfer_syntax_uid
            )));
        }
        if request.frames.is_empty() || request.frames.len() > self.limits.max_frames {
            return Err(service_error(format!(
                "frame count {} exceeds bounded range 1..={}",
                request.frames.len(),
                self.limits.max_frames
            )));
        }
        let registry = TransferSyntaxBackendRegistry::load_committed()
            .map_err(|error| service_error(error.to_string()))?;
        let descriptor = registry
            .for_transfer_syntax(&request.target_transfer_syntax_uid)
            .ok_or_else(|| service_error("target transfer syntax has no executable backend"))?;
        if descriptor.boundary == BackendBoundary::LockedFullFileTransform {
            return Err(service_error(format!(
                "backend {} is the locked full-file boundary and cannot run as an encoded-frame service",
                descriptor.backend_id
            )));
        }
        if descriptor.boundary != BackendBoundary::EncodedFrames {
            return Err(service_error(format!(
                "backend {} is not an encoded-frame backend",
                descriptor.backend_id
            )));
        }

        let enabled_features = enabled_codec_features();
        let available_tools = self.available_tools();
        for frame in &request.frames {
            registry
                .resolve(CodecDispatchRequest {
                    transfer_syntax_uid: &request.target_transfer_syntax_uid,
                    backend_id: &request.backend_id,
                    enabled_features: &enabled_features,
                    available_tools: &available_tools,
                    source: CodecSourceRequest::NativeFrame {
                        samples_per_pixel: frame.samples_per_pixel,
                        bits_allocated: frame.bits_allocated,
                        photometric_interpretation: &frame.photometric_interpretation,
                    },
                })
                .map_err(|error| service_error(error.to_string()))?;
        }

        match request.backend_id.as_str() {
            NativeRleLosslessEncoder::BACKEND_ID => self.encode_with(
                request,
                cancellation,
                resolve,
                NativeRleLosslessEncoder::new(),
                None,
            ),
            #[cfg(feature = "deflate")]
            DicomRsDeflatedImageFrameEncoder::BACKEND_ID => self.encode_with(
                request,
                cancellation,
                resolve,
                DicomRsDeflatedImageFrameEncoder::new(),
                None,
            ),
            #[cfg(feature = "charls")]
            DicomRsJpegLsLosslessEncoder::BACKEND_ID => self.encode_with(
                request,
                cancellation,
                resolve,
                DicomRsJpegLsLosslessEncoder::new(),
                None,
            ),
            #[cfg(feature = "jpeg")]
            DicomRsJpegBaselineEncoder::BACKEND_ID => self.encode_with(
                request,
                cancellation,
                resolve,
                DicomRsJpegBaselineEncoder::new(),
                None,
            ),
            #[cfg(feature = "jpeg2000")]
            OpenJp2Jpeg2000LosslessEncoder::BACKEND_ID => self.encode_with(
                request,
                cancellation,
                resolve,
                OpenJp2Jpeg2000LosslessEncoder::new(),
                None,
            ),
            #[cfg(feature = "jpegxl")]
            DicomRsJpegXlLosslessEncoder::BACKEND_ID => self.encode_with(
                request,
                cancellation,
                resolve,
                DicomRsJpegXlLosslessEncoder::new(),
                None,
            ),
            #[cfg(feature = "htj2k_openjph")]
            OpenJphHtj2kLosslessEncoder::BACKEND_ID => {
                let command = self.external.openjph.as_ref().ok_or_else(|| {
                    service_error("OpenJPH command path was not explicitly configured")
                })?;
                let encoder = OpenJphHtj2kLosslessEncoder::with_command(command);
                let identity = external_identity(&encoder, FrameEncoder::backend(&encoder))?;
                self.encode_with(request, cancellation, resolve, encoder, Some(identity))
            }
            #[cfg(feature = "htj2k_openjph")]
            OpenJphHtj2kLossyEncoder::BACKEND_ID => {
                let command = self.external.openjph.as_ref().ok_or_else(|| {
                    service_error("OpenJPH command path was not explicitly configured")
                })?;
                let encoder = OpenJphHtj2kLossyEncoder::with_command(command);
                let identity = external_identity(&encoder, FrameEncoder::backend(&encoder))?;
                self.encode_with(request, cancellation, resolve, encoder, Some(identity))
            }
            #[cfg(feature = "jpegxl")]
            CjxlJpegXlLossyEncoder::BACKEND_ID => {
                let command = self.external.cjxl.as_ref().ok_or_else(|| {
                    service_error("cjxl command path was not explicitly configured")
                })?;
                let encoder = CjxlJpegXlLossyEncoder::with_command(command);
                let identity = external_identity(&encoder, FrameEncoder::backend(&encoder))?;
                self.encode_with(request, cancellation, resolve, encoder, Some(identity))
            }
            _ => Err(service_error(format!(
                "codec backend {} is unavailable in this build",
                request.backend_id
            ))),
        }
    }

    fn available_tools(&self) -> BTreeSet<String> {
        let mut tools = BTreeSet::new();
        if self.external.openjph.is_some() {
            tools.insert("ojph_compress".into());
        }
        if self.external.cjxl.is_some() {
            tools.insert("cjxl".into());
        }
        tools
    }

    fn validate_external_identities(
        &self,
        expected: &BTreeMap<String, QualifiedExecutableIdentity>,
    ) -> Result<(), ServiceInvocationError> {
        #[cfg(not(feature = "htj2k_openjph"))]
        if self.external.openjph.is_some() {
            return Err(service_error(
                "OpenJPH command was injected but feature htj2k_openjph is disabled",
            ));
        }
        #[cfg(feature = "htj2k_openjph")]
        if let Some(command) = self.external.openjph.as_ref() {
            let encoder = OpenJphHtj2kLosslessEncoder::with_command(command);
            validate_qualified_identity(
                "ojph_compress",
                encoder
                    .discover_backend_identity()
                    .map_err(|error| service_error(error.to_string()))?,
                expected,
            )?;
        }
        #[cfg(not(feature = "jpegxl"))]
        if self.external.cjxl.is_some() {
            return Err(service_error(
                "cjxl command was injected but feature jpegxl is disabled",
            ));
        }
        #[cfg(feature = "jpegxl")]
        if let Some(command) = self.external.cjxl.as_ref() {
            let encoder = CjxlJpegXlLossyEncoder::with_command(command);
            validate_qualified_identity(
                "cjxl",
                encoder
                    .discover_backend_identity()
                    .map_err(|error| service_error(error.to_string()))?,
                expected,
            )?;
        }
        for executable in expected.keys() {
            if !self.available_tools().contains(executable) {
                return Err(service_error(format!(
                    "qualified executable {executable} has no injected command path"
                )));
            }
        }
        Ok(())
    }

    fn encode_with<E: FrameEncoder + FrameDecoder>(
        &self,
        request: &CodecRequest,
        cancellation: &CancellationToken,
        resolve: impl Fn(&ByteBinding) -> Result<Vec<u8>, ServiceInvocationError>,
        encoder: E,
        external_identity: Option<ToolIdentity>,
    ) -> Result<CodecServiceOutcome, ServiceInvocationError> {
        let backend = FrameEncoder::backend(&encoder);
        let descriptor = TransferSyntaxBackendRegistry::load_committed()
            .map_err(|error| service_error(error.to_string()))?
            .for_transfer_syntax(&request.target_transfer_syntax_uid)
            .ok_or_else(|| service_error("target transfer syntax has no executable backend"))?;
        let lossless = descriptor
            .evidence
            .contains(&CodecEvidenceRequirement::ExactDecodedFrameHashes);
        let bits_stored = request
            .parameters
            .get("bits_stored")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok());
        let mut frames = request.frames.iter().collect::<Vec<_>>();
        frames.sort_by_key(|frame| frame.frame_number);
        let mut results = Vec::with_capacity(frames.len());
        let mut decoded_frame_sha256 = BTreeMap::new();
        let mut metrics = BTreeMap::new();
        let mut total_encoded = 0usize;
        for (index, frame) in frames.into_iter().enumerate() {
            if cancellation.is_cancelled() {
                return Err(service_error("execution cancelled"));
            }
            if frame.frame_number != u32::try_from(index + 1).unwrap_or(u32::MAX) {
                return Err(service_error(
                    "native frame numbers must be contiguous from one",
                ));
            }
            let native = resolve(&frame.bytes)?;
            if native.len() > self.limits.max_native_frame_bytes {
                return Err(service_error(format!(
                    "frame {} native bytes exceed limit {}",
                    frame.frame_number, self.limits.max_native_frame_bytes
                )));
            }
            let rows = u16::try_from(frame.rows)
                .map_err(|error| service_error(format!("invalid rows: {error}")))?;
            let columns = u16::try_from(frame.columns)
                .map_err(|error| service_error(format!("invalid columns: {error}")))?;
            let stored = bits_stored.unwrap_or(frame.bits_allocated);
            let encoded = encoder
                .encode_frame(frame_input(frame, &native, rows, columns, stored))
                .map_err(|error| service_error(error.to_string()))?;
            check_encoded_bounds(
                frame.frame_number,
                &encoded,
                self.limits.max_encoded_frame_bytes,
                &mut total_encoded,
                self.limits.max_total_encoded_bytes,
            )?;
            let decoded = encoder
                .decode_frame(FrameDecodeInput {
                    encoded_frame: &encoded.bytes,
                    rows,
                    columns,
                    samples_per_pixel: frame.samples_per_pixel,
                    bits_allocated: frame.bits_allocated,
                    bits_stored: stored,
                    photometric_interpretation: &frame.photometric_interpretation,
                })
                .map_err(|error| service_error(error.to_string()))?;
            if lossless && decoded.native_bytes != native {
                return Err(service_error(format!(
                    "frame {} lossless semantic round trip changed",
                    frame.frame_number
                )));
            }
            decoded_frame_sha256.insert(frame.frame_number, sha256_hex(&decoded.native_bytes));
            if !lossless {
                let observed = calculate_lossy_frame_metrics(
                    &native,
                    &decoded.native_bytes,
                    rows,
                    columns,
                    frame.samples_per_pixel,
                    frame.bits_allocated,
                )
                .map_err(|error| service_error(error.to_string()))?;
                metrics.insert(
                    format!("frame_{}_overall_rmse", frame.frame_number),
                    observed.overall_rmse,
                );
                for channel in observed.channels {
                    metrics.insert(
                        format!(
                            "frame_{}_channel_{}_max_absolute_error",
                            frame.frame_number, channel.channel_index
                        ),
                        channel.max_absolute_error as f64,
                    );
                }
            }
            let encoded_sha256 = sha256_hex(&encoded.bytes);
            results.push(EncodedFrameResult {
                frame_number: frame.frame_number,
                encoded_size_bytes: u64::try_from(encoded.bytes.len())
                    .map_err(|error| service_error(error.to_string()))?,
                encoded_sha256: encoded_sha256.clone(),
                bytes: ByteBinding::Inline {
                    bytes: encoded.bytes,
                    sha256: encoded_sha256,
                },
            });
        }
        let identity = external_identity.unwrap_or_else(|| ToolIdentity {
            backend_id: backend.backend_id.into(),
            version: backend.version.into(),
            protocol_version: None,
            executable_sha256: None,
        });
        let claims = BTreeMap::from([
            ("bounded_output_bytes".into(), json!(total_encoded)),
            ("frame_count".into(), json!(results.len())),
            ("source_boundary".into(), json!("verified_native_frames")),
        ]);
        Ok(CodecServiceOutcome {
            result: CodecResult {
                request_id: request.request_id.clone(),
                backend: identity.clone(),
                frames: results,
                evidence: vec![ServiceEvidence {
                    evidence_id: "codec_runtime_identity".into(),
                    evidence_kind: "codec_execution".into(),
                    producer: identity,
                    claims: claims.clone(),
                }],
            },
            backend_kind: backend.backend_kind.as_str().into(),
            display_name: backend.display_name.into(),
            feature_gate: backend.feature_gate.map(str::to_owned),
            determinism: match descriptor.determinism {
                BackendDeterminism::ByteStable => "byte_stable",
                BackendDeterminism::SemanticStable => "semantic_stable",
            }
            .into(),
            decoded_frame_sha256,
            metrics,
            claims,
        })
    }
}

fn frame_input<'a>(
    frame: &'a crate::executor::services::NativeFrameBinding,
    native: &'a [u8],
    rows: u16,
    columns: u16,
    bits_stored: u16,
) -> FrameEncodeInput<'a> {
    FrameEncodeInput {
        native_frame: native,
        rows,
        columns,
        samples_per_pixel: frame.samples_per_pixel,
        bits_allocated: frame.bits_allocated,
        bits_stored,
        photometric_interpretation: &frame.photometric_interpretation,
    }
}

fn check_encoded_bounds(
    frame_number: u32,
    encoded: &EncodedFrame,
    max_frame: usize,
    total: &mut usize,
    max_total: usize,
) -> Result<(), ServiceInvocationError> {
    if encoded.bytes.len() > max_frame {
        return Err(service_error(format!(
            "frame {frame_number} encoded bytes {} exceed limit {max_frame}",
            encoded.bytes.len()
        )));
    }
    *total = total
        .checked_add(encoded.bytes.len())
        .ok_or_else(|| service_error("total encoded byte count overflowed"))?;
    if *total > max_total {
        return Err(service_error(format!(
            "total encoded bytes {total} exceed limit {max_total}"
        )));
    }
    Ok(())
}

fn enabled_codec_features() -> BTreeSet<String> {
    const ACTIVE: &[&str] = &[
        #[cfg(feature = "deflate")]
        "deflate",
        #[cfg(feature = "charls")]
        "charls",
        #[cfg(feature = "jpeg")]
        "jpeg",
        #[cfg(feature = "jpegxl")]
        "jpegxl",
        #[cfg(feature = "jpeg2000")]
        "jpeg2000",
        #[cfg(feature = "htj2k_openjph")]
        "htj2k_openjph",
    ];
    ACTIVE.iter().map(|feature| (*feature).to_owned()).collect()
}

#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
trait ExternalIdentity {
    fn discover(
        &self,
    ) -> Result<crate::codecs::ExternalCommandBackendIdentity, crate::codecs::CodecError>;
}

#[cfg(feature = "htj2k_openjph")]
impl ExternalIdentity for OpenJphHtj2kLosslessEncoder {
    fn discover(
        &self,
    ) -> Result<crate::codecs::ExternalCommandBackendIdentity, crate::codecs::CodecError> {
        self.discover_backend_identity()
    }
}

#[cfg(feature = "htj2k_openjph")]
impl ExternalIdentity for OpenJphHtj2kLossyEncoder {
    fn discover(
        &self,
    ) -> Result<crate::codecs::ExternalCommandBackendIdentity, crate::codecs::CodecError> {
        self.discover_backend_identity()
    }
}

#[cfg(feature = "jpegxl")]
impl ExternalIdentity for CjxlJpegXlLossyEncoder {
    fn discover(
        &self,
    ) -> Result<crate::codecs::ExternalCommandBackendIdentity, crate::codecs::CodecError> {
        self.discover_backend_identity()
    }
}

#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
fn external_identity<E: ExternalIdentity>(
    encoder: &E,
    backend: CodecBackendInfo,
) -> Result<ToolIdentity, ServiceInvocationError> {
    let identity = encoder
        .discover()
        .map_err(|error| service_error(error.to_string()))?;
    let version = qualified_external_version(&identity)?;
    Ok(ToolIdentity {
        backend_id: backend.backend_id.into(),
        version,
        protocol_version: Some(identity.version_source.into()),
        executable_sha256: Some(identity.executable_sha256),
    })
}

#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
fn qualified_external_version(
    identity: &crate::codecs::ExternalCommandBackendIdentity,
) -> Result<String, ServiceInvocationError> {
    if let Some(version) = identity.version.as_ref() {
        return Ok(version.clone());
    }
    if identity.version_source == "executable_sha256" {
        return Ok(format!("sha256:{}", identity.executable_sha256));
    }
    Err(service_error(format!(
        "injected command {} did not report a version",
        identity.command
    )))
}

#[cfg(any(feature = "htj2k_openjph", feature = "jpegxl"))]
fn validate_qualified_identity(
    executable_id: &str,
    actual: crate::codecs::ExternalCommandBackendIdentity,
    expected: &BTreeMap<String, QualifiedExecutableIdentity>,
) -> Result<(), ServiceInvocationError> {
    let expected = expected.get(executable_id).ok_or_else(|| {
        service_error(format!(
            "injected command {executable_id} has no planning-qualified identity"
        ))
    })?;
    let actual_version = qualified_external_version(&actual)?;
    if actual.executable_sha256 != expected.executable_sha256 || actual_version != expected.version
    {
        return Err(service_error(format!(
            "injected command {executable_id} identity differs from planning inventory"
        )));
    }
    Ok(())
}

fn service_error(message: impl Into<String>) -> ServiceInvocationError {
    ServiceInvocationError::new("frame codec", message)
}
