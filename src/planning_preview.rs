//! Bounded, filesystem-free DICOM planning preview.
//!
//! This is the single audited planning exception for deterministic byte-range
//! consumers. It accepts only inline native bytes and the built-in native RLE
//! encoder; staged assets, providers, pipelines, external codecs, and tools
//! remain execution-only boundaries.

use std::error::Error;
use std::fmt;

use crate::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use crate::composition::{ContentMaterialization, Part10Materializer};
use crate::corpus_plan::PlannedDicomArtifact;
use crate::encoded_content::{EncodedSlotInput, resolve_encoded_content};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, NativeFrameBinding, SlotExecutionBinding,
};
use crate::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanningPreviewLimits {
    pub max_frames: usize,
    pub max_native_frame_bytes: usize,
    pub max_encoded_frame_bytes: usize,
    pub max_total_encoded_bytes: usize,
    pub max_output_bytes: u64,
}

impl Default for PlanningPreviewLimits {
    fn default() -> Self {
        Self {
            max_frames: 4_096,
            max_native_frame_bytes: 256 * 1024 * 1024,
            max_encoded_frame_bytes: 256 * 1024 * 1024,
            max_total_encoded_bytes: 512 * 1024 * 1024,
            max_output_bytes: 512 * 1024 * 1024,
        }
    }
}

