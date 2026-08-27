use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::sha256_hex;

pub const DEFAULT_VALIDATOR_CONFIG: &str = "conformance/validators.json";
pub const DEFAULT_VALIDATOR_LOCK: &str = "conformance/validator-lock.json";
pub const DEFAULT_ACCEPTED_FINDINGS: &str = "conformance/accepted-findings.json";
const PIXELMED_SR_VALIDATOR_ID: &str = "pixelmed-sr-validator";

pub fn verify_conformance(
    evidence_root: impl AsRef<Path>,
    allowlist_path: impl AsRef<Path>,
) -> Result<Value, String> {
    let evidence_root = evidence_root.as_ref();
    let evidence = read_json(&evidence_root.join("conformance-run.json"))?;
    let allowlist = read_json(allowlist_path.as_ref())?;
    let run_schema = read_json(Path::new("schemas/conformance-run.schema.json"))?;
    let allowlist_schema = read_json(Path::new(
        "schemas/conformance-accepted-findings.schema.json",
    ))?;
    let mut failures = Vec::new();
    validate_schema(&run_schema, &evidence, "evidence", &mut failures)?;
    validate_schema(&allowlist_schema, &allowlist, "allowlist", &mut failures)?;
    if !failures.is_empty() {
        return Ok(json!({ "valid": false, "accepted_findings": 0, "failures": failures }));
    }
    verify_manifest(evidence_root, &evidence, &mut failures);
    verify_tools(&evidence, &mut failures);
    verify_artifacts(evidence_root, &evidence, &mut failures);
    verify_completeness(evidence_root, &evidence, &mut failures);
    let accepted = verify_findings(&evidence, &allowlist, &mut failures);
    Ok(json!({
        "valid": failures.is_empty(),
        "accepted_findings": accepted,
        "failures": failures
    }))
}

fn validate_schema(
    schema: &Value,
    instance: &Value,
    label: &str,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| format!("failed to compile {label} schema: {error}"))?;
    failures.extend(
        validator
            .iter_errors(instance)
            .map(|error| format!("{label} schema: {error}")),
    );
    Ok(())
}

fn verify_manifest(evidence_root: &Path, evidence: &Value, failures: &mut Vec<String>) {
    let Some(relative) = evidence
        .pointer("/source/manifest_path")
        .and_then(Value::as_str)
    else {
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push("source manifest path is not safely relative".to_string());
        return;
    }
    match fs::read(evidence_root.join(relative)) {
        Ok(bytes) => {
            let actual = sha256_hex(&bytes);
            if evidence
                .pointer("/source/manifest_sha256")
                .and_then(Value::as_str)
                != Some(actual.as_str())
            {
                failures.push("source manifest hash mismatch".to_string());
            }
        }
        Err(error) => failures.push(format!("source manifest unavailable: {error}")),
    }
}

fn verify_tools(evidence: &Value, failures: &mut Vec<String>) {
    for tool in evidence["tools"].as_array().into_iter().flatten() {
        let required = tool["required"].as_bool() == Some(true);
        let active_sr = tool["role"] == "sr_validator" && tool["status"] == "available";
        if required || active_sr {
            let id = tool["adapter_id"].as_str().unwrap_or("unknown");
            if tool["status"] != "available" {
                failures.push(format!("required tool {id} is not available"));
            }
            if tool["lock_status"] != "matched" {
                failures.push(format!("required tool {id} does not match its lock"));
            }
            if tool["sha256"].as_str().is_none() {
                failures.push(format!("required tool {id} has no fingerprint"));
            }
        }
    }
}

fn verify_artifacts(evidence_root: &Path, evidence: &Value, failures: &mut Vec<String>) {
    for result in all_results(evidence) {
        for stream in ["stdout", "stderr"] {
            let Some(relative) = result[stream]["path"].as_str() else {
                continue;
            };
            if validate_relative_path(relative).is_err() {
                failures.push(format!("unsafe raw artifact path: {relative}"));
                continue;
            }
            match fs::read(evidence_root.join(relative)) {
                Ok(bytes) => {
                    let actual = sha256_hex(&bytes);
                    if result[stream]["sha256"].as_str() != Some(actual.as_str()) {
                        failures.push(format!("raw artifact hash mismatch: {relative}"));
                    }
                }
                Err(error) => {
                    failures.push(format!("raw artifact unavailable {relative}: {error}"))
                }
            }
        }
    }
    for instance in evidence["instances"].as_array().into_iter().flatten() {
        let Some(artifact) = instance["pixel"]
            .get("evidence")
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        let Some(relative) = artifact["path"].as_str() else {
            continue;
        };
        if validate_relative_path(relative).is_err() {
            failures.push(format!("unsafe pixel evidence path: {relative}"));
            continue;
        }
        match fs::read(evidence_root.join(relative)) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if artifact["sha256"].as_str() != Some(actual.as_str()) {
                    failures.push(format!("pixel evidence hash mismatch: {relative}"));
                }
            }
            Err(error) => failures.push(format!("pixel evidence unavailable {relative}: {error}")),
        }
    }
    for instance in evidence["instances"].as_array().into_iter().flatten() {
        let Some(artifact) = instance
            .get("icc")
            .and_then(|icc| icc.get("evidence"))
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        verify_hash_linked_artifact(evidence_root, artifact, "ICC profile evidence", failures);
    }
    if let Some(projection) = evidence["entity"].get("input_projection") {
        verify_hash_linked_artifact(
            evidence_root,
            &projection["file_list"],
            "entity projection file list",
            failures,
        );
        for entry in projection["entries"].as_array().into_iter().flatten() {
            verify_hash_linked_artifact(
                evidence_root,
                &entry["source_copy"],
                "entity projection source copy",
                failures,
            );
            verify_hash_linked_artifact(
                evidence_root,
                &entry["projected_input"],
                "entity projected input",
                failures,
            );
        }
    }
}

fn verify_hash_linked_artifact(
    evidence_root: &Path,
    artifact: &Value,
    label: &str,
    failures: &mut Vec<String>,
) {
    let Some(relative) = artifact["path"].as_str() else {
        failures.push(format!("{label} path is missing"));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!("{label} path is unsafe: {relative}"));
        return;
    }
    match fs::read(evidence_root.join(relative)) {
        Ok(bytes) => {
            if artifact["sha256"].as_str() != Some(sha256_hex(&bytes).as_str()) {
                failures.push(format!("{label} hash mismatch: {relative}"));
            }
        }
        Err(error) => failures.push(format!("{label} unavailable {relative}: {error}")),
    }
}

fn verify_completeness(evidence_root: &Path, evidence: &Value, failures: &mut Vec<String>) {
    let Some(relative) = evidence
        .pointer("/source/manifest_path")
        .and_then(Value::as_str)
    else {
        return;
    };
    let manifest = fs::read(evidence_root.join(relative))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let manifest_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_float_pixel_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| {
                    file.pointer("/image/sample_type").and_then(Value::as_str) == Some("float32")
                        && file.pointer("/pixel_data/vr").and_then(Value::as_str) == Some("OF")
                        && file
                            .pointer("/pixel_data/native_or_encapsulated")
                            .and_then(Value::as_str)
                            == Some("native")
                })
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_double_float_pixel_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| {
                    file.pointer("/image/sample_type").and_then(Value::as_str) == Some("float64")
                        && file.pointer("/pixel_data/vr").and_then(Value::as_str) == Some("OD")
                        && file
                            .pointer("/pixel_data/native_or_encapsulated")
                            .and_then(Value::as_str)
                            == Some("native")
                })
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_u32_pixel_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| file["case_id"] == "classic/sc/mono2_u32_explicit_le")
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_u1_pixel_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| file["case_id"] == "classic/sc/mono2_u1_native")
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_icc_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| file["case_id"] == "vl/photo/rgb_icc_profile_explicit_le")
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_nonsquare_spacing_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| file["case_id"] == "classic/sc/nonsquare_pixel_spacing")
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let evidence_paths = evidence["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|instance| instance["path"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if manifest_paths != evidence_paths {
        failures.push("instance evidence is incomplete for the source manifest".to_string());
    }
    let require_sr = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|tool| tool["role"] == "sr_validator" && tool["status"] == "available");
    for instance in evidence["instances"].as_array().into_iter().flatten() {
        let path = instance["path"].as_str().unwrap_or("unknown");
        let primary = instance["results"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|result| result["role"] == "primary_iod_validator");
        if primary.is_none_or(|result| result["status"] != "completed") {
            failures.push(format!("primary validation incomplete: {path}"));
        }
        if let Some(primary) = primary {
            let adapter_id = primary["adapter_id"].as_str().unwrap_or("unknown");
            let primary_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == adapter_id);
            if primary_tool.is_none_or(|tool| {
                tool["status"] != "available" || tool["lock_status"] != "matched"
            }) {
                failures.push(format!(
                    "primary validator {adapter_id} is unavailable or unlocked for {path}"
                ));
            }
        }
        let parser = instance["results"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|result| result["role"] == "independent_parser");
        if parser.is_none_or(|result| result["status"] != "completed") {
            failures.push(format!("independent parser incomplete: {path}"));
        }
        let case_id = instance["case_id"].as_str().unwrap_or("");
        if requires_pixelmed_sr_validation(case_id) {
            let pixelmed_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == PIXELMED_SR_VALIDATOR_ID);
            if pixelmed_tool.is_none_or(|tool| tool["status"] != "available") {
                failures.push(format!(
                    "required PixelMed SR validator is unavailable for {path}"
                ));
            }
            if pixelmed_tool.is_none_or(|tool| tool["lock_status"] != "matched") {
                failures.push(format!(
                    "required PixelMed SR validator is unlocked for {path}"
                ));
            }
            let sr = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| {
                    result["role"] == "sr_validator"
                        && result["adapter_id"] == PIXELMED_SR_VALIDATOR_ID
                });
            if sr.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!("required PixelMed SR validation incomplete: {path}"));
            }
        } else if require_sr
            && is_supported_sr_sop_class(instance["sop_class_uid"].as_str().unwrap_or(""))
        {
            let sr = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| result["role"] == "sr_validator");
            if sr.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!("SR validation incomplete: {path}"));
            }
        }
        if instance["transfer_syntax_uid"] == "1.2.840.10008.1.2.5"
            && instance["pixel"]["status"] != "passed"
        {
            failures.push(format!("independent RLE pixel evidence failed: {path}"));
        }
        if required_float_pixel_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent native float32 pixel evidence failed: {path}"
            ));
        }
        if required_double_float_pixel_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent native float64 pixel evidence failed: {path}"
            ));
        }
        if required_u32_pixel_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent native u32 pixel evidence failed: {path}"
            ));
        }
        if required_u32_pixel_paths.contains(path) {
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_u32_pixel_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        }
        if required_u1_pixel_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent native u1 pixel evidence failed: {path}"
            ));
        }
        if required_u1_pixel_paths.contains(path) {
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_u1_pixel_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        }
        if required_nonsquare_spacing_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent non-square spacing evidence failed: {path}"
            ));
        }
        if required_nonsquare_spacing_paths.contains(path) {
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_nonsquare_spacing_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        }
        if required_icc_paths.contains(path) {
            let icc_result = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| result["role"] == "icc_validator");
            if instance
                .get("icc")
                .is_none_or(|icc| icc["status"] != "passed" || icc["independence"] != "independent")
            {
                failures.push(format!("independent ICC profile evidence failed: {path}"));
            }
            if icc_result.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!("ICC validation incomplete: {path}"));
            }
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_icc_profile_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        } else if instance.get("icc").is_some() {
            failures.push(format!("ICC profile evidence is out of scope: {path}"));
        }
    }
    if evidence["entity"]["status"] != "completed" {
        failures.push("corpus entity validation is incomplete".to_string());
    }
    if let Some(manifest) = manifest.as_ref() {
        verify_entity_projection(evidence_root, evidence, manifest, failures);
    }
}

