//! Shared, version-aware manifest loading before validation or reporting.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::engine_resources::EngineResources;

const LEGACY_MANIFEST_ID: &str = "https://dicom-test-suite.local/schemas/manifest.schema.json";
const IDENTITY_SCHEMA_ID: &str =
    "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManifestContractKind {
    ExternalCorpus,
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
            "1.0.0" => (
                ManifestContractKind::QualifiedComposition,
                include_bytes!("../schemas/composition-manifest-v1.schema.json").to_vec(),
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
            "2.0.0" => (
                ManifestContractKind::StructuralAssembly,
                include_bytes!("../schemas/structural-assembly-manifest-v2.schema.json").to_vec(),
            ),
            _ => {
                return Err(contract_error(format!(
                    "unsupported assembly manifest schema version {schema_version}"
                )));
            }
        },
        Some("external_corpus") if schema_version == "2.0.0" => (
            ManifestContractKind::ExternalCorpus,
            include_bytes!("../schemas/manifest-v2.schema.json").to_vec(),
        ),
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
            "https://synth-dicom-gen.local/schemas/manifest-v1.schema.json",
            jsonschema::Resource::from_contents(
                serde_json::from_slice::<Value>(include_bytes!(
                    "../schemas/manifest-v1.schema.json"
                ))
                .map_err(|error| contract_error(error.to_string()))?,
            )
            .map_err(|error| contract_error(error.to_string()))?,
        )
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
    if matches!(
        (kind, schema_version.as_str()),
        (ManifestContractKind::CuratedGeneration, "1.0.0")
            | (ManifestContractKind::QualifiedComposition, "1.0.0")
            | (ManifestContractKind::StructuralAssembly, "2.0.0")
    ) {
        validate_identity_projection_runtime_uniqueness(&value)?;
    }
    if kind == ManifestContractKind::ExternalCorpus {
        validate_external_corpus_manifest(&value)?;
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

pub(crate) fn validate_identity_projection_runtime_uniqueness(
    value: &Value,
) -> Result<(), ManifestContractError> {
    let runtimes = value
        .pointer("/identity_projection/external_runtime")
        .and_then(Value::as_array)
        .ok_or_else(|| contract_error("identity_projection.external_runtime must be an array"))?;
    let mut runtime_ids = BTreeSet::new();
    for runtime in runtimes {
        let runtime_id = runtime
            .get("runtime_id")
            .and_then(Value::as_str)
            .ok_or_else(|| contract_error("external runtime identity has no runtime_id"))?;
        if !runtime_ids.insert(runtime_id) {
            return Err(contract_error(format!(
                "identity_projection.external_runtime contains duplicate runtime_id {runtime_id}"
            )));
        }
    }
    Ok(())
}

/// Validate the external-corpus wire contract before transaction publication.
pub(crate) fn validate_external_corpus_manifest(
    value: &Value,
) -> Result<(), ManifestContractError> {
    let mut options = jsonschema::options().with_draft(jsonschema::Draft::Draft202012);
    for (id, bytes) in [
        (
            LEGACY_MANIFEST_ID,
            include_bytes!("../schemas/manifest.schema.json").as_slice(),
        ),
        (
            IDENTITY_SCHEMA_ID,
            include_bytes!("../schemas/version-result-v2.schema.json").as_slice(),
        ),
        (
            "https://synth-dicom-gen.local/schemas/manifest-v1.schema.json",
            include_bytes!("../schemas/manifest-v1.schema.json").as_slice(),
        ),
    ] {
        let schema: Value =
            serde_json::from_slice(bytes).map_err(|e| contract_error(e.to_string()))?;
        options = options.with_resource(
            id,
            jsonschema::Resource::from_contents(schema)
                .map_err(|e| contract_error(e.to_string()))?,
        );
    }
    let schema: Value =
        serde_json::from_slice(include_bytes!("../schemas/manifest-v2.schema.json"))
            .map_err(|e| contract_error(e.to_string()))?;
    let validator = options
        .build(&schema)
        .map_err(|e| contract_error(e.to_string()))?;
    validator
        .validate(value)
        .map_err(|e| contract_error(format!("external corpus manifest schema invalid: {e}")))?;
    validate_identity_projection_runtime_uniqueness(value)?;
    validate_external_selection(value)
}

fn validate_external_selection(value: &Value) -> Result<(), ManifestContractError> {
    let ledger = value["selection_ledger"].as_array().unwrap();
    let mut previous = None;
    let mut direct = BTreeSet::new();
    let mut owned = std::collections::BTreeMap::new();
    let mut generated = BTreeSet::new();
    for entry in ledger {
        let id = entry["case_id"].as_str().unwrap();
        if previous.is_some_and(|prior| prior >= id) {
            return Err(contract_error(
                "selection ledger case IDs must be sorted and unique",
            ));
        }
        previous = Some(id);
        if entry["selection"] == "direct" {
            direct.insert(id);
        }
        let status = entry["registry_status"].as_str().unwrap();
        let outcome = entry["outcome"].as_str().unwrap();
        if (outcome == "generated" && !entry["reason_code"].is_null())
            || (outcome != "generated" && entry["reason_code"].as_str().is_none_or(str::is_empty))
        {
            return Err(contract_error(format!("invalid reason code for {id}")));
        }
        if (status == "implemented" && !matches!(outcome, "generated" | "unavailable"))
            || (status != "implemented" && status != outcome)
        {
            return Err(contract_error(format!("contradictory outcome for {id}")));
        }
        let paths = entry["artifact_paths"].as_array().unwrap();
        if outcome != "generated" && !paths.is_empty() {
            return Err(contract_error(format!(
                "non-generated case owns artifacts: {id}"
            )));
        }
        if outcome == "generated" {
            generated.insert(id);
        }
        let mut previous_path = None;
        for path in paths {
            let path = path.as_str().unwrap();
            if previous_path.is_some_and(|prior| prior >= path) || owned.insert(path, id).is_some()
            {
                return Err(contract_error(
                    "artifact paths must be sorted and uniquely owned",
                ));
            }
            previous_path = Some(path);
        }
    }
    if value["run"]["selector"]["kind"] == "case_ids" {
        let ids: Vec<_> = value["run"]["selector"]["case_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect();
        if ids.windows(2).any(|pair| pair[0] >= pair[1])
            || ids.into_iter().collect::<BTreeSet<_>>() != direct
        {
            return Err(contract_error(
                "selector case IDs must be sorted and equal direct ledger selections",
            ));
        }
    }
    let mut evidenced = BTreeSet::new();
    let mut files = BTreeSet::new();
    for file in value["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        let id = file["case_id"].as_str().unwrap();
        if owned.get(path).copied() != Some(id) || !files.insert(path) {
            return Err(contract_error(
                "file lacks unique generated ledger ownership",
            ));
        }
        evidenced.insert(id);
    }
    if files.len() != owned.len() {
        return Err(contract_error("ledger artifact absent from files"));
    }
    for qualification in value["qualifications"].as_array().unwrap() {
        let id = qualification["case_id"].as_str().unwrap();
        if !generated.contains(id) {
            return Err(contract_error("orphan qualification"));
        }
        evidenced.insert(id);
    }
    if generated != evidenced {
        return Err(contract_error(
            "generated ledger case lacks file or qualification evidence",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod external_manifest_contract_tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        let mut value: Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/cli/curated-manifest-v0.3.json"
        ))
        .unwrap();
        value.as_object_mut().unwrap().remove("skipped_cases");
        value["manifest_schema_version"] = json!("2.0.0");
        value["files"] = json!([]);
        value["qualifications"] = json!([]);
        value["run"] = json!({"kind":"external_corpus","profile":"smoke","seed":1,"include_stress":false,"selector":{"kind":"profile"}});
        value["selection_ledger"] = json!([]);
        value["identity_projection"] = serde_json::to_value(
            crate::identity::project_manifest_identities(
                &EngineResources::embedded(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .unwrap();
        value["identity_projection"]["corpus_definition"] = json!({"state":"verified_bundle","identity":{"schema_version":"1.0.0","definition_id":"test","definition_version":"1.0.0","manifest_sha256":"a".repeat(64),"corpus_definition_sha256":"b".repeat(64),"file_count":1,"total_size_bytes":1}});
        value
    }

    fn entry(id: &str) -> Value {
        json!({"case_id":id,"selection":"direct","registry_status":"planned","outcome":"planned","reason_code":"planned","artifact_paths":[]})
    }

    #[test]
    fn accepts_empty_profile_and_explicit_planned_selection() {
        let mut value = fixture();
        validate_external_corpus_manifest(&value).unwrap();
        value["selection_ledger"] = json!([entry("a"), entry("b")]);
        value["run"]["selector"] = json!({"kind":"case_ids","case_ids":["a","b"]});
        validate_external_corpus_manifest(&value).unwrap();
    }

    #[test]
    fn rejects_identity_version_and_unknown_fields() {
        let original = fixture();
        for pointer in [
            "/identity_projection/corpus_definition/identity",
            "/identity_projection/corpus_definition/identity/manifest_sha256",
            "/manifest_schema_version",
        ] {
            let mut value = original.clone();
            *value.pointer_mut(pointer).unwrap() = json!("invalid");
            assert!(
                validate_external_corpus_manifest(&value).is_err(),
                "{pointer}"
            );
        }
        let mut value = original;
        value["skipped_cases"] = json!([]);
        assert!(validate_external_corpus_manifest(&value).is_err());
    }

    #[test]
    fn rejects_duplicate_unsorted_and_selector_mismatched_ledgers() {
        for entries in [
            json!([entry("a"), entry("a")]),
            json!([entry("b"), entry("a")]),
            json!([entry("b")]),
        ] {
            let mut value = fixture();
            value["selection_ledger"] = entries;
            value["run"]["selector"] = json!({"kind":"case_ids","case_ids":["a"]});
            assert!(validate_external_corpus_manifest(&value).is_err());
        }
    }

    #[test]
    fn rejects_contradictory_and_unevidenced_outcomes() {
        for (status, outcome, reason, paths) in [
            ("planned", "generated", Value::Null, json!([])),
            ("implemented", "generated", Value::Null, json!([])),
            (
                "implemented",
                "unavailable",
                json!("absent"),
                json!(["a.dcm"]),
            ),
            ("planned", "planned", Value::Null, json!([])),
        ] {
            let mut value = fixture();
            let mut item = entry("a");
            item["registry_status"] = json!(status);
            item["outcome"] = json!(outcome);
            item["reason_code"] = reason;
            item["artifact_paths"] = paths;
            value["selection_ledger"] = json!([item]);
            assert!(validate_external_corpus_manifest(&value).is_err());
        }
    }

    #[test]
    fn artifact_ownership_and_qualification_evidence_are_closed() {
        // Relationship checks run only after full schema validation in production.
        let mut value = json!({
            "run":{"selector":{"kind":"profile"}},
            "selection_ledger":[{"case_id":"a","selection":"direct","registry_status":"implemented","outcome":"generated","reason_code":null,"artifact_paths":["a.dcm"]}],
            "files":[{"case_id":"a","path":"a.dcm"}],"qualifications":[]
        });
        validate_external_selection(&value).unwrap();
        for files in [
            json!([]),
            json!([{"case_id":"b","path":"a.dcm"}]),
            json!([{"case_id":"a","path":"a.dcm"},{"case_id":"a","path":"a.dcm"}]),
        ] {
            let mut changed = value.clone();
            changed["files"] = files;
            assert!(validate_external_selection(&changed).is_err());
        }
        value["files"] = json!([]);
        value["selection_ledger"][0]["artifact_paths"] = json!([]);
        value["qualifications"] = json!([{"case_id":"a"}]);
        validate_external_selection(&value).unwrap();
        value["qualifications"][0]["case_id"] = json!("b");
        assert!(validate_external_selection(&value).is_err());
    }

    #[test]
    fn runtime_ids_remain_unique() {
        let value = json!({"identity_projection":{"external_runtime":[{"runtime_id":"x"},{"runtime_id":"x"}]}});
        assert!(validate_identity_projection_runtime_uniqueness(&value).is_err());
    }
}
