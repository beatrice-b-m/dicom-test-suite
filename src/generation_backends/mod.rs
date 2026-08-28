//! Versioned contracts and policy checks for optional generation backends.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::sha256_hex;

mod discovery;
pub use discovery::{BackendDiscovery, PreparedBackend, discover_prepared_backend};
mod process;
pub use process::{
    BackendInvocation, BackendRun, environment_fingerprint, executable_fingerprint, invoke_backend,
};
mod parametric_map;
pub use parametric_map::{
    ControlledMetadata, FLOAT32_SPEC, FLOAT64_SPEC, ParametricMapDoublePayload,
    ParametricMapFloatPayload, ParametricMapGenerated, ParametricMapGenerationInput,
    ParametricMapIdentities, ParametricMapOutcome, ParametricMapPayload, ParametricMapSampleKind,
    ParametricMapSource, ParametricMapSpec, ParametricMapVariantGenerated,
    ParametricMapVariantOutcome, StandardsProvenance, generate_parametric_map,
    generate_parametric_map_for_spec,
};
mod staging;
pub use staging::{
    OutputLimits, promote_staged_outputs, stage_declared_sources, verify_staged_outputs,
};
mod scoord3d;
pub use scoord3d::{
    Scoord3dGenerated, Scoord3dGenerationInput, Scoord3dIdentities, Scoord3dOutcome,
    generate_scoord3d,
};
mod tid1500;
pub use tid1500::{
    Tid1500Generated, Tid1500GenerationInput, Tid1500Identities, Tid1500Outcome, generate_tid1500,
};
mod wsi_tile_segmentation;
pub use wsi_tile_segmentation::{
    CASE_ID as WSI_TILE_SEGMENTATION_CASE_ID, FRAME_SHA256 as WSI_TILE_SEGMENTATION_FRAME_SHA256,
    FRAME_VALUES as WSI_TILE_SEGMENTATION_FRAME_VALUES,
    MATRIX_SHA256 as WSI_TILE_SEGMENTATION_MATRIX_SHA256,
    OUTPUT_FILE as WSI_TILE_SEGMENTATION_OUTPUT_FILE,
    PAYLOAD_SHA256 as WSI_TILE_SEGMENTATION_PAYLOAD_SHA256,
    RECIPE_ID as WSI_TILE_SEGMENTATION_RECIPE_ID,
    RECIPE_VERSION as WSI_TILE_SEGMENTATION_RECIPE_VERSION,
    SOURCE_CASE_ID as WSI_TILE_SEGMENTATION_SOURCE_CASE_ID,
    SOURCE_FRAME_NUMBERS as WSI_TILE_SEGMENTATION_SOURCE_FRAME_NUMBERS,
    WsiTileSegmentationGenerated, WsiTileSegmentationGenerationInput,
    WsiTileSegmentationIdentities, WsiTileSegmentationOutcome, generate_wsi_tile_segmentation,
};

pub const PROTOCOL_VERSION: &str = "0.1.0";
pub const BACKEND_LOCK_FILE: &str = "generation-backends.lock.json";

const LOCK_SCHEMA: &str = include_str!("../../schemas/generation-backend-lock.schema.json");
const REQUEST_SCHEMA: &str = include_str!("../../schemas/generation-backend-request.schema.json");
const RESPONSE_SCHEMA: &str = include_str!("../../schemas/generation-backend-response.schema.json");

#[derive(Debug)]
pub enum BackendContractError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        label: String,
        source: serde_json::Error,
    },
    Invalid {
        label: String,
        problems: Vec<String>,
    },
}

impl fmt::Display for BackendContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(formatter, "read {}: {source}", path.display()),
            Self::Parse { label, source } => write!(formatter, "parse {label}: {source}"),
            Self::Invalid { label, problems } => {
                write!(formatter, "invalid {label}: {}", problems.join("; "))
            }
        }
    }
}

impl Error for BackendContractError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

