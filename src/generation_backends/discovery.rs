use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::sha256_hex;

use super::{
    BackendContractError, OutputLimits, PROTOCOL_VERSION, executable_fingerprint,
    is_safe_relative_path,
};

#[derive(Debug, Clone)]
pub struct PreparedBackend {
    pub backend_id: String,
    pub executable: PathBuf,
    pub fixed_arguments: Vec<String>,
    pub version: String,
    pub dependency_lock_sha256: String,
    pub executable_fingerprint: String,
    pub entrypoint_fingerprint: String,
    pub environment_fingerprint: String,
    pub runtime_identity: Value,
    pub timeout: Duration,
    pub max_response_bytes: u64,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub output_limits: OutputLimits,
}

#[derive(Debug, Clone)]
pub enum BackendDiscovery {
    Available(PreparedBackend),
    Unavailable { code: String, message: String },
}

pub fn discover_prepared_backend(
    repository_root: &Path,
    policy: &Value,
) -> Result<BackendDiscovery, BackendContractError> {
    let backend_id = required_str(policy, "/backend_id")?;
    let discovery = policy.pointer("/discovery").ok_or_else(|| {
        invalid(format!(
            "backend {backend_id} policy has no discovery field"
        ))
    })?;
    if discovery.is_null() {
        return Ok(BackendDiscovery::Unavailable {
            code: "backend_not_configured".to_string(),
            message: format!("backend {backend_id} has no configured runtime discovery"),
        });
    }

    let override_name = discovery
        .pointer("/environment_override")
        .and_then(Value::as_str);
    let executable = override_name
        .and_then(std::env::var_os)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let platform = std::env::consts::OS;
            repository_root.join(
                discovery["default_relative_executables"][platform]
                    .as_str()
                    .expect("lock schema checked platform executable"),
            )
        });
    let executable = if executable.is_absolute() {
        executable
    } else {
        std::env::current_dir()
            .map_err(|source| BackendContractError::Read {
                path: PathBuf::from("."),
                source,
            })?
            .join(executable)
    };
    if !executable.exists() {
        return Ok(BackendDiscovery::Unavailable {
            code: "dependency_unavailable".to_string(),
            message: format!(
                "prepared backend runtime {} does not exist; provision it with the committed uv lock{}",
                executable.display(),
                override_name
                    .map(|name| format!(" or set {name}"))
                    .unwrap_or_default()
            ),
        });
    }
    let canonical_executable =
        fs::canonicalize(&executable).map_err(|source| BackendContractError::Read {
            path: executable.clone(),
            source,
        })?;
    if !canonical_executable.is_file() {
        return Err(invalid(format!(
            "prepared backend executable {} is not a regular file",
            canonical_executable.display()
        )));
    }

    let limits = policy
        .pointer("/resource_limits")
        .expect("lock schema checked resource limits");
    let probe_timeout = Duration::from_secs(required_u64(limits, "/version_timeout_seconds")?);
    let max_stdout = required_usize(limits, "/max_stdout_bytes")?;
    let max_stderr = required_usize(limits, "/max_stderr_bytes")?;
    let version_arguments = string_array(discovery, "/version_arguments")?;
    let version_probe = run_probe(
        &executable,
        &version_arguments,
        probe_timeout,
        max_stdout,
        max_stderr,
    );
    let version_probe = match version_probe {
        Ok(output) => output,
        Err(error) => {
            return Ok(BackendDiscovery::Unavailable {
                code: "runtime_probe_failed".to_string(),
                message: error,
            });
        }
    };
    let version = String::from_utf8(version_probe.stdout)
        .map_err(|_| invalid("backend version output is not UTF-8"))?
        .trim()
        .to_string();
    if version.is_empty() {
        return Err(invalid("backend version output is empty"));
    }

    let identity_arguments = string_array(discovery, "/runtime_identity_arguments")?;
    let identity_probe = run_probe(
        &executable,
        &identity_arguments,
        probe_timeout,
        max_stdout,
        max_stderr,
    )
    .map_err(invalid)?;
    let runtime_identity: Value =
        serde_json::from_slice(&identity_probe.stdout).map_err(|source| {
            BackendContractError::Parse {
                label: format!("backend {backend_id} runtime identity"),
                source,
            }
        })?;
    verify_runtime_identity(backend_id, &runtime_identity)?;

    let dependency = policy
        .pointer("/dependency_lock")
        .ok_or_else(|| invalid(format!("backend {backend_id} has no dependency lock")))?;
    let dependency_relative = Path::new(required_str(dependency, "/path")?);
    if !is_safe_relative_path(dependency_relative) {
        return Err(invalid(format!(
            "backend {backend_id} dependency lock path is unsafe"
        )));
    }
    let dependency_bytes =
        fs::read(repository_root.join(dependency_relative)).map_err(|source| {
            BackendContractError::Read {
                path: repository_root.join(dependency_relative),
                source,
            }
        })?;
    let dependency_lock_sha256 = sha256_hex(&dependency_bytes);
    let expected_dependency = required_str(dependency, "/sha256")?;
    if dependency_lock_sha256 != expected_dependency {
        return Err(invalid(format!(
            "backend {backend_id} dependency lock fingerprint drifted"
        )));
    }
    verify_locked_distributions(&dependency_bytes, &runtime_identity)?;
    verify_locked_python(&dependency_bytes, &runtime_identity)?;

    let entrypoint_paths = string_array(discovery, "/entrypoint_paths")?;
    let entrypoint_fingerprint = fingerprint_entrypoints(repository_root, &entrypoint_paths)?;
    let executable_fingerprint = executable_fingerprint(&canonical_executable)?;
    let fixed_arguments = string_array(discovery, "/fixed_arguments")?;
    let environment_fingerprint = prepared_environment_fingerprint(
        backend_id,
        &fixed_arguments,
        &dependency_lock_sha256,
        &executable_fingerprint,
        &entrypoint_fingerprint,
        &runtime_identity,
    )?;

    Ok(BackendDiscovery::Available(PreparedBackend {
        backend_id: backend_id.to_string(),
        executable,
        fixed_arguments,
        version,
        dependency_lock_sha256,
        executable_fingerprint,
        entrypoint_fingerprint,
        environment_fingerprint,
        runtime_identity,
        timeout: Duration::from_secs(required_u64(limits, "/invocation_timeout_seconds")?),
        max_response_bytes: required_u64(limits, "/max_response_bytes")?,
        max_stdout_bytes: max_stdout,
        max_stderr_bytes: max_stderr,
        output_limits: OutputLimits {
            max_output_files: required_usize(limits, "/max_output_files")?,
            max_file_bytes: required_u64(limits, "/max_file_bytes")?,
            max_total_output_bytes: required_u64(limits, "/max_total_output_bytes")?,
        },
    }))
}

