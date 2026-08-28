//! Executable, opt-in DICOMDIR qualification runner.
//!
//! Existing generated DICOM files are copied into a private File-set staging
//! tree. External commands are invoked directly, never through a shell. The
//! staging tree is removed on every return path, while the returned
//! [`DicomDirQualification`] contains identities, hashes, provider provenance,
//! and validation outcomes but no DICOM payload bytes.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::media::{
    CheckStatus, DcmtkProviderFingerprint, DcmtkProviderResult, DicomDirQualification,
    DirectoryRecordReference, FileId, FileSetMember, LOCKED_FILE_SET_ID, LOCKED_PROVIDER_ID,
    LOCKED_PROVIDER_VERSION, MediaError, MediaValidationEvidence, MixedFileSet,
};

const MAX_TOOL_OUTPUT_BYTES: u64 = 1024 * 1024;
static STAGING_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaToolPaths {
    pub dcmmkdir: PathBuf,
    pub dcmdump: PathBuf,
    pub dciodvfy: PathBuf,
    /// `None` records the explicitly permitted "where supported" state.
    pub dcentvfy: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSourcePath {
    pub source_path: PathBuf,
    pub member: FileSetMember,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DicomDirRunRequest {
    pub tools: MediaToolPaths,
    pub sources: Vec<MediaSourcePath>,
    pub timeout: Duration,
    /// Primarily useful for controlled CI and tests. The created child is
    /// always private to this invocation and is always removed.
    pub staging_parent: Option<PathBuf>,
}

pub fn run_dicomdir_qualification(
    request: &DicomDirRunRequest,
) -> Result<DicomDirQualification, MediaRunnerError> {
    if request.timeout.is_zero() {
        return Err(MediaRunnerError::InvalidRequest(
            "tool timeout must be non-zero",
        ));
    }
    if request.sources.is_empty() {
        return Err(MediaRunnerError::InvalidRequest(
            "at least one media source is required",
        ));
    }

    let staging = StagingDirectory::create(request.staging_parent.as_deref())?;
    let mut staged_paths = Vec::with_capacity(request.sources.len());
    let mut staged_hashes = BTreeMap::new();
    let mut members = Vec::with_capacity(request.sources.len());
    let mut sorted_sources = request.sources.iter().collect::<Vec<_>>();
    sorted_sources.sort_by(|left, right| left.member.file_id.cmp(&right.member.file_id));
    for source in sorted_sources {
        let source_bytes = fs::read(&source.source_path).map_err(|error| {
            MediaRunnerError::Io(format!(
                "read source {}: {error}",
                source.source_path.display()
            ))
        })?;
        let source_sha256 = crate::sha256_hex(&source_bytes);
        if source_sha256 != source.member.sha256 {
            return Err(MediaRunnerError::SourceHashMismatch {
                path: source.source_path.clone(),
                expected: source.member.sha256.clone(),
                actual: source_sha256,
            });
        }
        let staged_path = staging.path_for(&source.member.file_id);
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                MediaRunnerError::Io(format!("create {}: {error}", parent.display()))
            })?;
        }
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
            .map_err(|error| {
                MediaRunnerError::Io(format!("create {}: {error}", staged_path.display()))
            })?;
        target.write_all(&source_bytes).map_err(|error| {
            MediaRunnerError::Io(format!("write {}: {error}", staged_path.display()))
        })?;
        staged_hashes.insert(source.member.file_id.clone(), source.member.sha256.clone());
        staged_paths.push(staged_path);
        members.push(source.member.clone());
    }

    let dicomdir_path = staging.fileset_root.join("DICOMDIR");
    let actual_arguments = vec![
        "-Pgp".to_owned(),
        "+F".to_owned(),
        LOCKED_FILE_SET_ID.to_owned(),
        "+id".to_owned(),
        staging.fileset_root.to_string_lossy().into_owned(),
        "+r".to_owned(),
        "+D".to_owned(),
        dicomdir_path.to_string_lossy().into_owned(),
        "-nb".to_owned(),
    ];

    let version = run_tool(
        &request.tools.dcmmkdir,
        &["--version".to_owned()],
        &staging.root,
        request.timeout,
        &staging.log_root,
        "dcmmkdir-version",
    )?;
    if !version.status.success() || !version.stdout.contains("dcmmkdir v3.7.0") {
        return Err(MediaRunnerError::ProviderVersionMismatch(version.stdout));
    }
    let executable_bytes = fs::read(&request.tools.dcmmkdir).map_err(|error| {
        MediaRunnerError::Io(format!(
            "read provider {}: {error}",
            request.tools.dcmmkdir.display()
        ))
    })?;
    let fingerprint = DcmtkProviderFingerprint {
        provider_id: LOCKED_PROVIDER_ID.to_owned(),
        executable_name: "dcmmkdir".to_owned(),
        version: LOCKED_PROVIDER_VERSION.to_owned(),
        executable_sha256: crate::sha256_hex(&executable_bytes),
        arguments: actual_arguments.clone(),
    };

    let provider = run_tool(
        &request.tools.dcmmkdir,
        &actual_arguments,
        &staging.root,
        request.timeout,
        &staging.log_root,
        "dcmmkdir",
    )?;
    if !provider.status.success() {
        return Err(MediaRunnerError::ToolFailed {
            tool: "dcmmkdir",
            status: provider.status.code(),
            stderr: provider.stderr,
        });
    }
    if !dicomdir_path.is_file() {
        return Err(MediaRunnerError::MissingDicomDir);
    }

    let dump = run_tool(
        &request.tools.dcmdump,
        &[
            "-Un".to_owned(),
            dicomdir_path.to_string_lossy().into_owned(),
        ],
        &staging.root,
        request.timeout,
        &staging.log_root,
        "dcmdump",
    )?;
    if !dump.status.success() {
        return Err(MediaRunnerError::ToolFailed {
            tool: "dcmdump",
            status: dump.status.code(),
            stderr: dump.stderr,
        });
    }
    let parsed = ParsedDicomDir::parse(&dump.stdout)?;
    if parsed.file_set_id != LOCKED_FILE_SET_ID {
        return Err(MediaRunnerError::ParsedFileSetIdMismatch(
            parsed.file_set_id,
        ));
    }
    let file_set = MixedFileSet::validate(members, &parsed.records)?;

    let dciodvfy = run_tool(
        &request.tools.dciodvfy,
        &[
            "-new".to_owned(),
            dicomdir_path.to_string_lossy().into_owned(),
        ],
        &staging.root,
        request.timeout,
        &staging.log_root,
        "dciodvfy",
    )?;
    if !dciodvfy.status.success() {
        return Err(MediaRunnerError::ToolFailed {
            tool: "dciodvfy",
            status: dciodvfy.status.code(),
            stderr: dciodvfy.stderr,
        });
    }

    let dcentvfy_status = if let Some(executable) = &request.tools.dcentvfy {
        let mut arguments = vec![dicomdir_path.to_string_lossy().into_owned()];
        arguments.extend(
            staged_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        let result = run_tool(
            executable,
            &arguments,
            &staging.root,
            request.timeout,
            &staging.log_root,
            "dcentvfy",
        )?;
        if !result.status.success() {
            return Err(MediaRunnerError::ToolFailed {
                tool: "dcentvfy",
                status: result.status.code(),
                stderr: result.stderr,
            });
        }
        CheckStatus::Passed
    } else {
        CheckStatus::Unavailable
    };

    let dicomdir_bytes = fs::read(&dicomdir_path).map_err(|error| {
        MediaRunnerError::Io(format!("read {}: {error}", dicomdir_path.display()))
    })?;
    let provider_result = DcmtkProviderResult {
        fingerprint,
        exit_code: provider.status.code().unwrap_or(0),
        file_set_id: parsed.file_set_id,
        file_set_uid: parsed.file_meta_sop_instance_uid.clone(),
        dicomdir_sop_class_uid: parsed.file_meta_sop_class_uid,
        dicomdir_sop_instance_uid: parsed.file_meta_sop_instance_uid,
        dicomdir_transfer_syntax_uid: parsed.transfer_syntax_uid,
        dicomdir_sha256: crate::sha256_hex(&dicomdir_bytes),
        member_sha256: staged_hashes,
        warnings: provider
            .stderr
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect(),
    };
    let evidence = MediaValidationEvidence {
        rust_closure: CheckStatus::Passed,
        dicom3tools_dciodvfy: CheckStatus::Passed,
        dicom3tools_dcentvfy: dcentvfy_status,
        dcmtk_parser_same_family: CheckStatus::Passed,
        // A DCMTK-generated File-set parsed by DCMTK is not independent.
        dcm4che_independent_peer: CheckStatus::Unavailable,
    };
    DicomDirQualification::qualify(&file_set, provider_result, evidence)
        .map_err(MediaRunnerError::from)
}