pub fn load_backend_lock(repository_root: &Path) -> Result<Value, BackendContractError> {
    let path = repository_root.join(BACKEND_LOCK_FILE);
    let bytes = fs::read(&path).map_err(|source| BackendContractError::Read {
        path: path.clone(),
        source,
    })?;
    let lock = serde_json::from_slice(&bytes).map_err(|source| BackendContractError::Parse {
        label: path.display().to_string(),
        source,
    })?;
    validate_backend_lock(repository_root, &lock)?;
    Ok(lock)
}

pub fn validate_backend_lock(
    repository_root: &Path,
    lock: &Value,
) -> Result<(), BackendContractError> {
    validate_schema("generation backend lock", LOCK_SCHEMA, lock)?;
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    let backends = lock["backends"]
        .as_array()
        .expect("schema checked backends");

    for backend in backends {
        let id = backend["backend_id"].as_str().expect("schema checked id");
        if !ids.insert(id) {
            problems.push(format!("duplicate backend_id {id}"));
        }
        let state = backend["state"].as_str().expect("schema checked state");
        let kind = backend["implementation_kind"]
            .as_str()
            .expect("schema checked implementation kind");
        if state == "available" && backend["dependency_lock"].is_null() {
            problems.push(format!("available backend {id} has no dependency lock"));
        }
        if state == "available" && kind != "rust_native" && backend["discovery"].is_null() {
            problems.push(format!(
                "available external backend {id} has no discovery policy"
            ));
        }
        if state == "planned" && !backend["discovery"].is_null() {
            problems.push(format!(
                "planned backend {id} must not imply executable discovery"
            ));
        }
        if let Some(discovery) = backend["discovery"].as_object() {
            for platform in ["linux", "macos", "windows"] {
                let default_executable = Path::new(
                    discovery["default_relative_executables"][platform]
                        .as_str()
                        .expect("schema checked platform executable"),
                );
                if !is_safe_relative_path(default_executable) {
                    problems.push(format!(
                        "backend {id} {platform} default runtime executable path is unsafe"
                    ));
                }
            }
            for entrypoint in discovery["entrypoint_paths"]
                .as_array()
                .expect("schema checked entrypoint paths")
            {
                let relative =
                    Path::new(entrypoint.as_str().expect("schema checked entrypoint path"));
                if !is_safe_relative_path(relative) {
                    problems.push(format!("backend {id} entrypoint path is unsafe"));
                }
            }
        }

        if let Some(dependency_lock) = backend["dependency_lock"].as_object() {
            let relative = Path::new(
                dependency_lock["path"]
                    .as_str()
                    .expect("schema checked dependency path"),
            );
            if !is_safe_relative_path(relative) {
                problems.push(format!("backend {id} dependency lock path is unsafe"));
                continue;
            }
            let dependency_path = repository_root.join(relative);
            match fs::read(&dependency_path) {
                Ok(bytes) => {
                    let expected = dependency_lock["sha256"]
                        .as_str()
                        .expect("schema checked dependency hash");
                    let actual = sha256_hex(&bytes);
                    if actual != expected {
                        problems.push(format!(
                            "backend {id} dependency lock hash mismatch: expected {expected}, got {actual}"
                        ));
                    }
                }
                Err(error) => problems.push(format!(
                    "backend {id} dependency lock {} is unreadable: {error}",
                    dependency_path.display()
                )),
            }
        }
    }

    invalid_if_any("generation backend lock", problems)
}

