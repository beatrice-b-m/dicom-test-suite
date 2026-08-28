use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

use super::{BackendContractError, is_safe_relative_path};
use crate::sha256_hex;

#[derive(Debug, Clone, Copy)]
pub struct OutputLimits {
    pub max_output_files: usize,
    pub max_file_bytes: u64,
    pub max_total_output_bytes: u64,
}

pub fn stage_declared_sources(
    request: &Value,
    input_root: &Path,
    staging_input_root: &Path,
) -> Result<Vec<PathBuf>, BackendContractError> {
    verify_directory_without_symlinks(input_root, "input root")?;
    verify_directory_without_symlinks(staging_input_root, "staging input root")?;

    let sources = request["sources"]
        .as_array()
        .ok_or_else(|| invalid("request sources must be an array"))?;
    let mut relative_paths = BTreeSet::new();
    let mut sop_instance_uids = BTreeSet::new();
    let mut staged = Vec::with_capacity(sources.len());

    for source in sources {
        let relative = PathBuf::from(
            source["relative_path"]
                .as_str()
                .ok_or_else(|| invalid("source relative_path must be a string"))?,
        );
        if !is_safe_relative_path(&relative) {
            return Err(invalid(format!(
                "source relative_path {} is unsafe",
                relative.display()
            )));
        }
        if !relative_paths.insert(relative.clone()) {
            return Err(invalid(format!(
                "request declares source path {} more than once",
                relative.display()
            )));
        }
        let sop_instance_uid = source["sop_instance_uid"]
            .as_str()
            .ok_or_else(|| invalid("source SOP Instance UID must be a string"))?;
        if !sop_instance_uids.insert(sop_instance_uid.to_string()) {
            return Err(invalid(format!(
                "request declares SOP Instance UID {sop_instance_uid} more than once"
            )));
        }

        let source_path = verify_regular_path_without_symlinks(input_root, &relative)?;
        let source_bytes = fs::read(&source_path).map_err(|source| BackendContractError::Read {
            path: source_path.clone(),
            source,
        })?;
        let expected_hash = source["sha256"]
            .as_str()
            .ok_or_else(|| invalid("source sha256 must be a string"))?;
        let actual_hash = sha256_hex(&source_bytes);
        if actual_hash != expected_hash {
            return Err(invalid(format!(
                "source {} sha256 is {actual_hash}, expected {expected_hash}",
                relative.display()
            )));
        }
        verify_source_part10_identity(&source_path, source)?;

        let destination = staging_input_root.join(&relative);
        let parent = destination
            .parent()
            .ok_or_else(|| invalid("staged source must have a parent directory"))?;
        fs::create_dir_all(parent).map_err(|source| BackendContractError::Read {
            path: parent.to_path_buf(),
            source,
        })?;
        let mut input = File::open(&source_path).map_err(|source| BackendContractError::Read {
            path: source_path.clone(),
            source,
        })?;
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|source| BackendContractError::Read {
                path: destination.clone(),
                source,
            })?;
        io::copy(&mut input, &mut output).map_err(|source| BackendContractError::Read {
            path: destination.clone(),
            source,
        })?;
        let mut permissions = fs::metadata(&destination)
            .map_err(|source| BackendContractError::Read {
                path: destination.clone(),
                source,
            })?
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&destination, permissions).map_err(|source| {
            BackendContractError::Read {
                path: destination.clone(),
                source,
            }
        })?;
        staged.push(relative);
    }

    staged.sort();
    Ok(staged)
}

pub fn verify_staged_outputs(
    request: &Value,
    response: &Value,
    output_root: &Path,
    limits: OutputLimits,
) -> Result<Vec<PathBuf>, BackendContractError> {
    verify_directory_without_symlinks(output_root, "output root")?;
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
        verify_part10_identity(&path, output, &request["identities"])?;
    }
    Ok(actual_paths)
}

pub fn promote_staged_outputs(
    output_root: &Path,
    destination_root: &Path,
) -> Result<(), BackendContractError> {
    verify_directory_without_symlinks(output_root, "promotion source root")?;
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

fn verify_directory_without_symlinks(path: &Path, label: &str) -> Result<(), BackendContractError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| BackendContractError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid(format!(
            "{label} {} must be a directory and not a symbolic link",
            path.display()
        )));
    }
    Ok(())
}