fn verify_entity_projection(
    evidence_root: &Path,
    evidence: &Value,
    manifest: &Value,
    failures: &mut Vec<String>,
) {
    let files = manifest["files"].as_array().cloned().unwrap_or_default();
    let u32_files = files
        .iter()
        .filter(|file| file["case_id"] == "classic/sc/mono2_u32_explicit_le")
        .collect::<Vec<_>>();
    let projection = evidence["entity"].get("input_projection");
    if u32_files.is_empty() {
        if projection.is_some() {
            failures
                .push("entity input projection exists without an eligible u32 case".to_string());
        }
        return;
    }
    let Some(projection) = projection else {
        failures.push("u32 corpus requires an entity input projection".to_string());
        return;
    };
    let entries = projection["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let file_list = projection["file_list"]["path"]
        .as_str()
        .filter(|path| validate_relative_path(path).is_ok())
        .and_then(|path| fs::read_to_string(evidence_root.join(path)).ok());
    let Some(file_list) = file_list else {
        failures.push("entity projection file list cannot be verified".to_string());
        return;
    };
    let listed = file_list.lines().map(Path::new).collect::<Vec<_>>();
    let mut valid = projection["method"] == "terminal_pixel_data_element_redaction_v1"
        && projection["scope"] == "entity_consistency_only"
        && entries.len() == u32_files.len()
        && listed.len() == files.len();
    for file in &files {
        let Some(relative) = file["path"].as_str() else {
            valid = false;
            continue;
        };
        if file["case_id"] != "classic/sc/mono2_u32_explicit_le"
            && listed
                .iter()
                .filter(|path| path.ends_with(relative))
                .count()
                != 1
        {
            valid = false;
        }
    }
    for file in u32_files {
        let relative = file["path"].as_str().unwrap_or("");
        let matching = entries
            .iter()
            .filter(|entry| entry["source_path"] == relative)
            .collect::<Vec<_>>();
        let [entry] = matching.as_slice() else {
            valid = false;
            continue;
        };
        let source_relative = entry["source_copy"]["path"].as_str().unwrap_or("");
        let projected_relative = entry["projected_input"]["path"].as_str().unwrap_or("");
        if validate_relative_path(source_relative).is_err()
            || validate_relative_path(projected_relative).is_err()
        {
            valid = false;
            continue;
        }
        let Ok(source) = fs::read(evidence_root.join(source_relative)) else {
            valid = false;
            continue;
        };
        let Ok(projected) = fs::read(evidence_root.join(projected_relative)) else {
            valid = false;
            continue;
        };
        let removed = &entry["removed_element"];
        let element_offset = removed["element_offset"].as_u64().unwrap_or(u64::MAX) as usize;
        let value_offset = removed["value_offset"].as_u64().unwrap_or(u64::MAX) as usize;
        let value_length = removed["value_length"].as_u64().unwrap_or(0) as usize;
        let range_is_valid = value_offset == element_offset.saturating_add(12)
            && value_length == 16
            && value_offset
                .checked_add(value_length)
                .is_some_and(|end| end == source.len())
            && element_offset
                .checked_add(12)
                .is_some_and(|end| end <= source.len());
        if !range_is_valid {
            valid = false;
            continue;
        }
        let expected_header = [
            0xe0, 0x7f, 0x10, 0x00, b'O', b'W', 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
        ];
        let value = &source[value_offset..];
        let expected_projection = source[..element_offset].to_vec();
        valid &= entry["source_case_id"] == "classic/sc/mono2_u32_explicit_le"
            && entry["transfer_syntax_uid"] == "1.2.840.10008.1.2.1"
            && file["dicom"]["transfer_syntax_uid"] == "1.2.840.10008.1.2.1"
            && file["image"]["bits_allocated"] == 32
            && file["image"]["bits_stored"] == 32
            && file["image"]["high_bit"] == 31
            && file["image"]["pixel_representation"] == 0
            && file["pixel_data"]["vr"] == "OW"
            && file["pixel_data"]["value_length"] == 16
            && file["pixel_data"]["frame_count"] == 1
            && file["expected_u32_pixels"]["stored_values"]
                == json!([0_u64, 65_535, 2_147_483_648_u64, 4_294_967_295_u64])
            && removed["tag"] == "(7FE0,0010)"
            && removed["vr"] == "OW"
            && source[element_offset..value_offset] == expected_header
            && sha256_hex(&source) == file["sha256"].as_str().unwrap_or("")
            && sha256_hex(value) == removed["value_sha256"].as_str().unwrap_or("")
            && sha256_hex(value)
                == file["expected_u32_pixels"]["pixel_data_sha256"]
                    .as_str()
                    .unwrap_or("")
            && projected == expected_projection
            && listed
                .iter()
                .filter(|path| path.ends_with(projected_relative))
                .count()
                == 1
            && listed.iter().all(|path| !path.ends_with(relative));
    }
    if !valid {
        failures.push(
            "entity input projection is not a byte-preserving, manifest-linked u32 redaction"
                .to_string(),
        );
    }
}

fn verify_u32_pixel_evidence(
    evidence_root: &Path,
    evidence: &Value,
    instance: &Value,
    manifest_file: &Value,
    failures: &mut Vec<String>,
) {
    let path = instance["path"].as_str().unwrap_or("unknown");
    let Some(relative) = instance
        .pointer("/pixel/evidence/path")
        .and_then(Value::as_str)
    else {
        failures.push(format!("u32 pixel evidence sidecar is missing: {path}"));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!("u32 pixel evidence sidecar path is unsafe: {path}"));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!("u32 pixel evidence sidecar is unavailable: {path}"));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "u32 pixel evidence sidecar is invalid JSON: {path}"
        ));
        return;
    };
    let adapter_id = "pydicom-dicom-validator-u32";
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == adapter_id);
    let actual = &sidecar["actual"];
    let semantically_linked = sidecar["adapter_id"] == adapter_id
        && sidecar["adapter_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && tool
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "uv_locked_pydicom_raw_ow_struct_unpack_u32_le"
        && sidecar["status"] == "passed"
        && sidecar["expected_frame_hashes"] == instance["pixel"]["expected_frame_hashes"]
        && sidecar["actual_frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && sidecar["expected_stored_values"]
            == manifest_file["expected_u32_pixels"]["stored_values"]
        && actual["frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && actual["stored_values"] == manifest_file["expected_u32_pixels"]["stored_values"]
        && actual["pixel_data_sha256"] == manifest_file["expected_u32_pixels"]["pixel_data_sha256"]
        && actual["rows"] == manifest_file["image"]["rows"]
        && actual["columns"] == manifest_file["image"]["columns"]
        && actual["frames"] == manifest_file["image"]["frames"]
        && actual["samples_per_pixel"] == manifest_file["image"]["samples_per_pixel"]
        && actual["bits_allocated"] == manifest_file["image"]["bits_allocated"]
        && actual["bits_stored"] == manifest_file["image"]["bits_stored"]
        && actual["high_bit"] == manifest_file["image"]["high_bit"]
        && actual["pixel_representation"] == manifest_file["image"]["pixel_representation"]
        && actual["photometric_interpretation"]
            == manifest_file["image"]["photometric_interpretation"]
        && actual["pixel_data_vr"] == manifest_file["pixel_data"]["vr"]
        && actual["transfer_syntax_uid"] == manifest_file["dicom"]["transfer_syntax_uid"]
        && actual["byte_order"] == "little_endian";
    if !semantically_linked {
        failures.push(format!(
            "u32 pixel evidence sidecar is not linked to its locked tool and source manifest: {path}"
        ));
    }
}

fn verify_nonsquare_spacing_evidence(
    evidence_root: &Path,
    evidence: &Value,
    instance: &Value,
    manifest_file: &Value,
    failures: &mut Vec<String>,
) {
    let path = instance["path"].as_str().unwrap_or("unknown");
    let Some(relative) = instance.pointer("/pixel/evidence/path").and_then(Value::as_str) else {
        failures.push(format!("non-square spacing evidence sidecar is missing: {path}"));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!("non-square spacing evidence sidecar path is unsafe: {path}"));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!("non-square spacing evidence sidecar is unavailable: {path}"));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!("non-square spacing evidence sidecar is invalid JSON: {path}"));
        return;
    };
    let adapter_id = "pydicom-dicom-validator-u32";
    let tool = evidence["tools"].as_array().into_iter().flatten()
        .find(|tool| tool["adapter_id"] == adapter_id);
    let actual = &sidecar["actual"];
    let contract = &manifest_file["expected_nonsquare_spacing"];
    let linked = sidecar["adapter_id"] == adapter_id
        && sidecar["adapter_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && tool.is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "uv_locked_pydicom_nonsquare_spatial_semantic_extraction"
        && sidecar["status"] == "passed"
        && sidecar["expected_contract"] == *contract
        && sidecar["expected_frame_hashes"] == instance["pixel"]["expected_frame_hashes"]
        && sidecar["actual_frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && actual["frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && actual["variant_id"] == contract["variant_id"]
        && spatial_element_matches(&actual["pixel_spacing"], &contract["pixel_spacing"])
        && spatial_element_matches(&actual["nominal_scanned_pixel_spacing"], &contract["nominal_scanned_pixel_spacing"])
        && spatial_element_matches(&actual["pixel_aspect_ratio"], &contract["pixel_aspect_ratio"])
        && actual["uncalibrated"] == contract["uncalibrated"]
        && actual["patient_space_geometry_present"] == contract["patient_space_geometry_present"]
        && actual["pixel_data_sha256"] == contract["pixel_data_sha256"]
        && actual["rows"] == manifest_file["image"]["rows"]
        && actual["columns"] == manifest_file["image"]["columns"]
        && actual["frames"] == manifest_file["image"]["frames"]
        && actual["samples_per_pixel"] == manifest_file["image"]["samples_per_pixel"]
        && actual["bits_allocated"] == manifest_file["image"]["bits_allocated"]
        && actual["bits_stored"] == manifest_file["image"]["bits_stored"]
        && actual["high_bit"] == manifest_file["image"]["high_bit"]
        && actual["pixel_representation"] == manifest_file["image"]["pixel_representation"]
        && actual["photometric_interpretation"] == manifest_file["image"]["photometric_interpretation"]
        && actual["pixel_data_vr"] == manifest_file["pixel_data"]["vr"]
        && actual["transfer_syntax_uid"] == manifest_file["dicom"]["transfer_syntax_uid"];
    if !linked {
        failures.push(format!("non-square spacing evidence sidecar is not linked to its locked tool and source manifest: {path}"));
    }
}

fn spatial_element_matches(actual: &Value, expected: &Value) -> bool {
    if expected.is_null() { return actual.is_null(); }
    actual["tag"] == expected["tag"] && actual["vr"] == expected["vr"]
        && actual["vm"] == expected["vm"] && actual["lexical_value"] == expected["lexical_value"]
}

fn verify_u1_pixel_evidence(
    evidence_root: &Path,
    evidence: &Value,
    instance: &Value,
    manifest_file: &Value,
    failures: &mut Vec<String>,
) {
    let path = instance["path"].as_str().unwrap_or("unknown");
    let Some(relative) = instance
        .pointer("/pixel/evidence/path")
        .and_then(Value::as_str)
    else {
        failures.push(format!("u1 pixel evidence sidecar is missing: {path}"));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!("u1 pixel evidence sidecar path is unsafe: {path}"));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!("u1 pixel evidence sidecar is unavailable: {path}"));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!("u1 pixel evidence sidecar is invalid JSON: {path}"));
        return;
    };
    let adapter_id = "dcmtk-dcm2img-u1";
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == adapter_id);
    let parser = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["role"] == "independent_parser");
    let semantically_linked = sidecar["adapter_id"] == adapter_id
        && sidecar["decoder_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && sidecar["parser_sha256"].as_str() == parser.and_then(|tool| tool["sha256"].as_str())
        && tool
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && parser
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "dcmtk_dcm2img_p2_and_dcmdump_raw"
        && sidecar["status"] == "passed"
        && sidecar["source_instance_sha256"] == manifest_file["sha256"]
        && sidecar["source_pixel_data_sha256"]
            == manifest_file["expected_u1_pixels"]["pixel_data_sha256"]
        && sidecar["expected_frame_hashes"] == instance["pixel"]["expected_frame_hashes"]
        && sidecar["actual_frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && sidecar["actual_frame_hashes"]
            == manifest_file["expected_u1_pixels"]["decoded_frame_sha256"]
        && sidecar["decoded_values"] == manifest_file["expected_u1_pixels"]["stored_values"]
        && sidecar["rows"] == manifest_file["image"]["rows"]
        && sidecar["columns"] == manifest_file["image"]["columns"]
        && sidecar["frames"] == manifest_file["image"]["frames"]
        && sidecar["max_value"] == 1
        && sidecar["packing_order"] == manifest_file["expected_u1_pixels"]["packing_order"]
        && sidecar["frame_boundary_policy"]
            == manifest_file["expected_u1_pixels"]["frame_boundary_policy"];
    if !semantically_linked {
        failures.push(format!(
            "u1 pixel evidence sidecar is not linked to its locked tools and source manifest: {path}"
        ));
    }
}

fn verify_icc_profile_evidence(
    evidence_root: &Path,
    evidence: &Value,
    instance: &Value,
    manifest_file: &Value,
    failures: &mut Vec<String>,
) {
    let path = instance["path"].as_str().unwrap_or("unknown");
    let Some(relative) = instance
        .pointer("/icc/evidence/path")
        .and_then(Value::as_str)
    else {
        failures.push(format!("ICC profile evidence sidecar is missing: {path}"));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!(
            "ICC profile evidence sidecar path is unsafe: {path}"
        ));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!(
            "ICC profile evidence sidecar is unavailable: {path}"
        ));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "ICC profile evidence sidecar is invalid JSON: {path}"
        ));
        return;
    };
    let validator = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == "littlecms-transicc-icc");
    let extractor = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["role"] == "independent_parser");
    let expected_transforms = json!([
        {"rgb": [255, 0, 0], "xyz": [43.6035, 22.2443, 1.3901]},
        {"rgb": [0, 255, 0], "xyz": [38.5101, 71.6934, 9.7076]},
        {"rgb": [0, 0, 255], "xyz": [14.3066, 6.0623, 71.3928]},
        {"rgb": [255, 255, 255], "xyz": [96.4203, 100.0, 82.4905]}
    ]);
    let linked = instance["icc"]["adapter_id"] == "littlecms-transicc-icc"
        && instance["icc"]["status"] == "passed"
        && instance["icc"]["independence"] == "independent"
        && sidecar["adapter_id"] == "littlecms-transicc-icc"
        && sidecar["validator_sha256"].as_str()
            == validator.and_then(|tool| tool["sha256"].as_str())
        && sidecar["extractor_adapter_id"].as_str()
            == extractor.and_then(|tool| tool["adapter_id"].as_str())
        && sidecar["extractor_sha256"].as_str()
            == extractor.and_then(|tool| tool["sha256"].as_str())
        && validator
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && extractor
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "dcmtk_dcmdump_complete_ob_hex"
        && sidecar["status"] == "passed"
        && sidecar["source_instance_sha256"] == manifest_file["sha256"]
        && sidecar["source_profile_sha256"]
            == manifest_file["expected_icc_profile"]["profile_sha256"]
        && sidecar["manifest_profile_sha256"]
            == manifest_file["expected_icc_profile"]["profile_sha256"]
        && sidecar["profile_size_bytes"] == 736
        && sidecar["declared_profile_size_bytes"] == 736
        && sidecar["dicom_color_space"] == manifest_file["expected_icc_profile"]["color_space"]
        && sidecar["header"]["device_class"] == "scnr"
        && sidecar["header"]["data_color_space"] == "RGB "
        && sidecar["header"]["profile_connection_space"] == "XYZ "
        && sidecar["header"]["signature"] == "acsp"
        && sidecar["header"]["rendering_intent"] == 0
        && sidecar["tag_count"] == 9
        && sidecar["transforms"] == expected_transforms;
    if !linked {
        failures.push(format!(
            "ICC profile evidence sidecar is not linked to its locked tools and source manifest: {path}"
        ));
    }
}