impl PlanningPreviewLimits {
    fn validate(self) -> Result<Self, PlanningPreviewError> {
        if self.max_frames == 0
            || self.max_native_frame_bytes == 0
            || self.max_encoded_frame_bytes == 0
            || self.max_total_encoded_bytes == 0
            || self.max_output_bytes == 0
        {
            return Err(PlanningPreviewError::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningPreview {
    pub bytes: Vec<u8>,
    pub size_bytes: u64,
    pub sha256: String,
}

pub fn preview_planned_dicom(
    artifact: &PlannedDicomArtifact,
    bindings: &ArtifactExecutionBindings,
    limits: PlanningPreviewLimits,
    cancelled: &dyn Fn() -> bool,
) -> Result<PlanningPreview, PlanningPreviewError> {
    let limits = limits.validate()?;
    if bindings.artifact_id != artifact.logical_id {
        return Err(PlanningPreviewError::BindingIdentity);
    }
    if cancelled() {
        return Err(PlanningPreviewError::Cancelled);
    }
    let mut instance = artifact.instance.clone();
    let mut encoded_inputs = Vec::new();
    for (slot, binding) in &bindings.slots {
        if cancelled() {
            return Err(PlanningPreviewError::Cancelled);
        }
        match binding {
            SlotExecutionBinding::NativeFrames { frames } => {
                let bytes = resolve_native_frames(frames, limits, cancelled)?;
                let content = instance
                    .content
                    .iter_mut()
                    .find(|content| content.slot == *slot)
                    .ok_or_else(|| PlanningPreviewError::MissingSlot(slot.clone()))?;
                if bytes.len() as u64 != content.size_bytes || sha256_hex(&bytes) != content.sha256
                {
                    return Err(PlanningPreviewError::CanonicalContent(slot.clone()));
                }
                content.materialization = Some(ContentMaterialization::Inline(bytes));
            }
            SlotExecutionBinding::CodecRequest { request } => {
                if request.artifact_id != artifact.logical_id || request.slot != *slot {
                    return Err(PlanningPreviewError::BindingIdentity);
                }
                encoded_inputs.push(EncodedSlotInput {
                    slot: slot.clone(),
                    ordered_frames: encode_native_rle(request, limits, cancelled)?,
                });
            }
            SlotExecutionBinding::StagedAsset { .. }
            | SlotExecutionBinding::ProviderRequest { .. }
            | SlotExecutionBinding::ProviderCodecPipeline { .. }
            | SlotExecutionBinding::EncodedFrames { .. } => {
                return Err(PlanningPreviewError::ExecutionOnlyBinding(slot.clone()));
            }
        }
    }
    if !encoded_inputs.is_empty() {
        instance = resolve_encoded_content(&instance, &artifact.encoding, &encoded_inputs)
            .map_err(PlanningPreviewError::EncodedContent)?
            .instance;
    }
    if cancelled() {
        return Err(PlanningPreviewError::Cancelled);
    }
    let bytes = Part10Materializer
        .preview_part10_bytes_with_encoding(&instance, &artifact.encoding, limits.max_output_bytes)
        .map_err(|error| PlanningPreviewError::Materialize(error.to_string()))?;
    if cancelled() {
        return Err(PlanningPreviewError::Cancelled);
    }
    let size_bytes = u64::try_from(bytes.len()).map_err(|_| PlanningPreviewError::ResourceLimit)?;
    Ok(PlanningPreview {
        sha256: sha256_hex(&bytes),
        size_bytes,
        bytes,
    })
}

fn resolve_native_frames(
    frames: &[NativeFrameBinding],
    limits: PlanningPreviewLimits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, PlanningPreviewError> {
    let ordered = ordered_frames(frames, limits)?;
    let mut bytes = Vec::new();
    for frame in ordered {
        if cancelled() {
            return Err(PlanningPreviewError::Cancelled);
        }
        let payload = inline_bytes(&frame.bytes)?;
        if payload.len() > limits.max_native_frame_bytes {
            return Err(PlanningPreviewError::ResourceLimit);
        }
        bytes
            .try_reserve(payload.len())
            .map_err(|_| PlanningPreviewError::ResourceLimit)?;
        bytes.extend_from_slice(payload);
    }
    Ok(bytes)
}

fn encode_native_rle(
    request: &crate::executor::services::CodecRequest,
    limits: PlanningPreviewLimits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, PlanningPreviewError> {
    if request.backend_id != NativeRleLosslessEncoder::BACKEND_ID
        || request.source_transfer_syntax_uid != "1.2.840.10008.1.2.1"
        || request.target_transfer_syntax_uid != crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
    {
        return Err(PlanningPreviewError::UnsupportedCodec(
            request.backend_id.clone(),
        ));
    }
    let bits_stored = request
        .parameters
        .get("bits_stored")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok());
    let ordered = ordered_frames(&request.frames, limits)?;
    let mut result = Vec::with_capacity(ordered.len());
    let mut total = 0usize;
    for frame in ordered {
        if cancelled() {
            return Err(PlanningPreviewError::Cancelled);
        }
        let native = inline_bytes(&frame.bytes)?;
        if native.len() > limits.max_native_frame_bytes {
            return Err(PlanningPreviewError::ResourceLimit);
        }
        let rows = u16::try_from(frame.rows).map_err(|_| PlanningPreviewError::FrameShape)?;
        let columns = u16::try_from(frame.columns).map_err(|_| PlanningPreviewError::FrameShape)?;
        let encoded = NativeRleLosslessEncoder::new()
            .encode_frame(FrameEncodeInput {
                native_frame: native,
                rows,
                columns,
                samples_per_pixel: frame.samples_per_pixel,
                bits_allocated: frame.bits_allocated,
                bits_stored: bits_stored.unwrap_or(frame.bits_allocated),
                photometric_interpretation: &frame.photometric_interpretation,
            })
            .map_err(|error| PlanningPreviewError::Codec(error.to_string()))?
            .bytes;
        total = total
            .checked_add(encoded.len())
            .ok_or(PlanningPreviewError::ResourceLimit)?;
        if encoded.len() > limits.max_encoded_frame_bytes || total > limits.max_total_encoded_bytes
        {
            return Err(PlanningPreviewError::ResourceLimit);
        }
        result.push(encoded);
    }
    Ok(result)
}

fn ordered_frames(
    frames: &[NativeFrameBinding],
    limits: PlanningPreviewLimits,
) -> Result<Vec<&NativeFrameBinding>, PlanningPreviewError> {
    if frames.is_empty() || frames.len() > limits.max_frames {
        return Err(PlanningPreviewError::ResourceLimit);
    }
    let mut ordered = frames.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|frame| frame.frame_number);
    for (index, frame) in ordered.iter().enumerate() {
        if frame.frame_number != u32::try_from(index + 1).unwrap_or(u32::MAX) {
            return Err(PlanningPreviewError::FrameOrder);
        }
    }
    Ok(ordered)
}

fn inline_bytes(binding: &ByteBinding) -> Result<&[u8], PlanningPreviewError> {
    let ByteBinding::Inline { bytes, sha256 } = binding else {
        return Err(PlanningPreviewError::ExecutionOnlyBytes);
    };
    if sha256_hex(bytes) != *sha256 {
        return Err(PlanningPreviewError::ByteIdentity);
    }
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanningPreviewError {
    InvalidLimits,
    Cancelled,
    BindingIdentity,
    MissingSlot(String),
    CanonicalContent(String),
    ExecutionOnlyBinding(String),
    ExecutionOnlyBytes,
    UnsupportedCodec(String),
    FrameOrder,
    FrameShape,
    ByteIdentity,
    ResourceLimit,
    Codec(String),
    EncodedContent(String),
    Materialize(String),
}

impl fmt::Display for PlanningPreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("planning preview limits must be nonzero"),
            Self::Cancelled => formatter.write_str("planning preview was cancelled"),
            Self::BindingIdentity => {
                formatter.write_str("planning preview binding identity differs from the artifact")
            }
            Self::MissingSlot(slot) => write!(formatter, "planning preview slot {slot} is absent"),
            Self::CanonicalContent(slot) => write!(
                formatter,
                "planning preview slot {slot} differs from canonical content"
            ),
            Self::ExecutionOnlyBinding(slot) => write!(
                formatter,
                "planning preview slot {slot} requires execution-only input"
            ),
            Self::ExecutionOnlyBytes => {
                formatter.write_str("planning preview rejects staged or ranged bytes")
            }
            Self::UnsupportedCodec(codec) => {
                write!(formatter, "planning preview rejects codec {codec}")
            }
            Self::FrameOrder => {
                formatter.write_str("planning preview frames must be contiguous from one")
            }
            Self::FrameShape => {
                formatter.write_str("planning preview frame shape exceeds the bounded encoder")
            }
            Self::ByteIdentity => formatter
                .write_str("planning preview inline byte hash differs from its declaration"),
            Self::ResourceLimit => formatter.write_str("planning preview resource limit exceeded"),
            Self::Codec(message) | Self::EncodedContent(message) | Self::Materialize(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl Error for PlanningPreviewError {}
