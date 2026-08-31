use std::borrow::Cow;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/embedded_product_resources.rs"));

pub const PRODUCT_RESOURCE_SET_VERSION: &str = "1.0.0";

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
