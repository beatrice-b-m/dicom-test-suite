//! Bounded streaming content service for reduced-scale stress recipes.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::executor::cancellation::CancellationToken;
use crate::executor::services::{
    AssetDeclaration, AssetVisibility, ProducedAsset, ProviderRequest, ProviderResult,
    ServiceEvidence, StagedAssetHandle, StagingRelativePath, ToolIdentity,
};
use crate::sha256_hex;

pub const STRESS_CONTENT_PROVIDER_ID: &str = "native.stress_content";
pub const STRESS_CONTENT_PROVIDER_VERSION: &str = "1";
pub const MAX_STRESS_CONTENT_BYTES: u64 = 64 * 1024 * 1024;
const CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StressPayloadRequest {
    RepeatedByte {
        byte: u8,
        length: u64,
    },
    Literal {
        bytes: Vec<u8>,
    },
    DeterministicU8Frames {
        rows: u32,
        columns: u32,
        frames: u32,
        algorithm: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressPayloadIdentity {
    pub size_bytes: u64,
    pub sha256: String,
    pub frame_sha256: Vec<String>,
    pub frame_ranges: Vec<(u64, u64)>,
}

pub fn stress_payload_identity(
    payload: &StressPayloadRequest,
) -> Result<StressPayloadIdentity, StressContentError> {
    let mut sink = HashSink::default();
    let (frames, ranges) = emit_payload(
        payload,
        &mut sink,
        &CancellationToken::new(),
        &mut |_, _| {},
    )?;
    Ok(StressPayloadIdentity {
        size_bytes: sink.length,
        sha256: sink.hasher.finish_hex(),
        frame_sha256: frames,
        frame_ranges: ranges,
    })
}

pub fn execute_stress_content(
    request: &ProviderRequest,
    staging_root: &Path,
    cancellation: &CancellationToken,
) -> Result<ProviderResult, StressContentError> {
    execute_stress_content_with_checkpoint(request, staging_root, cancellation, &mut |_, _| {})
}

fn execute_stress_content_with_checkpoint(
    request: &ProviderRequest,
    staging_root: &Path,
    cancellation: &CancellationToken,
    checkpoint: &mut dyn FnMut(u64, &CancellationToken),
) -> Result<ProviderResult, StressContentError> {
    if request.provider_id != STRESS_CONTENT_PROVIDER_ID
        || request.required_version != STRESS_CONTENT_PROVIDER_VERSION
        || request.expected_outputs.len() != 1
        || !request.input_assets.is_empty()
    {
        return Err(StressContentError::Contract(
            "stress provider request identity or output cardinality mismatch".into(),
        ));
    }
    let payload: StressPayloadRequest = serde_json::from_value(
        request
            .parameters
            .get("payload")
            .cloned()
            .ok_or_else(|| StressContentError::Contract("missing payload contract".into()))?,
    )
    .map_err(|error| StressContentError::Contract(error.to_string()))?;
    let expected = &request.expected_outputs[0];
    let declared = stress_payload_identity(&payload)?;
    if declared.size_bytes > expected.maximum_size_bytes
        || expected
            .expected_sha256
            .as_ref()
            .is_some_and(|digest| digest != &declared.sha256)
    {
        return Err(StressContentError::Contract(
            "stress output identity exceeds or differs from its declaration".into(),
        ));
    }
    if cancellation.is_cancelled() {
        return Err(StressContentError::Cancelled);
    }
    let directory = staging_root.join("stress-content");
    fs::create_dir_all(&directory).map_err(StressContentError::Io)?;
    let leaf = safe_leaf(&request.request_id);
    let relative_path = format!("stress-content/{leaf}.bin");
    let path = directory.join(format!("{leaf}.bin"));
    let mut cleanup = PartialOutput::new(path.clone());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(StressContentError::Io)?;
    let (frame_sha256, frame_ranges) = emit_payload(&payload, &mut file, cancellation, checkpoint)?;
    file.flush().map_err(StressContentError::Io)?;
    file.sync_all().map_err(StressContentError::Io)?;
    let observed = hash_file(&path, cancellation)?;
    if observed.size_bytes != declared.size_bytes || observed.sha256 != declared.sha256 {
        return Err(StressContentError::IdentityMismatch);
    }
    cleanup.keep = true;
    let slot = expected.slot.clone();
    let handle = StagedAssetHandle::new(format!(
        "stress_{}_{}",
        safe_leaf(&request.artifact_id),
        slot
    ))
    .map_err(|error| StressContentError::Contract(error.to_string()))?;
    let declaration = AssetDeclaration {
        handle,
        relative_path: StagingRelativePath::new(relative_path)
            .map_err(|error| StressContentError::Contract(error.to_string()))?,
        size_bytes: observed.size_bytes,
        sha256: observed.sha256.clone(),
        media_type: expected.media_type.clone(),
        visibility: AssetVisibility::Private,
    };
    Ok(ProviderResult {
        request_id: request.request_id.clone(),
        provider: ToolIdentity {
            backend_id: STRESS_CONTENT_PROVIDER_ID.into(),
            version: STRESS_CONTENT_PROVIDER_VERSION.into(),
            protocol_version: Some("1".into()),
            executable_sha256: None,
        },
        outputs: BTreeMap::from([(
            slot,
            ProducedAsset {
                declaration,
                observed_size_bytes: observed.size_bytes,
                observed_sha256: observed.sha256.clone(),
            },
        )]),
        evidence: vec![ServiceEvidence {
            evidence_id: format!("stress_content_{}", safe_leaf(&request.request_id)),
            evidence_kind: "bounded_stream_generation".into(),
            producer: ToolIdentity {
                backend_id: STRESS_CONTENT_PROVIDER_ID.into(),
                version: STRESS_CONTENT_PROVIDER_VERSION.into(),
                protocol_version: Some("1".into()),
                executable_sha256: None,
            },
            claims: BTreeMap::from([
                ("size_bytes".into(), Value::from(observed.size_bytes)),
                ("sha256".into(), Value::String(observed.sha256)),
                ("peak_buffer_bytes".into(), Value::from(CHUNK_BYTES as u64)),
                (
                    "frame_sha256".into(),
                    serde_json::to_value(frame_sha256).unwrap(),
                ),
                (
                    "frame_ranges".into(),
                    serde_json::to_value(frame_ranges).unwrap(),
                ),
            ]),
        }],
    })
}

fn emit_payload(
    payload: &StressPayloadRequest,
    writer: &mut dyn Write,
    cancellation: &CancellationToken,
    checkpoint: &mut dyn FnMut(u64, &CancellationToken),
) -> Result<(Vec<String>, Vec<(u64, u64)>), StressContentError> {
    match payload {
        StressPayloadRequest::RepeatedByte { byte, length } => {
            require_size(*length)?;
            let chunk = vec![*byte; CHUNK_BYTES];
            let mut remaining = *length;
            while remaining != 0 {
                if cancellation.is_cancelled() {
                    return Err(StressContentError::Cancelled);
                }
                let count = usize::try_from(remaining.min(CHUNK_BYTES as u64))
                    .map_err(|_| StressContentError::ResourceOverflow)?;
                writer
                    .write_all(&chunk[..count])
                    .map_err(StressContentError::Io)?;
                remaining -= count as u64;
                checkpoint(*length - remaining, cancellation);
            }
            Ok((Vec::new(), vec![(0, *length)]))
        }
        StressPayloadRequest::Literal { bytes } => {
            require_size(bytes.len() as u64)?;
            if cancellation.is_cancelled() {
                return Err(StressContentError::Cancelled);
            }
            writer.write_all(bytes).map_err(StressContentError::Io)?;
            checkpoint(bytes.len() as u64, cancellation);
            Ok((vec![sha256_hex(bytes)], vec![(0, bytes.len() as u64)]))
        }
        StressPayloadRequest::DeterministicU8Frames {
            rows,
            columns,
            frames,
            algorithm,
        } => {
            if algorithm != "index_mul_37_frame_mul_17_xor_index_shift_8" {
                return Err(StressContentError::Contract(
                    "unknown deterministic stress frame algorithm".into(),
                ));
            }
            let frame_bytes = u64::from(*rows)
                .checked_mul(u64::from(*columns))
                .ok_or(StressContentError::ResourceOverflow)?;
            let total = frame_bytes
                .checked_mul(u64::from(*frames))
                .ok_or(StressContentError::ResourceOverflow)?;
            require_size(total)?;
            let frame_len =
                usize::try_from(frame_bytes).map_err(|_| StressContentError::ResourceOverflow)?;
            let mut hashes = Vec::with_capacity(*frames as usize);
            let mut ranges = Vec::with_capacity(*frames as usize);
            for frame in 0..*frames {
                if cancellation.is_cancelled() {
                    return Err(StressContentError::Cancelled);
                }
                let bytes = (0..frame_len)
                    .map(|index| {
                        ((index.wrapping_mul(37) + frame as usize * 17) ^ (index >> 8)) as u8
                    })
                    .collect::<Vec<_>>();
                hashes.push(sha256_hex(&bytes));
                ranges.push((u64::from(frame) * frame_bytes, frame_bytes));
                writer.write_all(&bytes).map_err(StressContentError::Io)?;
                checkpoint((u64::from(frame) + 1) * frame_bytes, cancellation);
            }
            Ok((hashes, ranges))
        }
    }
}

fn require_size(size: u64) -> Result<(), StressContentError> {
    if size == 0 || size > MAX_STRESS_CONTENT_BYTES {
        Err(StressContentError::ResourceLimit(size))
    } else {
        Ok(())
    }
}

fn safe_leaf(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

struct PartialOutput {
    path: PathBuf,
    keep: bool,
}

impl PartialOutput {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }
}

impl Drop for PartialOutput {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[derive(Default)]
struct HashSink {
    hasher: Sha256,
    length: u64,
}

impl Write for HashSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.hasher.update(bytes);
        self.length = self.length.saturating_add(bytes.len() as u64);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FileIdentity {
    size_bytes: u64,
    sha256: String,
}

fn hash_file(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<FileIdentity, StressContentError> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(StressContentError::Io)?;
    let mut hasher = Sha256::default();
    let mut size = 0_u64;
    let mut chunk = vec![0_u8; CHUNK_BYTES];
    loop {
        if cancellation.is_cancelled() {
            return Err(StressContentError::Cancelled);
        }
        let count = file.read(&mut chunk).map_err(StressContentError::Io)?;
        if count == 0 {
            break;
        }
        hasher.update(&chunk[..count]);
        size = size
            .checked_add(count as u64)
            .ok_or(StressContentError::ResourceOverflow)?;
    }
    Ok(FileIdentity {
        size_bytes: size,
        sha256: hasher.finish_hex(),
    })
}

#[derive(Default)]
struct Sha256 {
    state: [u32; 8],
    buffer: Vec<u8>,
    length: u64,
    initialized: bool,
}

impl Sha256 {
    fn update(&mut self, bytes: &[u8]) {
        if !self.initialized {
            self.state = [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ];
            self.initialized = true;
        }
        self.length += bytes.len() as u64;
        self.buffer.extend_from_slice(bytes);
        while self.buffer.len() >= 64 {
            let block: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.compress(&block);
            self.buffer.drain(..64);
        }
    }

    fn finish_hex(mut self) -> String {
        if !self.initialized {
            self.update(&[]);
        }
        let bit_length = self.length * 8;
        self.buffer.push(0x80);
        while self.buffer.len() % 64 != 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&bit_length.to_be_bytes());
        while !self.buffer.is_empty() {
            let block: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.compress(&block);
            self.buffer.drain(..64);
        }
        self.state
            .iter()
            .map(|word| format!("{word:08x}"))
            .collect()
    }

    fn compress(&mut self, block: &[u8; 64]) {
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
        let mut words = [0_u32; 64];
        for (index, chunk) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes(chunk.try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(value);
        }
    }
}

#[derive(Debug)]
pub enum StressContentError {
    Contract(String),
    ResourceLimit(u64),
    ResourceOverflow,
    Cancelled,
    IdentityMismatch,
    Io(io::Error),
}

impl std::fmt::Display for StressContentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contract(message) => formatter.write_str(message),
            Self::ResourceLimit(size) => write!(
                formatter,
                "stress payload size {size} exceeds its bounded limit"
            ),
            Self::ResourceOverflow => {
                formatter.write_str("stress payload resource arithmetic overflow")
            }
            Self::Cancelled => formatter.write_str("stress payload generation cancelled"),
            Self::IdentityMismatch => {
                formatter.write_str("stress payload output identity mismatch")
            }
            Self::Io(error) => write!(formatter, "stress payload I/O failed: {error}"),
        }
    }
}

