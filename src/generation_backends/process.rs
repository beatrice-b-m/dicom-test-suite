use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{
    BackendContractError, OutputLimits, validate_request, validate_response_for_request,
    verify_staged_outputs,
};

const REQUEST_FILE: &str = "request.json";
const RESPONSE_FILE: &str = "response.json";
const INPUTS_DIRECTORY: &str = "inputs";
const OUTPUT_DIRECTORY: &str = "outputs";

#[derive(Debug, Clone)]
pub struct BackendInvocation {
    pub executable: PathBuf,
    pub fixed_arguments: Vec<String>,
    pub timeout: Duration,
    pub max_response_bytes: u64,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub output_limits: OutputLimits,
}

#[derive(Debug)]
pub struct BackendRun {
    pub response: Value,
    pub staging_root: PathBuf,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn invoke_backend(
    invocation: &BackendInvocation,
    request: &Value,
    staging_root: &Path,
) -> Result<BackendRun, BackendContractError> {
    validate_request(request)?;
    prepare_private_staging(staging_root)?;
    let inputs = staging_root.join(INPUTS_DIRECTORY);
    let outputs = staging_root.join(OUTPUT_DIRECTORY);
    let request_path = staging_root.join(REQUEST_FILE);
    let response_path = staging_root.join(RESPONSE_FILE);

    let mut staged_request = request.clone();
    staged_request["staging"]["root"] = Value::String(staging_root.display().to_string());
    staged_request["staging"]["inputs_directory"] = Value::String(inputs.display().to_string());
    staged_request["staging"]["output_directory"] = Value::String(outputs.display().to_string());
    validate_request(&staged_request)?;
    write_json_exclusive(&request_path, &staged_request)?;

    let executable =
        fs::canonicalize(&invocation.executable).map_err(|source| BackendContractError::Read {
            path: invocation.executable.clone(),
            source,
        })?;
    if !executable.is_file() {
        return Err(invalid(format!(
            "backend executable {} is not a regular file",
            executable.display()
        )));
    }

    let mut command = Command::new(&executable);
    command
        .args(&invocation.fixed_arguments)
        .current_dir(staging_root)
        .env_clear()
        .env("DTS_BACKEND_REQUEST", &request_path)
        .env("DTS_BACKEND_RESPONSE", &response_path)
        .env("DTS_BACKEND_OUTPUTS", &outputs)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| invalid(format!("spawn backend {}: {error}", executable.display())))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_limit = invocation.max_stdout_bytes;
    let stderr_limit = invocation.max_stderr_bytes;
    let stdout_thread = thread::spawn(move || drain_bounded(stdout, stdout_limit));
    let stderr_thread = thread::spawn(move || drain_bounded(stderr, stderr_limit));

    let deadline = Instant::now() + invocation.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(invalid(format!(
                    "backend invocation exceeded {} ms",
                    invocation.timeout.as_millis()
                )));
            }
            Err(error) => return Err(invalid(format!("wait for backend: {error}"))),
        }
    };

    let stdout = stdout_thread
        .join()
        .map_err(|_| invalid("backend stdout reader panicked".to_string()))??;
    let stderr = stderr_thread
        .join()
        .map_err(|_| invalid("backend stderr reader panicked".to_string()))??;
    if !status.success() {
        return Err(invalid(format!("backend exited with status {status}")));
    }

    let metadata =
        fs::symlink_metadata(&response_path).map_err(|source| BackendContractError::Read {
            path: response_path.clone(),
            source,
        })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid(
            "backend response must be a regular file".to_string(),
        ));
    }
    if metadata.len() > invocation.max_response_bytes {
        return Err(invalid(format!(
            "backend response is {} bytes, limit is {}",
            metadata.len(),
            invocation.max_response_bytes
        )));
    }
    let response_bytes = fs::read(&response_path).map_err(|source| BackendContractError::Read {
        path: response_path.clone(),
        source,
    })?;
    let response =
        serde_json::from_slice(&response_bytes).map_err(|source| BackendContractError::Parse {
            label: response_path.display().to_string(),
            source,
        })?;
    validate_response_for_request(&staged_request, &response)?;
    verify_staged_outputs(&response, &outputs, invocation.output_limits)?;

    Ok(BackendRun {
        response,
        staging_root: staging_root.to_path_buf(),
        stdout,
        stderr,
    })
}

fn prepare_private_staging(root: &Path) -> Result<(), BackendContractError> {
    fs::create_dir(root).map_err(|source| BackendContractError::Read {
        path: root.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|source| {
            BackendContractError::Read {
                path: root.to_path_buf(),
                source,
            }
        })?;
    }
    for directory in [INPUTS_DIRECTORY, OUTPUT_DIRECTORY] {
        let path = root.join(directory);
        fs::create_dir(&path).map_err(|source| BackendContractError::Read {
            path: path.clone(),
            source,
        })?;
    }
    Ok(())
}