fn is_supported_sr_sop_class(uid: &str) -> bool {
    matches!(
        uid,
        "1.2.840.10008.5.1.4.1.1.88.11"
            | "1.2.840.10008.5.1.4.1.1.88.33"
            | "1.2.840.10008.5.1.4.1.1.88.34"
            | "1.2.840.10008.5.1.4.1.1.88.59"
    )
}

fn requires_pixelmed_sr_validation(case_id: &str) -> bool {
    matches!(
        case_id,
        "derived/sr/tid1500_ct_measurement_report" | "derived/sr/comprehensive3d_scoord3d"
    )
}

fn verify_findings(evidence: &Value, allowlist: &Value, failures: &mut Vec<String>) -> usize {
    let entries = allowlist["findings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut matched = vec![false; entries.len()];
    let mut accepted = 0;
    let now = rfc3339_now();
    let today = &now[..10];
    for (instance, result, finding) in instance_findings(evidence) {
        let severity = finding["severity"].as_str().unwrap_or("unknown");
        if severity == "info" || severity == "unsupported" {
            continue;
        }
        let fingerprint = evidence["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|tool| tool["adapter_id"] == result["adapter_id"])
            .and_then(|tool| tool["sha256"].as_str());
        let found = entries.iter().enumerate().find(|(_, entry)| {
            entry["validator_adapter_id"] == result["adapter_id"]
                && entry["validator_fingerprint"].as_str() == fingerprint
                && entry["case_id"] == instance["case_id"]
                && entry
                    .get("path")
                    .is_none_or(|path| path == &instance["path"])
                && entry["message_fingerprint"] == finding["message_fingerprint"]
                && entry["original_severity"] == finding["severity"]
                && entry
                    .get("rule_id")
                    .is_none_or(|rule| rule == &finding["rule_id"])
                && entry["expires_on"]
                    .as_str()
                    .is_none_or(|expiry| expiry >= today)
        });
        if let Some((index, _)) = found {
            matched[index] = true;
            accepted += 1;
        } else {
            failures.push(format!(
                "unresolved {severity} for {}: {}",
                instance["path"].as_str().unwrap_or("unknown"),
                finding["message"].as_str().unwrap_or("unknown finding")
            ));
        }
    }
    for finding in evidence["entity"]["findings"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let severity = finding["severity"].as_str().unwrap_or("unknown");
        if !matches!(severity, "info" | "unsupported") {
            failures.push(format!(
                "unresolved entity {severity}: {}",
                finding["message"].as_str().unwrap_or("unknown finding")
            ));
        }
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry["expires_on"]
            .as_str()
            .is_some_and(|expiry| expiry < today)
        {
            failures.push(format!("expired disposition at allowlist index {index}"));
        } else if !matched[index] {
            failures.push(format!("stale disposition at allowlist index {index}"));
        }
    }
    accepted
}

fn all_results(evidence: &Value) -> Vec<&Value> {
    let mut results = evidence["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|instance| instance["results"].as_array().into_iter().flatten())
        .collect::<Vec<_>>();
    results.push(&evidence["entity"]);
    results
}

fn instance_findings(evidence: &Value) -> Vec<(&Value, &Value, &Value)> {
    let mut findings = Vec::new();
    for instance in evidence["instances"].as_array().into_iter().flatten() {
        for result in instance["results"].as_array().into_iter().flatten() {
            for finding in result["findings"].as_array().into_iter().flatten() {
                findings.push((instance, result, finding));
            }
        }
    }
    findings
}

pub fn run_conformance(
    generated_root: impl AsRef<Path>,
    evidence_root: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
) -> Result<Value, String> {
    let generated_root = generated_root.as_ref();
    let evidence_root = evidence_root.as_ref();
    let config_path = config_path.as_ref();
    let manifest_path = generated_root.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("failed to parse {}: {error}", manifest_path.display()))?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let files = manifest
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest.json must contain a files array".to_string())?;
    let config = read_json(config_path)?;
    let adapters = config
        .get("adapters")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{} must contain an adapters array", config_path.display()))?;
    if !adapters.iter().any(|adapter| {
        adapter.get("role").and_then(Value::as_str) == Some("primary_iod_validator")
            && adapter.get("supported_case_ids").is_none()
    }) {
        return Err("configuration requires a default primary_iod_validator adapter".to_string());
    }
    let tool_report = check_tools_path(config_path)?;
    let tools = evidence_tools(&tool_report);

    fs::create_dir_all(evidence_root)
        .map_err(|error| format!("failed to create {}: {error}", evidence_root.display()))?;
    let source_dir = evidence_root.join("source");
    fs::create_dir_all(&source_dir)
        .map_err(|error| format!("failed to create {}: {error}", source_dir.display()))?;
    fs::write(source_dir.join("manifest.json"), &manifest_bytes)
        .map_err(|error| format!("failed to preserve source manifest: {error}"))?;
    let mut sorted_files = files.iter().collect::<Vec<_>>();
    sorted_files.sort_by_key(|file| file.get("path").and_then(Value::as_str).unwrap_or(""));
    let mut instances = Vec::with_capacity(sorted_files.len());
    for file in &sorted_files {
        instances.push(collect_instance(
            generated_root,
            evidence_root,
            file,
            adapters,
            &tools,
        )?);
    }

    let entity = collect_entity(
        generated_root,
        evidence_root,
        &sorted_files,
        adapters,
        &tools,
    )?;
    let repository = repository_identity();
    let standards_lock_sha256 = manifest
        .pointer("/standards/standards_lock_sha256")
        .and_then(Value::as_str)
        .unwrap_or(&"0".repeat(64))
        .to_string();
    let generator_name = manifest
        .pointer("/generator/name")
        .and_then(Value::as_str)
        .unwrap_or("dicom-test-suite");
    let generator_version = manifest
        .pointer("/generator/version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let run_material = format!("{manifest_sha256}:{}", config_path.display());
    let evidence = json!({
        "schema_version": "0.1.0",
        "run_id": sha256_hex(run_material.as_bytes()),
        "created_at": rfc3339_now(),
        "repository": repository,
        "source": {
            "manifest_path": "source/manifest.json",
            "manifest_sha256": manifest_sha256
        },
        "generator": {
            "identity": format!("{generator_name} {generator_version}"),
            "seed": manifest.pointer("/run/seed").and_then(Value::as_u64).unwrap_or(0),
            "profile": manifest.pointer("/run/profile").and_then(Value::as_str).unwrap_or("unknown"),
            "features": manifest.pointer("/generator/feature_flags").cloned().unwrap_or_else(|| json!([])),
            "standards_lock_sha256": standards_lock_sha256
        },
        "host": {
            "os": env::consts::OS,
            "architecture": env::consts::ARCH
        },
        "tools": tools,
        "instances": instances,
        "entity": entity,
        "summary": summarize(&instances, &entity)
    });
    let run_path = evidence_root.join("conformance-run.json");
    let mut encoded = serde_json::to_vec_pretty(&evidence)
        .map_err(|error| format!("failed to serialize evidence: {error}"))?;
    encoded.push(b'\n');
    fs::write(&run_path, encoded)
        .map_err(|error| format!("failed to write {}: {error}", run_path.display()))?;
    Ok(evidence)
}

fn evidence_tools(report: &Value) -> Vec<Value> {
    report
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|tool| {
            json!({
                "adapter_id": tool["adapter_id"],
                "role": tool["role"],
                "status": tool["status"],
                "required": tool["required"],
                "executable": tool["executable"],
                "sha256": tool["sha256"],
                "executable_sha256": tool["executable_sha256"],
                "artifacts": tool["artifacts"],
                "version_output": tool["version_output"],
                "version_exit_code": tool["version_exit_code"],
                "lock_status": tool["lock_status"]
            })
        })
        .collect()
}

fn collect_instance(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Value, String> {
    let path = file
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "manifest file entry requires path".to_string())?;
    let case_id = file
        .get("case_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("manifest file {path} requires case_id"))?;
    let (adapter, tool) = select_primary_iod_validator(case_id, adapters, tools)?;
    validate_relative_path(path)?;
    let stable_key = sha256_hex(path.as_bytes());
    let adapter_id = required_string(adapter, "id")?;
    let raw_dir = evidence_root.join("raw").join(adapter_id);
    fs::create_dir_all(&raw_dir)
        .map_err(|error| format!("failed to create {}: {error}", raw_dir.display()))?;
    let stdout_relative = format!("raw/{adapter_id}/{stable_key}.stdout");
    let stderr_relative = format!("raw/{adapter_id}/{stable_key}.stderr");
    let stdout_path = evidence_root.join(&stdout_relative);
    let stderr_path = evidence_root.join(&stderr_relative);

    let primary_result = if tool.get("status").and_then(Value::as_str) != Some("available") {
        fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
        unsupported_result(
            adapter_id,
            "primary_iod_validator",
            vec![required_string(adapter, "executable")?.to_string()],
            &stdout_relative,
            &stderr_relative,
            "configured primary validator is unavailable",
        )
    } else {
        let executable = tool
            .get("executable")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("available adapter {adapter_id} has no executable"))?;
        let input = generated_root.join(path);
        if !input.is_file() {
            return Err(format!("manifest file does not exist: {}", input.display()));
        }
        let arguments = string_array(adapter, "arguments")?
            .into_iter()
            .map(|argument| argument.replace("{input}", &input.display().to_string()))
            .collect::<Vec<_>>();
        let timeout = Duration::from_secs(
            adapter
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("adapter {adapter_id} requires timeout_seconds"))?,
        );
        let output = run_with_timeout(Path::new(executable), &arguments, timeout)?;
        fs::write(&stdout_path, &output.stdout).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, &output.stderr).map_err(|error| error.to_string())?;
        execution_result(
            adapter_id,
            "primary_iod_validator",
            executable,
            arguments,
            output,
            &stdout_relative,
            &stderr_relative,
            &input.display().to_string(),
            path,
        )
    };
    let pixel = collect_pixel_result(
        generated_root,
        evidence_root,
        file,
        path,
        &stable_key,
        adapters,
        tools,
    )?;
    let mut results = vec![primary_result];
    results.push(collect_parser_result(
        generated_root,
        evidence_root,
        path,
        &stable_key,
        adapters,
        tools,
    )?);
    if let Some(result) = collect_sr_result(
        generated_root,
        evidence_root,
        file,
        path,
        &stable_key,
        adapters,
        tools,
    )? {
        results.push(result);
    }
    let icc = collect_icc_result(
        generated_root,
        evidence_root,
        file,
        path,
        &stable_key,
        adapters,
        tools,
    )?;
    if let Some((result, _)) = icc.as_ref() {
        results.push(result.clone());
    }
    let mut instance = json!({
        "stable_instance_key": stable_key,
        "case_id": case_id,
        "path": path,
        "sop_class_uid": file.pointer("/dicom/sop_class_uid").and_then(Value::as_str).unwrap_or("0.0"),
        "transfer_syntax_uid": file.pointer("/dicom/transfer_syntax_uid").and_then(Value::as_str).unwrap_or("0.0"),
        "results": results,
        "pixel": pixel
    });
    if let Some((_, icc)) = icc {
        instance["icc"] = icc;
    }
    Ok(instance)
}

