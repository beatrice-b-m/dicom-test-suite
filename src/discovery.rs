use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::engine_resources::{EngineResourceError, EngineResourceIdentity, EngineResources};
use crate::identity::{
    IdentityInspectionContext, IdentityProjectionError, InstalledIdentityDomains,
    project_installed_identities,
};

pub const VERSION_RESULT_SCHEMA_VERSION: &str = "2.0.0";
pub const CAPABILITIES_RESULT_SCHEMA_VERSION: &str = "2.0.0";
pub const SUPPORTED_VERSION_RESULT_SCHEMA_VERSIONS: &[&str] = &["1.0.0", "2.0.0"];
pub const SUPPORTED_CAPABILITIES_RESULT_SCHEMA_VERSIONS: &[&str] = &["1.0.0", "2.0.0"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProductIdentity {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionResult {
    pub version_result_schema_version: &'static str,
    pub product: ProductIdentity,
    pub cli_api_version: &'static str,
    pub target: &'static str,
    pub rust_toolchain: &'static str,
    pub enabled_features: Vec<&'static str>,
    pub product_resources: EngineResourceIdentity,
    pub identity_domains: InstalledIdentityDomains,
}

pub fn version_result(resources: &EngineResources) -> Result<VersionResult, DiscoveryError> {
    version_result_with_context(resources, IdentityInspectionContext::default())
}

pub fn version_result_with_context(
    resources: &EngineResources,
    context: IdentityInspectionContext<'_>,
) -> Result<VersionResult, DiscoveryError> {
    Ok(VersionResult {
        version_result_schema_version: VERSION_RESULT_SCHEMA_VERSION,
        product: ProductIdentity {
            name: crate::PACKAGE_NAME,
            version: crate::PACKAGE_VERSION,
        },
        cli_api_version: crate::cli_protocol::CLI_API_VERSION,
        target: crate::TARGET_TRIPLE,
        rust_toolchain: crate::RUSTC_VERSION,
        enabled_features: crate::ACTIVE_FEATURE_FLAGS.to_vec(),
        product_resources: resources.verify_integrity()?,
        identity_domains: project_installed_identities(resources, context)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilitiesResult {
    pub capabilities_result_schema_version: &'static str,
    pub product_version: &'static str,
    pub cli_api_version: &'static str,
    pub enabled_features: Vec<&'static str>,
    pub product_resources: EngineResourceIdentity,
    pub identity_domains: InstalledIdentityDomains,
    pub supported_versions: SupportedVersions,
    pub qualified_templates: Vec<QualifiedTemplateCapability>,
    pub transfer_syntaxes: Vec<TransferSyntaxCapability>,
    pub optional_runtimes: Vec<OptionalRuntimeCapability>,
    pub resource_ceilings: crate::composition::ResourceLimits,
    pub assembly_resource_ceilings: crate::assembly::AssemblyLimits,
    pub structural_assembly: WorkflowCapability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupportedVersions {
    pub cli_api: Vec<&'static str>,
    /// Versions emitted by the current producer for each result family.
    pub result_schemas: BTreeMap<&'static str, Vec<&'static str>>,
    /// Versions covered by immutable JSON Schema consumer fixtures.
    ///
    /// This is validation compatibility, not a producer selector or a claim
    /// that a typed Rust reader normalizes legacy documents.
    pub result_schema_validation: BTreeMap<&'static str, Vec<&'static str>>,
    pub composition_request: Vec<&'static str>,
    pub assembly_request: Vec<&'static str>,
    pub assembly_manifest: Vec<&'static str>,
    pub release_manifest: Vec<&'static str>,
    pub curated_manifest: Vec<&'static str>,
    /// Curated manifest versions covered by validate/report compatibility fixtures.
    pub curated_manifest_validation: Vec<&'static str>,
    pub composition_manifest: Vec<&'static str>,
    pub coverage_report: Vec<&'static str>,
    pub template_catalog: Vec<String>,
    pub case_registry: Vec<&'static str>,
    pub composition_provider_protocol: Vec<&'static str>,
    pub generation_backend_protocol: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualifiedTemplateCapability {
    pub template_id: String,
    pub template_version: String,
    pub qualification_status: &'static str,
    pub artifact_kind: String,
    pub transfer_syntax_uids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnavailableReason {
    pub code: &'static str,
    pub capability_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransferSyntaxCapability {
    pub uid: String,
    pub keyword: String,
    pub name: String,
    pub determinism: String,
    pub write_dataset: bool,
    pub encode_pixel: bool,
    pub declared_status: String,
    pub availability: &'static str,
    pub required_features: Vec<String>,
    pub unavailable_reasons: Vec<UnavailableReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OptionalRuntimeCapability {
    pub runtime_id: String,
    pub kind: &'static str,
    pub executable: Option<String>,
    pub environment_override: Option<String>,
    pub required_by_default: bool,
    pub availability: &'static str,
    pub reason_code: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowCapability {
    pub availability: &'static str,
    pub reason_code: Option<&'static str>,
    pub supported_content_kinds: Vec<&'static str>,
    pub supported_transfer_syntax_uids: Vec<&'static str>,
}

#[derive(Debug)]
pub enum DiscoveryError {
    Resources(EngineResourceError),
    Identity(IdentityProjectionError),
    TemplateCatalog(String),
    ResourceDocument {
        logical_path: &'static str,
        message: String,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resources(error) => write!(formatter, "product resources: {error}"),
            Self::Identity(error) => write!(formatter, "identity projection: {error}"),
            Self::TemplateCatalog(message) => write!(formatter, "template catalog: {message}"),
            Self::ResourceDocument {
                logical_path,
                message,
            } => write!(
                formatter,
                "invalid product resource {logical_path}: {message}"
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<EngineResourceError> for DiscoveryError {
    fn from(value: EngineResourceError) -> Self {
        Self::Resources(value)
    }
}

impl From<IdentityProjectionError> for DiscoveryError {
    fn from(value: IdentityProjectionError) -> Self {
        Self::Identity(value)
    }
}

#[derive(Debug, Deserialize)]
struct TransferSyntaxMatrix {
    entries: Vec<TransferSyntaxEntry>,
}

#[derive(Debug, Deserialize)]
struct TransferSyntaxEntry {
    uid: String,
    keyword: String,
    name: String,
    status: String,
    write_dataset: bool,
    encode_pixel: bool,
    feature_flags: Vec<String>,
    determinism: String,
}

#[derive(Debug, Deserialize)]
struct BackendLock {
    protocol_version: String,
    backends: Vec<BackendDeclaration>,
}

#[derive(Debug, Deserialize)]
struct BackendDeclaration {
    backend_id: String,
    state: String,
    implementation_kind: String,
    required_by_default: bool,
    discovery: Option<BackendDiscovery>,
}

#[derive(Debug, Deserialize)]
struct BackendDiscovery {
    executable: String,
    environment_override: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidatorConfiguration {
    adapters: Vec<ValidatorDeclaration>,
}

#[derive(Debug, Deserialize)]
struct ValidatorDeclaration {
    id: String,
    executable: String,
    executable_env: Option<String>,
    required: bool,
}

fn parse_resource<T: for<'de> Deserialize<'de>>(
    resources: &EngineResources,
    logical_path: &'static str,
) -> Result<T, DiscoveryError> {
    let bytes = resources.bytes(logical_path)?;
    serde_json::from_slice(&bytes).map_err(|error| DiscoveryError::ResourceDocument {
        logical_path,
        message: error.to_string(),
    })
}

pub fn capabilities_result(
    resources: &EngineResources,
) -> Result<CapabilitiesResult, DiscoveryError> {
    capabilities_result_with_context(resources, IdentityInspectionContext::default())
}

pub fn capabilities_result_with_context(
    resources: &EngineResources,
    context: IdentityInspectionContext<'_>,
) -> Result<CapabilitiesResult, DiscoveryError> {
    let product_resources = resources.verify_integrity()?;
    let identity_domains = project_installed_identities(resources, context)?;
    let snapshot = resources.snapshot()?;
    let catalog =
        crate::composition::TemplateCatalog::load(snapshot.root().join("templates/catalog.json"))
            .map_err(|error| DiscoveryError::TemplateCatalog(error.to_string()))?;
    let transfer_matrix: TransferSyntaxMatrix =
        parse_resource(resources, "transfer-syntax/capability-matrix.json")?;
    let backend_lock: BackendLock = parse_resource(resources, "generation-backends.lock.json")?;
    let validator_configuration: ValidatorConfiguration =
        parse_resource(resources, "conformance/validators.json")?;

    let enabled = crate::ACTIVE_FEATURE_FLAGS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut qualified_templates = catalog
        .templates
        .iter()
        .filter(|template| template.status == crate::composition::TemplateStatus::Qualified)
        .map(|template| QualifiedTemplateCapability {
            template_id: template.template_id.to_string(),
            template_version: template.template_version.to_string(),
            qualification_status: "qualified",
            artifact_kind: template.artifact_kind.clone(),
            transfer_syntax_uids: template
                .transfer_syntaxes
                .iter()
                .map(|syntax| syntax.uid.clone())
                .collect(),
        })
        .collect::<Vec<_>>();
    qualified_templates.sort_by(|left, right| {
        (&left.template_id, &left.template_version)
            .cmp(&(&right.template_id, &right.template_version))
    });

    let transfer_syntaxes = transfer_matrix
        .entries
        .into_iter()
        .map(|entry| {
            let mut unavailable_reasons = entry
                .feature_flags
                .iter()
                .filter(|feature| !enabled.contains(feature.as_str()))
                .map(|feature| UnavailableReason {
                    code: "capability.feature.unavailable",
                    capability_id: feature.clone(),
                })
                .collect::<Vec<_>>();
            if entry.status == "unavailable" || !entry.write_dataset || !entry.encode_pixel {
                unavailable_reasons.push(UnavailableReason {
                    code: "capability.transfer_syntax.unavailable",
                    capability_id: entry.uid.clone(),
                });
            }
            unavailable_reasons.sort_by(|left, right| {
                (left.code, &left.capability_id).cmp(&(right.code, &right.capability_id))
            });
            TransferSyntaxCapability {
                uid: entry.uid,
                keyword: entry.keyword,
                name: entry.name,
                determinism: entry.determinism,
                write_dataset: entry.write_dataset,
                encode_pixel: entry.encode_pixel,
                declared_status: entry.status,
                availability: if unavailable_reasons.is_empty() {
                    "available"
                } else {
                    "unavailable"
                },
                required_features: entry.feature_flags,
                unavailable_reasons,
            }
        })
        .collect();

    let mut optional_runtimes = backend_lock
        .backends
        .into_iter()
        .filter(|backend| backend.implementation_kind != "rust_native")
        .map(|backend| {
            let supported = backend.state == "available";
            OptionalRuntimeCapability {
                runtime_id: backend.backend_id,
                kind: "generation_backend",
                executable: backend
                    .discovery
                    .as_ref()
                    .map(|discovery| discovery.executable.clone()),
                environment_override: backend
                    .discovery
                    .and_then(|discovery| discovery.environment_override),
                required_by_default: backend.required_by_default,
                availability: if supported {
                    "requires_explicit_configuration"
                } else {
                    "unavailable"
                },
                reason_code: Some("capability.runtime.unavailable"),
            }
        })
        .chain(validator_configuration.adapters.into_iter().map(|adapter| {
            OptionalRuntimeCapability {
                runtime_id: adapter.id,
                kind: "validator",
                executable: Some(adapter.executable),
                environment_override: adapter.executable_env,
                required_by_default: adapter.required,
                availability: "requires_explicit_configuration",
                reason_code: Some("capability.validator.unavailable"),
            }
        }))
        .collect::<Vec<_>>();
    optional_runtimes
        .sort_by(|left, right| (left.kind, &left.runtime_id).cmp(&(right.kind, &right.runtime_id)));
    optional_runtimes
        .dedup_by(|left, right| left.kind == right.kind && left.runtime_id == right.runtime_id);

    Ok(CapabilitiesResult {
        capabilities_result_schema_version: CAPABILITIES_RESULT_SCHEMA_VERSION,
        product_version: crate::PACKAGE_VERSION,
        cli_api_version: crate::cli_protocol::CLI_API_VERSION,
        enabled_features: enabled.into_iter().collect(),
        product_resources,
        identity_domains,
        supported_versions: SupportedVersions {
            cli_api: vec![crate::cli_protocol::CLI_API_VERSION],
            result_schemas: BTreeMap::from([
                (
                    "assembly",
                    vec![crate::cli_protocol::ASSEMBLY_RESULT_SCHEMA_VERSION],
                ),
                ("capabilities", vec![CAPABILITIES_RESULT_SCHEMA_VERSION]),
                (
                    "case_list",
                    vec![crate::cli_protocol::CASE_LIST_RESULT_SCHEMA_VERSION],
                ),
                (
                    "composition",
                    vec![crate::cli_protocol::COMPOSITION_RESULT_SCHEMA_VERSION],
                ),
                (
                    "conformance",
                    vec![crate::cli_protocol::CONFORMANCE_RESULT_SCHEMA_VERSION],
                ),
                (
                    "generation",
                    vec![crate::cli_protocol::GENERATION_RESULT_SCHEMA_VERSION],
                ),
                (
                    "interoperability",
                    vec![crate::cli_protocol::INTEROPERABILITY_RESULT_SCHEMA_VERSION],
                ),
                (
                    "report",
                    vec![crate::cli_protocol::REPORT_RESULT_SCHEMA_VERSION],
                ),
                (
                    "standards",
                    vec![crate::cli_protocol::STANDARDS_RESULT_SCHEMA_VERSION],
                ),
                (
                    "templates",
                    vec![crate::cli_protocol::TEMPLATES_RESULT_SCHEMA_VERSION],
                ),
                (
                    "validation",
                    vec![crate::cli_protocol::VALIDATION_RESULT_SCHEMA_VERSION],
                ),
                ("version", vec![VERSION_RESULT_SCHEMA_VERSION]),
            ]),
            result_schema_validation: BTreeMap::from([
                (
                    "capabilities",
                    SUPPORTED_CAPABILITIES_RESULT_SCHEMA_VERSIONS.to_vec(),
                ),
                ("version", SUPPORTED_VERSION_RESULT_SCHEMA_VERSIONS.to_vec()),
                ("generation", vec!["1.0.0", "2.0.0"]),
                ("composition", vec!["1.0.0", "2.0.0"]),
            ]),
            composition_request: vec!["0.1.0"],
            assembly_request: vec![crate::assembly::ASSEMBLY_REQUEST_SCHEMA_VERSION],
            assembly_manifest: vec![crate::assembly::ASSEMBLY_MANIFEST_SCHEMA_VERSION],
            release_manifest: vec!["1.0.0"],
            curated_manifest: vec!["1.0.0"],
            curated_manifest_validation: vec!["0.2.0", "0.3.0", "1.0.0"],
            composition_manifest: vec!["1.0.0"],
            coverage_report: vec!["0.1.0"],
            template_catalog: vec![catalog.template_catalog_schema_version],
            case_registry: vec!["0.2.0"],
            composition_provider_protocol: vec!["1.0.0"],
            generation_backend_protocol: vec![backend_lock.protocol_version],
        },
        qualified_templates,
        transfer_syntaxes,
        optional_runtimes,
        resource_ceilings: crate::composition::ResourceLimits::default(),
        assembly_resource_ceilings: crate::assembly::AssemblyLimits::default(),
        structural_assembly: WorkflowCapability {
            availability: "available",
            reason_code: None,
            supported_content_kinds: vec![
                "standard_elements",
                "unknown_explicit_vr_elements",
                "managed_private_elements",
                "recursive_sequences",
                "integer_pixel_data",
                "float_pixel_data",
                "double_float_pixel_data",
                "waveform_data",
                "encapsulated_document",
                "mesh",
                "general_bulk",
            ],
            supported_transfer_syntax_uids: vec!["1.2.840.10008.1.2", "1.2.840.10008.1.2.1"],
        },
    })
}
