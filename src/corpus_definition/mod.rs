//! Integrity-checked caller-owned corpus definitions.
//!
//! Loading is intentionally inspection-only in R4.2. Execution remains on the
//! embedded corpus route until the supported SDK and CLI boundary lands in R5.

mod strict_json;

#[cfg(test)]
#[path = "../../tests/corpus_definition_bundle.rs"]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine_resources::EngineResources;

pub const CORPUS_DEFINITION_BUNDLE_SCHEMA_VERSION: &str = "1.0.0";
pub const CORPUS_DEFINITION_MANIFEST: &str = "corpus-definition.json";

#[derive(Debug, Clone, Copy)]
pub struct CorpusDefinitionLimits {
    pub manifest_bytes: u64,
    pub document_bytes: u64,
    pub total_document_bytes: u64,
    pub asset_bytes: u64,
    pub total_asset_bytes: u64,
    pub files: usize,
    pub profiles: usize,
    pub cases: usize,
    pub evidence: usize,
    pub assets: usize,
    pub dependencies_per_case: usize,
    pub json_depth: usize,
    pub json_array_entries: usize,
    pub json_string_bytes: usize,
}

impl Default for CorpusDefinitionLimits {
    fn default() -> Self {
        Self {
            manifest_bytes: 1024 * 1024,
            document_bytes: 2 * 1024 * 1024,
            total_document_bytes: 32 * 1024 * 1024,
            asset_bytes: 64 * 1024 * 1024,
            total_asset_bytes: 256 * 1024 * 1024,
            files: 4096,
            profiles: 32,
            cases: 4096,
            evidence: 4096,
            assets: 256,
            dependencies_per_case: 64,
            json_depth: 64,
            json_array_entries: 65_536,
            json_string_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusDefinitionManifest {
    pub corpus_definition_bundle_schema_version: String,
    pub definition_id: String,
    pub definition_version: String,
    pub profiles: Vec<ProfileDefinition>,
    pub registry: FileDescriptor,
    pub cases: Vec<CaseDefinition>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRecord>,
    #[serde(default)]
    pub assets: Vec<AssetRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDefinition {
    pub profile_id: String,
    pub scope: CorpusScope,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub union_of: Vec<String>,
    #[serde(default)]
    pub optional_profile: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusScope {
    Valid,
    Legacy,
    Stress,
    ExpectedInvalid,
    Fuzz,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDescriptor {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseDefinition {
    pub case_id: String,
    pub recipe_id: String,
    pub recipe_version: String,
    pub recipe: FileDescriptor,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub media_type: String,
    #[serde(flatten)]
    pub file: FileDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRecord {
    pub asset_id: String,
    pub media_type: String,
    #[serde(flatten)]
    pub file: FileDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorpusDefinitionIdentity {
    pub schema_version: String,
    pub definition_id: String,
    pub definition_version: String,
    pub manifest_sha256: String,
    pub corpus_definition_sha256: String,
    pub file_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CorpusDefinitionBundle {
    manifest: CorpusDefinitionManifest,
    identity: CorpusDefinitionIdentity,
    files: BTreeMap<String, Vec<u8>>,
}

impl CorpusDefinitionBundle {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, CorpusDefinitionError> {
        Self::load_with_limits(root, CorpusDefinitionLimits::default())
    }

    pub fn load_with_limits(
        root: impl AsRef<Path>,
        limits: CorpusDefinitionLimits,
    ) -> Result<Self, CorpusDefinitionError> {
        let root = root.as_ref();
        let bundle_root = BundleRoot::open(root)?;
        let manifest_bytes =
            bundle_root.capture(CORPUS_DEFINITION_MANIFEST, limits.manifest_bytes)?;
        let manifest_value =
            strict_json::parse(&manifest_bytes, CORPUS_DEFINITION_MANIFEST, limits)?;
        preflight_version_and_paths(&manifest_value)?;
        validate_schema(&manifest_value)?;
        let manifest: CorpusDefinitionManifest = serde_json::from_value(manifest_value)
            .map_err(|error| invalid(CORPUS_DEFINITION_MANIFEST, error.to_string()))?;
        validate_manifest_shape(&manifest, limits)?;

        let mut declared = BTreeMap::<String, (&str, &str, &FileDescriptor)>::new();
        insert_descriptor(&mut declared, "registry", "registry", &manifest.registry)?;
        for case in &manifest.cases {
            insert_descriptor(&mut declared, "recipe", &case.case_id, &case.recipe)?;
        }
        for evidence in &manifest.evidence {
            insert_descriptor(
                &mut declared,
                "evidence",
                &evidence.evidence_id,
                &evidence.file,
            )?;
        }
        for asset in &manifest.assets {
            insert_descriptor(&mut declared, "asset", &asset.asset_id, &asset.file)?;
        }
        if declared.len() + 1 > limits.files {
            return Err(limit("declared files", limits.files as u64));
        }
        reject_casefold_collisions(declared.keys().map(String::as_str))?;

        let mut files = BTreeMap::new();
        let mut document_total = manifest_bytes.len() as u64;
        let mut asset_total = 0_u64;
        for (path, (role, _id, descriptor)) in &declared {
            let per_file_limit = if *role == "asset" {
                limits.asset_bytes
            } else {
                limits.document_bytes
            };
            if descriptor.size_bytes > per_file_limit {
                return Err(limit(path, per_file_limit));
            }
            let bytes = bundle_root.capture(path, per_file_limit)?;
            if bytes.len() as u64 != descriptor.size_bytes {
                return Err(CorpusDefinitionError::Integrity(format!(
                    "{path}: size {} does not match declared {}",
                    bytes.len(),
                    descriptor.size_bytes
                )));
            }
            let actual = crate::sha256_hex(&bytes);
            if actual != descriptor.sha256 {
                return Err(CorpusDefinitionError::Integrity(format!(
                    "{path}: SHA-256 {actual} does not match declared {}",
                    descriptor.sha256
                )));
            }
            if *role == "asset" {
                asset_total = checked_total(
                    asset_total,
                    bytes.len() as u64,
                    limits.total_asset_bytes,
                    "asset bytes",
                )?;
            } else {
                document_total = checked_total(
                    document_total,
                    bytes.len() as u64,
                    limits.total_document_bytes,
                    "document bytes",
                )?;
                let _ = strict_json_or_evidence(&bytes, path, *role, limits)?;
            }
            files.insert(path.clone(), bytes);
        }
        bundle_root.reject_undeclared(declared.keys().map(String::as_str))?;
        validate_closure(&manifest, &files, limits)?;

        let manifest_sha256 = crate::sha256_hex(&manifest_bytes);
        let corpus_definition_sha256 = definition_digest(&manifest, &manifest_sha256, &declared);
        let total_size_bytes = manifest_bytes.len() as u64 + document_total
            - manifest_bytes.len() as u64
            + asset_total;
        Ok(Self {
            identity: CorpusDefinitionIdentity {
                schema_version: manifest.corpus_definition_bundle_schema_version.clone(),
                definition_id: manifest.definition_id.clone(),
                definition_version: manifest.definition_version.clone(),
                manifest_sha256,
                corpus_definition_sha256,
                file_count: declared.len() + 1,
                total_size_bytes,
            },
            manifest,
            files,
        })
    }

    pub fn manifest(&self) -> &CorpusDefinitionManifest {
        &self.manifest
    }
    pub fn identity(&self) -> &CorpusDefinitionIdentity {
        &self.identity
    }
    pub fn bytes(&self, logical_path: &str) -> Option<&[u8]> {
        self.files.get(logical_path).map(Vec::as_slice)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum CorpusDefinitionError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Invalid(String),
    UnsupportedVersion(String),
    Limit(String),
    UnsafePath(String),
    Symlink(PathBuf),
    NotRegular(PathBuf),
    Unstable(PathBuf),
    Integrity(String),
    Closure(String),
}

impl CorpusDefinitionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Read { .. } => "io.read.failed",
            Self::Invalid(_) => "request.json.invalid",
            Self::UnsupportedVersion(_) => "request.version.unsupported",
            Self::Limit(_) => "resource.limit.exceeded",
            Self::Integrity(_) => "evidence.integrity.failed",
            Self::UnsafePath(_)
            | Self::Symlink(_)
            | Self::NotRegular(_)
            | Self::Unstable(_)
            | Self::Closure(_) => "resource.document.invalid",
        }
    }
}

impl fmt::Display for CorpusDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "read corpus definition {}: {source}", path.display())
            }
            Self::Invalid(message) => write!(f, "invalid corpus definition JSON: {message}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported corpus definition bundle version {version}")
            }
            Self::Limit(message) => write!(f, "corpus definition limit exceeded: {message}"),
            Self::UnsafePath(path) => write!(f, "unsafe corpus definition path: {path}"),
            Self::Symlink(path) => write!(
                f,
                "corpus definition path is a symbolic link: {}",
                path.display()
            ),
            Self::NotRegular(path) => write!(
                f,
                "corpus definition path is not regular: {}",
                path.display()
            ),
            Self::Unstable(path) => write!(
                f,
                "corpus definition changed while loading: {}",
                path.display()
            ),
            Self::Integrity(message) => write!(f, "corpus definition integrity failed: {message}"),
            Self::Closure(message) => write!(f, "corpus definition closure failed: {message}"),
        }
    }
}
impl std::error::Error for CorpusDefinitionError {}

fn validate_schema(value: &Value) -> Result<(), CorpusDefinitionError> {
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/corpus-definition-bundle.schema.json"
    ))
    .expect("embedded corpus definition schema");
    let validator = jsonschema::validator_for(&schema).expect("corpus definition schema compiles");
    let errors = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CorpusDefinitionError::Invalid(errors.join("; ")))
    }
}