fn fingerprint_entrypoints(
    repository_root: &Path,
    relative_paths: &[String],
) -> Result<String, BackendContractError> {
    let mut paths = relative_paths.to_vec();
    paths.sort();
    let mut material = Vec::new();
    for relative in paths {
        let relative_path = Path::new(&relative);
        if !is_safe_relative_path(relative_path) {
            return Err(invalid(format!("entrypoint path {relative} is unsafe")));
        }
        let bytes = fs::read(repository_root.join(relative_path)).map_err(|source| {
            BackendContractError::Read {
                path: repository_root.join(relative_path),
                source,
            }
        })?;
        material.extend_from_slice(relative.as_bytes());
        material.push(0);
        material.extend_from_slice(&bytes);
        material.push(0);
    }
    Ok(sha256_hex(&material))
}

fn prepared_environment_fingerprint(
    backend_id: &str,
    fixed_arguments: &[String],
    dependency_lock_sha256: &str,
    executable_fingerprint: &str,
    entrypoint_fingerprint: &str,
    runtime_identity: &Value,
) -> Result<String, BackendContractError> {
    let mut material = b"dicom-test-suite:prepared-backend-environment:v1\0".to_vec();
    for component in [
        backend_id,
        PROTOCOL_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
        dependency_lock_sha256,
        executable_fingerprint,
        entrypoint_fingerprint,
    ] {
        material.extend_from_slice(component.as_bytes());
        material.push(0);
    }
    for argument in fixed_arguments {
        material.extend_from_slice(argument.as_bytes());
        material.push(0);
    }
    let identity =
        serde_json::to_vec(runtime_identity).map_err(|source| BackendContractError::Parse {
            label: "runtime identity serialization".to_string(),
            source,
        })?;
    material.extend_from_slice(&identity);
    Ok(sha256_hex(&material))
}

fn verify_runtime_identity(backend_id: &str, identity: &Value) -> Result<(), BackendContractError> {
    if identity.pointer("/backend_id").and_then(Value::as_str) != Some(backend_id) {
        return Err(invalid("runtime identity backend_id mismatch"));
    }
    if identity
        .pointer("/protocol_version")
        .and_then(Value::as_str)
        != Some(PROTOCOL_VERSION)
    {
        return Err(invalid("runtime identity protocol_version mismatch"));
    }
    if identity
        .pointer("/python/implementation")
        .and_then(Value::as_str)
        != Some("cpython")
    {
        return Err(invalid("runtime identity must report CPython"));
    }
    let distributions = identity
        .pointer("/distributions")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("runtime identity distributions must be an array"))?;
    if distributions.is_empty() {
        return Err(invalid("runtime identity has no installed distributions"));
    }
    for distribution in distributions {
        for field in ["name", "version", "files_sha256"] {
            if distribution.get(field).and_then(Value::as_str).is_none() {
                return Err(invalid(format!(
                    "runtime identity distribution is missing {field}"
                )));
            }
        }
    }
    Ok(())
}

fn verify_locked_distributions(
    lock_bytes: &[u8],
    identity: &Value,
) -> Result<(), BackendContractError> {
    let lock =
        std::str::from_utf8(lock_bytes).map_err(|_| invalid("uv dependency lock is not UTF-8"))?;
    let locked = parse_uv_lock_versions(lock);
    for distribution in identity["distributions"]
        .as_array()
        .expect("identity verified distributions")
    {
        let name = distribution["name"]
            .as_str()
            .expect("identity verified name");
        let version = distribution["version"]
            .as_str()
            .expect("identity verified version");
        if locked.get(name).map(String::as_str) != Some(version) {
            return Err(invalid(format!(
                "installed distribution {name} {version} does not match uv lock"
            )));
        }
    }
    Ok(())
}

