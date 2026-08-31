use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use dicom_object::open_file;
use serde_json::{Value, json};

pub fn validate_assembly_root(root: &Path, manifest: &Value) -> (usize, Vec<String>) {
    let mut failures = Vec::new();
    if manifest
        .pointer("/run/iod_conformance")
        .and_then(Value::as_str)
        != Some("not_assessed")
    {
        failures.push("structural run must record iod_conformance=not_assessed".into());
    }
    for forbidden in ["case_id", "profile", "template_id", "qualification_status"] {
        if contains_key(manifest, forbidden) {
            failures.push(format!(
                "structural manifest contains forbidden claim {forbidden}"
            ));
        }
    }
    let Some(instances) = manifest.get("instances").and_then(Value::as_array) else {
        return (0, vec!["structural manifest is missing instances".into()]);
    };
    let mut paths = BTreeSet::new();
    for instance in instances {
        let Some(relative) = instance.get("output_path").and_then(Value::as_str) else {
            failures.push("structural instance output_path is missing".into());
            continue;
        };
        if !safe_relative_path(relative) || !paths.insert(relative.to_owned()) {
            failures.push(format!("unsafe or duplicate structural path {relative}"));
            continue;
        }
        if instance.get("iod_conformance").and_then(Value::as_str) != Some("not_assessed") {
            failures.push(format!(
                "structural instance {relative} overclaims IOD conformance"
            ));
        }
        let path = root.join(relative);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            failures.push(format!("structural artifact is missing: {relative}"));
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            failures.push(format!(
                "structural artifact is not a regular file: {relative}"
            ));
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            failures.push(format!("structural artifact cannot be read: {relative}"));
            continue;
        };
        let expected_size = instance.get("size_bytes").and_then(Value::as_u64);
        let expected_sha = instance.get("sha256").and_then(Value::as_str);
        if expected_size != Some(bytes.len() as u64)
            || expected_sha != Some(crate::sha256_hex(&bytes).as_str())
        {
            failures.push(format!("structural artifact identity mismatch: {relative}"));
            continue;
        }
        match open_file(&path) {
            Ok(object) => {
                let sop_class = instance.get("sop_class_uid").and_then(Value::as_str);
                let sop_instance = instance.get("sop_instance_uid").and_then(Value::as_str);
                let transfer = instance.get("transfer_syntax_uid").and_then(Value::as_str);
                if sop_class != Some(object.meta().media_storage_sop_class_uid())
                    || sop_instance != Some(object.meta().media_storage_sop_instance_uid())
                    || transfer != Some(object.meta().transfer_syntax())
                {
                    failures.push(format!("structural Part 10 identity mismatch: {relative}"));
                }
            }
            Err(error) => failures.push(format!(
                "structural Part 10 reopen failed for {relative}: {error}"
            )),
        }
    }
    let declared = paths;
    if let Ok(entries) = walk_files(root) {
        for relative in entries {
            if relative != "manifest.json" && !declared.contains(&relative) {
                failures.push(format!("undeclared structural output file: {relative}"));
            }
        }
    }
    (instances.len(), failures)
}

pub fn assembly_report(manifest: &Value) -> Value {
    let instances = manifest
        .get("instances")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total_bytes = instances
        .iter()
        .filter_map(|instance| instance.get("size_bytes").and_then(Value::as_u64))
        .sum::<u64>();
    let bulk_kinds = instances
        .iter()
        .flat_map(|instance| {
            instance
                .get("bulk")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|bulk| bulk.get("kind").and_then(Value::as_str))
        .fold(
            std::collections::BTreeMap::<String, u64>::new(),
            |mut counts, kind| {
                *counts.entry(kind.to_owned()).or_default() += 1;
                counts
            },
        );
    json!({
        "structural_assembly_report_schema_version": "1.0.0",
        "report_kind": "structural_assembly",
        "iod_conformance": "not_assessed",
        "counts": { "instances": instances.len(), "output_bytes": total_bytes },
        "bulk_kinds": bulk_kinds,
        "instances": instances.iter().map(|instance| json!({
            "instance_id": instance.get("instance_id"), "output_path": instance.get("output_path"),
            "sop_class_uid": instance.get("sop_class_uid"), "transfer_syntax_uid": instance.get("transfer_syntax_uid"),
            "iod_conformance": "not_assessed"
        })).collect::<Vec<_>>(),
        "warnings": ["IOD conformance was not assessed; structural results are excluded from curated and qualified coverage"]
    })
}

fn contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(key) || object.values().any(|value| contains_key(value, key))
        }
        Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        _ => false,
    }
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && !value.contains('\\')
        && !value.contains(':')
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn walk_files(root: &Path) -> std::io::Result<Vec<String>> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                walk(root, &path, out)?;
            } else if metadata.is_file() {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}