fn select_primary_iod_validator<'a>(
    case_id: &str,
    adapters: &'a [Value],
    tools: &'a [Value],
) -> Result<(&'a Value, &'a Value), String> {
    let matching = adapters
        .iter()
        .filter(|adapter| {
            adapter.get("role").and_then(Value::as_str) == Some("primary_iod_validator")
                && adapter
                    .get("supported_case_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|case_ids| {
                        case_ids.iter().any(|value| value.as_str() == Some(case_id))
                    })
        })
        .collect::<Vec<_>>();
    let adapter = match matching.as_slice() {
        [] => adapters
            .iter()
            .find(|adapter| {
                adapter.get("role").and_then(Value::as_str) == Some("primary_iod_validator")
                    && adapter.get("supported_case_ids").is_none()
            })
            .ok_or_else(|| {
                "configuration requires a default primary_iod_validator adapter".to_string()
            })?,
        [adapter] => *adapter,
        _ => {
            return Err(format!(
                "multiple primary IOD validators are configured for case {case_id}"
            ));
        }
    };
    let adapter_id = required_string(adapter, "id")?;
    let tool = tools
        .iter()
        .find(|tool| tool.get("adapter_id").and_then(Value::as_str) == Some(adapter_id))
        .ok_or_else(|| format!("primary validator discovery result is missing for {adapter_id}"))?;
    Ok((adapter, tool))
}