struct StagingDirectory {
    root: PathBuf,
    fileset_root: PathBuf,
    log_root: PathBuf,
}

impl StagingDirectory {
    fn create(parent: Option<&Path>) -> Result<Self, MediaRunnerError> {
        let parent = parent
            .map(Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&parent).map_err(|error| {
            MediaRunnerError::Io(format!(
                "create staging parent {}: {error}",
                parent.display()
            ))
        })?;
        for _ in 0..100 {
            let nonce = STAGING_NONCE.fetch_add(1, Ordering::Relaxed);
            let root = parent.join(format!("dts-media-{}-{nonce}", std::process::id()));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let fileset_root = root.join("fileset");
                    let log_root = root.join("logs");
                    if let Err(error) = fs::create_dir(&fileset_root) {
                        let _ = fs::remove_dir_all(&root);
                        return Err(MediaRunnerError::Io(format!(
                            "create File-set staging {}: {error}",
                            fileset_root.display()
                        )));
                    }
                    if let Err(error) = fs::create_dir(&log_root) {
                        let _ = fs::remove_dir_all(&root);
                        return Err(MediaRunnerError::Io(format!(
                            "create tool log staging {}: {error}",
                            log_root.display()
                        )));
                    }
                    return Ok(Self {
                        root,
                        fileset_root,
                        log_root,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(MediaRunnerError::Io(format!(
                        "create staging directory {}: {error}",
                        root.display()
                    )));
                }
            }
        }
        Err(MediaRunnerError::Io(
            "could not allocate a unique staging directory".to_owned(),
        ))
    }

