//! Independent discovery identity domains.
//!
//! R4.3 projects these identities alongside the locked transitional resource
//! identity. Generated manifest contracts remain unchanged until their owning
//! sequential slices migrate them.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

use crate::corpus_definition::{CorpusDefinitionBundle, CorpusDefinitionIdentity};
use crate::engine_resources::{
    EngineResourceError, EngineResourceIdentity, EngineResourceOrigin, EngineResources,
};

pub const IDENTITY_DOMAINS_SCHEMA_VERSION: &str = "1.0.0";
pub const IDENTITY_DOMAIN_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineIdentity {
    pub identity_version: &'static str,
    pub engine_sha256: String,
    pub member_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaSetIdentity {
    pub identity_version: &'static str,
    pub schema_set_sha256: String,
    pub member_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplateCatalogIdentity {
    pub identity_version: &'static str,
    pub template_catalog_sha256: String,
    pub member_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderCatalogIdentity {
    pub identity_version: &'static str,
    pub provider_catalog_sha256: String,
    pub member_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolchainIdentity {
    pub identity_version: &'static str,
    pub rust_toolchain: &'static str,
    pub target: &'static str,
    pub enabled_features: Vec<&'static str>,
    pub cargo_lock_sha256: String,
    pub toolchain_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandardsIdentity {
    pub identity_version: &'static str,
    pub standards_lock_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionIdentity {
    pub identity_version: &'static str,
    pub product_name: &'static str,
    pub product_version: &'static str,
    pub cli_api_version: &'static str,
    pub target: &'static str,
    pub enabled_features: Vec<&'static str>,
    pub execution_sha256: String,
}

/// A runtime identity is evidence from one actual invocation. Capability
/// declarations and environment-variable names are intentionally not runtime
/// fingerprints and therefore never populate this type by themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalRuntimeIdentity {
    pub runtime_id: String,
    pub runtime_kind: String,
    pub executable_sha256: String,
    pub version: String,
    pub invocation_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityMigrationContext {
    pub source: &'static str,
    pub legacy_resource_set_version: String,
    pub legacy_resource_origin: EngineResourceOrigin,
    pub legacy_resource_count: usize,
    pub legacy_resource_set_sha256: String,
    pub corpus_identity_status: &'static str,
    pub manifest_projection_status: &'static str,
    pub removal_phase: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledIdentityDomains {
    pub identity_domains_schema_version: &'static str,
    pub engine: EngineIdentity,
    pub schema_set: SchemaSetIdentity,
    pub template_catalog: TemplateCatalogIdentity,
    pub provider_catalog: ProviderCatalogIdentity,
    pub corpus_definition: Option<CorpusDefinitionIdentity>,
    pub toolchain: ToolchainIdentity,
    pub external_runtime: Vec<ExternalRuntimeIdentity>,
    pub standards: StandardsIdentity,
    pub execution: ExecutionIdentity,
    pub migration: IdentityMigrationContext,
}

pub const MANIFEST_IDENTITY_PROJECTION_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestIdentityProjectionState {
    Projected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusDefinitionProjectionState {
    VerifiedBundle,
    TransitionalEmbeddedUnverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestCorpusDefinitionIdentity {
    pub state: CorpusDefinitionProjectionState,
    pub identity: Option<CorpusDefinitionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyResourceProvenance {
    pub resource_set_version: String,
    pub origin: EngineResourceOrigin,
    pub resource_count: usize,
    pub resource_set_sha256: String,
    pub removal_phase: &'static str,
}

/// Identity state projected into a curated-generation manifest.
///
/// Unlike discovery migration context, this type records that projection has
/// happened. It cannot serialize the discovery-only deferred marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CuratedManifestIdentityProjection {
    pub identity_projection_schema_version: &'static str,
    pub projection_state: ManifestIdentityProjectionState,
    pub engine: EngineIdentity,
    pub schema_set: SchemaSetIdentity,
    pub template_catalog: TemplateCatalogIdentity,
    pub provider_catalog: ProviderCatalogIdentity,
    pub corpus_definition: ManifestCorpusDefinitionIdentity,
    pub toolchain: ToolchainIdentity,
    pub external_runtime: Vec<ExternalRuntimeIdentity>,
    pub standards: StandardsIdentity,
    pub execution: ExecutionIdentity,
    pub legacy_provenance: LegacyResourceProvenance,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityInspectionContext<'a> {
    pub corpus_definition: Option<&'a CorpusDefinitionBundle>,
}

#[derive(Debug)]
pub enum IdentityProjectionError {
    Resources(EngineResourceError),
    MissingMember(&'static str),
    UnclassifiedMember(String),
    InvalidExternalRuntime(String),
}

impl fmt::Display for IdentityProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resources(error) => write!(formatter, "engine resources: {error}"),
            Self::MissingMember(path) => {
                write!(
                    formatter,
                    "identity projection is missing required member {path}"
                )
            }
            Self::UnclassifiedMember(path) => {
                write!(
                    formatter,
                    "identity projection has unclassified member {path}"
                )
            }
            Self::InvalidExternalRuntime(message) => {
                write!(formatter, "invalid external runtime identity: {message}")
            }
        }
    }
}

pub(crate) fn project_curated_manifest_identities(
    resources: &EngineResources,
    corpus_definition: Option<&CorpusDefinitionBundle>,
    mut external_runtime: Vec<ExternalRuntimeIdentity>,
) -> Result<CuratedManifestIdentityProjection, IdentityProjectionError> {
    validate_external_runtime_identities(&mut external_runtime)?;
    let installed =
        project_installed_identities(resources, IdentityInspectionContext { corpus_definition })?;
    let corpus_definition = ManifestCorpusDefinitionIdentity {
        state: if installed.corpus_definition.is_some() {
            CorpusDefinitionProjectionState::VerifiedBundle
        } else {
            CorpusDefinitionProjectionState::TransitionalEmbeddedUnverified
        },
        identity: installed.corpus_definition,
    };
    Ok(CuratedManifestIdentityProjection {
        identity_projection_schema_version: MANIFEST_IDENTITY_PROJECTION_SCHEMA_VERSION,
        projection_state: ManifestIdentityProjectionState::Projected,
        engine: installed.engine,
        schema_set: installed.schema_set,
        template_catalog: installed.template_catalog,
        provider_catalog: installed.provider_catalog,
        corpus_definition,
        toolchain: installed.toolchain,
        external_runtime,
        standards: installed.standards,
        execution: installed.execution,
        legacy_provenance: LegacyResourceProvenance {
            resource_set_version: installed.migration.legacy_resource_set_version,
            origin: installed.migration.legacy_resource_origin,
            resource_count: installed.migration.legacy_resource_count,
            resource_set_sha256: installed.migration.legacy_resource_set_sha256,
            removal_phase: installed.migration.removal_phase,
        },
    })
}

fn validate_external_runtime_identities(
    identities: &mut Vec<ExternalRuntimeIdentity>,
) -> Result<(), IdentityProjectionError> {
    for identity in identities.iter() {
        if identity.runtime_id.trim().is_empty()
            || identity.runtime_kind.trim().is_empty()
            || identity.version.trim().is_empty()
        {
            return Err(IdentityProjectionError::InvalidExternalRuntime(
                "runtime_id, runtime_kind, and version must be nonempty".into(),
            ));
        }
        for (field, digest) in [
            ("executable_sha256", &identity.executable_sha256),
            ("invocation_sha256", &identity.invocation_sha256),
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(IdentityProjectionError::InvalidExternalRuntime(format!(
                    "{} has invalid {field}",
                    identity.runtime_id
                )));
            }
        }
    }
    identities.sort_by(|left, right| {
        (
            &left.runtime_id,
            &left.runtime_kind,
            &left.executable_sha256,
            &left.version,
            &left.invocation_sha256,
        )
            .cmp(&(
                &right.runtime_id,
                &right.runtime_kind,
                &right.executable_sha256,
                &right.version,
                &right.invocation_sha256,
            ))
    });
    if identities
        .windows(2)
        .any(|pair| pair[0].runtime_id == pair[1].runtime_id)
    {
        return Err(IdentityProjectionError::InvalidExternalRuntime(
            "runtime_id values must be unique".into(),
        ));
    }
    Ok(())
}

impl std::error::Error for IdentityProjectionError {}

impl From<EngineResourceError> for IdentityProjectionError {
    fn from(value: EngineResourceError) -> Self {
        Self::Resources(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum InstalledMemberDomain {
    Engine,
    SchemaSet,
    TemplateCatalog,
    ProviderCatalog,
    TransitionalCorpus,
    Toolchain,
    Standards,
}

struct Member {
    logical_path: String,
    bytes: Vec<u8>,
}

const DIRECT_SCHEMA_MEMBERS: &[(&str, &[u8])] = &[
    (
        "schemas/capabilities-result-v2.schema.json",
        include_bytes!("../schemas/capabilities-result-v2.schema.json"),
    ),
    (
        "schemas/corpus-definition-bundle.schema.json",
        include_bytes!("../schemas/corpus-definition-bundle.schema.json"),
    ),
    (
        "schemas/generation-result-v2.schema.json",
        include_bytes!("../schemas/generation-result-v2.schema.json"),
    ),
    (
        "schemas/manifest-v1.schema.json",
        include_bytes!("../schemas/manifest-v1.schema.json"),
    ),
    (
        "schemas/version-result-v2.schema.json",
        include_bytes!("../schemas/version-result-v2.schema.json"),
    ),
];

pub fn project_installed_identities(
    resources: &EngineResources,
    context: IdentityInspectionContext<'_>,
) -> Result<InstalledIdentityDomains, IdentityProjectionError> {
    let legacy = resources.verify_integrity()?;
    let mut members = BTreeMap::<InstalledMemberDomain, Vec<Member>>::new();
    for logical_path in resources.logical_paths() {
        let bytes = resources.bytes(logical_path)?;
        members
            .entry(classify_member(logical_path)?)
            .or_default()
            .push(Member {
                logical_path: logical_path.to_string(),
                bytes: bytes.into_owned(),
            });
    }
    project_members(members, context, legacy)
}

fn project_members(
    mut members: BTreeMap<InstalledMemberDomain, Vec<Member>>,
    context: IdentityInspectionContext<'_>,
    legacy: EngineResourceIdentity,
) -> Result<InstalledIdentityDomains, IdentityProjectionError> {
    members
        .entry(InstalledMemberDomain::SchemaSet)
        .or_default()
        .extend(DIRECT_SCHEMA_MEMBERS.iter().map(|(path, bytes)| Member {
            logical_path: (*path).to_string(),
            bytes: bytes.to_vec(),
        }));
    for records in members.values_mut() {
        records.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    }

    let engine = digest_domain(
        "engine",
        take_domain(&mut members, InstalledMemberDomain::Engine)?,
    )?;
    let schemas = digest_domain(
        "schema_set",
        take_domain(&mut members, InstalledMemberDomain::SchemaSet)?,
    )?;
    let templates = digest_domain(
        "template_catalog",
        take_domain(&mut members, InstalledMemberDomain::TemplateCatalog)?,
    )?;
    let providers = digest_domain(
        "provider_catalog",
        take_domain(&mut members, InstalledMemberDomain::ProviderCatalog)?,
    )?;
    // The temporary embedded corpus remains in the legacy resource oracle but
    // is intentionally not projected as CorpusDefinitionIdentity. Only a
    // verified R4.2 bundle may populate that public identity domain.
    let _transitional_corpus =
        take_domain(&mut members, InstalledMemberDomain::TransitionalCorpus)?;
    let toolchain_members = take_domain(&mut members, InstalledMemberDomain::Toolchain)?;
    let standards_members = take_domain(&mut members, InstalledMemberDomain::Standards)?;
    debug_assert!(
        members.is_empty(),
        "every installed member is classified once"
    );

    let cargo_lock = toolchain_members
        .iter()
        .find(|member| member.logical_path == "Cargo.lock")
        .ok_or(IdentityProjectionError::MissingMember("Cargo.lock"))?;
    let standards_lock = standards_members
        .iter()
        .find(|member| member.logical_path == "standards.lock.json")
        .ok_or(IdentityProjectionError::MissingMember(
            "standards.lock.json",
        ))?;
    let enabled_features = crate::ACTIVE_FEATURE_FLAGS.to_vec();
    let toolchain_sha256 = crate::sha256_hex(
        format!(
            "identity_version={IDENTITY_DOMAIN_VERSION}\nrust_toolchain={}\ntarget={}\nenabled_features={}\ncargo_lock_sha256={}\n",
            crate::RUSTC_VERSION,
            crate::TARGET_TRIPLE,
            enabled_features.join(","),
            crate::sha256_hex(&cargo_lock.bytes),
        )
        .as_bytes(),
    );
    let execution_sha256 = crate::sha256_hex(
        format!(
            "identity_version={IDENTITY_DOMAIN_VERSION}\nproduct={}:{}\ncli_api={}\ntarget={}\nenabled_features={}\n",
            crate::PACKAGE_NAME,
            crate::PACKAGE_VERSION,
            crate::cli_protocol::CLI_API_VERSION,
            crate::TARGET_TRIPLE,
            enabled_features.join(","),
        )
        .as_bytes(),
    );

    Ok(InstalledIdentityDomains {
        identity_domains_schema_version: IDENTITY_DOMAINS_SCHEMA_VERSION,
        engine: EngineIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            engine_sha256: engine.sha256,
            member_count: engine.member_count,
            total_size_bytes: engine.total_size_bytes,
        },
        schema_set: SchemaSetIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            schema_set_sha256: schemas.sha256,
            member_count: schemas.member_count,
            total_size_bytes: schemas.total_size_bytes,
        },
        template_catalog: TemplateCatalogIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            template_catalog_sha256: templates.sha256,
            member_count: templates.member_count,
            total_size_bytes: templates.total_size_bytes,
        },
        provider_catalog: ProviderCatalogIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            provider_catalog_sha256: providers.sha256,
            member_count: providers.member_count,
            total_size_bytes: providers.total_size_bytes,
        },
        corpus_definition: context
            .corpus_definition
            .map(|bundle| bundle.identity().clone()),
        toolchain: ToolchainIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            rust_toolchain: crate::RUSTC_VERSION,
            target: crate::TARGET_TRIPLE,
            enabled_features: enabled_features.clone(),
            cargo_lock_sha256: crate::sha256_hex(&cargo_lock.bytes),
            toolchain_sha256,
        },
        external_runtime: Vec::new(),
        standards: StandardsIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            standards_lock_sha256: crate::sha256_hex(&standards_lock.bytes),
        },
        execution: ExecutionIdentity {
            identity_version: IDENTITY_DOMAIN_VERSION,
            product_name: crate::PACKAGE_NAME,
            product_version: crate::PACKAGE_VERSION,
            cli_api_version: crate::cli_protocol::CLI_API_VERSION,
            target: crate::TARGET_TRIPLE,
            enabled_features,
            execution_sha256,
        },
        migration: IdentityMigrationContext {
            source: "transitional_monolithic_resource_set_v1",
            legacy_resource_set_version: legacy.resource_set_version,
            legacy_resource_origin: legacy.origin,
            legacy_resource_count: legacy.resource_count,
            legacy_resource_set_sha256: legacy.resource_set_sha256,
            corpus_identity_status: if context.corpus_definition.is_some() {
                "verified_bundle_loaded"
            } else {
                "absent_without_verified_bundle"
            },
            manifest_projection_status: "deferred_to_sequential_r4_3_slices",
            removal_phase: "R4.4",
        },
    })
}

fn classify_member(path: &str) -> Result<InstalledMemberDomain, IdentityProjectionError> {
    if path == "Cargo.lock" {
        Ok(InstalledMemberDomain::Toolchain)
    } else if path == "standards.lock.json" {
        Ok(InstalledMemberDomain::Standards)
    } else if path.starts_with("cases/") {
        Ok(InstalledMemberDomain::TransitionalCorpus)
    } else if path.starts_with("schemas/") {
        Ok(InstalledMemberDomain::SchemaSet)
    } else if path.starts_with("templates/") {
        Ok(InstalledMemberDomain::TemplateCatalog)
    } else if path == "generation-backends.lock.json"
        || path.starts_with("generation-backends/")
        || path.starts_with("conformance/")
        || path.starts_with("transfer-syntax/")
    {
        Ok(InstalledMemberDomain::ProviderCatalog)
    } else if path == "assets/dcmtk_srgb_input_profile.hex"
        || path == "product/cli-error-codes.json"
        || path == "security/fixtures/fixtures.lock.json"
    {
        Ok(InstalledMemberDomain::Engine)
    } else {
        Err(IdentityProjectionError::UnclassifiedMember(
            path.to_string(),
        ))
    }
}

fn take_domain(
    members: &mut BTreeMap<InstalledMemberDomain, Vec<Member>>,
    domain: InstalledMemberDomain,
) -> Result<Vec<Member>, IdentityProjectionError> {
    members.remove(&domain).ok_or(match domain {
        InstalledMemberDomain::Engine => IdentityProjectionError::MissingMember("engine"),
        InstalledMemberDomain::SchemaSet => IdentityProjectionError::MissingMember("schemas"),
        InstalledMemberDomain::TemplateCatalog => {
            IdentityProjectionError::MissingMember("templates")
        }
        InstalledMemberDomain::ProviderCatalog => {
            IdentityProjectionError::MissingMember("providers")
        }
        InstalledMemberDomain::TransitionalCorpus => {
            IdentityProjectionError::MissingMember("transitional corpus")
        }
        InstalledMemberDomain::Toolchain => IdentityProjectionError::MissingMember("Cargo.lock"),
        InstalledMemberDomain::Standards => {
            IdentityProjectionError::MissingMember("standards.lock.json")
        }
    })
}

struct DomainDigest {
    sha256: String,
    member_count: usize,
    total_size_bytes: u64,
}

fn digest_domain(
    domain: &'static str,
    members: Vec<Member>,
) -> Result<DomainDigest, IdentityProjectionError> {
    let mut framed = format!(
        "identity_domains_schema_version={IDENTITY_DOMAINS_SCHEMA_VERSION}\nidentity_version={IDENTITY_DOMAIN_VERSION}\ndomain={domain}\n"
    )
    .into_bytes();
    let mut total_size_bytes = 0_u64;
    for member in &members {
        total_size_bytes = total_size_bytes
            .checked_add(member.bytes.len() as u64)
            .ok_or(IdentityProjectionError::MissingMember(
                "domain size overflow",
            ))?;
        framed.extend_from_slice(member.logical_path.as_bytes());
        framed.push(0);
        framed.extend_from_slice(crate::sha256_hex(&member.bytes).as_bytes());
        framed.push(0);
        framed.extend_from_slice(member.bytes.len().to_string().as_bytes());
        framed.push(b'\n');
    }
    Ok(DomainDigest {
        sha256: crate::sha256_hex(&framed),
        member_count: members.len(),
        total_size_bytes,
    })
}

#[cfg(test)]
mod identity_domain_tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    fn unique_temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "synth-dicom-gen-identity-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn copy_fixture(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_fixture(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn assert_installed_domains_equal(
        left: &InstalledIdentityDomains,
        right: &InstalledIdentityDomains,
    ) {
        assert_eq!(left.engine, right.engine);
        assert_eq!(left.schema_set, right.schema_set);
        assert_eq!(left.template_catalog, right.template_catalog);
        assert_eq!(left.provider_catalog, right.provider_catalog);
        assert_eq!(left.toolchain, right.toolchain);
        assert_eq!(left.external_runtime, right.external_runtime);
        assert_eq!(left.standards, right.standards);
        assert_eq!(left.execution, right.execution);
    }

    #[test]
    fn installed_membership_is_exhaustive_and_relocation_stable() {
        let embedded = project_installed_identities(
            &EngineResources::embedded(),
            IdentityInspectionContext::default(),
        )
        .unwrap();
        assert_eq!(embedded.engine.member_count, 3);
        assert_eq!(embedded.schema_set.member_count, 40);
        assert_eq!(embedded.template_catalog.member_count, 3);
        assert_eq!(embedded.provider_catalog.member_count, 16);
        assert_eq!(embedded.migration.legacy_resource_count, 240);
        assert!(embedded.corpus_definition.is_none());
        assert!(embedded.external_runtime.is_empty());
        assert!(matches!(
            classify_member("future/unclassified-resource.json"),
            Err(IdentityProjectionError::UnclassifiedMember(_))
        ));

        let snapshot = EngineResources::embedded().snapshot().unwrap();
        let explicit_resources = EngineResources::explicit(snapshot.root().to_path_buf()).unwrap();
        let explicit =
            project_installed_identities(&explicit_resources, IdentityInspectionContext::default())
                .unwrap();
        assert_installed_domains_equal(&embedded, &explicit);
        assert_eq!(
            embedded.migration.legacy_resource_origin,
            EngineResourceOrigin::Embedded
        );
        assert_eq!(
            explicit.migration.legacy_resource_origin,
            EngineResourceOrigin::Explicit
        );
    }

    #[test]
    fn verified_corpus_perturbation_changes_only_corpus_identity() {
        let fixture = Path::new("tests/fixtures/corpus-definition/minimal");
        let original_bundle = CorpusDefinitionBundle::load(fixture).unwrap();
        let changed_root = unique_temp_root("corpus-perturbation");
        copy_fixture(fixture, &changed_root);

        let evidence_path = changed_root.join("evidence/minimal.md");
        let mut evidence = fs::read(&evidence_path).unwrap();
        evidence.extend_from_slice(b"\nIdentity perturbation fixture.\n");
        fs::write(&evidence_path, &evidence).unwrap();
        let manifest_path = changed_root.join("corpus-definition.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["evidence"][0]["size_bytes"] = serde_json::json!(evidence.len());
        manifest["evidence"][0]["sha256"] = serde_json::json!(crate::sha256_hex(&evidence));
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let changed_bundle = CorpusDefinitionBundle::load(&changed_root).unwrap();

        let resources = EngineResources::embedded();
        let original = project_installed_identities(
            &resources,
            IdentityInspectionContext {
                corpus_definition: Some(&original_bundle),
            },
        )
        .unwrap();
        let changed = project_installed_identities(
            &resources,
            IdentityInspectionContext {
                corpus_definition: Some(&changed_bundle),
            },
        )
        .unwrap();
        assert_installed_domains_equal(&original, &changed);
        assert_ne!(original.corpus_definition, changed.corpus_definition);
        assert_eq!(
            original.migration.corpus_identity_status,
            "verified_bundle_loaded"
        );
        assert_eq!(
            changed.migration.corpus_identity_status,
            "verified_bundle_loaded"
        );
        fs::remove_dir_all(changed_root).unwrap();
    }
}
