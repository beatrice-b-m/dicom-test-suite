//! Shared, version-aware manifest loading before validation or reporting.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::engine_resources::EngineResources;

const LEGACY_MANIFEST_ID: &str = "https://dicom-test-suite.local/schemas/manifest.schema.json";
const CASE_REGISTRY_ID: &str = "https://dicom-test-suite.local/schemas/case-registry.schema.json";
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
            CASE_REGISTRY_ID,
            jsonschema::Resource::from_contents(
                serde_json::from_slice::<Value>(include_bytes!(
                    "../schemas/case-registry.schema.json"
                ))
                .map_err(|error| contract_error(error.to_string()))?,
            )
            .map_err(|error| contract_error(error.to_string()))?,
        )
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
            CASE_REGISTRY_ID,
            include_bytes!("../schemas/case-registry.schema.json").as_slice(),
        ),
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
    let mut edges = std::collections::BTreeMap::new();
    let profile = value["run"]["profile"].as_str().unwrap();
    let include_stress = value["run"]["include_stress"].as_bool().unwrap();
    if include_stress && profile != "all" {
        return Err(contract_error("include_stress requires all profile"));
    }
    for entry in ledger {
        let id = entry["case_id"].as_str().unwrap();
        let definition = &entry["case_definition"];
        if definition["case_id"] != entry["case_id"]
            || definition["status"] != entry["registry_status"]
        {
            return Err(contract_error(
                "ledger differs from captured case definition",
            ));
        }
        if previous.is_some_and(|prior| prior >= id) {
            return Err(contract_error(
                "selection ledger case IDs must be sorted and unique",
            ));
        }
        previous = Some(id);
        let dependencies = entry["dependency_case_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<Vec<_>>();
        if dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(contract_error("dependency IDs must be sorted and unique"));
        }
        edges.insert(id, dependencies);
        if entry["selection"] == "direct" {
            let profiles = definition["profiles"].as_array().unwrap();
            let in_scope = if profile == "all" {
                profiles.iter().any(|p| {
                    matches!(p.as_str(), Some("smoke" | "core" | "extended"))
                        || (include_stress && p == "stress")
                })
            } else {
                profiles.iter().any(|p| p == profile)
            };
            if !in_scope {
                return Err(contract_error(
                    "direct case definition outside selector profile",
                ));
            }
            direct.insert(id);
        }
        let status = entry["registry_status"].as_str().unwrap();
        let outcome = entry["outcome"].as_str().unwrap();
        if status != "implemented" {
            let expected = definition["skip"]["reason_code"]
                .as_str()
                .or_else(|| definition["blockers"][0]["code"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("case_{status}"));
            if entry["reason_code"] != expected {
                return Err(contract_error(
                    "outcome reason differs from captured definition",
                ));
            }
        }
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
    if direct.is_empty() || generated.is_empty() {
        return Err(contract_error(
            "published corpus requires direct selection and generated evidence",
        ));
    }
    if edges.values().flatten().any(|id| !edges.contains_key(id)) {
        return Err(contract_error("selection dependency absent from ledger"));
    }
    let rows = ledger
        .iter()
        .map(|row| (row["case_id"].as_str().unwrap(), row))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (owner, dependencies) in &edges {
        for dependency in dependencies {
            let owner = rows[owner];
            let dependency = rows[dependency];
            if owner["registry_status"] != "implemented"
                || dependency["registry_status"] != "implemented"
            {
                return Err(contract_error("dependency endpoints must be implemented"));
            }
            let has = |row: &Value, profile: &str| {
                row["case_definition"]["profiles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p == profile)
            };
            let ordinary = ["smoke", "core", "extended"].iter().any(|p| has(owner, p));
            let forbidden = (ordinary
                && ["legacy", "stress", "negative", "fuzz"]
                    .iter()
                    .any(|p| has(dependency, p)))
                || (has(owner, "legacy")
                    && ["stress", "negative", "fuzz"]
                        .iter()
                        .any(|p| has(dependency, p)))
                || (has(owner, "stress")
                    && ["legacy", "negative", "fuzz"]
                        .iter()
                        .any(|p| has(dependency, p)))
                || (has(owner, "negative")
                    && ["legacy", "stress", "fuzz"]
                        .iter()
                        .any(|p| has(dependency, p)))
                || (has(owner, "fuzz")
                    && ["legacy", "stress", "negative"]
                        .iter()
                        .any(|p| has(dependency, p)));
            if forbidden {
                return Err(contract_error("dependency profile scope leakage"));
            }
        }
    }
    let mut remaining = edges.keys().copied().collect::<BTreeSet<_>>();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .copied()
            .filter(|id| {
                edges[id]
                    .iter()
                    .all(|dependency| !remaining.contains(dependency))
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(contract_error("selection dependency cycle"));
        }
        for id in ready {
            remaining.remove(id);
        }
    }
    let mut reached = BTreeSet::new();
    let mut pending = direct.iter().copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if reached.insert(id) {
            pending.extend(edges[id].iter().copied());
        }
    }
    if reached.len() != ledger.len() {
        return Err(contract_error("orphan dependency ledger row"));
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
        let profiles = |value: &Value| -> Result<BTreeSet<String>, ManifestContractError> {
            value
                .as_array()
                .ok_or_else(|| contract_error("file profile membership must be an array"))?
                .iter()
                .map(|profile| {
                    profile
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| contract_error("profile must be a string"))
                })
                .collect()
        };
        if profiles(&file["profile_membership"])?
            != profiles(&rows[id]["case_definition"]["profiles"])?
        {
            return Err(contract_error(
                "file profile membership differs from captured case definition",
            ));
        }
        evidenced.insert(id);
    }
    if files.len() != owned.len() {
        return Err(contract_error("ledger artifact absent from files"));
    }
    let mut qualified = BTreeSet::new();
    for qualification in value["qualifications"].as_array().unwrap() {
        let id = qualification["case_id"].as_str().unwrap();
        if !generated.contains(id) || !qualified.insert(id) {
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
        // A real bounded native output supplies schema-valid evidence; the frozen
        // reader fixtures intentionally contain no generated files.
        static FILE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        let file = FILE
            .get_or_init(|| {
                let root = std::env::temp_dir().join(format!(
                    "synth-dicom-gen-contract-evidence-{}",
                    std::process::id()
                ));
                assert!(!root.exists());
                let run = crate::prepare_generation_run(crate::GenerateOptions {
                    profile: "smoke".into(),
                    out_dir: root.clone(),
                    seed: 1,
                    include_stress: false,
                })
                .unwrap();
                crate::write_generation_run(&run).unwrap();
                let manifest: Value =
                    serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap())
                        .unwrap();
                std::fs::remove_dir_all(root).unwrap();
                manifest["files"][0].clone()
            })
            .clone();
        value["files"] = json!([file]);
        value["qualifications"] = json!([]);
        value["run"] = json!({"kind":"external_corpus","profile":"smoke","seed":1,"include_stress":false,"selector":{"kind":"profile"}});
        value["selection_ledger"] = json!([{"case_id":file["case_id"],"selection":"direct","registry_status":"implemented","outcome":"generated","reason_code":null,"artifact_paths":[file["path"]],"dependency_case_ids":[]}]);
        let registry: Value =
            serde_json::from_slice(include_bytes!("../cases/registry.json")).unwrap();
        value["selection_ledger"][0]["case_definition"] = registry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["case_id"] == file["case_id"])
            .unwrap()
            .clone();
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
        let registry: Value =
            serde_json::from_slice(include_bytes!("../cases/registry.json")).unwrap();
        let mut definition = registry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["status"] == "planned")
            .unwrap()
            .clone();
        definition["case_id"] = json!(id);
        definition["profiles"] = json!(["smoke"]);
        json!({"case_id":id,"case_definition":definition,"selection":"direct","registry_status":"planned","outcome":"planned","reason_code":definition["blockers"][0]["code"],"artifact_paths":[],"dependency_case_ids":[]})
    }

    #[test]
    fn accepts_mixed_selection_and_rejects_empty_publication() {
        let mut value = fixture();
        validate_external_corpus_manifest(&value).unwrap();
        let generated = value["selection_ledger"][0].clone();
        value["selection_ledger"] = json!([generated, entry("test/a"), entry("test/b")]);
        value["run"]["selector"] =
            json!({"kind":"case_ids","case_ids":[generated["case_id"],"test/a","test/b"]});
        validate_external_corpus_manifest(&value).unwrap();
        value["selection_ledger"] = json!([]);
        value["files"] = json!([]);
        assert!(validate_external_corpus_manifest(&value).is_err());
        for (status, skip, blockers, expected) in [
            (
                "skipped",
                json!({"reason_code":"explicit_skip","message":"test","recheck_phase":null}),
                json!([{"code":"independent_decoder_unavailable","message":"test","recheck_phase":"phase-5"}]),
                "explicit_skip",
            ),
            ("deprecated", Value::Null, json!([]), "case_deprecated"),
        ] {
            let mut value = fixture();
            let mut row = entry("test/reason");
            row["registry_status"] = json!(status);
            row["outcome"] = json!(status);
            row["case_definition"]["status"] = json!(status);
            row["case_definition"]["skip"] = skip;
            row["case_definition"]["blockers"] = blockers;
            row["reason_code"] = json!(expected);
            value["selection_ledger"].as_array_mut().unwrap().push(row);
            validate_external_corpus_manifest(&value).unwrap();
            value["selection_ledger"][1]["reason_code"] = json!("wrong_reason");
            assert!(validate_external_corpus_manifest(&value).is_err());
        }
    }

    #[test]
    fn rejects_identity_version_and_unknown_fields() {
        // External names must not select historical corpus policies; all generic
        // structure and typed evidence remain exactly the legacy base contract.
        let legacy: Value =
            serde_json::from_slice(include_bytes!("../schemas/manifest.schema.json")).unwrap();
        let external: Value =
            serde_json::from_slice(include_bytes!("../schemas/manifest-v2.schema.json")).unwrap();
        assert_eq!(
            crate::sha256_hex(include_bytes!("../schemas/manifest.schema.json")),
            "8adc86156169dcb96a46565b246a4a4fc9dd36222036f4e22001c4002b2d635b"
        );
        assert_eq!(
            crate::sha256_hex(include_bytes!("../schemas/manifest-v1.schema.json")),
            "9a35174d28e2040ca84fe1d4936dc9c450b54a03ab0f0257cb410ded92bcbb9f"
        );
        let mut expected = legacy["$defs"]["file"].clone();
        let branches = expected["allOf"].as_array_mut().unwrap();
        assert_eq!(branches.len(), 37);
        assert!(branches[1..]
            .iter()
            .all(|b| b["if"]["properties"].get("case_id").is_some()));
        branches.truncate(1);
        fn rebase(value: &mut Value) {
            match value {
                Value::Object(fields) => {
                    for (key, value) in fields {
                        if key == "$ref" && value.as_str().is_some_and(|v| v.starts_with("#/")) {
                            *value =
                                json!(format!("{LEGACY_MANIFEST_ID}{}", value.as_str().unwrap()));
                        } else {
                            rebase(value);
                        }
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        rebase(value);
                    }
                }
                _ => {}
            }
        }
        rebase(&mut expected);
        expected["properties"]["expected_us_multiframe"]["$ref"] =
            json!("#/$defs/expected_us_multiframe");
        expected["properties"]["expected_nm_multiframe"]["$ref"] =
            json!("#/$defs/expected_nm_multiframe");
        expected["allOf"]
            .as_array_mut()
            .unwrap()
            .push(external["$defs"]["external_file"]["allOf"][1].clone());
        assert_eq!(external["$defs"]["external_file"], expected);
        assert_eq!(
            external["properties"]["files"]["items"]["$ref"],
            "#/$defs/external_file"
        );
        let value = fixture();
        for (key, bad) in [
            ("sha256", json!("bad")),
            ("size_bytes", json!(-1)),
            ("standards_evidence", json!([{}])),
            ("validation", json!({})),
            ("path", json!("../escape")),
            ("expected_pet_activity", json!({})),
            ("expected_icc_profile", json!({})),
            ("unexpected", json!(true)),
        ] {
            let mut corrupt = value.clone();
            corrupt["files"][0][key] = bad;
            assert!(
                validate_external_corpus_manifest(&corrupt).is_err(),
                "{key}"
            );
        }
        let mut generic_schema = external["$defs"]["external_file"].clone();
        generic_schema["$defs"] = json!({
            "expected_us_multiframe": external["$defs"]["expected_us_multiframe"].clone(),
            "expected_us_frame": external["$defs"]["expected_us_frame"].clone(),
            "expected_nm_multiframe": external["$defs"]["expected_nm_multiframe"].clone(),
            "expected_nm_energy_window": external["$defs"]["expected_nm_energy_window"].clone(),
            "expected_nm_detector": external["$defs"]["expected_nm_detector"].clone(),
            "expected_nm_frame_dimension": external["$defs"]["expected_nm_frame_dimension"].clone()
        });
        let generic_validator = jsonschema::options()
            .with_resource(
                LEGACY_MANIFEST_ID,
                jsonschema::Resource::from_contents(legacy.clone()).unwrap(),
            )
            .build(&generic_schema)
            .unwrap();
        let mut legacy_file = legacy.clone();
        legacy_file["$ref"] = json!("#/$defs/file");
        for key in [
            "type",
            "required",
            "properties",
            "additionalProperties",
            "allOf",
        ] {
            legacy_file.as_object_mut().unwrap().remove(key);
        }
        let historical_validator = jsonschema::validator_for(&legacy_file).unwrap();
        for case_id in [
            "classic/pet/rescaled_activity_explicit_le",
            "classic/nm/multiframe_explicit_le",
            "classic/xa/monoplane_explicit_le",
            "vl/photo/rgb_icc_profile_explicit_le",
        ] {
            let mut file = value["files"][0].clone();
            file["case_id"] = json!(case_id);
            assert!(generic_validator.is_valid(&file), "{case_id}");
            assert!(!historical_validator.is_valid(&file), "{case_id}");
        }
        for field in [
            "dicom",
            "uids",
            "references",
            "expected_capabilities",
            "expected_semantics",
            "expected_visual_checks",
            "validation",
            "known_stressors",
            "standards_evidence",
        ] {
            let mut file = value["files"][0].clone();
            file.as_object_mut().unwrap().remove(field);
            assert!(!generic_validator.is_valid(&file), "missing {field}");
        }
        let mut invalid = value["files"][0].clone();
        invalid["validity"] = json!("expected_invalid");
        let forbidden = [
            "dicom",
            "uids",
            "image",
            "pixel_data",
            "generation_backend",
            "references",
            "expected_capabilities",
            "expected_semantics",
            "expected_visual_checks",
            "validation",
            "known_stressors",
        ];
        for field in forbidden {
            invalid.as_object_mut().unwrap().remove(field);
        }
        invalid["provider"] = json!({"kind":"mutation_layer", "id":"synthetic_test"});
        let hash = "00".repeat(32);
        invalid["negative_evidence"] = json!({
            "contract_version":"0.1.0", "recipe_version":"0.1.0",
            "source":{"case_id":"synthetic/source", "sha256":hash, "transfer_syntax_uid":"1.2.840.10008.1.2.1", "size_bytes":1},
            "source_shape":"synthetic schema fixture",
            "mutation_steps":[{"ordinal":1, "mutation_id":"synthetic", "parameters":{"offset":0}, "changed_byte_ranges":[{"source":{"start":0,"end":1},"output":{"start":0,"end":1}}], "source_sha256":hash, "output_sha256":hash, "expected_failure_layer":"semantic_validation", "acceptable_outcomes":["validation_failure"]}],
            "probe":{"kind":"same_project_bounded_parser_classifier", "independence":"same_project", "outcome":"validation_failure", "detail":"schema fixture only"},
            "unacceptable_outcomes":["timeout","crash","hang"], "final_sha256":hash
        });
        assert!(generic_validator.is_valid(&invalid));
        for field in forbidden {
            let mut mixed = invalid.clone();
            mixed[field] = value["files"][0][field].clone();
            assert!(!generic_validator.is_valid(&mixed), "forbidden {field}");
        }

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
        for (pointer, replacement) in [
            (
                "/selection_ledger/0/case_definition/case_id",
                json!("test/other"),
            ),
            (
                "/selection_ledger/0/case_definition/status",
                json!("planned"),
            ),
            (
                "/selection_ledger/0/case_definition/profiles",
                json!(["negative"]),
            ),
        ] {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                validate_external_corpus_manifest(&changed).is_err(),
                "{pointer}"
            );
        }
        value["skipped_cases"] = json!([]);
        assert!(validate_external_corpus_manifest(&value).is_err());
    }

    #[test]
    fn rejects_duplicate_unsorted_and_selector_mismatched_ledgers() {
        for entries in [
            json!([entry("test/a"), entry("test/a")]),
            json!([entry("test/b"), entry("test/a")]),
            json!([entry("test/b")]),
        ] {
            let mut value = fixture();
            value["selection_ledger"] = entries;
            value["run"]["selector"] = json!({"kind":"case_ids","case_ids":["test/a"]});
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
            let mut item = entry("test/a");
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
        let mut original = fixture();
        original["selection_ledger"][0]["case_definition"]["profiles"] = json!(["smoke", "core"]);
        original["files"][0]["profile_membership"] = json!(["smoke", "core"]);
        validate_external_corpus_manifest(&original).unwrap();
        for profiles in [
            json!([]),
            json!(["smoke"]),
            json!(["stress"]),
            json!(["smoke", "core", "extended", "negative"]),
        ] {
            let mut bad = original.clone();
            bad["files"][0]["profile_membership"] = profiles;
            assert!(validate_external_corpus_manifest(&bad).is_err());
        }
        let mut reordered = original.clone();
        reordered["files"][0]["profile_membership"]
            .as_array_mut()
            .unwrap()
            .reverse();
        assert_ne!(
            reordered["files"][0]["profile_membership"],
            original["files"][0]["profile_membership"]
        );
        validate_external_corpus_manifest(&reordered).unwrap();

        // Relationship checks run only after full schema validation in production.
        let mut value = json!({
            "run":{"profile":"smoke","include_stress":false,"selector":{"kind":"profile"}},
            "selection_ledger":[{"case_id":"a","case_definition":{"case_id":"a","status":"implemented","profiles":["smoke"]},"selection":"direct","registry_status":"implemented","outcome":"generated","reason_code":null,"artifact_paths":["a.dcm"],"dependency_case_ids":[]}],
            "files":[{"case_id":"a","path":"a.dcm","profile_membership":["smoke"]}],"qualifications":[]
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
        value["qualifications"] = json!([{"case_id":"a"},{"case_id":"a"}]);
        assert!(validate_external_selection(&value).is_err());
        value["qualifications"] = json!([{"case_id":"a"}]);
        value["qualifications"][0]["case_id"] = json!("b");
        assert!(validate_external_selection(&value).is_err());
    }

    #[test]
    fn dependency_graph_is_closed_acyclic_and_reachable() {
        let mut value = fixture();
        let direct = value["selection_ledger"][0]["case_id"].clone();
        let mut dependency = entry("test/dependency");
        dependency["registry_status"] = json!("implemented");
        dependency["case_definition"] = value["selection_ledger"][0]["case_definition"].clone();
        dependency["case_definition"]["case_id"] = json!("test/dependency");
        dependency["outcome"] = json!("unavailable");
        dependency["reason_code"] = json!("unavailable_backend");
        dependency["selection"] = json!("dependency");
        value["selection_ledger"][0]["dependency_case_ids"] = json!(["test/dependency"]);
        value["selection_ledger"]
            .as_array_mut()
            .unwrap()
            .push(dependency);
        validate_external_corpus_manifest(&value).unwrap();
        // Exercise every isolation pairing independently of case-specific schema
        // requirements. Production reaches this only after schema validation.
        for owner in ["smoke", "legacy", "stress", "negative", "fuzz"] {
            for target in ["smoke", "legacy", "stress", "negative", "fuzz"] {
                let mut changed = value.clone();
                changed["run"]["profile"] = json!(owner);
                changed["selection_ledger"][0]["case_definition"]["profiles"] = json!([owner]);
                changed["files"][0]["profile_membership"] = json!([owner]);
                changed["selection_ledger"][1]["case_definition"]["profiles"] = json!([target]);
                let allowed = target == "smoke" || (owner != "smoke" && owner == target);
                assert_eq!(
                    validate_external_selection(&changed).is_ok(),
                    allowed,
                    "{owner} -> {target}"
                );
            }
        }
        for direct_target in [false, true] {
            let mut changed = value.clone();
            if direct_target {
                changed["selection_ledger"][1]["selection"] = json!("direct");
            }
            changed["run"]["profile"] = json!("all");
            changed["run"]["include_stress"] = json!(true);
            changed["selection_ledger"][1]["case_definition"]["profiles"] = json!(["stress"]);
            assert!(validate_external_selection(&changed).is_err());
            changed["selection_ledger"][1]["case_definition"]["profiles"] = json!(["smoke"]);
            changed["selection_ledger"][1]["registry_status"] = json!("planned");
            changed["selection_ledger"][1]["case_definition"] =
                entry("test/dependency")["case_definition"].clone();
            changed["selection_ledger"][1]["outcome"] = json!("planned");
            changed["selection_ledger"][1]["reason_code"] =
                changed["selection_ledger"][1]["case_definition"]["blockers"][0]["code"].clone();
            assert!(validate_external_corpus_manifest(&changed).is_err());
            changed["selection_ledger"][1]["reason_code"] = json!("fabricated_reason");
            changed["selection_ledger"][0]["dependency_case_ids"] = json!([]);
            changed["selection_ledger"][1]["selection"] = json!("direct");
            assert!(validate_external_corpus_manifest(&changed).is_err());
        }
        for (pointer, replacement) in [
            ("/selection_ledger/0/dependency_case_ids", json!([])),
            (
                "/selection_ledger/0/dependency_case_ids",
                json!(["test/missing"]),
            ),
            (
                "/selection_ledger/0/dependency_case_ids",
                json!(["test/dependency", "test/dependency"]),
            ),
            ("/selection_ledger/1/dependency_case_ids", json!([direct])),
            ("/selection_ledger/0/selection", json!("dependency")),
        ] {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                validate_external_corpus_manifest(&changed).is_err(),
                "{pointer}"
            );
        }
    }

    #[test]
    fn runtime_ids_remain_unique() {
        let value = json!({"identity_projection":{"external_runtime":[{"runtime_id":"x"},{"runtime_id":"x"}]}});
        assert!(validate_identity_projection_runtime_uniqueness(&value).is_err());
    }
}