fn preflight_version_and_paths(value: &Value) -> Result<(), CorpusDefinitionError> {
    if let Some(version) = value
        .get("corpus_definition_bundle_schema_version")
        .and_then(Value::as_str)
    {
        if version != CORPUS_DEFINITION_BUNDLE_SCHEMA_VERSION {
            return Err(CorpusDefinitionError::UnsupportedVersion(
                version.to_string(),
            ));
        }
    }
    let mut descriptors = Vec::new();
    if let Some(registry) = value.get("registry") {
        descriptors.push(registry);
    }
    for collection in ["cases", "evidence", "assets"] {
        if let Some(items) = value.get(collection).and_then(Value::as_array) {
            for item in items {
                descriptors.push(if collection == "cases" {
                    item.get("recipe").unwrap_or(&Value::Null)
                } else {
                    item
                });
            }
        }
    }
    for descriptor in descriptors {
        if let Some(path) = descriptor.get("path").and_then(Value::as_str) {
            validate_logical_path(path)?;
        }
    }
    Ok(())
}

fn validate_manifest_shape(
    m: &CorpusDefinitionManifest,
    l: CorpusDefinitionLimits,
) -> Result<(), CorpusDefinitionError> {
    if m.corpus_definition_bundle_schema_version != CORPUS_DEFINITION_BUNDLE_SCHEMA_VERSION {
        return Err(CorpusDefinitionError::UnsupportedVersion(
            m.corpus_definition_bundle_schema_version.clone(),
        ));
    }
    for (name, actual, max) in [
        ("profiles", m.profiles.len(), l.profiles),
        ("cases", m.cases.len(), l.cases),
        ("evidence", m.evidence.len(), l.evidence),
        ("assets", m.assets.len(), l.assets),
    ] {
        if actual > max {
            return Err(limit(name, max as u64));
        }
    }
    Ok(())
}

