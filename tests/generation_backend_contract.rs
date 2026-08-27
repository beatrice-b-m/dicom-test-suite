use std::fs;
use std::path::Path;

use dicom_test_suite::generation_backends::{
    backend_policy, load_backend_lock, validate_request, validate_response_for_request,
};
use serde_json::{Value, json};

#[test]
fn committed_backend_lock_loads_with_dependency_verification() {
    let lock = load_backend_lock(Path::new(".")).expect("committed backend lock should load");
    assert_eq!(
        backend_policy(&lock, "rust_native").and_then(|backend| backend["state"].as_str()),
        Some("available")
    );
    assert_eq!(
        backend_policy(&lock, "highdicom_pydicom").and_then(|backend| backend["state"].as_str()),
        Some("planned")
    );
    let highdicom = backend_policy(&lock, "highdicom_pydicom")
        .expect("highdicom/pydicom policy must be present");
    assert_eq!(
        highdicom
            .pointer("/dependency_lock/format")
            .and_then(Value::as_str),
        Some("uv-lock-v1")
    );
    assert_eq!(
        highdicom
            .pointer("/dependency_lock/path")
            .and_then(Value::as_str),
        Some("generation-backends/highdicom-pydicom/uv.lock")
    );
    assert!(
        highdicom["blockers"]
            .as_array()
            .expect("blockers must be an array")
            .iter()
            .all(|blocker| !blocker
                .as_str()
                .unwrap_or_default()
                .contains("runtime manager")),
        "the explicit uv decision must remove the runtime-manager blocker"
    );
}

#[test]
fn response_must_echo_request_identity_and_use_safe_unique_outputs() {
    let request = fixture("tests/fixtures/generation-backend/request.json");
    let response = fixture("tests/fixtures/generation-backend/response.json");
    validate_request(&request).expect("request fixture should be valid");
    validate_response_for_request(&request, &response).expect("response fixture should be valid");

    let mut mismatched = response.clone();
    mismatched["request_id"] =
        json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    assert!(
        validate_response_for_request(&request, &mismatched)
            .unwrap_err()
            .to_string()
            .contains("request_id")
    );

    let mut escaping = response;
    escaping["outputs"][0]["relative_path"] = json!("nested/../escaped.dcm");
    assert!(
        validate_response_for_request(&request, &escaping)
            .unwrap_err()
            .to_string()
            .contains("unsafe")
    );
}

fn fixture(path: &str) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {path}: {error}"))
}
