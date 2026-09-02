//! Shared, version-aware manifest loading before validation or reporting.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::engine_resources::EngineResources;

const LEGACY_MANIFEST_ID: &str = "https://dicom-test-suite.local/schemas/manifest.schema.json";
const IDENTITY_SCHEMA_ID: &str =
    "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManifestContractKind {
    CuratedGeneration,
    QualifiedComposition,
    StructuralAssembly,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedManifest {
    path: PathBuf,
    bytes: Vec<u8>,
    value: Value,
    schema_version: String,
    kind: ManifestContractKind,
    seed: u64,
}

impl ValidatedManifest {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
    pub(crate) fn schema_version(&self) -> &str {
        &self.schema_version
    }
    pub(crate) fn kind(&self) -> ManifestContractKind {
        self.kind
    }
    pub(crate) fn seed(&self) -> u64 {
        self.seed
    }
}

#[derive(Debug)]
pub(crate) struct ManifestContractError(String);

impl fmt::Display for ManifestContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ManifestContractError {}

pub(crate) fn load_manifest_contract(
    output_root: &Path,
    resources: &EngineResources,
) -> Result<ValidatedManifest, ManifestContractError> {
    let path = output_root.join("manifest.json");
    let bytes = std::fs::read(&path).map_err(|error| {
        contract_error(format!(
            "failed to read manifest {}: {error}",
            path.display()
        ))
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract_error(format!(
            "manifest JSON invalid in {}: {error}",
            path.display()
        ))
    })?;
    let schema_version = value
        .pointer("/manifest_schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| contract_error("manifest_schema_version must be a string"))?
        .to_owned();
    let run_kind = value.pointer("/run/kind").and_then(Value::as_str);
    let (kind, schema_bytes): (ManifestContractKind, Vec<u8>) = match run_kind {
        None => match schema_version.as_str() {
            "0.2.0" | "0.3.0" => (
                ManifestContractKind::CuratedGeneration,
                resource_bytes(resources, "schemas/manifest.schema.json")?,
            ),
            "1.0.0" => (
                ManifestContractKind::CuratedGeneration,
                include_bytes!("../schemas/manifest-v1.schema.json").to_vec(),
            ),
            _ => {
                return Err(contract_error(format!(
                    "unsupported curated manifest schema version {schema_version}"
                )));
            }
        },
        Some("composition") => match schema_version.as_str() {
            "0.4.0" | "0.5.0" => (
                ManifestContractKind::QualifiedComposition,
                resource_bytes(resources, "schemas/composition-manifest.schema.json")?,
            ),
            _ => {
                return Err(contract_error(format!(
                    "unsupported composition manifest schema version {schema_version}"
                )));
            }
        },
        Some("structural_assembly") => match schema_version.as_str() {
            "1.0.0" => (
                ManifestContractKind::StructuralAssembly,
                resource_bytes(
                    resources,
                    "schemas/structural-assembly-manifest.schema.json",
                )?,
            ),
            _ => {
                return Err(contract_error(format!(
                    "unsupported assembly manifest schema version {schema_version}"
                )));
            }
        },
        Some(kind) => {
            return Err(contract_error(format!(
                "unsupported manifest run kind {kind}"
            )));
        }
    };
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| contract_error(format!("embedded manifest schema invalid: {error}")))?;
    let legacy_manifest: Value =
        serde_json::from_slice(&resource_bytes(resources, "schemas/manifest.schema.json")?)
            .map_err(|error| contract_error(format!("legacy manifest schema invalid: {error}")))?;
    let identities: Value =
        serde_json::from_slice(include_bytes!("../schemas/version-result-v2.schema.json"))
            .map_err(|error| contract_error(format!("identity schema invalid: {error}")))?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_resource(
            LEGACY_MANIFEST_ID,
            jsonschema::Resource::from_contents(legacy_manifest)
                .map_err(|error| contract_error(error.to_string()))?,
        )
        .with_resource(
            IDENTITY_SCHEMA_ID,
            jsonschema::Resource::from_contents(identities)
                .map_err(|error| contract_error(error.to_string()))?,
        )
        .build(&schema)
        .map_err(|error| contract_error(format!("manifest schema compilation failed: {error}")))?;
    if let Err(error) = validator.validate(&value) {
        return Err(contract_error(format!(
            "manifest schema invalid in {}: {error}",
            path.display()
        )));
    }
    let seed = value
        .pointer("/run/seed")
        .and_then(Value::as_u64)
        .ok_or_else(|| contract_error("manifest run seed must be an unsigned integer"))?;
    Ok(ValidatedManifest {
        path,
        bytes,
        value,
        schema_version,
        kind,
        seed,
    })
}

fn resource_bytes(
    resources: &EngineResources,
    path: &str,
) -> Result<Vec<u8>, ManifestContractError> {
    resources
        .bytes(path)
        .map(|bytes| bytes.into_owned())
        .map_err(|error| contract_error(format!("manifest schema resource {path}: {error}")))
}

fn contract_error(message: impl Into<String>) -> ManifestContractError {
    ManifestContractError(message.into())
}