fn write_json_exclusive(path: &Path, value: &Value) -> Result<(), BackendContractError> {
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| BackendContractError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| BackendContractError::Parse {
        label: "generation backend request".to_string(),
        source,
    })?;
    file.write_all(&bytes)
        .map_err(|source| BackendContractError::Read {
            path: path.to_path_buf(),
            source,
        })
}

fn drain_bounded(mut stream: impl Read, limit: usize) -> Result<Vec<u8>, BackendContractError> {
    let mut retained = Vec::with_capacity(limit.min(8192));
    let mut total = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| invalid(format!("read backend output: {error}")))?;
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
        Err(invalid(format!(
            "backend output is {total} bytes, limit is {limit}"
        )))
    } else {
        Ok(retained)
    }
}

fn invalid(message: String) -> BackendContractError {
    BackendContractError::Invalid {
        label: "generation backend invocation".to_string(),
        problems: vec![message],
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn fake_backend_round_trips_an_explicit_unavailable_result() {
        let staging = unique_staging("unavailable");
        let result = invoke_backend(
            &fake_invocation(Duration::from_secs(2)),
            &request(),
            &staging,
        )
        .expect("fake backend should return unavailable");
        assert_eq!(result.response["status"], "unavailable");
        assert!(result.stderr.is_empty());
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_identity_mismatch_is_rejected() {
        let staging = unique_staging("mismatch");
        let error = invoke_backend(
            &fake_invocation(Duration::from_secs(2)),
            &request(),
            &staging,
        )
        .expect_err("mismatched response must fail");
        assert!(error.to_string().contains("request_id"));
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_timeout_is_enforced() {
        let staging = unique_staging("timeout");
        let error = invoke_backend(
            &fake_invocation(Duration::from_millis(30)),
            &request(),
            &staging,
        )
        .expect_err("slow backend must time out");
        assert!(error.to_string().contains("exceeded"));
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_undeclared_output_is_rejected() {
        let staging = unique_staging("undeclared");
        let error = invoke_backend(
            &fake_invocation(Duration::from_secs(2)),
            &request(),
            &staging,
        )
        .expect_err("undeclared backend output must fail");
        assert!(error.to_string().contains("undeclared"));
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    #[ignore]
    fn fake_backend_process() {
        let current_directory = std::env::current_dir().expect("fake current directory");
        let behavior = current_directory
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fake staging name");
        if behavior.contains("timeout") {
            thread::sleep(Duration::from_secs(2));
            return;
        }
        let request_path = std::env::var_os("DTS_BACKEND_REQUEST").expect("request path");
        let response_path = std::env::var_os("DTS_BACKEND_RESPONSE").expect("response path");
        let request: Value = serde_json::from_slice(&fs::read(request_path).expect("read request"))
            .expect("parse request");
        if behavior.contains("undeclared") {
            let output_directory = std::env::var_os("DTS_BACKEND_OUTPUTS").expect("outputs path");
            fs::write(
                Path::new(&output_directory).join("rogue.txt"),
                b"undeclared",
            )
            .expect("write undeclared output");
        }
        let request_id = if behavior.contains("mismatch") {
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        } else {
            request["request_id"].as_str().expect("request id")
        };
        let response = json!({
            "response_schema_version": "0.1.0",
            "protocol_version": request["protocol_version"],
            "request_id": request_id,
            "backend_id": request["backend_id"],
            "status": "unavailable",
            "backend": {
                "name": "reentrant Rust fake",
                "version": "1.0.0",
                "dependency_lock_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "executable_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "environment_fingerprint": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            },
            "outputs": [],
            "warnings": [],
            "failure": {
                "code": "dependency_unavailable",
                "message": "fake dependency is intentionally unavailable",
                "retryable": false
            }
        });
        fs::write(
            response_path,
            serde_json::to_vec_pretty(&response).expect("serialize response"),
        )
        .expect("write response");
    }

    fn fake_invocation(timeout: Duration) -> BackendInvocation {
        BackendInvocation {
            executable: std::env::current_exe().expect("current test executable"),
            fixed_arguments: vec![
                "--ignored".to_string(),
                "--exact".to_string(),
                "generation_backends::process::tests::fake_backend_process".to_string(),
            ],
            timeout,
            max_response_bytes: 64 * 1024,
            max_stdout_bytes: 64 * 1024,
            max_stderr_bytes: 64 * 1024,
            output_limits: OutputLimits {
                max_output_files: 8,
                max_file_bytes: 1024 * 1024,
                max_total_output_bytes: 4 * 1024 * 1024,
            },
        }
    }

    fn request() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/generation-backend/request.json"
        ))
        .expect("request fixture")
    }

    fn unique_staging(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dts-backend-{label}-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