    fn path_for(&self, file_id: &FileId) -> PathBuf {
        file_id
            .components()
            .iter()
            .fold(self.fileset_root.clone(), |path, component| {
                path.join(component)
            })
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ToolOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_tool(
    executable: &Path,
    arguments: &[String],
    current_dir: &Path,
    timeout: Duration,
    log_dir: &Path,
    label: &str,
) -> Result<ToolOutput, MediaRunnerError> {
    let stdout_path = log_dir.join(format!(".{label}.stdout"));
    let stderr_path = log_dir.join(format!(".{label}.stderr"));
    let stdout_file = File::create(&stdout_path).map_err(|error| {
        MediaRunnerError::Io(format!("create {}: {error}", stdout_path.display()))
    })?;
    let stderr_file = File::create(&stderr_path).map_err(|error| {
        MediaRunnerError::Io(format!("create {}: {error}", stderr_path.display()))
    })?;
    let mut child = Command::new(executable)
        .args(arguments)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|error| {
            MediaRunnerError::Io(format!("spawn {}: {error}", executable.display()))
        })?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            MediaRunnerError::Io(format!("wait for {}: {error}", executable.display()))
        })? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MediaRunnerError::ToolTimedOut {
                tool: executable.to_path_buf(),
                timeout,
            });
        }
        thread::sleep(Duration::from_millis(10));
    };
    Ok(ToolOutput {
        status,
        stdout: read_limited(&stdout_path)?,
        stderr: read_limited(&stderr_path)?,
    })
}

fn read_limited(path: &Path) -> Result<String, MediaRunnerError> {
    let file = File::open(path)
        .map_err(|error| MediaRunnerError::Io(format!("open {}: {error}", path.display())))?;
    let mut bytes = Vec::new();
    file.take(MAX_TOOL_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| MediaRunnerError::Io(format!("read {}: {error}", path.display())))?;
    if bytes.len() as u64 > MAX_TOOL_OUTPUT_BYTES {
        return Err(MediaRunnerError::ToolOutputTooLarge(path.to_path_buf()));
    }
    String::from_utf8(bytes).map_err(|_| MediaRunnerError::NonUtf8ToolOutput(path.to_path_buf()))
}

#[derive(Debug)]
struct ParsedDicomDir {
    file_set_id: String,
    file_meta_sop_class_uid: String,
    file_meta_sop_instance_uid: String,
    transfer_syntax_uid: String,
    records: Vec<DirectoryRecordReference>,
}

