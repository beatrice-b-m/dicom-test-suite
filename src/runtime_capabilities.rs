//! Pure runtime capability evaluation for registry requirements.
//!
//! Planning supplies an explicit inventory. This module never probes the file
//! system, process environment, network, or executable search path.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::recipes::{BackendAvailability, CodecRegistryError, TransferSyntaxBackendRegistry};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryRuntimeRequirements {
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub external_codecs: Vec<String>,
    #[serde(default)]
    pub external_validators: Vec<String>,
    #[serde(default)]
    pub external_providers: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityInventory {
    pub compiled_features: BTreeSet<String>,
    pub executable_codec_backends: BTreeSet<String>,
    pub available_executables: BTreeSet<String>,
    /// Caller-qualified executable identities. These are planning assertions,
    /// not results of PATH or filesystem discovery.
    pub executable_identities: BTreeMap<String, QualifiedExecutableIdentity>,
    pub external_validators: BTreeSet<String>,
    pub external_providers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualifiedExecutableIdentity {
    pub version: String,
    pub executable_sha256: String,
}

pub fn qualified_executable_version_id(identity: &QualifiedExecutableIdentity) -> String {
    format!(
        "qualified_{}",
        crate::sha256_hex(identity.version.as_bytes())
    )
}

impl CapabilityInventory {
    /// Compile-time features and their linked in-process codec backends.
    /// External commands, validators, and providers remain empty until the
    /// caller injects an already-qualified runtime inventory.
    pub fn compiled() -> Self {
        let mut compiled_features = BTreeSet::new();
        let mut executable_codec_backends = BTreeSet::new();
        for (name, enabled, linked_backends) in [
            (
                "charls",
                cfg!(feature = "charls"),
                &["dicom_rs_charls_jpeg_ls_lossless_writer"][..],
            ),
            (
                "deflate",
                cfg!(feature = "deflate"),
                &[
                    "dicom_rs_deflated_dataset_writer",
                    "dicom_rs_deflated_image_frame_writer",
                ][..],
            ),
            (
                "jpeg",
                cfg!(feature = "jpeg"),
                &["dicom_rs_jpeg_baseline_writer"][..],
            ),
            (
                "jpegxl",
                cfg!(feature = "jpegxl"),
                &["dicom_rs_jpegxl_lossless_writer"][..],
            ),
            (
                "jpeg2000",
                cfg!(feature = "jpeg2000"),
                &["project_openjp2_jpeg2000_lossless_writer"][..],
            ),
            ("htj2k_openjph", cfg!(feature = "htj2k_openjph"), &[][..]),
            (
                "legacy_jpeg_dcmtk",
                cfg!(feature = "legacy_jpeg_dcmtk"),
                &[][..],
            ),
        ] {
            if enabled {
                compiled_features.insert(name.into());
                executable_codec_backends
                    .extend(linked_backends.iter().map(|backend| (*backend).to_string()));
            }
        }
        Self {
            compiled_features,
            executable_codec_backends,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvaluationRequest<'a> {
    pub transfer_syntax_uid: &'a str,
    pub determinism: &'a str,
    pub requirements: &'a RegistryRuntimeRequirements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapabilityKind {
    CompileTimeFeature,
    CodecBackend,
    CodecExecutable,
    ExternalValidator,
    ExternalProvider,
    RegistryContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    FeatureDisabled,
    CodecBackendUnavailable,
    ExecutableUnavailable,
    ExternalValidatorUnavailable,
    ExternalProviderUnavailable,
    RegistryContractInvalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableCapability {
    pub kind: CapabilityKind,
    pub capability_id: String,
    pub reason: UnavailableReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvaluation {
    pub transfer_syntax_uid: String,
    pub backend_id: Option<String>,
    pub available: bool,
    pub unavailable: Vec<UnavailableCapability>,
}

#[derive(Debug)]
pub enum CapabilityEvaluatorError {
    CodecRegistry(CodecRegistryError),
}

impl fmt::Display for CapabilityEvaluatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CodecRegistry(error) => write!(formatter, "codec registry: {error}"),
        }
    }
}

impl std::error::Error for CapabilityEvaluatorError {}

impl From<CodecRegistryError> for CapabilityEvaluatorError {
    fn from(value: CodecRegistryError) -> Self {
        Self::CodecRegistry(value)
    }
}

#[derive(Debug)]
pub struct RuntimeCapabilityEvaluator {
    codecs: TransferSyntaxBackendRegistry,
}

impl RuntimeCapabilityEvaluator {
    pub(crate) fn from_installed_matrix(json: &str) -> Result<Self, CapabilityEvaluatorError> {
        Ok(Self {
            codecs: TransferSyntaxBackendRegistry::from_capability_matrix(json)?,
        })
    }

    pub fn committed() -> Result<Self, CapabilityEvaluatorError> {
        Ok(Self {
            codecs: TransferSyntaxBackendRegistry::load_committed()?,
        })
    }

    pub fn evaluate(
        &self,
        request: CapabilityEvaluationRequest<'_>,
        inventory: &CapabilityInventory,
    ) -> CapabilityEvaluation {
        let Some(backend) = self.codecs.for_transfer_syntax(request.transfer_syntax_uid) else {
            return CapabilityEvaluation {
                transfer_syntax_uid: request.transfer_syntax_uid.into(),
                backend_id: None,
                available: false,
                unavailable: vec![UnavailableCapability {
                    kind: CapabilityKind::RegistryContract,
                    capability_id: request.transfer_syntax_uid.into(),
                    reason: UnavailableReason::RegistryContractInvalid(
                        "transfer syntax has no committed executable backend".into(),
                    ),
                }],
            };
        };

        let mut unavailable = Vec::new();
        if let Err(error) = self.codecs.validate_registry_requirements(
            request.transfer_syntax_uid,
            request.determinism,
            &request.requirements.features,
            &request.requirements.external_codecs,
        ) {
            unavailable.push(UnavailableCapability {
                kind: CapabilityKind::RegistryContract,
                capability_id: request.transfer_syntax_uid.into(),
                reason: UnavailableReason::RegistryContractInvalid(error.to_string()),
            });
        }
        for feature in &request.requirements.features {
            if !inventory.compiled_features.contains(feature) {
                unavailable.push(UnavailableCapability {
                    kind: CapabilityKind::CompileTimeFeature,
                    capability_id: feature.clone(),
                    reason: UnavailableReason::FeatureDisabled,
                });
            }
        }
        if backend.availability != BackendAvailability::BuiltIn
            && !inventory
                .executable_codec_backends
                .contains(backend.backend_id)
        {
            unavailable.push(UnavailableCapability {
                kind: CapabilityKind::CodecBackend,
                capability_id: backend.backend_id.into(),
                reason: UnavailableReason::CodecBackendUnavailable,
            });
        }
        if let Some(executable) = backend.external_tool {
            if !inventory.available_executables.contains(executable) {
                unavailable.push(UnavailableCapability {
                    kind: CapabilityKind::CodecExecutable,
                    capability_id: executable.into(),
                    reason: UnavailableReason::ExecutableUnavailable,
                });
            }
        }
        for validator in &request.requirements.external_validators {
            if !inventory.external_validators.contains(validator) {
                unavailable.push(UnavailableCapability {
                    kind: CapabilityKind::ExternalValidator,
                    capability_id: validator.clone(),
                    reason: UnavailableReason::ExternalValidatorUnavailable,
                });
            }
        }
        for provider in &request.requirements.external_providers {
            if !inventory.external_providers.contains(provider) {
                unavailable.push(UnavailableCapability {
                    kind: CapabilityKind::ExternalProvider,
                    capability_id: provider.clone(),
                    reason: UnavailableReason::ExternalProviderUnavailable,
                });
            }
        }
        unavailable.sort_by(|left, right| {
            (left.kind, &left.capability_id).cmp(&(right.kind, &right.capability_id))
        });
        unavailable.dedup_by(|left, right| {
            left.kind == right.kind && left.capability_id == right.capability_id
        });
        CapabilityEvaluation {
            transfer_syntax_uid: request.transfer_syntax_uid.into(),
            backend_id: Some(backend.backend_id.into()),
            available: unavailable.is_empty(),
            unavailable,
        }
    }
}
