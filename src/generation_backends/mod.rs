//! Versioned contracts and policy checks for optional generation backends.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::sha256_hex;

mod process;
pub use process::{BackendInvocation, BackendRun, invoke_backend};
mod staging;
pub use staging::{OutputLimits, promote_staged_outputs, verify_staged_outputs};

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
    }
    invalid_if_any("generation backend response", problems)
}

pub fn backend_policy<'a>(lock: &'a Value, backend_id: &str) -> Option<&'a Value> {
    lock.get("backends")?
        .as_array()?
        .iter()
        .find(|backend| backend.get("backend_id").and_then(Value::as_str) == Some(backend_id))
}

pub fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
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

    #[test]
    fn safe_relative_paths_reject_escape_and_ambiguous_components() {
        assert!(is_safe_relative_path(Path::new("nested/object.dcm")));
        assert!(!is_safe_relative_path(Path::new("../object.dcm")));
        assert!(!is_safe_relative_path(Path::new("nested/../object.dcm")));
        assert!(!is_safe_relative_path(Path::new("./object.dcm")));
        assert!(!is_safe_relative_path(Path::new("/absolute/object.dcm")));
        assert!(!is_safe_relative_path(Path::new("")));
    }
}
