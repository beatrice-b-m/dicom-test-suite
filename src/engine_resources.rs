use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/embedded_engine_resources.rs"));

pub const ENGINE_RESOURCE_SET_VERSION: &str = "1.0.0";
/// R4.1 preserves the existing digest membership until R4.3/R4.4 split its
/// independently versioned identity domains.
pub const ENGINE_RESOURCE_SET_MEMBERSHIP: EngineResourceSetMembership =
    EngineResourceSetMembership::TransitionalMonolithic;
pub const TEMPLATE_CATALOG_RESOURCE: &str = "templates/catalog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineResourceSetMembership {
    TransitionalMonolithic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineResourceOrigin {
    Embedded,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineResourceRecord {
    pub logical_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineResourceIdentity {
    pub resource_set_version: String,
    pub origin: EngineResourceOrigin,
    pub resource_count: usize,
    pub resource_set_sha256: String,
    pub resources: Vec<EngineResourceRecord>,
}

#[derive(Debug, Clone)]
enum EngineResourceSource {
    Embedded,
    Explicit(Arc<BTreeMap<&'static str, Vec<u8>>>),
}

#[derive(Debug, Clone)]
pub struct EngineResources {
    source: EngineResourceSource,
}

#[derive(Debug)]
pub struct EngineResourceSnapshot {
    root: PathBuf,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EngineResourceError {
    UnsafeLogicalPath(String),
    UnknownResource(String),
    Read {
        logical_path: String,
        path: PathBuf,
        source: std::io::Error,
    },
    Symlink {
        logical_path: String,
        path: PathBuf,
    },
    NotRegular {
        logical_path: String,
        path: PathBuf,
    },
    SizeMismatch {
        logical_path: String,
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    Unstable {
        logical_path: String,
        path: PathBuf,
    },
    NonUtf8(String),
    Integrity {
        expected_resource_set_sha256: String,
        actual_resource_set_sha256: String,
    },
    CreateSnapshot {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteSnapshot {
        logical_path: String,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for EngineResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeLogicalPath(path) => {
                write!(formatter, "unsafe product resource path: {path}")
            }
            Self::UnknownResource(path) => write!(formatter, "unknown product resource: {path}"),
            Self::Read {
                logical_path,
                path,
                source,
            } => write!(
                formatter,
                "read product resource {logical_path} at {}: {source}",
                path.display()
            ),
            Self::Symlink { logical_path, path } => write!(
                formatter,
                "engine resource {logical_path} resolves through a symbolic link at {}",
                path.display()
            ),
            Self::NotRegular { logical_path, path } => write!(
                formatter,
                "engine resource {logical_path} is not a regular file at {}",
                path.display()
            ),
            Self::SizeMismatch {
                logical_path,
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "engine resource {logical_path} at {} has size {actual}, expected {expected}",
                path.display()
            ),
            Self::Unstable { logical_path, path } => write!(
                formatter,
                "engine resource {logical_path} changed while being captured at {}",
                path.display()
            ),
            Self::NonUtf8(path) => write!(formatter, "product resource is not UTF-8: {path}"),
            Self::Integrity {
                expected_resource_set_sha256,
                actual_resource_set_sha256,
            } => write!(
                formatter,
                "product resource integrity failed: expected set {expected_resource_set_sha256}, got {actual_resource_set_sha256}"
            ),
            Self::CreateSnapshot { path, source } => {
                write!(
                    formatter,
                    "create product resource snapshot {}: {source}",
                    path.display()
                )
            }
            Self::WriteSnapshot {
                logical_path,
                path,
                source,
            } => write!(
                formatter,
                "write product resource {logical_path} to snapshot {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for EngineResourceError {}

impl EngineResources {
    pub fn embedded() -> Self {
        Self {
            source: EngineResourceSource::Embedded,
        }
    }

    pub fn explicit(root: impl Into<PathBuf>) -> Result<Self, EngineResourceError> {
        let root = root.into();
        let captured = capture_explicit_resources(&root)?;
        let resources = Self {
            source: EngineResourceSource::Explicit(Arc::new(captured)),
        };
        resources.verify_integrity()?;
        Ok(resources)
    }

    pub fn origin(&self) -> EngineResourceOrigin {
        match self.source {
            EngineResourceSource::Embedded => EngineResourceOrigin::Embedded,
            EngineResourceSource::Explicit(_) => EngineResourceOrigin::Explicit,
        }
    }

    pub fn logical_paths(&self) -> Vec<&'static str> {
        EMBEDDED_ENGINE_RESOURCES
            .iter()
            .map(|(path, _)| *path)
            .collect()
    }

    pub fn contains(&self, logical_path: &str) -> bool {
        validate_logical_path(logical_path).is_ok()
            && EMBEDDED_ENGINE_RESOURCES
                .binary_search_by_key(&logical_path, |(path, _)| *path)
                .is_ok()
    }

    pub fn bytes(&self, logical_path: &str) -> Result<Cow<'static, [u8]>, EngineResourceError> {
        validate_logical_path(logical_path)?;
        let index = EMBEDDED_ENGINE_RESOURCES
            .binary_search_by_key(&logical_path, |(path, _)| *path)
            .map_err(|_| EngineResourceError::UnknownResource(logical_path.to_string()))?;
        match &self.source {
            EngineResourceSource::Embedded => Ok(Cow::Borrowed(EMBEDDED_ENGINE_RESOURCES[index].1)),
            EngineResourceSource::Explicit(captured) => Ok(Cow::Owned(
                captured
                    .get(logical_path)
                    .expect("validated engine resource was captured")
                    .clone(),
            )),
        }
    }

    pub fn text(&self, logical_path: &str) -> Result<Cow<'static, str>, EngineResourceError> {
        match self.bytes(logical_path)? {
            Cow::Borrowed(bytes) => std::str::from_utf8(bytes)
                .map(Cow::Borrowed)
                .map_err(|_| EngineResourceError::NonUtf8(logical_path.to_string())),
            Cow::Owned(bytes) => String::from_utf8(bytes)
                .map(Cow::Owned)
                .map_err(|_| EngineResourceError::NonUtf8(logical_path.to_string())),
        }
    }

    pub fn identity(&self) -> Result<EngineResourceIdentity, EngineResourceError> {
        let mut records = Vec::with_capacity(EMBEDDED_ENGINE_RESOURCES.len());
        let mut identity_bytes = Vec::new();
        for logical_path in self.logical_paths() {
            let bytes = self.bytes(logical_path)?;
            let sha256 = crate::sha256_hex(&bytes);
            identity_bytes.extend_from_slice(logical_path.as_bytes());
            identity_bytes.push(0);
            identity_bytes.extend_from_slice(sha256.as_bytes());
            identity_bytes.push(0);
            identity_bytes.extend_from_slice(bytes.len().to_string().as_bytes());
            identity_bytes.push(b'\n');
            records.push(EngineResourceRecord {
                logical_path: logical_path.to_string(),
                size_bytes: bytes.len() as u64,
                sha256,
            });
        }
        Ok(EngineResourceIdentity {
            resource_set_version: ENGINE_RESOURCE_SET_VERSION.to_string(),
            origin: self.origin(),
            resource_count: records.len(),
            resource_set_sha256: crate::sha256_hex(&identity_bytes),
            resources: records,
        })
    }

    pub fn verify_integrity(&self) -> Result<EngineResourceIdentity, EngineResourceError> {
        let actual = self.identity()?;
        if self.origin() == EngineResourceOrigin::Embedded {
            return Ok(actual);
        }
        let expected = Self::embedded().identity()?;
        if actual.resource_set_sha256 != expected.resource_set_sha256 {
            return Err(EngineResourceError::Integrity {
                expected_resource_set_sha256: expected.resource_set_sha256,
                actual_resource_set_sha256: actual.resource_set_sha256,
            });
        }
        Ok(actual)
    }

    pub fn snapshot(&self) -> Result<EngineResourceSnapshot, EngineResourceError> {
        self.verify_integrity()?;
        static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir();
        let root = (0..128)
            .find_map(|_| {
                let sequence = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
                let candidate = parent.join(format!(
                    "synth-dicom-gen-resources-{}-{sequence}",
                    std::process::id()
                ));
                match create_private_directory(&candidate) {
                    Ok(()) => Some(Ok(candidate)),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(source) => Some(Err(EngineResourceError::CreateSnapshot {
                        path: candidate,
                        source,
                    })),
                }
            })
            .transpose()?
            .ok_or_else(|| EngineResourceError::CreateSnapshot {
                path: parent,
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "could not allocate a unique resource snapshot",
                ),
            })?;

        for logical_path in self.logical_paths() {
            let path = root.join(logical_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    EngineResourceError::WriteSnapshot {
                        logical_path: logical_path.to_string(),
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
            let bytes = self.bytes(logical_path)?;
            fs::write(&path, &bytes).map_err(|source| EngineResourceError::WriteSnapshot {
                logical_path: logical_path.to_string(),
                path,
                source,
            })?;
        }
        Ok(EngineResourceSnapshot { root })
    }
}

impl EngineResourceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsafeLogicalPath(_)
            | Self::UnknownResource(_)
            | Self::NonUtf8(_)
            | Self::Symlink { .. }
            | Self::NotRegular { .. }
            | Self::SizeMismatch { .. }
            | Self::Unstable { .. } => "resource.document.invalid",
            Self::Integrity { .. } => "evidence.integrity.failed",
            Self::Read { .. } => "io.read.failed",
            Self::CreateSnapshot { .. } | Self::WriteSnapshot { .. } => "io.write.failed",
        }
    }
}

impl EngineResourceSnapshot {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, logical_path: &str) -> Result<PathBuf, EngineResourceError> {
        validate_logical_path(logical_path)?;
        Ok(self.root.join(logical_path))
    }
}

impl Drop for EngineResourceSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

fn validate_logical_path(path: &str) -> Result<(), EngineResourceError> {
    let parsed = Path::new(path);
    let safe = !path.is_empty()
        && !path.contains('\\')
        && !parsed.is_absolute()
        && parsed
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if safe {
        Ok(())
    } else {
        Err(EngineResourceError::UnsafeLogicalPath(path.to_string()))
    }
}

#[cfg(unix)]
fn capture_explicit_resources(
    root: &Path,
) -> Result<BTreeMap<&'static str, Vec<u8>>, EngineResourceError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let root_before = fs::symlink_metadata(root).map_err(|source| EngineResourceError::Read {
        logical_path: ".".to_string(),
        path: root.to_path_buf(),
        source,
    })?;
    if root_before.file_type().is_symlink() {
        return Err(EngineResourceError::Symlink {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }
    if !root_before.is_dir() {
        return Err(EngineResourceError::NotRegular {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }
    let root_handle = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)
        .map_err(|source| map_open_error(".", root.to_path_buf(), source))?;
    let root_open = root_handle
        .metadata()
        .map_err(|source| EngineResourceError::Read {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
            source,
        })?;
    if root_before.dev() != root_open.dev() || root_before.ino() != root_open.ino() {
        return Err(EngineResourceError::Unstable {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }

    let expected_total = EMBEDDED_ENGINE_RESOURCES
        .iter()
        .try_fold(0_u64, |total, (_, bytes)| {
            total.checked_add(bytes.len() as u64)
        })
        .expect("embedded engine resource total fits u64");
    let mut inspected_total = 0_u64;
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let file = open_resource_at(&root_handle, root, logical_path)?;
        let metadata = validate_open_resource(&file, root, logical_path, expected.len() as u64)?;
        inspected_total = inspected_total.checked_add(metadata.len()).ok_or_else(|| {
            EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                expected: expected_total,
                actual: u64::MAX,
            }
        })?;
    }
    if inspected_total != expected_total {
        return Err(EngineResourceError::SizeMismatch {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
            expected: expected_total,
            actual: inspected_total,
        });
    }

    let mut captured = BTreeMap::new();
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let mut file = open_resource_at(&root_handle, root, logical_path)?;
        let before = validate_open_resource(&file, root, logical_path, expected.len() as u64)?;
        let mut bytes = Vec::with_capacity(expected.len());
        file.by_ref()
            .take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| EngineResourceError::Read {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                source,
            })?;
        if bytes.len() != expected.len() {
            return Err(EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                expected: expected.len() as u64,
                actual: bytes.len() as u64,
            });
        }
        let after = file
            .metadata()
            .map_err(|source| EngineResourceError::Read {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                source,
            })?;
        if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len()
        {
            return Err(EngineResourceError::Unstable {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
            });
        }
        captured.insert(*logical_path, bytes);
    }

    let root_after = fs::symlink_metadata(root).map_err(|source| EngineResourceError::Read {
        logical_path: ".".to_string(),
        path: root.to_path_buf(),
        source,
    })?;
    if root_after.file_type().is_symlink()
        || !root_after.is_dir()
        || root_after.dev() != root_open.dev()
        || root_after.ino() != root_open.ino()
    {
        return Err(EngineResourceError::Unstable {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }
    Ok(captured)
}

#[cfg(unix)]
fn open_resource_at(
    root_handle: &fs::File,
    root: &Path,
    logical_path: &'static str,
) -> Result<fs::File, EngineResourceError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let components = Path::new(logical_path).components().collect::<Vec<_>>();
    let mut directory = root_handle
        .try_clone()
        .map_err(|source| EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path: root.to_path_buf(),
            source,
        })?;
    let mut traversed = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(EngineResourceError::UnsafeLogicalPath(
                logical_path.to_string(),
            ));
        };
        traversed.push(component);
        let name = CString::new(component.as_bytes())
            .map_err(|_| EngineResourceError::UnsafeLogicalPath(logical_path.to_string()))?;
        let is_last = index + 1 == components.len();
        let flags = libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if is_last {
                libc::O_RDONLY | libc::O_NONBLOCK
            } else {
                libc::O_RDONLY | libc::O_DIRECTORY
            };
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(map_open_error(
                logical_path,
                traversed,
                std::io::Error::last_os_error(),
            ));
        }
        let opened = unsafe { fs::File::from_raw_fd(descriptor) };
        if is_last {
            return Ok(opened);
        }
        directory = opened;
    }
    Err(EngineResourceError::UnsafeLogicalPath(
        logical_path.to_string(),
    ))
}

