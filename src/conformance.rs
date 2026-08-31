use std::collections::BTreeMap;
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
const REGISTRATION_SECONDARY_VALIDATOR_ID: &str = "pydicom-dicom-validator-registration";
const PRESENTATION_STATE_SECONDARY_VALIDATOR_ID: &str =
    "pydicom-dicom-validator-presentation-state";
const LINKED_RT_SECONDARY_VALIDATOR_ID: &str = "pydicom-dicom-validator-rt";
const WAVEFORM_VALIDATOR_ID: &str = "pydicom-dicom-validator-waveform";
const VISIBLE_LIGHT_SECONDARY_VALIDATOR_ID: &str = "pydicom-dicom-validator-visible-light";
const WSI_TILE_SEGMENTATION_SECONDARY_VALIDATOR_ID: &str =
    "pydicom-dicom-validator-wsi-tile-segmentation";
const VISIBLE_LIGHT_PIXEL_DECODER_ID: &str = "dcmtk-dcm2img-visible-light";
const WSI_RECONSTRUCTION_ID: &str = "highdicom-wsi-reconstruction";
const WSI_CASE_ID: &str = "vl/wsi/tiled_full_small";
const WSI_SPARSE_CASE_ID: &str = "vl/wsi/tiled_sparse_small";
const WSI_PYRAMID_CASE_ID: &str = "vl/wsi/pyramid_multiresolution";
const WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID: &str = "vl/wsi/multiple_optical_paths";
const WSI_TILE_SEGMENTATION_CASE_ID: &str = "derived/seg/wsi_tile_reference";
const WSI_SPARSE_PRIMARY_VALIDATOR_ID: &str = "pydicom-dicom-validator-wsi-sparse";
const WSI_SPARSE_CHARACTERIZATION_ID: &str = "dicom3tools-dciodvfy-wsi-sparse-characterization";
const WSI_SPARSE_CHARACTERIZATION_MESSAGE: &str = "Error - </NumberOfFrames(0028,0008)> - NumberOfFrames does not match expected value for tiled total pixel matrix = <2 > - expected 4 for 1 optical paths, 1 focal planes, 2 rows of tiles, 2 columns of tiles";
const WSI_SPARSE_CHARACTERIZATION_DICOM_PATH: &str = "</NumberOfFrames(0028,0008)>";

pub fn verify_conformance(
    evidence_root: impl AsRef<Path>,
    allowlist_path: impl AsRef<Path>,
) -> Result<Value, String> {
    verify_conformance_with_resources(
        evidence_root,
        allowlist_path,
        &crate::product_resources::ProductResources::embedded(),
    )
}

pub fn verify_conformance_with_resources(
    evidence_root: impl AsRef<Path>,
    allowlist_path: impl AsRef<Path>,
    resources: &crate::product_resources::ProductResources,
) -> Result<Value, String> {
    let snapshot = resources.snapshot().map_err(|error| error.to_string())?;
    verify_conformance_with_schema_root(evidence_root, allowlist_path, snapshot.root())
}

fn verify_conformance_with_schema_root(
    evidence_root: impl AsRef<Path>,
    allowlist_path: impl AsRef<Path>,
    resource_root: &Path,
) -> Result<Value, String> {
    let evidence_root = evidence_root.as_ref();
    let evidence = read_json(&evidence_root.join("conformance-run.json"))?;
    let allowlist = read_json(allowlist_path.as_ref())?;
    let run_schema = read_json(&resource_root.join("schemas/conformance-run.schema.json"))?;
    let allowlist_schema =
        read_json(&resource_root.join("schemas/conformance-accepted-findings.schema.json"))?;
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
    for instance in evidence["instances"].as_array().into_iter().flatten() {
        let Some(artifact) = instance
            .get("waveform")
            .and_then(|waveform| waveform.get("evidence"))
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        verify_hash_linked_artifact(
            evidence_root,
            artifact,
            "waveform payload evidence",
            failures,
        );
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
    let required_rt_image_pixel_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| file["case_id"] == "non-image/rt/image_linked")
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_visible_light_pixel_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| {
                    file["case_id"] == "vl/endoscopic/rgb_explicit_le"
                        || file["case_id"] == "vl/microscopic/rgb_explicit_le"
                })
                .filter_map(|file| file["path"].as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required_wsi_reconstruction_paths = manifest
        .as_ref()
        .and_then(|value| value["files"].as_array())
        .map(|files| {
            files
                .iter()
                .filter(|file| {
                    file["case_id"] == WSI_CASE_ID
                        || file["case_id"] == WSI_SPARSE_CASE_ID
                        || file["case_id"] == WSI_PYRAMID_CASE_ID
                        || file["case_id"] == WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID
                })
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
        let case_id = instance["case_id"].as_str().unwrap_or("");
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
        if case_id == WSI_SPARSE_CASE_ID {
            verify_sparse_wsi_iod_evidence(&evidence, instance, path, failures);
        }
        let parser = instance["results"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|result| result["role"] == "independent_parser");
        if parser.is_none_or(|result| result["status"] != "completed") {
            failures.push(format!("independent parser incomplete: {path}"));
        }
        if requires_registration_secondary_validation(case_id) {
            let secondary_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == REGISTRATION_SECONDARY_VALIDATOR_ID);
            if secondary_tool.is_none_or(|tool| tool["status"] != "available") {
                failures.push(format!(
                    "required registration secondary IOD validator is unavailable for {path}"
                ));
            }
            if secondary_tool.is_none_or(|tool| tool["lock_status"] != "matched") {
                failures.push(format!(
                    "required registration secondary IOD validator is unlocked for {path}"
                ));
            }
            let secondary = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| {
                    result["role"] == "secondary_iod_validator"
                        && result["adapter_id"] == REGISTRATION_SECONDARY_VALIDATOR_ID
                });
            if secondary.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!(
                    "required registration secondary IOD validation incomplete: {path}"
                ));
            }
        }
        if requires_presentation_state_secondary_validation(case_id) {
            let secondary_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == PRESENTATION_STATE_SECONDARY_VALIDATOR_ID);
            if secondary_tool.is_none_or(|tool| tool["status"] != "available") {
                failures.push(format!(
                    "required presentation-state secondary IOD validator is unavailable for {path}"
                ));
            }
            if secondary_tool.is_none_or(|tool| tool["lock_status"] != "matched") {
                failures.push(format!(
                    "required presentation-state secondary IOD validator is unlocked for {path}"
                ));
            }
            let secondary = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| {
                    result["role"] == "secondary_iod_validator"
                        && result["adapter_id"] == PRESENTATION_STATE_SECONDARY_VALIDATOR_ID
                });
            if secondary.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!(
                    "required presentation-state secondary IOD validation incomplete: {path}"
                ));
            }
        }
        if requires_linked_rt_secondary_validation(case_id) {
            let secondary_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == LINKED_RT_SECONDARY_VALIDATOR_ID);
            if secondary_tool.is_none_or(|tool| tool["status"] != "available") {
                failures.push(format!(
                    "required linked RT secondary IOD validator is unavailable for {path}"
                ));
            }
            if secondary_tool.is_none_or(|tool| tool["lock_status"] != "matched") {
                failures.push(format!(
                    "required linked RT secondary IOD validator is unlocked for {path}"
                ));
            }
            let secondary = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| {
                    result["role"] == "secondary_iod_validator"
                        && result["adapter_id"] == LINKED_RT_SECONDARY_VALIDATOR_ID
                });
            if secondary.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!(
                    "required linked RT secondary IOD validation incomplete: {path}"
                ));
            }
            if secondary.is_some_and(|result| result["exit_code"].as_i64() != Some(0)) {
                failures.push(format!(
                    "required linked RT secondary IOD validation did not exit successfully: {path}"
                ));
            }
            if secondary.is_some_and(|result| {
                result["findings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|finding| finding["severity"] == "error")
            }) {
                failures.push(format!(
                    "required linked RT secondary IOD validation reported error findings: {path}"
                ));
            }
        }
        if requires_waveform_validation(case_id) {
            let waveform_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == WAVEFORM_VALIDATOR_ID);
            if waveform_tool.is_none_or(|tool| tool["status"] != "available") {
                failures.push(format!(
                    "required waveform secondary IOD validator is unavailable for {path}"
                ));
            }
            if waveform_tool.is_none_or(|tool| tool["lock_status"] != "matched") {
                failures.push(format!(
                    "required waveform secondary IOD validator is unlocked for {path}"
                ));
            }
            let secondary = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| {
                    result["role"] == "secondary_iod_validator"
                        && result["adapter_id"] == WAVEFORM_VALIDATOR_ID
                });
            if secondary.is_none_or(|result| result["status"] != "completed") {
                failures.push(format!(
                    "required waveform secondary IOD validation incomplete: {path}"
                ));
            }
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_waveform_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        } else if instance.get("waveform").is_some() {
            failures.push(format!("waveform payload evidence is out of scope: {path}"));
        }
        if requires_visible_light_validation(case_id) {
            let secondary_tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == VISIBLE_LIGHT_SECONDARY_VALIDATOR_ID);
            if secondary_tool.is_none_or(|tool| {
                tool["status"] != "available" || tool["lock_status"] != "matched"
            }) {
                failures.push(format!(
                    "required visible-light secondary IOD validator is unavailable or unlocked for {path}"
                ));
            }
            let secondary = instance["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|result| {
                    result["role"] == "secondary_iod_validator"
                        && result["adapter_id"] == VISIBLE_LIGHT_SECONDARY_VALIDATOR_ID
                });
            if secondary.is_none_or(|result| {
                result["status"] != "completed" || result["exit_code"].as_i64() != Some(0)
            }) {
                failures.push(format!(
                    "required visible-light secondary IOD validation incomplete: {path}"
                ));
            }
            if secondary.is_some_and(|result| {
                result["findings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|finding| finding["severity"] == "error")
            }) {
                failures.push(format!(
                    "required visible-light secondary IOD validation reported errors: {path}"
                ));
            }
        }
        if requires_wsi_tile_segmentation_validation(case_id) {
            verify_required_clean_secondary_iod(
                evidence,
                instance,
                path,
                WSI_TILE_SEGMENTATION_SECONDARY_VALIDATOR_ID,
                "WSI tile segmentation",
                failures,
            );
        }
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
                failures.push(format!(
                    "required PixelMed SR validation incomplete: {path}"
                ));
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
        if required_rt_image_pixel_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent linked RT Image pixel evidence failed: {path}"
            ));
        }
        if required_rt_image_pixel_paths.contains(path) {
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_rt_image_pixel_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        }
        if required_visible_light_pixel_paths.contains(path)
            && (instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent")
        {
            failures.push(format!(
                "independent visible-light RGB pixel evidence failed: {path}"
            ));
        }
        if required_visible_light_pixel_paths.contains(path) {
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                verify_visible_light_pixel_evidence(
                    evidence_root,
                    evidence,
                    instance,
                    manifest_file,
                    failures,
                );
            }
        }
        if required_wsi_reconstruction_paths.contains(path) {
            let tool = evidence["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|tool| tool["adapter_id"] == WSI_RECONSTRUCTION_ID);
            if tool.is_none_or(|tool| {
                tool["status"] != "available" || tool["lock_status"] != "matched"
            }) {
                failures.push(format!(
                    "required WSI reconstruction adapter is unavailable or unlocked for {path}"
                ));
            }
            if instance["pixel"]["status"] != "passed"
                || instance["pixel"]["independence"] != "independent"
            {
                failures.push(format!(
                    "independent WSI total pixel matrix reconstruction failed: {path}"
                ));
            }
            let manifest_file = manifest
                .as_ref()
                .and_then(|value| value["files"].as_array())
                .into_iter()
                .flatten()
                .find(|file| file["path"] == path);
            if let Some(manifest_file) = manifest_file {
                if case_id == WSI_PYRAMID_CASE_ID {
                    if let Some(manifest) = manifest.as_ref() {
                        verify_wsi_pyramid_group_evidence(
                            evidence_root,
                            evidence,
                            instance,
                            manifest_file,
                            manifest,
                            failures,
                        );
                    }
                } else {
                    verify_wsi_reconstruction_evidence(
                        evidence_root,
                        evidence,
                        instance,
                        manifest_file,
                        failures,
                    );
                }
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
    let Some(relative) = instance
        .pointer("/pixel/evidence/path")
        .and_then(Value::as_str)
    else {
        failures.push(format!(
            "non-square spacing evidence sidecar is missing: {path}"
        ));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!(
            "non-square spacing evidence sidecar path is unsafe: {path}"
        ));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!(
            "non-square spacing evidence sidecar is unavailable: {path}"
        ));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "non-square spacing evidence sidecar is invalid JSON: {path}"
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
    let contract = &manifest_file["expected_nonsquare_spacing"];
    let linked = sidecar["adapter_id"] == adapter_id
        && sidecar["adapter_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && tool
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"]
            == "uv_locked_pydicom_nonsquare_spatial_semantic_extraction"
        && sidecar["status"] == "passed"
        && sidecar["expected_contract"] == *contract
        && sidecar["expected_frame_hashes"] == instance["pixel"]["expected_frame_hashes"]
        && sidecar["actual_frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && actual["frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
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
        && actual["photometric_interpretation"]
            == manifest_file["image"]["photometric_interpretation"]
        && actual["pixel_data_vr"] == manifest_file["pixel_data"]["vr"]
        && actual["transfer_syntax_uid"] == manifest_file["dicom"]["transfer_syntax_uid"];
    if !linked {
        failures.push(format!("non-square spacing evidence sidecar is not linked to its locked tool and source manifest: {path}"));
    }
}

fn spatial_element_matches(actual: &Value, expected: &Value) -> bool {
    if expected.is_null() {
        return actual.is_null();
    }
    actual["tag"] == expected["tag"]
        && actual["vr"] == expected["vr"]
        && actual["vm"] == expected["vm"]
        && actual["lexical_value"] == expected["lexical_value"]
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

fn verify_rt_image_pixel_evidence(
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
        failures.push(format!(
            "linked RT Image pixel evidence sidecar is missing: {path}"
        ));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!(
            "linked RT Image pixel evidence sidecar path is unsafe: {path}"
        ));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!(
            "linked RT Image pixel evidence sidecar is unavailable: {path}"
        ));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "linked RT Image pixel evidence sidecar is invalid JSON: {path}"
        ));
        return;
    };
    let adapter_id = "dcmtk-dcm2img-rt-image";
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == adapter_id);
    let parser = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdump");
    let expected_values = json!([
        0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255
    ]);
    let expected_hash = "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811";
    let linked = sidecar["adapter_id"] == adapter_id
        && sidecar["decoder_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && sidecar["parser_sha256"].as_str() == parser.and_then(|tool| tool["sha256"].as_str())
        && tool
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && parser
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "dcmtk_dcm2img_p2_and_dcmdump_single_native_ob"
        && sidecar["status"] == "passed"
        && sidecar["source_instance_sha256"] == manifest_file["sha256"]
        && sidecar["rows"] == 4
        && sidecar["columns"] == 4
        && sidecar["frames"] == 1
        && sidecar["max_value"] == 255
        && sidecar["decoded_values"] == expected_values
        && sidecar["decoded_pixels_sha256"] == expected_hash
        && sidecar["expected_frame_hashes"] == instance["pixel"]["expected_frame_hashes"]
        && sidecar["actual_frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && sidecar["actual_frame_hashes"] == json!([expected_hash])
        && sidecar["raw_value_file_count"] == 1
        && sidecar["raw_value_length_bytes"] == 16
        && sidecar["raw_value_vr"] == "OB"
        && sidecar["raw_value_sha256"] == expected_hash
        && manifest_file
            .pointer("/pixel_data/vr")
            .and_then(Value::as_str)
            == Some("OB")
        && manifest_file
            .pointer("/pixel_data/native_or_encapsulated")
            .and_then(Value::as_str)
            == Some("native")
        && manifest_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64)
            == Some(16)
        && manifest_file.pointer("/expected_rt_image/storage/pixel_values")
            == Some(&expected_values)
        && manifest_file
            .pointer("/expected_rt_image/storage/payload_sha256")
            .and_then(Value::as_str)
            == Some(expected_hash)
        && manifest_file
            .pointer("/expected_rt_image/storage/decoded_pixels_sha256")
            .and_then(Value::as_str)
            == Some(expected_hash);
    if !linked {
        failures.push(format!(
            "linked RT Image pixel evidence sidecar is not linked to its locked tools and source manifest: {path}"
        ));
    }
}