impl std::error::Error for StressContentError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::services::ProviderOutputExpectation;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temporary_staging_root() -> PathBuf {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dts-stress-content-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn streaming_sha256_matches_repository_digest_at_block_boundaries() {
        for length in [0, 1, 55, 56, 63, 64, 65, CHUNK_BYTES * 3 + 17] {
            let bytes = (0..length)
                .map(|index| (index.wrapping_mul(37) ^ (index >> 3)) as u8)
                .collect::<Vec<_>>();
            let mut sink = HashSink::default();
            for chunk in bytes.chunks(13) {
                sink.write_all(chunk).unwrap();
            }
            assert_eq!(sink.length, bytes.len() as u64);
            assert_eq!(sink.hasher.finish_hex(), sha256_hex(&bytes));
        }
    }

    #[test]
    fn identity_derivation_is_bounded_and_exact() {
        let payload = StressPayloadRequest::DeterministicU8Frames {
            rows: 64,
            columns: 64,
            frames: 3,
            algorithm: "index_mul_37_frame_mul_17_xor_index_shift_8".into(),
        };
        let identity = stress_payload_identity(&payload).unwrap();
        assert_eq!(identity.size_bytes, 3 * 64 * 64);
        assert_eq!(identity.frame_sha256.len(), 3);
        assert_eq!(
            identity.frame_ranges,
            [(0, 4096), (4096, 4096), (8192, 4096)]
        );
    }

