use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::StreamingSha256;
use crate::sha256_hex;

pub const CONTENT_PROVIDER_PROTOCOL_VERSION: &str = "1.0.0";
const REQUEST_SCHEMA: &str = include_str!("../../schemas/composition-provider-request.schema.json");
const RESPONSE_SCHEMA: &str =
    include_str!("../../schemas/composition-provider-response.schema.json");
const REQUEST_FILE: &str = "request.json";
const RESPONSE_FILE: &str = "response.json";
const OUTPUT_DIRECTORY: &str = "outputs";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub protocol_version: String,
    pub request_id: String,
    pub provider_id: String,
    pub expected_provider_version: String,
    pub argument_sha256: String,
    pub instance_id: String,
    pub template_id: String,
    pub template_version: String,
    pub identities: BTreeMap<String, String>,
    pub output: ProviderOutputDeclaration,
    pub parameters: BTreeMap<String, Value>,
    pub network_policy: String,
}

impl ProviderRequest {
    pub fn canonical_request_id(&self) -> String {
        let mut canonical = self.clone();
        canonical.request_id.clear();
        sha256_hex(&serde_json::to_vec(&canonical).expect("provider request serializes"))
    }

    pub fn validate(&self) -> Result<(), ProviderError> {
        validate_schema(
            "composition provider request",
            REQUEST_SCHEMA,
            &serde_json::to_value(self).expect("provider request serializes"),
        )?;
        let expected = self.canonical_request_id();
        if self.request_id != expected {
            return Err(invalid(format!(
                "request_id {} does not match canonical request {expected}",
                self.request_id
            )));
        }
        if self.output.size_bytes > self.output.max_size_bytes {
            return Err(invalid(format!(
                "declared output size {} exceeds bound {}",
                self.output.size_bytes, self.output.max_size_bytes
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderOutputDeclaration {
    pub slot: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub max_size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub protocol_version: String,
    pub request_id: String,
    pub provider_id: String,
    pub provider_version: String,
    pub executable_sha256: String,
    pub argument_sha256: String,
    pub output: ProviderResponseOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponseOutput {
    pub slot: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct ProviderInvocation {
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub arguments: Vec<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOutput {
    pub path: PathBuf,
    pub request_sha256: String,
    pub response_sha256: String,
    pub provider_id: String,
    pub provider_version: String,
    pub executable_sha256: String,
    pub argument_sha256: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn invoke_content_provider(
    invocation: &ProviderInvocation,
    request: &ProviderRequest,
    staging_root: &Path,
) -> Result<ProviderOutput, ProviderError> {
    invoke_content_provider_cancellable(invocation, request, staging_root, &|| false)
}

pub(crate) fn invoke_content_provider_cancellable(
    invocation: &ProviderInvocation,
    request: &ProviderRequest,
    staging_root: &Path,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ProviderOutput, ProviderError> {
    request.validate()?;
    if invocation.timeout.is_zero() || invocation.timeout > Duration::from_secs(300) {
        return Err(invalid("provider timeout must be between 1 ms and 300 s"));
    }
    prepare_staging(staging_root)?;
    let request_path = staging_root.join(REQUEST_FILE);
    let response_path = staging_root.join(RESPONSE_FILE);
    let output_root = staging_root.join(OUTPUT_DIRECTORY);
    write_json_exclusive(&request_path, request)?;

    if !invocation.executable.is_absolute() {
        return Err(invalid("provider executable must be absolute"));
    }
    let requested_executable_metadata =
        fs::symlink_metadata(&invocation.executable).map_err(|source| ProviderError::Io {
            path: invocation.executable.clone(),
            source,
        })?;
    if requested_executable_metadata.file_type().is_symlink()
        || !requested_executable_metadata.is_file()
    {
        return Err(invalid(
            "provider executable must be a non-symlink regular file",
        ));
    }
    let canonical_executable =
        fs::canonicalize(&invocation.executable).map_err(|source| ProviderError::Io {
            path: invocation.executable.clone(),
            source,
        })?;
    let executable_metadata =
        fs::symlink_metadata(&canonical_executable).map_err(|source| ProviderError::Io {
            path: canonical_executable.clone(),
            source,
        })?;
    if !executable_metadata.is_file() {
        return Err(invalid("provider executable is not a regular file"));
    }
    let executable_sha256 = hash_file(&canonical_executable, u64::MAX)?.1;
    if executable_sha256 != invocation.executable_sha256 {
        return Err(invalid(format!(
            "provider executable hash is {executable_sha256}, expected {}",
            invocation.executable_sha256
        )));
    }
    let argument_sha256 = provider_arguments_sha256(&invocation.arguments);
    if argument_sha256 != request.argument_sha256 {
        return Err(invalid("provider argument hash does not match its request"));
    }

    let mut command = Command::new(&canonical_executable);
    command
        .args(&invocation.arguments)
        .current_dir(staging_root)
        .env_clear()
        .env("DTS_COMPOSITION_PROVIDER_REQUEST", &request_path)
        .env("DTS_COMPOSITION_PROVIDER_RESPONSE", &response_path)
        .env("DTS_COMPOSITION_PROVIDER_OUTPUTS", &output_root)
        .env("DTS_COMPOSITION_PROVIDER_NETWORK", "disabled")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_tree(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| invalid(format!("spawn provider: {error}")))?;
    let process_group_id = child.id();
    let stdout = child.stdout.take().expect("provider stdout is piped");
    let stderr = child.stderr.take().expect("provider stderr is piped");
    let (stdout_sender, stdout_receiver) = mpsc::sync_channel(1);
    let (stderr_sender, stderr_receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = stdout_sender.send(drain_bounded(stdout));
    });
    thread::spawn(move || {
        let _ = stderr_sender.send(drain_bounded(stderr));
    });
    let (status, stdout, stderr) = wait_for_provider(
        &mut child,
        process_group_id,
        invocation.timeout,
        stdout_receiver,
        stderr_receiver,
        is_cancelled,
        staging_root,
        &response_path,
        &output_root,
        request.output.max_size_bytes,
    )?;
    if !status.success() {
        return Err(invalid(format!("provider exited with status {status}")));
    }
    audit_running_staging(
        staging_root,
        &response_path,
        &output_root,
        request.output.max_size_bytes,
    )?;

    let response_metadata =
        fs::symlink_metadata(&response_path).map_err(|source| ProviderError::Io {
            path: response_path.clone(),
            source,
        })?;
    if response_metadata.file_type().is_symlink() || !response_metadata.is_file() {
        return Err(invalid("provider response must be a regular file"));
    }
    if response_metadata.len() > MAX_RESPONSE_BYTES {
        return Err(invalid(format!(
            "provider response exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    let response_bytes = fs::read(&response_path).map_err(|source| ProviderError::Io {
        path: response_path.clone(),
        source,
    })?;
    let response_value: Value =
        serde_json::from_slice(&response_bytes).map_err(|source| ProviderError::Parse {
            label: "provider response".into(),
            source,
        })?;
    validate_schema(
        "composition provider response",
        RESPONSE_SCHEMA,
        &response_value,
    )?;
    let response: ProviderResponse =
        serde_json::from_value(response_value).map_err(|source| ProviderError::Parse {
            label: "provider response".into(),
            source,
        })?;
    validate_response(request, &response, &executable_sha256)?;
    let output_path = output_root.join(&response.output.relative_path);
    audit_output_directory(&output_root, &output_path)?;
    let (size_bytes, sha256) = hash_file(&output_path, request.output.max_size_bytes)?;
    if size_bytes != request.output.size_bytes || sha256 != request.output.sha256 {
        return Err(invalid(format!(
            "provider output is {size_bytes} bytes with hash {sha256}, expected {} bytes with hash {}",
            request.output.size_bytes, request.output.sha256
        )));
    }

    let canonical_output = fs::canonicalize(&output_path).map_err(|source| ProviderError::Io {
        path: output_path,
        source,
    })?;
    Ok(ProviderOutput {
        path: canonical_output,
        request_sha256: sha256_hex(&serde_json::to_vec(request).expect("request serializes")),
        response_sha256: sha256_hex(&response_bytes),
        provider_id: response.provider_id,
        provider_version: response.provider_version,
        executable_sha256,
        argument_sha256,
        size_bytes,
        sha256,
        stdout,
        stderr,
    })
}

fn validate_response(
    request: &ProviderRequest,
    response: &ProviderResponse,
    executable_sha256: &str,
) -> Result<(), ProviderError> {
    let matches = response.protocol_version == request.protocol_version
        && response.request_id == request.request_id
        && response.provider_id == request.provider_id
        && response.provider_version == request.expected_provider_version
        && response.executable_sha256 == executable_sha256
        && response.argument_sha256 == request.argument_sha256
        && response.output.slot == request.output.slot
        && response.output.size_bytes == request.output.size_bytes
        && response.output.sha256 == request.output.sha256;
    if matches {
        Ok(())
    } else {
        Err(invalid("provider response does not match its request"))
    }
}

pub fn provider_arguments_sha256(arguments: &[String]) -> String {
    let mut canonical = Vec::new();
    for argument in arguments {
        canonical.extend_from_slice(&(argument.len() as u64).to_be_bytes());
        canonical.extend_from_slice(argument.as_bytes());
    }
    sha256_hex(&canonical)
}

fn wait_for_provider(
    child: &mut Child,
    process_group_id: u32,
    timeout: Duration,
    stdout_receiver: mpsc::Receiver<Result<Vec<u8>, ProviderError>>,
    stderr_receiver: mpsc::Receiver<Result<Vec<u8>, ProviderError>>,
    is_cancelled: &dyn Fn() -> bool,
    staging_root: &Path,
    response_path: &Path,
    output_root: &Path,
    max_output_bytes: u64,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>), ProviderError> {
    let deadline = Instant::now() + timeout;
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if is_cancelled() {
            terminate_process_tree(child, process_group_id);
            return Err(ProviderError::Cancelled);
        }
        if let Err(error) =
            audit_running_staging(staging_root, response_path, output_root, max_output_bytes)
        {
            terminate_process_tree(child, process_group_id);
            return Err(error);
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(status) => status,
                Err(error) => {
                    terminate_process_tree(child, process_group_id);
                    return Err(invalid(format!("wait for provider: {error}")));
                }
            };
        }
        if let Err(error) = poll_reader(&stdout_receiver, &mut stdout) {
            terminate_process_tree(child, process_group_id);
            return Err(error);
        }
        if let Err(error) = poll_reader(&stderr_receiver, &mut stderr) {
            terminate_process_tree(child, process_group_id);
            return Err(error);
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            return Ok((
                status.expect("provider status completed"),
                stdout.take().expect("provider stdout completed")?,
                stderr.take().expect("provider stderr completed")?,
            ));
        }
        if Instant::now() >= deadline {
            terminate_process_tree(child, process_group_id);
            return Err(ProviderError::Timeout {
                milliseconds: timeout.as_millis(),
            });
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn audit_running_staging(
    staging_root: &Path,
    response_path: &Path,
    output_root: &Path,
    max_output_bytes: u64,
) -> Result<(), ProviderError> {
    for entry in fs::read_dir(staging_root).map_err(|source| ProviderError::Io {
        path: staging_root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ProviderError::Io {
            path: staging_root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path != staging_root.join(REQUEST_FILE) && path != response_path && path != output_root {
            return Err(invalid("provider created an undeclared staging entry"));
        }
        let metadata = fs::symlink_metadata(&path).map_err(|source| ProviderError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("provider staging entries cannot be symlinks"));
        }
        if path == output_root && !metadata.is_dir() {
            return Err(invalid("provider replaced its output directory"));
        }
        if path == staging_root.join(REQUEST_FILE) && !metadata.is_file() {
            return Err(invalid("provider replaced its request file"));
        }
        if path == response_path && (!metadata.is_file() || metadata.len() > MAX_RESPONSE_BYTES) {
            return Err(invalid("provider response is not a bounded regular file"));
        }
    }

    let mut output_count = 0_u64;
    for entry in fs::read_dir(output_root).map_err(|source| ProviderError::Io {
        path: output_root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ProviderError::Io {
            path: output_root.to_path_buf(),
            source,
        })?;
        output_count += 1;
        if output_count > 1 {
            return Err(invalid("provider created more than one output"));
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| ProviderError::Io {
            path: entry.path(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(invalid("provider output must be a regular file"));
        }
        if metadata.len() > max_output_bytes {
            return Err(invalid(format!(
                "provider output exceeds {max_output_bytes} bytes while running"
            )));
        }
    }
    Ok(())
}

fn poll_reader(
    receiver: &mpsc::Receiver<Result<Vec<u8>, ProviderError>>,
    result: &mut Option<Result<Vec<u8>, ProviderError>>,
) -> Result<(), ProviderError> {
    if result.is_none() {
        match receiver.try_recv() {
            Ok(value) => *result = Some(value),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(invalid("provider diagnostic reader terminated"));
            }
        }
    }
    Ok(())
}

fn drain_bounded(mut reader: impl Read) -> Result<Vec<u8>, ProviderError> {
    let mut output = Vec::new();
    reader
        .by_ref()
        .take(MAX_DIAGNOSTIC_BYTES as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|error| invalid(format!("read provider diagnostics: {error}")))?;
    if output.len() > MAX_DIAGNOSTIC_BYTES {
        return Err(invalid(format!(
            "provider diagnostics exceed {MAX_DIAGNOSTIC_BYTES} bytes"
        )));
    }
    Ok(output)
}

fn prepare_staging(root: &Path) -> Result<(), ProviderError> {
    fs::create_dir(root).map_err(|source| ProviderError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|source| {
            ProviderError::Io {
                path: root.to_path_buf(),
                source,
            }
        })?;
    }
    fs::create_dir(root.join(OUTPUT_DIRECTORY)).map_err(|source| ProviderError::Io {
        path: root.join(OUTPUT_DIRECTORY),
        source,
    })?;
    Ok(())
}

fn write_json_exclusive(path: &Path, value: &impl Serialize) -> Result<(), ProviderError> {
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ProviderError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::to_writer_pretty(file, value).map_err(|source| ProviderError::JsonWrite {
        path: path.to_path_buf(),
        source,
    })
}

fn audit_output_directory(root: &Path, declared: &Path) -> Result<(), ProviderError> {
    let mut regular_files = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|source| ProviderError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ProviderError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| ProviderError::Io {
            path: entry.path(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(invalid("provider outputs must contain only regular files"));
        }
        regular_files.insert(entry.path());
    }
    if regular_files == BTreeSet::from([declared.to_path_buf()]) {
        Ok(())
    } else {
        Err(invalid("provider created an undeclared output file"))
    }
}

fn hash_file(path: &Path, maximum: u64) -> Result<(u64, String), ProviderError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ProviderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("provider output must be a regular file"));
    }
    if metadata.len() > maximum {
        return Err(invalid(format!(
            "provider output is {} bytes, limit is {maximum}",
            metadata.len()
        )));
    }
    let mut file = File::open(path).map_err(|source| ProviderError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = StreamingSha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(|source| ProviderError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| invalid("provider output size overflow"))?;
        if total > maximum {
            return Err(invalid(format!("provider output exceeds {maximum} bytes")));
        }
        hasher.update(&buffer[..read]);
    }
    Ok((total, hasher.finish_hex()))
}

fn validate_schema(label: &str, schema: &str, value: &Value) -> Result<(), ProviderError> {
    let schema: Value = serde_json::from_str(schema).expect("embedded provider schema parses");
    let validator = jsonschema::validator_for(&schema).expect("provider schema compiles");
    let problems = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ProviderError::Invalid {
            message: format!("invalid {label}: {}", problems.join("; ")),
        })
    }
}

fn configure_process_tree(command: &mut Command) {
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

fn terminate_process_tree(child: &mut Child, process_group_id: u32) {
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

fn invalid(message: impl Into<String>) -> ProviderError {
    ProviderError::Invalid {
        message: message.into(),
    }
}

#[derive(Debug)]
pub enum ProviderError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        label: String,
        source: serde_json::Error,
    },
    JsonWrite {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid {
        message: String,
    },
    Timeout {
        milliseconds: u128,
    },
    Cancelled,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProviderError {}