fn verify_locked_python(lock_bytes: &[u8], identity: &Value) -> Result<(), BackendContractError> {
    let lock =
        std::str::from_utf8(lock_bytes).map_err(|_| invalid("uv dependency lock is not UTF-8"))?;
    let required = lock
        .lines()
        .find_map(|line| line.strip_prefix("requires-python = \"=="))
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| invalid("uv dependency lock must pin an exact Python version"))?;
    let actual = identity
        .pointer("/python/version")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("runtime identity has no Python version"))?;
    if required != actual {
        return Err(invalid(format!(
            "prepared Python {actual} does not match uv lock requirement {required}"
        )));
    }
    Ok(())
}

fn parse_uv_lock_versions(lock: &str) -> BTreeMap<String, String> {
    let mut packages = BTreeMap::new();
    let mut name: Option<String> = None;
    for line in lock.lines() {
        if line == "[[package]]" {
            name = None;
        } else if let Some(value) = line.strip_prefix("name = \"") {
            name = value.strip_suffix('"').map(ToOwned::to_owned);
        } else if let Some(value) = line.strip_prefix("version = \"") {
            if let (Some(name), Some(version)) = (name.take(), value.strip_suffix('"')) {
                packages.insert(name, version.to_string());
            }
        }
    }
    packages
}

#[derive(Debug)]
struct ProbeOutput {
    stdout: Vec<u8>,
}

fn run_probe(
    executable: &Path,
    arguments: &[String],
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<ProbeOutput, String> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn runtime probe: {error}"))?;
    let stdout = child.stdout.take().expect("probe stdout piped");
    let stderr = child.stderr.take().expect("probe stderr piped");
    let stdout_thread = thread::spawn(move || drain_probe(stdout, stdout_limit));
    let stderr_thread = thread::spawn(move || drain_probe(stderr, stderr_limit));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("runtime probe exceeded {} ms", timeout.as_millis()));
            }
            Err(error) => return Err(format!("wait for runtime probe: {error}")),
        }
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| "runtime probe stdout reader panicked".to_string())??;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "runtime probe stderr reader panicked".to_string())??;
    if !status.success() {
        return Err(format!(
            "runtime probe exited with {status}: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    Ok(ProbeOutput { stdout })
}

fn drain_probe(mut stream: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut retained = Vec::with_capacity(limit.min(8192));
    let mut total = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| format!("read runtime probe output: {error}"))?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count);
        if retained.len() < limit {
            let keep = (limit - retained.len()).min(count);
            retained.extend_from_slice(&buffer[..keep]);
        }
    }
    if total > limit {
        Err(format!(
            "runtime probe output is {total} bytes, limit is {limit}"
        ))
    } else {
        Ok(retained)
    }
}

fn required_str<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, BackendContractError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("required string {pointer} is missing")))
}

fn required_u64(value: &Value, pointer: &str) -> Result<u64, BackendContractError> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("required integer {pointer} is missing")))
}

fn required_usize(value: &Value, pointer: &str) -> Result<usize, BackendContractError> {
    usize::try_from(required_u64(value, pointer)?)
        .map_err(|_| invalid(format!("integer {pointer} does not fit usize")))
}

fn string_array(value: &Value, pointer: &str) -> Result<Vec<String>, BackendContractError> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("required array {pointer} is missing")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid(format!("array {pointer} contains a non-string")))
        })
        .collect()
}

fn invalid(message: impl Into<String>) -> BackendContractError {
    BackendContractError::Invalid {
        label: "generation backend discovery".to_string(),
        problems: vec![message.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_uv_lock_matches_exact_runtime_versions() {
        let bytes = fs::read("generation-backends/highdicom-pydicom/uv.lock")
            .expect("committed uv lock must be readable");
        let lock = std::str::from_utf8(&bytes).expect("uv lock must be UTF-8");
        let packages = parse_uv_lock_versions(lock);
        assert_eq!(
            packages.get("highdicom").map(String::as_str),
            Some("0.28.1")
        );
        assert_eq!(packages.get("pydicom").map(String::as_str), Some("3.0.2"));
        assert!(lock.contains("requires-python = \"==3.12.12\""));
    }

    #[test]
    fn entrypoint_fingerprint_is_path_and_content_bound() {
        let paths = vec![
            "generation-backends/highdicom-pydicom/pyproject.toml".to_string(),
            "generation-backends/highdicom-pydicom/src/dts_highdicom_backend/__init__.py"
                .to_string(),
        ];
        let forward = fingerprint_entrypoints(Path::new("."), &paths).unwrap();
        let mut reversed = paths;
        reversed.reverse();
        assert_eq!(
            forward,
            fingerprint_entrypoints(Path::new("."), &reversed).unwrap()
        );
    }
}