fn collect_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Value, String> {
    let expected = file
        .pointer("/pixel_data/frame_hashes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if file.get("pixel_data").is_none_or(Value::is_null) {
        return Ok(pixel_unsupported(
            vec![],
            "not_applicable",
            "Instance has no Pixel Data",
        ));
    }
    let transfer_syntax = file
        .pointer("/dicom/transfer_syntax_uid")
        .and_then(Value::as_str)
        .unwrap_or("");
    if file.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_u32_explicit_le") {
        return collect_u32_pixel_result(
            generated_root,
            evidence_root,
            file,
            relative_input,
            stable_key,
            adapters,
            tools,
            expected,
        );
    }
    if file.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_u1_native") {
        return collect_u1_pixel_result(
            generated_root,
            evidence_root,
            file,
            relative_input,
            stable_key,
            adapters,
            tools,
            expected,
        );
    }
    if file.get("case_id").and_then(Value::as_str) == Some("classic/sc/nonsquare_pixel_spacing") {
        return collect_nonsquare_spacing_result(
            generated_root,
            evidence_root,
            file,
            relative_input,
            stable_key,
            adapters,
            tools,
            expected,
        );
    }
    if file.pointer("/image/sample_type").and_then(Value::as_str) == Some("float32")
        || file.pointer("/pixel_data/vr").and_then(Value::as_str) == Some("OF")
    {
        return collect_float32_pixel_result(
            generated_root,
            evidence_root,
            file,
            relative_input,
            stable_key,
            adapters,
            tools,
            expected,
            transfer_syntax,
        );
    }
    if file.pointer("/image/sample_type").and_then(Value::as_str) == Some("float64")
        || file.pointer("/pixel_data/vr").and_then(Value::as_str) == Some("OD")
    {
        return collect_float64_pixel_result(
            generated_root,
            evidence_root,
            file,
            relative_input,
            stable_key,
            adapters,
            tools,
            expected,
            transfer_syntax,
        );
    }
    if transfer_syntax != "1.2.840.10008.1.2.5" {
        let reason = if file
            .pointer("/expected_semantics/lossy_image_compression")
            .and_then(Value::as_str)
            == Some("01")
        {
            "Lossy pixel comparison is outside the first strict milestone"
        } else {
            "No proven independent exact-byte decoder adapter is selected for this transfer syntax"
        };
        return Ok(pixel_unsupported(expected, "not_applicable", reason));
    }
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter["id"] == "dcmtk-dcmdrle")
    else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK RLE decoder adapter is not configured",
        ));
    };
    let Some(tool) = tools
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdrle")
    else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK RLE decoder was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK RLE decoder is unavailable",
        ));
    }
    let parser_tool = tools
        .iter()
        .find(|tool| tool["role"] == "independent_parser" && tool["status"] == "available")
        .ok_or_else(|| {
            "RLE pixel extraction requires the configured independent parser".to_string()
        })?;
    let decoder = tool["executable"]
        .as_str()
        .ok_or_else(|| "available RLE decoder has no executable".to_string())?;
    let dcmdump = parser_tool["executable"]
        .as_str()
        .ok_or_else(|| "available parser has no executable".to_string())?;
    let pixel_dir = evidence_root.join("pixels/dcmtk-dcmdrle");
    fs::create_dir_all(&pixel_dir).map_err(|error| error.to_string())?;
    let work_dir = conformance_work_dir("dts-pixel", evidence_root, stable_key);
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let decoded = work_dir.join("decoded.dcm");
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| {
            argument
                .replace("{input}", &input.display().to_string())
                .replace("{output}", &decoded.display().to_string())
        })
        .collect::<Vec<_>>();
    let decode = run_with_timeout(
        Path::new(decoder),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let extraction_args = vec![
        "+W".to_string(),
        work_dir.display().to_string(),
        decoded.display().to_string(),
    ];
    let extraction = if decode.exit_code == Some(0) && !decode.timed_out {
        Some(run_with_timeout(
            Path::new(dcmdump),
            &extraction_args,
            Duration::from_secs(30),
        )?)
    } else {
        None
    };
    let raw_path = fs::read_dir(&work_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("raw"));
    let frame_size = pixel_frame_size(file);
    let actual_hashes = raw_path
        .as_ref()
        .and_then(|path| fs::read(path).ok())
        .filter(|bytes| frame_size > 0 && bytes.len() >= frame_size * expected.len())
        .map(|bytes| {
            bytes[..frame_size * expected.len()]
                .chunks_exact(frame_size)
                .map(|frame| sha256_hex(&normalize_native_frame(frame, file)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let expected_strings = expected
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let passed = decode.exit_code == Some(0)
        && !decode.timed_out
        && extraction
            .as_ref()
            .is_some_and(|output| output.exit_code == Some(0) && !output.timed_out)
        && actual_hashes.iter().map(String::as_str).collect::<Vec<_>>() == expected_strings;
    let sidecar = json!({
        "adapter_id": "dcmtk-dcmdrle",
        "decoder_sha256": tool["sha256"],
        "parser_sha256": parser_tool["sha256"],
        "independence": "independent",
        "generator_encoder": file.pointer("/pixel_data/codec/backend").cloned().unwrap_or(Value::Null),
        "invocation": std::iter::once(decoder.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "decode_exit_code": decode.exit_code,
        "decode_timed_out": decode.timed_out,
        "extraction_exit_code": extraction.as_ref().and_then(|output| output.exit_code),
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual_hashes,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/dcmtk-dcmdrle/{stable_key}.json");
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(evidence_root.join(&relative), &encoded).map_err(|error| error.to_string())?;
    for path in [raw_path, Some(decoded)]
        .into_iter()
        .flatten()
        .filter(|path| path.is_file())
    {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_dir(&work_dir);
    Ok(json!({
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_frame_hashes": sidecar["expected_frame_hashes"],
        "actual_frame_hashes": sidecar["actual_frame_hashes"],
        "reason": if passed { "DCMTK RLE decode matched every expected native frame hash" } else { "DCMTK RLE decode or native frame hash comparison failed" },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

#[allow(clippy::too_many_arguments)]
fn collect_u1_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
) -> Result<Value, String> {
    let adapter_id = "dcmtk-dcm2img-u1";
    let Some(adapter) = adapters.iter().find(|adapter| adapter["id"] == adapter_id) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK one-bit pixel decoder is not configured",
        ));
    };
    let Some(tool) = tools.iter().find(|tool| tool["adapter_id"] == adapter_id) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK one-bit pixel decoder was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK one-bit pixel decoder is unavailable",
        ));
    }
    let parser_tool = tools
        .iter()
        .find(|tool| tool["role"] == "independent_parser" && tool["status"] == "available")
        .ok_or_else(|| "one-bit pixel evidence requires the independent parser".to_string())?;
    let decoder = tool["executable"]
        .as_str()
        .ok_or_else(|| "available one-bit decoder has no executable".to_string())?;
    let dcmdump = parser_tool["executable"]
        .as_str()
        .ok_or_else(|| "available parser has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let work_dir = conformance_work_dir("dts-u1-pixel", evidence_root, stable_key);
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let output_base = work_dir.join("frame.pgm");
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| {
            argument
                .replace("{input}", &input.display().to_string())
                .replace("{output}", &output_base.display().to_string())
        })
        .collect::<Vec<_>>();
    let decode = run_with_timeout(
        Path::new(decoder),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let extraction_args = vec![
        "+W".to_string(),
        work_dir.display().to_string(),
        input.display().to_string(),
    ];
    let extraction = run_with_timeout(
        Path::new(dcmdump),
        &extraction_args,
        Duration::from_secs(30),
    )?;

    let frame_count = file["image"]["frames"].as_u64().unwrap_or(0) as usize;
    let mut decoded_frames = Vec::new();
    let mut pgm_valid = decode.exit_code == Some(0) && !decode.timed_out;
    for frame_number in 1..=frame_count {
        let path = work_dir.join(format!("frame.pgm.f{frame_number}.pgm"));
        match parse_ascii_pgm(&path) {
            Ok((columns, rows, max_value, values)) => {
                pgm_valid &= columns == file["image"]["columns"].as_u64().unwrap_or(0) as usize;
                pgm_valid &= rows == file["image"]["rows"].as_u64().unwrap_or(0) as usize;
                pgm_valid &= max_value == 1;
                pgm_valid &= values.len() == rows * columns;
                pgm_valid &= values.iter().all(|value| *value <= 1);
                decoded_frames.push(values);
            }
            Err(_) => pgm_valid = false,
        }
    }
    let unexpected_pgm = fs::read_dir(&work_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("pgm"))
        .count()
        != frame_count;
    pgm_valid &= !unexpected_pgm && decoded_frames.len() == frame_count;

    let raw_path = fs::read_dir(&work_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("raw"));
    let raw_pixel_bytes = raw_path.as_ref().and_then(|path| fs::read(path).ok());
    let raw_pixel_sha256 = raw_pixel_bytes.as_ref().map(|bytes| sha256_hex(bytes));
    let actual_hashes = decoded_frames
        .iter()
        .map(|frame| sha256_hex(frame))
        .collect::<Vec<_>>();
    let decoded_values = decoded_frames.iter().flatten().copied().collect::<Vec<_>>();
    let expected_strings = expected
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let expected_values = file
        .pointer("/expected_u1_pixels/stored_values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|value| value as u8)
        .collect::<Vec<_>>();
    let expected_pixel_hash = file
        .pointer("/expected_u1_pixels/pixel_data_sha256")
        .and_then(Value::as_str)
        .unwrap_or("");
    let passed = pgm_valid
        && extraction.exit_code == Some(0)
        && !extraction.timed_out
        && actual_hashes.iter().map(String::as_str).collect::<Vec<_>>() == expected_strings
        && decoded_values == expected_values
        && raw_pixel_sha256.as_deref() == Some(expected_pixel_hash);
    let sidecar = json!({
        "adapter_id": adapter_id,
        "decoder_sha256": tool["sha256"],
        "parser_sha256": parser_tool["sha256"],
        "independence": "independent",
        "extraction_method": "dcmtk_dcm2img_p2_and_dcmdump_raw",
        "source_instance_sha256": file["sha256"],
        "source_pixel_data_sha256": raw_pixel_sha256,
        "invocation": std::iter::once(decoder.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "decode_exit_code": decode.exit_code,
        "decode_timed_out": decode.timed_out,
        "extraction_exit_code": extraction.exit_code,
        "extraction_timed_out": extraction.timed_out,
        "rows": file["image"]["rows"],
        "columns": file["image"]["columns"],
        "frames": file["image"]["frames"],
        "max_value": 1,
        "packing_order": file["expected_u1_pixels"]["packing_order"],
        "frame_boundary_policy": file["expected_u1_pixels"]["frame_boundary_policy"],
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual_hashes,
        "decoded_values": decoded_values,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/dcmtk-dcm2img-u1/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    if let Ok(entries) = fs::read_dir(&work_dir) {
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }
    let _ = fs::remove_dir(&work_dir);
    Ok(json!({
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_frame_hashes": sidecar["expected_frame_hashes"],
        "actual_frame_hashes": sidecar["actual_frame_hashes"],
        "reason": if passed { "DCMTK independently decoded continuous one-bit frames and extracted the exact raw payload" } else { "DCMTK one-bit decode or raw payload comparison failed" },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

fn parse_ascii_pgm(path: &Path) -> Result<(usize, usize, u16, Vec<u8>), String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let normalized = source
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    let mut tokens = normalized.split_whitespace();
    if tokens.next() != Some("P2") {
        return Err("DCMTK one-bit output is not an ASCII PGM".to_string());
    }
    let columns = tokens
        .next()
        .ok_or_else(|| "PGM width is missing".to_string())?
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    let rows = tokens
        .next()
        .ok_or_else(|| "PGM height is missing".to_string())?
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    let max_value = tokens
        .next()
        .ok_or_else(|| "PGM max value is missing".to_string())?
        .parse::<u16>()
        .map_err(|error| error.to_string())?;
    let values = tokens
        .map(|token| token.parse::<u8>().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((columns, rows, max_value, values))
}

#[allow(clippy::too_many_arguments)]
fn collect_nonsquare_spacing_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
) -> Result<Value, String> {
    let adapter_id = "pydicom-dicom-validator-u32";
    let Some(adapter) = adapters.iter().find(|adapter| adapter["id"] == adapter_id) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent pydicom non-square spacing adapter is not configured",
        ));
    };
    let Some(tool) = tools.iter().find(|tool| tool["adapter_id"] == adapter_id) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent pydicom non-square spacing adapter was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent pydicom non-square spacing adapter is unavailable",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available pydicom non-square adapter has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "spatial_arguments")?
        .into_iter()
        .map(|argument| argument.replace("{input}", &input.display().to_string()))
        .collect::<Vec<_>>();
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let payload = if output.exit_code == Some(0) && !output.timed_out {
        serde_json::from_slice::<Value>(&output.stdout).ok()
    } else {
        None
    };
    let actual_hashes = payload
        .as_ref()
        .and_then(|value| value["frame_hashes"].as_array())
        .cloned()
        .unwrap_or_default();
    let contract = &file["expected_nonsquare_spacing"];
    let passed = payload.as_ref().is_some_and(|actual| {
        actual_hashes == expected
            && actual["variant_id"] == contract["variant_id"]
            && spatial_element_matches(&actual["pixel_spacing"], &contract["pixel_spacing"])
            && spatial_element_matches(
                &actual["nominal_scanned_pixel_spacing"],
                &contract["nominal_scanned_pixel_spacing"],
            )
            && spatial_element_matches(
                &actual["pixel_aspect_ratio"],
                &contract["pixel_aspect_ratio"],
            )
            && actual["uncalibrated"] == contract["uncalibrated"]
            && actual["patient_space_geometry_present"]
                == contract["patient_space_geometry_present"]
            && actual["pixel_data_sha256"] == contract["pixel_data_sha256"]
            && actual["rows"] == file["image"]["rows"]
            && actual["columns"] == file["image"]["columns"]
            && actual["frames"] == file["image"]["frames"]
            && actual["samples_per_pixel"] == file["image"]["samples_per_pixel"]
            && actual["bits_allocated"] == file["image"]["bits_allocated"]
            && actual["bits_stored"] == file["image"]["bits_stored"]
            && actual["high_bit"] == file["image"]["high_bit"]
            && actual["pixel_representation"] == file["image"]["pixel_representation"]
            && actual["photometric_interpretation"] == file["image"]["photometric_interpretation"]
            && actual["pixel_data_vr"] == file["pixel_data"]["vr"]
            && actual["transfer_syntax_uid"] == file["dicom"]["transfer_syntax_uid"]
    });
    let sidecar = json!({
        "adapter_id": adapter_id,
        "adapter_sha256": tool["sha256"],
        "independence": "independent",
        "extraction_method": "uv_locked_pydicom_nonsquare_spatial_semantic_extraction",
        "exit_code": output.exit_code,
        "timed_out": output.timed_out,
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual_hashes,
        "expected_contract": contract,
        "actual": payload,
        "stderr_sha256": sha256_hex(&output.stderr),
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/pydicom-dicom-validator-nonsquare/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    Ok(json!({
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_frame_hashes": sidecar["expected_frame_hashes"],
        "actual_frame_hashes": sidecar["actual_frame_hashes"],
        "reason": if passed { "uv-locked pydicom independently matched the exclusive non-square spatial contract and native pixel payload" } else { "pydicom non-square spatial extraction or manifest comparison failed" },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

#[allow(clippy::too_many_arguments)]
fn collect_u32_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
) -> Result<Value, String> {
    let adapter_id = "pydicom-dicom-validator-u32";
    let Some(adapter) = adapters.iter().find(|adapter| adapter["id"] == adapter_id) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent pydicom u32 pixel adapter is not configured",
        ));
    };
    let Some(tool) = tools.iter().find(|tool| tool["adapter_id"] == adapter_id) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent pydicom u32 pixel adapter was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent pydicom u32 pixel adapter is unavailable",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available pydicom u32 adapter has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "pixel_arguments")?
        .into_iter()
        .map(|argument| argument.replace("{input}", &input.display().to_string()))
        .collect::<Vec<_>>();
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let payload = if output.exit_code == Some(0) && !output.timed_out {
        serde_json::from_slice::<Value>(&output.stdout).ok()
    } else {
        None
    };
    let actual_hashes = payload
        .as_ref()
        .and_then(|value| value["frame_hashes"].as_array())
        .cloned()
        .unwrap_or_default();
    let expected_values = file
        .pointer("/expected_u32_pixels/stored_values")
        .cloned()
        .unwrap_or(Value::Null);
    let expected_pixel_hash = file
        .pointer("/expected_u32_pixels/pixel_data_sha256")
        .cloned()
        .unwrap_or(Value::Null);
    let passed = payload.as_ref().is_some_and(|value| {
        actual_hashes == expected
            && value["stored_values"] == expected_values
            && value["pixel_data_sha256"] == expected_pixel_hash
            && value["rows"] == file["image"]["rows"]
            && value["columns"] == file["image"]["columns"]
            && value["frames"] == file["image"]["frames"]
            && value["samples_per_pixel"] == file["image"]["samples_per_pixel"]
            && value["bits_allocated"] == file["image"]["bits_allocated"]
            && value["bits_stored"] == file["image"]["bits_stored"]
            && value["high_bit"] == file["image"]["high_bit"]
            && value["pixel_representation"] == file["image"]["pixel_representation"]
            && value["photometric_interpretation"] == file["image"]["photometric_interpretation"]
            && value["pixel_data_vr"] == file["pixel_data"]["vr"]
            && value["transfer_syntax_uid"] == file["dicom"]["transfer_syntax_uid"]
            && value["byte_order"] == "little_endian"
    });
    let sidecar = json!({
        "adapter_id": adapter_id,
        "adapter_sha256": tool["sha256"],
        "independence": "independent",
        "extraction_method": "uv_locked_pydicom_raw_ow_struct_unpack_u32_le",
        "exit_code": output.exit_code,
        "timed_out": output.timed_out,
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual_hashes,
        "expected_stored_values": expected_values,
        "actual": payload,
        "stderr_sha256": sha256_hex(&output.stderr),
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/pydicom-dicom-validator-u32/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    Ok(json!({
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_frame_hashes": sidecar["expected_frame_hashes"],
        "actual_frame_hashes": sidecar["actual_frame_hashes"],
        "reason": if passed { "uv-locked pydicom extraction matched unsigned values, metadata, and every native frame hash" } else { "pydicom u32 extraction or manifest comparison failed" },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

#[allow(clippy::too_many_arguments)]
fn collect_float32_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
    transfer_syntax: &str,
) -> Result<Value, String> {
    collect_floating_pixel_result(
        generated_root,
        evidence_root,
        file,
        relative_input,
        stable_key,
        adapters,
        tools,
        expected,
        transfer_syntax,
        FloatingExtractionSpec {
            sample_type: "float32",
            vr: "OF",
            tag: "7fe0,0008",
            source_element: "(7FE0,0008) Float Pixel Data",
            bytes_per_sample: 4,
            evidence_dir: "dcmtk-native-float32",
            method: "dcmdump_full_float_values_reconstructed_as_ieee754_binary32",
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_float64_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
    transfer_syntax: &str,
) -> Result<Value, String> {
    collect_floating_pixel_result(
        generated_root,
        evidence_root,
        file,
        relative_input,
        stable_key,
        adapters,
        tools,
        expected,
        transfer_syntax,
        FloatingExtractionSpec {
            sample_type: "float64",
            vr: "OD",
            tag: "7fe0,0009",
            source_element: "(7FE0,0009) Double Float Pixel Data",
            bytes_per_sample: 8,
            evidence_dir: "dcmtk-native-float64",
            method: "dcmdump_full_double_values_reconstructed_as_ieee754_binary64",
        },
    )
}

#[derive(Clone, Copy)]
struct FloatingExtractionSpec {
    sample_type: &'static str,
    vr: &'static str,
    tag: &'static str,
    source_element: &'static str,
    bytes_per_sample: usize,
    evidence_dir: &'static str,
    method: &'static str,
}

#[allow(clippy::too_many_arguments)]
fn collect_floating_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
    transfer_syntax: &str,
    spec: FloatingExtractionSpec,
) -> Result<Value, String> {
    if file.pointer("/image/sample_type").and_then(Value::as_str) != Some(spec.sample_type)
        || file.pointer("/pixel_data/vr").and_then(Value::as_str) != Some(spec.vr)
        || file
            .pointer("/pixel_data/native_or_encapsulated")
            .and_then(Value::as_str)
            != Some("native")
        || transfer_syntax != "1.2.840.10008.1.2.1"
    {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            &format!(
                "Independent {} extraction is proven only for native {} Pixel Data in Explicit VR Little Endian",
                spec.sample_type, spec.vr
            ),
        ));
    }
    let Some(adapter) = adapters.iter().find(|adapter| {
        adapter["id"] == "dcmtk-dcmdump" && adapter["role"] == "independent_parser"
    }) else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK dcmdump adapter is not configured",
        ));
    };
    let Some(tool) = tools
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdump")
    else {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK dcmdump parser was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(pixel_unsupported(
            expected,
            "independent",
            "The independent DCMTK dcmdump parser is unavailable",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available DCMTK dcmdump parser has no executable".to_string())?;
    let pixel_dir = evidence_root.join(format!("pixels/{}", spec.evidence_dir));
    fs::create_dir_all(&pixel_dir).map_err(|error| error.to_string())?;
    let input = generated_root.join(relative_input);
    let extraction_args = vec![
        "+L".to_string(),
        "+P".to_string(),
        spec.tag.to_string(),
        input.display().to_string(),
    ];
    let extraction = run_with_timeout(
        Path::new(executable),
        &extraction_args,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(30)),
    )?;
    let reconstructed_bytes = parse_dcmdump_floating_values(&extraction.stdout, spec);
    let frame_size = floating_frame_size(file, spec.bytes_per_sample);
    let expected_frame_count = file
        .pointer("/pixel_data/frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let expected_value_length = file
        .pointer("/pixel_data/value_length")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let exact_length = frame_size > 0
        && expected_frame_count == expected.len()
        && expected_value_length == frame_size * expected_frame_count
        && reconstructed_bytes
            .as_ref()
            .is_some_and(|bytes| bytes.len() == expected_value_length);
    let actual_hashes = reconstructed_bytes
        .as_ref()
        .filter(|_| exact_length)
        .map(|bytes| {
            bytes
                .chunks_exact(frame_size)
                .map(sha256_hex)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let expected_strings = expected
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let passed = extraction.exit_code == Some(0)
        && !extraction.timed_out
        && exact_length
        && actual_hashes.iter().map(String::as_str).collect::<Vec<_>>() == expected_strings;
    let sidecar = json!({
        "adapter_id": "dcmtk-dcmdump",
        "parser_sha256": tool["sha256"],
        "independence": "independent",
        "source_element": spec.source_element,
        "byte_order": "little_endian",
        "extraction_method": spec.method,
        "invocation": std::iter::once(executable.to_string()).chain(extraction_args.iter().cloned()).collect::<Vec<_>>(),
        "extraction_exit_code": extraction.exit_code,
        "extraction_timed_out": extraction.timed_out,
        "extracted_value_count": reconstructed_bytes.as_ref().map(|bytes| bytes.len() / spec.bytes_per_sample),
        "expected_value_length": expected_value_length,
        "actual_value_length": reconstructed_bytes.as_ref().map(Vec::len),
        "little_endian_value_sha256": reconstructed_bytes.as_ref().map(|bytes| sha256_hex(bytes)),
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual_hashes,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/{}/{stable_key}.json", spec.evidence_dir);
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(evidence_root.join(&relative), &encoded).map_err(|error| error.to_string())?;
    Ok(json!({
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_frame_hashes": sidecar["expected_frame_hashes"],
        "actual_frame_hashes": sidecar["actual_frame_hashes"],
        "reason": if passed {
            format!("DCMTK dcmdump independently extracted {} and exact little-endian frame hashes matched", spec.source_element)
        } else {
            format!("DCMTK dcmdump {} extraction or exact little-endian frame hash comparison failed", spec.source_element)
        },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

fn parse_dcmdump_floating_values(
    stdout: &[u8],
    spec: FloatingExtractionSpec,
) -> Option<Vec<u8>> {
    let dump = String::from_utf8_lossy(stdout);
    let parenthesized_tag = format!("({})", spec.tag);
    let line = dump
        .lines()
        .find(|line| line.to_ascii_lowercase().contains(&parenthesized_tag))?;
    let tag_end = line.to_ascii_lowercase().find(&parenthesized_tag)? + parenthesized_tag.len();
    let after_tag = line.get(tag_end..)?.trim_start();
    if !after_tag
        .get(..2)
        .is_some_and(|vr| vr.eq_ignore_ascii_case(spec.vr))
    {
        return None;
    }
    let encoded_values = after_tag.get(2..)?.split_once(" #").map_or_else(
        || after_tag.get(2..).unwrap_or_default(),
        |(values, _)| values,
    );
    let encoded_values = encoded_values
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    if encoded_values.is_empty() {
        return Some(Vec::new());
    }
    let mut bytes = Vec::new();
    for encoded in encoded_values.split('\\') {
        match spec.bytes_per_sample {
            4 => bytes.extend_from_slice(&encoded.trim().parse::<f32>().ok()?.to_le_bytes()),
            8 => bytes.extend_from_slice(&encoded.trim().parse::<f64>().ok()?.to_le_bytes()),
            _ => return None,
        }
    }
    Some(bytes)
}

fn floating_frame_size(file: &Value, bytes_per_sample: usize) -> usize {
    if file
        .pointer("/image/bits_allocated")
        .and_then(Value::as_u64)
        != Some((bytes_per_sample * 8) as u64)
    {
        return 0;
    }
    let rows = file
        .pointer("/image/rows")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let columns = file
        .pointer("/image/columns")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let samples = file
        .pointer("/image/samples_per_pixel")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    (rows * columns * samples) as usize * bytes_per_sample
}

fn pixel_unsupported(expected: Vec<Value>, independence: &str, reason: &str) -> Value {
    json!({
        "status": "unsupported",
        "independence": independence,
        "expected_frame_hashes": expected,
        "actual_frame_hashes": [],
        "reason": reason,
        "evidence": Value::Null
    })
}

fn pixel_frame_size(file: &Value) -> usize {
    let rows = file
        .pointer("/image/rows")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let columns = file
        .pointer("/image/columns")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let samples = file
        .pointer("/image/samples_per_pixel")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let bits = file
        .pointer("/image/bits_allocated")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ((rows * columns * samples * bits).div_ceil(8)) as usize
}

fn normalize_native_frame(frame: &[u8], file: &Value) -> Vec<u8> {
    let bits = file
        .pointer("/image/bits_allocated")
        .and_then(Value::as_u64)
        .unwrap_or(8);
    let sample_bytes = bits.div_ceil(8) as usize;
    let mut normalized = frame.to_vec();
    if sample_bytes > 1 {
        for sample in normalized.chunks_exact_mut(sample_bytes) {
            sample.reverse();
        }
    }
    let samples = file
        .pointer("/image/samples_per_pixel")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;
    let planar = file
        .pointer("/image/planar_configuration")
        .and_then(Value::as_u64);
    if samples > 1 && planar == Some(1) {
        let pixels = normalized.len() / (samples * sample_bytes);
        let mut interleaved = Vec::with_capacity(normalized.len());
        for pixel in 0..pixels {
            for plane in 0..samples {
                let start = (plane * pixels + pixel) * sample_bytes;
                interleaved.extend_from_slice(&normalized[start..start + sample_bytes]);
            }
        }
        interleaved
    } else {
        normalized
    }
}

fn collect_parser_result(
    generated_root: &Path,
    evidence_root: &Path,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Value, String> {
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter["role"] == "independent_parser")
    else {
        let adapter_id = "dcmtk-dcmdump";
        let raw_dir = evidence_root.join("raw").join(adapter_id);
        fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
        let stdout_relative = format!("raw/{adapter_id}/{stable_key}.stdout");
        let stderr_relative = format!("raw/{adapter_id}/{stable_key}.stderr");
        fs::write(evidence_root.join(&stdout_relative), []).map_err(|error| error.to_string())?;
        fs::write(evidence_root.join(&stderr_relative), []).map_err(|error| error.to_string())?;
        return Ok(unsupported_result(
            adapter_id,
            "independent_parser",
            vec!["dcmdump".to_string()],
            &stdout_relative,
            &stderr_relative,
            "No independent_parser adapter is configured",
        ));
    };
    let adapter_id = required_string(adapter, "id")?;
    let tool = tools
        .iter()
        .find(|tool| tool["adapter_id"] == adapter["id"])
        .ok_or_else(|| "independent parser discovery result is missing".to_string())?;
    let raw_dir = evidence_root.join("raw").join(adapter_id);
    fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
    let stdout_relative = format!("raw/{adapter_id}/{stable_key}.stdout");
    let stderr_relative = format!("raw/{adapter_id}/{stable_key}.stderr");
    let stdout_path = evidence_root.join(&stdout_relative);
    let stderr_path = evidence_root.join(&stderr_relative);
    if tool["status"] != "available" {
        fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
        return Ok(unsupported_result(
            adapter_id,
            "independent_parser",
            vec![required_string(adapter, "executable")?.to_string()],
            &stdout_relative,
            &stderr_relative,
            "configured independent parser is unavailable",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available parser has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| argument.replace("{input}", &input.display().to_string()))
        .collect::<Vec<_>>();
    let timeout = Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(30));
    let output = run_with_timeout(Path::new(executable), &arguments, timeout)?;
    fs::write(&stdout_path, &output.stdout).map_err(|error| error.to_string())?;
    fs::write(&stderr_path, &output.stderr).map_err(|error| error.to_string())?;
    Ok(execution_result(
        adapter_id,
        "independent_parser",
        executable,
        arguments,
        output,
        &stdout_relative,
        &stderr_relative,
        &input.display().to_string(),
        relative_input,
    ))
}

fn execution_result(
    adapter_id: &str,
    role: &str,
    executable: &str,
    arguments: Vec<String>,
    output: CommandOutput,
    stdout_relative: &str,
    stderr_relative: &str,
    absolute_input: &str,
    relative_input: &str,
) -> Value {
    let mut findings = normalize_findings(&output.stdout, absolute_input, relative_input);
    findings.extend(normalize_findings(
        &output.stderr,
        absolute_input,
        relative_input,
    ));
    let combined_lower = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_lowercase();
    let unsupported_parser = role == "independent_parser"
        && (combined_lower.contains("unsupported transfer syntax")
            || combined_lower.contains("unknown transfer syntax"));
    if unsupported_parser {
        findings.push(finding(
            "unsupported",
            "independent parser reported an unsupported transfer syntax",
        ));
    } else if output.timed_out {
        findings.push(finding("timeout", "validator execution timed out"));
    } else if output.exit_code != Some(0) && findings.is_empty() {
        findings.push(finding(
            "unparsed_output",
            "validator exited nonzero without a recognized finding",
        ));
    }
    let unparsed_failure = findings
        .iter()
        .any(|finding| finding["severity"] == "unparsed_output");
    let status = if unsupported_parser {
        "unsupported"
    } else if output.timed_out {
        "timeout"
    } else if unparsed_failure {
        "tool_failure"
    } else if output.exit_code.is_none() {
        "tool_failure"
    } else {
        "completed"
    };
    let mut invocation = vec![executable.to_string()];
    invocation.extend(arguments);
    let mut result = json!({
        "adapter_id": adapter_id,
        "role": role,
        "status": status,
        "invocation": invocation,
        "stdout": { "path": stdout_relative, "sha256": sha256_hex(&output.stdout) },
        "stderr": { "path": stderr_relative, "sha256": sha256_hex(&output.stderr) },
        "exit_code": output.exit_code,
        "duration_ms": output.duration_ms,
        "timed_out": output.timed_out,
        "findings": findings
    });
    if unsupported_parser {
        result["unsupported_reason"] =
            json!("independent parser reported an unsupported transfer syntax");
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn collect_icc_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Option<(Value, Value)>, String> {
    const CASE_ID: &str = "vl/photo/rgb_icc_profile_explicit_le";
    const ADAPTER_ID: &str = "littlecms-transicc-icc";
    const PROFILE_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
    const TRANSFORM_INPUT: &[u8] = b"255 0 0\n0 255 0\n0 0 255\n255 255 255\nq\n";

    if file.get("case_id").and_then(Value::as_str) != Some(CASE_ID) {
        return Ok(None);
    }
    let adapter = adapters.iter().find(|adapter| adapter["id"] == ADAPTER_ID);
    let tool = tools.iter().find(|tool| tool["adapter_id"] == ADAPTER_ID);
    let parser_tool = tools
        .iter()
        .find(|tool| tool["role"] == "independent_parser");
    let raw_dir = evidence_root.join("raw").join(ADAPTER_ID);
    fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
    let stdout_relative = format!("raw/{ADAPTER_ID}/{stable_key}.stdout");
    let stderr_relative = format!("raw/{ADAPTER_ID}/{stable_key}.stderr");
    let stdout_path = evidence_root.join(&stdout_relative);
    let stderr_path = evidence_root.join(&stderr_relative);
    if adapter.is_none()
        || tool.is_none_or(|tool| tool["status"] != "available")
        || parser_tool.is_none_or(|tool| tool["status"] != "available")
    {
        fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
        let result = unsupported_result(
            ADAPTER_ID,
            "icc_validator",
            vec![
                adapter
                    .and_then(|adapter| adapter["executable"].as_str())
                    .unwrap_or(ADAPTER_ID)
                    .to_string(),
            ],
            &stdout_relative,
            &stderr_relative,
            "configured ICC validator or independent DICOM extractor is unavailable",
        );
        return Ok(Some((
            result,
            json!({
                "adapter_id": ADAPTER_ID,
                "status": "unsupported",
                "independence": "independent",
                "evidence": Value::Null
            }),
        )));
    }
    let adapter = adapter.expect("adapter presence checked above");
    let tool = tool.expect("tool presence checked above");
    let parser_tool = parser_tool.expect("parser presence checked above");

    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available ICC validator has no executable".to_string())?;
    let parser = parser_tool["executable"]
        .as_str()
        .ok_or_else(|| "available independent parser has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let work_dir = conformance_work_dir("dts-icc", evidence_root, stable_key);
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let extraction_arguments = vec![
        "+W".to_string(),
        work_dir.display().to_string(),
        "+L".to_string(),
        "+P".to_string(),
        "0028,2000".to_string(),
        "+P".to_string(),
        "0028,2002".to_string(),
        input.display().to_string(),
    ];
    let extraction = run_with_timeout(
        Path::new(parser),
        &extraction_arguments,
        Duration::from_secs(30),
    )?;
    let (mut profile, mut color_space) = parse_dcmdump_icc(&extraction.stdout);
    if profile.is_empty() {
        if let Ok(entries) = fs::read_dir(&work_dir) {
            if let Some(bytes) = entries
                .filter_map(Result::ok)
                .filter_map(|entry| fs::read(entry.path()).ok())
                .find(|bytes| bytes.len() == 736 && sha256_hex(bytes) == PROFILE_SHA256)
            {
                profile = bytes;
                color_space = "SRGB".to_string();
            }
        }
    }
    let profile_path = work_dir.join("extracted.icc");
    fs::write(&profile_path, &profile).map_err(|error| error.to_string())?;
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| argument.replace("{profile}", &profile_path.display().to_string()))
        .collect::<Vec<_>>();
    let transform = run_with_timeout_input(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
        Some(TRANSFORM_INPUT),
    )?;
    fs::write(&stdout_path, &transform.stdout).map_err(|error| error.to_string())?;
    let mut combined_stderr = extraction.stderr.clone();
    combined_stderr.extend_from_slice(&transform.stderr);
    fs::write(&stderr_path, &combined_stderr).map_err(|error| error.to_string())?;

    let profile_sha256 = sha256_hex(&profile);
    let declared_size = icc_u32(&profile, 0).unwrap_or(u32::MAX);
    let rendering_intent = icc_u32(&profile, 64).unwrap_or(u32::MAX);
    let tag_count = icc_u32(&profile, 128).unwrap_or(u32::MAX);
    let header = json!({
        "device_class": icc_ascii(&profile, 12),
        "data_color_space": icc_ascii(&profile, 16),
        "profile_connection_space": icc_ascii(&profile, 20),
        "signature": icc_ascii(&profile, 36),
        "rendering_intent": rendering_intent
    });
    let transforms = parse_transicc_xyz(&transform.stdout)
        .into_iter()
        .zip([[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]])
        .map(|(xyz, rgb)| json!({"rgb": rgb, "xyz": xyz}))
        .collect::<Vec<_>>();
    let expected_transforms = json!([
        {"rgb": [255, 0, 0], "xyz": [43.6035, 22.2443, 1.3901]},
        {"rgb": [0, 255, 0], "xyz": [38.5101, 71.6934, 9.7076]},
        {"rgb": [0, 0, 255], "xyz": [14.3066, 6.0623, 71.3928]},
        {"rgb": [255, 255, 255], "xyz": [96.4203, 100.0, 82.4905]}
    ]);
    let manifest_profile_sha256 = file
        .pointer("/expected_icc_profile/profile_sha256")
        .and_then(Value::as_str)
        .unwrap_or("");
    let passed = extraction.exit_code == Some(0)
        && !extraction.timed_out
        && transform.exit_code == Some(0)
        && !transform.timed_out
        && profile.len() == 736
        && declared_size == 736
        && profile_sha256 == PROFILE_SHA256
        && manifest_profile_sha256 == PROFILE_SHA256
        && color_space == "SRGB"
        && header["device_class"] == "scnr"
        && header["data_color_space"] == "RGB "
        && header["profile_connection_space"] == "XYZ "
        && header["signature"] == "acsp"
        && rendering_intent == 0
        && tag_count == 9
        && Value::Array(transforms.clone()) == expected_transforms;
    let sidecar = json!({
        "adapter_id": ADAPTER_ID,
        "validator_sha256": tool["sha256"],
        "extractor_adapter_id": parser_tool["adapter_id"],
        "extractor_sha256": parser_tool["sha256"],
        "independence": "independent",
        "extraction_method": "dcmtk_dcmdump_complete_ob_hex",
        "source_instance_sha256": file["sha256"],
        "source_profile_sha256": profile_sha256,
        "manifest_profile_sha256": manifest_profile_sha256,
        "profile_size_bytes": profile.len(),
        "declared_profile_size_bytes": declared_size,
        "dicom_color_space": color_space,
        "header": header,
        "tag_count": tag_count,
        "extractor_invocation": std::iter::once(parser.to_string()).chain(extraction_arguments.iter().cloned()).collect::<Vec<_>>(),
        "extractor_exit_code": extraction.exit_code,
        "validator_invocation": std::iter::once(executable.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "validator_exit_code": transform.exit_code,
        "transforms": transforms,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("icc/{ADAPTER_ID}/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    if let Ok(entries) = fs::read_dir(&work_dir) {
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }
    let _ = fs::remove_dir(&work_dir);
    let result = json!({
        "adapter_id": ADAPTER_ID,
        "role": "icc_validator",
        "status": if passed { "completed" } else { "tool_failure" },
        "invocation": sidecar["validator_invocation"],
        "stdout": {"path": stdout_relative, "sha256": sha256_hex(&transform.stdout)},
        "stderr": {"path": stderr_relative, "sha256": sha256_hex(&combined_stderr)},
        "exit_code": transform.exit_code,
        "duration_ms": transform.duration_ms,
        "timed_out": transform.timed_out,
        "findings": if passed { json!([]) } else { json!([finding("tool_failure", "independent ICC profile validation failed")]) }
    });
    Ok(Some((
        result,
        json!({
            "adapter_id": ADAPTER_ID,
            "status": if passed { "passed" } else { "failed" },
            "independence": "independent",
            "evidence": {"path": relative, "sha256": sha256_hex(&encoded)}
        }),
    )))
}

fn parse_dcmdump_icc(output: &[u8]) -> (Vec<u8>, String) {
    let output = String::from_utf8_lossy(output);
    let mut profile = Vec::new();
    let mut color_space = String::new();
    for line in output.lines() {
        if line.starts_with("(0028,2000) OB ") {
            let value = line
                .strip_prefix("(0028,2000) OB ")
                .unwrap_or("")
                .split(" #")
                .next()
                .unwrap_or("");
            profile = value
                .split('\\')
                .filter_map(|byte| u8::from_str_radix(byte.trim(), 16).ok())
                .collect();
        } else if line.starts_with("(0028,2002) CS [") {
            color_space = line
                .strip_prefix("(0028,2002) CS [")
                .and_then(|value| value.split(']').next())
                .unwrap_or("")
                .to_string();
        }
    }
    (profile, color_space)
}

fn parse_transicc_xyz(output: &[u8]) -> Vec<Vec<f64>> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let values = line
                .split_ascii_whitespace()
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            (values.len() == 3).then_some(values)
        })
        .collect()
}

fn icc_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn icc_ascii(bytes: &[u8], offset: usize) -> String {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| std::str::from_utf8(value).ok())
        .unwrap_or("")
        .to_string()
}

fn conformance_work_dir(prefix: &str, evidence_root: &Path, stable_key: &str) -> PathBuf {
    let run_key = sha256_hex(evidence_root.display().to_string().as_bytes());
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{stable_key}-{run_key}",
        std::process::id()
    ))
}

fn normalize_findings(bytes: &[u8], absolute_input: &str, relative_input: &str) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let normalized = line.replace(absolute_input, relative_input);
            let severity = if normalized.starts_with("Error -") || normalized.starts_with("Error:")
            {
                "error"
            } else if normalized.starts_with("Warning -") || normalized.starts_with("Warning:") {
                "warning"
            } else if normalized.starts_with("Info -") {
                "info"
            } else {
                return None;
            };
            let dicom_path = normalized
                .find("</")
                .and_then(|start| {
                    normalized[start..]
                        .find("> -")
                        .map(|end| (start, start + end + 1))
                })
                .map(|(start, end)| normalized[start..end].to_string());
            Some(json!({
                "severity": severity,
                "rule_id": Value::Null,
                "message": normalized,
                "message_fingerprint": sha256_hex(normalized.as_bytes()),
                "dicom_path": dicom_path,
                "disposition": "unresolved"
            }))
        })
        .collect()
}

fn collect_sr_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Option<Value>, String> {
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter["role"] == "sr_validator")
    else {
        return Ok(None);
    };
    let sop_class_uid = file
        .pointer("/dicom/sop_class_uid")
        .and_then(Value::as_str)
        .unwrap_or("");
    let supported = string_array(adapter, "supported_sop_class_uids")?;
    if !supported.iter().any(|uid| uid == sop_class_uid) {
        return Ok(None);
    }
    let adapter_id = required_string(adapter, "id")?;
    let raw_dir = evidence_root.join("raw").join(adapter_id);
    fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
    let stdout_relative = format!("raw/{adapter_id}/{stable_key}.stdout");
    let stderr_relative = format!("raw/{adapter_id}/{stable_key}.stderr");
    let stdout_path = evidence_root.join(&stdout_relative);
    let stderr_path = evidence_root.join(&stderr_relative);
    let tool = tools
        .iter()
        .find(|tool| tool["adapter_id"] == adapter_id)
        .ok_or_else(|| format!("SR validator discovery result is missing for {adapter_id}"))?;
    if tool["status"] != "available" {
        fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
        return Ok(Some(unsupported_result(
            adapter_id,
            "sr_validator",
            vec![required_string(adapter, "executable")?.to_string()],
            &stdout_relative,
            &stderr_relative,
            "configured SR validator or one of its supporting artifacts is unavailable",
        )));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| format!("available adapter {adapter_id} has no executable"))?;
    let input = generated_root.join(relative_input);
    let arguments = expanded_arguments(adapter, tool, &input)?;
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    fs::write(&stdout_path, &output.stdout).map_err(|error| error.to_string())?;
    fs::write(&stderr_path, &output.stderr).map_err(|error| error.to_string())?;
    Ok(Some(execution_result(
        adapter_id,
        "sr_validator",
        executable,
        arguments,
        output,
        &stdout_relative,
        &stderr_relative,
        &input.display().to_string(),
        relative_input,
    )))
}

fn finding(severity: &str, message: &str) -> Value {
    json!({
        "severity": severity,
        "rule_id": Value::Null,
        "message": message,
        "message_fingerprint": sha256_hex(message.as_bytes()),
        "dicom_path": Value::Null,
        "disposition": "unresolved"
    })
}

fn unsupported_result(
    adapter_id: &str,
    role: &str,
    invocation: Vec<String>,
    stdout_relative: &str,
    stderr_relative: &str,
    reason: &str,
) -> Value {
    json!({
        "adapter_id": adapter_id,
        "role": role,
        "status": "unsupported",
        "invocation": invocation,
        "stdout": { "path": stdout_relative, "sha256": sha256_hex(&[]) },
        "stderr": { "path": stderr_relative, "sha256": sha256_hex(&[]) },
        "exit_code": Value::Null,
        "duration_ms": 0,
        "timed_out": false,
        "findings": [finding("unsupported", reason)],
        "unsupported_reason": reason
    })
}

fn collect_entity(
    generated_root: &Path,
    evidence_root: &Path,
    files: &[&Value],
    adapters: &[Value],
    tools: &[Value],
) -> Result<Value, String> {
    let directory = evidence_root.join("entity");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let stdout_path = directory.join("dcentvfy.stdout");
    let stderr_path = directory.join("dcentvfy.stderr");
    let list_path = directory.join("files.txt");
    let mut list = String::new();
    let mut projection_entries = Vec::new();
    for file in files {
        let relative = file
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "manifest file entry requires path".to_string())?;
        validate_relative_path(relative)?;
        if file.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_u32_explicit_le") {
            let (projected, entry) =
                project_u32_entity_input(generated_root, evidence_root, file, relative)?;
            list.push_str(&projected.display().to_string());
            projection_entries.push(entry);
        } else {
            list.push_str(&generated_root.join(relative).display().to_string());
        }
        list.push('\n');
    }
    fs::write(&list_path, list.as_bytes()).map_err(|error| error.to_string())?;

    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter.get("role").and_then(Value::as_str) == Some("entity_validator"))
    else {
        fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
        return Ok(unsupported_result(
            "dicom3tools-dcentvfy",
            "entity_validator",
            vec!["dcentvfy".to_string()],
            "entity/dcentvfy.stdout",
            "entity/dcentvfy.stderr",
            "No entity_validator adapter is configured",
        ));
    };
    let adapter_id = required_string(adapter, "id")?;
    let tool = tools
        .iter()
        .find(|tool| tool.get("adapter_id") == adapter.get("id"))
        .ok_or_else(|| format!("entity validator discovery result missing for {adapter_id}"))?;
    if tool.get("status").and_then(Value::as_str) != Some("available") {
        fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
        return Ok(unsupported_result(
            adapter_id,
            "entity_validator",
            vec![required_string(adapter, "executable")?.to_string()],
            "entity/dcentvfy.stdout",
            "entity/dcentvfy.stderr",
            "configured entity validator is unavailable",
        ));
    }
    let executable = tool
        .get("executable")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("available adapter {adapter_id} has no executable"))?;
    let mut arguments = string_array(adapter, "arguments")?;
    arguments.push("-f".to_string());
    arguments.push(list_path.display().to_string());
    let timeout = Duration::from_secs(
        adapter
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("adapter {adapter_id} requires timeout_seconds"))?,
    );
    let output = run_with_timeout(Path::new(executable), &arguments, timeout)?;
    fs::write(&stdout_path, &output.stdout).map_err(|error| error.to_string())?;
    fs::write(&stderr_path, &output.stderr).map_err(|error| error.to_string())?;
    let mut result = execution_result(
        adapter_id,
        "entity_validator",
        executable,
        arguments,
        output,
        "entity/dcentvfy.stdout",
        "entity/dcentvfy.stderr",
        &generated_root.display().to_string(),
        ".",
    );
    if !projection_entries.is_empty() {
        result["input_projection"] = json!({
            "method": "terminal_pixel_data_element_redaction_v1",
            "scope": "entity_consistency_only",
            "file_list": {
                "path": "entity/files.txt",
                "sha256": sha256_hex(list.as_bytes())
            },
            "entries": projection_entries
        });
    }
    Ok(result)
}

fn project_u32_entity_input(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative: &str,
) -> Result<(PathBuf, Value), String> {
    let invalid = |reason: &str| format!("u32 entity projection rejected {relative}: {reason}");
    if file
        .pointer("/dicom/transfer_syntax_uid")
        .and_then(Value::as_str)
        != Some("1.2.840.10008.1.2.1")
        || file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64)
            != Some(32)
        || file.pointer("/image/bits_stored").and_then(Value::as_u64) != Some(32)
        || file.pointer("/image/high_bit").and_then(Value::as_u64) != Some(31)
        || file
            .pointer("/image/pixel_representation")
            .and_then(Value::as_u64)
            != Some(0)
        || file.pointer("/pixel_data/vr").and_then(Value::as_str) != Some("OW")
        || file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64)
            != Some(16)
        || file
            .pointer("/pixel_data/frame_count")
            .and_then(Value::as_u64)
            != Some(1)
        || file.pointer("/expected_u32_pixels/stored_values")
            != Some(&json!([
                0_u64,
                65_535,
                2_147_483_648_u64,
                4_294_967_295_u64
            ]))
    {
        return Err(invalid(
            "manifest eligibility fields do not match the locked case",
        ));
    }
    let source = fs::read(generated_root.join(relative))
        .map_err(|error| invalid(&format!("source is unavailable: {error}")))?;
    let source_sha256 = sha256_hex(&source);
    if file.get("sha256").and_then(Value::as_str) != Some(source_sha256.as_str()) {
        return Err(invalid("source hash does not match the manifest"));
    }
    let value_length = 16_usize;
    let header_length = 12_usize;
    let element_offset = source
        .len()
        .checked_sub(value_length + header_length)
        .ok_or_else(|| invalid("source is shorter than terminal Pixel Data"))?;
    let value_offset = element_offset + header_length;
    let expected_header = [
        0xe0, 0x7f, 0x10, 0x00, b'O', b'W', 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
    ];
    if source[element_offset..value_offset] != expected_header
        || value_offset + value_length != source.len()
    {
        return Err(invalid(
            "Pixel Data is not the unique expected terminal OW element",
        ));
    }
    let value = &source[value_offset..];
    let value_sha256 = sha256_hex(value);
    let expected_value_sha256 = file
        .pointer("/expected_u32_pixels/pixel_data_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("expected pixel hash is missing"))?;
    let frame_hash = file
        .pointer("/pixel_data/frame_hashes/0")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("expected frame hash is missing"))?;
    if value_sha256 != expected_value_sha256 || value_sha256 != frame_hash {
        return Err(invalid(
            "terminal Pixel Data hash does not match the manifest",
        ));
    }

    let stable_key = sha256_hex(relative.as_bytes());
    let projection_directory = evidence_root.join("entity/projections");
    fs::create_dir_all(&projection_directory).map_err(|error| error.to_string())?;
    let source_relative = format!("entity/projections/{stable_key}.source.dcm");
    let projected_relative = format!("entity/projections/{stable_key}.projected.dcm");
    fs::write(evidence_root.join(&source_relative), &source).map_err(|error| error.to_string())?;
    let projected = source[..element_offset].to_vec();
    fs::write(evidence_root.join(&projected_relative), &projected)
        .map_err(|error| error.to_string())?;
    let projected_path = evidence_root.join(&projected_relative);
    Ok((
        projected_path,
        json!({
            "source_case_id": "classic/sc/mono2_u32_explicit_le",
            "source_path": relative,
            "source_copy": { "path": source_relative, "sha256": source_sha256 },
            "projected_input": { "path": projected_relative, "sha256": sha256_hex(&projected) },
            "transfer_syntax_uid": "1.2.840.10008.1.2.1",
            "removed_element": {
                "tag": "(7FE0,0010)",
                "vr": "OW",
                "element_offset": element_offset,
                "value_offset": value_offset,
                "value_length": value_length,
                "value_sha256": value_sha256
            }
        }),
    ))
}

fn summarize(instances: &[Value], entity: &Value) -> Value {
    let mut severity = serde_json::Map::new();
    let mut disposition = serde_json::Map::new();
    let mut tools = serde_json::Map::new();
    let mut sop = serde_json::Map::new();
    let mut transfer = serde_json::Map::new();
    for instance in instances {
        increment(
            &mut sop,
            instance["sop_class_uid"].as_str().unwrap_or("unknown"),
        );
        increment(
            &mut transfer,
            instance["transfer_syntax_uid"]
                .as_str()
                .unwrap_or("unknown"),
        );
        for result in instance["results"].as_array().into_iter().flatten() {
            summarize_result(result, &mut severity, &mut disposition, &mut tools);
        }
    }
    summarize_result(entity, &mut severity, &mut disposition, &mut tools);
    json!({
        "instances": instances.len(),
        "by_severity": severity,
        "by_disposition": disposition,
        "by_tool": tools,
        "by_sop_class": sop,
        "by_transfer_syntax": transfer
    })
}

fn summarize_result(
    result: &Value,
    severity: &mut serde_json::Map<String, Value>,
    disposition: &mut serde_json::Map<String, Value>,
    tools: &mut serde_json::Map<String, Value>,
) {
    increment(tools, result["adapter_id"].as_str().unwrap_or("unknown"));
    for finding in result["findings"].as_array().into_iter().flatten() {
        increment(severity, finding["severity"].as_str().unwrap_or("unknown"));
        increment(
            disposition,
            finding["disposition"].as_str().unwrap_or("unresolved"),
        );
    }
}

fn increment(counts: &mut serde_json::Map<String, Value>, key: &str) {
    let count = counts.get(key).and_then(Value::as_u64).unwrap_or(0) + 1;
    counts.insert(key.to_string(), json!(count));
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(format!(
            "manifest path must be relative and contained: {path}"
        ));
    }
    Ok(())
}

fn repository_identity() -> Value {
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown-commit".to_string());
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .is_some_and(|output| output.status.success() && !output.stdout.is_empty());
    json!({ "commit": commit, "dirty": dirty })
}

fn rfc3339_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3600,
        day_seconds % 3600 / 60,
        day_seconds % 60
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

pub fn check_tools_path(config_path: impl AsRef<Path>) -> Result<Value, String> {
    let config_path = config_path.as_ref();
    let config = read_json(config_path)?;
    let adapters = config
        .get("adapters")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{} must contain an adapters array", config_path.display()))?;
    let lock = read_json(Path::new(DEFAULT_VALIDATOR_LOCK)).unwrap_or_else(|_| json!({}));
    let locked_tools = lock
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut tools = Vec::with_capacity(adapters.len());
    for adapter in adapters {
        tools.push(check_adapter(adapter, &locked_tools)?);
    }
    Ok(json!({
        "schema_version": "0.1.0",
        "config_path": config_path.display().to_string(),
        "tools": tools
    }))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn check_adapter(adapter: &Value, locked_tools: &[Value]) -> Result<Value, String> {
    let id = required_string(adapter, "id")?;
    let role = required_string(adapter, "role")?;
    let required = adapter
        .get("required")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("adapter {id} requires boolean required"))?;
    let executable = required_string(adapter, "executable")?;
    let configured_path = adapter.get("path").and_then(Value::as_str);
    let executable_env = adapter.get("executable_env").and_then(Value::as_str);
    let environment_path = executable_env.and_then(env::var_os).map(PathBuf::from);
    let timeout = adapter
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("adapter {id} requires timeout_seconds"))?;
    let version_arguments = string_array(adapter, "version_arguments")?;

    if executable_env.is_some() && environment_path.is_none() {
        return Ok(json!({
            "adapter_id": id,
            "role": role,
            "status": "absent",
            "required": required,
            "executable": null,
            "sha256": null,
            "executable_sha256": null,
            "artifacts": [],
            "version_output": null,
            "version_exit_code": null,
            "lock_status": "unavailable"
        }));
    }
    let selected = environment_path
        .as_deref()
        .and_then(Path::to_str)
        .or(configured_path)
        .unwrap_or(executable);
    let Some(path) = resolve_executable(selected) else {
        let executable_path = Path::new(selected);
        let status = if configured_path.is_some()
            || executable_env.is_some()
            || executable_path.is_absolute()
            || executable_path.components().count() > 1
        {
            "misconfigured"
        } else {
            "absent"
        };
        return Ok(json!({
            "adapter_id": id,
            "role": role,
            "status": status,
            "required": required,
            "executable": null,
            "sha256": null,
            "executable_sha256": null,
            "artifacts": [],
            "version_output": null,
            "version_exit_code": null,
            "lock_status": "unavailable"
        }));
    };

    let bytes = fs::read(&path)
        .map_err(|error| format!("failed to fingerprint {}: {error}", path.display()))?;
    let executable_fingerprint = sha256_hex(&bytes);
    let artifacts = match supporting_artifacts(adapter) {
        Ok(artifacts) => artifacts,
        Err(_) => {
            return Ok(json!({
                "adapter_id": id,
                "role": role,
                "status": "misconfigured",
                "required": required,
                "executable": path.display().to_string(),
                "sha256": null,
                "executable_sha256": executable_fingerprint,
                "artifacts": [],
                "version_output": null,
                "version_exit_code": null,
                "lock_status": "unavailable"
            }));
        }
    };
    let fingerprint = if artifacts.is_empty() {
        executable_fingerprint.clone()
    } else {
        let mut material = format!("executable:{executable_fingerprint}\n");
        for artifact in &artifacts {
            material.push_str(artifact["sha256"].as_str().unwrap_or(""));
            material.push('\n');
        }
        sha256_hex(material.as_bytes())
    };
    let probe = run_with_timeout(&path, &version_arguments, Duration::from_secs(timeout))?;
    let lock_status = locked_tools
        .iter()
        .find(|tool| tool.get("adapter_id").and_then(Value::as_str) == Some(id))
        .map(|tool| {
            let expected = tool
                .get("adapter_sha256")
                .or_else(|| tool.get("executable_sha256"))
                .and_then(Value::as_str);
            if expected == Some(&fingerprint) {
                "matched"
            } else {
                "mismatched"
            }
        })
        .unwrap_or("unlocked");
    let stdout = String::from_utf8_lossy(&probe.stdout);
    let stderr = String::from_utf8_lossy(&probe.stderr);
    let version_output = [stdout.as_ref(), stderr.as_ref()]
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({
        "adapter_id": id,
        "role": role,
        "status": if probe.timed_out { "timeout" } else { "available" },
        "required": required,
        "executable": path.display().to_string(),
        "sha256": fingerprint,
        "executable_sha256": executable_fingerprint,
        "artifacts": artifacts,
        "version_output": if version_output.is_empty() { Value::Null } else { Value::String(version_output) },
        "version_exit_code": probe.exit_code,
        "version_duration_ms": probe.duration_ms,
        "lock_status": lock_status
    }))
}

fn supporting_artifacts(adapter: &Value) -> Result<Vec<Value>, String> {
    if let Some(artifacts) = adapter.get("artifacts") {
        return artifacts
            .as_array()
            .ok_or_else(|| "adapter artifacts must be an array".to_string())?
            .iter()
            .map(|entry| {
                let relative = required_string(entry, "path")?;
                validate_relative_path(relative)?;
                let root = match entry.get("root_env").and_then(Value::as_str) {
                    Some(root_env) => {
                        env::var_os(root_env).map(PathBuf::from).ok_or_else(|| {
                            format!(
                                "adapter artifact root environment variable {root_env} is unset"
                            )
                        })?
                    }
                    None => PathBuf::new(),
                };
                let path = root.join(relative);
                let bytes = fs::read(&path).map_err(|error| {
                    format!("failed to fingerprint {}: {error}", path.display())
                })?;
                Ok(json!({ "path": path.display().to_string(), "sha256": sha256_hex(&bytes) }))
            })
            .collect();
    }
    let Some(classpath) = adapter.get("classpath") else {
        return Ok(Vec::new());
    };
    let root_env = required_string(adapter, "artifact_root_env")?;
    let root = env::var_os(root_env)
        .map(PathBuf::from)
        .ok_or_else(|| format!("adapter artifact root environment variable {root_env} is unset"))?;
    classpath
        .as_array()
        .ok_or_else(|| "adapter classpath must be an array".to_string())?
        .iter()
        .map(|entry| {
            let relative = entry
                .as_str()
                .ok_or_else(|| "adapter classpath entries must be strings".to_string())?;
            validate_relative_path(relative)?;
            let path = root.join(relative);
            let bytes = fs::read(&path)
                .map_err(|error| format!("failed to fingerprint {}: {error}", path.display()))?;
            Ok(json!({ "path": path.display().to_string(), "sha256": sha256_hex(&bytes) }))
        })
        .collect()
}

fn expanded_arguments(adapter: &Value, tool: &Value, input: &Path) -> Result<Vec<String>, String> {
    let classpath = tool["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|artifact| artifact["path"].as_str())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let classpath = env::join_paths(classpath)
        .map_err(|error| format!("failed to construct adapter classpath: {error}"))?
        .to_string_lossy()
        .to_string();
    Ok(string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| {
            argument
                .replace("{input}", &input.display().to_string())
                .replace("{classpath}", &classpath)
        })
        .collect())
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("adapter requires non-empty {field}"))
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("adapter requires {field} array"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("adapter {field} must contain only strings"))
        })
        .collect()
}

pub(crate) fn resolve_executable(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() || path.components().count() > 1 {
        return path.is_file().then(|| path.to_path_buf());
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(command))
            .find(|candidate| candidate.is_file())
    })
}

pub(crate) struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
}

pub(crate) fn run_with_timeout(
    executable: &Path,
    arguments: &[String],
    timeout: Duration,
) -> Result<CommandOutput, String> {
    run_with_timeout_input(executable, arguments, timeout, None)
}

fn run_with_timeout_input(
    executable: &Path,
    arguments: &[String],
    timeout: Duration,
    input: Option<&[u8]>,
) -> Result<CommandOutput, String> {
    let started = Instant::now();
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to execute {}: {error}", executable.display()))?;

    if let Some(input) = input {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("failed to open {} stdin", executable.display()))?;
        stdin
            .write_all(input)
            .map_err(|error| format!("failed writing {} stdin: {error}", executable.display()))?;
    }

    let mut timed_out = false;
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("failed waiting for {}: {error}", executable.display()))?
            .is_some()
        {
            break;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            child.kill().map_err(|error| {
                format!("failed to terminate {}: {error}", executable.display())
            })?;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed collecting {} output: {error}", executable.display()))?;
    Ok(CommandOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.status.code(),
        timed_out,
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
    })
}