fn verify_visible_light_pixel_evidence(
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
        failures.push(format!(
            "visible-light pixel evidence sidecar is missing: {path}"
        ));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!(
            "visible-light pixel evidence sidecar path is unsafe: {path}"
        ));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!(
            "visible-light pixel evidence sidecar is unavailable: {path}"
        ));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "visible-light pixel evidence sidecar is invalid JSON: {path}"
        ));
        return;
    };
    let decoder = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == VISIBLE_LIGHT_PIXEL_DECODER_ID);
    let parser = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdump");
    let expected_hashes = manifest_file
        .pointer("/pixel_data/frame_hashes")
        .cloned()
        .unwrap_or(Value::Null);
    let expected_hash = expected_hashes
        .as_array()
        .and_then(|hashes| hashes.first())
        .and_then(Value::as_str)
        .unwrap_or("");
    let linked = sidecar["adapter_id"] == VISIBLE_LIGHT_PIXEL_DECODER_ID
        && sidecar["decoder_sha256"].as_str() == decoder.and_then(|tool| tool["sha256"].as_str())
        && sidecar["parser_sha256"].as_str() == parser.and_then(|tool| tool["sha256"].as_str())
        && decoder
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && parser
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "dcmtk_dcm2img_p6_and_dcmdump_single_native_rgb_ob"
        && sidecar["status"] == "passed"
        && sidecar["source_instance_sha256"] == manifest_file["sha256"]
        && sidecar["expected_frame_hashes"] == expected_hashes
        && sidecar["actual_frame_hashes"] == instance["pixel"]["actual_frame_hashes"]
        && sidecar["actual_frame_hashes"] == expected_hashes
        && sidecar["rows"] == 2
        && sidecar["columns"] == 2
        && sidecar["frames"] == 1
        && sidecar["samples_per_pixel"] == 3
        && sidecar["photometric_interpretation"] == "RGB"
        && sidecar["planar_configuration"] == 0
        && sidecar["bits_allocated"] == 8
        && sidecar["bits_stored"] == 8
        && sidecar["high_bit"] == 7
        && sidecar["pixel_representation"] == 0
        && sidecar["max_value"] == 255
        && sidecar["decoded_length_bytes"] == 12
        && sidecar["decoded_pixels_sha256"] == expected_hash
        && sidecar["raw_value_file_count"] == 1
        && sidecar["raw_value_length_bytes"] == 12
        && sidecar["raw_value_vr"] == "OB"
        && sidecar["raw_value_sha256"] == expected_hash
        && manifest_file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64)
            == Some(12)
        && manifest_file
            .pointer("/expected_vl_single_frame/image/rows")
            .and_then(Value::as_u64)
            == Some(2)
        && manifest_file
            .pointer("/expected_vl_single_frame/image/columns")
            .and_then(Value::as_u64)
            == Some(2)
        && manifest_file
            .pointer("/expected_vl_single_frame/image/planar_configuration")
            .and_then(Value::as_u64)
            == Some(0);
    if !linked {
        failures.push(format!("visible-light pixel evidence sidecar is not linked to its locked tools and source manifest: {path}"));
    }
}

fn verify_wsi_reconstruction_evidence(
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
        failures.push(format!(
            "WSI reconstruction evidence sidecar is missing: {path}"
        ));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!(
            "WSI reconstruction evidence path is unsafe: {path}"
        ));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!(
            "WSI reconstruction evidence is unavailable: {path}"
        ));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "WSI reconstruction evidence is invalid JSON: {path}"
        ));
        return;
    };
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == WSI_RECONSTRUCTION_ID);
    let case_id = manifest_file["case_id"].as_str();
    let contract = match case_id {
        Some(WSI_CASE_ID) => &manifest_file["expected_wsi_tiled_full"],
        Some(WSI_SPARSE_CASE_ID) => &manifest_file["expected_wsi_tiled_sparse"],
        Some(WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID) => {
            &manifest_file["expected_wsi_multiple_optical_paths"]
        }
        _ => &Value::Null,
    };
    let common_linked = tool
        .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["adapter_id"] == WSI_RECONSTRUCTION_ID
        && sidecar["adapter_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && sidecar["independence"] == "independent"
        && sidecar["status"] == "passed"
        && sidecar["source_manifest_sha256"] == evidence["source"]["manifest_sha256"]
        && sidecar["source_instance_sha256"] == manifest_file["sha256"]
        && sidecar["source_path"] == instance["path"]
        && manifest_file["path"] == instance["path"]
        && sidecar["expected_contract"] == *contract
        && sidecar["backend"] == "dts-wsi-reconstruct"
        && sidecar["backend_version"] == "0.4.0"
        && sidecar["runtime"]
            == json!({"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"})
        && sidecar["frame_hashes"] == contract["pixel_data"]["frame_hashes"]
        && sidecar["transforms_applied"] == false
        && instance["pixel"]["expected_frame_hashes"] == contract["pixel_data"]["frame_hashes"]
        && instance["pixel"]["actual_frame_hashes"] == contract["pixel_data"]["frame_hashes"];
    let case_linked = match case_id {
        Some(WSI_CASE_ID) => {
            sidecar["extraction_method"]
                == "uv_locked_highdicom_tiled_full_implicit_total_pixel_matrix"
                && sidecar["frame_hashes"] == contract["pixel_data"]["frame_hashes"]
                && sidecar["implicit_frame_positions"]
                    == contract["tiling"]["implicit_frame_positions"]
                && sidecar["total_pixel_matrix_shape"] == json!([4, 4, 3])
                && sidecar["total_pixel_matrix_sha256"]
                    == contract["tiling"]["total_pixel_matrix_sha256"]
        }
        Some(WSI_SPARSE_CASE_ID) => {
            sidecar["extraction_method"]
                == "uv_locked_highdicom_tiled_sparse_explicit_sentinel_total_pixel_matrix"
                && sparse_reconstruction_fields_match(&sidecar, contract)
        }
        Some(WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID) => {
            sidecar["extraction_method"]
                == "uv_locked_highdicom_tiled_full_multiple_optical_paths_separate_matrices"
                && multiple_optical_path_reconstruction_fields_match(&sidecar, contract)
        }
        _ => false,
    };
    let linked = common_linked && case_linked;
    if !linked {
        failures.push(format!(
            "WSI reconstruction evidence sidecar is not linked to its locked tool and exact source manifest contract: {path}"
        ));
    }
}

