use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::sha256_hex;

use super::{
    BackendContractError, OutputLimits, stage_declared_sources, validate_request,
    validate_response_for_request, verify_staged_outputs,
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
    pub dependency_lock_sha256: String,
    pub environment_fingerprint: String,
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
    input_root: &Path,
    staging_root: &Path,
) -> Result<BackendRun, BackendContractError> {
    invoke_backend_cancellable(invocation, request, input_root, staging_root, &|| false)
}

pub fn invoke_backend_cancellable(
    invocation: &BackendInvocation,
    request: &Value,
    input_root: &Path,
    staging_root: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<BackendRun, BackendContractError> {
    if cancelled() {
        return Err(invalid("backend invocation cancelled".into()));
    }
    validate_request(request)?;
    prepare_private_staging(staging_root)?;
    let inputs = staging_root.join(INPUTS_DIRECTORY);
    let outputs = staging_root.join(OUTPUT_DIRECTORY);
    let request_path = staging_root.join(REQUEST_FILE);
    let response_path = staging_root.join(RESPONSE_FILE);
    stage_declared_sources(request, input_root, &inputs)?;

    let mut staged_request = request.clone();
    staged_request["staging"]["root"] = Value::String(staging_root.display().to_string());
    staged_request["staging"]["inputs_directory"] = Value::String(inputs.display().to_string());
    staged_request["staging"]["output_directory"] = Value::String(outputs.display().to_string());
    validate_request(&staged_request)?;
    write_json_exclusive(&request_path, &staged_request)?;

    if !invocation.executable.is_absolute() {
        return Err(invalid(format!(
            "backend executable {} must be an absolute prepared-runtime path",
            invocation.executable.display()
        )));
    }
    let canonical_executable =
        fs::canonicalize(&invocation.executable).map_err(|source| BackendContractError::Read {
            path: invocation.executable.clone(),
            source,
        })?;
    if !canonical_executable.is_file() {
        return Err(invalid(format!(
            "backend executable {} is not a regular file",
            canonical_executable.display()
        )));
    }
    let executable_fingerprint = executable_fingerprint(&canonical_executable)?;

    let mut command = Command::new(&invocation.executable);
    command
        .args(&invocation.fixed_arguments)
        .current_dir(staging_root)
        .env_clear()
        .env("DTS_BACKEND_REQUEST", &request_path)
        .env("DTS_BACKEND_RESPONSE", &response_path)
        .env("DTS_BACKEND_OUTPUTS", &outputs)
        .env(
            "DTS_BACKEND_DEPENDENCY_LOCK_SHA256",
            &invocation.dependency_lock_sha256,
        )
        .env(
            "DTS_BACKEND_EXECUTABLE_FINGERPRINT",
            &executable_fingerprint,
        )
        .env(
            "DTS_BACKEND_ENVIRONMENT_FINGERPRINT",
            &invocation.environment_fingerprint,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_tree(&mut command);

    let mut child = command.spawn().map_err(|error| {
        invalid(format!(
            "spawn backend {}: {error}",
            invocation.executable.display()
        ))
    })?;
    let process_group_id = child.id();
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_limit = invocation.max_stdout_bytes;
    let stderr_limit = invocation.max_stderr_bytes;
    let (stdout_sender, stdout_receiver) = mpsc::sync_channel(1);
    let (stderr_sender, stderr_receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = stdout_sender.send(drain_bounded(stdout, stdout_limit));
    });
    thread::spawn(move || {
        let _ = stderr_sender.send(drain_bounded(stderr, stderr_limit));
    });

    let deadline = Instant::now() + invocation.timeout;
    let mut status = None;
    let mut stdout_result = None;
    let mut stderr_result = None;
    while status.is_none() || stdout_result.is_none() || stderr_result.is_none() {
        if cancelled() {
            terminate_process_tree(&mut child, process_group_id);
            return Err(invalid("backend invocation cancelled".into()));
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(error) => {
                    terminate_process_tree(&mut child, process_group_id);
                    return Err(invalid(format!("wait for backend: {error}")));
                }
            }
        }
        poll_backend_reader(
            &stdout_receiver,
            &mut stdout_result,
            "stdout",
            &mut child,
            process_group_id,
        )?;
        poll_backend_reader(
            &stderr_receiver,
            &mut stderr_result,
            "stderr",
            &mut child,
            process_group_id,
        )?;
        if status.is_some() && stdout_result.is_some() && stderr_result.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            terminate_process_tree(&mut child, process_group_id);
            return Err(invalid(format!(
                "backend invocation exceeded {} ms",
                invocation.timeout.as_millis()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }

    let status = status.expect("completed child status");
    let stdout = stdout_result.expect("completed stdout reader")?;
    let stderr = stderr_result.expect("completed stderr reader")?;
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
    verify_response_provenance(
        &response,
        &invocation.dependency_lock_sha256,
        &executable_fingerprint,
        &invocation.environment_fingerprint,
    )?;
    verify_staged_outputs(
        &staged_request,
        &response,
        &outputs,
        invocation.output_limits,
    )?;

    Ok(BackendRun {
        response,
        staging_root: staging_root.to_path_buf(),
        stdout,
        stderr,
    })
}

fn poll_backend_reader(
    receiver: &mpsc::Receiver<Result<Vec<u8>, BackendContractError>>,
    result: &mut Option<Result<Vec<u8>, BackendContractError>>,
    label: &str,
    child: &mut Child,
    process_group_id: u32,
) -> Result<(), BackendContractError> {
    if result.is_some() {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(value) => *result = Some(value),
        Err(TryRecvError::Empty) => {}
        Err(TryRecvError::Disconnected) => {
            terminate_process_tree(child, process_group_id);
            return Err(invalid(format!("backend {label} reader terminated")));
        }
    }
    Ok(())
}

pub(super) fn configure_process_tree(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
}

pub(super) fn terminate_process_tree(child: &mut Child, process_group_id: u32) {
    #[cfg(unix)]
    unsafe {
        let _ = libc::kill(-(process_group_id as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &process_group_id.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub fn executable_fingerprint(executable: &Path) -> Result<String, BackendContractError> {
    let bytes = fs::read(executable).map_err(|source| BackendContractError::Read {
        path: executable.to_path_buf(),
        source,
    })?;
    Ok(sha256_hex(&bytes))
}

pub fn environment_fingerprint(fixed_arguments: &[String]) -> String {
    let mut material = Vec::new();
    for component in [
        super::PROTOCOL_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
    ] {
        material.extend_from_slice(component.as_bytes());
        material.push(0);
    }
    for argument in fixed_arguments {
        material.extend_from_slice(argument.as_bytes());
        material.push(0);
    }
    sha256_hex(&material)
}

fn verify_response_provenance(
    response: &Value,
    expected_dependency_lock: &str,
    expected_executable: &str,
    expected_environment: &str,
) -> Result<(), BackendContractError> {
    for (field, expected) in [
        ("dependency_lock_sha256", expected_dependency_lock),
        ("executable_fingerprint", expected_executable),
        ("environment_fingerprint", expected_environment),
    ] {
        let actual = response
            .pointer(&format!("/backend/{field}"))
            .and_then(Value::as_str)
            .expect("response schema checked backend provenance");
        if actual != expected {
            return Err(invalid(format!(
                "backend response {field} is {actual}, expected {expected}"
            )));
        }
    }
    Ok(())
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
            Path::new("."),
            &staging,
        )
        .expect("fake backend should return unavailable");
        assert_eq!(result.response["status"], "unavailable");
        assert!(result.stderr.is_empty());
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_identity_mismatch_is_rejected() {
        let staging = unique_staging("identity-mismatch");
        let error = invoke_backend(
            &fake_invocation(Duration::from_secs(2)),
            &request(),
            Path::new("."),
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
            Path::new("."),
            &staging,
        )
        .expect_err("slow backend must time out");
        assert!(error.to_string().contains("exceeded"));
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_cancellation_kills_and_reaps_the_child_promptly() {
        let staging = unique_staging("grandchild-cancelled");
        let started = Instant::now();
        let error = invoke_backend_cancellable(
            &fake_invocation(Duration::from_secs(30)),
            &request(),
            Path::new("."),
            &staging,
            &|| started.elapsed() >= Duration::from_millis(30),
        )
        .expect_err("cancelled backend must be terminated");
        assert!(error.to_string().contains("cancelled"));
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "cancellation must not wait for the backend timeout"
        );
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_inherited_pipe_timeout_is_enforced() {
        let staging = unique_staging("grandchild");
        let started = Instant::now();
        let error = invoke_backend(
            &fake_pipe_holder_invocation(Duration::from_millis(500)),
            &request(),
            Path::new("."),
            &staging,
        )
        .expect_err("inherited backend pipe must time out");
        assert!(error.to_string().contains("exceeded"));
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "pipe readers must not wait for a long-lived grandchild"
        );
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_undeclared_output_is_rejected() {
        let staging = unique_staging("undeclared");
        let error = invoke_backend(
            &fake_invocation(Duration::from_secs(2)),
            &request(),
            Path::new("."),
            &staging,
        )
        .expect_err("undeclared backend output must fail");
        assert!(error.to_string().contains("undeclared"));
        fs::remove_dir_all(staging).expect("remove fake staging");
    }

    #[test]
    fn fake_backend_fingerprint_mismatch_is_rejected() {
        let staging = unique_staging("fingerprint-mismatch");
        let error = invoke_backend(
            &fake_invocation(Duration::from_secs(2)),
            &request(),
            Path::new("."),
            &staging,
        )
        .expect_err("false executable fingerprint must fail");
        assert!(error.to_string().contains("executable_fingerprint"));
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
        if behavior.contains("grandchild") {
            #[cfg(unix)]
            Command::new("/bin/sleep")
                .arg("10")
                .spawn()
                .expect("spawn pipe-holding grandchild");
            #[cfg(windows)]
            Command::new(std::env::current_exe().expect("pipe-holder executable"))
                .args([
                    "--ignored",
                    "--exact",
                    "generation_backends::process::tests::fake_backend_pipe_holder",
                ])
                .spawn()
                .expect("spawn pipe-holding grandchild");
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
        let request_id = if behavior.contains("identity-mismatch") {
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        } else {
            request["request_id"].as_str().expect("request id")
        };
        let dependency_lock_sha256 =
            std::env::var("DTS_BACKEND_DEPENDENCY_LOCK_SHA256").expect("dependency fingerprint");
        let executable_fingerprint = if behavior.contains("fingerprint-mismatch") {
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()
        } else {
            std::env::var("DTS_BACKEND_EXECUTABLE_FINGERPRINT").expect("executable fingerprint")
        };
        let environment_fingerprint =
            std::env::var("DTS_BACKEND_ENVIRONMENT_FINGERPRINT").expect("environment fingerprint");
        let response = json!({
            "response_schema_version": "0.1.0",
            "protocol_version": request["protocol_version"],
            "request_id": request_id,
            "backend_id": request["backend_id"],
            "status": "unavailable",
            "backend": {
                "name": "reentrant Rust fake",
                "version": "1.0.0",
                "dependency_lock_sha256": dependency_lock_sha256,
                "executable_fingerprint": executable_fingerprint,
                "environment_fingerprint": environment_fingerprint
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

    #[test]
    #[ignore]
    fn fake_backend_pipe_holder() {
        thread::sleep(Duration::from_secs(10));
    }

    fn fake_invocation(timeout: Duration) -> BackendInvocation {
        let fixed_arguments = vec![
            "--ignored".to_string(),
            "--exact".to_string(),
            "generation_backends::process::tests::fake_backend_process".to_string(),
        ];
        let environment_fingerprint = environment_fingerprint(&fixed_arguments);
        BackendInvocation {
            executable: std::env::current_exe().expect("current test executable"),
            fixed_arguments,
            timeout,
            max_response_bytes: 64 * 1024,
            max_stdout_bytes: 64 * 1024,
            max_stderr_bytes: 64 * 1024,
            output_limits: OutputLimits {
                max_output_files: 8,
                max_file_bytes: 1024 * 1024,
                max_total_output_bytes: 4 * 1024 * 1024,
            },
            dependency_lock_sha256:
                "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            environment_fingerprint,
        }
    }

    fn fake_pipe_holder_invocation(timeout: Duration) -> BackendInvocation {
        #[cfg(unix)]
        let (executable, fixed_arguments) = (
            PathBuf::from("/bin/sh"),
            vec!["-c".to_string(), "/bin/sleep 10 &".to_string()],
        );
        #[cfg(windows)]
        let (executable, fixed_arguments) = (
            std::env::current_exe().expect("current test executable"),
            vec![
                "--ignored".to_string(),
                "--exact".to_string(),
                "generation_backends::process::tests::fake_backend_process".to_string(),
            ],
        );
        let environment_fingerprint = environment_fingerprint(&fixed_arguments);
        BackendInvocation {
            executable,
            fixed_arguments,
            timeout,
            max_response_bytes: 4096,
            max_stdout_bytes: 4096,
            max_stderr_bytes: 4096,
            output_limits: OutputLimits {
                max_output_files: 8,
                max_file_bytes: 1024 * 1024,
                max_total_output_bytes: 4 * 1024 * 1024,
            },
            dependency_lock_sha256: "d".repeat(64),
            environment_fingerprint,
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
