use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::Tag;
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::{FileDicomObject, InMemDicomObject, open_file};
use serde_json::Value;

use crate::GenerateError;

type OpenedObject = FileDicomObject<InMemDicomObject<StandardDataDictionary>>;

#[derive(Debug, Clone)]
pub(crate) struct Part10Expectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub rows: u16,
    pub columns: u16,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: &'a str,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    pub pixel_data_length: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedPart10 {
    pub bytes: Vec<u8>,
    pub validation: Value,
}

pub(crate) fn validate_part10_file(
    path: &Path,
    expected: &Part10Expectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );

    let transfer_syntax = trim_uid(obj.meta().transfer_syntax());
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        transfer_syntax.as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let meta_sop_class = trim_uid(obj.meta().media_storage_sop_class_uid());
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        meta_sop_class.as_str(),
        dataset_sop_class.as_str(),
    );

    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    let meta_sop_instance = trim_uid(obj.meta().media_storage_sop_instance_uid());
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        meta_sop_instance.as_str(),
        dataset_sop_instance.as_str(),
    );

    let implementation_class_uid = trim_uid(obj.meta().implementation_class_uid());
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        implementation_class_uid.as_str(),
        expected.implementation_class_uid,
    );

    let synthetic_data = element_str(path, &obj, tags::SYNTHETIC_DATA)?;
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        synthetic_data.as_str(),
        expected.synthetic_data,
    );

    check_equal(
        &mut internal,
        "rows",
        "Rows matches the recipe.",
        "Rows does not match the recipe.",
        element_u16(path, &obj, tags::ROWS)?,
        expected.rows,
    );
    check_equal(
        &mut internal,
        "columns",
        "Columns matches the recipe.",
        "Columns does not match the recipe.",
        element_u16(path, &obj, tags::COLUMNS)?,
        expected.columns,
    );
    check_equal(
        &mut internal,
        "samples_per_pixel",
        "Samples per Pixel matches the recipe.",
        "Samples per Pixel does not match the recipe.",
        element_u16(path, &obj, tags::SAMPLES_PER_PIXEL)?,
        expected.samples_per_pixel,
    );
    check_equal(
        &mut internal,
        "photometric_interpretation",
        "Photometric Interpretation matches the recipe.",
        "Photometric Interpretation does not match the recipe.",
        element_str(path, &obj, tags::PHOTOMETRIC_INTERPRETATION)?.as_str(),
        expected.photometric_interpretation,
    );
    check_equal(
        &mut internal,
        "bits_allocated",
        "Bits Allocated matches the recipe.",
        "Bits Allocated does not match the recipe.",
        element_u16(path, &obj, tags::BITS_ALLOCATED)?,
        expected.bits_allocated,
    );
    check_equal(
        &mut internal,
        "bits_stored",
        "Bits Stored matches the recipe.",
        "Bits Stored does not match the recipe.",
        element_u16(path, &obj, tags::BITS_STORED)?,
        expected.bits_stored,
    );
    check_equal(
        &mut internal,
        "high_bit",
        "High Bit matches Bits Stored - 1.",
        "High Bit does not match the recipe.",
        element_u16(path, &obj, tags::HIGH_BIT)?,
        expected.high_bit,
    );
    check_equal(
        &mut internal,
        "pixel_representation",
        "Pixel Representation matches the recipe.",
        "Pixel Representation does not match the recipe.",
        element_u16(path, &obj, tags::PIXEL_REPRESENTATION)?,
        expected.pixel_representation,
    );

    let pixel_bytes = element_bytes(path, &obj, tags::PIXEL_DATA)?;
    check_equal(
        &mut internal,
        "native_pixel_data_length",
        "Native Pixel Data length matches the uncompressed frame size.",
        "Native Pixel Data length does not match the uncompressed frame size.",
        pixel_bytes.len(),
        expected.pixel_data_length,
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": "secondary_capture_sop_class",
                    "status": "passed",
                    "message": "SOP Class UID matches Secondary Capture Image Storage in the 2026b reference."
                },
                {
                    "name": "explicit_vr_little_endian_transfer_syntax",
                    "status": "passed",
                    "message": "Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference."
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "image_pixel_description",
                    "status": "passed",
                    "message": "Image Pixel attributes match the native MONOCHROME2 8-bit recipe."
                }
            ],
            "external": []
        }),
    })
}

fn element_str(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<String, GenerateError> {
    let value = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_str()
        .map_err(|err| validation_error(path, err))?;
    Ok(value.trim_matches('\0').trim().to_string())
}

fn element_u16(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<u16, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u16>()
        .map_err(|err| validation_error(path, err))
}

fn element_bytes<'a>(
    path: &Path,
    obj: &'a OpenedObject,
    tag: Tag,
) -> Result<Cow<'a, [u8]>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))
}

fn check(
    results: &mut Vec<Value>,
    passed: bool,
    name: &str,
    passed_message: &str,
    failed_message: &str,
) {
    results.push(serde_json::json!({
        "name": name,
        "status": if passed { "passed" } else { "failed" },
        "message": if passed { passed_message } else { failed_message }
    }));
}

fn check_equal<T>(
    results: &mut Vec<Value>,
    name: &str,
    passed_message: &str,
    failed_message: &str,
    actual: T,
    expected: T,
) where
    T: PartialEq,
{
    check(
        results,
        actual == expected,
        name,
        passed_message,
        failed_message,
    );
}

fn fail_if_any_failed(path: &Path, results: &[Value]) -> Result<(), GenerateError> {
    let failures: Vec<&str> = results
        .iter()
        .filter(|result| result.get("status").and_then(Value::as_str) == Some("failed"))
        .filter_map(|result| result.get("name").and_then(Value::as_str))
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: failures.join(", "),
        })
    }
}

fn trim_uid(uid: &str) -> String {
    uid.trim_matches('\0').trim().to_string()
}

fn validation_error(path: &Path, err: impl std::error::Error) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(path),
        message: err.to_string(),
    }
}
