//! Supported Rust integration facade for the standalone product.
//!
//! Modules outside `sdk` remain available during the productization migration,
//! but only this facade is the supported Rust compatibility surface.

use std::fmt;
use std::path::Path;

use crate::discovery::{CapabilitiesResult, VersionResult};
use crate::product_resources::ProductResources;

/// A relocatable product handle backed by an integrity-checked resource set.
#[derive(Debug, Clone)]
pub struct DicomTestSuite {
    resources: ProductResources,
}

impl DicomTestSuite {
    /// Construct a product using the immutable resources embedded in the crate.
    pub fn embedded() -> Result<Self, SdkError> {
        Self::from_resources(ProductResources::embedded())
    }

    /// Construct a product from an explicit resource root.
    ///
    /// The root must contain the complete, byte-identical product resource set.
    /// There is no fallback to embedded or repository-relative resources.
    pub fn explicit_resource_root(root: impl AsRef<Path>) -> Result<Self, SdkError> {
        Self::from_resources(ProductResources::explicit(root.as_ref().to_path_buf()))
    }

    fn from_resources(resources: ProductResources) -> Result<Self, SdkError> {
        resources
            .verify_integrity()
            .map_err(|error| SdkError::classify("capabilities", error))?;
        Ok(Self { resources })
    }

    /// Return typed product, build, CLI, feature, and resource identity.
    pub fn version(&self) -> Result<VersionResult, SdkError> {
        crate::discovery::version_result(&self.resources)
            .map_err(|error| SdkError::classify("version", error))
    }

    /// Return typed live capabilities without converting absence into support.
    pub fn capabilities(&self) -> Result<CapabilitiesResult, SdkError> {
        crate::discovery::capabilities_result(&self.resources)
            .map_err(|error| SdkError::classify("capabilities", error))
    }
}

/// Stable broad classification for SDK failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SdkErrorKind {
    Request,
    Unavailable,
    Output,
    Execution,
    Internal,
}

/// A typed SDK failure carrying the same stable code taxonomy as CLI API 1.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SdkError {
    kind: SdkErrorKind,
    code: &'static str,
    message: String,
    retryable: bool,
    diagnostic: String,
}

impl SdkError {
    pub(crate) fn classify(command: &str, error: impl fmt::Display) -> Self {
        let diagnostic = error.to_string();
        let failure = crate::cli_protocol::CliFailure::classify(command, &diagnostic);
        let kind = match failure.exit {
            2 => SdkErrorKind::Request,
            3 => SdkErrorKind::Unavailable,
            4 => SdkErrorKind::Output,
            5 => SdkErrorKind::Execution,
            _ => SdkErrorKind::Internal,
        };
        Self {
            kind,
            code: failure.error.code,
            message: failure.error.message,
            retryable: failure.error.retryable,
            diagnostic,
        }
    }

    /// Stable namespaced code shared with the CLI error registry.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Stable broad failure category.
    pub fn kind(&self) -> SdkErrorKind {
        self.kind
    }

    /// Stable public error description associated with [`Self::code`].
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether retrying after an external state change can be meaningful.
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    /// Human diagnostic detail; callers must branch on [`Self::code`] instead.
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.diagnostic)
    }
}

impl std::error::Error for SdkError {}