fn verify_regular_path_without_symlinks(
    root: &Path,
    relative: &Path,
) -> Result<PathBuf, BackendContractError> {
    let mut current = root.to_path_buf();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|source| BackendContractError::Read {
                path: current.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            return Err(invalid(format!(
                "declared source {} traverses symbolic link {}",
                relative.display(),
                current.display()
            )));
        }
        let is_last = index + 1 == component_count;
        if is_last && !metadata.is_file() {
            return Err(invalid(format!(
                "declared source {} is not a regular file",
                relative.display()
            )));
        }
        if !is_last && !metadata.is_dir() {
            return Err(invalid(format!(
                "declared source {} has non-directory ancestor {}",
                relative.display(),
                current.display()
            )));
        }
    }
    Ok(current)
}

fn verify_source_part10_identity(
    path: &Path,
    expected: &Value,
) -> Result<(), BackendContractError> {
    let object = open_file(path).map_err(|error| {
        invalid(format!(
            "reopen declared source Part 10 file {}: {error}",
            path.display()
        ))
    })?;
    let expected_sop_class = expected["sop_class_uid"]
        .as_str()
        .ok_or_else(|| invalid("source SOP Class UID must be a string"))?;
    let expected_sop_instance = expected["sop_instance_uid"]
        .as_str()
        .ok_or_else(|| invalid("source SOP Instance UID must be a string"))?;

    compare_uid(
        path,
        "source File Meta SOP Class UID",
        object.meta().media_storage_sop_class_uid(),
        expected_sop_class,
    )?;
    compare_uid(
        path,
        "source File Meta SOP Instance UID",
        object.meta().media_storage_sop_instance_uid(),
        expected_sop_instance,
    )?;
    let dataset_sop_class = required_dataset_uid(&object, tags::SOP_CLASS_UID, "SOP Class UID")?;
    let dataset_sop_instance =
        required_dataset_uid(&object, tags::SOP_INSTANCE_UID, "SOP Instance UID")?;
    compare_uid(
        path,
        "source dataset SOP Class UID",
        &dataset_sop_class,
        expected_sop_class,
    )?;
    compare_uid(
        path,
        "source dataset SOP Instance UID",
        &dataset_sop_instance,
        expected_sop_instance,
    )?;

    let expected_series = expected["series_instance_uid"].as_str();
    let actual_series = object
        .element(tags::SERIES_INSTANCE_UID)
        .ok()
        .map(|element| {
            element
                .to_str()
                .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
                .map_err(|error| invalid(format!("decode source Series Instance UID: {error}")))
        })
        .transpose()?;
    if actual_series.as_deref() != expected_series {
        return Err(invalid(format!(
            "{} source Series Instance UID is {}, expected {}",
            path.display(),
            actual_series.as_deref().unwrap_or("absent"),
            expected_series.unwrap_or("absent")
        )));
    }

    if let Some(frame_numbers) = expected["frame_numbers"].as_array() {
        let number_of_frames = object
            .element(tags::NUMBER_OF_FRAMES)
            .ok()
            .map(|element| {
                element
                    .to_int::<u64>()
                    .map_err(|error| invalid(format!("decode source Number of Frames: {error}")))
            })
            .transpose()?
            .unwrap_or(1);
        for frame in frame_numbers {
            let frame = frame.as_u64().expect("request schema checked frame number");
            if frame > number_of_frames {
                return Err(invalid(format!(
                    "{} source frame {frame} exceeds Number of Frames {number_of_frames}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn required_dataset_uid(
    object: &dicom_object::DefaultDicomObject,
    tag: dicom_core::Tag,
    label: &str,
) -> Result<String, BackendContractError> {
    object
        .element(tag)
        .map_err(|error| invalid(format!("read source dataset {label}: {error}")))?
        .to_str()
        .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
        .map_err(|error| invalid(format!("decode source dataset {label}: {error}")))
}

fn verify_part10_identity(
    path: &Path,
    expected: &Value,
    identities: &Value,
) -> Result<(), BackendContractError> {
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
    )?;

    for (tag, label, field) in [
        (tags::STUDY_INSTANCE_UID, "Study Instance UID", "study_instance_uid"),
        (tags::SERIES_INSTANCE_UID, "Series Instance UID", "series_instance_uid"),
    ] {
        let expected_uid = identities[field]
            .as_str()
            .ok_or_else(|| invalid(format!("request {label} must be a string")))?;
        let actual_uid = required_dataset_uid(&object, tag, label)?;
        compare_uid(path, &format!("dataset {label}"), &actual_uid, expected_uid)?;
    }

    let expected_frame_of_reference = identities["frame_of_reference_uid"].as_str();
    let actual_frame_of_reference = object
        .element(tags::FRAME_OF_REFERENCE_UID)
        .ok()
        .map(|element| {
            element
                .to_str()
                .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
                .map_err(|error| invalid(format!("decode Frame of Reference UID: {error}")))
        })
        .transpose()?;
    if actual_frame_of_reference.as_deref() != expected_frame_of_reference {
        return Err(invalid(format!(
            "{} dataset Frame of Reference UID is {}, expected {}",
            path.display(),
            actual_frame_of_reference.as_deref().unwrap_or("absent"),
            expected_frame_of_reference.unwrap_or("absent")
        )));
    }
    Ok(())
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

    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_dictionary_std::{tags, uids};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
    use serde_json::json;

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn declared_source_is_verified_and_copied_read_only() {
        let root = temporary_directory("source-valid");
        let input_root = root.join("generated");
        let staging_root = root.join("staged");
        fs::create_dir_all(input_root.join("nested")).expect("create generated root");
        fs::create_dir(&staging_root).expect("create staging input root");
        let source_path = input_root.join("nested/source.dcm");
        write_source_dicom(&source_path);
        let mut source = source_declaration(&source_path);
        source["relative_path"] = json!("nested/source.dcm");
        let request = request_with_sources(vec![source]);

        let staged = stage_declared_sources(&request, &input_root, &staging_root)
            .expect("valid declared source should stage");

        assert_eq!(staged, vec![PathBuf::from("nested/source.dcm")]);
        let staged_path = staging_root.join("nested/source.dcm");
        assert_eq!(
            fs::read(&staged_path).expect("read staged source"),
            fs::read(&source_path).expect("read original source")
        );
        assert!(
            fs::metadata(staged_path)
                .expect("staged source metadata")
                .permissions()
                .readonly()
        );
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[test]
    fn declared_source_hash_and_dicom_identity_mismatches_are_rejected() {
        let root = temporary_directory("source-mismatch");
        let input_root = root.join("generated");
        let staging_root = root.join("staged");
        fs::create_dir(&input_root).expect("create generated root");
        fs::create_dir(&staging_root).expect("create staging input root");
        let source_path = input_root.join("source.dcm");
        write_source_dicom(&source_path);

        let mut wrong_hash = source_declaration(&source_path);
        wrong_hash["sha256"] = json!("f".repeat(64));
        let error = stage_declared_sources(
            &request_with_sources(vec![wrong_hash]),
            &input_root,
            &staging_root,
        )
        .expect_err("source hash mismatch must fail");
        assert!(error.to_string().contains("sha256"));

        let mut wrong_identity = source_declaration(&source_path);
        wrong_identity["sop_instance_uid"] = json!("1.2.826.0.1.3680043.10.543.999");
        let error = stage_declared_sources(
            &request_with_sources(vec![wrong_identity]),
            &input_root,
            &staging_root,
        )
        .expect_err("source identity mismatch must fail");
        assert!(error.to_string().contains("SOP Instance UID"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[test]
    fn duplicate_source_path_or_sop_instance_is_rejected_as_ambiguous() {
        let root = temporary_directory("source-duplicate");
        let input_root = root.join("generated");
        let staging_root = root.join("staged");
        fs::create_dir(&input_root).expect("create generated root");
        fs::create_dir(&staging_root).expect("create staging input root");
        let source_path = input_root.join("source.dcm");
        write_source_dicom(&source_path);
        let source = source_declaration(&source_path);

        let error = stage_declared_sources(
            &request_with_sources(vec![source.clone(), source]),
            &input_root,
            &staging_root,
        )
        .expect_err("duplicate source declaration must fail");
        assert!(error.to_string().contains("more than once"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[cfg(unix)]
    #[test]
    fn declared_source_symbolic_link_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory("source-symlink");
        let input_root = root.join("generated");
        let staging_root = root.join("staged");
        fs::create_dir(&input_root).expect("create generated root");
        fs::create_dir(&staging_root).expect("create staging input root");
        let real_path = root.join("real.dcm");
        write_source_dicom(&real_path);
        let source_path = input_root.join("source.dcm");
        symlink(&real_path, &source_path).expect("create source symlink");
        let request = request_with_sources(vec![source_declaration(&real_path)]);
        let mut source = request["sources"][0].clone();
        source["relative_path"] = json!("source.dcm");

        let error = stage_declared_sources(
            &request_with_sources(vec![source]),
            &input_root,
            &staging_root,
        )
        .expect_err("source symlink must fail");
        assert!(error.to_string().contains("symbolic link"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[test]
    fn undeclared_regular_output_is_rejected() {
        let root = temporary_directory("undeclared");
        fs::write(root.join("rogue.dcm"), b"not trusted").expect("write rogue output");
        let error = verify_staged_outputs(
            &request_identities(),
            &json!({"outputs": []}),
            &root,
            limits(),
        )
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
        let error = verify_staged_outputs(
            &request_identities(),
            &json!({"outputs": []}),
            &root,
            limits(),
        )
            .expect_err("staged symlink must fail");
        assert!(error.to_string().contains("symbolic link"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[cfg(unix)]
    #[test]
    fn staged_output_root_symbolic_link_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = temporary_directory("root-symlink");
        let external = root.join("external");
        fs::create_dir(&external).expect("create external output directory");
        let output_root = root.join("outputs");
        symlink(&external, &output_root).expect("create output-root symlink");

        let error = verify_staged_outputs(
            &request_identities(),
            &json!({"outputs": []}),
            &output_root,
            limits(),
        )
            .expect_err("output-root symlink must fail verification");
        assert!(error.to_string().contains("output root"));

        let destination = root.join("promoted");
        let error = promote_staged_outputs(&output_root, &destination)
            .expect_err("output-root symlink must fail promotion");
        assert!(error.to_string().contains("promotion source root"));
        assert!(!destination.exists());
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    #[test]
    fn staged_output_must_match_prederived_study_series_and_frame_of_reference() {
        let root = temporary_directory("output-identities");
        let output_path = root.join("output.dcm");
        write_source_dicom(&output_path);
        let response = json!({
            "outputs": [{
                "relative_path": "output.dcm",
                "sop_class_uid": uids::CT_IMAGE_STORAGE,
                "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
                "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
                "references": [],
                "expected_semantics": {},
                "payload_expectations": {}
            }]
        });

        verify_staged_outputs(&request_identities(), &response, &root, limits())
            .expect("matching output identities should pass");

        let mut wrong_series = request_identities();
        wrong_series["identities"]["series_instance_uid"] =
            json!("1.2.826.0.1.3680043.10.543.999");
        let error = verify_staged_outputs(&wrong_series, &response, &root, limits())
            .expect_err("different Series Instance UID must fail");
        assert!(error.to_string().contains("Series Instance UID"));
        fs::remove_dir_all(root).expect("remove staging fixture");
    }

    fn limits() -> OutputLimits {
        OutputLimits {
            max_output_files: 8,
            max_file_bytes: 1024,
            max_total_output_bytes: 4096,
        }
    }

    fn request_with_sources(sources: Vec<Value>) -> Value {
        json!({"sources": sources})
    }

    fn request_identities() -> Value {
        json!({
            "identities": {
                "study_instance_uid": "1.2.826.0.1.3680043.10.543.1",
                "series_instance_uid": "1.2.826.0.1.3680043.10.543.2",
                "frame_of_reference_uid": null
            }
        })
    }

    fn source_declaration(source_path: &Path) -> Value {
        json!({
            "role": "source_image",
            "source_case_id": "geometry/ct/source",
            "relative_path": source_path.file_name().unwrap().to_str().unwrap(),
            "sha256": sha256_hex(&fs::read(source_path).expect("read source fixture")),
            "sop_class_uid": uids::CT_IMAGE_STORAGE,
            "sop_instance_uid": "1.2.826.0.1.3680043.10.543.4",
            "series_instance_uid": "1.2.826.0.1.3680043.10.543.2",
            "frame_numbers": null
        })
    }

    fn write_source_dicom(path: &Path) {
        let mut object = InMemDicomObject::new_empty();
        for (tag, value) in [
            (tags::SOP_CLASS_UID, uids::CT_IMAGE_STORAGE),
            (tags::SOP_INSTANCE_UID, "1.2.826.0.1.3680043.10.543.4"),
            (tags::STUDY_INSTANCE_UID, "1.2.826.0.1.3680043.10.543.1"),
            (tags::SERIES_INSTANCE_UID, "1.2.826.0.1.3680043.10.543.2"),
        ] {
            object.put(DataElement::new(tag, VR::UI, PrimitiveValue::from(value)));
        }
        object
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                    .implementation_class_uid("1.2.826.0.1.3680043.10.543.9"),
            )
            .expect("create source file meta")
            .write_to_file(path)
            .expect("write source DICOM fixture");
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
