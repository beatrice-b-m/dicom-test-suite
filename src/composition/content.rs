use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use super::{
    AttributeAddress, CanonicalContent, ContentMaterialization, DicomVr, ResolvedProviderContent,
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAsset {
    pub slot: String,
    pub kind: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub spec_relative_path: String,
    pub staged_path: PathBuf,
    pub properties: BTreeMap<String, String>,
}

impl StagedAsset {
    pub fn into_canonical_content(
        self,
        address: AttributeAddress,
        vr: DicomVr,
    ) -> CanonicalContent {
        let mut properties = self.properties;
        properties.insert("spec_relative_path".to_string(), self.spec_relative_path);
        CanonicalContent {
            slot: self.slot,
            kind: self.kind,
            address,
            vr,
            size_bytes: self.size_bytes,
            sha256: self.sha256,
            properties,
            placement: super::ContentPlacement::TopLevel,
            materialization: Some(ContentMaterialization::StagedFile(self.staged_path)),
        }
    }
}

#[derive(Debug)]
pub struct LocalContentResolver {
    spec_root: PathBuf,
    staging_root: PathBuf,
    limits: ContentLimits,
    files: usize,
    total_bytes: u64,
}

impl LocalContentResolver {
    pub fn new(
        spec_root: impl Into<PathBuf>,
        staging_root: impl Into<PathBuf>,
        limits: ContentLimits,
    ) -> Result<Self, ContentError> {
        if limits.max_files == 0 || limits.max_file_bytes == 0 || limits.max_total_bytes == 0 {
            return Err(ContentError::InvalidLimits);
        }
        let spec_root = spec_root.into();
        let staging_root = staging_root.into();
        verify_plain_directory(&spec_root)?;
        verify_plain_directory(&staging_root)?;
        Ok(Self {
            spec_root,
            staging_root,
            limits,
            files: 0,
            total_bytes: 0,
        })
    }

    pub fn resolve(
        &mut self,
        slot: &str,
        kind: &str,
        relative_path: &Path,
        expected_sha256: Option<&str>,
    ) -> Result<StagedAsset, ContentError> {
        if slot.is_empty() || kind.is_empty() {
            return Err(ContentError::InvalidSlot);
        }
        let relative_text = safe_relative_text(relative_path)?;
        if self.files >= self.limits.max_files {
            return Err(ContentError::FileCountLimit {
                limit: self.limits.max_files,
            });
        }
        let source_path = verify_component_path(&self.spec_root, relative_path)?;
        self.stage_source(
            slot,
            kind,
            relative_text,
            source_path,
            expected_sha256,
            BTreeMap::new(),
        )
    }

    pub(crate) fn resolve_inline(
        &mut self,
        slot: &str,
        kind: &str,
        bytes: &[u8],
        expected_sha256: Option<&str>,
    ) -> Result<StagedAsset, ContentError> {
        if slot.is_empty() || kind.is_empty() {
            return Err(ContentError::InvalidSlot);
        }
        if self.files >= self.limits.max_files {
            return Err(ContentError::FileCountLimit {
                limit: self.limits.max_files,
            });
        }
        let size_bytes = u64::try_from(bytes.len()).map_err(|_| ContentError::TotalSizeOverflow)?;
        if size_bytes > self.limits.max_file_bytes {
            return Err(ContentError::FileSizeLimit {
                path: format!("inline/{slot}"),
                size: size_bytes,
                limit: self.limits.max_file_bytes,
            });
        }
        let projected_total = self
            .total_bytes
            .checked_add(size_bytes)
            .ok_or(ContentError::TotalSizeOverflow)?;
        if projected_total > self.limits.max_total_bytes {
            return Err(ContentError::TotalSizeLimit {
                size: projected_total,
                limit: self.limits.max_total_bytes,
            });
        }
        let sha256 = crate::sha256_hex(bytes);
        if let Some(expected) = expected_sha256 {
            if expected != sha256 {
                return Err(ContentError::HashMismatch {
                    path: format!("inline/{slot}"),
                    expected: expected.to_string(),
                    actual: sha256,
                });
            }
        }
        let staged_path = self
            .staging_root
            .join(format!("asset-{:08}.bin", self.files));
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
            .map_err(|source| ContentError::Io {
                path: staged_path.clone(),
                source,
            })?;
        destination
            .write_all(bytes)
            .and_then(|_| destination.flush())
            .map_err(|source| ContentError::Io {
                path: staged_path.clone(),
                source,
            })?;
        let mut permissions = destination
            .metadata()
            .map_err(|source| ContentError::Io {
                path: staged_path.clone(),
                source,
            })?
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&staged_path, permissions).map_err(|source| ContentError::Io {
            path: staged_path.clone(),
            source,
        })?;
        self.files += 1;
        self.total_bytes = projected_total;
        Ok(StagedAsset {
            slot: slot.to_string(),
            kind: kind.to_string(),
            size_bytes,
            sha256,
            spec_relative_path: format!("inline/{slot}"),
            staged_path,
            properties: BTreeMap::from([("content_origin".into(), "inline_fixture".into())]),
        })
    }

    pub(crate) fn resolve_private(
        &mut self,
        slot: &str,
        kind: &str,
        source_path: &Path,
        label: String,
        expected_sha256: &str,
        properties: BTreeMap<String, String>,
    ) -> Result<StagedAsset, ContentError> {
        if !source_path.is_absolute() {
            return Err(ContentError::UnsafePath(source_path.to_path_buf()));
        }
        self.stage_source(
            slot,
            kind,
            label,
            source_path.to_path_buf(),
            Some(expected_sha256),
            properties,
        )
    }

    pub(crate) fn resolve_provider(
        &mut self,
        slot: &str,
        kind: &str,
        output: &ResolvedProviderContent,
    ) -> Result<StagedAsset, ContentError> {
        let properties = BTreeMap::from([
            ("content_origin".into(), "provider".into()),
            ("provider_id".into(), output.provider_id.clone()),
            ("provider_version".into(), output.provider_version.clone()),
            (
                "provider_executable_sha256".into(),
                output.executable_sha256.clone(),
            ),
            (
                "provider_argument_sha256".into(),
                output.argument_sha256.clone(),
            ),
            (
                "provider_request_sha256".into(),
                output.request_sha256.clone(),
            ),
            (
                "provider_response_sha256".into(),
                output.response_sha256.clone(),
            ),
            (
                "provider_protocol_version".into(),
                super::CONTENT_PROVIDER_PROTOCOL_VERSION.into(),
            ),
            ("provider_network_policy".into(), "disabled".into()),
            ("provider_resource_outcome".into(), "within_limits".into()),
            ("provider_termination".into(), "exit_zero".into()),
        ]);
        let asset = self.resolve_private(
            slot,
            kind,
            &output.path,
            format!("providers/{}/{}", output.provider_id, slot),
            &output.sha256,
            properties,
        )?;
        if asset.size_bytes != output.size_bytes {
            return Err(ContentError::FileChanged(format!(
                "providers/{}/{}",
                output.provider_id, slot
            )));
        }
        Ok(asset)
    }

    fn stage_source(
        &mut self,
        slot: &str,
        kind: &str,
        relative_text: String,
        source_path: PathBuf,
        expected_sha256: Option<&str>,
        properties: BTreeMap<String, String>,
    ) -> Result<StagedAsset, ContentError> {
        let mut source = open_no_follow(&source_path)?;
        let before = source.metadata().map_err(|source| ContentError::Io {
            path: source_path.clone(),
            source,
        })?;
        if !before.is_file() {
            return Err(ContentError::NotRegular(relative_text));
        }
        if before.len() > self.limits.max_file_bytes {
            return Err(ContentError::FileSizeLimit {
                path: relative_text,
                size: before.len(),
                limit: self.limits.max_file_bytes,
            });
        }
        let projected_total = self
            .total_bytes
            .checked_add(before.len())
            .ok_or(ContentError::TotalSizeOverflow)?;
        if projected_total > self.limits.max_total_bytes {
            return Err(ContentError::TotalSizeLimit {
                size: projected_total,
                limit: self.limits.max_total_bytes,
            });
        }

        let staged_name = format!("asset-{:08}.bin", self.files);
        let staged_path = self.staging_root.join(staged_name);
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
            .map_err(|source| ContentError::Io {
                path: staged_path.clone(),
                source,
            })?;
        let copy_result = copy_and_hash(&mut source, &mut destination, self.limits.max_file_bytes);
        let (observed_bytes, sha256) = match copy_result {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_file(&staged_path);
                return Err(error);
            }
        };
        if observed_bytes != before.len() {
            let _ = fs::remove_file(&staged_path);
            return Err(ContentError::FileChanged(relative_text));
        }
        let after_path = fs::symlink_metadata(&source_path).map_err(|source| ContentError::Io {
            path: source_path.clone(),
            source,
        })?;
        if !same_file(&before, &after_path) {
            let _ = fs::remove_file(&staged_path);
            return Err(ContentError::FileChanged(relative_text));
        }
        if let Some(expected) = expected_sha256 {
            if expected != sha256 {
                let _ = fs::remove_file(&staged_path);
                return Err(ContentError::HashMismatch {
                    path: relative_text,
                    expected: expected.to_string(),
                    actual: sha256,
                });
            }
        }
        destination.flush().map_err(|source| ContentError::Io {
            path: staged_path.clone(),
            source,
        })?;
        let mut permissions = destination
            .metadata()
            .map_err(|source| ContentError::Io {
                path: staged_path.clone(),
                source,
            })?
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&staged_path, permissions).map_err(|source| ContentError::Io {
            path: staged_path.clone(),
            source,
        })?;

        self.files += 1;
        self.total_bytes = projected_total;
        Ok(StagedAsset {
            slot: slot.to_string(),
            kind: kind.to_string(),
            size_bytes: observed_bytes,
            sha256,
            spec_relative_path: relative_text,
            staged_path,
            properties,
        })
    }
}