fn insert_descriptor<'a>(
    out: &mut BTreeMap<String, (&'a str, &'a str, &'a FileDescriptor)>,
    role: &'a str,
    id: &'a str,
    d: &'a FileDescriptor,
) -> Result<(), CorpusDefinitionError> {
    validate_logical_path(&d.path)?;
    let canonical_role_path = match role {
        "registry" => d.path == "cases/registry.json",
        "recipe" => d.path.starts_with("cases/recipes/") && d.path.ends_with(".json"),
        "evidence" => d.path.starts_with("evidence/"),
        "asset" => d.path.starts_with("assets/"),
        _ => false,
    };
    if !canonical_role_path {
        return Err(CorpusDefinitionError::Closure(format!(
            "noncanonical {role} path {}",
            d.path
        )));
    }
    if is_reserved_engine_path(&d.path) {
        return Err(CorpusDefinitionError::Closure(format!(
            "caller file attempts engine resource override: {}",
            d.path
        )));
    }
    validate_sha256(&d.sha256)?;
    if out.insert(d.path.clone(), (role, id, d)).is_some() {
        return Err(CorpusDefinitionError::Closure(format!(
            "duplicate declared path {}",
            d.path
        )));
    }
    if id.is_empty() {
        return Err(CorpusDefinitionError::Closure(format!(
            "empty {role} identity"
        )));
    }
    Ok(())
}

fn is_reserved_engine_path(path: &str) -> bool {
    [
        "schemas/",
        "templates/",
        "transfer-syntax/",
        "conformance/",
        "generation-backends/",
        "security/",
        "product/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
        || matches!(
            path,
            "Cargo.lock" | "standards.lock.json" | "generation-backends.lock.json"
        )
        || path == "assets/dcmtk_srgb_input_profile.hex"
}

fn validate_logical_path(path: &str) -> Result<(), CorpusDefinitionError> {
    let parsed = Path::new(path);
    let safe = !path.is_empty()
        && path.is_ascii()
        && path.len() <= 512
        && !path.contains('\\')
        && !path.contains('\0')
        && !parsed.is_absolute()
        && parsed
            .components()
            .all(|c| matches!(c, Component::Normal(s) if s.to_string_lossy().len() <= 128));
    if safe {
        Ok(())
    } else {
        Err(CorpusDefinitionError::UnsafePath(path.to_string()))
    }
}

fn validate_sha256(value: &str) -> Result<(), CorpusDefinitionError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(CorpusDefinitionError::Invalid(format!(
            "invalid lowercase SHA-256 {value:?}"
        )))
    }
}

fn reject_casefold_collisions<'a>(
    paths: impl Iterator<Item = &'a str>,
) -> Result<(), CorpusDefinitionError> {
    let mut folded = BTreeMap::new();
    for path in paths {
        if let Some(previous) = folded.insert(path.to_ascii_lowercase(), path) {
            return Err(CorpusDefinitionError::UnsafePath(format!(
                "case-fold collision: {previous} and {path}"
            )));
        }
    }
    Ok(())
}

fn checked_total(
    current: u64,
    add: u64,
    max: u64,
    name: &str,
) -> Result<u64, CorpusDefinitionError> {
    let total = current.checked_add(add).ok_or_else(|| limit(name, max))?;
    if total > max {
        Err(limit(name, max))
    } else {
        Ok(total)
    }
}

fn limit(name: &str, max: u64) -> CorpusDefinitionError {
    CorpusDefinitionError::Limit(format!("{name} exceeds {max}"))
}
fn invalid(path: &str, message: String) -> CorpusDefinitionError {
    CorpusDefinitionError::Invalid(format!("{path}: {message}"))
}

fn strict_json_or_evidence(
    bytes: &[u8],
    path: &str,
    role: &str,
    limits: CorpusDefinitionLimits,
) -> Result<Option<Value>, CorpusDefinitionError> {
    if role == "evidence" && !path.ends_with(".json") {
        if bytes.starts_with(&[0xef, 0xbb, 0xbf]) || std::str::from_utf8(bytes).is_err() {
            return Err(invalid(path, "evidence must be UTF-8 without BOM".into()));
        }
        Ok(None)
    } else {
        strict_json::parse(bytes, path, limits).map(Some)
    }
}