pub fn validate_request(request: &Value) -> Result<(), BackendContractError> {
    validate_schema("generation backend request", REQUEST_SCHEMA, request)?;
    let mut problems = Vec::new();
    let mut source_paths = BTreeSet::new();
    let mut source_sop_instance_uids = BTreeSet::new();
    for source in request["sources"]
        .as_array()
        .expect("schema checked sources")
    {
        let relative = Path::new(
            source["relative_path"]
                .as_str()
                .expect("schema checked source path"),
        );
        if !is_safe_relative_path(relative) {
            problems.push(format!(
                "source relative_path {} is unsafe",
                relative.display()
            ));
        }
        if !source_paths.insert(relative.to_path_buf()) {
            problems.push(format!(
                "source relative_path {} is declared more than once",
                relative.display()
            ));
        }
        let sop_instance_uid = source["sop_instance_uid"]
            .as_str()
            .expect("schema checked source SOP Instance UID");
        if !source_sop_instance_uids.insert(sop_instance_uid) {
            problems.push(format!(
                "source SOP Instance UID {sop_instance_uid} is declared more than once"
            ));
        }
    }

    let mut slot_keys = BTreeSet::new();
    let mut slot_uids = BTreeSet::new();
    for slot in request["identities"]["sop_instances"]
        .as_array()
        .expect("schema checked SOP slots")
    {
        let role = slot["role"].as_str().expect("schema checked slot role");
        let index = slot["index"].as_u64().expect("schema checked slot index");
        let uid = slot["uid"].as_str().expect("schema checked slot UID");
        if !slot_keys.insert((role, index)) {
            problems.push(format!(
                "SOP slot role {role} index {index} is declared more than once"
            ));
        }
        if !slot_uids.insert(uid) {
            problems.push(format!("SOP slot UID {uid} is declared more than once"));
        }
    }
    invalid_if_any("generation backend request", problems)
}

pub fn validate_response_for_request(
    request: &Value,
    response: &Value,
) -> Result<(), BackendContractError> {
    validate_request(request)?;
    validate_schema("generation backend response", RESPONSE_SCHEMA, response)?;
    let mut problems = Vec::new();
    for field in ["protocol_version", "request_id", "backend_id"] {
        if request[field] != response[field] {
            problems.push(format!("response {field} does not match request"));
        }
    }

    let mut paths = BTreeSet::new();
    let mut sop_instance_uids = BTreeSet::new();
    let requested_slots = request["identities"]["sop_instances"]
        .as_array()
        .expect("request schema checked SOP slots");
    let requested_sop_instance_uids = requested_slots
        .iter()
        .map(|slot| {
            slot["uid"]
                .as_str()
                .expect("request schema checked slot UID")
        })
        .collect::<BTreeSet<_>>();
    let expected_sop_class = request["case"]["expected_sop_class_uid"]
        .as_str()
        .expect("request schema checked expected SOP Class UID");
    let expected_transfer_syntax = request["case"]["expected_transfer_syntax_uid"]
        .as_str()
        .expect("request schema checked expected Transfer Syntax UID");
    let declared_sources = request["sources"]
        .as_array()
        .expect("request schema checked sources");
    for output in response["outputs"]
        .as_array()
        .expect("schema checked outputs")
    {
        let relative = Path::new(
            output["relative_path"]
                .as_str()
                .expect("schema checked output path"),
        );
        if !is_safe_relative_path(relative) {
            problems.push(format!(
                "output relative_path {} is unsafe",
                relative.display()
            ));
        }
        if !paths.insert(relative.to_path_buf()) {
            problems.push(format!("duplicate output path {}", relative.display()));
        }
        let sop_instance_uid = output["sop_instance_uid"]
            .as_str()
            .expect("schema checked SOP Instance UID");
        if !sop_instance_uids.insert(sop_instance_uid) {
            problems.push(format!(
                "duplicate output SOP Instance UID {sop_instance_uid}"
            ));
        }
        if !requested_sop_instance_uids.contains(sop_instance_uid) {
            problems.push(format!(
                "output SOP Instance UID {sop_instance_uid} does not match a requested SOP slot"
            ));
        }
        let sop_class_uid = output["sop_class_uid"]
            .as_str()
            .expect("schema checked output SOP Class UID");
        if sop_class_uid != expected_sop_class {
            problems.push(format!(
                "output SOP Class UID {sop_class_uid} does not match requested {expected_sop_class}"
            ));
        }
        let transfer_syntax_uid = output["transfer_syntax_uid"]
            .as_str()
            .expect("schema checked output Transfer Syntax UID");
        if transfer_syntax_uid != expected_transfer_syntax {
            problems.push(format!(
                "output Transfer Syntax UID {transfer_syntax_uid} does not match requested {expected_transfer_syntax}"
            ));
        }

        for reference in output["references"]
            .as_array()
            .expect("schema checked output references")
        {
            let matches = declared_sources
                .iter()
                .filter(|source| reference_matches_source(reference, source))
                .count();
            match matches {
                1 => {}
                0 => problems.push(format!(
                    "output {sop_instance_uid} invents reference to undeclared source SOP Instance UID {}",
                    reference["sop_instance_uid"]
                        .as_str()
                        .expect("schema checked reference SOP Instance UID")
                )),
                count => problems.push(format!(
                    "output {sop_instance_uid} reference matches {count} ambiguous request sources"
                )),
            }
        }
    }
    if response["status"].as_str() == Some("generated") {
        for missing in requested_sop_instance_uids.difference(&sop_instance_uids) {
            problems.push(format!(
                "requested SOP slot UID {missing} has no generated output"
            ));
        }
    }
    invalid_if_any("generation backend response", problems)
}