fn safe_relative_text(path: &Path) -> Result<String, ContentError> {
    let text = path
        .to_str()
        .ok_or_else(|| ContentError::UnsafePath(path.to_path_buf()))?;
    if text.is_empty()
        || text.contains(['\\', ':', '\0'])
        || text.split('/').any(str::is_empty)
        || path.is_absolute()
        || !path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component.as_os_str() != "."
                && component.as_os_str() != ".."
        })
    {
        return Err(ContentError::UnsafePath(path.to_path_buf()));
    }
    Ok(text.to_string())
}

fn verify_plain_directory(path: &Path) -> Result<(), ContentError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ContentError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ContentError::NotPlainDirectory(path.to_path_buf()));
    }
    Ok(())
}

fn verify_component_path(root: &Path, relative: &Path) -> Result<PathBuf, ContentError> {
    let mut current = root.to_path_buf();
    let count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current).map_err(|source| ContentError::Io {
            path: current.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ContentError::Symlink(current));
        }
        if index + 1 == count {
            if !metadata.is_file() {
                return Err(ContentError::NotRegular(relative.display().to_string()));
            }
        } else if !metadata.is_dir() {
            return Err(ContentError::NonDirectoryAncestor(current));
        }
    }
    Ok(current)
}

fn open_no_follow(path: &Path) -> Result<File, ContentError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    options.open(path).map_err(|source| ContentError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.dev() == after.dev() && before.ino() == after.ino() && before.len() == after.len()
}