fn validate_closure(
    m: &CorpusDefinitionManifest,
    files: &BTreeMap<String, Vec<u8>>,
    limits: CorpusDefinitionLimits,
) -> Result<(), CorpusDefinitionError> {
    let registry: Value = strict_json::parse(
        files.get(&m.registry.path).expect("captured registry"),
        &m.registry.path,
        limits,
    )?;
    let registry_schema: Value =
        serde_json::from_str(include_str!("../../schemas/case-registry.schema.json"))
            .expect("embedded case registry schema");
    let registry_validator =
        jsonschema::validator_for(&registry_schema).expect("case registry schema compiles");
    let registry_errors = registry_validator
        .iter_errors(&registry)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if !registry_errors.is_empty() {
        return Err(CorpusDefinitionError::Closure(format!(
            "registry schema: {}",
            registry_errors.join("; ")
        )));
    }
    let rows = registry
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| CorpusDefinitionError::Closure("registry has no cases array".into()))?;
    let mut registry_cases = BTreeMap::new();
    for row in rows {
        let case_id = row
            .get("case_id")
            .and_then(Value::as_str)
            .ok_or_else(|| CorpusDefinitionError::Closure("registry case has no case_id".into()))?;
        if registry_cases.insert(case_id, row).is_some() {
            return Err(CorpusDefinitionError::Closure(format!(
                "duplicate registry case {case_id}"
            )));
        }
    }
    let mut definitions = BTreeMap::new();
    for case in &m.cases {
        if definitions.insert(case.case_id.as_str(), case).is_some() {
            return Err(CorpusDefinitionError::Closure(format!(
                "duplicate case definition {}",
                case.case_id
            )));
        }
        if case.dependencies.len() > limits.dependencies_per_case {
            return Err(limit(&case.case_id, limits.dependencies_per_case as u64));
        }
        let row = registry_cases.get(case.case_id.as_str()).ok_or_else(|| {
            CorpusDefinitionError::Closure(format!("case {} absent from registry", case.case_id))
        })?;
        if row.get("status").and_then(Value::as_str) != Some("implemented") {
            return Err(CorpusDefinitionError::Closure(format!(
                "non-implemented case {} has a recipe",
                case.case_id
            )));
        }
        if row.get("recipe_id").and_then(Value::as_str) != Some(&case.recipe_id)
            || row.get("recipe_version").and_then(Value::as_str) != Some(&case.recipe_version)
        {
            return Err(CorpusDefinitionError::Closure(format!(
                "registry binding mismatch for {}",
                case.case_id
            )));
        }
        let recipe: Value = strict_json::parse(
            files.get(&case.recipe.path).expect("captured recipe"),
            &case.recipe.path,
            limits,
        )?;
        if recipe.pointer("/binding/case_id").and_then(Value::as_str) != Some(&case.case_id)
            || recipe.get("recipe_id").and_then(Value::as_str) != Some(&case.recipe_id)
            || recipe.get("recipe_version").and_then(Value::as_str) != Some(&case.recipe_version)
        {
            return Err(CorpusDefinitionError::Closure(format!(
                "recipe binding mismatch for {}",
                case.case_id
            )));
        }
        let dependency_identities =
            crate::recipes::inspect_corpus_recipe(&case.recipe.path, recipe.clone())
                .map_err(|error| CorpusDefinitionError::Closure(error.to_string()))?;
        let expected_dependencies = dependency_identities
            .iter()
            .map(|(recipe_id, recipe_version)| {
                m.cases
                    .iter()
                    .find(|candidate| {
                        &candidate.recipe_id == recipe_id
                            && &candidate.recipe_version == recipe_version
                    })
                    .map(|candidate| candidate.case_id.as_str())
                    .ok_or_else(|| {
                        CorpusDefinitionError::Closure(format!(
                            "{} references missing recipe {recipe_id}@{recipe_version}",
                            case.case_id
                        ))
                    })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if expected_dependencies
            != case
                .dependencies
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        {
            return Err(CorpusDefinitionError::Closure(format!(
                "dependency declaration mismatch for {}",
                case.case_id
            )));
        }
    }
    for (id, row) in &registry_cases {
        let implemented = row.get("status").and_then(Value::as_str) == Some("implemented");
        if implemented != definitions.contains_key(id) {
            return Err(CorpusDefinitionError::Closure(format!(
                "registry/definition completeness mismatch for {id}"
            )));
        }
        let row_profiles = row
            .get("profiles")
            .and_then(Value::as_array)
            .expect("registry schema validated profiles")
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        let valid_scope = row_profiles.iter().any(|profile| {
            matches!(
                *profile,
                "smoke" | "core" | "extended" | "legacy" | "stress"
            )
        });
        if row_profiles.contains("all")
            || (valid_scope && (row_profiles.contains("negative") || row_profiles.contains("fuzz")))
            || (row_profiles.contains("negative") && row_profiles.contains("fuzz"))
        {
            return Err(CorpusDefinitionError::Closure(format!(
                "profile scope leakage for {id}"
            )));
        }
    }
    let evidence = unique_ids(
        m.evidence.iter().map(|e| e.evidence_id.as_str()),
        "evidence",
    )?;
    let assets = unique_ids(m.assets.iter().map(|a| a.asset_id.as_str()), "asset")?;
    let profiles = unique_ids(m.profiles.iter().map(|p| p.profile_id.as_str()), "profile")?;
    for profile in &m.profiles {
        let expected_scope = match profile.profile_id.as_str() {
            "smoke" | "core" | "extended" | "all" => CorpusScope::Valid,
            "legacy" => CorpusScope::Legacy,
            "stress" => CorpusScope::Stress,
            "negative" => CorpusScope::ExpectedInvalid,
            "fuzz" => CorpusScope::Fuzz,
            other => {
                return Err(CorpusDefinitionError::Closure(format!(
                    "unknown profile {other}"
                )));
            }
        };
        if profile.scope != expected_scope {
            return Err(CorpusDefinitionError::Closure(format!(
                "scope mismatch for profile {}",
                profile.profile_id
            )));
        }
        if profile.profile_id == "all" {
            let expected = ["smoke", "core", "extended"]
                .into_iter()
                .collect::<BTreeSet<_>>();
            if profile
                .union_of
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != expected
                || profile.optional_profile.as_deref() != Some("stress")
            {
                return Err(CorpusDefinitionError::Closure(
                    "all must be smoke/core/extended with optional stress".into(),
                ));
            }
            if !profile.members.is_empty() {
                return Err(CorpusDefinitionError::Closure(
                    "all cannot contain direct members".into(),
                ));
            }
        } else if !profile.union_of.is_empty() || profile.optional_profile.is_some() {
            return Err(CorpusDefinitionError::Closure(format!(
                "direct profile {} cannot be computed",
                profile.profile_id
            )));
        }
        if profile.profile_id != "all" {
            let expected_members = registry_cases
                .iter()
                .filter(|(_, row)| {
                    row.get("profiles")
                        .and_then(Value::as_array)
                        .is_some_and(|profiles| {
                            profiles
                                .iter()
                                .any(|value| value.as_str() == Some(&profile.profile_id))
                        })
                })
                .map(|(case_id, _)| *case_id)
                .collect::<BTreeSet<_>>();
            if expected_members
                != profile
                    .members
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
            {
                return Err(CorpusDefinitionError::Closure(format!(
                    "profile membership mismatch for {}",
                    profile.profile_id
                )));
            }
        }
        for member in &profile.members {
            if !registry_cases.contains_key(member.as_str()) {
                return Err(CorpusDefinitionError::Closure(format!(
                    "profile {} references unknown case {member}",
                    profile.profile_id
                )));
            }
        }
    }
    for required in [
        "smoke", "core", "extended", "legacy", "stress", "negative", "fuzz", "all",
    ] {
        if !profiles.contains(required) {
            return Err(CorpusDefinitionError::Closure(format!(
                "missing required profile {required}"
            )));
        }
    }
    for case in &m.cases {
        let mut seen = BTreeSet::new();
        for dependency in &case.dependencies {
            if dependency == &case.case_id
                || !seen.insert(dependency)
                || !definitions.contains_key(dependency.as_str())
            {
                return Err(CorpusDefinitionError::Closure(format!(
                    "invalid dependency {dependency} for {}",
                    case.case_id
                )));
            }
            let owner_profiles = registry_cases[case.case_id.as_str()]["profiles"]
                .as_array()
                .expect("registry schema validated profiles");
            let dependency_profiles = registry_cases[dependency.as_str()]["profiles"]
                .as_array()
                .expect("registry schema validated profiles");
            let owner_is_ordinary = owner_profiles
                .iter()
                .any(|value| matches!(value.as_str(), Some("smoke" | "core" | "extended")));
            let owner_is_legacy = owner_profiles
                .iter()
                .any(|value| value.as_str() == Some("legacy"));
            let owner_is_stress = owner_profiles
                .iter()
                .any(|value| value.as_str() == Some("stress"));
            let dependency_is_invalid = dependency_profiles
                .iter()
                .any(|value| matches!(value.as_str(), Some("negative" | "fuzz")));
            let dependency_is_legacy = dependency_profiles
                .iter()
                .any(|value| value.as_str() == Some("legacy"));
            let dependency_is_stress = dependency_profiles
                .iter()
                .any(|value| value.as_str() == Some("stress"));
            let owner_negative = owner_profiles
                .iter()
                .any(|value| value.as_str() == Some("negative"));
            let owner_fuzz = owner_profiles
                .iter()
                .any(|value| value.as_str() == Some("fuzz"));
            let dependency_negative = dependency_profiles
                .iter()
                .any(|value| value.as_str() == Some("negative"));
            let dependency_fuzz = dependency_profiles
                .iter()
                .any(|value| value.as_str() == Some("fuzz"));
            if (owner_is_ordinary
                && (dependency_is_legacy || dependency_is_stress || dependency_is_invalid))
                || (owner_is_legacy && (dependency_is_stress || dependency_is_invalid))
                || (owner_is_stress && (dependency_is_legacy || dependency_is_invalid))
                || (owner_negative && dependency_fuzz)
                || (owner_fuzz && dependency_negative)
            {
                return Err(CorpusDefinitionError::Closure(format!(
                    "dependency scope leakage from {} to {dependency}",
                    case.case_id
                )));
            }
        }
        for id in &case.evidence_ids {
            if !evidence.contains(id.as_str()) {
                return Err(CorpusDefinitionError::Closure(format!(
                    "unknown evidence {id}"
                )));
            }
        }
        let row = registry_cases[case.case_id.as_str()];
        let expected_evidence = row
            .get("standards_evidence")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|record| {
                record.get("source").and_then(Value::as_str) == Some("local-source-note")
            })
            .map(|record| {
                let path = record.get("query").and_then(Value::as_str).ok_or_else(|| {
                    CorpusDefinitionError::Closure(format!(
                        "local source note without a path for {}",
                        case.case_id
                    ))
                })?;
                let evidence_id = source_note_evidence_id(path)?;
                evidence.get(evidence_id.as_str()).copied().ok_or_else(|| {
                    CorpusDefinitionError::Closure(format!(
                        "missing evidence descriptor for {path}"
                    ))
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if expected_evidence
            != case
                .evidence_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        {
            return Err(CorpusDefinitionError::Closure(format!(
                "evidence declaration mismatch for {}",
                case.case_id
            )));
        }
        for id in &case.asset_ids {
            if !assets.contains(id.as_str()) {
                return Err(CorpusDefinitionError::Closure(format!(
                    "unknown asset {id}"
                )));
            }
        }
        let recipe = strict_json::parse(
            files.get(&case.recipe.path).expect("captured recipe"),
            &case.recipe.path,
            limits,
        )?;
        let mut referenced_assets = BTreeSet::new();
        collect_named_string_values(&recipe, "asset_id", &mut referenced_assets);
        if referenced_assets
            != case
                .asset_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        {
            return Err(CorpusDefinitionError::Closure(format!(
                "asset declaration mismatch for {}",
                case.case_id
            )));
        }
    }
    detect_cycles(&definitions)?;
    let used_evidence = registry_cases
        .values()
        .flat_map(|row| {
            row.get("standards_evidence")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|record| record.get("source").and_then(Value::as_str) == Some("local-source-note"))
        .filter_map(|record| record.get("query").and_then(Value::as_str))
        .map(|path| {
            let evidence_id = source_note_evidence_id(path)?;
            evidence.get(evidence_id.as_str()).copied().ok_or_else(|| {
                CorpusDefinitionError::Closure(format!("missing evidence descriptor for {path}"))
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let used_assets = m
        .cases
        .iter()
        .flat_map(|c| c.asset_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    if used_evidence != evidence || used_assets != assets {
        return Err(CorpusDefinitionError::Closure(
            "orphan evidence or asset descriptor".into(),
        ));
    }
    // Caller data must not claim trusted product resource paths or identifiers.
    let trusted = EngineResources::embedded();
    for path in files.keys() {
        let transitional_corpus_path =
            path == &m.registry.path || path.starts_with("cases/recipes/");
        if trusted.contains(path) && !transitional_corpus_path {
            return Err(CorpusDefinitionError::Closure(format!(
                "caller file attempts engine resource override: {path}"
            )));
        }
    }
    Ok(())
}

fn source_note_evidence_id(path: &str) -> Result<String, CorpusDefinitionError> {
    validate_logical_path(path)?;
    let relative = path
        .strip_prefix("standards/source-notes/")
        .or_else(|| path.strip_prefix("evidence/"))
        .ok_or_else(|| {
            CorpusDefinitionError::Closure(format!(
                "local source note is outside an evidence namespace: {path}"
            ))
        })?;
    let stem = relative.strip_suffix(".md").unwrap_or(relative);
    Ok(format!("source-note.{}", stem.replace('/', ".")))
}

fn collect_named_string_values<'a>(value: &'a Value, key: &str, output: &mut BTreeSet<&'a str>) {
    match value {
        Value::Object(map) => {
            for (name, child) in map {
                if name == key {
                    if let Some(value) = child.as_str() {
                        output.insert(value);
                    }
                }
                collect_named_string_values(child, key, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_named_string_values(child, key, output);
            }
        }
        _ => {}
    }
}

fn unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    role: &str,
) -> Result<BTreeSet<&'a str>, CorpusDefinitionError> {
    let mut out = BTreeSet::new();
    for id in ids {
        if id.is_empty() || !out.insert(id) {
            return Err(CorpusDefinitionError::Closure(format!(
                "duplicate or empty {role} id {id:?}"
            )));
        }
    }
    Ok(out)
}

fn detect_cycles(
    definitions: &BTreeMap<&str, &CaseDefinition>,
) -> Result<(), CorpusDefinitionError> {
    fn visit<'a>(
        id: &'a str,
        defs: &BTreeMap<&'a str, &'a CaseDefinition>,
        visiting: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<(), CorpusDefinitionError> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id) {
            return Err(CorpusDefinitionError::Closure(format!(
                "dependency cycle at {id}"
            )));
        }
        for dep in &defs[id].dependencies {
            visit(dep, defs, visiting, done)?;
        }
        visiting.remove(id);
        done.insert(id);
        Ok(())
    }
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    for id in definitions.keys() {
        visit(id, definitions, &mut visiting, &mut done)?;
    }
    Ok(())
}

fn definition_digest(
    m: &CorpusDefinitionManifest,
    manifest_sha: &str,
    files: &BTreeMap<String, (&str, &str, &FileDescriptor)>,
) -> String {
    let mut framed = b"synth-dicom-gen/corpus-definition-bundle\0".to_vec();
    framed.extend_from_slice(m.corpus_definition_bundle_schema_version.as_bytes());
    framed.push(0);
    framed.extend_from_slice(manifest_sha.as_bytes());
    framed.push(b'\n');
    for (path, (role, id, descriptor)) in files {
        framed.extend_from_slice(role.as_bytes());
        framed.push(0);
        framed.extend_from_slice(id.as_bytes());
        framed.push(0);
        framed.extend_from_slice(path.as_bytes());
        framed.push(0);
        framed.extend_from_slice(descriptor.size_bytes.to_string().as_bytes());
        framed.push(0);
        framed.extend_from_slice(descriptor.sha256.as_bytes());
        framed.push(b'\n');
    }
    crate::sha256_hex(&framed)
}

struct BundleRoot {
    path: PathBuf,
    #[cfg(unix)]
    file: fs::File,
}

impl BundleRoot {
    fn open(root: &Path) -> Result<Self, CorpusDefinitionError> {
        validate_root(root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
            let before =
                fs::symlink_metadata(root).map_err(|source| CorpusDefinitionError::Read {
                    path: root.to_path_buf(),
                    source,
                })?;
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
                .open(root)
                .map_err(|source| CorpusDefinitionError::Read {
                    path: root.to_path_buf(),
                    source,
                })?;
            let opened = file
                .metadata()
                .map_err(|source| CorpusDefinitionError::Read {
                    path: root.to_path_buf(),
                    source,
                })?;
            if before.dev() != opened.dev() || before.ino() != opened.ino() {
                return Err(CorpusDefinitionError::Unstable(root.to_path_buf()));
            }
            Ok(Self {
                path: root.to_path_buf(),
                file,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                path: root.to_path_buf(),
            })
        }
    }

    fn capture(&self, logical: &str, max: u64) -> Result<Vec<u8>, CorpusDefinitionError> {
        #[cfg(unix)]
        {
            capture_file(&self.file, &self.path, logical, max)
        }
        #[cfg(not(unix))]
        {
            capture_file(&self.path, logical, max)
        }
    }

    fn reject_undeclared<'a>(
        &self,
        declared: impl Iterator<Item = &'a str>,
    ) -> Result<(), CorpusDefinitionError> {
        let expected = declared
            .chain(std::iter::once(CORPUS_DEFINITION_MANIFEST))
            .collect::<BTreeSet<_>>();
        #[cfg(unix)]
        {
            inventory_directory(&self.file, "", &expected)
        }
        #[cfg(not(unix))]
        {
            reject_undeclared(&self.path, expected.into_iter())
        }
    }
}

fn validate_root(root: &Path) -> Result<(), CorpusDefinitionError> {
    let meta = fs::symlink_metadata(root).map_err(|source| CorpusDefinitionError::Read {
        path: root.to_path_buf(),
        source,
    })?;
    if meta.file_type().is_symlink() {
        Err(CorpusDefinitionError::Symlink(root.to_path_buf()))
    } else if !meta.is_dir() {
        Err(CorpusDefinitionError::NotRegular(root.to_path_buf()))
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn capture_file(root: &Path, logical: &str, max: u64) -> Result<Vec<u8>, CorpusDefinitionError> {
    validate_logical_path(logical)?;
    let mut current = root.to_path_buf();
    for segment in Path::new(logical).components() {
        let Component::Normal(segment) = segment else {
            unreachable!()
        };
        current.push(segment);
        let meta =
            fs::symlink_metadata(&current).map_err(|source| CorpusDefinitionError::Read {
                path: current.clone(),
                source,
            })?;
        if meta.file_type().is_symlink() {
            return Err(CorpusDefinitionError::Symlink(current));
        }
    }
    let before = fs::metadata(&current).map_err(|source| CorpusDefinitionError::Read {
        path: current.clone(),
        source,
    })?;
    if !before.is_file() {
        return Err(CorpusDefinitionError::NotRegular(current));
    }
    if before.len() > max {
        return Err(limit(logical, max));
    }
    let mut file = fs::File::open(&current).map_err(|source| CorpusDefinitionError::Read {
        path: current.clone(),
        source,
    })?;
    let mut bytes = Vec::with_capacity(before.len() as usize);
    file.by_ref()
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| CorpusDefinitionError::Read {
            path: current.clone(),
            source,
        })?;
    if bytes.len() as u64 > max {
        return Err(limit(logical, max));
    }
    let after = file
        .metadata()
        .map_err(|source| CorpusDefinitionError::Read {
            path: current.clone(),
            source,
        })?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err(CorpusDefinitionError::Unstable(current));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn capture_file(
    root_file: &fs::File,
    root: &Path,
    logical: &str,
    max: u64,
) -> Result<Vec<u8>, CorpusDefinitionError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;

    validate_logical_path(logical)?;
    let root_open = root_file
        .metadata()
        .map_err(|source| CorpusDefinitionError::Read {
            path: root.to_path_buf(),
            source,
        })?;
    let components = Path::new(logical).components().collect::<Vec<_>>();
    let mut parent: Option<OwnedFd> = None;
    let mut display = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(segment) = component else {
            unreachable!()
        };
        display.push(segment);
        let name = CString::new(segment.as_bytes())
            .map_err(|_| CorpusDefinitionError::UnsafePath(logical.into()))?;
        let is_file = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if is_file {
                libc::O_NONBLOCK
            } else {
                libc::O_DIRECTORY
            };
        let parent_fd = parent
            .as_ref()
            .map_or(root_file.as_raw_fd(), AsRawFd::as_raw_fd);
        let raw = unsafe { libc::openat(parent_fd, name.as_ptr(), flags) };
        if raw < 0 {
            let source = std::io::Error::last_os_error();
            return if source.raw_os_error() == Some(libc::ELOOP) {
                Err(CorpusDefinitionError::Symlink(display))
            } else {
                Err(CorpusDefinitionError::Read {
                    path: display,
                    source,
                })
            };
        }
        // SAFETY: `openat` returned a new descriptor owned by this function.
        let owned = unsafe { OwnedFd::from_raw_fd(raw) };
        if !is_file {
            parent = Some(owned);
            continue;
        }
        let mut file = fs::File::from(owned);
        let before = file
            .metadata()
            .map_err(|source| CorpusDefinitionError::Read {
                path: display.clone(),
                source,
            })?;
        if !before.is_file() {
            return Err(CorpusDefinitionError::NotRegular(display));
        }
        if before.nlink() != 1 {
            return Err(CorpusDefinitionError::NotRegular(display));
        }
        if before.len() > max {
            return Err(limit(logical, max));
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        file.by_ref()
            .take(max + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| CorpusDefinitionError::Read {
                path: display.clone(),
                source,
            })?;
        if bytes.len() as u64 > max {
            return Err(limit(logical, max));
        }
        let after = file
            .metadata()
            .map_err(|source| CorpusDefinitionError::Read {
                path: display.clone(),
                source,
            })?;
        let root_after = root_file
            .metadata()
            .map_err(|source| CorpusDefinitionError::Read {
                path: root.to_path_buf(),
                source,
            })?;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.len() != after.len()
            || root_open.dev() != root_after.dev()
            || root_open.ino() != root_after.ino()
        {
            return Err(CorpusDefinitionError::Unstable(display));
        }
        return Ok(bytes);
    }
    unreachable!("validated logical paths contain a component")
}

#[cfg(unix)]
struct DirectoryStream(*mut libc::DIR);

#[cfg(unix)]
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        unsafe { libc::closedir(self.0) };
    }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
unsafe fn errno_pointer() -> *mut libc::c_int {
    unsafe { libc::__error() }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn errno_pointer() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}

#[cfg(unix)]
fn inventory_directory(
    directory: &fs::File,
    prefix: &str,
    expected: &BTreeSet<&str>,
) -> Result<(), CorpusDefinitionError> {
    use std::ffi::{CStr, CString};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;

    let before = directory
        .metadata()
        .map_err(|source| CorpusDefinitionError::Read {
            path: PathBuf::from(prefix),
            source,
        })?;
    let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicate < 0 {
        return Err(CorpusDefinitionError::Read {
            path: PathBuf::from(prefix),
            source: std::io::Error::last_os_error(),
        });
    }
    let raw_stream = unsafe { libc::fdopendir(duplicate) };
    if raw_stream.is_null() {
        unsafe { libc::close(duplicate) };
        return Err(CorpusDefinitionError::Read {
            path: PathBuf::from(prefix),
            source: std::io::Error::last_os_error(),
        });
    }
    let stream = DirectoryStream(raw_stream);
    let mut entry_count = 0_usize;
    loop {
        unsafe { *errno_pointer() = 0 };
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let errno = unsafe { *errno_pointer() };
            if errno != 0 {
                return Err(CorpusDefinitionError::Read {
                    path: PathBuf::from(prefix),
                    source: std::io::Error::from_raw_os_error(errno),
                });
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        entry_count = entry_count
            .checked_add(1)
            .ok_or_else(|| limit(prefix, expected.len() as u64))?;
        if entry_count > expected.len() {
            return Err(CorpusDefinitionError::Closure(format!(
                "directory {prefix:?} contains more entries than the declared bundle can own"
            )));
        }
        let name_bytes = name.to_vec();
        let name = std::str::from_utf8(&name_bytes)
            .map_err(|_| CorpusDefinitionError::UnsafePath(format!("{prefix}<non-UTF8>")))?;
        let logical = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        validate_logical_path(&logical)?;
        let c_name = CString::new(name_bytes).expect("directory names contain no NUL");
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                c_name.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(CorpusDefinitionError::Read {
                path: PathBuf::from(&logical),
                source: std::io::Error::last_os_error(),
            });
        }
        let kind = stat.st_mode & libc::S_IFMT;
        if kind == libc::S_IFLNK {
            return Err(CorpusDefinitionError::Symlink(PathBuf::from(logical)));
        }
        if kind == libc::S_IFDIR {
            let ancestor = format!("{logical}/");
            if !expected.iter().any(|path| path.starts_with(&ancestor)) {
                return Err(CorpusDefinitionError::Closure(format!(
                    "undeclared directory {logical}"
                )));
            }
            let raw = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    c_name.as_ptr(),
                    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                )
            };
            if raw < 0 {
                return Err(CorpusDefinitionError::Read {
                    path: PathBuf::from(&logical),
                    source: std::io::Error::last_os_error(),
                });
            }
            let child = unsafe { fs::File::from_raw_fd(raw) };
            inventory_directory(&child, &logical, expected)?;
        } else if kind == libc::S_IFREG {
            if !expected.contains(logical.as_str()) {
                return Err(CorpusDefinitionError::Closure(format!(
                    "undeclared file {logical}"
                )));
            }
        } else {
            return Err(CorpusDefinitionError::NotRegular(PathBuf::from(logical)));
        }
    }
    let after = directory
        .metadata()
        .map_err(|source| CorpusDefinitionError::Read {
            path: PathBuf::from(prefix),
            source,
        })?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.len() != after.len()
    {
        return Err(CorpusDefinitionError::Unstable(PathBuf::from(prefix)));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_undeclared<'a>(
    root: &Path,
    declared: impl Iterator<Item = &'a str>,
) -> Result<(), CorpusDefinitionError> {
    let expected = declared
        .chain(std::iter::once(CORPUS_DEFINITION_MANIFEST))
        .collect::<BTreeSet<_>>();
    fn walk(
        root: &Path,
        dir: &Path,
        expected: &BTreeSet<&str>,
    ) -> Result<(), CorpusDefinitionError> {
        let mut entries = fs::read_dir(dir)
            .map_err(|source| CorpusDefinitionError::Read {
                path: dir.to_path_buf(),
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| CorpusDefinitionError::Read {
                path: dir.to_path_buf(),
                source,
            })?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let meta =
                fs::symlink_metadata(&path).map_err(|source| CorpusDefinitionError::Read {
                    path: path.clone(),
                    source,
                })?;
            if meta.file_type().is_symlink() {
                return Err(CorpusDefinitionError::Symlink(path));
            }
            if meta.is_dir() {
                walk(root, &path, expected)?;
            } else if meta.is_file() {
                let logical = path
                    .strip_prefix(root)
                    .expect("under root")
                    .to_string_lossy()
                    .replace('\\', "/");
                if !expected.contains(logical.as_str()) {
                    return Err(CorpusDefinitionError::Closure(format!(
                        "undeclared file {logical}"
                    )));
                }
            } else {
                return Err(CorpusDefinitionError::NotRegular(path));
            }
        }
        Ok(())
    }
    walk(root, root, &expected)
}