    #[test]
    fn cancellation_after_first_chunk_removes_partial_private_asset() {
        let payload = StressPayloadRequest::RepeatedByte {
            byte: 0xa5,
            length: (CHUNK_BYTES * 3) as u64,
        };
        let identity = stress_payload_identity(&payload).unwrap();
        let request = ProviderRequest {
            request_id: "cancel_after_chunk".into(),
            artifact_id: "stress_artifact".into(),
            provider_id: STRESS_CONTENT_PROVIDER_ID.into(),
            required_version: STRESS_CONTENT_PROVIDER_VERSION.into(),
            parameters: BTreeMap::from([(
                "payload".into(),
                serde_json::to_value(payload).unwrap(),
            )]),
            input_assets: BTreeMap::new(),
            expected_outputs: vec![ProviderOutputExpectation {
                slot: "pixels".into(),
                media_type: "application/octet-stream".into(),
                maximum_size_bytes: identity.size_bytes,
                expected_sha256: Some(identity.sha256),
            }],
        };
        let staging_root = temporary_staging_root();
        let cancellation = CancellationToken::new();
        let mut checkpoints = 0;
        let result = execute_stress_content_with_checkpoint(
            &request,
            &staging_root,
            &cancellation,
            &mut |written, token| {
                checkpoints += 1;
                if written >= CHUNK_BYTES as u64 {
                    token.cancel_with_reason("test checkpoint");
                }
            },
        );
        assert!(matches!(result, Err(StressContentError::Cancelled)));
        assert_eq!(checkpoints, 1);
        assert!(
            fs::read_dir(staging_root.join("stress-content"))
                .unwrap()
                .next()
                .is_none(),
            "cancelled content left a partial private asset"
        );
        fs::remove_dir_all(staging_root).unwrap();
    }
}