fn reference_matches_source(reference: &Value, source: &Value) -> bool {
    reference["role"] == source["role"]
        && reference["sop_class_uid"] == source["sop_class_uid"]
        && reference["sop_instance_uid"] == source["sop_instance_uid"]
        && reference["series_instance_uid"] == source["series_instance_uid"]
        && reference["frame_numbers"] == source["frame_numbers"]
}

pub fn backend_policy<'a>(lock: &'a Value, backend_id: &str) -> Option<&'a Value> {
    lock.get("backends")?
        .as_array()?
        .iter()
        .find(|backend| backend.get("backend_id").and_then(Value::as_str) == Some(backend_id))
}

pub fn is_safe_relative_path(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    !text.is_empty()
        && !text.contains(['\\', ':'])
        && !text.split('/').any(|component| component.is_empty())
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component.as_os_str() != "."
                && component.as_os_str() != ".."
        })
}

fn validate_schema(
    label: &str,
    schema_source: &str,
    instance: &Value,
) -> Result<(), BackendContractError> {
    let schema =
        serde_json::from_str(schema_source).map_err(|source| BackendContractError::Parse {
            label: format!("embedded {label} schema"),
            source,
        })?;
    let validator =
        jsonschema::validator_for(&schema).map_err(|error| BackendContractError::Invalid {
            label: format!("embedded {label} schema"),
            problems: vec![error.to_string()],
        })?;
    let problems = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    invalid_if_any(label, problems)
}