#[cfg(unix)]
fn validate_open_resource(
    file: &fs::File,
    root: &Path,
    logical_path: &'static str,
    expected: u64,
) -> Result<fs::Metadata, EngineResourceError> {
    let path = root.join(logical_path);
    let metadata = file
        .metadata()
        .map_err(|source| EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path: path.clone(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(EngineResourceError::NotRegular {
            logical_path: logical_path.to_string(),
            path,
        });
    }
    if metadata.len() != expected {
        return Err(EngineResourceError::SizeMismatch {
            logical_path: logical_path.to_string(),
            path,
            expected,
            actual: metadata.len(),
        });
    }
    Ok(metadata)
}

#[cfg(unix)]
fn map_open_error(
    logical_path: &str,
    path: PathBuf,
    source: std::io::Error,
) -> EngineResourceError {
    match source.raw_os_error() {
        Some(libc::ELOOP) => EngineResourceError::Symlink {
            logical_path: logical_path.to_string(),
            path,
        },
        Some(libc::ENOTDIR) | Some(libc::EISDIR) | Some(libc::ENXIO) => {
            EngineResourceError::NotRegular {
                logical_path: logical_path.to_string(),
                path,
            }
        }
        _ => EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path,
            source,
        },
    }
}

#[cfg(not(unix))]
fn capture_explicit_resources(
    root: &Path,
) -> Result<BTreeMap<&'static str, Vec<u8>>, EngineResourceError> {
    let expected_total = EMBEDDED_ENGINE_RESOURCES
        .iter()
        .map(|(_, bytes)| bytes.len() as u64)
        .sum::<u64>();
    let mut inspected_total = 0_u64;
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let path = explicit_resource_path(root, logical_path)?;
        let metadata = fs::metadata(&path).map_err(|source| EngineResourceError::Read {
            logical_path: (*logical_path).to_string(),
            path: path.clone(),
            source,
        })?;
        if metadata.len() != expected.len() as u64 {
            return Err(EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path,
                expected: expected.len() as u64,
                actual: metadata.len(),
            });
        }
        inspected_total += metadata.len();
    }
    if inspected_total != expected_total {
        return Err(EngineResourceError::SizeMismatch {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
            expected: expected_total,
            actual: inspected_total,
        });
    }
    let mut captured = BTreeMap::new();
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let path = explicit_resource_path(root, logical_path)?;
        let file = fs::File::open(&path).map_err(|source| EngineResourceError::Read {
            logical_path: (*logical_path).to_string(),
            path: path.clone(),
            source,
        })?;
        let mut bytes = Vec::with_capacity(expected.len());
        file.take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| EngineResourceError::Read {
                logical_path: (*logical_path).to_string(),
                path: path.clone(),
                source,
            })?;
        if bytes.len() != expected.len() {
            return Err(EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path,
                expected: expected.len() as u64,
                actual: bytes.len() as u64,
            });
        }
        captured.insert(*logical_path, bytes);
    }
    Ok(captured)
}