impl ParsedDicomDir {
    fn parse(dump: &str) -> Result<Self, MediaRunnerError> {
        let file_set_id = required_dump_value(dump, "(0004,1130)")?;
        let file_meta_sop_class_uid = required_dump_value(dump, "(0002,0002)")?;
        let file_meta_sop_instance_uid = required_dump_value(dump, "(0002,0003)")?;
        let transfer_syntax_uid = required_dump_value(dump, "(0002,0010)")?;
        let mut records = Vec::new();
        let mut pending: Option<(FileId, Option<String>, Option<String>)> = None;
        for line in dump.lines() {
            if let Some(value) = dump_value(line, "(0004,1500)") {
                flush_record(&mut pending, &mut records)?;
                let file_id = FileId::new(value.split('\\').map(str::to_owned))?;
                pending = Some((file_id, None, None));
            } else if let Some(value) = dump_value(line, "(0004,1510)") {
                if let Some((_, sop_class_uid, _)) = &mut pending {
                    *sop_class_uid = Some(value);
                }
            } else if let Some(value) = dump_value(line, "(0004,1511)") {
                if let Some((_, _, sop_instance_uid)) = &mut pending {
                    *sop_instance_uid = Some(value);
                }
            }
        }
        flush_record(&mut pending, &mut records)?;
        if records.is_empty() {
            return Err(MediaRunnerError::MalformedDicomDump(
                "no referenced File IDs were present",
            ));
        }
        Ok(Self {
            file_set_id,
            file_meta_sop_class_uid,
            file_meta_sop_instance_uid,
            transfer_syntax_uid,
            records,
        })
    }
}

fn required_dump_value(dump: &str, tag: &'static str) -> Result<String, MediaRunnerError> {
    dump.lines()
        .find_map(|line| dump_value(line, tag))
        .ok_or(MediaRunnerError::MissingDumpTag(tag))
}

fn dump_value(line: &str, tag: &str) -> Option<String> {
    line.contains(tag).then_some(())?;
    let start = line.find('[')? + 1;
    let end = line[start..].find(']')? + start;
    Some(line[start..end].trim().to_owned())
}

type PendingRecord = Option<(FileId, Option<String>, Option<String>)>;

fn flush_record(
    pending: &mut PendingRecord,
    records: &mut Vec<DirectoryRecordReference>,
) -> Result<(), MediaRunnerError> {
    let Some((file_id, sop_class_uid, sop_instance_uid)) = pending.take() else {
        return Ok(());
    };
    let Some(referenced_sop_class_uid) = sop_class_uid else {
        return Err(MediaRunnerError::MalformedDicomDump(
            "a File ID lacks Referenced SOP Class UID in File",
        ));
    };
    let Some(referenced_sop_instance_uid) = sop_instance_uid else {
        return Err(MediaRunnerError::MalformedDicomDump(
            "a File ID lacks Referenced SOP Instance UID in File",
        ));
    };
    records.push(DirectoryRecordReference {
        file_id,
        referenced_sop_class_uid,
        referenced_sop_instance_uid,
    });
    Ok(())
}

#[derive(Debug)]
pub enum MediaRunnerError {
    InvalidRequest(&'static str),
    Io(String),
    SourceHashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    ProviderVersionMismatch(String),
    ToolTimedOut {
        tool: PathBuf,
        timeout: Duration,
    },
    ToolFailed {
        tool: &'static str,
        status: Option<i32>,
        stderr: String,
    },
    ToolOutputTooLarge(PathBuf),
    NonUtf8ToolOutput(PathBuf),
    MissingDicomDir,
    MissingDumpTag(&'static str),
    MalformedDicomDump(&'static str),
    ParsedFileSetIdMismatch(String),
    Media(MediaError),
}

impl fmt::Display for MediaRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(detail) => write!(formatter, "invalid media request: {detail}"),
            Self::Io(detail) => write!(formatter, "media I/O failed: {detail}"),
            Self::SourceHashMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "source {} SHA-256 is {actual}, expected {expected}",
                path.display()
            ),
            Self::ProviderVersionMismatch(output) => {
                write!(formatter, "dcmmkdir version lock mismatch: {output}")
            }
            Self::ToolTimedOut { tool, timeout } => {
                write!(formatter, "{} exceeded {timeout:?}", tool.display())
            }
            Self::ToolFailed {
                tool,
                status,
                stderr,
            } => write!(formatter, "{tool} failed with {status:?}: {stderr}"),
            Self::ToolOutputTooLarge(path) => {
                write!(formatter, "tool output {} exceeds 1 MiB", path.display())
            }
            Self::NonUtf8ToolOutput(path) => {
                write!(formatter, "tool output {} is not UTF-8", path.display())
            }
            Self::MissingDicomDir => write!(formatter, "dcmmkdir did not create DICOMDIR"),
            Self::MissingDumpTag(tag) => write!(formatter, "dcmdump lacks required tag {tag}"),
            Self::MalformedDicomDump(detail) => write!(formatter, "malformed dcmdump: {detail}"),
            Self::ParsedFileSetIdMismatch(actual) => {
                write!(
                    formatter,
                    "DICOMDIR File-set ID is {actual}, expected DTSMIXED"
                )
            }
            Self::Media(error) => error.fmt(formatter),
        }
    }
}

