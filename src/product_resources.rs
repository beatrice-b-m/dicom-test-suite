use std::borrow::Cow;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/embedded_product_resources.rs"));

pub const PRODUCT_RESOURCE_SET_VERSION: &str = "1.0.0";
pub const TEMPLATE_CATALOG_RESOURCE: &str = "templates/catalog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductResourceOrigin {
    Embedded,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductResourceRecord {
    pub logical_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductResourceIdentity {
    pub resource_set_version: String,
    pub origin: ProductResourceOrigin,
    pub resource_count: usize,
    pub resource_set_sha256: String,
    pub resources: Vec<ProductResourceRecord>,
}

#[derive(Debug, Clone)]
enum ProductResourceSource {
    Embedded,
    Explicit(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ProductResources {
    source: ProductResourceSource,
}

#[derive(Debug)]
pub struct ProductResourceSnapshot {
    root: PathBuf,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ProductResourceError {
    UnsafeLogicalPath(String),
    UnknownResource(String),
    Read {
        logical_path: String,
        path: PathBuf,
        source: std::io::Error,
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

impl fmt::Display for ProductResourceError {
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

impl std::error::Error for ProductResourceError {}

impl ProductResources {
    pub fn embedded() -> Self {
        Self {
            source: ProductResourceSource::Embedded,
        }
    }

    pub fn explicit(root: impl Into<PathBuf>) -> Self {
        Self {
            source: ProductResourceSource::Explicit(root.into()),
        }
    }

    pub fn origin(&self) -> ProductResourceOrigin {
        match self.source {
            ProductResourceSource::Embedded => ProductResourceOrigin::Embedded,
            ProductResourceSource::Explicit(_) => ProductResourceOrigin::Explicit,
        }
    }

    pub fn logical_paths(&self) -> Vec<&'static str> {
        EMBEDDED_PRODUCT_RESOURCES
            .iter()
            .map(|(path, _)| *path)
            .collect()
    }

    pub fn contains(&self, logical_path: &str) -> bool {
        validate_logical_path(logical_path).is_ok()
            && EMBEDDED_PRODUCT_RESOURCES
                .binary_search_by_key(&logical_path, |(path, _)| *path)
                .is_ok()
    }

    pub fn bytes(&self, logical_path: &str) -> Result<Cow<'static, [u8]>, ProductResourceError> {
        validate_logical_path(logical_path)?;
        let index = EMBEDDED_PRODUCT_RESOURCES
            .binary_search_by_key(&logical_path, |(path, _)| *path)
            .map_err(|_| ProductResourceError::UnknownResource(logical_path.to_string()))?;
        match &self.source {
            ProductResourceSource::Embedded => {
                Ok(Cow::Borrowed(EMBEDDED_PRODUCT_RESOURCES[index].1))
            }
            ProductResourceSource::Explicit(root) => {
                let path = root.join(logical_path);
                fs::read(&path)
                    .map(Cow::Owned)
                    .map_err(|source| ProductResourceError::Read {
                        logical_path: logical_path.to_string(),
                        path,
                        source,
                    })
            }
        }
    }

    pub fn text(&self, logical_path: &str) -> Result<Cow<'static, str>, ProductResourceError> {
        match self.bytes(logical_path)? {
            Cow::Borrowed(bytes) => std::str::from_utf8(bytes)
                .map(Cow::Borrowed)
                .map_err(|_| ProductResourceError::NonUtf8(logical_path.to_string())),
            Cow::Owned(bytes) => String::from_utf8(bytes)
                .map(Cow::Owned)
                .map_err(|_| ProductResourceError::NonUtf8(logical_path.to_string())),
        }
    }

    pub fn identity(&self) -> Result<ProductResourceIdentity, ProductResourceError> {
        let mut records = Vec::with_capacity(EMBEDDED_PRODUCT_RESOURCES.len());
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
            records.push(ProductResourceRecord {
                logical_path: logical_path.to_string(),
                size_bytes: bytes.len() as u64,
                sha256,
            });
        }
        Ok(ProductResourceIdentity {
            resource_set_version: PRODUCT_RESOURCE_SET_VERSION.to_string(),
            origin: self.origin(),
            resource_count: records.len(),
            resource_set_sha256: crate::sha256_hex(&identity_bytes),
            resources: records,
        })
    }

    pub fn verify_integrity(&self) -> Result<ProductResourceIdentity, ProductResourceError> {
        let actual = self.identity()?;
        if self.origin() == ProductResourceOrigin::Embedded {
            return Ok(actual);
        }
        let expected = Self::embedded().identity()?;
        if actual.resource_set_sha256 != expected.resource_set_sha256 {
            return Err(ProductResourceError::Integrity {
                expected_resource_set_sha256: expected.resource_set_sha256,
                actual_resource_set_sha256: actual.resource_set_sha256,
            });
        }
        Ok(actual)
    }

    pub fn snapshot(&self) -> Result<ProductResourceSnapshot, ProductResourceError> {
        self.verify_integrity()?;
        static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir();
        let root = (0..128)
            .find_map(|_| {
                let sequence = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
                let candidate = parent.join(format!(
                    "dicom-test-suite-resources-{}-{sequence}",
                    std::process::id()
                ));
                match create_private_directory(&candidate) {
                    Ok(()) => Some(Ok(candidate)),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(source) => Some(Err(ProductResourceError::CreateSnapshot {
                        path: candidate,
                        source,
                    })),
                }
            })
            .transpose()?
            .ok_or_else(|| ProductResourceError::CreateSnapshot {
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
                    ProductResourceError::WriteSnapshot {
                        logical_path: logical_path.to_string(),
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
            let bytes = self.bytes(logical_path)?;
            fs::write(&path, &bytes).map_err(|source| ProductResourceError::WriteSnapshot {
                logical_path: logical_path.to_string(),
                path,
                source,
            })?;
        }
        Ok(ProductResourceSnapshot { root })
    }
}

impl ProductResourceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsafeLogicalPath(_) | Self::UnknownResource(_) | Self::NonUtf8(_) => {
                "resource.document.invalid"
            }
            Self::Integrity { .. } => "evidence.integrity.failed",
            Self::Read { .. } => "io.read.failed",
            Self::CreateSnapshot { .. } | Self::WriteSnapshot { .. } => "io.write.failed",
        }
    }
}

impl ProductResourceSnapshot {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, logical_path: &str) -> Result<PathBuf, ProductResourceError> {
        validate_logical_path(logical_path)?;
        Ok(self.root.join(logical_path))
    }
}

impl Drop for ProductResourceSnapshot {
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

fn validate_logical_path(path: &str) -> Result<(), ProductResourceError> {
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
        Err(ProductResourceError::UnsafeLogicalPath(path.to_string()))
    }
}
