//! Select the deterministic mixed-object input set for DICOMDIR qualification.

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::media::{FileId, FileSetMember, MemberRole};
use crate::media_runner::MediaSourcePath;

pub const MEDIA_IMAGE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
pub const MEDIA_DERIVED_CASE_ID: &str = "derived/seg/binary_multiframe_explicit_le";
pub const MEDIA_NON_IMAGE_CASE_ID: &str = "non-image/waveform/general_ecg";

/// Load the locked image, derived, and non-image representatives from a
/// generated corpus manifest. The returned order and File IDs are stable.
pub fn load_mixed_media_sources(
    generated_root: impl AsRef<Path>,
) -> Result<Vec<MediaSourcePath>, MediaSourceError> {
    let root = generated_root.as_ref();
    let manifest_path = root.join("manifest.json");
    let bytes = fs::read(&manifest_path).map_err(|source| MediaSourceError::Read {
        path: manifest_path.clone(),
        source,
    })?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(MediaSourceError::Parse)?;
    let files = manifest
        .get("files")
        .and_then(Value::as_array)
        .ok_or(MediaSourceError::InvalidManifest("files must be an array"))?;

    let specifications = [
        (MEDIA_IMAGE_CASE_ID, MemberRole::Image, 1),
        (MEDIA_DERIVED_CASE_ID, MemberRole::Derived, 1),
        (MEDIA_NON_IMAGE_CASE_ID, MemberRole::NonImage, 1),
    ];
    let mut selected = Vec::with_capacity(specifications.len());
    for (case_id, role, ordinal) in specifications {
        let matches = files
            .iter()
            .filter(|file| file.get("case_id").and_then(Value::as_str) == Some(case_id))
            .collect::<Vec<_>>();
        let [file] = matches.as_slice() else {
            return Err(MediaSourceError::CaseCardinality {
                case_id,
                actual: matches.len(),
            });
        };
        let relative_path = required_string(file, "path")?;
        let relative_path = safe_relative_path(relative_path)?;
        let sha256 = required_string(file, "sha256")?.to_owned();
        let sop_class_uid = file
            .pointer("/dicom/sop_class_uid")
            .and_then(Value::as_str)
            .ok_or(MediaSourceError::InvalidManifest(
                "selected file dicom.sop_class_uid must be a string",
            ))?
            .to_owned();
        let sop_instance_uid = file
            .pointer("/uids/sop_instance_uid")
            .and_then(Value::as_str)
            .ok_or(MediaSourceError::InvalidManifest(
                "selected file uids.sop_instance_uid must be a string",
            ))?
            .to_owned();
        let referenced_sop_instance_uids = file
            .get("references")
            .and_then(Value::as_array)
            .ok_or(MediaSourceError::InvalidManifest(
                "selected file references must be an array",
            ))?
            .iter()
            .map(|reference| {
                reference
                    .get("sop_instance_uid")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or(MediaSourceError::InvalidManifest(
                        "reference sop_instance_uid must be a string",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        selected.push(MediaSourcePath {
            source_path: root.join(relative_path),
            member: FileSetMember {
                case_id: case_id.to_owned(),
                role,
                file_id: FileId::for_member(role, ordinal)?,
                sha256,
                sop_class_uid,
                sop_instance_uid,
                referenced_sop_instance_uids,
            },
        });
    }

    let source_uid = &selected[0].member.sop_instance_uid;
    if selected[1].member.referenced_sop_instance_uids.len() != 1
        || selected[1].member.referenced_sop_instance_uids[0] != *source_uid
    {
        return Err(MediaSourceError::ReferenceMismatch);
    }
    if !selected[2].member.referenced_sop_instance_uids.is_empty() {
        return Err(MediaSourceError::ReferenceMismatch);
    }
    Ok(selected)
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, MediaSourceError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(MediaSourceError::InvalidManifest(
            "selected file field must be a string",
        ))
}

fn safe_relative_path(value: &str) -> Result<PathBuf, MediaSourceError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MediaSourceError::UnsafePath(value.to_owned()));
    }
    Ok(path.to_path_buf())
}

#[derive(Debug)]
pub enum MediaSourceError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse(serde_json::Error),
    InvalidManifest(&'static str),
    CaseCardinality {
        case_id: &'static str,
        actual: usize,
    },
    UnsafePath(String),
    ReferenceMismatch,
    Media(crate::media::MediaError),
}

impl fmt::Display for MediaSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(formatter, "read {}: {source}", path.display()),
            Self::Parse(source) => write!(formatter, "parse manifest: {source}"),
            Self::InvalidManifest(message) => write!(formatter, "invalid manifest: {message}"),
            Self::CaseCardinality { case_id, actual } => write!(
                formatter,
                "manifest must contain exactly one {case_id} file; found {actual}"
            ),
            Self::UnsafePath(path) => write!(formatter, "unsafe manifest path: {path}"),
            Self::ReferenceMismatch => write!(
                formatter,
                "locked media references must contain only SEG to Enhanced CT"
            ),
            Self::Media(source) => source.fmt(formatter),
        }
    }
}

impl Error for MediaSourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
            Self::Media(source) => Some(source),
            _ => None,
        }
    }
}

impl From<crate::media::MediaError> for MediaSourceError {
    fn from(source: crate::media::MediaError) -> Self {
        Self::Media(source)
    }
}
