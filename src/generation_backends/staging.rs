use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

use super::{BackendContractError, is_safe_relative_path};

#[derive(Debug, Clone, Copy)]
pub struct OutputLimits {
    pub max_output_files: usize,
    pub max_file_bytes: u64,
    pub max_total_output_bytes: u64,
}

pub fn verify_staged_outputs(
    response: &Value,
    output_root: &Path,
    limits: OutputLimits,
) -> Result<Vec<PathBuf>, BackendContractError> {
    let declared = response["outputs"]
        .as_array()
        .ok_or_else(|| invalid("response outputs must be an array"))?;
    if declared.len() > limits.max_output_files {
        return Err(invalid(format!(
            "response declares {} outputs, limit is {}",
            declared.len(),
            limits.max_output_files
        )));
    }

    let mut declared_paths = BTreeSet::new();
    for output in declared {
        let relative = PathBuf::from(
            output["relative_path"]
                .as_str()
                .ok_or_else(|| invalid("output relative_path must be a string"))?,
        );
        if !is_safe_relative_path(&relative) {
            return Err(invalid(format!(
                "output relative_path {} is unsafe",
                relative.display()
            )));
        }
        declared_paths.insert(relative);
    }

    let mut actual_paths = Vec::new();
    collect_regular_files(output_root, output_root, &mut actual_paths)?;
    actual_paths.sort();
    let actual_set = actual_paths.iter().cloned().collect::<BTreeSet<_>>();
    for path in actual_set.difference(&declared_paths) {
        return Err(invalid(format!(
            "backend created undeclared output {}",
            path.display()
        )));
    }
    for path in declared_paths.difference(&actual_set) {
        return Err(invalid(format!(
            "backend did not create declared output {}",
            path.display()
        )));
    }

    let mut total_bytes = 0u64;
    for output in declared {
        let relative = PathBuf::from(output["relative_path"].as_str().expect("checked path"));
        let path = output_root.join(&relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| BackendContractError::Read {
                path: path.clone(),
                source,
            })?;
        if metadata.len() > limits.max_file_bytes {
            return Err(invalid(format!(
                "output {} is {} bytes, per-file limit is {}",
                relative.display(),
                metadata.len(),
                limits.max_file_bytes
            )));
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| invalid("output byte count overflow"))?;
        if total_bytes > limits.max_total_output_bytes {
            return Err(invalid(format!(
                "outputs total {total_bytes} bytes, limit is {}",
                limits.max_total_output_bytes
            )));
        }
        verify_part10_identity(&path, output)?;
    }
    Ok(actual_paths)
}

pub fn promote_staged_outputs(
    output_root: &Path,
    destination_root: &Path,
) -> Result<(), BackendContractError> {
    if destination_root.exists() {
        return Err(invalid(format!(
            "promotion destination {} already exists",
            destination_root.display()
        )));
    }
    if let Some(parent) = destination_root.parent() {
        fs::create_dir_all(parent).map_err(|source| BackendContractError::Read {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::rename(output_root, destination_root).map_err(|source| BackendContractError::Read {
        path: output_root.to_path_buf(),
        source,
    })
}

fn collect_regular_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), BackendContractError> {
    for entry in fs::read_dir(directory).map_err(|source| BackendContractError::Read {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| BackendContractError::Read {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| BackendContractError::Read {
                path: path.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(invalid(format!(
                "staged output {} is a symbolic link",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_regular_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid(format!("output {} escaped staging root", path.display())))?;
            files.push(relative.to_path_buf());
        } else {
            return Err(invalid(format!(
                "staged output {} is not a regular file",
                path.display()
            )));
        }
    }
    Ok(())
}

fn verify_part10_identity(path: &Path, expected: &Value) -> Result<(), BackendContractError> {
    let object = open_file(path).map_err(|error| {
        invalid(format!(
            "reopen staged Part 10 file {}: {error}",
            path.display()
        ))
    })?;
    let expected_sop_class = expected["sop_class_uid"]
        .as_str()
        .ok_or_else(|| invalid("output SOP Class UID must be a string"))?;
    let expected_sop_instance = expected["sop_instance_uid"]
        .as_str()
        .ok_or_else(|| invalid("output SOP Instance UID must be a string"))?;
    let expected_transfer_syntax = expected["transfer_syntax_uid"]
        .as_str()
        .ok_or_else(|| invalid("output Transfer Syntax UID must be a string"))?;

    compare_uid(
        path,
        "File Meta SOP Class UID",
        object.meta().media_storage_sop_class_uid(),
        expected_sop_class,
    )?;
    compare_uid(
        path,
        "File Meta SOP Instance UID",
        object.meta().media_storage_sop_instance_uid(),
        expected_sop_instance,
    )?;
    compare_uid(
        path,
        "File Meta Transfer Syntax UID",
        object.meta().transfer_syntax(),
        expected_transfer_syntax,
    )?;
    let dataset_sop_class = object
        .element(tags::SOP_CLASS_UID)
        .map_err(|error| invalid(format!("read dataset SOP Class UID: {error}")))?
        .to_str()
        .map_err(|error| invalid(format!("decode dataset SOP Class UID: {error}")))?;
    let dataset_sop_instance = object
        .element(tags::SOP_INSTANCE_UID)
        .map_err(|error| invalid(format!("read dataset SOP Instance UID: {error}")))?
        .to_str()
        .map_err(|error| invalid(format!("decode dataset SOP Instance UID: {error}")))?;
    compare_uid(
        path,
        "dataset SOP Class UID",
        &dataset_sop_class,
        expected_sop_class,
    )?;
    compare_uid(
        path,
        "dataset SOP Instance UID",
        &dataset_sop_instance,
        expected_sop_instance,
    )
}

fn compare_uid(
    path: &Path,
    label: &str,
    actual: &str,
    expected: &str,
) -> Result<(), BackendContractError> {
    let actual = actual.trim_end_matches(['\0', ' ']);
    if actual == expected {
        Ok(())
    } else {
        Err(invalid(format!(
            "{} {label} is {actual}, expected {expected}",
            path.display()
        )))
    }
}

fn invalid(message: impl Into<String>) -> BackendContractError {
    BackendContractError::Invalid {
        label: "generation backend staging".to_string(),
        problems: vec![message.into()],
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn undeclared_regular_output_is_rejected() {
        let root = temporary_directory("undeclared");
        fs::write(root.join("rogue.dcm"), b"not trusted").expect("write rogue output");
        let error = verify_staged_outputs(&json!({"outputs": []}), &root, limits())
            .expect_err("undeclared output must fail");
        assert!(error.to_string().contains("undeclared"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[cfg(unix)]
    #[test]
    fn staged_symbolic_link_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory("symlink");
        symlink("missing-target", root.join("linked.dcm")).expect("create staged symlink");
        let error = verify_staged_outputs(&json!({"outputs": []}), &root, limits())
            .expect_err("staged symlink must fail");
        assert!(error.to_string().contains("symbolic link"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    fn limits() -> OutputLimits {
        OutputLimits {
            max_output_files: 8,
            max_file_bytes: 1024,
            max_total_output_bytes: 4096,
        }
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dts-staging-{label}-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create staging fixture");
        path
    }
}