#[cfg(not(unix))]
fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.len() == after.len()
        && before.modified().ok().is_some()
        && before.modified().ok() == after.modified().ok()
}

pub(crate) fn copy_and_hash(
    source: &mut File,
    destination: &mut File,
    maximum: u64,
) -> Result<(u64, String), ContentError> {
    let mut hasher = StreamingSha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let read = source.read(&mut buffer).map_err(ContentError::Stream)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ContentError::TotalSizeOverflow)?;
        if total > maximum {
            return Err(ContentError::StreamLimit {
                size: total,
                maximum,
            });
        }
        hasher.update(&buffer[..read]);
        destination
            .write_all(&buffer[..read])
            .map_err(ContentError::Stream)?;
    }
    Ok((total, hasher.finish_hex()))
}

#[derive(Debug, Clone)]
pub(crate) struct StreamingSha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length_bytes: u64,
}

impl StreamingSha256 {
    pub(crate) fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffered: 0,
            length_bytes: 0,
        }
    }

    pub(crate) fn update(&mut self, mut bytes: &[u8]) {
        self.length_bytes = self
            .length_bytes
            .checked_add(bytes.len() as u64)
            .expect("content resource limits fit u64");
        if self.buffered > 0 {
            let take = (64 - self.buffered).min(bytes.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&bytes[..take]);
            self.buffered += take;
            bytes = &bytes[take..];
            if self.buffered < 64 {
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffered = 0;
        }
        while bytes.len() >= 64 {
            self.compress(&bytes[..64]);
            bytes = &bytes[64..];
        }
        self.buffer[..bytes.len()].copy_from_slice(bytes);
        self.buffered = bytes.len();
    }

    pub(crate) fn finish_hex(mut self) -> String {
        let bit_length = self.length_bytes * 8;
        let mut tail = [0_u8; 128];
        tail[..self.buffered].copy_from_slice(&self.buffer[..self.buffered]);
        tail[self.buffered] = 0x80;
        let blocks = if self.buffered < 56 { 1 } else { 2 };
        let end = blocks * 64;
        tail[end - 8..end].copy_from_slice(&bit_length.to_be_bytes());
        for block in tail[..end].chunks_exact(64) {
            self.compress(block);
        }
        self.state
            .iter()
            .map(|word| format!("{word:08x}"))
            .collect()
    }

    fn compress(&mut self, block: &[u8]) {
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
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes(block[start..start + 4].try_into().unwrap());
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
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(value);
        }
    }
}

