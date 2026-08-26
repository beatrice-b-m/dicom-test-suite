use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::sha256_hex;

pub const DEFAULT_VALIDATOR_CONFIG: &str = "conformance/validators.json";
pub const DEFAULT_VALIDATOR_LOCK: &str = "conformance/validator-lock.json";
pub const DEFAULT_ACCEPTED_FINDINGS: &str = "conformance/accepted-findings.json";

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
        if tool["required"].as_bool() == Some(true) {
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
    let evidence_paths = evidence["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|instance| instance["path"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if manifest_paths != evidence_paths {
        failures.push("instance evidence is incomplete for the source manifest".to_string());
    }
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
        let parser = instance["results"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|result| result["role"] == "independent_parser");
        if parser.is_none_or(|result| result["status"] != "completed") {
            failures.push(format!("independent parser incomplete: {path}"));
        }
        if instance["transfer_syntax_uid"] == "1.2.840.10008.1.2.5"
            && instance["pixel"]["status"] != "passed"
        {
            failures.push(format!("independent RLE pixel evidence failed: {path}"));
        }
    }
    if evidence["entity"]["status"] != "completed" {
        failures.push("corpus entity validation is incomplete".to_string());
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
    let primary = adapters
        .iter()
        .find(|adapter| {
            adapter.get("role").and_then(Value::as_str) == Some("primary_iod_validator")
        })
        .ok_or_else(|| "configuration requires a primary_iod_validator adapter".to_string())?;
    let tool_report = check_tools_path(config_path)?;
    let tools = evidence_tools(&tool_report);
    let primary_tool = tools
        .iter()
        .find(|tool| tool.get("adapter_id") == primary.get("id"))
        .ok_or_else(|| "primary validator discovery result is missing".to_string())?;

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
            primary,
            primary_tool,
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
    adapter: &Value,
    tool: &Value,
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
    Ok(json!({
        "stable_instance_key": stable_key,
        "case_id": case_id,
        "path": path,
        "sop_class_uid": file.pointer("/dicom/sop_class_uid").and_then(Value::as_str).unwrap_or("0.0"),
        "transfer_syntax_uid": file.pointer("/dicom/transfer_syntax_uid").and_then(Value::as_str).unwrap_or("0.0"),
        "results": results,
        "pixel": pixel
    }))
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
    let work_dir =
        std::env::temp_dir().join(format!("dts-pixel-{}-{stable_key}", std::process::id()));
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
    for file in files {
        let relative = file
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "manifest file entry requires path".to_string())?;
        validate_relative_path(relative)?;
        list.push_str(&generated_root.join(relative).display().to_string());
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
    Ok(execution_result(
        adapter_id,
        "entity_validator",
        executable,
        arguments,
        output,
        "entity/dcentvfy.stdout",
        "entity/dcentvfy.stderr",
        &generated_root.display().to_string(),
        ".",
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
    let timeout = adapter
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("adapter {id} requires timeout_seconds"))?;
    let version_arguments = string_array(adapter, "version_arguments")?;

    let Some(path) = resolve_executable(configured_path.unwrap_or(executable)) else {
        let executable_path = Path::new(configured_path.unwrap_or(executable));
        let status = if configured_path.is_some()
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
    let started = Instant::now();
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to execute {}: {error}", executable.display()))?;

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