#[cfg(not(unix))]
fn explicit_resource_path(root: &Path, logical_path: &str) -> Result<PathBuf, EngineResourceError> {
    let mut path = root.to_path_buf();
    let root_metadata = fs::symlink_metadata(root).map_err(|source| EngineResourceError::Read {
        logical_path: logical_path.to_string(),
        path: root.to_path_buf(),
        source,
    })?;
    if root_metadata.file_type().is_symlink() {
        return Err(EngineResourceError::Symlink {
            logical_path: logical_path.to_string(),
            path,
        });
    }
    if !root_metadata.is_dir() {
        return Err(EngineResourceError::NotRegular {
            logical_path: logical_path.to_string(),
            path,
        });
    }
    let components = Path::new(logical_path).components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(EngineResourceError::UnsafeLogicalPath(
                logical_path.to_string(),
            ));
        };
        path.push(component);
        let metadata = fs::symlink_metadata(&path).map_err(|source| EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EngineResourceError::Symlink {
                logical_path: logical_path.to_string(),
                path,
            });
        }
        let is_last = index + 1 == components.len();
        if (is_last && !metadata.is_file()) || (!is_last && !metadata.is_dir()) {
            return Err(EngineResourceError::NotRegular {
                logical_path: logical_path.to_string(),
                path,
            });
        }
    }
    Ok(path)
}