#[derive(Debug)]
pub enum ContentError {
    InvalidLimits,
    InvalidSlot,
    UnsafePath(PathBuf),
    NotPlainDirectory(PathBuf),
    Symlink(PathBuf),
    NonDirectoryAncestor(PathBuf),
    NotRegular(String),
    FileCountLimit {
        limit: usize,
    },
    FileSizeLimit {
        path: String,
        size: u64,
        limit: u64,
    },
    TotalSizeOverflow,
    TotalSizeLimit {
        size: u64,
        limit: u64,
    },
    StreamLimit {
        size: u64,
        maximum: u64,
    },
    FileChanged(String),
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Stream(std::io::Error),
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("content limits must be non-zero"),
            Self::InvalidSlot => formatter.write_str("content slot and kind must be non-empty"),
            Self::UnsafePath(path) => write!(formatter, "unsafe content path {}", path.display()),
            Self::NotPlainDirectory(path) => {
                write!(formatter, "{} is not a plain directory", path.display())
            }
            Self::Symlink(path) => write!(
                formatter,
                "content path traverses symlink {}",
                path.display()
            ),
            Self::NonDirectoryAncestor(path) => write!(
                formatter,
                "content path ancestor {} is not a directory",
                path.display()
            ),
            Self::NotRegular(path) => {
                write!(formatter, "content path {path} is not a regular file")
            }
            Self::FileCountLimit { limit } => {
                write!(formatter, "content file count exceeds {limit}")
            }
            Self::FileSizeLimit { path, size, limit } => write!(
                formatter,
                "content file {path} is {size} bytes, limit {limit}"
            ),
            Self::TotalSizeOverflow => formatter.write_str("content total byte count overflow"),
            Self::TotalSizeLimit { size, limit } => {
                write!(formatter, "content total is {size} bytes, limit {limit}")
            }
            Self::StreamLimit { size, maximum } => {
                write!(formatter, "stream reached {size} bytes, limit {maximum}")
            }
            Self::FileChanged(path) => {
                write!(formatter, "content file {path} changed during staging")
            }
            Self::HashMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "content file {path} hash {actual}, expected {expected}"
            ),
            Self::Io { path, source } => {
                write!(formatter, "content I/O {}: {source}", path.display())
            }
            Self::Stream(source) => write!(formatter, "content stream I/O: {source}"),
        }
    }
}

