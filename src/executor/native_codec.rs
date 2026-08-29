//! Frontend-neutral built-in codec execution.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::codecs::{
    FrameDecodeInput, FrameDecoder, FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder,
};
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::{CodecServiceOutcome, ServiceInvocationError};
use crate::executor::services::{
    ByteBinding, CodecRequest, CodecResult, EncodedFrameResult, ToolIdentity,
};
use crate::sha256_hex;

/// Execute the in-project RLE backend from verified native frame bindings.
///
/// The caller supplies byte resolution because inline, staged, and ranged
/// assets are transaction concerns. Encoding, semantic round-trip checks, and
/// evidence are shared by every frontend.
pub fn execute_native_rle(
    request: &CodecRequest,
    cancellation: &CancellationToken,
    resolve: impl Fn(&ByteBinding) -> Result<Vec<u8>, ServiceInvocationError>,
) -> Result<CodecServiceOutcome, ServiceInvocationError> {
    if request.backend_id != NativeRleLosslessEncoder::BACKEND_ID {
        return Err(ServiceInvocationError::new(
            "codec",
            format!("unsupported built-in codec {}", request.backend_id),
        ));
    }
    let encoder = NativeRleLosslessEncoder::new();
    let backend = FrameEncoder::backend(&encoder);
    let bits_stored = request
        .parameters
        .get("bits_stored")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok());
    let mut frames = request.frames.iter().collect::<Vec<_>>();
    frames.sort_by_key(|frame| frame.frame_number);
    let mut encoded = Vec::with_capacity(frames.len());
    let mut decoded_frame_sha256 = BTreeMap::new();
    let mut native_content = Vec::new();
    for (index, frame) in frames.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            return Err(ServiceInvocationError::new("codec", "execution cancelled"));
        }
        if frame.frame_number != index as u32 + 1 {
            return Err(ServiceInvocationError::new(
                "codec",
                "native frame numbers must be contiguous from one",
            ));
        }
        let native = resolve(&frame.bytes)?;
        native_content.extend_from_slice(&native);
        let stored = bits_stored.unwrap_or(frame.bits_allocated);
        let rows = u16::try_from(frame.rows)
            .map_err(|error| ServiceInvocationError::new("codec", error.to_string()))?;
        let columns = u16::try_from(frame.columns)
            .map_err(|error| ServiceInvocationError::new("codec", error.to_string()))?;
        let result = encoder
            .encode_frame(FrameEncodeInput {
                native_frame: &native,
                rows,
                columns,
                samples_per_pixel: frame.samples_per_pixel,
                bits_allocated: frame.bits_allocated,
                bits_stored: stored,
                photometric_interpretation: &frame.photometric_interpretation,
            })
            .map_err(|error| ServiceInvocationError::new("codec", error.to_string()))?;
        let decoded = encoder
            .decode_frame(FrameDecodeInput {
                encoded_frame: &result.bytes,
                rows,
                columns,
                samples_per_pixel: frame.samples_per_pixel,
                bits_allocated: frame.bits_allocated,
                bits_stored: stored,
                photometric_interpretation: &frame.photometric_interpretation,
            })
            .map_err(|error| ServiceInvocationError::new("codec", error.to_string()))?;
        if decoded.native_bytes != native {
            return Err(ServiceInvocationError::new(
                "codec",
                format!("frame {} semantic round trip changed", frame.frame_number),
            ));
        }
        decoded_frame_sha256.insert(frame.frame_number, sha256_hex(&decoded.native_bytes));
        let encoded_sha256 = sha256_hex(&result.bytes);
        encoded.push(EncodedFrameResult {
            frame_number: frame.frame_number,
            encoded_size_bytes: result.bytes.len() as u64,
            encoded_sha256: encoded_sha256.clone(),
            bytes: ByteBinding::Inline {
                bytes: result.bytes,
                sha256: encoded_sha256,
            },
        });
    }
    Ok(CodecServiceOutcome {
        result: CodecResult {
            request_id: request.request_id.clone(),
            backend: ToolIdentity {
                backend_id: backend.backend_id.into(),
                version: backend.version.into(),
                protocol_version: None,
                executable_sha256: None,
            },
            frames: encoded,
            evidence: vec![],
        },
        backend_kind: backend.backend_kind.as_str().into(),
        display_name: backend.display_name.into(),
        feature_gate: backend.feature_gate.map(str::to_owned),
        determinism: "byte_stable".into(),
        decoded_frame_sha256,
        metrics: BTreeMap::new(),
        claims: BTreeMap::from([
            ("native_sha256".into(), json!(sha256_hex(&native_content))),
            (
                "codec_backend_kind".into(),
                json!(backend.backend_kind.as_str()),
            ),
            (
                "codec_feature_gate".into(),
                json!(backend.feature_gate.unwrap_or("none")),
            ),
            ("codec_availability".into(), json!("available")),
        ]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID;
    use crate::executor::services::NativeFrameBinding;

    #[test]
    fn native_rle_reports_typed_backend_identity() {
        let native = vec![1, 2];
        let request = CodecRequest {
            request_id: "codec:test:pixels".into(),
            artifact_id: "test".into(),
            slot: "pixels".into(),
            backend_id: NativeRleLosslessEncoder::BACKEND_ID.into(),
            source_transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            target_transfer_syntax_uid: RLE_LOSSLESS_TRANSFER_SYNTAX_UID.into(),
            frames: vec![NativeFrameBinding {
                frame_number: 1,
                bytes: ByteBinding::Inline {
                    sha256: sha256_hex(&native),
                    bytes: native,
                },
                rows: 1,
                columns: 2,
                samples_per_pixel: 1,
                bits_allocated: 8,
                photometric_interpretation: "MONOCHROME2".into(),
            }],
            parameters: BTreeMap::from([("bits_stored".into(), Value::from(8))]),
        };
        let outcome = execute_native_rle(&request, &CancellationToken::new(), |binding| {
            let ByteBinding::Inline { bytes, .. } = binding else {
                unreachable!()
            };
            Ok(bytes.clone())
        })
        .unwrap();
        assert_eq!(outcome.backend_kind, "native");
        assert_eq!(outcome.display_name, "Native project RLE Lossless encoder");
        assert_eq!(outcome.feature_gate, None);
    }
}