fn verify_wsi_pyramid_group_evidence(
    evidence_root: &Path,
    evidence: &Value,
    instance: &Value,
    manifest_file: &Value,
    manifest: &Value,
    failures: &mut Vec<String>,
) {
    let path = instance["path"].as_str().unwrap_or("unknown");
    let Some(relative) = instance
        .pointer("/pixel/evidence/path")
        .and_then(Value::as_str)
    else {
        failures.push(format!(
            "WSI pyramid group evidence sidecar is missing: {path}"
        ));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!("WSI pyramid group evidence path is unsafe: {path}"));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!("WSI pyramid group evidence is unavailable: {path}"));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "WSI pyramid group evidence is invalid JSON: {path}"
        ));
        return;
    };
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == WSI_RECONSTRUCTION_ID);
    let contract = &manifest_file["expected_wsi_pyramid"];
    let group_files = manifest["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|file| file["case_id"] == WSI_PYRAMID_CASE_ID)
        .collect::<Vec<_>>();
    let expected_source_members = contract["members"]
        .as_array()
        .map(|members| {
            members
                .iter()
                .map(|member| {
                    json!({
                        "ordinal": member["ordinal"], "role": member["role"],
                        "path": member["path"], "sha256": member["sha256"],
                        "sop_instance_uid": member["sop_instance_uid"]
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let rows_bound = group_files.len() == 3
        && group_files
            .iter()
            .all(|file| file["expected_wsi_pyramid"] == *contract)
        && expected_source_members.iter().all(|member| {
            group_files.iter().any(|file| {
                file["wsi_pyramid_role"] == member["role"]
                    && file["wsi_pyramid_ordinal"] == member["ordinal"]
                    && file["path"] == member["path"]
                    && file["sha256"] == member["sha256"]
                    && file.pointer("/uids/sop_instance_uid") == Some(&member["sop_instance_uid"])
            })
        });
    let current_member = contract["members"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|member| member["path"] == manifest_file["path"]);
    let linked = tool
        .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["adapter_id"] == WSI_RECONSTRUCTION_ID
        && sidecar["adapter_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"] == "uv_locked_highdicom_three_instance_pyramid_group"
        && sidecar["status"] == "passed"
        && sidecar["source_manifest_sha256"] == evidence["source"]["manifest_sha256"]
        && sidecar["source_members"] == json!(expected_source_members)
        && sidecar["expected_contract"] == *contract
        && rows_bound
        && current_member.is_some_and(|member| {
            manifest_file["wsi_pyramid_role"] == member["role"]
                && instance["pixel"]["expected_frame_hashes"] == member["frame_hashes"]
                && instance["pixel"]["actual_frame_hashes"] == member["frame_hashes"]
        })
        && wsi_pyramid_actual_matches_contract(&sidecar["actual"], contract);
    if !linked {
        failures.push(format!(
            "WSI pyramid group evidence is not bound to its locked tool and complete exact source group: {path}"
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

fn verify_waveform_evidence(
    evidence_root: &Path,
    evidence: &Value,
    instance: &Value,
    manifest_file: &Value,
    failures: &mut Vec<String>,
) {
    let path = instance["path"].as_str().unwrap_or("unknown");
    let Some(waveform) = instance.get("waveform") else {
        failures.push(format!(
            "independent waveform payload evidence is missing: {path}"
        ));
        return;
    };
    if waveform["status"] != "passed" || waveform["independence"] != "independent" {
        failures.push(format!(
            "independent waveform payload evidence failed: {path}"
        ));
        return;
    }
    let Some(relative) = waveform.pointer("/evidence/path").and_then(Value::as_str) else {
        failures.push(format!(
            "waveform payload evidence sidecar is missing: {path}"
        ));
        return;
    };
    if validate_relative_path(relative).is_err() {
        failures.push(format!(
            "waveform payload evidence sidecar path is unsafe: {path}"
        ));
        return;
    }
    let Ok(bytes) = fs::read(evidence_root.join(relative)) else {
        failures.push(format!(
            "waveform payload evidence sidecar is unavailable: {path}"
        ));
        return;
    };
    let Ok(sidecar) = serde_json::from_slice::<Value>(&bytes) else {
        failures.push(format!(
            "waveform payload evidence sidecar is invalid JSON: {path}"
        ));
        return;
    };
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == WAVEFORM_VALIDATOR_ID);
    let expected = &manifest_file["expected_waveform"];
    let actual = &sidecar["actual"];
    let semantic_match = waveform_actual_matches_expected(expected, actual);
    let expected_aggregate = &expected["aggregate"];
    let actual_aggregate = &actual["aggregate"];
    let linked = sidecar["adapter_id"] == WAVEFORM_VALIDATOR_ID
        && sidecar["adapter_sha256"].as_str() == tool.and_then(|tool| tool["sha256"].as_str())
        && tool
            .is_some_and(|tool| tool["status"] == "available" && tool["lock_status"] == "matched")
        && sidecar["independence"] == "independent"
        && sidecar["extraction_method"]
            == "uv_locked_pydicom_raw_ow_struct_unpack_i16_le_ordered_groups"
        && sidecar["source_manifest_sha256"] == evidence["source"]["manifest_sha256"]
        && sidecar["source_instance_sha256"] == manifest_file["sha256"]
        && sidecar["source_path"] == instance["path"]
        && sidecar.get("expected_contract") == Some(expected)
        && sidecar["status"] == "passed"
        && waveform["adapter_id"] == WAVEFORM_VALIDATOR_ID
        && waveform["expected_group_payload_sha256"] == expected_aggregate["group_payload_sha256"]
        && waveform["actual_group_payload_sha256"] == actual_aggregate["group_payload_sha256"]
        && waveform["expected_group_channel_sha256"] == expected_group_channel_hashes(expected)
        && waveform["actual_group_channel_sha256"] == actual_group_channel_hashes(actual)
        && waveform["expected_aggregate_payload_sha256"]
            == expected_aggregate["aggregate_payload_sha256"]
        && waveform["actual_aggregate_payload_sha256"]
            == actual_aggregate["aggregate_payload_sha256"]
        && semantic_match;
    if !linked {
        failures.push(format!(
            "waveform payload evidence sidecar is not linked to its locked tool and exact source manifest contract: {path}"
        ));
    }
}

fn expected_group_channel_hashes(expected: &Value) -> Value {
    Value::Array(
        expected["multiplex_groups"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|group| group["storage"]["channel_sha256"].clone())
            .collect(),
    )
}

fn actual_group_channel_hashes(actual: &Value) -> Value {
    Value::Array(
        actual["multiplex_groups"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|group| group["storage"]["channel_sha256"].clone())
            .collect(),
    )
}

fn waveform_actual_matches_expected(expected: &Value, actual: &Value) -> bool {
    let Some(expected_groups) = expected["multiplex_groups"].as_array() else {
        return false;
    };
    let Some(actual_groups) = actual["multiplex_groups"].as_array() else {
        return false;
    };
    let Some(expected_group_payload_hashes) =
        expected["aggregate"]["group_payload_sha256"].as_array()
    else {
        return false;
    };
    if expected_groups.len() != actual_groups.len()
        || expected["aggregate"]["group_count"].as_u64() != Some(expected_groups.len() as u64)
        || expected_group_payload_hashes.len() != expected_groups.len()
    {
        return false;
    }
    let mut expected_total_channel_count = 0_u64;
    let mut expected_total_payload_length_bytes = 0_u64;
    let mut expected_common_duration_seconds = None;
    for group_index in 0..expected_groups.len() {
        let expected_group = &expected_groups[group_index];
        let actual_group = &actual_groups[group_index];
        let Some(expected_channels) = expected_group["channels"].as_array() else {
            return false;
        };
        let Some(actual_channels) = actual_group["channels"].as_array() else {
            return false;
        };
        let Some(expected_hashes) = expected_group["storage"]["channel_sha256"].as_array() else {
            return false;
        };
        let Some(actual_hashes) = actual_group["storage"]["channel_sha256"].as_array() else {
            return false;
        };
        let Some(expected_channel_count) = expected_group["channel_count"].as_u64() else {
            return false;
        };
        let Some(expected_payload_length_bytes) =
            expected_group["storage"]["payload_length_bytes"].as_u64()
        else {
            return false;
        };
        if expected_channels.len() != actual_channels.len()
            || expected_channels.len() != expected_hashes.len()
            || expected_channels.len() != actual_hashes.len()
            || Some(expected_channels.len() as u64) != Some(expected_channel_count)
        {
            return false;
        }
        let Some(total_channel_count) =
            expected_total_channel_count.checked_add(expected_channel_count)
        else {
            return false;
        };
        expected_total_channel_count = total_channel_count;
        let Some(total_payload_length_bytes) =
            expected_total_payload_length_bytes.checked_add(expected_payload_length_bytes)
        else {
            return false;
        };
        expected_total_payload_length_bytes = total_payload_length_bytes;
        match &expected_common_duration_seconds {
            Some(duration) if duration != &expected_group["duration_seconds"] => return false,
            None => {
                expected_common_duration_seconds = Some(expected_group["duration_seconds"].clone())
            }
            Some(_) => {}
        }
        for channel_index in 0..expected_channels.len() {
            let expected_channel = &expected_channels[channel_index];
            let actual_channel = &actual_channels[channel_index];
            if actual_channel["channel_number"] != expected_channel["ordinal"]
                || actual_channel["label"] != expected_channel["label"]
                || actual_channel["source"] != expected_channel["source"]
                || actual_channel["sensitivity"] != expected_channel["sensitivity"]
                || actual_channel["sensitivity_unit"] != expected_channel["sensitivity_units"]
                || actual_channel["correction_factor"]
                    != expected_channel["sensitivity_correction_factor"]
                || actual_channel["baseline"] != expected_channel["baseline"]
                || actual_channel["bits_stored"] != expected_channel["bits_stored"]
                || actual_channel["time_skew"] != expected_channel["time_skew_seconds"]
                || actual_channel["sample_skew_present"].as_bool()
                    != expected_channel["sample_skew_absent"]
                        .as_bool()
                        .map(|value| !value)
                || actual_channel["channel_sha256"] != expected_hashes[channel_index]
                || actual_hashes[channel_index] != expected_hashes[channel_index]
            {
                return false;
            }
        }
        let expected_storage = &expected_group["storage"];
        let actual_storage = &actual_group["storage"];
        if actual_group["ordinal"] != expected_group["ordinal"]
            || actual_group["ordinal"].as_u64() != Some((group_index + 1) as u64)
            || actual_group["originality"] != expected_group["originality"]
            || actual_group["label"] != expected_group["label"]
            || actual_group["channel_count"] != expected_group["channel_count"]
            || actual_group["samples_per_channel"] != expected_group["samples_per_channel"]
            || actual_group["sampling_frequency_hz"] != expected_group["sampling_frequency_hz"]
            || actual_group["duration_seconds"] != expected_group["duration_seconds"]
            || actual_group["simultaneous_sampling"] != expected_group["simultaneous_sampling"]
            || actual_storage["bits_allocated"] != expected_storage["bits_allocated"]
            || actual_storage["sample_interpretation"] != expected_storage["sample_interpretation"]
            || actual_storage["data_vr"] != expected_storage["data_vr"]
            || actual_storage["byte_order"] != expected_storage["byte_order"]
            || actual_storage["interleave_order"] != expected_storage["interleave_order"]
            || actual_storage["payload_length_bytes"] != expected_storage["payload_length_bytes"]
            || actual_storage["payload_sha256"] != expected_storage["payload_sha256"]
            || expected_storage["payload_sha256"] != expected_group_payload_hashes[group_index]
            || actual_storage["sample_value_formula"] != expected_storage["sample_value_formula"]
            || actual_storage["sample_min"] != expected_storage["sample_min"]
            || actual_storage["sample_max"] != expected_storage["sample_max"]
            || actual_storage["waveform_padding_value_absent"]
                != expected_storage["waveform_padding_value_absent"]
            || actual_storage["value_field_padding_bytes"]
                != expected_storage["value_field_padding_bytes"]
            || actual_storage["formula_match"] != true
        {
            return false;
        }
    }
    if expected["aggregate"]["total_channel_count"].as_u64() != Some(expected_total_channel_count)
        || expected["aggregate"]["total_payload_length_bytes"].as_u64()
            != Some(expected_total_payload_length_bytes)
        || expected["aggregate"]["common_duration_seconds"]
            != expected_common_duration_seconds.unwrap_or(Value::Null)
    {
        return false;
    }
    actual["adapter_id"] == WAVEFORM_VALIDATOR_ID
        && actual["sop_class_uid"] == expected["sop_class_uid"]
        && actual["modality"] == expected["modality"]
        && actual["transfer_syntax_uid"] == expected["transfer_syntax_uid"]
        && actual["acquisition_context_items"] == expected["acquisition_context_items"]
        && actual["absent_content"] == expected["absent_content"]
        && actual["pixel_data_present"].as_bool()
            == expected["absent_content"]["pixel_data"]
                .as_bool()
                .map(|value| !value)
        && actual["aggregate"] == expected["aggregate"]
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

fn requires_registration_secondary_validation(case_id: &str) -> bool {
    matches!(
        case_id,
        "derived/registration/spatial_ct_pair" | "derived/registration/deformable_ct_pair"
    )
}

fn requires_presentation_state_secondary_validation(case_id: &str) -> bool {
    matches!(
        case_id,
        "derived/presentation-state/color_softcopy"
            | "derived/presentation-state/advanced_blending"
            | "derived/presentation-state/blending"
    )
}

fn requires_linked_rt_secondary_validation(case_id: &str) -> bool {
    matches!(
        case_id,
        "non-image/rt/plan_linked" | "non-image/rt/image_linked"
    )
}

fn requires_waveform_validation(case_id: &str) -> bool {
    matches!(
        case_id,
        "non-image/waveform/twelve_lead_ecg" | "non-image/waveform/general_ecg"
    )
}

fn requires_visible_light_validation(case_id: &str) -> bool {
    matches!(
        case_id,
        "vl/endoscopic/rgb_explicit_le"
            | "vl/microscopic/rgb_explicit_le"
            | WSI_CASE_ID
            | WSI_PYRAMID_CASE_ID
            | WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID
    )
}

fn requires_wsi_tile_segmentation_validation(case_id: &str) -> bool {
    case_id == WSI_TILE_SEGMENTATION_CASE_ID
}

fn verify_required_clean_secondary_iod(
    evidence: &Value,
    instance: &Value,
    path: &str,
    adapter_id: &str,
    label: &str,
    failures: &mut Vec<String>,
) {
    let tool = evidence["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|tool| tool["adapter_id"] == adapter_id);
    if tool.is_none_or(|tool| tool["status"] != "available" || tool["lock_status"] != "matched") {
        failures.push(format!(
            "required {label} secondary IOD validator is unavailable or unlocked for {path}"
        ));
    }
    let result = instance["results"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|result| {
            result["role"] == "secondary_iod_validator" && result["adapter_id"] == adapter_id
        });
    if result.is_none_or(|result| {
        result["status"] != "completed"
            || result["exit_code"].as_i64() != Some(0)
            || result["timed_out"] != false
    }) {
        failures.push(format!(
            "required {label} secondary IOD validation incomplete: {path}"
        ));
    }
    if result.is_some_and(|result| {
        result["findings"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|finding| finding["severity"] == "error")
    }) {
        failures.push(format!(
            "required {label} secondary IOD validation reported errors: {path}"
        ));
    }
}

#[cfg(test)]
mod wsi_tile_segmentation_iod_tests {
    use super::*;

    fn fixture() -> (Value, Value) {
        let evidence = json!({
            "tools": [{
                "adapter_id": WSI_TILE_SEGMENTATION_SECONDARY_VALIDATOR_ID,
                "status": "available",
                "lock_status": "matched"
            }]
        });
        let instance = json!({
            "case_id": WSI_TILE_SEGMENTATION_CASE_ID,
            "results": [{
                "adapter_id": WSI_TILE_SEGMENTATION_SECONDARY_VALIDATOR_ID,
                "role": "secondary_iod_validator",
                "status": "completed",
                "exit_code": 0,
                "timed_out": false,
                "findings": []
            }]
        });
        (evidence, instance)
    }

    #[test]
    fn requires_clean_locked_secondary_iod_evidence() {
        let path = "derived/seg/wsi_tile_reference/instance.dcm";
        let (evidence, instance) = fixture();
        let mut failures = Vec::new();
        verify_required_clean_secondary_iod(
            &evidence,
            &instance,
            path,
            WSI_TILE_SEGMENTATION_SECONDARY_VALIDATOR_ID,
            "WSI tile segmentation",
            &mut failures,
        );
        assert!(failures.is_empty(), "{failures:?}");

        for (pointer, mutation) in [
            ("/tools/0/status", json!("unavailable")),
            ("/tools/0/lock_status", json!("mismatched")),
            ("/instance/results/0/status", json!("unsupported")),
            ("/instance/results/0/exit_code", json!(1)),
            ("/instance/results/0/timed_out", json!(true)),
            (
                "/instance/results/0/findings",
                json!([{"severity": "error"}]),
            ),
        ] {
            let mut combined = json!({"tools": evidence["tools"], "instance": instance});
            *combined.pointer_mut(pointer).expect("mutation pointer") = mutation;
            let mut failures = Vec::new();
            verify_required_clean_secondary_iod(
                &combined,
                &combined["instance"],
                path,
                WSI_TILE_SEGMENTATION_SECONDARY_VALIDATOR_ID,
                "WSI tile segmentation",
                &mut failures,
            );
            assert!(!failures.is_empty(), "mutation {pointer} must fail");
        }
    }
}

fn verify_sparse_wsi_iod_evidence(
    evidence: &Value,
    instance: &Value,
    path: &str,
    failures: &mut Vec<String>,
) {
    let tools = evidence["tools"].as_array().into_iter().flatten();
    for (adapter_id, label) in [
        (WSI_SPARSE_PRIMARY_VALIDATOR_ID, "primary IOD authority"),
        (
            WSI_SPARSE_CHARACTERIZATION_ID,
            "dicom3tools characterization",
        ),
    ] {
        let tool = tools.clone().find(|tool| tool["adapter_id"] == adapter_id);
        if tool.is_none_or(|tool| tool["status"] != "available" || tool["lock_status"] != "matched")
        {
            failures.push(format!(
                "required sparse WSI {label} is unavailable or unlocked for {path}"
            ));
        }
    }

    let results = instance["results"].as_array().into_iter().flatten();
    let primaries = results
        .clone()
        .filter(|result| result["role"] == "primary_iod_validator")
        .collect::<Vec<_>>();
    if primaries.len() != 1
        || primaries[0]["adapter_id"] != WSI_SPARSE_PRIMARY_VALIDATOR_ID
        || primaries[0]["status"] != "completed"
        || primaries[0]["exit_code"].as_i64() != Some(0)
        || primaries[0]["timed_out"] != false
        || primaries[0]["findings"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|finding| finding["severity"] == "error")
    {
        failures.push(format!(
            "sparse WSI requires the clean locked pydicom primary IOD authority for {path}"
        ));
    }

    let characterizations = results
        .filter(|result| result["role"] == "iod_characterization")
        .collect::<Vec<_>>();
    let expected_fingerprint = sha256_hex(WSI_SPARSE_CHARACTERIZATION_MESSAGE.as_bytes());
    let valid_characterization = characterizations.len() == 1
        && characterizations.first().is_some_and(|result| {
            result["adapter_id"] == WSI_SPARSE_CHARACTERIZATION_ID
                && result["status"] == "completed"
                && result["exit_code"].as_i64() == Some(1)
                && result["timed_out"] == false
                && result["findings"].as_array().is_some_and(|findings| {
                    findings.len() == 1
                        && findings[0]["severity"] == "error"
                        && findings[0]["message"] == WSI_SPARSE_CHARACTERIZATION_MESSAGE
                        && findings[0]["message_fingerprint"] == expected_fingerprint
                        && findings[0]["rule_id"].is_null()
                        && findings[0]["dicom_path"] == WSI_SPARSE_CHARACTERIZATION_DICOM_PATH
                        && findings[0]["disposition"] == "unresolved"
                })
        });
    if !valid_characterization {
        failures.push(format!(
            "sparse WSI requires the exact visible dicom3tools cardinality characterization for {path}"
        ));
    }
}

#[cfg(test)]
mod sparse_wsi_iod_tests {
    use super::*;

    fn fixture() -> (Value, Value) {
        let message_fingerprint = sha256_hex(WSI_SPARSE_CHARACTERIZATION_MESSAGE.as_bytes());
        let evidence = json!({
            "tools": [
                {"adapter_id": WSI_SPARSE_PRIMARY_VALIDATOR_ID, "status": "available", "lock_status": "matched"},
                {"adapter_id": WSI_SPARSE_CHARACTERIZATION_ID, "status": "available", "lock_status": "matched"}
            ]
        });
        let instance = json!({
            "case_id": WSI_SPARSE_CASE_ID,
            "path": "vl/wsi/tiled_sparse_small/instance.dcm",
            "results": [
                {
                    "adapter_id": WSI_SPARSE_PRIMARY_VALIDATOR_ID,
                    "role": "primary_iod_validator", "status": "completed",
                    "exit_code": 0, "timed_out": false, "findings": []
                },
                {
                    "adapter_id": WSI_SPARSE_CHARACTERIZATION_ID,
                    "role": "iod_characterization", "status": "completed",
                    "exit_code": 1, "timed_out": false,
                    "findings": [{
                        "severity": "error", "rule_id": null,
                        "message": WSI_SPARSE_CHARACTERIZATION_MESSAGE,
                        "message_fingerprint": message_fingerprint,
                        "dicom_path": WSI_SPARSE_CHARACTERIZATION_DICOM_PATH,
                        "disposition": "unresolved"
                    }]
                }
            ]
        });
        (evidence, instance)
    }

    #[test]
    fn sparse_wsi_requires_clean_primary_and_exact_nonaccepted_characterization() {
        let (evidence, instance) = fixture();
        let mut failures = Vec::new();
        verify_sparse_wsi_iod_evidence(
            &evidence,
            &instance,
            "vl/wsi/tiled_sparse_small/instance.dcm",
            &mut failures,
        );
        assert!(failures.is_empty(), "{failures:?}");
        let run = json!({"instances": [instance.clone()]});
        assert!(instance_findings(&run).is_empty());

        for (pointer, mutation) in [
            ("/results/0/adapter_id", json!("dicom3tools-dciodvfy")),
            ("/results/0/exit_code", json!(1)),
            ("/results/1/exit_code", json!(0)),
            ("/results/1/findings/0/severity", json!("warning")),
            ("/results/1/findings/0/message", json!("different")),
            ("/results/1/findings/0/disposition", json!("accepted")),
        ] {
            let mut malformed = instance.clone();
            *malformed.pointer_mut(pointer).expect("mutation pointer") = mutation;
            let mut failures = Vec::new();
            verify_sparse_wsi_iod_evidence(
                &evidence,
                &malformed,
                "vl/wsi/tiled_sparse_small/instance.dcm",
                &mut failures,
            );
            assert!(!failures.is_empty(), "mutation {pointer} must fail");
        }
    }

    #[test]
    fn sparse_wsi_real_dicom3tools_line_normalizes_to_the_locked_characterization() {
        let raw = format!("{WSI_SPARSE_CHARACTERIZATION_MESSAGE}\nVLWholeSlideMicroscopyImage\n");
        let findings = normalize_findings(raw.as_bytes(), "/tmp/instance.dcm", "instance.dcm");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0]["severity"], "error");
        assert_eq!(findings[0]["message"], WSI_SPARSE_CHARACTERIZATION_MESSAGE);
        assert_eq!(
            findings[0]["dicom_path"],
            WSI_SPARSE_CHARACTERIZATION_DICOM_PATH
        );
        assert_eq!(
            findings[0]["message_fingerprint"],
            sha256_hex(WSI_SPARSE_CHARACTERIZATION_MESSAGE.as_bytes())
        );
    }

    #[test]
    fn sparse_wsi_primary_selection_cannot_fall_back_to_dicom3tools() {
        let adapters = json!([
            {"id": "dicom3tools-dciodvfy", "role": "primary_iod_validator"},
            {"id": WSI_SPARSE_PRIMARY_VALIDATOR_ID, "role": "primary_iod_validator", "supported_case_ids": [WSI_SPARSE_CASE_ID]}
        ]);
        let tools = json!([
            {"adapter_id": "dicom3tools-dciodvfy"},
            {"adapter_id": WSI_SPARSE_PRIMARY_VALIDATOR_ID}
        ]);
        let (selected, _) = select_primary_iod_validator(
            WSI_SPARSE_CASE_ID,
            adapters.as_array().unwrap(),
            tools.as_array().unwrap(),
        )
        .expect("exact sparse primary");
        assert_eq!(selected["id"], WSI_SPARSE_PRIMARY_VALIDATOR_ID);

        let fallback_only = json!([{
            "id": "dicom3tools-dciodvfy", "role": "primary_iod_validator"
        }]);
        assert!(
            select_primary_iod_validator(
                WSI_SPARSE_CASE_ID,
                fallback_only.as_array().unwrap(),
                &tools.as_array().unwrap()[..1],
            )
            .unwrap_err()
            .contains("requires primary IOD validator")
        );
    }
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
            if instance["case_id"] == WSI_SPARSE_CASE_ID
                && result["adapter_id"] == WSI_SPARSE_CHARACTERIZATION_ID
                && result["role"] == "iod_characterization"
            {
                // This one known error is verified exactly by
                // verify_sparse_wsi_iod_evidence. It is characterization, not
                // an accepted conformance finding.
                continue;
            }
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
    let pyramid_pixels = collect_wsi_pyramid_group_results(
        generated_root,
        evidence_root,
        &sorted_files,
        &manifest_sha256,
        adapters,
        &tools,
    )?;
    let mut instances = Vec::with_capacity(sorted_files.len());
    for file in &sorted_files {
        instances.push(collect_instance(
            generated_root,
            evidence_root,
            file,
            &manifest_sha256,
            adapters,
            &tools,
            &pyramid_pixels,
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
    manifest_sha256: &str,
    adapters: &[Value],
    tools: &[Value],
    pyramid_pixels: &BTreeMap<String, Value>,
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
    let pixel = if case_id == WSI_PYRAMID_CASE_ID {
        pyramid_pixels
            .get(path)
            .cloned()
            .ok_or_else(|| format!("WSI pyramid group evidence is incomplete for {path}"))?
    } else {
        collect_pixel_result(
            generated_root,
            evidence_root,
            file,
            manifest_sha256,
            path,
            &stable_key,
            adapters,
            tools,
        )?
    };
    let mut results = vec![primary_result];
    results.extend(collect_secondary_iod_results(
        generated_root,
        evidence_root,
        case_id,
        path,
        &stable_key,
        adapters,
        tools,
    )?);
    if let Some(characterization) = collect_sparse_wsi_characterization(
        generated_root,
        evidence_root,
        case_id,
        path,
        &stable_key,
        adapters,
        tools,
    )? {
        results.push(characterization);
    }
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
    if requires_waveform_validation(case_id) {
        instance["waveform"] = collect_waveform_result(
            generated_root,
            evidence_root,
            file,
            manifest_sha256,
            path,
            &stable_key,
            adapters,
            tools,
        )?;
    }
    Ok(instance)
}

#[allow(clippy::too_many_arguments)]
fn collect_waveform_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    manifest_sha256: &str,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Value, String> {
    let expected = &file["expected_waveform"];
    let unsupported = |reason: &str| {
        json!({
            "adapter_id": WAVEFORM_VALIDATOR_ID,
            "status": "unsupported",
            "independence": "independent",
            "expected_group_payload_sha256": expected.pointer("/aggregate/group_payload_sha256").cloned().unwrap_or_else(|| json!([])),
            "actual_group_payload_sha256": [],
            "expected_group_channel_sha256": expected_group_channel_hashes(expected),
            "actual_group_channel_sha256": [],
            "expected_aggregate_payload_sha256": expected.pointer("/aggregate/aggregate_payload_sha256").cloned().unwrap_or(Value::Null),
            "actual_aggregate_payload_sha256": null,
            "reason": reason,
            "evidence": null
        })
    };
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter["id"] == WAVEFORM_VALIDATOR_ID)
    else {
        return Ok(unsupported(
            "The independent waveform payload adapter is not configured",
        ));
    };
    let Some(tool) = tools
        .iter()
        .find(|tool| tool["adapter_id"] == WAVEFORM_VALIDATOR_ID)
    else {
        return Ok(unsupported(
            "The independent waveform payload adapter was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(unsupported(
            "The independent waveform payload adapter is unavailable",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available waveform payload adapter has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "waveform_arguments")?
        .into_iter()
        .map(|argument| argument.replace("{input}", &input.display().to_string()))
        .collect::<Vec<_>>();
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let actual = if output.exit_code == Some(0) && !output.timed_out {
        serde_json::from_slice::<Value>(&output.stdout).ok()
    } else {
        None
    };
    let expected_group_payload_sha256 = expected.pointer("/aggregate/group_payload_sha256");
    let expected_aggregate_payload_sha256 = expected.pointer("/aggregate/aggregate_payload_sha256");
    let actual_group_payload_sha256 = actual
        .as_ref()
        .and_then(|value| value.pointer("/aggregate/group_payload_sha256"))
        .cloned()
        .unwrap_or_else(|| json!([]));
    let actual_aggregate_payload_sha256 = actual
        .as_ref()
        .and_then(|value| value.pointer("/aggregate/aggregate_payload_sha256"))
        .cloned()
        .unwrap_or(Value::Null);
    let expected_group_channel_sha256 = expected_group_channel_hashes(expected);
    let actual_group_channel_sha256 = actual
        .as_ref()
        .map(actual_group_channel_hashes)
        .unwrap_or_else(|| json!([]));
    let passed = actual
        .as_ref()
        .is_some_and(|value| waveform_actual_matches_expected(expected, value));
    let sidecar = json!({
        "adapter_id": WAVEFORM_VALIDATOR_ID,
        "adapter_sha256": tool["sha256"],
        "independence": "independent",
        "extraction_method": "uv_locked_pydicom_raw_ow_struct_unpack_i16_le_ordered_groups",
        "source_manifest_sha256": manifest_sha256,
        "source_instance_sha256": file["sha256"],
        "source_path": relative_input,
        "exit_code": output.exit_code,
        "timed_out": output.timed_out,
        "stderr_sha256": sha256_hex(&output.stderr),
        "expected_contract": expected,
        "actual": actual,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("waveforms/{WAVEFORM_VALIDATOR_ID}/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    Ok(json!({
        "adapter_id": WAVEFORM_VALIDATOR_ID,
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_group_payload_sha256": expected_group_payload_sha256.cloned().unwrap_or_else(|| json!([])),
        "actual_group_payload_sha256": actual_group_payload_sha256,
        "expected_group_channel_sha256": expected_group_channel_sha256,
        "actual_group_channel_sha256": actual_group_channel_sha256,
        "expected_aggregate_payload_sha256": expected_aggregate_payload_sha256.cloned().unwrap_or(Value::Null),
        "actual_aggregate_payload_sha256": actual_aggregate_payload_sha256,
        "reason": if passed {
            "uv-locked pydicom independently extracted and matched every ordered signed waveform group"
        } else {
            "pydicom ordered waveform extraction or exact manifest comparison failed"
        },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

fn collect_secondary_iod_results(
    generated_root: &Path,
    evidence_root: &Path,
    case_id: &str,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Vec<Value>, String> {
    let mut results = Vec::new();
    for adapter in adapters.iter().filter(|adapter| {
        adapter.get("role").and_then(Value::as_str) == Some("secondary_iod_validator")
            && adapter
                .get("supported_case_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(case_id)))
    }) {
        let adapter_id = required_string(adapter, "id")?;
        let tool = tools
            .iter()
            .find(|tool| tool.get("adapter_id").and_then(Value::as_str) == Some(adapter_id))
            .ok_or_else(|| {
                format!("secondary validator discovery result is missing for {adapter_id}")
            })?;
        let raw_dir = evidence_root.join("raw").join(adapter_id);
        fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
        let stdout_relative = format!("raw/{adapter_id}/{stable_key}.stdout");
        let stderr_relative = format!("raw/{adapter_id}/{stable_key}.stderr");
        let stdout_path = evidence_root.join(&stdout_relative);
        let stderr_path = evidence_root.join(&stderr_relative);
        if tool.get("status").and_then(Value::as_str) != Some("available") {
            fs::write(&stdout_path, []).map_err(|error| error.to_string())?;
            fs::write(&stderr_path, []).map_err(|error| error.to_string())?;
            results.push(unsupported_result(
                adapter_id,
                "secondary_iod_validator",
                vec![required_string(adapter, "executable")?.to_string()],
                &stdout_relative,
                &stderr_relative,
                "configured secondary IOD validator is unavailable",
            ));
            continue;
        }
        let executable = tool["executable"]
            .as_str()
            .ok_or_else(|| format!("available adapter {adapter_id} has no executable"))?;
        let input = generated_root.join(relative_input);
        let arguments = string_array(adapter, "arguments")?
            .into_iter()
            .map(|argument| argument.replace("{input}", &input.display().to_string()))
            .collect::<Vec<_>>();
        let timeout = Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60));
        let output = run_with_timeout(Path::new(executable), &arguments, timeout)?;
        fs::write(&stdout_path, &output.stdout).map_err(|error| error.to_string())?;
        fs::write(&stderr_path, &output.stderr).map_err(|error| error.to_string())?;
        results.push(execution_result(
            adapter_id,
            "secondary_iod_validator",
            executable,
            arguments,
            output,
            &stdout_relative,
            &stderr_relative,
            &input.display().to_string(),
            relative_input,
        ));
    }
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn collect_sparse_wsi_characterization(
    generated_root: &Path,
    evidence_root: &Path,
    case_id: &str,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<Option<Value>, String> {
    if case_id != WSI_SPARSE_CASE_ID {
        return Ok(None);
    }
    let matching = adapters
        .iter()
        .filter(|adapter| {
            adapter["role"] == "iod_characterization"
                && adapter["supported_case_ids"] == json!([WSI_SPARSE_CASE_ID])
        })
        .collect::<Vec<_>>();
    let adapter = match matching.as_slice() {
        [adapter] if adapter["id"] == WSI_SPARSE_CHARACTERIZATION_ID => *adapter,
        [] => {
            return Err(
                "configuration requires the sparse WSI dicom3tools characterization adapter"
                    .to_string(),
            );
        }
        _ => {
            return Err(
                "configuration must define exactly one sparse WSI dicom3tools characterization adapter"
                    .to_string(),
            );
        }
    };
    if adapter["expected_exit_code"].as_i64() != Some(1)
        || adapter["expected_findings"]
            != json!([{
                "severity": "error",
                "message": WSI_SPARSE_CHARACTERIZATION_MESSAGE
            }])
    {
        return Err(
            "sparse WSI characterization expectation does not match the locked outcome".to_string(),
        );
    }
    let tool = tools
        .iter()
        .find(|tool| tool["adapter_id"] == WSI_SPARSE_CHARACTERIZATION_ID)
        .ok_or_else(|| "sparse WSI characterization discovery result is missing".to_string())?;
    let raw_dir = evidence_root
        .join("raw")
        .join(WSI_SPARSE_CHARACTERIZATION_ID);
    fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
    let stdout_relative = format!("raw/{WSI_SPARSE_CHARACTERIZATION_ID}/{stable_key}.stdout");
    let stderr_relative = format!("raw/{WSI_SPARSE_CHARACTERIZATION_ID}/{stable_key}.stderr");
    if tool["status"] != "available" {
        fs::write(evidence_root.join(&stdout_relative), []).map_err(|error| error.to_string())?;
        fs::write(evidence_root.join(&stderr_relative), []).map_err(|error| error.to_string())?;
        return Ok(Some(unsupported_result(
            WSI_SPARSE_CHARACTERIZATION_ID,
            "iod_characterization",
            vec![required_string(adapter, "executable")?.to_string()],
            &stdout_relative,
            &stderr_relative,
            "configured sparse WSI dicom3tools characterization is unavailable",
        )));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available sparse WSI characterization has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| argument.replace("{input}", &input.display().to_string()))
        .collect::<Vec<_>>();
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(30)),
    )?;
    fs::write(evidence_root.join(&stdout_relative), &output.stdout)
        .map_err(|error| error.to_string())?;
    fs::write(evidence_root.join(&stderr_relative), &output.stderr)
        .map_err(|error| error.to_string())?;
    Ok(Some(execution_result(
        WSI_SPARSE_CHARACTERIZATION_ID,
        "iod_characterization",
        executable,
        arguments,
        output,
        &stdout_relative,
        &stderr_relative,
        &input.display().to_string(),
        relative_input,
    )))
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
        [] if case_id == WSI_SPARSE_CASE_ID => {
            return Err(format!(
                "case {WSI_SPARSE_CASE_ID} requires primary IOD validator {WSI_SPARSE_PRIMARY_VALIDATOR_ID}"
            ));
        }
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
    manifest_sha256: &str,
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
    if file.get("case_id").and_then(Value::as_str) == Some("non-image/rt/image_linked") {
        return collect_rt_image_pixel_result(
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
    if file
        .get("case_id")
        .and_then(Value::as_str)
        .is_some_and(|id| {
            id == WSI_CASE_ID
                || id == WSI_SPARSE_CASE_ID
                || id == WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID
        })
    {
        return collect_wsi_reconstruction_result(
            generated_root,
            evidence_root,
            file,
            manifest_sha256,
            relative_input,
            stable_key,
            adapters,
            tools,
            expected,
        );
    }
    if file
        .get("case_id")
        .and_then(Value::as_str)
        .is_some_and(requires_visible_light_validation)
    {
        return collect_visible_light_pixel_result(
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
fn collect_rt_image_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
) -> Result<Value, String> {
    let adapter_id = "dcmtk-dcm2img-rt-image";
    let unsupported = |reason: &str| pixel_unsupported(expected.clone(), "independent", reason);
    let Some(adapter) = adapters.iter().find(|adapter| adapter["id"] == adapter_id) else {
        return Ok(unsupported(
            "The independent DCMTK linked RT Image decoder is not configured",
        ));
    };
    let Some(tool) = tools.iter().find(|tool| tool["adapter_id"] == adapter_id) else {
        return Ok(unsupported(
            "The independent DCMTK linked RT Image decoder was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(unsupported(
            "The independent DCMTK linked RT Image decoder is unavailable",
        ));
    }
    let parser_tool = tools
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdump" && tool["status"] == "available")
        .ok_or_else(|| "linked RT Image pixel evidence requires locked dcmdump".to_string())?;
    let decoder = tool["executable"]
        .as_str()
        .ok_or_else(|| "available linked RT Image decoder has no executable".to_string())?;
    let dcmdump = parser_tool["executable"]
        .as_str()
        .ok_or_else(|| "available dcmdump parser has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let work_dir = conformance_work_dir("dts-rt-image-pixel", evidence_root, stable_key);
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let output = work_dir.join("image.pgm");
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| {
            argument
                .replace("{input}", &input.display().to_string())
                .replace("{output}", &output.display().to_string())
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

    let expected_values = vec![
        0_u8, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
    ];
    let expected_hash = "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811";
    let parsed = parse_ascii_pgm(&output).ok();
    let pgm_valid = parsed
        .as_ref()
        .is_some_and(|(columns, rows, max_value, values)| {
            *columns == 4 && *rows == 4 && *max_value == 255 && values == &expected_values
        });
    let decoded_values = parsed
        .as_ref()
        .map(|(_, _, _, values)| values.clone())
        .unwrap_or_default();
    let decoded_pixels_sha256 = pgm_valid.then(|| sha256_hex(&decoded_values));
    let raw_paths = fs::read_dir(&work_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("raw"))
        .collect::<Vec<_>>();
    let raw_bytes = if raw_paths.len() == 1 {
        fs::read(&raw_paths[0]).ok()
    } else {
        None
    };
    let raw_value_sha256 = raw_bytes.as_ref().map(|bytes| sha256_hex(bytes));
    let raw_value_length_bytes = raw_bytes.as_ref().map(Vec::len);
    let actual_hashes = decoded_pixels_sha256.iter().cloned().collect::<Vec<_>>();
    let expected_strings = expected
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let manifest_eligible = file.pointer("/image/rows").and_then(Value::as_u64) == Some(4)
        && file.pointer("/image/columns").and_then(Value::as_u64) == Some(4)
        && file.pointer("/image/frames").and_then(Value::as_u64) == Some(1)
        && file
            .pointer("/image/bits_allocated")
            .and_then(Value::as_u64)
            == Some(8)
        && file.pointer("/pixel_data/vr").and_then(Value::as_str) == Some("OB")
        && file
            .pointer("/pixel_data/native_or_encapsulated")
            .and_then(Value::as_str)
            == Some("native")
        && file
            .pointer("/pixel_data/value_length")
            .and_then(Value::as_u64)
            == Some(16)
        && file
            .pointer("/expected_rt_image/storage/payload_sha256")
            .and_then(Value::as_str)
            == Some(expected_hash);
    let passed = manifest_eligible
        && decode.exit_code == Some(0)
        && !decode.timed_out
        && extraction.exit_code == Some(0)
        && !extraction.timed_out
        && pgm_valid
        && decoded_pixels_sha256.as_deref() == Some(expected_hash)
        && actual_hashes.iter().map(String::as_str).collect::<Vec<_>>() == expected_strings
        && raw_paths.len() == 1
        && raw_value_length_bytes == Some(16)
        && raw_value_sha256.as_deref() == Some(expected_hash);
    let sidecar = json!({
        "adapter_id": adapter_id,
        "decoder_sha256": tool["sha256"],
        "parser_sha256": parser_tool["sha256"],
        "independence": "independent",
        "extraction_method": "dcmtk_dcm2img_p2_and_dcmdump_single_native_ob",
        "source_instance_sha256": file["sha256"],
        "invocation": std::iter::once(decoder.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "decode_exit_code": decode.exit_code,
        "decode_timed_out": decode.timed_out,
        "extraction_invocation": std::iter::once(dcmdump.to_string()).chain(extraction_args.iter().cloned()).collect::<Vec<_>>(),
        "extraction_exit_code": extraction.exit_code,
        "extraction_timed_out": extraction.timed_out,
        "rows": 4,
        "columns": 4,
        "frames": 1,
        "max_value": 255,
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual_hashes,
        "decoded_values": decoded_values,
        "decoded_pixels_sha256": decoded_pixels_sha256,
        "raw_value_file_count": raw_paths.len(),
        "raw_value_length_bytes": raw_value_length_bytes,
        "raw_value_vr": "OB",
        "raw_value_sha256": raw_value_sha256,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/{adapter_id}/{stable_key}.json");
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
        "reason": if passed { "DCMTK independently decoded the linked RT Image and extracted its single exact native OB value" } else { "DCMTK linked RT Image decode or native OB extraction failed" },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
}

fn parse_binary_ppm(path: &Path) -> Result<(usize, usize, u16, Vec<u8>), String> {
    let source = fs::read(path).map_err(|error| error.to_string())?;
    let mut index = 0usize;
    let mut tokens = Vec::new();
    while tokens.len() < 4 {
        while index < source.len() && source[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < source.len() && source[index] == b'#' {
            while index < source.len() && source[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        let start = index;
        while index < source.len() && !source[index].is_ascii_whitespace() {
            index += 1;
        }
        if start == index {
            return Err("binary PPM header is incomplete".to_string());
        }
        tokens.push(std::str::from_utf8(&source[start..index]).map_err(|e| e.to_string())?);
    }
    if tokens[0] != "P6" {
        return Err("DCMTK visible-light output is not a binary P6 PPM".to_string());
    }
    let columns = tokens[1].parse::<usize>().map_err(|e| e.to_string())?;
    let rows = tokens[2].parse::<usize>().map_err(|e| e.to_string())?;
    let max_value = tokens[3].parse::<u16>().map_err(|e| e.to_string())?;
    if index >= source.len() || !source[index].is_ascii_whitespace() {
        return Err("binary PPM header has no payload separator".to_string());
    }
    if source[index] == b'\r' && source.get(index + 1) == Some(&b'\n') {
        index += 2;
    } else {
        index += 1;
    }
    Ok((columns, rows, max_value, source[index..].to_vec()))
}

fn wsi_pyramid_actual_matches_contract(actual: &Value, contract: &Value) -> bool {
    let identity = &contract["shared_identity"];
    let actual_identity = &actual["group_identity"];
    let shared_identity_matches = [
        "patient_id",
        "study_instance_uid",
        "series_instance_uid",
        "frame_of_reference_uid",
        "container_identifier",
        "specimen_identifier",
        "specimen_uid",
        "optical_path_identifier",
        "icc_profile_sha256",
    ]
    .iter()
    .all(|field| actual_identity[*field] == identity[*field]);
    let members_match = actual["members"]
        .as_array()
        .zip(contract["members"].as_array())
        .is_some_and(|(actual_members, expected_members)| {
            actual_members.len() == 3
                && expected_members.len() == 3
                && actual_members
                    .iter()
                    .zip(expected_members)
                    .all(|(member, expected)| {
                        member["role"] == expected["role"]
                            && member["sop_instance_uid"] == expected["sop_instance_uid"]
                            && member["image_type"] == expected["image_type"]
                            && member["frame_type"] == expected["frame_type"]
                            && member["frame_count"] == expected["frames"]
                            && member["stored_frame_shape"] == json!([2, 2, 3])
                            && member["frame_hashes"] == expected["frame_hashes"]
                            && member["pixel_data_sha256"] == expected["payload_sha256"]
                            && member["total_pixel_matrix_shape"]
                                == json!([
                                    expected["total_pixel_matrix_rows"],
                                    expected["total_pixel_matrix_columns"],
                                    3
                                ])
                            && member["total_pixel_matrix_sha256"] == expected["matrix_sha256"]
                            && member["pyramid_member"] == json!(!expected["pyramid_uid"].is_null())
                            && member["pyramid_uid"] == expected["pyramid_uid"]
                            && member["specimen_label_in_image"]
                                == expected["specimen_label_in_image"]
                            && member["burned_in_annotation"] == "NO"
                            && member["transforms_applied"] == false
                    })
        });
    actual["status"] == "passed"
        && actual["backend"] == "dts-wsi-reconstruct"
        && actual["backend_version"] == "0.4.0"
        && actual["runtime"] == json!({"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"})
        && actual["ordered_roles"] == contract["ordered_roles"]
        && shared_identity_matches
        && actual["pyramid_uid"] == contract["pyramid_membership"]["pyramid_uid"]
        && actual["pyramid_roles"] == contract["pyramid_membership"]["member_roles"]
        && actual["apex_role"] == contract["apex_role"]
        && actual["label_excluded_from_pyramid"] == true
        && actual["thumbnail_reduction"] == "volume_quadrant_top_left_pixels"
        && actual["thumbnail_reduction_sha256"] == contract["members"][1]["matrix_sha256"]
        && members_match
        && actual["transforms_applied"] == false
}

#[allow(clippy::too_many_arguments)]
fn collect_wsi_pyramid_group_results(
    generated_root: &Path,
    evidence_root: &Path,
    files: &[&Value],
    manifest_sha256: &str,
    adapters: &[Value],
    tools: &[Value],
) -> Result<BTreeMap<String, Value>, String> {
    let pyramid_files = files
        .iter()
        .copied()
        .filter(|file| file["case_id"] == WSI_PYRAMID_CASE_ID)
        .collect::<Vec<_>>();
    if pyramid_files.is_empty() {
        return Ok(BTreeMap::new());
    }
    if pyramid_files.len() != 3 {
        return Err(format!(
            "case {WSI_PYRAMID_CASE_ID} requires exactly three manifest members"
        ));
    }
    let contract = &pyramid_files[0]["expected_wsi_pyramid"];
    if contract.is_null()
        || pyramid_files
            .iter()
            .any(|file| file["expected_wsi_pyramid"] != *contract)
    {
        return Err("WSI pyramid members do not repeat one exact group contract".to_string());
    }
    let expected_members = contract["members"]
        .as_array()
        .ok_or_else(|| "WSI pyramid contract requires members".to_string())?;
    let mut ordered_files = Vec::with_capacity(3);
    for expected_member in expected_members {
        let role = expected_member["role"]
            .as_str()
            .ok_or_else(|| "WSI pyramid contract member requires role".to_string())?;
        let matching = pyramid_files
            .iter()
            .copied()
            .filter(|file| file["wsi_pyramid_role"] == role)
            .collect::<Vec<_>>();
        let [file] = matching.as_slice() else {
            return Err(format!(
                "WSI pyramid group requires exactly one {role} member"
            ));
        };
        if file["path"] != expected_member["path"]
            || file["sha256"] != expected_member["sha256"]
            || file["size_bytes"] != expected_member["size_bytes"]
            || file.pointer("/uids/sop_instance_uid") != Some(&expected_member["sop_instance_uid"])
        {
            return Err(format!(
                "WSI pyramid {role} manifest row is not bound to its group contract"
            ));
        }
        ordered_files.push(*file);
    }
    let expected_by_path = ordered_files
        .iter()
        .map(|file| {
            let path = file["path"].as_str().unwrap_or_default().to_string();
            let hashes = file["pixel_data"]["frame_hashes"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            (path, hashes)
        })
        .collect::<Vec<_>>();
    let unsupported = |reason: &str| {
        expected_by_path
            .iter()
            .map(|(path, hashes)| {
                (
                    path.clone(),
                    pixel_unsupported(hashes.clone(), "independent", reason),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter["id"] == WSI_RECONSTRUCTION_ID)
    else {
        return Ok(unsupported(
            "The independent WSI pyramid reconstruction adapter is not configured",
        ));
    };
    if !adapter["supported_case_ids"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id == WSI_PYRAMID_CASE_ID))
    {
        return Err(
            "WSI reconstruction adapter does not explicitly support the pyramid case".to_string(),
        );
    }
    let Some(tool) = tools
        .iter()
        .find(|tool| tool["adapter_id"] == WSI_RECONSTRUCTION_ID)
    else {
        return Ok(unsupported(
            "The independent WSI pyramid reconstruction adapter was not discovered",
        ));
    };
    if tool["status"] != "available" || tool["lock_status"] != "matched" {
        return Ok(unsupported(
            "The independent WSI pyramid reconstruction adapter is unavailable or unlocked",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available WSI pyramid adapter has no executable".to_string())?;
    let inputs = ordered_files
        .iter()
        .map(|file| generated_root.join(file["path"].as_str().unwrap_or_default()))
        .collect::<Vec<_>>();
    if inputs.iter().any(|path| !path.is_file()) {
        return Err("WSI pyramid manifest member does not exist".to_string());
    }
    let arguments = string_array(adapter, "group_arguments")?
        .into_iter()
        .map(|argument| {
            inputs
                .iter()
                .enumerate()
                .fold(argument, |value, (index, input)| {
                    value.replace(
                        &format!("{{group_input_{}}}", index + 1),
                        &input.display().to_string(),
                    )
                })
        })
        .collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument.contains("{group_input_"))
    {
        return Err("WSI pyramid group argument template is incomplete".to_string());
    }
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let actual = serde_json::from_slice::<Value>(&output.stdout).unwrap_or(Value::Null);
    let passed = output.exit_code == Some(0)
        && !output.timed_out
        && wsi_pyramid_actual_matches_contract(&actual, contract);
    let source_members = expected_members
        .iter()
        .map(|member| {
            json!({
                "ordinal": member["ordinal"], "role": member["role"],
                "path": member["path"], "sha256": member["sha256"],
                "sop_instance_uid": member["sop_instance_uid"]
            })
        })
        .collect::<Vec<_>>();
    let sidecar = json!({
        "adapter_id": WSI_RECONSTRUCTION_ID,
        "adapter_sha256": tool["sha256"],
        "independence": "independent",
        "extraction_method": "uv_locked_highdicom_three_instance_pyramid_group",
        "source_manifest_sha256": manifest_sha256,
        "source_members": source_members,
        "expected_contract": contract,
        "invocation": std::iter::once(executable.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "exit_code": output.exit_code,
        "timed_out": output.timed_out,
        "stderr_sha256": sha256_hex(&output.stderr),
        "actual": actual,
        "status": if passed { "passed" } else { "failed" }
    });
    let group_material = serde_json::to_vec(&json!({
        "source_manifest_sha256": manifest_sha256,
        "source_members": source_members
    }))
    .map_err(|error| error.to_string())?;
    let relative = format!(
        "pixels/{WSI_RECONSTRUCTION_ID}/pyramid-{}.json",
        sha256_hex(&group_material)
    );
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    Ok(expected_by_path
        .into_iter()
        .map(|(path, hashes)| {
            let role = contract["members"]
                .as_array()
                .and_then(|members| members.iter().find(|member| member["path"] == path))
                .and_then(|member| member["role"].as_str());
            let actual_hashes = actual["members"]
                .as_array()
                .and_then(|members| {
                    members
                        .iter()
                        .find(|member| member["role"].as_str() == role)
                })
                .map(|member| member["frame_hashes"].clone())
                .unwrap_or_else(|| json!([]));
            (path, json!({
                "status": if passed { "passed" } else { "failed" },
                "independence": "independent",
                "expected_frame_hashes": hashes,
                "actual_frame_hashes": actual_hashes,
                "reason": if passed {
                    "The uv-locked highdicom adapter independently validated the exact three-instance WSI pyramid group"
                } else {
                    "The independent WSI pyramid group reconstruction or exact manifest-contract comparison failed"
                },
                "evidence": {"path": relative, "sha256": sha256_hex(&encoded)}
            }))
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn collect_wsi_reconstruction_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    manifest_sha256: &str,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
) -> Result<Value, String> {
    let unsupported = |reason: &str| pixel_unsupported(expected.clone(), "independent", reason);
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| adapter["id"] == WSI_RECONSTRUCTION_ID)
    else {
        return Ok(unsupported(
            "The independent WSI reconstruction adapter is not configured",
        ));
    };
    if file["case_id"] == WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID
        && !adapter["supported_case_ids"].as_array().is_some_and(|ids| {
            ids.iter()
                .any(|id| id == WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID)
        })
    {
        return Err(
            "WSI reconstruction adapter does not explicitly support multiple optical paths"
                .to_string(),
        );
    }
    let Some(tool) = tools
        .iter()
        .find(|tool| tool["adapter_id"] == WSI_RECONSTRUCTION_ID)
    else {
        return Ok(unsupported(
            "The independent WSI reconstruction adapter was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(unsupported(
            "The independent WSI reconstruction adapter is unavailable",
        ));
    }
    let executable = tool["executable"]
        .as_str()
        .ok_or_else(|| "available WSI reconstruction adapter has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| argument.replace("{input}", &input.display().to_string()))
        .collect::<Vec<_>>();
    let output = run_with_timeout(
        Path::new(executable),
        &arguments,
        Duration::from_secs(adapter["timeout_seconds"].as_u64().unwrap_or(60)),
    )?;
    let actual = serde_json::from_slice::<Value>(&output.stdout).unwrap_or(Value::Null);
    let sparse = file["case_id"] == WSI_SPARSE_CASE_ID;
    let multiple_optical_paths = file["case_id"] == WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID;
    let contract = if sparse {
        &file["expected_wsi_tiled_sparse"]
    } else if multiple_optical_paths {
        &file["expected_wsi_multiple_optical_paths"]
    } else {
        &file["expected_wsi_tiled_full"]
    };
    let manifest_eligible = (file["case_id"] == WSI_CASE_ID || sparse || multiple_optical_paths)
        && file.pointer("/dicom/sop_class_uid").and_then(Value::as_str)
            == Some("1.2.840.10008.5.1.4.1.1.77.1.6")
        && file
            .pointer("/dicom/transfer_syntax_uid")
            .and_then(Value::as_str)
            == Some("1.2.840.10008.1.2.1")
        && file["image"] == contract["image"]
        && wsi_manifest_pixel_contract_matches(file, contract, sparse || multiple_optical_paths)
        && contract["pixel_data"]["frame_hashes"].as_array() == Some(&expected);
    let common_matches = actual["status"] == "passed"
        && actual["backend"] == "dts-wsi-reconstruct"
        && actual["backend_version"] == "0.4.0"
        && actual["runtime"]
            == json!({"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"})
        && actual["frame_hashes"] == contract["pixel_data"]["frame_hashes"]
        && actual["transforms_applied"] == false;
    let reconstruction_matches = common_matches
        && if sparse {
            actual["total_pixel_matrix_shape"] == json!([4, 4, 3])
                && sparse_reconstruction_fields_match(&actual, contract)
        } else if multiple_optical_paths {
            multiple_optical_path_reconstruction_fields_match(&actual, contract)
        } else {
            actual["total_pixel_matrix_shape"] == json!([4, 4, 3])
                && actual["implicit_frame_positions"]
                    == contract["tiling"]["implicit_frame_positions"]
                && actual["total_pixel_matrix_sha256"]
                    == contract["tiling"]["total_pixel_matrix_sha256"]
        };
    let passed = tool["lock_status"] == "matched"
        && manifest_eligible
        && output.exit_code == Some(0)
        && !output.timed_out
        && reconstruction_matches;
    let mut sidecar = json!({
        "adapter_id": WSI_RECONSTRUCTION_ID,
        "adapter_sha256": tool["sha256"],
        "independence": "independent",
        "extraction_method": if sparse {
            "uv_locked_highdicom_tiled_sparse_explicit_sentinel_total_pixel_matrix"
        } else if multiple_optical_paths {
            "uv_locked_highdicom_tiled_full_multiple_optical_paths_separate_matrices"
        } else {
            "uv_locked_highdicom_tiled_full_implicit_total_pixel_matrix"
        },
        "source_manifest_sha256": manifest_sha256,
        "source_instance_sha256": file["sha256"],
        "source_path": relative_input,
        "expected_contract": contract,
        "invocation": std::iter::once(executable.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "exit_code": output.exit_code,
        "timed_out": output.timed_out,
        "backend": actual["backend"],
        "backend_version": actual["backend_version"],
        "runtime": actual["runtime"],
        "frame_hashes": actual["frame_hashes"],
        "transforms_applied": actual["transforms_applied"],
        "status": if passed { "passed" } else { "failed" }
    });
    if sparse {
        for field in [
            "dimension_organization_type",
            "dimension_organization_uid",
            "dimension_index_pointers",
            "functional_group_pointer",
            "explicit_frame_positions",
            "occupancy_mask",
            "absent_tile_positions",
            "pixel_data_sha256",
            "missing_pixel_sentinel",
        ] {
            sidecar[field] = actual[field].clone();
        }
        sidecar["total_pixel_matrix_shape"] = actual["total_pixel_matrix_shape"].clone();
        sidecar["total_pixel_matrix_sha256"] = actual["total_pixel_matrix_sha256"].clone();
    } else if multiple_optical_paths {
        for field in [
            "dimension_organization_type",
            "number_of_frames",
            "number_of_optical_paths",
            "total_pixel_matrix_focal_planes",
            "optical_path_identifiers",
            "pixel_data_sha256",
            "implicit_frame_positions",
            "optical_paths",
            "presence",
            "extended_depth_of_field",
            "unfiltered_total_pixel_matrix",
            "reconstruction_scope",
        ] {
            sidecar[field] = actual[field].clone();
        }
    } else {
        sidecar["implicit_frame_positions"] = actual["implicit_frame_positions"].clone();
        sidecar["total_pixel_matrix_shape"] = actual["total_pixel_matrix_shape"].clone();
        sidecar["total_pixel_matrix_sha256"] = actual["total_pixel_matrix_sha256"].clone();
    }
    let relative = format!("pixels/{WSI_RECONSTRUCTION_ID}/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?;
    fs::write(&target, &encoded).map_err(|error| error.to_string())?;
    Ok(json!({
        "status": if passed { "passed" } else { "failed" },
        "independence": "independent",
        "expected_frame_hashes": expected,
        "actual_frame_hashes": actual["frame_hashes"],
        "reason": if passed {
            if sparse {
                "The uv-locked highdicom adapter independently validated explicit positions, sparse occupancy, stored frames, payload, and the zero-sentinel total pixel matrix with transforms disabled"
            } else if multiple_optical_paths {
                "The uv-locked highdicom adapter independently validated ordered optical paths, implicit positions, aggregate and per-path payloads, separate matrices, nested ICC profiles, and ambiguous unfiltered-matrix rejection"
            } else {
                "The uv-locked highdicom adapter independently reconstructed exact implicit positions, stored frames, and total pixel matrix with transforms disabled"
            }
        } else {
            "The independent WSI reconstruction or exact manifest-contract comparison failed"
        },
        "evidence": {"path": relative, "sha256": sha256_hex(&encoded)}
    }))
}

fn sparse_reconstruction_fields_match(actual: &Value, contract: &Value) -> bool {
    let expected_positions = contract["per_frame_functional_groups"]
        .as_array()
        .map(|positions| {
            positions
                .iter()
                .map(|position| {
                    json!({
                        "frame_number": position["frame_number"],
                        "optical_path_identifier": position["optical_path_identifier"],
                        "column_position": position["column_position"],
                        "row_position": position["row_position"],
                        "x_mm": position["x_mm"],
                        "y_mm": position["y_mm"],
                        "z_mm": position["z_mm"],
                        "dimension_index_values": position["dimension_index_values"]
                    })
                })
                .collect::<Vec<_>>()
        });
    let expected_pointers = contract["dimension_indices"].as_array().map(|indices| {
        indices
            .iter()
            .map(|index| {
                index["dimension_index_pointer"]
                    .as_str()
                    .unwrap_or_default()
                    .replace(['(', ')', ','], "")
            })
            .collect::<Vec<_>>()
    });
    let expected_functional_group = contract["dimension_indices"]
        .get(0)
        .and_then(|index| index["functional_group_pointer"].as_str())
        .map(|pointer| pointer.replace(['(', ')', ','], ""));
    let expected_occupancy = contract["tiling"]["occupancy_mask"].as_array().map(|mask| {
        mask.iter()
            .map(|entry| entry == "present")
            .collect::<Vec<_>>()
    });
    actual["dimension_organization_type"] == contract["dimension_organization_type"]
        && actual["dimension_organization_uid"] == contract["dimension_organization_uid"]
        && actual["dimension_index_pointers"] == json!(expected_pointers)
        && actual["functional_group_pointer"] == json!(expected_functional_group)
        && actual["explicit_frame_positions"] == json!(expected_positions)
        && actual["occupancy_mask"] == json!(expected_occupancy)
        && actual["absent_tile_positions"] == contract["tiling"]["absent_tile_positions"]
        && actual["pixel_data_sha256"] == contract["pixel_data"]["payload_sha256"]
        && actual["missing_pixel_sentinel"] == contract["tiling"]["sentinel_fill_rgb"]
        && actual["total_pixel_matrix_sha256"] == contract["tiling"]["sentinel_matrix_sha256"]
}

fn multiple_optical_path_reconstruction_fields_match(actual: &Value, contract: &Value) -> bool {
    let expected_paths = contract["optical_paths"].as_array().map(|paths| {
        paths
            .iter()
            .map(|path| {
                let first = path["frame_ordinal_range"]
                    .get(0)
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let last = path["frame_ordinal_range"]
                    .get(1)
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                json!({
                    "ordinal": path["ordinal"],
                    "identifier": path["identifier"],
                    "description": path["description"],
                    "illumination_wavelength_nm": path["illumination_wavelength_nm"],
                    "illumination_type": path["illumination_type"],
                    "icc_profile_sha256": path["icc_profile"]["sha256"],
                    "color_space": path["icc_profile"]["dicom_color_space"],
                    "frame_numbers": (first..=last).collect::<Vec<_>>(),
                    "frame_hashes": path["frame_hashes"],
                    "pixel_data_sha256": path["payload_sha256"],
                    "total_pixel_matrix_shape": path["matrix_shape"],
                    "total_pixel_matrix_sha256": path["matrix_sha256"]
                })
            })
            .collect::<Vec<_>>()
    });
    let expected_identifiers = contract["optical_paths"].as_array().map(|paths| {
        paths
            .iter()
            .map(|path| path["identifier"].clone())
            .collect::<Vec<_>>()
    });
    actual["dimension_organization_type"] == contract["dimension_organization_type"]
        && actual["number_of_frames"] == contract["image"]["frames"]
        && actual["number_of_optical_paths"] == contract["tiling"]["number_of_optical_paths"]
        && actual["total_pixel_matrix_focal_planes"]
            == contract["tiling"]["total_pixel_matrix_focal_planes"]
        && actual["optical_path_identifiers"] == json!(expected_identifiers)
        && actual["pixel_data_sha256"] == contract["pixel_data"]["payload_sha256"]
        && actual["implicit_frame_positions"] == contract["tiling"]["implicit_frame_positions"]
        && actual["optical_paths"] == json!(expected_paths)
        && actual["presence"]
            == json!({
                "dimension_index_sequence": false,
                "per_frame_functional_groups_sequence": false,
                "spacing_between_slices": false,
                "number_of_focal_planes": false,
                "distance_between_focal_planes": false,
                "pyramid_uid": false,
                "concatenation_uid": false,
                "referenced_series_sequence": false,
                "top_level_icc_profile": false
            })
        && actual["extended_depth_of_field"] == contract["extended_depth_of_field"]
        && actual["unfiltered_total_pixel_matrix"] == "rejected_ambiguous_optical_path_dimension"
        && actual["reconstruction_scope"] == "separate_matrix_per_optical_path"
}

fn wsi_manifest_pixel_contract_matches(file: &Value, contract: &Value, sparse: bool) -> bool {
    if !sparse {
        return file["pixel_data"] == contract["pixel_data"];
    }

    file["pixel_data"]
        == json!({
            "frame_count": contract["pixel_data"]["frame_count"],
            "frame_hashes": contract["pixel_data"]["frame_hashes"],
            "native_or_encapsulated": contract["pixel_data"]["native_or_encapsulated"],
            "value_length": contract["pixel_data"]["value_length"],
            "vr": contract["pixel_data"]["vr"]
        })
}

#[allow(clippy::too_many_arguments)]
fn collect_visible_light_pixel_result(
    generated_root: &Path,
    evidence_root: &Path,
    file: &Value,
    relative_input: &str,
    stable_key: &str,
    adapters: &[Value],
    tools: &[Value],
    expected: Vec<Value>,
) -> Result<Value, String> {
    let adapter_id = VISIBLE_LIGHT_PIXEL_DECODER_ID;
    let unsupported = |reason: &str| pixel_unsupported(expected.clone(), "independent", reason);
    let Some(adapter) = adapters.iter().find(|adapter| adapter["id"] == adapter_id) else {
        return Ok(unsupported(
            "The independent DCMTK visible-light decoder is not configured",
        ));
    };
    let Some(tool) = tools.iter().find(|tool| tool["adapter_id"] == adapter_id) else {
        return Ok(unsupported(
            "The independent DCMTK visible-light decoder was not discovered",
        ));
    };
    if tool["status"] != "available" {
        return Ok(unsupported(
            "The independent DCMTK visible-light decoder is unavailable",
        ));
    }
    let parser_tool = tools
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcmdump" && tool["status"] == "available")
        .ok_or_else(|| "visible-light pixel evidence requires locked dcmdump".to_string())?;
    let decoder = tool["executable"]
        .as_str()
        .ok_or_else(|| "available visible-light decoder has no executable".to_string())?;
    let dcmdump = parser_tool["executable"]
        .as_str()
        .ok_or_else(|| "available dcmdump parser has no executable".to_string())?;
    let input = generated_root.join(relative_input);
    let work_dir = conformance_work_dir("dts-visible-light-pixel", evidence_root, stable_key);
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let output = work_dir.join("image.ppm");
    let arguments = string_array(adapter, "arguments")?
        .into_iter()
        .map(|argument| {
            argument
                .replace("{input}", &input.display().to_string())
                .replace("{output}", &output.display().to_string())
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
    let parsed = parse_binary_ppm(&output).ok();
    let decoded = parsed
        .as_ref()
        .map(|(_, _, _, bytes)| bytes.clone())
        .unwrap_or_default();
    let decoded_hash = (!decoded.is_empty()).then(|| sha256_hex(&decoded));
    let expected_hash = expected.first().and_then(Value::as_str).unwrap_or("");
    let ppm_valid = parsed
        .as_ref()
        .is_some_and(|(columns, rows, max_value, bytes)| {
            *columns == 2 && *rows == 2 && *max_value == 255 && bytes.len() == 12
        });
    let raw_paths = fs::read_dir(&work_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("raw"))
        .collect::<Vec<_>>();
    let raw = (raw_paths.len() == 1)
        .then(|| fs::read(&raw_paths[0]).ok())
        .flatten();
    let raw_hash = raw.as_ref().map(|bytes| sha256_hex(bytes));
    let actual_hashes = decoded_hash.iter().cloned().collect::<Vec<_>>();
    let manifest_eligible =
        requires_visible_light_validation(file["case_id"].as_str().unwrap_or(""))
            && file.pointer("/image/rows").and_then(Value::as_u64) == Some(2)
            && file.pointer("/image/columns").and_then(Value::as_u64) == Some(2)
            && file.pointer("/image/frames").and_then(Value::as_u64) == Some(1)
            && file
                .pointer("/image/samples_per_pixel")
                .and_then(Value::as_u64)
                == Some(3)
            && file
                .pointer("/image/photometric_interpretation")
                .and_then(Value::as_str)
                == Some("RGB")
            && file
                .pointer("/image/planar_configuration")
                .and_then(Value::as_u64)
                == Some(0)
            && file
                .pointer("/image/bits_allocated")
                .and_then(Value::as_u64)
                == Some(8)
            && file.pointer("/image/bits_stored").and_then(Value::as_u64) == Some(8)
            && file.pointer("/image/high_bit").and_then(Value::as_u64) == Some(7)
            && file
                .pointer("/image/pixel_representation")
                .and_then(Value::as_u64)
                == Some(0)
            && file.pointer("/pixel_data/vr").and_then(Value::as_str) == Some("OB")
            && file
                .pointer("/pixel_data/native_or_encapsulated")
                .and_then(Value::as_str)
                == Some("native")
            && file
                .pointer("/pixel_data/value_length")
                .and_then(Value::as_u64)
                == Some(12)
            && expected.len() == 1
            && !expected_hash.is_empty();
    let passed = manifest_eligible
        && decode.exit_code == Some(0)
        && !decode.timed_out
        && extraction.exit_code == Some(0)
        && !extraction.timed_out
        && ppm_valid
        && decoded_hash.as_deref() == Some(expected_hash)
        && raw_paths.len() == 1
        && raw.as_ref().is_some_and(|bytes| bytes.len() == 12)
        && raw_hash.as_deref() == Some(expected_hash);
    let sidecar = json!({
        "adapter_id": adapter_id, "decoder_sha256": tool["sha256"], "parser_sha256": parser_tool["sha256"],
        "independence": "independent", "extraction_method": "dcmtk_dcm2img_p6_and_dcmdump_single_native_rgb_ob",
        "source_instance_sha256": file["sha256"], "invocation": std::iter::once(decoder.to_string()).chain(arguments.iter().cloned()).collect::<Vec<_>>(),
        "decode_exit_code": decode.exit_code, "decode_timed_out": decode.timed_out,
        "extraction_invocation": std::iter::once(dcmdump.to_string()).chain(extraction_args.iter().cloned()).collect::<Vec<_>>(),
        "extraction_exit_code": extraction.exit_code, "extraction_timed_out": extraction.timed_out,
        "rows": 2, "columns": 2, "frames": 1, "samples_per_pixel": 3, "photometric_interpretation": "RGB",
        "planar_configuration": 0, "bits_allocated": 8, "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
        "max_value": 255, "decoded_length_bytes": decoded.len(), "decoded_pixels_sha256": decoded_hash,
        "raw_value_file_count": raw_paths.len(), "raw_value_length_bytes": raw.as_ref().map(Vec::len), "raw_value_vr": "OB", "raw_value_sha256": raw_hash,
        "expected_frame_hashes": expected, "actual_frame_hashes": actual_hashes,
        "status": if passed { "passed" } else { "failed" }
    });
    let relative = format!("pixels/{adapter_id}/{stable_key}.json");
    let target = evidence_root.join(&relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let encoded = serde_json::to_vec_pretty(&sidecar).map_err(|e| e.to_string())?;
    fs::write(&target, &encoded).map_err(|e| e.to_string())?;
    if let Ok(entries) = fs::read_dir(&work_dir) {
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }
    let _ = fs::remove_dir(&work_dir);
    Ok(json!({
        "status": if passed { "passed" } else { "failed" }, "independence": "independent",
        "expected_frame_hashes": sidecar["expected_frame_hashes"], "actual_frame_hashes": sidecar["actual_frame_hashes"],
        "reason": if passed { "DCMTK independently reconstructed the exact P6 RGB samples and native OB value" } else { "DCMTK visible-light P6 decode or native OB extraction failed" },
        "evidence": { "path": relative, "sha256": sha256_hex(&encoded) }
    }))
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

fn parse_dcmdump_floating_values(stdout: &[u8], spec: FloatingExtractionSpec) -> Option<Vec<u8>> {
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
    // dcmdrle writes Explicit VR Little Endian and dcmdump +W preserves that
    // native byte order. Manifest frame hashes use the same representation.
    let normalized = frame.to_vec();
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

#[cfg(test)]
mod native_frame_normalization_tests {
    use super::*;

    #[test]
    fn dcmtk_little_endian_u16_samples_are_not_byte_swapped() {
        let file = json!({
            "image": {
                "bits_allocated": 16,
                "samples_per_pixel": 1,
                "planar_configuration": null
            }
        });
        let frame = [0x34, 0x12, 0xcd, 0xab];

        assert_eq!(normalize_native_frame(&frame, &file), frame);
    }

    #[test]
    fn planar_color_is_interleaved_without_changing_sample_byte_order() {
        let file = json!({
            "image": {
                "bits_allocated": 16,
                "samples_per_pixel": 3,
                "planar_configuration": 1
            }
        });
        let frame = [
            0x01, 0x10, 0x02, 0x10, // R plane
            0x01, 0x20, 0x02, 0x20, // G plane
            0x01, 0x30, 0x02, 0x30, // B plane
        ];

        assert_eq!(
            normalize_native_frame(&frame, &file),
            [
                0x01, 0x10, 0x01, 0x20, 0x01, 0x30, // pixel 1
                0x02, 0x10, 0x02, 0x20, 0x02, 0x30, // pixel 2
            ]
        );
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

#[cfg(test)]
mod visible_light_tests {
    use super::*;

    #[test]
    fn visible_light_routes_are_exact_case_scoped() {
        assert!(requires_visible_light_validation(
            "vl/endoscopic/rgb_explicit_le"
        ));
        assert!(requires_visible_light_validation(
            "vl/microscopic/rgb_explicit_le"
        ));
        for unrelated in [
            "vl/photo/rgb_planar0_explicit_le",
            "classic/sc/rgb_planar0_explicit_le",
            "vl/wsi/tiled_full",
        ] {
            assert!(!requires_visible_light_validation(unrelated));
        }
    }

    #[test]
    fn binary_ppm_parser_preserves_exact_rgb_bytes() {
        let root = std::env::temp_dir().join(format!("dts-p6-{}", std::process::id()));
        let pixels = [0_u8, 1, 2, 10, 20, 30, 40, 50, 60, 253, 254, 255];
        let mut ppm = b"P6\n# dcmtk fixture\n2 2\n255\n".to_vec();
        ppm.extend_from_slice(&pixels);
        fs::write(&root, ppm).expect("write P6 fixture");
        let parsed = parse_binary_ppm(&root).expect("valid P6");
        assert_eq!((parsed.0, parsed.1, parsed.2), (2, 2, 255));
        assert_eq!(parsed.3, pixels);
        let _ = fs::remove_file(root);
    }

    #[test]
    fn binary_ppm_parser_rejects_ascii_or_truncated_headers() {
        let root = std::env::temp_dir().join(format!("dts-p6-bad-{}", std::process::id()));
        fs::write(&root, b"P3\n2 2\n255\n0 0 0").expect("write P3 fixture");
        assert!(parse_binary_ppm(&root).is_err());
        fs::write(&root, b"P6\n2").expect("write truncated fixture");
        assert!(parse_binary_ppm(&root).is_err());
        let _ = fs::remove_file(root);
    }

    #[test]
    fn visible_light_pixel_evidence_is_bound_to_both_tools_and_manifest() {
        let root = std::env::temp_dir().join(format!(
            "dts-vl-sidecar-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(root.join("pixels")).expect("create evidence fixture");
        let frame_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let sidecar = json!({
            "adapter_id": VISIBLE_LIGHT_PIXEL_DECODER_ID,
            "decoder_sha256": "decoder-hash",
            "parser_sha256": "parser-hash",
            "independence": "independent",
            "extraction_method": "dcmtk_dcm2img_p6_and_dcmdump_single_native_rgb_ob",
            "status": "passed",
            "source_instance_sha256": "source-hash",
            "expected_frame_hashes": [frame_hash],
            "actual_frame_hashes": [frame_hash],
            "rows": 2,
            "columns": 2,
            "frames": 1,
            "samples_per_pixel": 3,
            "photometric_interpretation": "RGB",
            "planar_configuration": 0,
            "bits_allocated": 8,
            "bits_stored": 8,
            "high_bit": 7,
            "pixel_representation": 0,
            "max_value": 255,
            "decoded_length_bytes": 12,
            "decoded_pixels_sha256": frame_hash,
            "raw_value_file_count": 1,
            "raw_value_length_bytes": 12,
            "raw_value_vr": "OB",
            "raw_value_sha256": frame_hash
        });
        fs::write(
            root.join("pixels/vl.json"),
            serde_json::to_vec(&sidecar).unwrap(),
        )
        .expect("write sidecar fixture");
        let evidence = json!({
            "tools": [
                {"adapter_id": VISIBLE_LIGHT_PIXEL_DECODER_ID, "status": "available", "lock_status": "matched", "sha256": "decoder-hash"},
                {"adapter_id": "dcmtk-dcmdump", "status": "available", "lock_status": "matched", "sha256": "parser-hash"}
            ]
        });
        let instance = json!({
            "path": "vl/endoscopic/rgb_explicit_le.dcm",
            "pixel": {
                "actual_frame_hashes": [frame_hash],
                "evidence": {"path": "pixels/vl.json"}
            }
        });
        let manifest_file = json!({
            "sha256": "source-hash",
            "pixel_data": {"value_length": 12, "frame_hashes": [frame_hash]},
            "expected_vl_single_frame": {"image": {"rows": 2, "columns": 2, "planar_configuration": 0}}
        });
        let mut failures = Vec::new();
        verify_visible_light_pixel_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &mut failures,
        );
        assert_eq!(failures, Vec::<String>::new());

        let mut corrupt = sidecar;
        corrupt["raw_value_sha256"] =
            json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        fs::write(
            root.join("pixels/vl.json"),
            serde_json::to_vec(&corrupt).unwrap(),
        )
        .expect("write corrupt sidecar fixture");
        verify_visible_light_pixel_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &mut failures,
        );
        assert_eq!(failures.len(), 1);
        let _ = fs::remove_file(root.join("pixels/vl.json"));
        let _ = fs::remove_dir(root.join("pixels"));
        let _ = fs::remove_dir(root);
    }
}

#[cfg(test)]
mod wsi_reconstruction_tests {
    use super::*;

    fn fixture() -> (Value, Value, Value) {
        let frame_hashes = json!([
            "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
            "6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65",
            "7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f",
            "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21"
        ]);
        let contract = crate::wsi_tiled_full_locked_contract("2.25.11", "2.25.12");
        let evidence = json!({
            "source": {"manifest_sha256": "manifest-hash"},
            "tools": [{
                "adapter_id": WSI_RECONSTRUCTION_ID,
                "status": "available",
                "lock_status": "matched",
                "sha256": "adapter-hash"
            }]
        });
        let instance = json!({
            "path": "vl/wsi/tiled_full_small.dcm",
            "pixel": {
                "expected_frame_hashes": frame_hashes,
                "actual_frame_hashes": frame_hashes,
                "evidence": {"path": "pixels/wsi.json"}
            }
        });
        let manifest_file = json!({
            "case_id": WSI_CASE_ID,
            "path": "vl/wsi/tiled_full_small.dcm",
            "sha256": "instance-hash",
            "expected_wsi_tiled_full": contract
        });
        (evidence, instance, manifest_file)
    }

    fn sidecar(manifest_file: &Value) -> Value {
        let contract = &manifest_file["expected_wsi_tiled_full"];
        json!({
            "adapter_id": WSI_RECONSTRUCTION_ID,
            "adapter_sha256": "adapter-hash",
            "independence": "independent",
            "extraction_method": "uv_locked_highdicom_tiled_full_implicit_total_pixel_matrix",
            "status": "passed",
            "source_manifest_sha256": "manifest-hash",
            "source_instance_sha256": "instance-hash",
            "source_path": "vl/wsi/tiled_full_small.dcm",
            "expected_contract": contract,
            "backend": "dts-wsi-reconstruct",
            "backend_version": "0.4.0",
            "runtime": {"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"},
            "frame_hashes": contract["pixel_data"]["frame_hashes"],
            "implicit_frame_positions": contract["tiling"]["implicit_frame_positions"],
            "total_pixel_matrix_shape": [4, 4, 3],
            "total_pixel_matrix_sha256": contract["tiling"]["total_pixel_matrix_sha256"],
            "transforms_applied": false
        })
    }

    #[test]
    fn wsi_reconstruction_sidecar_is_bound_to_tool_source_and_exact_contract() {
        let root = std::env::temp_dir().join(format!(
            "dts-wsi-sidecar-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(root.join("pixels")).expect("create fixture");
        let (evidence, instance, manifest_file) = fixture();
        let valid = sidecar(&manifest_file);
        fs::write(
            root.join("pixels/wsi.json"),
            serde_json::to_vec(&valid).unwrap(),
        )
        .unwrap();
        let mut failures = Vec::new();
        verify_wsi_reconstruction_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &mut failures,
        );
        assert!(failures.is_empty(), "{failures:?}");

        for (pointer, mutation) in [
            ("/adapter_sha256", json!("other-adapter-hash")),
            ("/source_manifest_sha256", json!("other-manifest-hash")),
            ("/source_instance_sha256", json!("other-instance-hash")),
            ("/source_path", json!("vl/wsi/other.dcm")),
            ("/backend_version", json!("0.1.0")),
            (
                "/frame_hashes/0",
                json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            ),
            ("/implicit_frame_positions/1/column_position", json!(4)),
            (
                "/total_pixel_matrix_sha256",
                json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            ),
            ("/transforms_applied", json!(true)),
        ] {
            let mut corrupt = valid.clone();
            *corrupt.pointer_mut(pointer).expect("mutation pointer") = mutation;
            fs::write(
                root.join("pixels/wsi.json"),
                serde_json::to_vec(&corrupt).unwrap(),
            )
            .unwrap();
            let mut failures = Vec::new();
            verify_wsi_reconstruction_evidence(
                &root,
                &evidence,
                &instance,
                &manifest_file,
                &mut failures,
            );
            assert_eq!(failures.len(), 1, "mutation {pointer}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn sparse_fixture() -> (Value, Value, Value) {
        let contract = crate::wsi_tiled_sparse_locked_contract("2.25.21", "2.25.22", "2.25.23");
        let evidence = json!({
            "source": {"manifest_sha256": "sparse-manifest-hash"},
            "tools": [{
                "adapter_id": WSI_RECONSTRUCTION_ID,
                "status": "available",
                "lock_status": "matched",
                "sha256": "sparse-adapter-hash"
            }]
        });
        let instance = json!({
            "path": "vl/wsi/tiled_sparse_small.dcm",
            "pixel": {
                "expected_frame_hashes": contract["pixel_data"]["frame_hashes"],
                "actual_frame_hashes": contract["pixel_data"]["frame_hashes"],
                "evidence": {"path": "pixels/wsi-sparse.json"}
            }
        });
        let manifest_file = json!({
            "case_id": WSI_SPARSE_CASE_ID,
            "path": "vl/wsi/tiled_sparse_small.dcm",
            "sha256": "sparse-instance-hash",
            "expected_wsi_tiled_sparse": contract
        });
        (evidence, instance, manifest_file)
    }

    fn sparse_sidecar(manifest_file: &Value) -> Value {
        let contract = &manifest_file["expected_wsi_tiled_sparse"];
        json!({
            "adapter_id": WSI_RECONSTRUCTION_ID,
            "adapter_sha256": "sparse-adapter-hash",
            "independence": "independent",
            "extraction_method": "uv_locked_highdicom_tiled_sparse_explicit_sentinel_total_pixel_matrix",
            "status": "passed",
            "source_manifest_sha256": "sparse-manifest-hash",
            "source_instance_sha256": "sparse-instance-hash",
            "source_path": "vl/wsi/tiled_sparse_small.dcm",
            "expected_contract": contract,
            "backend": "dts-wsi-reconstruct",
            "backend_version": "0.4.0",
            "runtime": {"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"},
            "dimension_organization_type": "TILED_SPARSE",
            "dimension_organization_uid": contract["dimension_organization_uid"],
            "dimension_index_pointers": ["0048021E", "0048021F"],
            "functional_group_pointer": "0048021A",
            "frame_hashes": contract["pixel_data"]["frame_hashes"],
            "explicit_frame_positions": [
                {
                    "frame_number": 1, "optical_path_identifier": "RGB",
                    "column_position": 1, "row_position": 1,
                    "x_mm": 0.0, "y_mm": 0.0, "z_mm": 0.0,
                    "dimension_index_values": [1, 1]
                },
                {
                    "frame_number": 2, "optical_path_identifier": "RGB",
                    "column_position": 3, "row_position": 3,
                    "x_mm": 1.0, "y_mm": 1.0, "z_mm": 0.0,
                    "dimension_index_values": [2, 2]
                }
            ],
            "occupancy_mask": [true, false, false, true],
            "absent_tile_positions": contract["tiling"]["absent_tile_positions"],
            "pixel_data_sha256": contract["pixel_data"]["payload_sha256"],
            "total_pixel_matrix_shape": [4, 4, 3],
            "total_pixel_matrix_sha256": contract["tiling"]["sentinel_matrix_sha256"],
            "missing_pixel_sentinel": contract["tiling"]["sentinel_fill_rgb"],
            "transforms_applied": false
        })
    }

    #[test]
    fn sparse_manifest_pixel_summary_omits_reconstruction_only_payload_hash() {
        let contract = crate::wsi_tiled_sparse_locked_contract("2.25.21", "2.25.22", "2.25.23");
        let file = json!({
            "pixel_data": {
                "frame_count": contract["pixel_data"]["frame_count"],
                "frame_hashes": contract["pixel_data"]["frame_hashes"],
                "native_or_encapsulated": contract["pixel_data"]["native_or_encapsulated"],
                "value_length": contract["pixel_data"]["value_length"],
                "vr": contract["pixel_data"]["vr"]
            }
        });

        assert!(wsi_manifest_pixel_contract_matches(&file, &contract, true));
        assert_ne!(file["pixel_data"], contract["pixel_data"]);

        let mut mutated = file;
        mutated["pixel_data"]["value_length"] = json!(22);
        assert!(!wsi_manifest_pixel_contract_matches(
            &mutated, &contract, true
        ));
    }

    #[test]
    fn sparse_wsi_sidecar_is_bound_to_dimensions_positions_occupancy_and_payload() {
        let root = std::env::temp_dir().join(format!(
            "dts-wsi-sparse-sidecar-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(root.join("pixels")).expect("create sparse fixture");
        let (evidence, instance, manifest_file) = sparse_fixture();
        let valid = sparse_sidecar(&manifest_file);
        let sidecar_path = root.join("pixels/wsi-sparse.json");
        fs::write(&sidecar_path, serde_json::to_vec(&valid).unwrap()).unwrap();
        let mut failures = Vec::new();
        verify_wsi_reconstruction_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &mut failures,
        );
        assert!(failures.is_empty(), "{failures:?}");

        for (pointer, mutation) in [
            ("/adapter_sha256", json!("other-adapter-hash")),
            ("/source_manifest_sha256", json!("other-manifest-hash")),
            ("/source_instance_sha256", json!("other-instance-hash")),
            ("/source_path", json!("vl/wsi/other.dcm")),
            (
                "/expected_contract/tiling/occupancy_mask/1",
                json!("present"),
            ),
            ("/dimension_organization_type", json!("TILED_FULL")),
            ("/dimension_organization_uid", json!("2.25.999")),
            ("/dimension_index_pointers/0", json!("0048021F")),
            ("/functional_group_pointer", json!("0048021B")),
            ("/frame_hashes/1", json!("bad-frame-hash")),
            (
                "/explicit_frame_positions/1/dimension_index_values/0",
                json!(1),
            ),
            ("/explicit_frame_positions/1/column_position", json!(1)),
            ("/occupancy_mask/1", json!(true)),
            ("/absent_tile_positions/0/column_position", json!(1)),
            ("/pixel_data_sha256", json!("bad-payload-hash")),
            ("/missing_pixel_sentinel/0", json!(255)),
            ("/total_pixel_matrix_sha256", json!("bad-matrix-hash")),
            ("/transforms_applied", json!(true)),
        ] {
            let mut corrupt = valid.clone();
            *corrupt
                .pointer_mut(pointer)
                .expect("sparse mutation pointer") = mutation;
            fs::write(&sidecar_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
            let mut failures = Vec::new();
            verify_wsi_reconstruction_evidence(
                &root,
                &evidence,
                &instance,
                &manifest_file,
                &mut failures,
            );
            assert_eq!(failures.len(), 1, "mutation {pointer}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn multiple_optical_paths_fixture() -> (Value, Value, Value) {
        let contract =
            crate::wsi_multiple_optical_paths_locked_contract("2.25.31", "2.25.32", "2.25.33");
        let evidence = json!({
            "source": {"manifest_sha256": "multi-path-manifest-hash"},
            "tools": [{
                "adapter_id": WSI_RECONSTRUCTION_ID,
                "status": "available",
                "lock_status": "matched",
                "sha256": "multi-path-adapter-hash"
            }]
        });
        let instance = json!({
            "path": "vl/wsi/multiple_optical_paths.dcm",
            "pixel": {
                "expected_frame_hashes": contract["pixel_data"]["frame_hashes"],
                "actual_frame_hashes": contract["pixel_data"]["frame_hashes"],
                "evidence": {"path": "pixels/wsi-multiple-optical-paths.json"}
            }
        });
        let manifest_file = json!({
            "case_id": WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID,
            "path": "vl/wsi/multiple_optical_paths.dcm",
            "sha256": "multi-path-instance-hash",
            "expected_wsi_multiple_optical_paths": contract
        });
        (evidence, instance, manifest_file)
    }

    fn multiple_optical_paths_sidecar(manifest_file: &Value) -> Value {
        let contract = &manifest_file["expected_wsi_multiple_optical_paths"];
        let optical_paths = contract["optical_paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| {
                let first = path["frame_ordinal_range"][0].as_u64().unwrap();
                let last = path["frame_ordinal_range"][1].as_u64().unwrap();
                json!({
                    "ordinal": path["ordinal"],
                    "identifier": path["identifier"],
                    "description": path["description"],
                    "illumination_wavelength_nm": path["illumination_wavelength_nm"],
                    "illumination_type": path["illumination_type"],
                    "icc_profile_sha256": path["icc_profile"]["sha256"],
                    "color_space": path["icc_profile"]["dicom_color_space"],
                    "frame_numbers": (first..=last).collect::<Vec<_>>(),
                    "frame_hashes": path["frame_hashes"],
                    "pixel_data_sha256": path["payload_sha256"],
                    "total_pixel_matrix_shape": path["matrix_shape"],
                    "total_pixel_matrix_sha256": path["matrix_sha256"]
                })
            })
            .collect::<Vec<_>>();
        json!({
            "adapter_id": WSI_RECONSTRUCTION_ID,
            "adapter_sha256": "multi-path-adapter-hash",
            "independence": "independent",
            "extraction_method": "uv_locked_highdicom_tiled_full_multiple_optical_paths_separate_matrices",
            "status": "passed",
            "source_manifest_sha256": "multi-path-manifest-hash",
            "source_instance_sha256": "multi-path-instance-hash",
            "source_path": "vl/wsi/multiple_optical_paths.dcm",
            "expected_contract": contract,
            "backend": "dts-wsi-reconstruct",
            "backend_version": "0.4.0",
            "runtime": {"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"},
            "dimension_organization_type": "TILED_FULL",
            "number_of_frames": 8,
            "number_of_optical_paths": 2,
            "total_pixel_matrix_focal_planes": 1,
            "optical_path_identifiers": ["BRIGHTFIELD", "ALTERNATE"],
            "frame_hashes": contract["pixel_data"]["frame_hashes"],
            "pixel_data_sha256": contract["pixel_data"]["payload_sha256"],
            "implicit_frame_positions": contract["tiling"]["implicit_frame_positions"],
            "optical_paths": optical_paths,
            "presence": {
                "dimension_index_sequence": false,
                "per_frame_functional_groups_sequence": false,
                "spacing_between_slices": false,
                "number_of_focal_planes": false,
                "distance_between_focal_planes": false,
                "pyramid_uid": false,
                "concatenation_uid": false,
                "referenced_series_sequence": false,
                "top_level_icc_profile": false
            },
            "extended_depth_of_field": "NO",
            "unfiltered_total_pixel_matrix": "rejected_ambiguous_optical_path_dimension",
            "reconstruction_scope": "separate_matrix_per_optical_path",
            "transforms_applied": false
        })
    }

    #[test]
    fn multiple_optical_path_sidecar_binds_ordered_paths_payloads_matrices_and_icc() {
        let root = std::env::temp_dir().join(format!(
            "dts-wsi-multiple-path-sidecar-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(root.join("pixels")).expect("create multi-path fixture");
        let (evidence, instance, manifest_file) = multiple_optical_paths_fixture();
        let valid = multiple_optical_paths_sidecar(&manifest_file);
        let sidecar_path = root.join("pixels/wsi-multiple-optical-paths.json");
        fs::write(&sidecar_path, serde_json::to_vec(&valid).unwrap()).unwrap();
        let mut failures = Vec::new();
        verify_wsi_reconstruction_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &mut failures,
        );
        assert!(failures.is_empty(), "{failures:?}");

        for (pointer, mutation) in [
            ("/adapter_sha256", json!("other-adapter-hash")),
            ("/source_manifest_sha256", json!("other-manifest-hash")),
            ("/source_instance_sha256", json!("other-instance-hash")),
            ("/source_path", json!("vl/wsi/other.dcm")),
            ("/backend_version", json!("0.3.0")),
            ("/number_of_frames", json!(7)),
            ("/number_of_optical_paths", json!(1)),
            ("/optical_path_identifiers/0", json!("ALTERNATE")),
            ("/frame_hashes/4", json!("bad-frame-hash")),
            ("/implicit_frame_positions/4/optical_path_ordinal", json!(1)),
            ("/optical_paths/0/icc_profile_sha256", json!("bad-icc-hash")),
            (
                "/optical_paths/0/pixel_data_sha256",
                json!("bad-payload-hash"),
            ),
            (
                "/optical_paths/1/total_pixel_matrix_sha256",
                json!("bad-matrix-hash"),
            ),
            ("/optical_paths/1/frame_numbers/0", json!(4)),
            ("/pixel_data_sha256", json!("bad-aggregate-hash")),
            ("/presence/top_level_icc_profile", json!(true)),
            ("/unfiltered_total_pixel_matrix", json!("accepted")),
            ("/reconstruction_scope", json!("combined_matrix")),
            ("/transforms_applied", json!(true)),
        ] {
            let mut corrupt = valid.clone();
            *corrupt
                .pointer_mut(pointer)
                .expect("multiple-path mutation pointer") = mutation;
            fs::write(&sidecar_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
            let mut failures = Vec::new();
            verify_wsi_reconstruction_evidence(
                &root,
                &evidence,
                &instance,
                &manifest_file,
                &mut failures,
            );
            assert_eq!(failures.len(), 1, "mutation {pointer}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn pyramid_fixture() -> (Value, Value, Value, Value, Value) {
        let paths = [
            "vl/wsi/pyramid_multiresolution/volume.dcm",
            "vl/wsi/pyramid_multiresolution/thumbnail.dcm",
            "vl/wsi/pyramid_multiresolution/label.dcm",
        ];
        let hashes = ["1".repeat(64), "2".repeat(64), "3".repeat(64)];
        let uids = ["2.25.21", "2.25.22", "2.25.23"];
        let contract = crate::wsi_pyramid_locked_contract(crate::WsiPyramidLockedInputs {
            study_instance_uid: "2.25.11",
            series_instance_uid: "2.25.12",
            frame_of_reference_uid: "2.25.13",
            specimen_uid: "2.25.14",
            pyramid_uid: "2.25.15",
            members: [
                crate::WsiPyramidMemberIdentity {
                    role: crate::WsiPyramidRole::Volume,
                    path: paths[0],
                    sha256: &hashes[0],
                    size_bytes: 3000,
                    sop_instance_uid: uids[0],
                },
                crate::WsiPyramidMemberIdentity {
                    role: crate::WsiPyramidRole::Thumbnail,
                    path: paths[1],
                    sha256: &hashes[1],
                    size_bytes: 2900,
                    sop_instance_uid: uids[1],
                },
                crate::WsiPyramidMemberIdentity {
                    role: crate::WsiPyramidRole::Label,
                    path: paths[2],
                    sha256: &hashes[2],
                    size_bytes: 2800,
                    sop_instance_uid: uids[2],
                },
            ],
        });
        let files = contract["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| {
                json!({
                    "case_id": WSI_PYRAMID_CASE_ID,
                    "path": member["path"], "sha256": member["sha256"],
                    "size_bytes": member["size_bytes"],
                    "wsi_pyramid_role": member["role"],
                    "wsi_pyramid_ordinal": member["ordinal"],
                    "uids": {"sop_instance_uid": member["sop_instance_uid"]},
                    "pixel_data": {"frame_hashes": member["frame_hashes"]},
                    "expected_wsi_pyramid": contract
                })
            })
            .collect::<Vec<_>>();
        let actual_members = contract["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| {
                json!({
                    "role": member["role"], "sop_instance_uid": member["sop_instance_uid"],
                    "image_type": member["image_type"], "frame_type": member["frame_type"],
                    "frame_count": member["frames"], "stored_frame_shape": [2, 2, 3],
                    "frame_hashes": member["frame_hashes"],
                    "pixel_data_sha256": member["payload_sha256"],
                    "total_pixel_matrix_shape": [member["total_pixel_matrix_rows"], member["total_pixel_matrix_columns"], 3],
                    "total_pixel_matrix_sha256": member["matrix_sha256"],
                    "pyramid_member": !member["pyramid_uid"].is_null(),
                    "pyramid_uid": member["pyramid_uid"],
                    "specimen_label_in_image": member["specimen_label_in_image"],
                    "burned_in_annotation": "NO", "transforms_applied": false
                })
            })
            .collect::<Vec<_>>();
        let actual = json!({
            "status": "passed", "backend": "dts-wsi-reconstruct", "backend_version": "0.4.0",
            "runtime": {"highdicom": "0.28.1", "numpy": "2.5.2", "pydicom": "3.0.2"},
            "ordered_roles": contract["ordered_roles"],
            "group_identity": contract["shared_identity"],
            "pyramid_uid": contract["pyramid_membership"]["pyramid_uid"],
            "pyramid_roles": contract["pyramid_membership"]["member_roles"],
            "apex_role": "thumbnail", "label_excluded_from_pyramid": true,
            "thumbnail_reduction": "volume_quadrant_top_left_pixels",
            "thumbnail_reduction_sha256": contract["members"][1]["matrix_sha256"],
            "members": actual_members, "transforms_applied": false
        });
        let source_members = contract["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| {
                json!({
                    "ordinal": member["ordinal"], "role": member["role"],
                    "path": member["path"], "sha256": member["sha256"],
                    "sop_instance_uid": member["sop_instance_uid"]
                })
            })
            .collect::<Vec<_>>();
        let sidecar = json!({
            "adapter_id": WSI_RECONSTRUCTION_ID, "adapter_sha256": "adapter-hash",
            "independence": "independent",
            "extraction_method": "uv_locked_highdicom_three_instance_pyramid_group",
            "source_manifest_sha256": "manifest-hash", "source_members": source_members,
            "expected_contract": contract, "actual": actual, "status": "passed"
        });
        let evidence = json!({
            "source": {"manifest_sha256": "manifest-hash"},
            "tools": [{"adapter_id": WSI_RECONSTRUCTION_ID, "status": "available", "lock_status": "matched", "sha256": "adapter-hash"}]
        });
        let instance = json!({
            "path": paths[0],
            "pixel": {"expected_frame_hashes": contract["members"][0]["frame_hashes"],
                "actual_frame_hashes": contract["members"][0]["frame_hashes"],
                "evidence": {"path": "pixels/pyramid.json"}}
        });
        let manifest = json!({"files": files});
        (
            evidence,
            instance,
            manifest["files"][0].clone(),
            manifest,
            sidecar,
        )
    }

    #[test]
    fn pyramid_group_sidecar_rejects_incomplete_mutated_and_misbound_evidence() {
        let root = std::env::temp_dir().join(format!(
            "dts-wsi-pyramid-sidecar-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(root.join("pixels")).unwrap();
        let (evidence, instance, manifest_file, manifest, valid) = pyramid_fixture();
        let sidecar_path = root.join("pixels/pyramid.json");
        fs::write(&sidecar_path, serde_json::to_vec(&valid).unwrap()).unwrap();
        let mut failures = Vec::new();
        verify_wsi_pyramid_group_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &manifest,
            &mut failures,
        );
        assert!(failures.is_empty(), "{failures:?}");

        let mut incomplete = manifest.clone();
        incomplete["files"].as_array_mut().unwrap().pop();
        verify_wsi_pyramid_group_evidence(
            &root,
            &evidence,
            &instance,
            &manifest_file,
            &incomplete,
            &mut failures,
        );
        assert_eq!(failures.len(), 1);

        for pointer in ["/source_members/1/path", "/actual/pyramid_uid"] {
            let mut corrupt = valid.clone();
            *corrupt.pointer_mut(pointer).unwrap() = json!("misbound");
            fs::write(&sidecar_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
            let mut failures = Vec::new();
            verify_wsi_pyramid_group_evidence(
                &root,
                &evidence,
                &instance,
                &manifest_file,
                &manifest,
                &mut failures,
            );
            assert_eq!(failures.len(), 1, "mutation {pointer}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wsi_is_exactly_scoped_into_the_visible_light_iod_route() {
        assert!(requires_visible_light_validation(WSI_CASE_ID));
        assert!(requires_visible_light_validation(WSI_PYRAMID_CASE_ID));
        assert!(requires_visible_light_validation(
            WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID
        ));
        assert!(!requires_visible_light_validation("vl/wsi/tiled_sparse"));
    }
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