impl std::error::Error for ContentError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::sha256_hex;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        inputs: PathBuf,
        staging: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "dts-composition-content-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let inputs = root.join("inputs");
            let staging = root.join("staging");
            fs::create_dir_all(inputs.join("nested")).unwrap();
            fs::create_dir(&staging).unwrap();
            Self {
                root,
                inputs,
                staging,
            }
        }

        fn resolver(&self, limits: ContentLimits) -> LocalContentResolver {
            LocalContentResolver::new(&self.inputs, &self.staging, limits).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn limits() -> ContentLimits {
        ContentLimits {
            max_files: 2,
            max_file_bytes: 1024,
            max_total_bytes: 1536,
        }
    }

    #[test]
    fn streams_hashes_and_stages_under_allocator_owned_name() {
        let fixture = Fixture::new();
        let bytes = b"deterministic local content";
        fs::write(fixture.inputs.join("nested/frame.raw"), bytes).unwrap();
        let expected = sha256_hex(bytes);
        let asset = fixture
            .resolver(limits())
            .resolve(
                "pixels",
                "native_pixels",
                Path::new("nested/frame.raw"),
                Some(&expected),
            )
            .unwrap();
        assert_eq!(asset.sha256, expected);
        assert_eq!(asset.size_bytes, bytes.len() as u64);
        assert_eq!(asset.spec_relative_path, "nested/frame.raw");
        assert_eq!(asset.staged_path.file_name().unwrap(), "asset-00000000.bin");
        assert_eq!(fs::read(asset.staged_path).unwrap(), bytes);
    }

    #[test]
    fn streaming_sha_matches_existing_known_digest_across_block_boundaries() {
        let bytes = (0..200).map(|value| value as u8).collect::<Vec<_>>();
        let mut hasher = StreamingSha256::new();
        for chunk in bytes.chunks(7) {
            hasher.update(chunk);
        }
        assert_eq!(hasher.finish_hex(), sha256_hex(&bytes));
    }

    #[test]
    fn rejects_unsafe_paths_hash_mismatch_and_resource_overruns() {
        let fixture = Fixture::new();
        fs::write(fixture.inputs.join("one.bin"), vec![0_u8; 800]).unwrap();
        fs::write(fixture.inputs.join("two.bin"), vec![0_u8; 800]).unwrap();
        let mut resolver = fixture.resolver(limits());
        assert!(matches!(
            resolver.resolve("bulk", "mesh", Path::new("../one.bin"), None),
            Err(ContentError::UnsafePath(_))
        ));
        assert!(matches!(
            resolver.resolve("bulk", "mesh", Path::new("one.bin"), Some(&"0".repeat(64))),
            Err(ContentError::HashMismatch { .. })
        ));
        resolver
            .resolve("bulk", "mesh", Path::new("one.bin"), None)
            .unwrap();
        assert!(matches!(
            resolver.resolve("bulk2", "mesh", Path::new("two.bin"), None),
            Err(ContentError::TotalSizeLimit { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_at_any_path_component() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fs::write(fixture.inputs.join("nested/frame.raw"), b"pixels").unwrap();
        symlink(fixture.inputs.join("nested"), fixture.inputs.join("linked")).unwrap();
        assert!(matches!(
            fixture.resolver(limits()).resolve(
                "pixels",
                "native_pixels",
                Path::new("linked/frame.raw"),
                None
            ),
            Err(ContentError::Symlink(_))
        ));
    }
}
