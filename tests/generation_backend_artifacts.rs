use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use serde_json::Value;
use synth_dicom_gen::sha256_hex;

#[test]
fn committed_generation_backend_lock_validates_and_has_unique_ids() {
    let schema = read_json("schemas/generation-backend-lock.schema.json");
    let lock = read_json("generation-backends.lock.json");
    let validator = jsonschema::validator_for(&schema).expect("backend lock schema should compile");
    let errors = validator
        .iter_errors(&lock)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "backend lock schema errors: {errors:?}");

    let backends = lock
        .get("backends")
        .and_then(Value::as_array)
        .expect("backend lock must contain backends");
    let mut ids = BTreeSet::new();
    for backend in backends {
        let backend_id = backend
            .get("backend_id")
            .and_then(Value::as_str)
            .expect("backend id should be a string");
        assert!(ids.insert(backend_id), "duplicate backend id {backend_id}");
        if let Some(path) = backend
            .pointer("/dependency_lock/path")
            .and_then(Value::as_str)
        {
            assert!(
                !Path::new(path).is_absolute()
                    && !Path::new(path)
                        .components()
                        .any(|component| matches!(component, Component::ParentDir)),
                "dependency lock paths must be repository-relative"
            );
        }
    }
}

#[test]
fn external_registry_providers_resolve_to_optional_locked_backends() {
    let lock = read_json("generation-backends.lock.json");
    let registry = read_json("cases/registry.json");
    let backends = lock
        .get("backends")
        .and_then(Value::as_array)
        .expect("backend lock must contain backends");

    let external_provider_ids = registry
        .get("cases")
        .and_then(Value::as_array)
        .expect("registry must contain cases")
        .iter()
        .filter(|case| {
            case.pointer("/provider/kind").and_then(Value::as_str) == Some("external_backend")
        })
        .filter_map(|case| case.pointer("/provider/id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();

    for provider_id in external_provider_ids {
        let backend = backends
            .iter()
            .find(|backend| backend.get("backend_id").and_then(Value::as_str) == Some(provider_id))
            .unwrap_or_else(|| panic!("external provider {provider_id} must be locked"));
        assert_eq!(
            backend.get("required_by_default").and_then(Value::as_bool),
            Some(false),
            "external providers must not become default dependencies"
        );
        if matches!(
            provider_id,
            "highdicom_pydicom"
                | "cjxl_jpegxl_lossy_command_writer"
                | "openjph_htj2k_lossy_command_writer"
        ) {
            assert_eq!(
                backend.get("state").and_then(Value::as_str),
                Some("available"),
                "implemented external providers should be available"
            );
            assert!(
                backend.get("discovery").is_some_and(Value::is_object),
                "available providers must have portable discovery policy"
            );
        } else {
            assert_eq!(
                backend.get("state").and_then(Value::as_str),
                Some("planned"),
                "unselected external providers must remain planned"
            );
            assert!(
                backend.get("discovery").is_some_and(Value::is_null),
                "planned providers must not imply a selected launcher"
            );
        }
    }
}

#[test]
fn native_backend_dependency_hash_matches_cargo_lock() {
    let lock = read_json("generation-backends.lock.json");
    let native = lock
        .get("backends")
        .and_then(Value::as_array)
        .and_then(|backends| {
            backends.iter().find(|backend| {
                backend.get("backend_id").and_then(Value::as_str) == Some("rust_native")
            })
        })
        .expect("native backend must be locked");
    let expected = native
        .pointer("/dependency_lock/sha256")
        .and_then(Value::as_str)
        .expect("native dependency hash should be present");
    let cargo_lock = fs::read("Cargo.lock").expect("Cargo.lock should be readable");
    assert_eq!(expected, sha256_hex(&cargo_lock));
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {path}: {error}"))
}