fn invalid_if_any(label: &str, problems: Vec<String>) -> Result<(), BackendContractError> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(BackendContractError::Invalid {
            label: label.to_string(),
            problems,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn safe_relative_paths_reject_escape_and_ambiguous_components() {
        assert!(is_safe_relative_path(Path::new("nested/object.dcm")));
        assert!(!is_safe_relative_path(Path::new("../object.dcm")));
        assert!(!is_safe_relative_path(Path::new("nested/../object.dcm")));
        assert!(!is_safe_relative_path(Path::new("./object.dcm")));
        assert!(!is_safe_relative_path(Path::new("/absolute/object.dcm")));
        assert!(!is_safe_relative_path(Path::new("nested\\object.dcm")));
        assert!(!is_safe_relative_path(Path::new("C:/object.dcm")));
        assert!(!is_safe_relative_path(Path::new("nested//object.dcm")));
        assert!(!is_safe_relative_path(Path::new("")));
    }

    #[test]
    fn response_outputs_must_match_requested_sop_slots_class_and_transfer_syntax() {
        let request = request_fixture();
        let response = response_fixture();
        validate_response_for_request(&request, &response).expect("fixtures should bind");

        let mut wrong_slot = response.clone();
        wrong_slot["outputs"][0]["sop_instance_uid"] = json!("1.2.826.0.1.3680043.10.543.999");
        let error = validate_response_for_request(&request, &wrong_slot)
            .expect_err("invented SOP Instance UID must fail");
        assert!(error.to_string().contains("requested SOP slot"));

        let mut wrong_class = response.clone();
        wrong_class["outputs"][0]["sop_class_uid"] = json!("1.2.840.10008.5.1.4.1.1.4");
        let error = validate_response_for_request(&request, &wrong_class)
            .expect_err("wrong SOP Class UID must fail");
        assert!(error.to_string().contains("SOP Class UID"));

        let mut wrong_transfer_syntax = response;
        wrong_transfer_syntax["outputs"][0]["transfer_syntax_uid"] = json!("1.2.840.10008.1.2");
        let error = validate_response_for_request(&request, &wrong_transfer_syntax)
            .expect_err("wrong Transfer Syntax UID must fail");
        assert!(error.to_string().contains("Transfer Syntax UID"));
    }

    #[test]
    fn generated_response_must_cover_every_requested_sop_slot() {
        let mut request = request_fixture();
        request["identities"]["sop_instances"]
            .as_array_mut()
            .expect("SOP slots")
            .push(json!({
                "role": "secondary",
                "index": 1,
                "uid": "1.2.826.0.1.3680043.10.543.5"
            }));

        let error = validate_response_for_request(&request, &response_fixture())
            .expect_err("missing requested output must fail");
        assert!(error.to_string().contains("has no generated output"));
    }

    #[test]
    fn response_references_must_match_declared_request_sources() {
        let mut request = request_fixture();
        request["sources"] = json!([declared_source()]);
        let mut response = response_fixture();
        response["outputs"][0]["references"] = json!([declared_reference()]);
        validate_response_for_request(&request, &response)
            .expect("declared source reference should bind");

        response["outputs"][0]["references"][0]["sop_instance_uid"] =
            json!("1.2.826.0.1.3680043.10.543.888");
        let error = validate_response_for_request(&request, &response)
            .expect_err("invented source reference must fail");
        assert!(error.to_string().contains("undeclared source"));
    }

    #[test]
    fn request_rejects_ambiguous_source_and_sop_slot_declarations() {
        let mut duplicate_source = request_fixture();
        let source = declared_source();
        let mut alias = source.clone();
        alias["role"] = json!("alternate_source");
        duplicate_source["sources"] = json!([source, alias]);
        let error = validate_request(&duplicate_source).expect_err("duplicate source must fail");
        assert!(error.to_string().contains("declared more than once"));

        let mut duplicate_slot = request_fixture();
        let mut slot = duplicate_slot["identities"]["sop_instances"][0].clone();
        slot["uid"] = json!("1.2.826.0.1.3680043.10.543.5");
        duplicate_slot["identities"]["sop_instances"]
            .as_array_mut()
            .expect("SOP slots")
            .push(slot);
        let error = validate_request(&duplicate_slot).expect_err("duplicate SOP slot must fail");
        assert!(error.to_string().contains("declared more than once"));
    }

    fn request_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/generation-backend/request.json"
        ))
        .expect("request fixture")
    }

    fn response_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/generation-backend/response.json"
        ))
        .expect("response fixture")
    }

    fn declared_source() -> Value {
        json!({
            "role": "source_image",
            "source_case_id": "geometry/ct/source",
            "relative_path": "geometry/ct/source/instance.dcm",
            "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.40",
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.20",
            "frame_numbers": null
        })
    }

    fn declared_reference() -> Value {
        json!({
            "role": "source_image",
            "relationship": "source_image",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.40",
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.20",
            "frame_numbers": null
        })
    }
}
