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
            "version_output": null,
            "version_exit_code": null,
            "lock_status": "unavailable"
        }));
    };

    let bytes = fs::read(&path)
        .map_err(|error| format!("failed to fingerprint {}: {error}", path.display()))?;
    let fingerprint = sha256_hex(&bytes);
    let probe = run_with_timeout(&path, &version_arguments, Duration::from_secs(timeout))?;
    let lock_status = locked_tools
        .iter()
        .find(|tool| tool.get("adapter_id").and_then(Value::as_str) == Some(id))
        .map(|tool| {
            if tool.get("executable_sha256").and_then(Value::as_str) == Some(&fingerprint) {
                "matched"
            } else {
                "mismatched"
            }
        })
        .unwrap_or("unlocked");
    let version_output = [probe.stdout.as_str(), probe.stderr.as_str()]
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
        "version_output": if version_output.is_empty() { Value::Null } else { Value::String(version_output) },
        "version_exit_code": probe.exit_code,
        "version_duration_ms": probe.duration_ms,
        "lock_status": lock_status
    }))
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
    pub stdout: String,
    pub stderr: String,
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
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
        timed_out,
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
    })
}