impl Error for MediaRunnerError {}

impl From<MediaError> for MediaRunnerError {
    fn from(value: MediaError) -> Self {
        Self::Media(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{MediaDeterminism, MemberRole};

    fn fixture_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "dts-media-runner-test-{}-{}-{label}",
            std::process::id(),
            STAGING_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[cfg(unix)]
    fn executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn member(
        role: MemberRole,
        ordinal: u32,
        class_uid: &str,
        instance_uid: &str,
        references: &[&str],
    ) -> FileSetMember {
        FileSetMember {
            case_id: format!("fixture/{role:?}"),
            role,
            file_id: FileId::for_member(role, ordinal).unwrap(),
            sha256: crate::sha256_hex(b"source"),
            sop_class_uid: class_uid.to_owned(),
            sop_instance_uid: instance_uid.to_owned(),
            referenced_sop_instance_uids: references.iter().map(|uid| (*uid).to_owned()).collect(),
        }
    }

    #[cfg(unix)]
    fn request(root: &Path, failing_validator: bool) -> DicomDirRunRequest {
        let tools = root.join("tools");
        let sources = root.join("sources");
        let staging = root.join("staging");
        fs::create_dir_all(&tools).unwrap();
        fs::create_dir_all(&sources).unwrap();
        fs::create_dir_all(&staging).unwrap();
        let dcmmkdir = tools.join("dcmmkdir");
        let dcmmkdir_script = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo '$dcmtk: dcmmkdir v3.7.0 2025-12-15 $'
  exit 0
fi
while [ "$#" -gt 0 ]; do
  if [ "$1" = "+D" ]; then
    shift
    printf 'synthetic dicomdir' > "$1"
  fi
  if [ "$1" = "+id" ]; then
    shift
    printf '%s' "$1" > "__MARKER__"
  fi
  shift
done
echo 'provider warning' >&2
"##
        .replace("__MARKER__", &root.join("stage-marker").to_string_lossy());
        executable(&dcmmkdir, &dcmmkdir_script);
        let dcmdump = tools.join("dcmdump");
        executable(
            &dcmdump,
            r##"#!/bin/sh
cat <<'DUMP'
(0002,0002) UI [1.2.840.10008.1.3.10]
(0002,0003) UI [1.2.826.0.1.3680043.10.543.8]
(0002,0010) UI [1.2.840.10008.1.2.1]
(0004,1130) CS [DTSMIXED]
(0004,1500) CS [IMAGE\IM000001]
(0004,1510) UI [1.2.840.10008.5.1.4.1.1.2.1]
(0004,1511) UI [1.2.826.0.1.3680043.10.543.1]
(0004,1500) CS [DERIVED\DR000001]
(0004,1510) UI [1.2.840.10008.5.1.4.1.1.66.4]
(0004,1511) UI [1.2.826.0.1.3680043.10.543.2]
(0004,1500) CS [NONIMAGE\NI000001]
(0004,1510) UI [1.2.840.10008.5.1.4.1.1.9.1.2]
(0004,1511) UI [1.2.826.0.1.3680043.10.543.3]
DUMP
"##,
        );
        let dciodvfy = tools.join("dciodvfy");
        executable(
            &dciodvfy,
            if failing_validator {
                "#!/bin/sh\necho invalid >&2\nexit 1\n"
            } else {
                "#!/bin/sh\nexit 0\n"
            },
        );
        let identities = [
            (
                MemberRole::Image,
                "1.2.840.10008.5.1.4.1.1.2.1",
                "1.2.826.0.1.3680043.10.543.1",
                Vec::new(),
            ),
            (
                MemberRole::Derived,
                "1.2.840.10008.5.1.4.1.1.66.4",
                "1.2.826.0.1.3680043.10.543.2",
                vec!["1.2.826.0.1.3680043.10.543.1"],
            ),
            (
                MemberRole::NonImage,
                "1.2.840.10008.5.1.4.1.1.9.1.2",
                "1.2.826.0.1.3680043.10.543.3",
                Vec::new(),
            ),
        ];
        let media_sources = identities
            .into_iter()
            .enumerate()
            .map(|(index, (role, class_uid, instance_uid, references))| {
                let source_path = sources.join(format!("source-{index}.dcm"));
                fs::write(&source_path, b"source").unwrap();
                MediaSourcePath {
                    source_path,
                    member: member(role, 1, class_uid, instance_uid, references.as_slice()),
                }
            })
            .collect();
        DicomDirRunRequest {
            tools: MediaToolPaths {
                dcmmkdir,
                dcmdump,
                dciodvfy,
                dcentvfy: None,
            },
            sources: media_sources,
            timeout: Duration::from_secs(2),
            staging_parent: Some(staging),
        }
    }

    #[cfg(unix)]
    #[test]
    fn runs_closed_file_set_and_removes_private_staging() {
        let root = fixture_root("success");
        let request = request(&root, false);
        let qualification = run_dicomdir_qualification(&request).unwrap();
        assert_eq!(qualification.determinism, MediaDeterminism::SemanticStable);
        assert_eq!(qualification.member_count, 3);
        assert_eq!(
            qualification.evidence.dcmtk_parser_same_family,
            CheckStatus::Passed
        );
        assert_eq!(
            qualification.evidence.dcm4che_independent_peer,
            CheckStatus::Unavailable
        );
        assert!(!qualification.independent_interoperability_proven);
        assert!(!qualification.is_promotable());
        assert_eq!(qualification.provider.provider_id, "dcmtk");
        assert_eq!(qualification.provider.version, "3.7.0");
        assert_eq!(qualification.provider_warnings, ["provider warning"]);
        if let Ok(staged) = fs::read_to_string(root.join("stage-marker")) {
            assert!(!Path::new(&staged).exists());
        }
        assert_eq!(fs::read_dir(root.join("staging")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn validator_failure_blocks_qualification_and_cleans_staging() {
        let root = fixture_root("failure");
        let request = request(&root, true);
        assert!(matches!(
            run_dicomdir_qualification(&request),
            Err(MediaRunnerError::ToolFailed {
                tool: "dciodvfy",
                ..
            })
        ));
        let staged = fs::read_to_string(root.join("stage-marker")).unwrap();
        assert!(!Path::new(&staged).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn source_hash_mismatch_prevents_provider_invocation_and_cleans_staging() {
        let root = fixture_root("hash-mismatch");
        let mut request = request(&root, false);
        request.sources[0].member.sha256 = "f".repeat(64);
        assert!(matches!(
            run_dicomdir_qualification(&request),
            Err(MediaRunnerError::SourceHashMismatch { .. })
        ));
        assert!(!root.join("stage-marker").exists());
        assert_eq!(fs::read_dir(root.join("staging")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn timed_out_parser_is_killed_and_staging_is_removed() {
        let root = fixture_root("timeout");
        let mut request = request(&root, false);
        executable(
            &request.tools.dcmdump,
            "#!/bin/sh\nsleep 2\necho should-not-complete\n",
        );
        request.timeout = Duration::from_millis(30);
        assert!(matches!(
            run_dicomdir_qualification(&request),
            Err(MediaRunnerError::ToolTimedOut { .. })
        ));
        if let Ok(staged) = fs::read_to_string(root.join("stage-marker")) {
            assert!(!Path::new(&staged).exists());
        }
        assert_eq!(fs::read_dir(root.join("staging")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_numeric_dcmdump_directory_records() {
        let dump = "\
(0002,0002) UI [1.2.840.10008.1.3.10]\n\
(0002,0003) UI [1.2.3]\n\
(0002,0010) UI [1.2.840.10008.1.2.1]\n\
(0004,1130) CS [DTSMIXED]\n\
(0004,1500) CS [IMAGE\\IM000001]\n\
(0004,1510) UI [1.2.4]\n\
(0004,1511) UI [1.2.5]\n";
        let parsed = ParsedDicomDir::parse(dump).unwrap();
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].file_id.display(), "IMAGE\\IM000001");
        assert_eq!(parsed.records[0].referenced_sop_instance_uid, "1.2.5");
    }
}
