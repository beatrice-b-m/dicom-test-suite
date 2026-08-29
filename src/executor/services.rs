//! Injectable execution services and safe staged-asset bindings.
//!
//! These contracts are deliberately frontend-neutral. Services receive only
//! planned artifacts, typed execution bindings, and opaque staged-asset
//! handles; they do not receive publication roots or frontend request types.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::corpus_plan::{PlannedArtifact, ValidationPlan};
use crate::sha256_hex;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StagedAssetHandle(String);

impl StagedAssetHandle {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceError> {
        let value = value.into();
        validate_identifier("staged asset handle", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StagedAssetHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StagingRelativePath(String);

impl StagingRelativePath {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceError> {
        let value = value.into();
        validate_relative_path(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StagingRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetDeclaration {
    pub handle: StagedAssetHandle,
    pub relative_path: StagingRelativePath,
    pub size_bytes: u64,
    pub sha256: String,
    pub media_type: String,
    #[serde(default)]
    pub visibility: AssetVisibility,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetVisibility {
    #[default]
    Private,
    PublicationCandidate,
}

impl AssetDeclaration {
    pub fn validate(&self) -> Result<(), ServiceError> {
        validate_sha256("asset SHA-256", &self.sha256)?;
        validate_identifier("asset media type", &self.media_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducedAsset {
    pub declaration: AssetDeclaration,
    pub observed_size_bytes: u64,
    pub observed_sha256: String,
}

impl ProducedAsset {
    pub fn from_bytes(
        handle: StagedAssetHandle,
        relative_path: StagingRelativePath,
        media_type: impl Into<String>,
        bytes: &[u8],
    ) -> Self {
        let sha256 = sha256_hex(bytes);
        let size_bytes = bytes.len() as u64;
        Self {
            declaration: AssetDeclaration {
                handle,
                relative_path,
                size_bytes,
                sha256: sha256.clone(),
                media_type: media_type.into(),
                visibility: AssetVisibility::Private,
            },
            observed_size_bytes: size_bytes,
            observed_sha256: sha256,
        }
    }

    pub fn validate(&self) -> Result<(), ServiceError> {
        self.declaration.validate()?;
        validate_sha256("observed asset SHA-256", &self.observed_sha256)?;
        if self.declaration.size_bytes != self.observed_size_bytes {
            return Err(ServiceError::AssetSizeMismatch {
                handle: self.declaration.handle.clone(),
                declared: self.declaration.size_bytes,
                observed: self.observed_size_bytes,
            });
        }
        if self.declaration.sha256 != self.observed_sha256 {
            return Err(ServiceError::AssetHashMismatch {
                handle: self.declaration.handle.clone(),
                declared: self.declaration.sha256.clone(),
                observed: self.observed_sha256.clone(),
            });
        }
        Ok(())
    }
}

/// Metadata registry for assets already written inside private staging.
///
/// Registration is accepted only when the observed identity matches the
/// declared identity. Paths remain relative and are unique, so the executor
/// can safely join them beneath its private root.
#[derive(Debug, Clone, Default)]
pub struct StagedAssetRegistry {
    assets: BTreeMap<StagedAssetHandle, AssetDeclaration>,
    paths: BTreeSet<StagingRelativePath>,
}

impl StagedAssetRegistry {
    pub fn register(&mut self, asset: ProducedAsset) -> Result<(), ServiceError> {
        asset.validate()?;
        if self.assets.contains_key(&asset.declaration.handle) {
            return Err(ServiceError::DuplicateAssetHandle(asset.declaration.handle));
        }
        if !self.paths.insert(asset.declaration.relative_path.clone()) {
            return Err(ServiceError::DuplicateStagedPath(
                asset.declaration.relative_path,
            ));
        }
        self.assets
            .insert(asset.declaration.handle.clone(), asset.declaration);
        Ok(())
    }

    pub fn resolve(&self, handle: &StagedAssetHandle) -> Result<&AssetDeclaration, ServiceError> {
        self.assets
            .get(handle)
            .ok_or_else(|| ServiceError::UnknownAssetHandle(handle.clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssetDeclaration> {
        self.assets.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolIdentity {
    pub backend_id: String,
    pub version: String,
    pub protocol_version: Option<String>,
    pub executable_sha256: Option<String>,
}

impl ToolIdentity {
    pub fn validate(&self) -> Result<(), ServiceError> {
        validate_identifier("backend ID", &self.backend_id)?;
        validate_identifier("backend version", &self.version)?;
        if let Some(protocol) = &self.protocol_version {
            validate_identifier("protocol version", protocol)?;
        }
        if let Some(digest) = &self.executable_sha256 {
            validate_sha256("executable SHA-256", digest)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceEvidence {
    pub evidence_id: String,
    pub evidence_kind: String,
    pub producer: ToolIdentity,
    #[serde(default)]
    pub claims: BTreeMap<String, Value>,
}

impl ServiceEvidence {
    pub fn validate(&self) -> Result<(), ServiceError> {
        validate_identifier("evidence ID", &self.evidence_id)?;
        validate_identifier("evidence kind", &self.evidence_kind)?;
        self.producer.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ByteBinding {
    Inline {
        bytes: Vec<u8>,
        sha256: String,
    },
    StagedRange {
        asset: StagedAssetHandle,
        offset: u64,
        length: u64,
        sha256: String,
    },
}

impl ByteBinding {
    fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        match self {
            Self::Inline { bytes, sha256 } => {
                validate_sha256("inline binding SHA-256", sha256)?;
                let observed = sha256_hex(bytes);
                if observed != *sha256 {
                    return Err(ServiceError::InlineHashMismatch {
                        declared: sha256.clone(),
                        observed,
                    });
                }
            }
            Self::StagedRange {
                asset,
                offset,
                length,
                sha256,
            } => {
                validate_sha256("staged range SHA-256", sha256)?;
                let declaration = registry.resolve(asset)?;
                let end = offset
                    .checked_add(*length)
                    .ok_or(ServiceError::AssetRangeOverflow)?;
                if end > declaration.size_bytes {
                    return Err(ServiceError::AssetRangeOutOfBounds {
                        handle: asset.clone(),
                        end,
                        size: declaration.size_bytes,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFrameBinding {
    pub frame_number: u32,
    pub bytes: ByteBinding,
    pub rows: u32,
    pub columns: u32,
    pub samples_per_pixel: u16,
    pub bits_allocated: u16,
    pub photometric_interpretation: String,
}

impl NativeFrameBinding {
    fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        if self.frame_number == 0
            || self.rows == 0
            || self.columns == 0
            || self.samples_per_pixel == 0
            || self.bits_allocated == 0
        {
            return Err(ServiceError::InvalidFrameShape(self.frame_number));
        }
        validate_identifier(
            "photometric interpretation",
            &self.photometric_interpretation,
        )?;
        self.bytes.validate(registry)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SlotExecutionBinding {
    StagedAsset { asset: StagedAssetHandle },
    ProviderRequest { request: ProviderRequest },
    NativeFrames { frames: Vec<NativeFrameBinding> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactExecutionBindings {
    pub artifact_id: String,
    #[serde(default)]
    pub slots: BTreeMap<String, SlotExecutionBinding>,
}

impl ArtifactExecutionBindings {
    pub fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        validate_identifier("binding artifact ID", &self.artifact_id)?;
        for (slot, binding) in &self.slots {
            validate_identifier("content slot", slot)?;
            match binding {
                SlotExecutionBinding::StagedAsset { asset } => {
                    registry.resolve(asset)?;
                }
                SlotExecutionBinding::ProviderRequest { request } => request.validate(registry)?,
                SlotExecutionBinding::NativeFrames { frames } => {
                    if frames.is_empty() {
                        return Err(ServiceError::EmptyNativeFrames(slot.clone()));
                    }
                    let mut numbers = BTreeSet::new();
                    for frame in frames {
                        frame.validate(registry)?;
                        if !numbers.insert(frame.frame_number) {
                            return Err(ServiceError::DuplicateFrameNumber {
                                slot: slot.clone(),
                                frame: frame.frame_number,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderOutputExpectation {
    pub slot: String,
    pub media_type: String,
    pub maximum_size_bytes: u64,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequest {
    pub request_id: String,
    pub artifact_id: String,
    pub provider_id: String,
    pub required_version: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
    #[serde(default)]
    pub input_assets: BTreeMap<String, StagedAssetHandle>,
    pub expected_outputs: Vec<ProviderOutputExpectation>,
}

impl ProviderRequest {
    pub fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        validate_identifier("provider request ID", &self.request_id)?;
        validate_identifier("provider artifact ID", &self.artifact_id)?;
        validate_identifier("provider ID", &self.provider_id)?;
        validate_identifier("provider version", &self.required_version)?;
        for (slot, handle) in &self.input_assets {
            validate_identifier("provider input slot", slot)?;
            registry.resolve(handle)?;
        }
        if self.expected_outputs.is_empty() {
            return Err(ServiceError::EmptyProviderOutputs(self.request_id.clone()));
        }
        let mut slots = BTreeSet::new();
        for output in &self.expected_outputs {
            validate_identifier("provider output slot", &output.slot)?;
            validate_identifier("provider output media type", &output.media_type)?;
            if output.maximum_size_bytes == 0 || !slots.insert(&output.slot) {
                return Err(ServiceError::InvalidProviderOutput(output.slot.clone()));
            }
            if let Some(digest) = &output.expected_sha256 {
                validate_sha256("provider output SHA-256", digest)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResult {
    pub request_id: String,
    pub provider: ToolIdentity,
    pub outputs: BTreeMap<String, ProducedAsset>,
    #[serde(default)]
    pub evidence: Vec<ServiceEvidence>,
}

impl ProviderResult {
    pub fn validate(&self, request: &ProviderRequest) -> Result<(), ServiceError> {
        if self.request_id != request.request_id
            || self.provider.backend_id != request.provider_id
            || self.provider.version != request.required_version
        {
            return Err(ServiceError::ResultIdentityMismatch(
                self.request_id.clone(),
            ));
        }
        self.provider.validate()?;
        let expected = request
            .expected_outputs
            .iter()
            .map(|output| (&output.slot, output))
            .collect::<BTreeMap<_, _>>();
        if self.outputs.len() != expected.len() {
            return Err(ServiceError::ResultOutputMismatch(self.request_id.clone()));
        }
        for (slot, asset) in &self.outputs {
            let expectation = expected
                .get(slot)
                .ok_or_else(|| ServiceError::ResultOutputMismatch(slot.clone()))?;
            asset.validate()?;
            if asset.declaration.size_bytes > expectation.maximum_size_bytes
                || asset.declaration.media_type != expectation.media_type
                || expectation
                    .expected_sha256
                    .as_ref()
                    .is_some_and(|digest| digest != &asset.declaration.sha256)
            {
                return Err(ServiceError::ResultOutputMismatch(slot.clone()));
            }
        }
        validate_evidence(&self.evidence)
    }
}

pub trait ProviderService: Send + Sync {
    fn invoke(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ProviderResult, ServiceError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecRequest {
    pub request_id: String,
    pub artifact_id: String,
    pub slot: String,
    pub backend_id: String,
    pub source_transfer_syntax_uid: String,
    pub target_transfer_syntax_uid: String,
    pub frames: Vec<NativeFrameBinding>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

impl CodecRequest {
    pub fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        validate_identifier("codec request ID", &self.request_id)?;
        validate_identifier("codec artifact ID", &self.artifact_id)?;
        validate_identifier("codec slot", &self.slot)?;
        validate_identifier("codec backend ID", &self.backend_id)?;
        validate_uid(
            "codec source transfer syntax",
            &self.source_transfer_syntax_uid,
        )?;
        validate_uid(
            "codec target transfer syntax",
            &self.target_transfer_syntax_uid,
        )?;
        if self.frames.is_empty() {
            return Err(ServiceError::EmptyNativeFrames(self.slot.clone()));
        }
        let mut frame_numbers = BTreeSet::new();
        for frame in &self.frames {
            frame.validate(registry)?;
            if !frame_numbers.insert(frame.frame_number) {
                return Err(ServiceError::DuplicateFrameNumber {
                    slot: self.slot.clone(),
                    frame: frame.frame_number,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodedFrameResult {
    pub frame_number: u32,
    pub bytes: ByteBinding,
    pub encoded_size_bytes: u64,
    pub encoded_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecResult {
    pub request_id: String,
    pub backend: ToolIdentity,
    pub frames: Vec<EncodedFrameResult>,
    #[serde(default)]
    pub evidence: Vec<ServiceEvidence>,
}

impl CodecResult {
    pub fn validate(
        &self,
        request: &CodecRequest,
        registry: &StagedAssetRegistry,
    ) -> Result<(), ServiceError> {
        if self.request_id != request.request_id || self.backend.backend_id != request.backend_id {
            return Err(ServiceError::ResultIdentityMismatch(
                self.request_id.clone(),
            ));
        }
        self.backend.validate()?;
        let expected_frames = request
            .frames
            .iter()
            .map(|frame| frame.frame_number)
            .collect::<BTreeSet<_>>();
        let actual_frames = self
            .frames
            .iter()
            .map(|frame| frame.frame_number)
            .collect::<BTreeSet<_>>();
        if expected_frames != actual_frames || actual_frames.len() != self.frames.len() {
            return Err(ServiceError::ResultOutputMismatch(self.request_id.clone()));
        }
        for frame in &self.frames {
            validate_sha256("encoded frame SHA-256", &frame.encoded_sha256)?;
            frame.bytes.validate(registry)?;
            let (size, digest) = byte_binding_identity(&frame.bytes, registry)?;
            if size != frame.encoded_size_bytes || digest != frame.encoded_sha256 {
                return Err(ServiceError::ResultOutputMismatch(format!(
                    "{} frame {}",
                    self.request_id, frame.frame_number
                )));
            }
        }
        validate_evidence(&self.evidence)
    }
}

pub trait CodecService: Send + Sync {
    fn encode(
        &self,
        request: &CodecRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<CodecResult, ServiceError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationRequest {
    pub artifact: PlannedArtifact,
    pub bindings: ArtifactExecutionBindings,
}

impl MaterializationRequest {
    pub fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        if self.artifact.logical_id() != self.bindings.artifact_id {
            return Err(ServiceError::ResultIdentityMismatch(
                self.bindings.artifact_id.clone(),
            ));
        }
        self.bindings.validate(registry)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationResult {
    pub artifact_id: String,
    pub output: Option<ProducedAsset>,
    pub backend: ToolIdentity,
    #[serde(default)]
    pub evidence: Vec<ServiceEvidence>,
}

impl MaterializationResult {
    pub fn validate(&self, request: &MaterializationRequest) -> Result<(), ServiceError> {
        if self.artifact_id != request.artifact.logical_id() {
            return Err(ServiceError::ResultIdentityMismatch(
                self.artifact_id.clone(),
            ));
        }
        match (request.artifact.output(), &self.output) {
            (Some(_), Some(output)) => output.validate()?,
            (None, None) => {}
            _ => return Err(ServiceError::ResultOutputMismatch(self.artifact_id.clone())),
        }
        self.backend.validate()?;
        validate_evidence(&self.evidence)
    }
}

pub trait MaterializationService: Send + Sync {
    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Passed,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleExecutionResult {
    pub rule_id: String,
    pub status: ValidationStatus,
    pub message: String,
    #[serde(default)]
    pub measurements: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRequest {
    pub artifact: PlannedArtifact,
    pub materialized_asset: StagedAssetHandle,
    pub plan: ValidationPlan,
}

impl ValidationRequest {
    pub fn validate(&self, registry: &StagedAssetRegistry) -> Result<(), ServiceError> {
        registry.resolve(&self.materialized_asset)?;
        if self.plan.rules.is_empty() {
            return Err(ServiceError::EmptyValidationRules(
                self.artifact.logical_id().into(),
            ));
        }
        let mut rules = BTreeSet::new();
        for rule in &self.plan.rules {
            validate_identifier("validation rule ID", &rule.rule_id)?;
            if !rules.insert(&rule.rule_id) {
                return Err(ServiceError::DuplicateValidationRule(rule.rule_id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationResult {
    pub artifact_id: String,
    pub validator: ToolIdentity,
    pub rules: Vec<RuleExecutionResult>,
    #[serde(default)]
    pub evidence: Vec<ServiceEvidence>,
}

impl ValidationResult {
    pub fn validate(&self, request: &ValidationRequest) -> Result<(), ServiceError> {
        if self.artifact_id != request.artifact.logical_id() {
            return Err(ServiceError::ResultIdentityMismatch(
                self.artifact_id.clone(),
            ));
        }
        self.validator.validate()?;
        let expected = request
            .plan
            .rules
            .iter()
            .map(|rule| rule.rule_id.as_str())
            .collect::<BTreeSet<_>>();
        let actual = self
            .rules
            .iter()
            .map(|rule| rule.rule_id.as_str())
            .collect::<BTreeSet<_>>();
        if expected != actual || actual.len() != self.rules.len() {
            return Err(ServiceError::ResultOutputMismatch(self.artifact_id.clone()));
        }
        for rule in &self.rules {
            validate_identifier("validation result rule ID", &rule.rule_id)?;
            if rule.message.trim().is_empty() {
                return Err(ServiceError::EmptyValidationMessage(rule.rule_id.clone()));
            }
        }
        validate_evidence(&self.evidence)
    }
}

pub trait ValidationService: Send + Sync {
    fn validate(
        &self,
        request: &ValidationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<ValidationResult, ServiceError>;
}

#[derive(Debug)]
pub enum ServiceError {
    InvalidIdentifier {
        label: &'static str,
        value: String,
    },
    UnsafeStagedPath(String),
    DuplicateAssetHandle(StagedAssetHandle),
    DuplicateStagedPath(StagingRelativePath),
    UnknownAssetHandle(StagedAssetHandle),
    AssetSizeMismatch {
        handle: StagedAssetHandle,
        declared: u64,
        observed: u64,
    },
    AssetHashMismatch {
        handle: StagedAssetHandle,
        declared: String,
        observed: String,
    },
    InlineHashMismatch {
        declared: String,
        observed: String,
    },
    AssetRangeOverflow,
    AssetRangeOutOfBounds {
        handle: StagedAssetHandle,
        end: u64,
        size: u64,
    },
    InvalidFrameShape(u32),
    EmptyNativeFrames(String),
    DuplicateFrameNumber {
        slot: String,
        frame: u32,
    },
    EmptyProviderOutputs(String),
    InvalidProviderOutput(String),
    ResultIdentityMismatch(String),
    ResultOutputMismatch(String),
    EmptyValidationRules(String),
    DuplicateValidationRule(String),
    EmptyValidationMessage(String),
    BackendFailure {
        backend_id: String,
        operation: String,
        message: String,
    },
    CapabilityUnavailable {
        capability_id: String,
        reason_code: String,
        message: String,
    },
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for ServiceError {}

fn validate_identifier(label: &'static str, value: &str) -> Result<(), ServiceError> {
    if value.is_empty()
        || value.len() > 256
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(ServiceError::InvalidIdentifier {
            label,
            value: value.into(),
        });
    }
    Ok(())
}

fn validate_sha256(label: &'static str, value: &str) -> Result<(), ServiceError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ServiceError::InvalidIdentifier {
            label,
            value: value.into(),
        });
    }
    Ok(())
}

fn validate_uid(label: &'static str, value: &str) -> Result<(), ServiceError> {
    if value.is_empty()
        || value.len() > 64
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(str::is_empty)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(ServiceError::InvalidIdentifier {
            label,
            value: value.into(),
        });
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), ServiceError> {
    let unsafe_path = value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.contains('\0')
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."));
    let drive_path = value
        .split('/')
        .next()
        .is_some_and(|first| first.len() == 2 && first.as_bytes()[1] == b':');
    if unsafe_path || drive_path {
        return Err(ServiceError::UnsafeStagedPath(value.into()));
    }
    Ok(())
}

fn validate_evidence(evidence: &[ServiceEvidence]) -> Result<(), ServiceError> {
    let mut ids = BTreeSet::new();
    for item in evidence {
        item.validate()?;
        if !ids.insert(&item.evidence_id) {
            return Err(ServiceError::ResultOutputMismatch(item.evidence_id.clone()));
        }
    }
    Ok(())
}

fn byte_binding_identity(
    binding: &ByteBinding,
    registry: &StagedAssetRegistry,
) -> Result<(u64, String), ServiceError> {
    match binding {
        ByteBinding::Inline { bytes, sha256 } => Ok((bytes.len() as u64, sha256.clone())),
        ByteBinding::StagedRange {
            asset,
            offset,
            length,
            sha256,
        } => {
            let declaration = registry.resolve(asset)?;
            if *offset == 0 && *length == declaration.size_bytes {
                Ok((*length, declaration.sha256.clone()))
            } else {
                Ok((*length, sha256.clone()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn produced(handle: &str, path: &str, bytes: &[u8]) -> ProducedAsset {
        ProducedAsset::from_bytes(
            StagedAssetHandle::new(handle).unwrap(),
            StagingRelativePath::new(path).unwrap(),
            "application/octet-stream",
            bytes,
        )
    }

    #[test]
    fn staged_registry_is_path_safe_hash_bound_and_collision_free() {
        for unsafe_path in ["../escape", "/absolute", "a/../../b", "a\\b", "C:/x"] {
            assert!(matches!(
                StagingRelativePath::new(unsafe_path),
                Err(ServiceError::UnsafeStagedPath(_))
            ));
        }

        let mut registry = StagedAssetRegistry::default();
        registry
            .register(produced("asset-1", "assets/one.bin", b"one"))
            .unwrap();
        assert!(matches!(
            registry.register(produced("asset-1", "assets/two.bin", b"two")),
            Err(ServiceError::DuplicateAssetHandle(_))
        ));
        assert!(matches!(
            registry.register(produced("asset-2", "assets/one.bin", b"two")),
            Err(ServiceError::DuplicateStagedPath(_))
        ));

        let mut mismatched = produced("asset-3", "assets/three.bin", b"three");
        mismatched.observed_size_bytes += 1;
        assert!(matches!(
            registry.register(mismatched),
            Err(ServiceError::AssetSizeMismatch { .. })
        ));
        let mut mismatched = produced("asset-4", "assets/four.bin", b"four");
        mismatched.observed_sha256 = "0".repeat(64);
        assert!(matches!(
            registry.register(mismatched),
            Err(ServiceError::AssetHashMismatch { .. })
        ));
    }

    #[test]
    fn bindings_resolve_only_registered_assets_and_bounded_native_frames() {
        let bytes = b"frame";
        let mut registry = StagedAssetRegistry::default();
        registry
            .register(produced("frame-1", "frames/one.raw", bytes))
            .unwrap();
        let binding = ArtifactExecutionBindings {
            artifact_id: "artifact".into(),
            slots: BTreeMap::from([(
                "pixels".into(),
                SlotExecutionBinding::NativeFrames {
                    frames: vec![NativeFrameBinding {
                        frame_number: 1,
                        bytes: ByteBinding::StagedRange {
                            asset: StagedAssetHandle::new("frame-1").unwrap(),
                            offset: 0,
                            length: bytes.len() as u64,
                            sha256: sha256_hex(bytes),
                        },
                        rows: 1,
                        columns: 5,
                        samples_per_pixel: 1,
                        bits_allocated: 8,
                        photometric_interpretation: "MONOCHROME2".into(),
                    }],
                },
            )]),
        };
        binding.validate(&registry).unwrap();

        let mut invalid = binding;
        let SlotExecutionBinding::NativeFrames { frames } =
            invalid.slots.get_mut("pixels").unwrap()
        else {
            unreachable!()
        };
        let ByteBinding::StagedRange { length, .. } = &mut frames[0].bytes else {
            unreachable!()
        };
        *length += 1;
        assert!(matches!(
            invalid.validate(&registry),
            Err(ServiceError::AssetRangeOutOfBounds { .. })
        ));
    }

    #[test]
    fn service_contracts_are_injectable_and_carry_backend_evidence() {
        struct FakeProvider;
        impl ProviderService for FakeProvider {
            fn invoke(
                &self,
                request: &ProviderRequest,
                assets: &StagedAssetRegistry,
            ) -> Result<ProviderResult, ServiceError> {
                request.validate(assets)?;
                let producer = ToolIdentity {
                    backend_id: request.provider_id.clone(),
                    version: request.required_version.clone(),
                    protocol_version: Some("1".into()),
                    executable_sha256: Some("a".repeat(64)),
                };
                producer.validate()?;
                Ok(ProviderResult {
                    request_id: request.request_id.clone(),
                    provider: producer.clone(),
                    outputs: BTreeMap::new(),
                    evidence: vec![ServiceEvidence {
                        evidence_id: "provider_invocation".into(),
                        evidence_kind: "tool_identity".into(),
                        producer,
                        claims: BTreeMap::new(),
                    }],
                })
            }
        }

        let request = ProviderRequest {
            request_id: "request-1".into(),
            artifact_id: "artifact".into(),
            provider_id: "fixture_provider".into(),
            required_version: "1.0.0".into(),
            parameters: BTreeMap::new(),
            input_assets: BTreeMap::new(),
            expected_outputs: vec![ProviderOutputExpectation {
                slot: "pixels".into(),
                media_type: "application/octet-stream".into(),
                maximum_size_bytes: 1024,
                expected_sha256: None,
            }],
        };
        let result = FakeProvider
            .invoke(&request, &StagedAssetRegistry::default())
            .unwrap();
        assert_eq!(result.provider.backend_id, "fixture_provider");
        assert_eq!(result.evidence[0].producer, result.provider);
    }

    #[test]
    fn every_execution_service_is_object_safe_and_injectable() {
        struct Unavailable;
        fn unavailable() -> ServiceError {
            ServiceError::CapabilityUnavailable {
                capability_id: "test".into(),
                reason_code: "not_configured".into(),
                message: "test service is not configured".into(),
            }
        }
        impl MaterializationService for Unavailable {
            fn materialize(
                &self,
                _: &MaterializationRequest,
                _: &StagedAssetRegistry,
            ) -> Result<MaterializationResult, ServiceError> {
                Err(unavailable())
            }
        }
        impl ProviderService for Unavailable {
            fn invoke(
                &self,
                _: &ProviderRequest,
                _: &StagedAssetRegistry,
            ) -> Result<ProviderResult, ServiceError> {
                Err(unavailable())
            }
        }
        impl CodecService for Unavailable {
            fn encode(
                &self,
                _: &CodecRequest,
                _: &StagedAssetRegistry,
            ) -> Result<CodecResult, ServiceError> {
                Err(unavailable())
            }
        }
        impl ValidationService for Unavailable {
            fn validate(
                &self,
                _: &ValidationRequest,
                _: &StagedAssetRegistry,
            ) -> Result<ValidationResult, ServiceError> {
                Err(unavailable())
            }
        }

        let service = Unavailable;
        let _: &dyn MaterializationService = &service;
        let _: &dyn ProviderService = &service;
        let _: &dyn CodecService = &service;
        let _: &dyn ValidationService = &service;
    }

    #[test]
    fn service_layer_has_no_frontend_dependencies() {
        let source = include_str!("services.rs");
        let forbidden = [
            ["crate::", "composition"].concat(),
            ["crate::", "generator"].concat(),
            ["Compose", "Options"].concat(),
        ];
        for forbidden in forbidden {
            assert!(
                !source.contains(&forbidden),
                "forbidden dependency {forbidden}"
            );
        }
    }
}
