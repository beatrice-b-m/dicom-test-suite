use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::VR;
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::{FileDicomObject, InMemDicomObject, open_file};
use serde_json::Value;

use crate::codecs::{
    DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID, FrameDecodeInput, FrameDecoder,
    HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID, JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID,
    JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID, JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID,
    JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID, JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID,
    JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID, NativeRleLosslessEncoder,
    RLE_LOSSLESS_TRANSFER_SYNTAX_UID,
};

#[cfg(feature = "deflate")]
use crate::codecs::DicomRsDeflatedImageFrameEncoder;
#[cfg(feature = "jpeg")]
use crate::codecs::DicomRsJpegBaselineEncoder;
#[cfg(feature = "charls")]
use crate::codecs::DicomRsJpegLsLosslessEncoder;
#[cfg(feature = "jpegxl")]
use crate::codecs::DicomRsJpegXlLosslessEncoder;
#[cfg(feature = "jpeg2000")]
use crate::codecs::OpenJp2Jpeg2000LosslessEncoder;
#[cfg(feature = "htj2k_openjph")]
use crate::codecs::OpenJphHtj2kLosslessEncoder;
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_encoding::{Codec, adapters::PixelDataReader};
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_transfer_syntax_registry::entries::{
    JPEG_LOSSLESS_NON_HIERARCHICAL, JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION,
};

pub mod codecs;
pub mod encapsulation;
mod generator;
pub mod uid;
mod validation;
pub use uid::{DeterministicUidInput, UidRole, deterministic_uid};

type OpenedObject = FileDicomObject<InMemDicomObject<StandardDataDictionary>>;
type DatasetObject = InMemDicomObject<StandardDataDictionary>;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RUSTC_VERSION: &str = env!("DICOM_TEST_SUITE_RUSTC_VERSION");
pub const TARGET_TRIPLE: &str = env!("DICOM_TEST_SUITE_TARGET");
pub(crate) const IMPLEMENTATION_VERSION_NAME: &str = "DICOMTS010";

pub fn version_banner() -> String {
    format!("{PACKAGE_NAME} {PACKAGE_VERSION}")
}

pub const SUPPORTED_PROFILES: &[&str] = &[
    "smoke", "core", "extended", "legacy", "stress", "all", "negative", "fuzz",
];
pub const SUPPORTED_CASE_STATUSES: &[&str] =
    &["planned", "implemented", "skipped", "blocked", "deprecated"];
pub(crate) const ACTIVE_FEATURE_FLAGS: &[&str] = &[
    #[cfg(feature = "deflate")]
    "deflate",
    #[cfg(feature = "charls")]
    "charls",
    #[cfg(feature = "jpeg")]
    "jpeg",
    #[cfg(feature = "jpegxl")]
    "jpegxl",
    #[cfg(feature = "jpeg2000")]
    "jpeg2000",
    #[cfg(feature = "htj2k_openjph")]
    "htj2k_openjph",
    #[cfg(feature = "legacy_jpeg_dcmtk")]
    "legacy_jpeg_dcmtk",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateOptions {
    pub profile: String,
    pub out_dir: PathBuf,
    pub seed: u64,
    pub include_stress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedGenerationRun {
    pub profile: String,
    pub out_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub seed: u64,
    pub include_stress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationSummary {
    pub files_written: usize,
    pub manifest_written: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationSummary {
    pub manifest_path: PathBuf,
    pub files_checked: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone)]
struct ManifestSourceObject {
    case_id: String,
    sop_class_uid: String,
    sop_instance_uid: String,
    series_instance_uid: Option<String>,
    frames: Option<u64>,
}

const TAG_SEGMENTATION_TYPE: dicom_core::Tag = dicom_core::Tag(0x0062, 0x0001);
const TAG_SEGMENT_SEQUENCE: dicom_core::Tag = dicom_core::Tag(0x0062, 0x0002);
const TAG_SEGMENT_IDENTIFICATION_SEQUENCE: dicom_core::Tag = dicom_core::Tag(0x0062, 0x000A);
const TAG_REFERENCED_SEGMENT_NUMBER: dicom_core::Tag = dicom_core::Tag(0x0062, 0x000B);
const TAG_MAXIMUM_FRACTIONAL_VALUE: dicom_core::Tag = dicom_core::Tag(0x0062, 0x000E);
const TAG_SEGMENTATION_FRACTIONAL_TYPE: dicom_core::Tag = dicom_core::Tag(0x0062, 0x0010);
const TAG_REFERENCED_SOP_CLASS_UID: dicom_core::Tag = dicom_core::Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: dicom_core::Tag = dicom_core::Tag(0x0008, 0x1155);
const TAG_REFERENCED_IMAGE_SEQUENCE: dicom_core::Tag = dicom_core::Tag(0x0008, 0x1140);
const TAG_SOURCE_IMAGE_SEQUENCE: dicom_core::Tag = dicom_core::Tag(0x0008, 0x2112);
const TAG_DERIVATION_IMAGE_SEQUENCE: dicom_core::Tag = dicom_core::Tag(0x0008, 0x9124);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: dicom_core::Tag = dicom_core::Tag(0x300C, 0x0060);

#[derive(Debug)]
pub struct StandardsLockSummary {
    pub path: PathBuf,
    pub schema_version: String,
    pub dicom_base_edition: String,
    pub include_final_text_after_base: bool,
    pub kb_repository: String,
    pub kb_db_edition: String,
    pub kb_source_manifest_sha256: String,
    pub source_artifacts: usize,
    pub verification_queries: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum StandardsError {
    ReadMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseMetadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    MetadataShape {
        path: PathBuf,
        message: String,
    },
}

#[derive(Debug)]
pub enum ReportError {
    ReadMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseMetadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    MetadataShape {
        path: PathBuf,
        message: &'static str,
    },
}

#[derive(Debug)]
pub enum GenerateError {
    InvalidProfile(String),
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseMetadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    MetadataShape {
        path: PathBuf,
        message: &'static str,
    },
    SerializeManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    WriteManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    CreateCaseOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteDicomFile {
        path: PathBuf,
        message: String,
    },
    ReadGeneratedFile {
        path: PathBuf,
        source: std::io::Error,
    },
    ValidateDicomFile {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(profile) => write!(
                f,
                "unsupported profile {profile}; expected one of {}",
                SUPPORTED_PROFILES.join(", ")
            ),
            Self::CreateOutputDir { path, source } => {
                write!(
                    f,
                    "failed to create output directory {}: {source}",
                    path.display()
                )
            }
            Self::ReadMetadata { path, source } => {
                write!(
                    f,
                    "failed to read metadata file {}: {source}",
                    path.display()
                )
            }
            Self::ParseMetadata { path, source } => {
                write!(
                    f,
                    "failed to parse metadata file {}: {source}",
                    path.display()
                )
            }
            Self::MetadataShape { path, message } => {
                write!(f, "invalid metadata shape in {}: {message}", path.display())
            }
            Self::SerializeManifest { path, source } => {
                write!(
                    f,
                    "failed to serialize manifest {}: {source}",
                    path.display()
                )
            }
            Self::WriteManifest { path, source } => {
                write!(f, "failed to write manifest {}: {source}", path.display())
            }
            Self::CreateCaseOutputDir { path, source } => {
                write!(
                    f,
                    "failed to create case output directory {}: {source}",
                    path.display()
                )
            }
            Self::WriteDicomFile { path, message } => {
                write!(
                    f,
                    "failed to write DICOM file {}: {message}",
                    path.display()
                )
            }
            Self::ReadGeneratedFile { path, source } => {
                write!(
                    f,
                    "failed to read generated file {}: {source}",
                    path.display()
                )
            }
            Self::ValidateDicomFile { path, message } => {
                write!(
                    f,
                    "generated DICOM file {} failed validation: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for GenerateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidProfile(_) => None,
            Self::CreateOutputDir { source, .. } => Some(source),
            Self::ReadMetadata { source, .. } => Some(source),
            Self::ParseMetadata { source, .. } => Some(source),
            Self::MetadataShape { .. } => None,
            Self::SerializeManifest { source, .. } => Some(source),
            Self::WriteManifest { source, .. } => Some(source),
            Self::CreateCaseOutputDir { source, .. } => Some(source),
            Self::WriteDicomFile { .. } => None,
            Self::ReadGeneratedFile { source, .. } => Some(source),
            Self::ValidateDicomFile { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum ValidateError {
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    ManifestShape {
        path: PathBuf,
        message: &'static str,
    },
}

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadManifest { path, source } => {
                write!(f, "failed to read manifest {}: {source}", path.display())
            }
            Self::ParseManifest { path, source } => {
                write!(f, "failed to parse manifest {}: {source}", path.display())
            }
            Self::ManifestShape { path, message } => {
                write!(f, "invalid manifest shape in {}: {message}", path.display())
            }
        }
    }
}

impl Error for ValidateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadManifest { source, .. } => Some(source),
            Self::ParseManifest { source, .. } => Some(source),
            Self::ManifestShape { .. } => None,
        }
    }
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadMetadata { path, source } => {
                write!(
                    f,
                    "failed to read report metadata {}: {source}",
                    path.display()
                )
            }
            Self::ParseMetadata { path, source } => {
                write!(
                    f,
                    "failed to parse report metadata {}: {source}",
                    path.display()
                )
            }
            Self::MetadataShape { path, message } => {
                write!(
                    f,
                    "invalid report metadata in {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for ReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadMetadata { source, .. } => Some(source),
            Self::ParseMetadata { source, .. } => Some(source),
            Self::MetadataShape { .. } => None,
        }
    }
}

impl fmt::Display for StandardsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadMetadata { path, source } => {
                write!(
                    f,
                    "failed to read standards lock metadata {}: {source}",
                    path.display()
                )
            }
            Self::ParseMetadata { path, source } => {
                write!(
                    f,
                    "failed to parse standards lock metadata {}: {source}",
                    path.display()
                )
            }
            Self::MetadataShape { path, message } => {
                write!(
                    f,
                    "invalid standards lock metadata in {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for StandardsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadMetadata { source, .. } => Some(source),
            Self::ParseMetadata { source, .. } => Some(source),
            Self::MetadataShape { .. } => None,
        }
    }
}

pub fn prepare_generation_run(
    options: GenerateOptions,
) -> Result<PreparedGenerationRun, GenerateError> {
    if !SUPPORTED_PROFILES.contains(&options.profile.as_str()) {
        return Err(GenerateError::InvalidProfile(options.profile));
    }

    fs::create_dir_all(&options.out_dir).map_err(|source| GenerateError::CreateOutputDir {
        path: options.out_dir.clone(),
        source,
    })?;

    Ok(PreparedGenerationRun {
        manifest_path: options.out_dir.join("manifest.json"),
        profile: options.profile,
        out_dir: options.out_dir,
        seed: options.seed,
        include_stress: options.include_stress,
    })
}

pub fn write_generation_run(
    run: &PreparedGenerationRun,
) -> Result<GenerationSummary, GenerateError> {
    let standards_lock_path = Path::new("standards.lock.json");
    let cargo_lock_path = Path::new("Cargo.lock");
    let registry_path = Path::new("cases/registry.json");

    let standards_lock = read_json_metadata(standards_lock_path)?;
    let standards_lock_bytes = read_bytes_metadata(standards_lock_path)?;
    let cargo_lock = read_bytes_metadata(cargo_lock_path)?;
    let registry = read_json_metadata(registry_path)?;

    let generated_files =
        generator::write_supported_cases(run, &registry, &sha256_hex(&standards_lock_bytes))?;
    let files_written = generated_files.len();
    let generated_case_ids: Vec<String> = generated_files
        .iter()
        .map(|file| file.case_id.clone())
        .collect();
    let manifest = build_generation_manifest(
        run,
        &standards_lock,
        &standards_lock_bytes,
        &cargo_lock,
        &registry,
        generated_files,
        &generated_case_ids,
    )?;
    let mut contents = serde_json::to_string_pretty(&manifest).map_err(|source| {
        GenerateError::SerializeManifest {
            path: run.manifest_path.clone(),
            source,
        }
    })?;
    contents.push('\n');

    fs::write(&run.manifest_path, contents).map_err(|source| GenerateError::WriteManifest {
        path: run.manifest_path.clone(),
        source,
    })?;

    Ok(GenerationSummary {
        files_written,
        manifest_written: true,
    })
}

pub fn validate_generated_root(
    root_dir: impl AsRef<Path>,
) -> Result<ValidationSummary, ValidateError> {
    let root_dir = root_dir.as_ref();
    let manifest_path = root_dir.join("manifest.json");
    let manifest_contents =
        fs::read_to_string(&manifest_path).map_err(|source| ValidateError::ReadManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest: Value = serde_json::from_str(&manifest_contents).map_err(|source| {
        ValidateError::ParseManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;
    let files =
        manifest
            .get("files")
            .and_then(Value::as_array)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.clone(),
                message: "missing files array",
            })?;
    let source_objects = build_manifest_source_object_map(&manifest_path, files)?;

    let mut failures = Vec::new();
    for file in files {
        validate_manifest_references(&manifest_path, file, &source_objects, &mut failures)?;
        validate_manifest_file(root_dir, &manifest_path, file, &mut failures)?;
    }

    Ok(ValidationSummary {
        manifest_path,
        files_checked: files.len(),
        failures,
    })
}

fn build_manifest_source_object_map(
    manifest_path: &Path,
    files: &[Value],
) -> Result<HashMap<String, ManifestSourceObject>, ValidateError> {
    let mut source_objects = HashMap::new();
    for file in files {
        let path = manifest_str(manifest_path, file, "/path", "file path must be a string")?;
        let case_id = manifest_str(manifest_path, file, "/case_id", "case_id must be a string")?;
        let sop_class_uid = manifest_str(
            manifest_path,
            file,
            "/dicom/sop_class_uid",
            "dicom sop_class_uid must be a string",
        )?;
        let sop_instance_uid = manifest_str(
            manifest_path,
            file,
            "/uids/sop_instance_uid",
            "uids sop_instance_uid must be a string",
        )?;
        let series_instance_uid = file
            .pointer("/uids/series_instance_uid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let frames = file.pointer("/image/frames").and_then(Value::as_u64);

        source_objects.insert(
            path.to_string(),
            ManifestSourceObject {
                case_id: case_id.to_string(),
                sop_class_uid: sop_class_uid.to_string(),
                sop_instance_uid: sop_instance_uid.to_string(),
                series_instance_uid,
                frames,
            },
        );
    }
    Ok(source_objects)
}

fn validate_manifest_references(
    manifest_path: &Path,
    file: &Value,
    source_objects: &HashMap<String, ManifestSourceObject>,
    failures: &mut Vec<String>,
) -> Result<(), ValidateError> {
    let relative_path = manifest_str(manifest_path, file, "/path", "file path must be a string")?;
    let references = match file.get("references") {
        Some(Value::Array(references)) => references,
        Some(_) => {
            return Err(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "file references must be an array",
            });
        }
        None => {
            return Err(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "file references are missing",
            });
        }
    };

    for reference in references {
        let source_path = manifest_str(
            manifest_path,
            reference,
            "/source_path",
            "reference source_path must be a string",
        )?;
        let source_case_id = manifest_str(
            manifest_path,
            reference,
            "/source_case_id",
            "reference source_case_id must be a string",
        )?;
        let Some(source) = source_objects.get(source_path) else {
            failures.push(format!(
                "{relative_path}: reference_source_path: {source_path} is not generated in this run"
            ));
            continue;
        };

        validate_equal(
            failures,
            relative_path,
            "reference_source_case_id",
            source_case_id,
            source.case_id.as_str(),
        );
        validate_equal(
            failures,
            relative_path,
            "reference_sop_class_uid",
            manifest_str(
                manifest_path,
                reference,
                "/sop_class_uid",
                "reference sop_class_uid must be a string",
            )?,
            source.sop_class_uid.as_str(),
        );
        validate_equal(
            failures,
            relative_path,
            "reference_sop_instance_uid",
            manifest_str(
                manifest_path,
                reference,
                "/sop_instance_uid",
                "reference sop_instance_uid must be a string",
            )?,
            source.sop_instance_uid.as_str(),
        );

        if let Some(expected_series) = reference
            .get("series_instance_uid")
            .filter(|value| !value.is_null())
        {
            let expected_series = expected_series
                .as_str()
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "reference series_instance_uid must be a string or null",
                })?;
            match source.series_instance_uid.as_deref() {
                Some(actual_series) => validate_equal(
                    failures,
                    relative_path,
                    "reference_series_instance_uid",
                    expected_series,
                    actual_series,
                ),
                None => failures.push(format!(
                    "{relative_path}: reference_series_instance_uid: source has no Series Instance UID"
                )),
            }
        }

        if let Some(frame_numbers) = reference
            .get("frame_numbers")
            .filter(|value| !value.is_null())
        {
            let frame_numbers = frame_numbers
                .as_array()
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "reference frame_numbers must be an array or null",
                })?;
            for frame_number in frame_numbers {
                let frame_number = frame_number.as_u64().ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "reference frame_numbers items must be integers",
                })?;
                if frame_number == 0 {
                    failures.push(format!(
                        "{relative_path}: reference_frame_number: frame numbers are 1-based"
                    ));
                }
                if let Some(frames) = source.frames {
                    if frame_number > frames {
                        failures.push(format!(
                            "{relative_path}: reference_frame_number: frame {frame_number} exceeds source frame count {frames}"
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn validate_manifest_file(
    root_dir: &Path,
    manifest_path: &Path,
    file: &Value,
    failures: &mut Vec<String>,
) -> Result<(), ValidateError> {
    let relative_path = manifest_str(manifest_path, file, "/path", "file path must be a string")?;
    let path = root_dir.join(relative_path);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            failures.push(format!("{relative_path}: read_file: {err}"));
            return Ok(());
        }
    };

    validate_equal(
        failures,
        relative_path,
        "size_bytes",
        bytes.len() as u64,
        manifest_u64(
            manifest_path,
            file,
            "/size_bytes",
            "size_bytes must be an integer",
        )?,
    );
    validate_equal(
        failures,
        relative_path,
        "sha256",
        sha256_hex(&bytes),
        manifest_str(manifest_path, file, "/sha256", "sha256 must be a string")?,
    );

    let expected_sop_class = manifest_str(
        manifest_path,
        file,
        "/dicom/sop_class_uid",
        "dicom sop_class_uid must be a string",
    )?;
    let expected_sop_instance = manifest_str(
        manifest_path,
        file,
        "/uids/sop_instance_uid",
        "uids sop_instance_uid must be a string",
    )?;
    let expected_transfer_syntax = manifest_str(
        manifest_path,
        file,
        "/dicom/transfer_syntax_uid",
        "dicom transfer_syntax_uid must be a string",
    )?;
    let expected_implementation_class_uid = manifest_str(
        manifest_path,
        file,
        "/uids/implementation_class_uid",
        "uids implementation_class_uid must be a string",
    )?;
    let expected_implementation_version_name = file
        .pointer("/uids/implementation_version_name")
        .and_then(Value::as_str)
        .unwrap_or(IMPLEMENTATION_VERSION_NAME);

    validate_raw_part10_file(
        failures,
        relative_path,
        &bytes,
        expected_sop_class,
        expected_sop_instance,
        expected_transfer_syntax,
        expected_implementation_class_uid,
        expected_implementation_version_name,
    );

    let obj = match open_file(&path) {
        Ok(obj) => obj,
        Err(err) => {
            failures.push(format!("{relative_path}: open_file: {err}"));
            return Ok(());
        }
    };

    validate_equal(
        failures,
        relative_path,
        "file_meta_transfer_syntax",
        trim_uid(obj.meta().transfer_syntax()),
        expected_transfer_syntax,
    );
    validate_equal(
        failures,
        relative_path,
        "media_storage_sop_class_uid",
        trim_uid(obj.meta().media_storage_sop_class_uid()),
        expected_sop_class,
    );
    validate_equal(
        failures,
        relative_path,
        "media_storage_sop_instance_uid",
        trim_uid(obj.meta().media_storage_sop_instance_uid()),
        expected_sop_instance,
    );
    validate_equal(
        failures,
        relative_path,
        "implementation_class_uid",
        trim_uid(obj.meta().implementation_class_uid()),
        expected_implementation_class_uid,
    );
    validate_str_element(
        failures,
        relative_path,
        &obj,
        tags::SOP_CLASS_UID,
        "dataset_sop_class_uid",
        expected_sop_class,
    );
    validate_str_element(
        failures,
        relative_path,
        &obj,
        tags::SOP_INSTANCE_UID,
        "dataset_sop_instance_uid",
        expected_sop_instance,
    );
    validate_standard_baseline_elements(failures, relative_path, manifest_path, file, &obj)?;
    validate_family_standard_elements(failures, relative_path, manifest_path, file, &obj)?;

    validate_str_element(
        failures,
        relative_path,
        &obj,
        tags::SYNTHETIC_DATA,
        "synthetic_data",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/synthetic_data",
            "expected synthetic_data must be a string",
        )?,
    );

    match (file.get("image"), file.get("pixel_data")) {
        (Some(Value::Object(_)), Some(Value::Object(_))) => {
            validate_manifest_image_pixel_data(failures, relative_path, manifest_path, file, &obj)
        }
        (None | Some(Value::Null), None | Some(Value::Null)) => Ok(()),
        _ => {
            failures.push(format!(
                "{relative_path}: image_pixel_metadata_pair: image and pixel_data must both be present for image objects or both absent/null for non-image objects"
            ));
            Ok(())
        }
    }
}

fn validate_manifest_image_pixel_data(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let rows = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/rows",
        tags::ROWS,
        "rows",
    )?;
    let columns = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/columns",
        tags::COLUMNS,
        "columns",
    )?;
    let samples_per_pixel = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/samples_per_pixel",
        tags::SAMPLES_PER_PIXEL,
        "samples_per_pixel",
    )?;
    let bits_allocated = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/bits_allocated",
        tags::BITS_ALLOCATED,
        "bits_allocated",
    )?;
    let bits_stored = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/bits_stored",
        tags::BITS_STORED,
        "bits_stored",
    )?;
    let high_bit = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/high_bit",
        tags::HIGH_BIT,
        "high_bit",
    )?;
    validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        &obj,
        "/image/pixel_representation",
        tags::PIXEL_REPRESENTATION,
        "pixel_representation",
    )?;
    let photometric_interpretation = manifest_str(
        manifest_path,
        file,
        "/image/photometric_interpretation",
        "photometric_interpretation must be a string",
    )?;
    validate_str_element(
        failures,
        relative_path,
        &obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        "photometric_interpretation",
        photometric_interpretation,
    );
    match file.pointer("/image/planar_configuration") {
        Some(Value::Null) => {
            if let Ok(Some(_)) = obj.element_opt(tags::PLANAR_CONFIGURATION) {
                failures.push(format!(
                    "{relative_path}: planar_configuration_absent: expected absent"
                ));
            }
        }
        Some(_) => {
            validate_u16_from_manifest_and_dataset(
                failures,
                relative_path,
                manifest_path,
                file,
                &obj,
                "/image/planar_configuration",
                tags::PLANAR_CONFIGURATION,
                "planar_configuration",
            )?;
        }
        None => {
            return Err(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "image planar_configuration is missing",
            });
        }
    }

    if bits_stored > bits_allocated {
        failures.push(format!(
            "{relative_path}: bits_stored_within_bits_allocated: {bits_stored} > {bits_allocated}"
        ));
    }
    if high_bit + 1 != bits_stored {
        failures.push(format!(
            "{relative_path}: high_bit_consistency: {high_bit} does not equal bits_stored - 1"
        ));
    }

    let frames = validate_frames(failures, relative_path, manifest_path, file, &obj)?;
    let pixel_element = match obj.element(tags::PIXEL_DATA) {
        Ok(element) => element,
        Err(err) => {
            failures.push(format!("{relative_path}: pixel_data: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "pixel_data_vr",
        vr_name(pixel_element.vr()),
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/vr",
            "pixel_data vr must be a string",
        )?,
    );
    let native_or_encapsulated = manifest_str(
        manifest_path,
        file,
        "/pixel_data/native_or_encapsulated",
        "pixel_data native_or_encapsulated must be a string",
    )?;
    let frame_count = manifest_u64(
        manifest_path,
        file,
        "/pixel_data/frame_count",
        "pixel_data frame_count must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "pixel_data_frame_count",
        frame_count,
        u64::from(frames),
    );
    let frame_hashes = manifest_array(
        manifest_path,
        file,
        "/pixel_data/frame_hashes",
        "pixel_data frame_hashes must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "pixel_data_frame_hash_count",
        frame_hashes.len(),
        usize::try_from(frame_count).unwrap_or(usize::MAX),
    );

    match native_or_encapsulated {
        "native" => {
            let pixel_bytes = match pixel_element.value().to_bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    failures.push(format!("{relative_path}: pixel_data_bytes: {err}"));
                    return Ok(());
                }
            };
            validate_native_pixel_data_manifest(
                failures,
                relative_path,
                manifest_path,
                file,
                pixel_bytes.as_ref(),
                rows,
                columns,
                frames,
                samples_per_pixel,
                bits_allocated,
            )?;
        }
        "encapsulated" => {
            let pixel_fragments = match pixel_element.value() {
                dicom_core::value::Value::PixelSequence(sequence) => Some(sequence.fragments()),
                _ => None,
            };
            validate_encapsulated_pixel_data_manifest(
                failures,
                relative_path,
                manifest_path,
                file,
                &obj,
                pixel_fragments,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
                frame_hashes,
            )?;
        }
        "absent" => failures.push(format!(
            "{relative_path}: pixel_data_absent_manifest: image objects must not use native_or_encapsulated absent"
        )),
        other => failures.push(format!(
            "{relative_path}: pixel_data_native_or_encapsulated: unsupported value {other}"
        )),
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_native_pixel_data_manifest(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    pixel_bytes: &[u8],
    rows: u16,
    columns: u16,
    frames: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
) -> Result<(), ValidateError> {
    if let Some(encapsulated) = file
        .pointer("/pixel_data/encapsulated_pixel_data")
        .filter(|value| !value.is_null())
    {
        if manifest_bool(
            manifest_path,
            encapsulated,
            "/extended_offset_table/present",
            "extended_offset_table present must be a boolean",
        )
        .unwrap_or(false)
        {
            failures.push(format!(
                "{relative_path}: extended_offset_table_native_pixel_data: Extended Offset Table is not valid for native Pixel Data"
            ));
        } else {
            failures.push(format!(
                "{relative_path}: encapsulated_pixel_data_native_pixel_data: native Pixel Data must not carry encapsulated layout metadata"
            ));
        }
    }

    let expected_value_length = manifest_u64(
        manifest_path,
        file,
        "/pixel_data/value_length",
        "pixel_data value_length must be an integer",
    )? as usize;
    validate_equal(
        failures,
        relative_path,
        "pixel_data_manifest_length",
        pixel_bytes.len(),
        expected_value_length,
    );
    let photometric = manifest_str(
        manifest_path,
        file,
        "/image/photometric_interpretation",
        "photometric_interpretation must be a string",
    )?;
    let bytes_per_sample = usize::from(bits_allocated).div_ceil(8);
    let expected_native_length = if bits_allocated == 1 {
        let frame_bits = usize::from(rows) * usize::from(columns) * usize::from(samples_per_pixel);
        let value_length = usize::from(frames) * frame_bits.div_ceil(8);
        value_length + (value_length % 2)
    } else if photometric == "YBR_FULL_422" {
        usize::from(rows) * usize::from(columns) * usize::from(frames) * 2 * bytes_per_sample
    } else {
        usize::from(rows)
            * usize::from(columns)
            * usize::from(frames)
            * usize::from(samples_per_pixel)
            * bytes_per_sample
    };
    validate_equal(
        failures,
        relative_path,
        "native_pixel_data_length",
        pixel_bytes.len(),
        expected_native_length,
    );

    Ok(())
}

fn validate_encapsulated_pixel_data_manifest(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    let transfer_syntax = manifest_str(
        manifest_path,
        file,
        "/dicom/transfer_syntax_uid",
        "dicom transfer_syntax_uid must be a string",
    )?;
    if is_native_or_dataset_deflated_transfer_syntax(transfer_syntax) {
        failures.push(format!(
            "{relative_path}: encapsulated_pixel_data_transfer_syntax: transfer syntax {transfer_syntax} does not encode encapsulated image frames"
        ));
    }

    if !file
        .pointer("/pixel_data/value_length")
        .is_some_and(Value::is_null)
    {
        failures.push(format!(
            "{relative_path}: encapsulated_pixel_data_value_length: encapsulated Pixel Data should use an undefined value length recorded as null"
        ));
    }

    let frame_count = manifest_u64(
        manifest_path,
        file,
        "/pixel_data/frame_count",
        "pixel_data frame_count must be an integer",
    )?;
    let layout = file.pointer("/pixel_data/encapsulated_pixel_data").ok_or(
        ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "encapsulated pixel_data requires encapsulated_pixel_data metadata",
        },
    )?;
    if layout.is_null() {
        failures.push(format!(
            "{relative_path}: encapsulated_pixel_data_layout: encapsulated Pixel Data layout metadata is required"
        ));
        return Ok(());
    }

    let basic_offset_table_present = manifest_bool(
        manifest_path,
        layout,
        "/basic_offset_table/present",
        "basic_offset_table present must be a boolean",
    )?;
    if !basic_offset_table_present {
        failures.push(format!(
            "{relative_path}: basic_offset_table_present: encapsulated Pixel Data requires a Basic Offset Table item"
        ));
    }
    let basic_offset_table_populated = manifest_bool(
        manifest_path,
        layout,
        "/basic_offset_table/populated",
        "basic_offset_table populated must be a boolean",
    )?;
    let basic_offset_count = manifest_u64(
        manifest_path,
        layout,
        "/basic_offset_table/offset_count",
        "basic_offset_table offset_count must be an integer",
    )?;
    let fragments_per_frame = manifest_array(
        manifest_path,
        layout,
        "/fragments_per_frame",
        "fragments_per_frame must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "encapsulated_fragments_frame_count",
        fragments_per_frame.len(),
        usize::try_from(frame_count).unwrap_or(usize::MAX),
    );
    let mut all_single_fragment = true;
    let mut all_multiple_fragments = true;
    let mut fragment_counts = Vec::with_capacity(fragments_per_frame.len());
    for fragment_count in fragments_per_frame {
        let Some(fragment_count) = fragment_count.as_u64() else {
            return Err(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "fragments_per_frame items must be integers",
            });
        };
        let fragment_count =
            usize::try_from(fragment_count).map_err(|_| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "fragments_per_frame item is too large",
            })?;
        if fragment_count == 0 {
            failures.push(format!(
                "{relative_path}: encapsulated_fragment_count: every frame must have at least one fragment"
            ));
        }
        all_single_fragment &= fragment_count == 1;
        all_multiple_fragments &= fragment_count > 1;
        fragment_counts.push(fragment_count);
    }

    let compressed_frame_hashes = manifest_array(
        manifest_path,
        layout,
        "/compressed_frame_hashes",
        "compressed_frame_hashes must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "compressed_frame_hash_count",
        compressed_frame_hashes.len(),
        usize::try_from(frame_count).unwrap_or(usize::MAX),
    );
    validate_rle_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
        frame_hashes,
    )?;
    validate_jpeg_baseline_manifest_decoded_frame_tolerance(
        failures,
        relative_path,
        manifest_path,
        file,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
    )?;
    validate_jpeg_ls_lossless_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
        frame_hashes,
    )?;
    validate_jpeg_xl_lossless_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
        frame_hashes,
    )?;
    validate_jpeg_2000_lossless_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
        frame_hashes,
    )?;
    validate_htj2k_lossless_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
        frame_hashes,
    )?;
    validate_legacy_jpeg_lossless_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        obj,
        transfer_syntax,
        pixel_fragments,
        &fragment_counts,
        frame_hashes,
    )?;
    validate_deflated_image_frame_manifest_decoded_frame_hashes(
        failures,
        relative_path,
        manifest_path,
        transfer_syntax,
        pixel_fragments,
        rows,
        columns,
        samples_per_pixel,
        bits_allocated,
        bits_stored,
        photometric_interpretation,
        &fragment_counts,
        frame_hashes,
    )?;

    let extended_offset_table_present = manifest_bool(
        manifest_path,
        layout,
        "/extended_offset_table/present",
        "extended_offset_table present must be a boolean",
    )?;
    let extended_lengths_present = manifest_bool(
        manifest_path,
        layout,
        "/extended_offset_table/lengths_present",
        "extended_offset_table lengths_present must be a boolean",
    )?;
    let extended_offset_count = manifest_u64(
        manifest_path,
        layout,
        "/extended_offset_table/offset_count",
        "extended_offset_table offset_count must be an integer",
    )?;
    let extended_length_count = manifest_u64(
        manifest_path,
        layout,
        "/extended_offset_table/length_count",
        "extended_offset_table length_count must be an integer",
    )?;

    if basic_offset_table_populated {
        validate_equal(
            failures,
            relative_path,
            "basic_offset_table_offset_count",
            basic_offset_count,
            frame_count,
        );
    } else {
        validate_equal(
            failures,
            relative_path,
            "basic_offset_table_empty",
            basic_offset_count,
            0,
        );
    }

    if extended_offset_table_present {
        if basic_offset_table_populated {
            failures.push(format!(
                "{relative_path}: extended_offset_table_with_populated_basic_offset_table: Extended Offset Table requires an empty Basic Offset Table"
            ));
        }
        if !all_single_fragment {
            failures.push(format!(
                "{relative_path}: extended_offset_table_multiple_fragments: Extended Offset Table requires one fragment per frame"
            ));
        }
        if extended_offset_count == 0 {
            failures.push(format!(
                "{relative_path}: extended_offset_table_empty: Extended Offset Table must contain one offset per frame"
            ));
        }
        validate_equal(
            failures,
            relative_path,
            "extended_offset_table_offset_count",
            extended_offset_count,
            frame_count,
        );
        if !extended_lengths_present {
            failures.push(format!(
                "{relative_path}: extended_offset_table_without_lengths: Extended Offset Table requires Extended Offset Table Lengths"
            ));
        }
        validate_equal(
            failures,
            relative_path,
            "extended_offset_table_length_count",
            extended_length_count,
            frame_count,
        );
    } else {
        validate_equal(
            failures,
            relative_path,
            "extended_offset_table_absent_offsets",
            extended_offset_count,
            0,
        );
        validate_equal(
            failures,
            relative_path,
            "extended_offset_table_lengths_absent",
            extended_length_count,
            0,
        );
        if extended_lengths_present {
            failures.push(format!(
                "{relative_path}: extended_offset_table_lengths_without_table: Extended Offset Table Lengths require Extended Offset Table"
            ));
        }
        if !basic_offset_table_populated && !all_multiple_fragments {
            failures.push(format!(
                "{relative_path}: empty_basic_offset_table_without_extended_offsets: empty Basic Offset Table without Extended Offset Table is reserved for multiple-fragment frames"
            ));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_rle_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    if transfer_syntax != RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
        return Ok(());
    }

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: rle_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: rle_decoded_frame_hashes: RLE round-trip validation currently requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: rle_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let decoder = NativeRleLosslessEncoder::new();
    let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
    for fragment in pixel_fragments {
        match decoder.decode_frame(FrameDecodeInput {
            encoded_frame: fragment,
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
        }) {
            Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
            Err(err) => {
                failures.push(format!("{relative_path}: rle_decode_round_trip: {err}"));
                return Ok(());
            }
        }
    }

    validate_equal(
        failures,
        relative_path,
        "rle_decoded_frame_hashes",
        decoded_hashes.join(","),
        expected_hashes.join(","),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_jpeg_baseline_manifest_decoded_frame_tolerance(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
) -> Result<(), ValidateError> {
    if transfer_syntax != JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID {
        return Ok(());
    }
    const MAX_ABS_DIFF: u8 = 10;

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: jpeg_baseline_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: jpeg_baseline_decoded_frame_tolerance: JPEG Baseline validation currently requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: jpeg_baseline_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_values = manifest_array(
        manifest_path,
        file,
        "/recipe/recipe_parameters/pixel_values",
        "recipe pixel_values must be an array",
    )?
    .iter()
    .map(|value| {
        let sample = value.as_i64().ok_or_else(|| ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "recipe pixel_values items must be integers",
        })?;
        u8::try_from(sample).map_err(|_| ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "JPEG Baseline recipe pixel_values items must fit in u8",
        })
    })
    .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "jpeg")]
    {
        let decoder = DicomRsJpegBaselineEncoder::new();
        for fragment in pixel_fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
            }) {
                Ok(decoded) => {
                    if decoded.native_bytes.len() != expected_values.len() {
                        failures.push(format!(
                            "{relative_path}: jpeg_baseline_decoded_frame_length: decoded frame has {} samples, expected {}",
                            decoded.native_bytes.len(),
                            expected_values.len()
                        ));
                        return Ok(());
                    }
                    let max_diff = expected_values
                        .iter()
                        .copied()
                        .zip(decoded.native_bytes.iter().copied())
                        .map(|(expected, actual)| expected.abs_diff(actual))
                        .max()
                        .unwrap_or(0);
                    if max_diff > MAX_ABS_DIFF {
                        failures.push(format!(
                            "{relative_path}: jpeg_baseline_decoded_frame_tolerance: maximum sample difference {max_diff} exceeds {MAX_ABS_DIFF}"
                        ));
                        return Ok(());
                    }
                }
                Err(err) => {
                    failures.push(format!(
                        "{relative_path}: jpeg_baseline_decode_round_trip: {err}"
                    ));
                    return Ok(());
                }
            }
        }
    }

    #[cfg(not(feature = "jpeg"))]
    {
        let _ = (
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
            pixel_fragments,
            expected_values,
            MAX_ABS_DIFF,
        );
        failures.push(format!(
            "{relative_path}: jpeg_baseline_decoder_unavailable: validate requires the jpeg Cargo feature"
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_jpeg_ls_lossless_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    if transfer_syntax != JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID {
        return Ok(());
    }

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: jpeg_ls_lossless_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: jpeg_ls_lossless_decoded_frame_hashes: JPEG-LS Lossless validation currently requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: jpeg_ls_lossless_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "charls")]
    {
        let decoder = DicomRsJpegLsLosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
        for fragment in pixel_fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(err) => {
                    failures.push(format!(
                        "{relative_path}: jpeg_ls_lossless_decode_round_trip: {err}"
                    ));
                    return Ok(());
                }
            }
        }

        validate_equal(
            failures,
            relative_path,
            "jpeg_ls_lossless_decoded_frame_hashes",
            decoded_hashes.join(","),
            expected_hashes.join(","),
        );
    }

    #[cfg(not(feature = "charls"))]
    {
        let _ = (
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
            pixel_fragments,
            expected_hashes,
        );
        failures.push(format!(
            "{relative_path}: jpeg_ls_lossless_decoder_unavailable: validate requires the charls Cargo feature"
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_jpeg_xl_lossless_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    if transfer_syntax != JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID {
        return Ok(());
    }

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: jpeg_xl_lossless_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: jpeg_xl_lossless_decoded_frame_hashes: JPEG XL Lossless validation currently requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: jpeg_xl_lossless_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "jpegxl")]
    {
        let decoder = DicomRsJpegXlLosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
        for fragment in pixel_fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(err) => {
                    failures.push(format!(
                        "{relative_path}: jpeg_xl_lossless_decode_round_trip: {err}"
                    ));
                    return Ok(());
                }
            }
        }

        validate_equal(
            failures,
            relative_path,
            "jpeg_xl_lossless_decoded_frame_hashes",
            decoded_hashes.join(","),
            expected_hashes.join(","),
        );
    }

    #[cfg(not(feature = "jpegxl"))]
    {
        let _ = (
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
            pixel_fragments,
            expected_hashes,
        );
        failures.push(format!(
            "{relative_path}: jpeg_xl_lossless_decoder_unavailable: validate requires the jpegxl Cargo feature"
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_jpeg_2000_lossless_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    if transfer_syntax != JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID {
        return Ok(());
    }

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: jpeg_2000_lossless_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: jpeg_2000_lossless_decoded_frame_hashes: JPEG 2000 Lossless validation currently requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: jpeg_2000_lossless_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "jpeg2000")]
    {
        let decoder = OpenJp2Jpeg2000LosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
        for fragment in pixel_fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(err) => {
                    failures.push(format!(
                        "{relative_path}: jpeg_2000_lossless_decode_round_trip: {err}"
                    ));
                    return Ok(());
                }
            }
        }

        validate_equal(
            failures,
            relative_path,
            "jpeg_2000_lossless_decoded_frame_hashes",
            decoded_hashes.join(","),
            expected_hashes.join(","),
        );
    }

    #[cfg(not(feature = "jpeg2000"))]
    {
        let _ = (
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
            pixel_fragments,
            expected_hashes,
        );
        failures.push(format!(
            "{relative_path}: jpeg_2000_lossless_decoder_unavailable: validate requires the jpeg2000 Cargo feature"
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_htj2k_lossless_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    if transfer_syntax != HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID {
        return Ok(());
    }

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: htj2k_lossless_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: htj2k_lossless_decoded_frame_hashes: HTJ2K Lossless validation currently requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: htj2k_lossless_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "htj2k_openjph")]
    {
        let decoder = OpenJphHtj2kLosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
        for fragment in pixel_fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(err) => {
                    failures.push(format!(
                        "{relative_path}: htj2k_lossless_decode_round_trip: {err}"
                    ));
                    return Ok(());
                }
            }
        }

        validate_equal(
            failures,
            relative_path,
            "htj2k_lossless_decoded_frame_hashes",
            decoded_hashes.join(","),
            expected_hashes.join(","),
        );
    }

    #[cfg(not(feature = "htj2k_openjph"))]
    {
        let _ = (
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
            pixel_fragments,
            expected_hashes,
        );
        failures.push(format!(
            "{relative_path}: htj2k_lossless_decoder_unavailable: validate requires the htj2k_openjph Cargo feature"
        ));
    }

    Ok(())
}

fn validate_legacy_jpeg_lossless_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &OpenedObject,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    let Some(validation) =
        LegacyJpegLosslessManifestValidation::for_transfer_syntax(transfer_syntax)
    else {
        return Ok(());
    };

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: {}: Pixel Data is not an encapsulated fragment sequence",
            validation.pixel_sequence_name
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: {}: {} validation currently requires one fragment per frame",
            validation.hash_check_name, validation.label
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: {}: expected {} fragment(s), got {}",
            validation.fragment_count_name,
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "legacy_jpeg_dcmtk")]
    {
        let codec = if transfer_syntax == JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID {
            JPEG_LOSSLESS_NON_HIERARCHICAL.codec()
        } else {
            JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.codec()
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) = codec else {
            failures.push(format!(
                "{relative_path}: {}: validate requires the legacy_jpeg_dcmtk Cargo feature",
                validation.decoder_unavailable_name
            ));
            return Ok(());
        };
        let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
        for frame_index in 0..pixel_fragments.len() {
            let mut decoded = Vec::new();
            if let Err(err) = reader.decode_frame(file, frame_index as u32, &mut decoded) {
                failures.push(format!(
                    "{relative_path}: {}: {err}",
                    validation.decode_check_name
                ));
                return Ok(());
            }
            decoded_hashes.push(sha256_hex(&decoded));
        }

        validate_equal(
            failures,
            relative_path,
            validation.hash_check_name,
            decoded_hashes.join(","),
            expected_hashes.join(","),
        );
    }

    #[cfg(not(feature = "legacy_jpeg_dcmtk"))]
    {
        let _ = (file, pixel_fragments, expected_hashes);
        failures.push(format!(
            "{relative_path}: {}: validate requires the legacy_jpeg_dcmtk Cargo feature",
            validation.decoder_unavailable_name
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_deflated_image_frame_manifest_decoded_frame_hashes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    transfer_syntax: &str,
    pixel_fragments: Option<&[Vec<u8>]>,
    rows: u16,
    columns: u16,
    samples_per_pixel: u16,
    bits_allocated: u16,
    bits_stored: u16,
    photometric_interpretation: &str,
    fragments_per_frame: &[usize],
    frame_hashes: &[Value],
) -> Result<(), ValidateError> {
    if transfer_syntax != DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID {
        return Ok(());
    }

    let Some(pixel_fragments) = pixel_fragments else {
        failures.push(format!(
            "{relative_path}: deflated_image_frame_pixel_sequence: Pixel Data is not an encapsulated fragment sequence"
        ));
        return Ok(());
    };
    if !fragments_per_frame.iter().all(|count| *count == 1) {
        failures.push(format!(
            "{relative_path}: deflated_image_frame_decoded_frame_hashes: Deflated Image Frame validation requires one fragment per frame"
        ));
        return Ok(());
    }
    if pixel_fragments.len() != fragments_per_frame.len() {
        failures.push(format!(
            "{relative_path}: deflated_image_frame_fragment_count: expected {} fragment(s), got {}",
            fragments_per_frame.len(),
            pixel_fragments.len()
        ));
        return Ok(());
    }

    let expected_hashes = frame_hashes
        .iter()
        .map(|hash| {
            hash.as_str().ok_or_else(|| ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "pixel_data frame_hashes items must be strings",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "deflate")]
    {
        let decoder = DicomRsDeflatedImageFrameEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(pixel_fragments.len());
        for fragment in pixel_fragments {
            let decoded = match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows,
                columns,
                samples_per_pixel,
                bits_allocated,
                bits_stored,
                photometric_interpretation,
            }) {
                Ok(decoded) => decoded,
                Err(err) => {
                    failures.push(format!(
                        "{relative_path}: deflated_image_frame_decode_round_trip: {err}"
                    ));
                    return Ok(());
                }
            };
            decoded_hashes.push(sha256_hex(&decoded.native_bytes));
        }

        validate_equal(
            failures,
            relative_path,
            "deflated_image_frame_decoded_frame_hashes",
            decoded_hashes.join(","),
            expected_hashes.join(","),
        );
    }

    #[cfg(not(feature = "deflate"))]
    {
        let _ = (
            rows,
            columns,
            samples_per_pixel,
            bits_allocated,
            bits_stored,
            photometric_interpretation,
            pixel_fragments,
            expected_hashes,
        );
        failures.push(format!(
            "{relative_path}: deflated_image_frame_decoder_unavailable: validate requires the deflate Cargo feature"
        ));
    }

    Ok(())
}

#[allow(dead_code)]
struct LegacyJpegLosslessManifestValidation {
    label: &'static str,
    pixel_sequence_name: &'static str,
    hash_check_name: &'static str,
    fragment_count_name: &'static str,
    decoder_unavailable_name: &'static str,
    decode_check_name: &'static str,
}

impl LegacyJpegLosslessManifestValidation {
    fn for_transfer_syntax(transfer_syntax: &str) -> Option<Self> {
        match transfer_syntax {
            JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID => Some(Self {
                label: "JPEG Lossless Process 14",
                pixel_sequence_name: "jpeg_lossless_process_14_pixel_sequence",
                hash_check_name: "jpeg_lossless_process_14_decoded_frame_hashes",
                fragment_count_name: "jpeg_lossless_process_14_fragment_count",
                decoder_unavailable_name: "jpeg_lossless_process_14_decoder_unavailable",
                decode_check_name: "jpeg_lossless_process_14_decode_round_trip",
            }),
            JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID => Some(Self {
                label: "JPEG Lossless SV1",
                pixel_sequence_name: "jpeg_lossless_sv1_pixel_sequence",
                hash_check_name: "jpeg_lossless_sv1_decoded_frame_hashes",
                fragment_count_name: "jpeg_lossless_sv1_fragment_count",
                decoder_unavailable_name: "jpeg_lossless_sv1_decoder_unavailable",
                decode_check_name: "jpeg_lossless_sv1_decode_round_trip",
            }),
            _ => None,
        }
    }
}

fn is_native_or_dataset_deflated_transfer_syntax(transfer_syntax: &str) -> bool {
    matches!(
        transfer_syntax,
        "1.2.840.10008.1.2"
            | "1.2.840.10008.1.2.1"
            | "1.2.840.10008.1.2.1.99"
            | "1.2.840.10008.1.2.2"
    )
}

#[derive(Debug)]
struct RawFileMetaElement {
    tag: (u16, u16),
    vr: String,
    value_offset: usize,
    value_length: usize,
    next_offset: usize,
}

fn validate_raw_part10_file(
    failures: &mut Vec<String>,
    relative_path: &str,
    bytes: &[u8],
    expected_sop_class: &str,
    expected_sop_instance: &str,
    expected_transfer_syntax: &str,
    expected_implementation_class_uid: &str,
    expected_implementation_version_name: &str,
) {
    if bytes.len() < 132 {
        failures.push(format!(
            "{relative_path}: part10_preamble: file is shorter than the 132-byte Part 10 prefix"
        ));
        return;
    }
    if bytes[..128].iter().any(|byte| *byte != 0) {
        failures.push(format!(
            "{relative_path}: part10_zero_preamble: expected 128 zero preamble bytes"
        ));
    }
    if &bytes[128..132] != b"DICM" {
        failures.push(format!(
            "{relative_path}: part10_dicm_prefix: expected DICM marker at byte offset 128"
        ));
        return;
    }

    let (file_meta, dataset_start) = parse_file_meta_elements(failures, relative_path, bytes);
    let Some(file_meta) = file_meta else {
        return;
    };

    validate_required_file_meta_element(
        failures,
        relative_path,
        &file_meta,
        (0x0002, 0x0000),
        "UL",
        "file_meta_group_length",
    );
    validate_required_file_meta_element(
        failures,
        relative_path,
        &file_meta,
        (0x0002, 0x0001),
        "OB",
        "file_meta_information_version",
    );
    validate_required_file_meta_element(
        failures,
        relative_path,
        &file_meta,
        (0x0002, 0x0002),
        "UI",
        "media_storage_sop_class_uid_raw",
    );
    validate_required_file_meta_element(
        failures,
        relative_path,
        &file_meta,
        (0x0002, 0x0003),
        "UI",
        "media_storage_sop_instance_uid_raw",
    );
    validate_required_file_meta_element(
        failures,
        relative_path,
        &file_meta,
        (0x0002, 0x0010),
        "UI",
        "file_meta_transfer_syntax_raw",
    );
    validate_required_file_meta_element(
        failures,
        relative_path,
        &file_meta,
        (0x0002, 0x0012),
        "UI",
        "implementation_class_uid_raw",
    );

    if let Some(group_length) = raw_file_meta_element(&file_meta, (0x0002, 0x0000)) {
        if group_length.value_length == 4 {
            let declared = read_u32_le(bytes, group_length.value_offset);
            let actual = dataset_start.saturating_sub(group_length.next_offset) as u32;
            validate_equal(
                failures,
                relative_path,
                "file_meta_group_length_value",
                actual,
                declared,
            );
        }
    }
    if let Some(version) = raw_file_meta_element(&file_meta, (0x0002, 0x0001)) {
        let value = raw_value(bytes, version);
        if value != [0, 1] {
            failures.push(format!(
                "{relative_path}: file_meta_information_version_value: expected 00 01"
            ));
        }
    }
    validate_raw_ui(
        failures,
        relative_path,
        bytes,
        &file_meta,
        (0x0002, 0x0002),
        "media_storage_sop_class_uid_raw_value",
        expected_sop_class,
    );
    validate_raw_ui(
        failures,
        relative_path,
        bytes,
        &file_meta,
        (0x0002, 0x0003),
        "media_storage_sop_instance_uid_raw_value",
        expected_sop_instance,
    );
    validate_raw_ui(
        failures,
        relative_path,
        bytes,
        &file_meta,
        (0x0002, 0x0010),
        "file_meta_transfer_syntax_raw_value",
        expected_transfer_syntax,
    );
    validate_raw_ui(
        failures,
        relative_path,
        bytes,
        &file_meta,
        (0x0002, 0x0012),
        "implementation_class_uid_raw_value",
        expected_implementation_class_uid,
    );
    if let Some(version_name) = raw_file_meta_element(&file_meta, (0x0002, 0x0013)) {
        validate_equal(
            failures,
            relative_path,
            "implementation_version_name",
            raw_text(bytes, version_name),
            expected_implementation_version_name,
        );
    }
    for element in &file_meta {
        if !is_allowed_file_meta_tag(element.tag) {
            failures.push(format!(
                "{relative_path}: file_meta_allowed_element: unexpected File Meta element ({:04X},{:04X})",
                element.tag.0, element.tag.1
            ));
        }
    }
    if dataset_start >= bytes.len() {
        failures.push(format!(
            "{relative_path}: file_meta_dataset_boundary: file ended before dataset"
        ));
    }
    if expected_transfer_syntax != dicom_dictionary_std::uids::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN
        && contains_dataset_group_0002(bytes, dataset_start, expected_transfer_syntax)
    {
        failures.push(format!(
            "{relative_path}: file_meta_group_boundary: dataset contains group 0002 elements after File Meta Information"
        ));
    }
}

fn parse_file_meta_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    bytes: &[u8],
) -> (Option<Vec<RawFileMetaElement>>, usize) {
    let mut offset = 132;
    let mut elements = Vec::new();
    loop {
        if offset + 4 > bytes.len() {
            failures.push(format!(
                "{relative_path}: file_meta_group_boundary: file ended before dataset"
            ));
            return (None, offset);
        }
        let group = read_u16_le(bytes, offset);
        if group != 0x0002 {
            return (Some(elements), offset);
        }
        let Some(element) = parse_explicit_vr_element(bytes, offset) else {
            failures.push(format!(
                "{relative_path}: file_meta_explicit_vr_little_endian: malformed File Meta element at byte offset {offset}"
            ));
            return (None, offset);
        };
        if !is_uppercase_vr(&element.vr) {
            failures.push(format!(
                "{relative_path}: file_meta_explicit_vr_little_endian: invalid VR {} at byte offset {offset}",
                element.vr
            ));
            return (None, offset);
        }
        offset = element.next_offset;
        elements.push(element);
    }
}

fn validate_required_file_meta_element(
    failures: &mut Vec<String>,
    relative_path: &str,
    file_meta: &[RawFileMetaElement],
    tag: (u16, u16),
    expected_vr: &str,
    name: &str,
) {
    match raw_file_meta_element(file_meta, tag) {
        Some(element) => {
            validate_equal(
                failures,
                relative_path,
                name,
                element.vr.as_str(),
                expected_vr,
            );
        }
        None => failures.push(format!(
            "{relative_path}: {name}: required File Meta element ({:04X},{:04X}) is missing",
            tag.0, tag.1
        )),
    }
}

fn validate_raw_ui(
    failures: &mut Vec<String>,
    relative_path: &str,
    bytes: &[u8],
    file_meta: &[RawFileMetaElement],
    tag: (u16, u16),
    name: &str,
    expected: &str,
) {
    if let Some(element) = raw_file_meta_element(file_meta, tag) {
        validate_equal(
            failures,
            relative_path,
            name,
            raw_text(bytes, element),
            expected,
        );
    }
}

fn raw_file_meta_element(
    file_meta: &[RawFileMetaElement],
    tag: (u16, u16),
) -> Option<&RawFileMetaElement> {
    file_meta.iter().find(|element| element.tag == tag)
}

fn is_allowed_file_meta_tag(tag: (u16, u16)) -> bool {
    matches!(
        tag,
        (0x0002, 0x0000)
            | (0x0002, 0x0001)
            | (0x0002, 0x0002)
            | (0x0002, 0x0003)
            | (0x0002, 0x0010)
            | (0x0002, 0x0012)
            | (0x0002, 0x0013)
    )
}

fn raw_value<'a>(bytes: &'a [u8], element: &RawFileMetaElement) -> &'a [u8] {
    &bytes[element.value_offset..element.value_offset + element.value_length]
}

fn raw_text(bytes: &[u8], element: &RawFileMetaElement) -> String {
    String::from_utf8_lossy(raw_value(bytes, element))
        .trim_matches('\0')
        .trim()
        .to_string()
}

fn contains_dataset_group_0002(bytes: &[u8], mut offset: usize, transfer_syntax_uid: &str) -> bool {
    let explicit_vr = transfer_syntax_uid != dicom_dictionary_std::uids::IMPLICIT_VR_LITTLE_ENDIAN;
    let big_endian = transfer_syntax_uid == "1.2.840.10008.1.2.2";
    while offset + 8 <= bytes.len() {
        let group = read_dataset_u16(bytes, offset, big_endian);
        if group == 0x0002 {
            return true;
        }
        let Some((value_length, next_offset)) =
            parse_dataset_element_header(bytes, offset, explicit_vr, big_endian)
        else {
            return false;
        };
        if value_length == 0xFFFF_FFFF {
            return false;
        }
        offset = next_offset.saturating_add(value_length as usize);
    }
    false
}

fn parse_dataset_element_header(
    bytes: &[u8],
    offset: usize,
    explicit_vr: bool,
    big_endian: bool,
) -> Option<(u32, usize)> {
    if explicit_vr {
        parse_explicit_vr_dataset_element(bytes, offset, big_endian)
    } else {
        if offset + 8 > bytes.len() {
            return None;
        }
        Some((read_u32_le(bytes, offset + 4), offset + 8))
    }
}

fn parse_explicit_vr_dataset_element(
    bytes: &[u8],
    offset: usize,
    big_endian: bool,
) -> Option<(u32, usize)> {
    if offset + 8 > bytes.len() {
        return None;
    }
    let vr = &bytes[offset + 4..offset + 6];
    if !vr.iter().all(u8::is_ascii_uppercase) {
        return None;
    }
    let long_vr = matches!(
        vr,
        b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
    );
    if long_vr {
        if offset + 12 > bytes.len() {
            return None;
        }
        Some((read_dataset_u32(bytes, offset + 8, big_endian), offset + 12))
    } else {
        Some((
            read_dataset_u16(bytes, offset + 6, big_endian).into(),
            offset + 8,
        ))
    }
}

fn read_dataset_u16(bytes: &[u8], offset: usize, big_endian: bool) -> u16 {
    if big_endian {
        u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
    } else {
        read_u16_le(bytes, offset)
    }
}

fn read_dataset_u32(bytes: &[u8], offset: usize, big_endian: bool) -> u32 {
    if big_endian {
        u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    } else {
        read_u32_le(bytes, offset)
    }
}

fn parse_explicit_vr_element(bytes: &[u8], offset: usize) -> Option<RawFileMetaElement> {
    if offset + 8 > bytes.len() {
        return None;
    }
    let group = read_u16_le(bytes, offset);
    let element = read_u16_le(bytes, offset + 2);
    let vr = std::str::from_utf8(&bytes[offset + 4..offset + 6])
        .ok()?
        .to_string();
    let long_vr = matches!(
        vr.as_str(),
        "OB" | "OD" | "OF" | "OL" | "OV" | "OW" | "SQ" | "UC" | "UR" | "UT" | "UN"
    );
    let (value_length, value_offset) = if long_vr {
        if offset + 12 > bytes.len() || bytes[offset + 6] != 0 || bytes[offset + 7] != 0 {
            return None;
        }
        (read_u32_le(bytes, offset + 8) as usize, offset + 12)
    } else {
        (usize::from(read_u16_le(bytes, offset + 6)), offset + 8)
    };
    let next_offset = value_offset.checked_add(value_length)?;
    if next_offset > bytes.len() {
        return None;
    }
    Some(RawFileMetaElement {
        tag: (group, element),
        vr,
        value_offset,
        value_length,
        next_offset,
    })
}

fn is_uppercase_vr(vr: &str) -> bool {
    vr.len() == 2 && vr.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn manifest_str<'a>(
    manifest_path: &Path,
    value: &'a Value,
    pointer: &str,
    message: &'static str,
) -> Result<&'a str, ValidateError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message,
        })
}

fn manifest_u64(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<u64, ValidateError> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message,
        })
}

fn manifest_bool(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<bool, ValidateError> {
    value
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message,
        })
}

fn manifest_array<'a>(
    manifest_path: &Path,
    value: &'a Value,
    pointer: &str,
    message: &'static str,
) -> Result<&'a Vec<Value>, ValidateError> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message,
        })
}

fn validate_u16_from_manifest_and_dataset(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
    manifest_pointer: &str,
    tag: dicom_core::Tag,
    name: &str,
) -> Result<u16, ValidateError> {
    let expected = manifest_u64(
        manifest_path,
        file,
        manifest_pointer,
        "image field must be an integer",
    )? as u16;
    match element_u16_for_validate(obj, tag) {
        Ok(actual) => {
            validate_equal(failures, relative_path, name, actual, expected);
            Ok(actual)
        }
        Err(err) => {
            failures.push(format!("{relative_path}: {name}: {err}"));
            Ok(expected)
        }
    }
}

fn validate_frames(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<u16, ValidateError> {
    let expected = manifest_u64(
        manifest_path,
        file,
        "/image/frames",
        "frames must be an integer",
    )? as u16;
    match obj.element_opt(tags::NUMBER_OF_FRAMES) {
        Ok(Some(element)) => match element.value().to_int::<u16>() {
            Ok(actual) => {
                validate_equal(failures, relative_path, "frames", actual, expected);
                Ok(actual)
            }
            Err(err) => {
                failures.push(format!("{relative_path}: frames: {err}"));
                Ok(expected)
            }
        },
        Ok(None) if expected == 1 => Ok(1),
        Ok(None) => {
            failures.push(format!(
                "{relative_path}: frames: Number of Frames is missing for {expected} frames"
            ));
            Ok(expected)
        }
        Err(err) => {
            failures.push(format!("{relative_path}: frames: {err}"));
            Ok(expected)
        }
    }
}

fn validate_standard_baseline_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::PATIENT_NAME,
        "patient_name_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::PATIENT_ID,
        "patient_id_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::PATIENT_BIRTH_DATE,
        "patient_birth_date_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::PATIENT_SEX,
        "patient_sex_type2",
    );

    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::STUDY_INSTANCE_UID,
        "study_instance_uid_type1",
        manifest_str(
            manifest_path,
            file,
            "/uids/study_instance_uid",
            "uids study_instance_uid must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::STUDY_DATE,
        "study_date_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::STUDY_TIME,
        "study_time_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::REFERRING_PHYSICIAN_NAME,
        "referring_physician_name_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::STUDY_ID,
        "study_id_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::ACCESSION_NUMBER,
        "accession_number_type2",
    );

    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "modality_type1",
        manifest_str(
            manifest_path,
            file,
            "/dicom/modality",
            "dicom modality must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SERIES_INSTANCE_UID,
        "series_instance_uid_type1",
        manifest_str(
            manifest_path,
            file,
            "/uids/series_instance_uid",
            "uids series_instance_uid must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::SERIES_NUMBER,
        "series_number_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::INSTANCE_NUMBER,
        "instance_number_type2",
    );

    Ok(())
}

fn validate_family_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    match manifest_str(
        manifest_path,
        file,
        "/dicom/iod_name",
        "dicom iod_name must be a string",
    )? {
        "Secondary Capture Image" => validate_secondary_capture_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "CT Image" => {
            validate_ct_image_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
        "Digital Mammography X-Ray Image" => validate_mammography_image_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "Digital X-Ray Image" => {
            validate_dx_image_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
        "Ultrasound Image" => validate_ultrasound_image_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "Computed Radiography Image" => {
            validate_computed_radiography_standard_elements(failures, relative_path, obj)
        }
        "MR Image" => {
            validate_mr_image_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
        "Enhanced CT Image" => validate_enhanced_ct_image_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "Enhanced MR Image" => validate_enhanced_mr_image_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "Segmentation" => validate_segmentation_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "Basic Text SR" | "Comprehensive SR" | "Key Object Selection Document" => {
            validate_structured_report_standard_elements(
                failures,
                relative_path,
                manifest_path,
                file,
                obj,
            )?
        }
        "RT Structure Set" => validate_rt_structure_set_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "RT Dose" => {
            validate_rt_dose_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
        "Encapsulated PDF" => validate_encapsulated_pdf_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        _ => {}
    }

    Ok(())
}

fn validate_secondary_capture_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::CONVERSION_TYPE,
        "sc_conversion_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/conversion_type",
            "expected conversion_type must be a string",
        )?,
    );

    Ok(())
}

fn validate_ct_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "ct_image_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/image_type",
            "expected image_type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::RESCALE_INTERCEPT,
        "ct_rescale_intercept_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rescale/intercept",
            "expected rescale intercept must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::RESCALE_SLOPE,
        "ct_rescale_slope_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rescale/slope",
            "expected rescale slope must be a string",
        )?,
    );
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::RESCALE_TYPE,
        "ct_rescale_type_present_type1c",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rescale/type",
            "expected rescale type must be a string",
        )?,
    );
    validate_type2_element(failures, relative_path, obj, tags::KVP, "ct_kvp_type2");
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::ACQUISITION_NUMBER,
        "ct_acquisition_number_type2",
    );

    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "frame_of_reference_uid_type1",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "uids frame_of_reference_uid must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::POSITION_REFERENCE_INDICATOR,
        "position_reference_indicator_type2",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::PIXEL_SPACING,
        "pixel_spacing_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/geometry/pixel_spacing",
            "expected pixel_spacing must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_ORIENTATION_PATIENT,
        "image_orientation_patient_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/geometry/image_orientation_patient",
            "expected image_orientation_patient must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_POSITION_PATIENT,
        "image_position_patient_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/geometry/image_position_patient",
            "expected image_position_patient must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::SLICE_THICKNESS,
        "slice_thickness_type2",
    );

    Ok(())
}

fn validate_mammography_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "mg_image_type_type1",
        "ORIGINAL\\PRIMARY",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::POSITIONER_TYPE,
        "mg_positioner_type_type1",
        "MAMMOGRAPHIC",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_LATERALITY,
        "mg_image_laterality_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/image_laterality",
            "expected image_laterality must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::ORGAN_EXPOSED,
        "mg_organ_exposed_type1",
        "BREAST",
    );

    validate_dx_family_standard_elements(failures, relative_path, manifest_path, file, obj)?;

    Ok(())
}

fn validate_dx_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "dx_image_type_type1",
        "ORIGINAL\\PRIMARY",
    );
    validate_dx_family_standard_elements(failures, relative_path, manifest_path, file, obj)
}

fn validate_dx_family_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::PIXEL_INTENSITY_RELATIONSHIP,
        "dx_pixel_intensity_relationship_type1",
        "LIN",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::RESCALE_INTERCEPT,
        "dx_rescale_intercept_type1",
        "0",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::RESCALE_SLOPE,
        "dx_rescale_slope_type1",
        "1",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::RESCALE_TYPE,
        "dx_rescale_type_type1",
        "US",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::PRESENTATION_LUT_SHAPE,
        "dx_presentation_lut_shape_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/presentation_lut_shape",
            "expected presentation_lut_shape must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::LOSSY_IMAGE_COMPRESSION,
        "dx_lossy_image_compression_type1",
        "00",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::BURNED_IN_ANNOTATION,
        "dx_burned_in_annotation_type1",
        "NO",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGER_PIXEL_SPACING,
        "dx_imager_pixel_spacing_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/imager_pixel_spacing",
            "expected imager_pixel_spacing must be a string",
        )?,
    );

    Ok(())
}

fn validate_ultrasound_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "us_image_type_type2",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/image_type",
            "expected image_type must be a string",
        )?,
    );

    Ok(())
}

fn validate_computed_radiography_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
) {
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::BODY_PART_EXAMINED,
        "cr_body_part_examined_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::VIEW_POSITION,
        "cr_view_position_type2",
    );
}

fn validate_mr_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "mr_image_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/image_type",
            "expected image_type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SCANNING_SEQUENCE,
        "mr_scanning_sequence_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/mr/scanning_sequence",
            "expected scanning_sequence must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SEQUENCE_VARIANT,
        "mr_sequence_variant_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/mr/sequence_variant",
            "expected sequence_variant must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::SCAN_OPTIONS,
        "mr_scan_options_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::MR_ACQUISITION_TYPE,
        "mr_acquisition_type_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::ECHO_TIME,
        "mr_echo_time_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::ECHO_TRAIN_LENGTH,
        "mr_echo_train_length_type2",
    );

    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "frame_of_reference_uid_type1",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "uids frame_of_reference_uid must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::POSITION_REFERENCE_INDICATOR,
        "position_reference_indicator_type2",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::PIXEL_SPACING,
        "pixel_spacing_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/geometry/pixel_spacing",
            "expected pixel_spacing must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_ORIENTATION_PATIENT,
        "image_orientation_patient_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/geometry/image_orientation_patient",
            "expected image_orientation_patient must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_POSITION_PATIENT,
        "image_position_patient_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/geometry/image_position_patient",
            "expected image_position_patient must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::SLICE_THICKNESS,
        "slice_thickness_type2",
    );

    Ok(())
}

fn validate_enhanced_ct_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_enhanced_ct_mr_common_standard_elements(
        failures,
        relative_path,
        manifest_path,
        file,
        obj,
        "enhanced_ct",
    )?;

    Ok(())
}

fn validate_enhanced_mr_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_enhanced_ct_mr_common_standard_elements(
        failures,
        relative_path,
        manifest_path,
        file,
        obj,
        "enhanced_mr",
    )?;

    Ok(())
}

fn validate_enhanced_ct_mr_common_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
    prefix: &str,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        &format!("{prefix}_image_type_type1"),
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/frame_type",
            "expected frame_type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::PIXEL_PRESENTATION,
        &format!("{prefix}_pixel_presentation_type1"),
        "MONOCHROME",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::VOLUMETRIC_PROPERTIES,
        &format!("{prefix}_volumetric_properties_type1"),
        "VOLUME",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
        &format!("{prefix}_volume_based_calculation_technique_type1"),
        "NONE",
    );

    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "frame_of_reference_uid_type1",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "uids frame_of_reference_uid must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::POSITION_REFERENCE_INDICATOR,
        "position_reference_indicator_type2",
    );

    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        &format!("{prefix}_shared_functional_groups_sequence_type1"),
        1,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        &format!("{prefix}_per_frame_functional_groups_sequence_type1c"),
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/per_frame_functional_groups_sequence_items",
            "expected per-frame functional groups count must be an integer",
        )?)
        .expect("manifest per-frame count must fit usize"),
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        &format!("{prefix}_dimension_organization_sequence_type1"),
        1,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::DIMENSION_INDEX_SEQUENCE,
        &format!("{prefix}_dimension_index_sequence_type1c"),
        1,
    );

    Ok(())
}

fn validate_segmentation_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SOP_CLASS_UID,
        "segmentation_storage_sop_class",
        manifest_str(
            manifest_path,
            file,
            "/dicom/sop_class_uid",
            "segmentation SOP Class UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "segmentation_modality_type1",
        "SEG",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        TAG_SEGMENTATION_TYPE,
        "segmentation_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/segmentation_type",
            "segmentation type must be a string",
        )?,
    );
    let segmentation_type = manifest_str(
        manifest_path,
        file,
        "/recipe/recipe_parameters/segmentation_type",
        "segmentation type must be a string",
    )?;
    if segmentation_type == "FRACTIONAL" {
        validate_type1_str_element(
            failures,
            relative_path,
            obj,
            TAG_SEGMENTATION_FRACTIONAL_TYPE,
            "segmentation_fractional_type_type1c",
            manifest_str(
                manifest_path,
                file,
                "/recipe/recipe_parameters/segmentation_fractional_type",
                "fractional segmentation type must be a string",
            )?,
        );
        validate_type1_u16_element(
            failures,
            relative_path,
            obj,
            TAG_MAXIMUM_FRACTIONAL_VALUE,
            "segmentation_maximum_fractional_value_type1c",
            u16::try_from(manifest_u64(
                manifest_path,
                file,
                "/recipe/recipe_parameters/maximum_fractional_value",
                "maximum fractional value must be an integer",
            )?)
            .expect("manifest maximum fractional value must fit u16"),
        );
    }
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        TAG_SEGMENT_SEQUENCE,
        "segmentation_segment_sequence_type1",
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/segment_sequence_items",
            "expected segment sequence item count must be an integer",
        )?)
        .expect("manifest segment sequence count must fit usize"),
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        "segmentation_shared_functional_groups_sequence_type1",
        1,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        "segmentation_per_frame_functional_groups_sequence_type1c",
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/per_frame_functional_groups_sequence_items",
            "expected per-frame functional groups count must be an integer",
        )?)
        .expect("manifest per-frame count must fit usize"),
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        "segmentation_dimension_organization_sequence_type1",
        1,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::DIMENSION_INDEX_SEQUENCE,
        "segmentation_dimension_index_sequence_type1c",
        1,
    );

    let frame_numbers = file
        .pointer("/recipe/recipe_parameters/referenced_frame_numbers")
        .and_then(Value::as_array)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "referenced_frame_numbers must be an array",
        })?;
    let source_sop_instance_uid = manifest_str(
        manifest_path,
        file,
        "/expected_semantics/source_sop_instance_uid",
        "source_sop_instance_uid must be a string",
    )?;
    let references =
        file.get("references")
            .and_then(Value::as_array)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "segmentation references must be an array",
            })?;
    let source_sop_class_uid = references
        .first()
        .and_then(|reference| reference.get("sop_class_uid"))
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "segmentation source reference sop_class_uid must be a string",
        })?;

    for (index, frame_number) in frame_numbers.iter().enumerate() {
        let expected_frame = frame_number.as_u64().ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "referenced_frame_numbers must contain integers",
        })?;
        let Ok(frame) = top_level_sequence_item_for_validate(
            obj,
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            index,
        ) else {
            failures.push(format!(
                "{relative_path}: segmentation_per_frame_functional_group_item: missing item {index}"
            ));
            continue;
        };
        match nested_sequence_item_u16_for_validate(
            frame,
            TAG_SEGMENT_IDENTIFICATION_SEQUENCE,
            0,
            TAG_REFERENCED_SEGMENT_NUMBER,
        ) {
            Ok(segment_number) => validate_equal(
                failures,
                relative_path,
                "segmentation_referenced_segment_number",
                segment_number,
                1,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: segmentation_referenced_segment_number: {err}"
            )),
        }
        let Ok(derivation) =
            item_sequence_item_for_validate(frame, TAG_DERIVATION_IMAGE_SEQUENCE, 0)
        else {
            failures.push(format!(
                "{relative_path}: segmentation_derivation_image_sequence: missing item"
            ));
            continue;
        };
        let Ok(source) = item_sequence_item_for_validate(derivation, TAG_SOURCE_IMAGE_SEQUENCE, 0)
        else {
            failures.push(format!(
                "{relative_path}: segmentation_source_image_sequence: missing item"
            ));
            continue;
        };
        match item_str_for_validate(source, TAG_REFERENCED_SOP_CLASS_UID) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "segmentation_source_image_sop_class_uid",
                actual,
                source_sop_class_uid,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: segmentation_source_image_sop_class_uid: {err}"
            )),
        }
        match item_str_for_validate(source, TAG_REFERENCED_SOP_INSTANCE_UID) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "segmentation_source_image_sop_instance_uid",
                actual,
                source_sop_instance_uid,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: segmentation_source_image_sop_instance_uid: {err}"
            )),
        }
        match item_str_for_validate(source, dicom_core::Tag(0x0008, 0x1160)) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "segmentation_source_image_frame_number",
                actual,
                expected_frame,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: segmentation_source_image_frame_number: {err}"
            )),
        }
    }

    Ok(())
}

fn validate_structured_report_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SOP_CLASS_UID,
        "sr_sop_class",
        manifest_str(
            manifest_path,
            file,
            "/dicom/sop_class_uid",
            "SR SOP Class UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "sr_modality_type1",
        manifest_str(
            manifest_path,
            file,
            "/dicom/modality",
            "SR modality must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::COMPLETION_FLAG,
        "sr_completion_flag_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/structured_report/completion_flag",
            "SR completion flag must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::VERIFICATION_FLAG,
        "sr_verification_flag_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/structured_report/verification_flag",
            "SR verification flag must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::VALUE_TYPE,
        "sr_root_value_type",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/structured_report/root_value_type",
            "SR root value type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::CONTINUITY_OF_CONTENT,
        "sr_root_continuity_of_content",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/structured_report/root_continuity_of_content",
            "SR root continuity must be a string",
        )?,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        "sr_current_requested_procedure_evidence_sequence_type1",
        1,
    );
    let structured_report = file
        .pointer("/expected_semantics/structured_report")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "SR expected structured_report semantics must be an object",
        })?;
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::CONTENT_SEQUENCE,
        "sr_content_sequence_type1c",
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/structured_report/content_sequence_items",
            "SR content sequence item count must be an integer",
        )?)
        .expect("manifest SR content item count must fit usize"),
    );

    let source_sop_instance_uid = manifest_str(
        manifest_path,
        file,
        "/expected_semantics/source_sop_instance_uid",
        "source_sop_instance_uid must be a string",
    )?;
    let references =
        file.get("references")
            .and_then(Value::as_array)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "SR references must be an array",
            })?;
    let source_sop_class_uid = references
        .first()
        .and_then(|reference| reference.get("sop_class_uid"))
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "SR source reference sop_class_uid must be a string",
        })?;

    let Ok(evidence) = top_level_sequence_item_for_validate(
        obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        0,
    ) else {
        failures.push(format!(
            "{relative_path}: sr_current_requested_procedure_evidence_sequence: missing item"
        ));
        return Ok(());
    };
    let Ok(series) = item_sequence_item_for_validate(evidence, tags::REFERENCED_SERIES_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: sr_evidence_referenced_series_sequence: missing item"
        ));
        return Ok(());
    };
    let Ok(sop) = item_sequence_item_for_validate(series, tags::REFERENCED_SOP_SEQUENCE, 0) else {
        failures.push(format!(
            "{relative_path}: sr_evidence_referenced_sop_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(sop, TAG_REFERENCED_SOP_CLASS_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_evidence_sop_class_uid",
            actual,
            source_sop_class_uid,
        ),
        Err(err) => failures.push(format!("{relative_path}: sr_evidence_sop_class_uid: {err}")),
    }
    match item_str_for_validate(sop, TAG_REFERENCED_SOP_INSTANCE_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_evidence_sop_instance_uid",
            actual,
            source_sop_instance_uid,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: sr_evidence_sop_instance_uid: {err}"
        )),
    }

    let Ok(observation) = top_level_sequence_item_for_validate(obj, tags::CONTENT_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: sr_content_sequence: missing observation item"
        ));
        return Ok(());
    };

    if structured_report.get("observation_text").is_some() {
        match item_str_for_validate(observation, tags::RELATIONSHIP_TYPE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "sr_observation_relationship_type",
                actual,
                "CONTAINS",
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: sr_observation_relationship_type: {err}"
            )),
        }
        match item_str_for_validate(observation, tags::VALUE_TYPE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "sr_observation_value_type",
                actual,
                "TEXT",
            ),
            Err(err) => failures.push(format!("{relative_path}: sr_observation_value_type: {err}")),
        }
        match item_str_for_validate(observation, tags::TEXT_VALUE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "sr_observation_text",
                actual,
                manifest_str(
                    manifest_path,
                    file,
                    "/expected_semantics/structured_report/observation_text",
                    "SR observation text must be a string",
                )?,
            ),
            Err(err) => failures.push(format!("{relative_path}: sr_observation_text: {err}")),
        }
    } else if structured_report.get("measurement").is_some() {
        validate_comprehensive_sr_content_items(
            failures,
            relative_path,
            manifest_path,
            file,
            observation,
            obj,
            source_sop_class_uid,
            source_sop_instance_uid,
        )?;
    } else if structured_report.get("key_objects").is_some() {
        validate_key_object_selection_content_items(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?;
    } else {
        failures.push(format!(
            "{relative_path}: sr_content_semantics: unsupported SR manifest content shape"
        ));
    }

    Ok(())
}

fn validate_comprehensive_sr_content_items(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    measurement: &DatasetObject,
    obj: &OpenedObject,
    source_sop_class_uid: &str,
    source_sop_instance_uid: &str,
) -> Result<(), ValidateError> {
    match item_str_for_validate(measurement, tags::RELATIONSHIP_TYPE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_measurement_relationship_type",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/structured_report/measurement/relationship_type",
                "SR measurement relationship type must be a string",
            )?,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: sr_measurement_relationship_type: {err}"
        )),
    }
    match item_str_for_validate(measurement, tags::VALUE_TYPE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_measurement_value_type",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/structured_report/measurement/value_type",
                "SR measurement value type must be a string",
            )?,
        ),
        Err(err) => failures.push(format!("{relative_path}: sr_measurement_value_type: {err}")),
    }
    let Ok(measured_value) =
        item_sequence_item_for_validate(measurement, tags::MEASURED_VALUE_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: sr_measured_value_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(measured_value, tags::NUMERIC_VALUE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_measurement_numeric_value",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/structured_report/measurement/numeric_value",
                "SR measurement numeric value must be a string",
            )?,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: sr_measurement_numeric_value: {err}"
        )),
    }
    let Ok(units) =
        item_sequence_item_for_validate(measured_value, tags::MEASUREMENT_UNITS_CODE_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: sr_measurement_units_code_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(units, tags::CODE_VALUE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_measurement_unit_code_value",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/structured_report/measurement/units/code_value",
                "SR measurement unit code value must be a string",
            )?,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: sr_measurement_unit_code_value: {err}"
        )),
    }

    let Ok(image) = top_level_sequence_item_for_validate(obj, tags::CONTENT_SEQUENCE, 1) else {
        failures.push(format!(
            "{relative_path}: sr_content_sequence: missing image reference item"
        ));
        return Ok(());
    };
    match item_str_for_validate(image, tags::RELATIONSHIP_TYPE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_image_relationship_type",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/structured_report/image_reference/relationship_type",
                "SR image relationship type must be a string",
            )?,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: sr_image_relationship_type: {err}"
        )),
    }
    match item_str_for_validate(image, tags::VALUE_TYPE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_image_value_type",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/structured_report/image_reference/value_type",
                "SR image value type must be a string",
            )?,
        ),
        Err(err) => failures.push(format!("{relative_path}: sr_image_value_type: {err}")),
    }
    let Ok(image_sop) = item_sequence_item_for_validate(image, tags::REFERENCED_SOP_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: sr_image_referenced_sop_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(image_sop, dicom_core::Tag(0x0008, 0x1150)) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_image_sop_class_uid",
            actual,
            source_sop_class_uid,
        ),
        Err(err) => failures.push(format!("{relative_path}: sr_image_sop_class_uid: {err}")),
    }
    match item_str_for_validate(image_sop, dicom_core::Tag(0x0008, 0x1155)) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "sr_image_sop_instance_uid",
            actual,
            source_sop_instance_uid,
        ),
        Err(err) => failures.push(format!("{relative_path}: sr_image_sop_instance_uid: {err}")),
    }

    Ok(())
}

fn validate_key_object_selection_content_items(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let references =
        file.get("references")
            .and_then(Value::as_array)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "KOS references must be an array",
            })?;
    let key_objects = file
        .pointer("/expected_semantics/structured_report/key_objects")
        .and_then(Value::as_array)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "KOS key object semantics must be an array",
        })?;

    for (index, key_object) in key_objects.iter().enumerate() {
        let Ok(content_item) =
            top_level_sequence_item_for_validate(obj, tags::CONTENT_SEQUENCE, index)
        else {
            failures.push(format!(
                "{relative_path}: kos_content_sequence: missing key object item {index}"
            ));
            continue;
        };
        match item_str_for_validate(content_item, tags::RELATIONSHIP_TYPE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "kos_image_relationship_type",
                actual,
                key_object
                    .get("relationship_type")
                    .and_then(Value::as_str)
                    .ok_or(ValidateError::ManifestShape {
                        path: manifest_path.to_path_buf(),
                        message: "KOS key object relationship_type must be a string",
                    })?,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: kos_image_relationship_type: {err}"
            )),
        }
        match item_str_for_validate(content_item, tags::VALUE_TYPE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "kos_image_value_type",
                actual,
                key_object.get("value_type").and_then(Value::as_str).ok_or(
                    ValidateError::ManifestShape {
                        path: manifest_path.to_path_buf(),
                        message: "KOS key object value_type must be a string",
                    },
                )?,
            ),
            Err(err) => failures.push(format!("{relative_path}: kos_image_value_type: {err}")),
        }

        let Ok(sop) =
            item_sequence_item_for_validate(content_item, tags::REFERENCED_SOP_SEQUENCE, 0)
        else {
            failures.push(format!(
                "{relative_path}: kos_referenced_sop_sequence: missing item {index}"
            ));
            continue;
        };
        let reference = references.get(index).ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "KOS references must align with key object items",
        })?;
        let expected_sop_class_uid = reference
            .get("sop_class_uid")
            .and_then(Value::as_str)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "KOS reference sop_class_uid must be a string",
            })?;
        let expected_sop_instance_uid = reference
            .get("sop_instance_uid")
            .and_then(Value::as_str)
            .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "KOS reference sop_instance_uid must be a string",
        })?;

        match item_str_for_validate(sop, dicom_core::Tag(0x0008, 0x1150)) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "kos_image_sop_class_uid",
                actual,
                expected_sop_class_uid,
            ),
            Err(err) => failures.push(format!("{relative_path}: kos_image_sop_class_uid: {err}")),
        }
        match item_str_for_validate(sop, dicom_core::Tag(0x0008, 0x1155)) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "kos_image_sop_instance_uid",
                actual,
                expected_sop_instance_uid,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: kos_image_sop_instance_uid: {err}"
            )),
        }

        if let Some(expected_frames) = reference.get("frame_numbers").and_then(Value::as_array) {
            let expected = expected_frames
                .iter()
                .map(|value| value.as_u64().unwrap_or_default().to_string())
                .collect::<Vec<_>>()
                .join("\\");
            match item_str_for_validate(sop, dicom_core::Tag(0x0008, 0x1160)) {
                Ok(actual) => validate_equal(
                    failures,
                    relative_path,
                    "kos_image_referenced_frame_numbers",
                    actual,
                    expected.as_str(),
                ),
                Err(err) => failures.push(format!(
                    "{relative_path}: kos_image_referenced_frame_numbers: {err}"
                )),
            }
        }
    }

    Ok(())
}

fn validate_rt_structure_set_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SOP_CLASS_UID,
        "rt_structure_set_sop_class",
        manifest_str(
            manifest_path,
            file,
            "/dicom/sop_class_uid",
            "RT Structure Set SOP Class UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "rt_structure_set_modality_type1",
        "RTSTRUCT",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "rt_structure_set_frame_of_reference_uid",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "RT Structure Set Frame of Reference UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::STRUCTURE_SET_LABEL,
        "rt_structure_set_label_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rt_structure_set/structure_set_label",
            "RT Structure Set label must be a string",
        )?,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::REFERENCED_FRAME_OF_REFERENCE_SEQUENCE,
        "rt_referenced_frame_of_reference_sequence",
        1,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::STRUCTURE_SET_ROI_SEQUENCE,
        "rt_structure_set_roi_sequence_type3",
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/rt_structure_set/structure_set_roi_items",
            "RT Structure Set ROI item count must be an integer",
        )?)
        .expect("manifest RT Structure Set ROI item count must fit usize"),
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::ROI_CONTOUR_SEQUENCE,
        "rt_roi_contour_sequence_type3",
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/rt_structure_set/roi_contour_items",
            "RT ROI Contour item count must be an integer",
        )?)
        .expect("manifest RT ROI Contour item count must fit usize"),
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::RTROI_OBSERVATIONS_SEQUENCE,
        "rt_roi_observations_sequence_type3",
        usize::try_from(manifest_u64(
            manifest_path,
            file,
            "/expected_semantics/rt_structure_set/rt_roi_observation_items",
            "RT ROI Observation item count must be an integer",
        )?)
        .expect("manifest RT ROI Observation item count must fit usize"),
    );

    let references =
        file.get("references")
            .and_then(Value::as_array)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "RT Structure Set references must be an array",
            })?;
    let source_reference = references.first().ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "RT Structure Set must have a source image reference",
    })?;
    let source_sop_class_uid = source_reference
        .get("sop_class_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Structure Set source reference sop_class_uid must be a string",
        })?;
    let source_sop_instance_uid = source_reference
        .get("sop_instance_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Structure Set source reference sop_instance_uid must be a string",
        })?;
    let source_series_instance_uid = source_reference
        .get("series_instance_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Structure Set source reference series_instance_uid must be a string",
        })?;
    let expected_frame_of_reference_uid = manifest_str(
        manifest_path,
        file,
        "/uids/frame_of_reference_uid",
        "RT Structure Set Frame of Reference UID must be a string",
    )?;
    let expected_roi_number = manifest_u64(
        manifest_path,
        file,
        "/expected_semantics/rt_structure_set/roi_number",
        "RT ROI Number must be an integer",
    )?
    .to_string();

    let Ok(referenced_for) =
        top_level_sequence_item_for_validate(obj, tags::REFERENCED_FRAME_OF_REFERENCE_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_referenced_frame_of_reference_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(referenced_for, tags::FRAME_OF_REFERENCE_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_referenced_frame_of_reference_uid",
            actual,
            expected_frame_of_reference_uid,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_referenced_frame_of_reference_uid: {err}"
        )),
    }
    let Ok(referenced_study) =
        item_sequence_item_for_validate(referenced_for, tags::RT_REFERENCED_STUDY_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_referenced_study_sequence: missing item"
        ));
        return Ok(());
    };
    let Ok(referenced_series) =
        item_sequence_item_for_validate(referenced_study, tags::RT_REFERENCED_SERIES_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_referenced_series_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(referenced_series, tags::SERIES_INSTANCE_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_referenced_series_uid",
            actual,
            source_series_instance_uid,
        ),
        Err(err) => failures.push(format!("{relative_path}: rt_referenced_series_uid: {err}")),
    }
    let Ok(referenced_contour_image) =
        item_sequence_item_for_validate(referenced_series, tags::CONTOUR_IMAGE_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_referenced_contour_image_sequence: missing item"
        ));
        return Ok(());
    };
    validate_rt_referenced_sop(
        failures,
        relative_path,
        "rt_referenced_contour_image",
        referenced_contour_image,
        source_sop_class_uid,
        source_sop_instance_uid,
    );

    let Ok(roi) = top_level_sequence_item_for_validate(obj, tags::STRUCTURE_SET_ROI_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_structure_set_roi_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(roi, tags::ROI_NUMBER) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_structure_set_roi_number",
            actual,
            expected_roi_number.as_str(),
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_structure_set_roi_number: {err}"
        )),
    }
    match item_str_for_validate(roi, tags::REFERENCED_FRAME_OF_REFERENCE_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_structure_set_roi_frame_of_reference_uid",
            actual,
            expected_frame_of_reference_uid,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_structure_set_roi_frame_of_reference_uid: {err}"
        )),
    }
    match item_str_for_validate(roi, tags::ROI_NAME) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_structure_set_roi_name",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/rt_structure_set/roi_name",
                "RT ROI Name must be a string",
            )?,
        ),
        Err(err) => failures.push(format!("{relative_path}: rt_structure_set_roi_name: {err}")),
    }
    match item_str_for_validate(roi, tags::ROI_GENERATION_ALGORITHM) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_structure_set_roi_generation_algorithm",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/rt_structure_set/roi_generation_algorithm",
                "RT ROI Generation Algorithm must be a string",
            )?,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_structure_set_roi_generation_algorithm: {err}"
        )),
    }

    let Ok(roi_contour) = top_level_sequence_item_for_validate(obj, tags::ROI_CONTOUR_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_roi_contour_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(roi_contour, tags::REFERENCED_ROI_NUMBER) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_roi_contour_referenced_roi_number",
            actual,
            expected_roi_number.as_str(),
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_roi_contour_referenced_roi_number: {err}"
        )),
    }
    let Ok(contour) = item_sequence_item_for_validate(roi_contour, tags::CONTOUR_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_contour_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(contour, tags::CONTOUR_GEOMETRIC_TYPE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_contour_geometric_type",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/rt_structure_set/contour_geometric_type",
                "RT Contour Geometric Type must be a string",
            )?,
        ),
        Err(err) => failures.push(format!("{relative_path}: rt_contour_geometric_type: {err}")),
    }
    match item_str_for_validate(contour, tags::NUMBER_OF_CONTOUR_POINTS) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_number_of_contour_points",
            actual,
            manifest_u64(
                manifest_path,
                file,
                "/expected_semantics/rt_structure_set/contour_points",
                "RT Contour Points must be an integer",
            )?
            .to_string()
            .as_str(),
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_number_of_contour_points: {err}"
        )),
    }
    match item_str_for_validate(contour, tags::CONTOUR_DATA) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_contour_data",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/rt_structure_set/contour_data",
                "RT Contour Data must be a string",
            )?,
        ),
        Err(err) => failures.push(format!("{relative_path}: rt_contour_data: {err}")),
    }
    let Ok(contour_image) =
        item_sequence_item_for_validate(contour, tags::CONTOUR_IMAGE_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_contour_image_sequence: missing item"
        ));
        return Ok(());
    };
    validate_rt_referenced_sop(
        failures,
        relative_path,
        "rt_contour_image",
        contour_image,
        source_sop_class_uid,
        source_sop_instance_uid,
    );

    let Ok(observation) =
        top_level_sequence_item_for_validate(obj, tags::RTROI_OBSERVATIONS_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_roi_observations_sequence: missing item"
        ));
        return Ok(());
    };
    match item_str_for_validate(observation, tags::REFERENCED_ROI_NUMBER) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_roi_observation_referenced_roi_number",
            actual,
            expected_roi_number.as_str(),
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_roi_observation_referenced_roi_number: {err}"
        )),
    }
    match item_str_for_validate(observation, tags::RTROI_INTERPRETED_TYPE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_roi_interpreted_type",
            actual,
            manifest_str(
                manifest_path,
                file,
                "/expected_semantics/rt_structure_set/roi_interpreted_type",
                "RT ROI Interpreted Type must be a string",
            )?,
        ),
        Err(err) => failures.push(format!("{relative_path}: rt_roi_interpreted_type: {err}")),
    }

    Ok(())
}

fn validate_rt_dose_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SOP_CLASS_UID,
        "rt_dose_sop_class",
        manifest_str(
            manifest_path,
            file,
            "/dicom/sop_class_uid",
            "RT Dose SOP Class UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "rt_dose_modality_type1",
        "RTDOSE",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "rt_dose_frame_of_reference_uid",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "RT Dose Frame of Reference UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::NUMBER_OF_FRAMES,
        "rt_dose_number_of_frames",
        manifest_u64(
            manifest_path,
            file,
            "/image/frames",
            "RT Dose image frame count must be an integer",
        )?
        .to_string()
        .as_str(),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::PIXEL_SPACING,
        "rt_dose_pixel_spacing",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/pixel_spacing",
            "RT Dose Pixel Spacing must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_ORIENTATION_PATIENT,
        "rt_dose_image_orientation_patient",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/image_orientation_patient",
            "RT Dose Image Orientation Patient must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_POSITION_PATIENT,
        "rt_dose_image_position_patient",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/image_position_patient",
            "RT Dose Image Position Patient must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SLICE_THICKNESS,
        "rt_dose_slice_thickness",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/slice_thickness",
            "RT Dose Slice Thickness must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::GRID_FRAME_OFFSET_VECTOR,
        "rt_dose_grid_frame_offset_vector",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rt_dose/grid_frame_offset_vector",
            "RT Dose Grid Frame Offset Vector must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::DOSE_UNITS,
        "rt_dose_units_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rt_dose/dose_units",
            "RT Dose Units must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::DOSE_TYPE,
        "rt_dose_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rt_dose/dose_type",
            "RT Dose Type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::DOSE_SUMMATION_TYPE,
        "rt_dose_summation_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rt_dose/dose_summation_type",
            "RT Dose Summation Type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::DOSE_GRID_SCALING,
        "rt_dose_grid_scaling_type1c",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/rt_dose/dose_grid_scaling",
            "RT Dose Grid Scaling must be a string",
        )?,
    );

    match element_tag_for_validate(obj, tags::FRAME_INCREMENT_POINTER) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "rt_dose_frame_increment_pointer",
            format!("{actual:?}"),
            format!("{:?}", tags::GRID_FRAME_OFFSET_VECTOR),
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: rt_dose_frame_increment_pointer: {err}"
        )),
    }

    let references =
        file.get("references")
            .and_then(Value::as_array)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "RT Dose references must be an array",
            })?;
    let image_reference = references.first().ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "RT Dose must have a source image reference",
    })?;
    let structure_set_reference = references.get(1).ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "RT Dose must have a source structure set reference",
    })?;
    let image_sop_class_uid = image_reference
        .get("sop_class_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Dose image reference sop_class_uid must be a string",
        })?;
    let image_sop_instance_uid = image_reference
        .get("sop_instance_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Dose image reference sop_instance_uid must be a string",
        })?;
    let structure_set_sop_class_uid = structure_set_reference
        .get("sop_class_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Dose structure set reference sop_class_uid must be a string",
        })?;
    let structure_set_sop_instance_uid = structure_set_reference
        .get("sop_instance_uid")
        .and_then(Value::as_str)
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "RT Dose structure set reference sop_instance_uid must be a string",
        })?;

    let Ok(referenced_image) =
        top_level_sequence_item_for_validate(obj, TAG_REFERENCED_IMAGE_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_dose_referenced_image_sequence: missing item"
        ));
        return Ok(());
    };
    validate_rt_referenced_sop(
        failures,
        relative_path,
        "rt_dose_referenced_image",
        referenced_image,
        image_sop_class_uid,
        image_sop_instance_uid,
    );

    let Ok(referenced_structure_set) =
        top_level_sequence_item_for_validate(obj, TAG_REFERENCED_STRUCTURE_SET_SEQUENCE, 0)
    else {
        failures.push(format!(
            "{relative_path}: rt_dose_referenced_structure_set_sequence: missing item"
        ));
        return Ok(());
    };
    validate_rt_referenced_sop(
        failures,
        relative_path,
        "rt_dose_referenced_structure_set",
        referenced_structure_set,
        structure_set_sop_class_uid,
        structure_set_sop_instance_uid,
    );

    Ok(())
}

fn validate_encapsulated_pdf_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SOP_CLASS_UID,
        "encapsulated_pdf_sop_class",
        manifest_str(
            manifest_path,
            file,
            "/dicom/sop_class_uid",
            "Encapsulated PDF SOP Class UID must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "encapsulated_document_modality_type1",
        "DOC",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::CONVERSION_TYPE,
        "encapsulated_pdf_conversion_type",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/conversion_type",
            "Encapsulated PDF conversion_type must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::INSTANCE_NUMBER,
        "encapsulated_document_instance_number_type1",
        "1",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::CONTENT_DATE,
        "encapsulated_document_content_date_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::CONTENT_TIME,
        "encapsulated_document_content_time_type2",
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::ACQUISITION_DATE_TIME,
        "encapsulated_document_acquisition_datetime_type2",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::BURNED_IN_ANNOTATION,
        "encapsulated_document_burned_in_annotation_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/encapsulated_document/burned_in_annotation",
            "Encapsulated PDF Burned In Annotation must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::RECOGNIZABLE_VISUAL_FEATURES,
        "encapsulated_document_recognizable_visual_features",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/encapsulated_document/recognizable_visual_features",
            "Encapsulated PDF Recognizable Visual Features must be a string",
        )?,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::DOCUMENT_TITLE,
        "encapsulated_document_title_type2",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/encapsulated_document/document_title",
            "Encapsulated PDF Document Title must be a string",
        )?,
    );
    validate_sequence_len(
        failures,
        relative_path,
        obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "encapsulated_document_concept_name_code_sequence_type2",
        0,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT,
        "encapsulated_document_mime_type_type1",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/encapsulated_document/mime_type",
            "Encapsulated PDF MIME Type must be a string",
        )?,
    );

    let expected_length = manifest_u64(
        manifest_path,
        file,
        "/expected_semantics/encapsulated_document/document_length",
        "Encapsulated PDF document_length must be an integer",
    )? as usize;
    match element_u32_for_validate(obj, tags::ENCAPSULATED_DOCUMENT_LENGTH) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "encapsulated_document_length",
            actual,
            expected_length as u32,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: encapsulated_document_length: {err}"
        )),
    }

    let expected_hash = manifest_str(
        manifest_path,
        file,
        "/expected_semantics/encapsulated_document/document_sha256",
        "Encapsulated PDF document_sha256 must be a string",
    )?;
    match obj.element(tags::ENCAPSULATED_DOCUMENT) {
        Ok(element) => {
            validate_equal(
                failures,
                relative_path,
                "encapsulated_document_vr",
                format!("{:?}", element.vr()),
                "OB",
            );
            match element.value().to_bytes() {
                Ok(bytes) => {
                    let value = bytes.as_ref();
                    if value.len() < expected_length {
                        failures.push(format!(
                            "{relative_path}: encapsulated_document_payload: value shorter than manifest document_length"
                        ));
                    } else {
                        validate_equal(
                            failures,
                            relative_path,
                            "encapsulated_document_sha256",
                            sha256_hex(&value[..expected_length]),
                            expected_hash,
                        );
                        if !value[..expected_length].starts_with(b"%PDF-") {
                            failures.push(format!(
                                "{relative_path}: encapsulated_document_pdf_header: payload does not start with %PDF-"
                            ));
                        }
                        if value.len() > expected_length + 1
                            || value[expected_length..].iter().any(|byte| *byte != 0)
                        {
                            failures.push(format!(
                                "{relative_path}: encapsulated_document_padding: unexpected bytes after original PDF payload"
                            ));
                        }
                    }
                }
                Err(err) => failures.push(format!(
                    "{relative_path}: encapsulated_document_payload: {err}"
                )),
            }
        }
        Err(err) => failures.push(format!(
            "{relative_path}: encapsulated_document_type1: {err}"
        )),
    }

    match obj.element(tags::PIXEL_DATA) {
        Ok(_) => failures.push(format!(
            "{relative_path}: encapsulated_pdf_pixel_data_absent: unexpected Pixel Data"
        )),
        Err(_) => {}
    }

    Ok(())
}

fn validate_rt_referenced_sop(
    failures: &mut Vec<String>,
    relative_path: &str,
    prefix: &str,
    item: &DatasetObject,
    expected_sop_class_uid: &str,
    expected_sop_instance_uid: &str,
) {
    match item_str_for_validate(item, TAG_REFERENCED_SOP_CLASS_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            &format!("{prefix}_sop_class_uid"),
            actual,
            expected_sop_class_uid,
        ),
        Err(err) => failures.push(format!("{relative_path}: {prefix}_sop_class_uid: {err}")),
    }
    match item_str_for_validate(item, TAG_REFERENCED_SOP_INSTANCE_UID) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            &format!("{prefix}_sop_instance_uid"),
            actual,
            expected_sop_instance_uid,
        ),
        Err(err) => failures.push(format!("{relative_path}: {prefix}_sop_instance_uid: {err}")),
    }
}

fn validate_type2_element(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    name: &str,
) {
    if let Err(err) = element_str_for_validate(obj, tag) {
        failures.push(format!("{relative_path}: {name}: {err}"));
    }
}

fn validate_sequence_len(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: usize,
) {
    match obj.element(tag) {
        Ok(element) => match element.items() {
            Some(items) => validate_equal(failures, relative_path, name, items.len(), expected),
            None => failures.push(format!(
                "{relative_path}: {name}: element is not a sequence"
            )),
        },
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn validate_type1_str_element(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: &str,
) {
    match element_str_for_validate(obj, tag) {
        Ok(actual) => {
            if actual.is_empty() {
                failures.push(format!(
                    "{relative_path}: {name}: Type 1 element must not be empty"
                ));
            }
            validate_equal(failures, relative_path, name, actual, expected);
        }
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn validate_type1_u16_element(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: u16,
) {
    match element_u16_for_validate(obj, tag) {
        Ok(actual) => validate_equal(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn validate_str_element(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: &str,
) {
    match element_str_for_validate(obj, tag) {
        Ok(actual) => validate_equal(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn top_level_sequence_item_for_validate(
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    index: usize,
) -> Result<&DatasetObject, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .items()
        .ok_or_else(|| format!("attribute {tag} is not a sequence"))?
        .get(index)
        .ok_or_else(|| format!("sequence {tag} has no item at index {index}"))
}

fn item_sequence_item_for_validate(
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    index: usize,
) -> Result<&DatasetObject, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .items()
        .ok_or_else(|| format!("attribute {tag} is not a sequence"))?
        .get(index)
        .ok_or_else(|| format!("sequence {tag} has no item at index {index}"))
}

fn nested_sequence_item_u16_for_validate(
    obj: &DatasetObject,
    sequence_tag: dicom_core::Tag,
    index: usize,
    tag: dicom_core::Tag,
) -> Result<u16, String> {
    let item = item_sequence_item_for_validate(obj, sequence_tag, index)?;
    item.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_int::<u16>()
        .map_err(|err| err.to_string())
}

fn item_str_for_validate(obj: &DatasetObject, tag: dicom_core::Tag) -> Result<String, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_str()
        .map_err(|err| err.to_string())
        .map(|value| value.trim_matches('\0').trim().to_string())
}

fn element_str_for_validate(obj: &OpenedObject, tag: dicom_core::Tag) -> Result<String, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_str()
        .map_err(|err| err.to_string())
        .map(|value| value.trim_matches('\0').trim().to_string())
}

fn element_u16_for_validate(obj: &OpenedObject, tag: dicom_core::Tag) -> Result<u16, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_int::<u16>()
        .map_err(|err| err.to_string())
}

fn element_u32_for_validate(obj: &OpenedObject, tag: dicom_core::Tag) -> Result<u32, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_int::<u32>()
        .map_err(|err| err.to_string())
}

fn element_tag_for_validate(
    obj: &OpenedObject,
    tag: dicom_core::Tag,
) -> Result<dicom_core::Tag, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .tags()
        .map_err(|err| err.to_string())?
        .first()
        .copied()
        .ok_or_else(|| "element is empty".to_string())
}

fn validate_equal<A: fmt::Display, E: fmt::Display>(
    failures: &mut Vec<String>,
    relative_path: &str,
    name: &str,
    actual: A,
    expected: E,
) {
    if actual.to_string() != expected.to_string() {
        failures.push(format!(
            "{relative_path}: {name}: expected {expected}, got {actual}"
        ));
    }
}

fn vr_name(vr: VR) -> &'static str {
    match vr {
        VR::OB => "OB",
        VR::OW => "OW",
        VR::OF => "OF",
        VR::OD => "OD",
        VR::OL => "OL",
        VR::OV => "OV",
        VR::UN => "UN",
        _ => "other",
    }
}

fn trim_uid(uid: &str) -> String {
    uid.trim_matches('\0').trim().to_string()
}

pub fn build_coverage_report(root_dir: impl AsRef<Path>) -> Result<Value, ReportError> {
    let root_dir = root_dir.as_ref();
    let manifest_path = root_dir.join("manifest.json");
    let manifest = read_report_json(&manifest_path)?;
    let registry_path = Path::new("cases/registry.json");
    let registry = read_report_json(registry_path)?;
    let files =
        manifest
            .get("files")
            .and_then(Value::as_array)
            .ok_or(ReportError::MetadataShape {
                path: manifest_path.clone(),
                message: "missing files array",
            })?;
    let skipped_cases = manifest
        .get("skipped_cases")
        .and_then(Value::as_array)
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.clone(),
            message: "missing skipped_cases array",
        })?;
    let run_profile = report_str(
        &manifest_path,
        &manifest,
        "/run/profile",
        "run profile must be a string",
    )?;

    let mut rows = Vec::new();
    let mut counts = CoverageCounts::default();
    let mut grouped = GroupedCoverage::default();
    for file in files {
        let row = generated_coverage_row(&manifest_path, file, run_profile)?;
        counts.generated += 1;
        grouped.record(&row);
        rows.push(row);
    }
    for skipped in skipped_cases {
        let row = skipped_coverage_row(&manifest_path, &registry, skipped, run_profile)?;
        match row.get("status").and_then(Value::as_str).unwrap_or("") {
            "planned" => counts.planned += 1,
            "blocked" => counts.blocked += 1,
            "deprecated" => counts.deprecated += 1,
            _ => counts.skipped += 1,
        }
        grouped.record(&row);
        rows.push(row);
    }

    let gaps = skipped_cases
        .iter()
        .map(|case| {
            serde_json::json!({
                "axis": "case",
                "value": case.get("case_id").and_then(Value::as_str).unwrap_or(""),
                "reason": case.get("message").and_then(Value::as_str).unwrap_or("not generated"),
                "recommended_case_id": case.get("case_id").and_then(Value::as_str).unwrap_or("")
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "coverage_report_schema_version": "0.1.0",
        "generated_at": manifest.get("generated_at").and_then(Value::as_str).unwrap_or("19700101000000.000000+0000"),
        "standards_lock_sha256": manifest.pointer("/standards/standards_lock_sha256").and_then(Value::as_str).unwrap_or("0000000000000000000000000000000000000000000000000000000000000000"),
        "counts": {
            "generated": counts.generated,
            "skipped": counts.skipped,
            "blocked": counts.blocked,
            "planned": counts.planned,
            "deprecated": counts.deprecated
        },
        "coverage_matrix": rows,
        "grouped_coverage": grouped.to_json(),
        "gaps": gaps
    }))
}

pub fn render_coverage_report_markdown(report: &Value) -> String {
    let mut output = String::new();
    output.push_str("# DICOM Test Suite Coverage Report\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n",
        markdown_cell(report.get("generated_at").and_then(Value::as_str))
    ));
    output.push_str(&format!(
        "- Standards lock SHA-256: {}\n",
        markdown_cell(report.get("standards_lock_sha256").and_then(Value::as_str))
    ));

    output.push_str("\n## Counts\n\n");
    output.push_str("| Status | Count |\n");
    output.push_str("|---|---:|\n");
    for status in ["generated", "planned", "skipped", "blocked", "deprecated"] {
        output.push_str(&format!(
            "| {} | {} |\n",
            status,
            report
                .pointer(&format!("/counts/{status}"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
    }

    output.push_str("\n## Grouped Coverage\n\n");
    append_count_map_section(
        &mut output,
        report,
        "Profiles",
        "/grouped_coverage/profiles",
    );
    append_count_map_section(&mut output, report, "IODs", "/grouped_coverage/iods");
    append_count_map_section(
        &mut output,
        report,
        "Transfer Syntaxes",
        "/grouped_coverage/transfer_syntaxes",
    );
    append_count_map_section(
        &mut output,
        report,
        "Photometric Interpretations",
        "/grouped_coverage/photometric_interpretations",
    );
    append_count_map_section(
        &mut output,
        report,
        "Bit Depths",
        "/grouped_coverage/bit_depths",
    );
    append_count_map_section(
        &mut output,
        report,
        "Object Types",
        "/grouped_coverage/object_types",
    );

    output.push_str("## Gaps\n\n");
    let gaps = report
        .get("gaps")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if gaps.is_empty() {
        output.push_str("No gaps reported.\n\n");
    } else {
        output.push_str("| Axis | Value | Reason | Recommended case |\n");
        output.push_str("|---|---|---|---|\n");
        for gap in gaps {
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                markdown_cell(gap.get("axis").and_then(Value::as_str)),
                markdown_cell(gap.get("value").and_then(Value::as_str)),
                markdown_cell(gap.get("reason").and_then(Value::as_str)),
                markdown_cell(gap.get("recommended_case_id").and_then(Value::as_str))
            ));
        }
        output.push('\n');
    }

    output.push_str("## Coverage Matrix\n\n");
    output.push_str("| Case ID | Status | Profile | IOD | Transfer Syntax | Photometric | Bits | Frames | Validation |\n");
    output.push_str("|---|---|---|---|---|---|---:|---:|---|\n");
    if let Some(rows) = report.get("coverage_matrix").and_then(Value::as_array) {
        for row in rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("status").and_then(Value::as_str)),
                markdown_cell(row.get("profile").and_then(Value::as_str)),
                markdown_cell(row.get("iod").and_then(Value::as_str)),
                markdown_cell(row.get("transfer_syntax").and_then(Value::as_str)),
                markdown_cell(row.get("photometric").and_then(Value::as_str)),
                markdown_number(row.get("bits")),
                markdown_number(row.get("frames")),
                markdown_cell(row.get("validation_status").and_then(Value::as_str))
            ));
        }
    }

    output
}

fn read_report_json(path: &Path) -> Result<Value, ReportError> {
    let contents = fs::read_to_string(path).map_err(|source| ReportError::ReadMetadata {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| ReportError::ParseMetadata {
        path: path.to_path_buf(),
        source,
    })
}

fn generated_coverage_row(
    manifest_path: &Path,
    file: &Value,
    run_profile: &str,
) -> Result<Value, ReportError> {
    let derived_refs = manifest_reference_case_ids(manifest_path, file)?;
    Ok(serde_json::json!({
        "case_id": report_str(manifest_path, file, "/case_id", "file case_id must be a string")?,
        "profile": run_profile,
        "status": "generated",
        "iod": report_str(manifest_path, file, "/dicom/iod_name", "dicom iod_name must be a string")?,
        "sop_class_uid": report_str(manifest_path, file, "/dicom/sop_class_uid", "dicom sop_class_uid must be a string")?,
        "transfer_syntax": report_str(manifest_path, file, "/dicom/transfer_syntax_uid", "dicom transfer_syntax_uid must be a string")?,
        "photometric": file.pointer("/image/photometric_interpretation").and_then(Value::as_str),
        "bits": file.pointer("/image/bits_stored").and_then(Value::as_u64),
        "frames": file.pointer("/image/frames").and_then(Value::as_u64),
        "geometry": {
            "rows": file.pointer("/image/rows").and_then(Value::as_u64),
            "columns": file.pointer("/image/columns").and_then(Value::as_u64),
            "spacing": Value::Null,
            "orientation": Value::Null
        },
        "derived_refs": derived_refs,
        "validation_status": file.pointer("/validation/status").and_then(Value::as_str).unwrap_or("not_run"),
        "determinism": report_str(manifest_path, file, "/determinism", "determinism must be a string")?,
        "object_type": file.get("case_id").and_then(Value::as_str).and_then(|case_id| case_id.split('/').next()),
        "known_stressors": file.get("known_stressors").cloned().unwrap_or_else(|| serde_json::json!([]))
    }))
}

fn manifest_reference_case_ids(
    manifest_path: &Path,
    file: &Value,
) -> Result<Vec<String>, ReportError> {
    let references = match file.get("references") {
        Some(Value::Array(references)) => references,
        Some(_) => {
            return Err(ReportError::MetadataShape {
                path: manifest_path.to_path_buf(),
                message: "file references must be an array",
            });
        }
        None => return Ok(Vec::new()),
    };

    references
        .iter()
        .map(|reference| {
            reference
                .get("source_case_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| ReportError::MetadataShape {
                    path: manifest_path.to_path_buf(),
                    message: "file reference source_case_id must be a string",
                })
        })
        .collect()
}

fn skipped_coverage_row(
    manifest_path: &Path,
    registry: &Value,
    skipped: &Value,
    run_profile: &str,
) -> Result<Value, ReportError> {
    let case_id = report_str(
        manifest_path,
        skipped,
        "/case_id",
        "skipped case_id must be a string",
    )?;
    let registry_case =
        registry_case_for_report(registry, case_id).ok_or(ReportError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "skipped case is missing from registry",
        })?;
    let status = match skipped.get("status").and_then(Value::as_str) {
        Some("blocked") => "blocked",
        Some("skipped") => "skipped",
        Some("unavailable")
            if matches!(
                skipped.get("reason_code").and_then(Value::as_str),
                Some("case_planned" | "feature_gated_case_planned")
            ) =>
        {
            "planned"
        }
        Some("unavailable") => "unavailable",
        _ => "skipped",
    };

    Ok(serde_json::json!({
        "case_id": case_id,
        "profile": run_profile,
        "status": status,
        "iod": registry_case.get("iod_name").and_then(Value::as_str).unwrap_or(""),
        "sop_class_uid": registry_case.get("sop_class_uid").and_then(Value::as_str).unwrap_or(""),
        "transfer_syntax": registry_case.get("transfer_syntax_uid").and_then(Value::as_str).unwrap_or(""),
        "photometric": Value::Null,
        "bits": Value::Null,
        "frames": Value::Null,
        "geometry": {
            "rows": Value::Null,
            "columns": Value::Null,
            "spacing": Value::Null,
            "orientation": Value::Null
        },
        "derived_refs": [],
        "validation_status": "unavailable",
        "determinism": registry_case.get("determinism").and_then(Value::as_str).unwrap_or("byte_stable"),
        "object_type": case_id.split('/').next(),
        "known_stressors": []
    }))
}

fn registry_case_for_report<'a>(registry: &'a Value, case_id: &str) -> Option<&'a Value> {
    registry
        .get("cases")
        .and_then(Value::as_array)?
        .iter()
        .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
}

fn report_str<'a>(
    path: &Path,
    value: &'a Value,
    pointer: &str,
    message: &'static str,
) -> Result<&'a str, ReportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or(ReportError::MetadataShape {
            path: path.to_path_buf(),
            message,
        })
}

#[derive(Default)]
struct CoverageCounts {
    generated: usize,
    skipped: usize,
    blocked: usize,
    planned: usize,
    deprecated: usize,
}

#[derive(Default)]
struct GroupedCoverage {
    profiles: BTreeMap<String, usize>,
    iods: BTreeMap<String, usize>,
    sop_classes: BTreeMap<String, usize>,
    transfer_syntaxes: BTreeMap<String, usize>,
    photometric_interpretations: BTreeMap<String, usize>,
    bit_depths: BTreeMap<String, usize>,
    object_types: BTreeMap<String, usize>,
}

impl GroupedCoverage {
    fn record(&mut self, row: &Value) {
        increment_map(
            &mut self.profiles,
            row.get("profile").and_then(Value::as_str),
        );
        increment_map(&mut self.iods, row.get("iod").and_then(Value::as_str));
        increment_map(
            &mut self.sop_classes,
            row.get("sop_class_uid").and_then(Value::as_str),
        );
        increment_map(
            &mut self.transfer_syntaxes,
            row.get("transfer_syntax").and_then(Value::as_str),
        );
        increment_map(
            &mut self.photometric_interpretations,
            row.get("photometric").and_then(Value::as_str),
        );
        if let Some(bits) = row.get("bits").and_then(Value::as_u64) {
            *self.bit_depths.entry(bits.to_string()).or_default() += 1;
        }
        increment_map(
            &mut self.object_types,
            row.get("object_type").and_then(Value::as_str),
        );
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "profiles": self.profiles,
            "iods": self.iods,
            "sop_classes": self.sop_classes,
            "transfer_syntaxes": self.transfer_syntaxes,
            "photometric_interpretations": self.photometric_interpretations,
            "bit_depths": self.bit_depths,
            "object_types": self.object_types
        })
    }
}

fn increment_map(map: &mut BTreeMap<String, usize>, key: Option<&str>) {
    if let Some(key) = key {
        *map.entry(key.to_string()).or_default() += 1;
    }
}

fn append_count_map_section(output: &mut String, report: &Value, title: &str, pointer: &str) {
    output.push_str(&format!("### {title}\n\n"));
    output.push_str("| Value | Count |\n");
    output.push_str("|---|---:|\n");
    if let Some(map) = report.pointer(pointer).and_then(Value::as_object) {
        for (value, count) in map {
            output.push_str(&format!(
                "| {} | {} |\n",
                markdown_cell(Some(value.as_str())),
                count.as_u64().unwrap_or(0)
            ));
        }
    }
    output.push('\n');
}

fn markdown_cell(value: Option<&str>) -> String {
    value
        .unwrap_or("")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn markdown_number(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_u64)
        .map_or_else(String::new, |number| number.to_string())
}

pub fn check_standards_lock_path(
    path: impl AsRef<Path>,
) -> Result<StandardsLockSummary, StandardsError> {
    let path = path.as_ref();
    let lock = read_standards_json(path)?;

    let schema_version = required_standards_str(path, &lock, "/schema_version")?;
    require_standards_value(path, "schema_version", &schema_version, "0.1.0")?;
    let dicom_base_edition = required_standards_str(path, &lock, "/dicom_base_edition")?;
    require_standards_value(path, "dicom_base_edition", &dicom_base_edition, "2026b")?;
    let include_final_text_after_base =
        required_standards_bool(path, &lock, "/include_final_text_after_base")?;
    if include_final_text_after_base {
        return Err(standards_shape(
            path,
            "include_final_text_after_base must be false for the current 2026b base policy",
        ));
    }
    require_non_empty_standards_str(path, &lock, "/final_text_policy")?;
    require_non_empty_standards_str(path, &lock, "/verified_at")?;
    require_non_empty_standards_str(path, &lock, "/official_source_policy")?;

    let kb = lock
        .get("dicom_standard_kb")
        .and_then(Value::as_object)
        .ok_or_else(|| standards_shape(path, "missing dicom_standard_kb object"))?;
    let kb_value = Value::Object(kb.clone());
    let kb_repository = required_standards_str(path, &kb_value, "/repository")?;
    let kb_db_edition = required_standards_str(path, &kb_value, "/db_edition")?;
    require_standards_value(
        path,
        "dicom_standard_kb.db_edition",
        &kb_db_edition,
        "2026b",
    )?;
    let kb_source_manifest_sha256 =
        required_standards_str(path, &kb_value, "/source_manifest_sha256")?;
    require_sha256(
        path,
        "dicom_standard_kb.source_manifest_sha256",
        &kb_source_manifest_sha256,
    )?;
    let parser_surface = kb
        .get("parser_surface")
        .and_then(Value::as_array)
        .ok_or_else(|| standards_shape(path, "missing dicom_standard_kb.parser_surface array"))?;
    for required_part in ["PS3.3", "PS3.4", "PS3.6"] {
        if !parser_surface
            .iter()
            .any(|part| part.as_str() == Some(required_part))
        {
            return Err(standards_shape(
                path,
                format!("dicom_standard_kb.parser_surface must include {required_part}"),
            ));
        }
    }
    let pin_status = kb
        .get("pin_status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let mut warnings = Vec::new();
    require_documented_nullable_pin(
        path,
        &kb_value,
        "/commit",
        "/commit_status",
        "/commit_unavailable_reason",
        "dicom_standard_kb.commit",
        &mut warnings,
    )?;
    require_documented_nullable_pin(
        path,
        &kb_value,
        "/db_sha256",
        "/db_sha256_status",
        "/db_sha256_unavailable_reason",
        "dicom_standard_kb.db_sha256",
        &mut warnings,
    )?;
    if !pin_status.is_empty() {
        // Retained as human-readable summary, but field-specific statuses above
        // are the validation contract for nullable reproducibility pins.
        require_non_empty_standards_str(path, &kb_value, "/pin_status")?;
    }

    let source_artifacts = lock
        .get("source_artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| standards_shape(path, "missing source_artifacts array"))?;
    if source_artifacts.is_empty() {
        return Err(standards_shape(path, "source_artifacts must not be empty"));
    }
    for artifact in source_artifacts {
        validate_source_artifact(path, artifact, &mut warnings)?;
    }

    let verification_queries = lock
        .get("verification_queries")
        .and_then(Value::as_array)
        .ok_or_else(|| standards_shape(path, "missing verification_queries array"))?;
    if verification_queries.is_empty() {
        return Err(standards_shape(
            path,
            "verification_queries must not be empty",
        ));
    }
    for query in verification_queries {
        validate_verification_query(path, query)?;
    }

    let notes = lock
        .get("notes")
        .and_then(Value::as_array)
        .ok_or_else(|| standards_shape(path, "missing notes array"))?;
    if notes.is_empty() || !notes.iter().all(|note| note.as_str().is_some()) {
        return Err(standards_shape(
            path,
            "notes must contain at least one string entry",
        ));
    }

    Ok(StandardsLockSummary {
        path: path.to_path_buf(),
        schema_version,
        dicom_base_edition,
        include_final_text_after_base,
        kb_repository,
        kb_db_edition,
        kb_source_manifest_sha256,
        source_artifacts: source_artifacts.len(),
        verification_queries: verification_queries.len(),
        warnings,
    })
}

pub fn format_standards_lock_summary(summary: &StandardsLockSummary) -> String {
    let mut output = String::new();
    output.push_str("status\tok\n");
    output.push_str(&format!("path\t{}\n", summary.path.display()));
    output.push_str(&format!("schema_version\t{}\n", summary.schema_version));
    output.push_str(&format!(
        "dicom_base_edition\t{}\n",
        summary.dicom_base_edition
    ));
    output.push_str(&format!(
        "include_final_text_after_base\t{}\n",
        summary.include_final_text_after_base
    ));
    output.push_str(&format!("kb_repository\t{}\n", summary.kb_repository));
    output.push_str(&format!("kb_db_edition\t{}\n", summary.kb_db_edition));
    output.push_str(&format!(
        "kb_source_manifest_sha256\t{}\n",
        summary.kb_source_manifest_sha256
    ));
    output.push_str(&format!("source_artifacts\t{}\n", summary.source_artifacts));
    output.push_str(&format!(
        "verification_queries\t{}\n",
        summary.verification_queries
    ));
    output.push_str(&format!("warnings\t{}\n", summary.warnings.len()));
    for warning in &summary.warnings {
        output.push_str(&format!("warning\t{warning}\n"));
    }
    output
}

fn read_standards_json(path: &Path) -> Result<Value, StandardsError> {
    let contents = fs::read_to_string(path).map_err(|source| StandardsError::ReadMetadata {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| StandardsError::ParseMetadata {
        path: path.to_path_buf(),
        source,
    })
}

fn required_standards_str(
    path: &Path,
    value: &Value,
    pointer: &str,
) -> Result<String, StandardsError> {
    let string = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| standards_shape(path, format!("{pointer} must be a string")))?;
    if string.trim().is_empty() {
        return Err(standards_shape(
            path,
            format!("{pointer} must not be empty"),
        ));
    }
    Ok(string.to_string())
}

fn require_non_empty_standards_str(
    path: &Path,
    value: &Value,
    pointer: &str,
) -> Result<(), StandardsError> {
    required_standards_str(path, value, pointer).map(|_| ())
}

fn required_standards_bool(
    path: &Path,
    value: &Value,
    pointer: &str,
) -> Result<bool, StandardsError> {
    value
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or_else(|| standards_shape(path, format!("{pointer} must be a boolean")))
}

fn require_standards_value(
    path: &Path,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), StandardsError> {
    if actual != expected {
        return Err(standards_shape(
            path,
            format!("{field} must be {expected}, got {actual}"),
        ));
    }
    Ok(())
}

fn require_sha256(path: &Path, field: &str, value: &str) -> Result<(), StandardsError> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(standards_shape(
            path,
            format!("{field} must be a 64-character SHA-256 hex digest"),
        ));
    }
    Ok(())
}

fn require_documented_nullable_pin(
    path: &Path,
    value: &Value,
    pointer: &str,
    status_pointer: &str,
    reason_pointer: &str,
    field: &str,
    warnings: &mut Vec<String>,
) -> Result<(), StandardsError> {
    match value.pointer(pointer) {
        Some(Value::Null) => {
            let status = required_standards_str(path, value, status_pointer)?;
            if status != "unavailable" {
                return Err(standards_shape(
                    path,
                    format!(
                        "{field} is null but {status_pointer} is {status}, expected unavailable"
                    ),
                ));
            }
            let reason = required_standards_str(path, value, reason_pointer)?;
            warnings.push(format!("{field} unavailable: {reason}"));
            Ok(())
        }
        Some(Value::String(text)) if pointer.ends_with("sha256") || field.ends_with("sha256") => {
            require_sha256(path, field, text)
        }
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(()),
        Some(_) => Err(standards_shape(
            path,
            format!("{field} must be a string or null"),
        )),
        None => Err(standards_shape(path, format!("{field} is missing"))),
    }
}

fn validate_source_artifact(
    path: &Path,
    artifact: &Value,
    warnings: &mut Vec<String>,
) -> Result<(), StandardsError> {
    let part = required_standards_str(path, artifact, "/part")?;
    let format = required_standards_str(path, artifact, "/format")?;
    let status = required_standards_str(path, artifact, "/status")?;
    match artifact.get("sha256") {
        Some(Value::Null) => {
            if !status.starts_with("unavailable_") {
                return Err(standards_shape(
                    path,
                    format!(
                        "source_artifact.{part}.{format}.sha256 is null but status is {status}"
                    ),
                ));
            }
            let reason = required_standards_str(path, artifact, "/unavailable_reason")?;
            warnings.push(format!(
                "source_artifact.{part}.{format} sha256 unavailable: {reason}"
            ));
        }
        Some(Value::String(sha256)) => require_sha256(
            path,
            &format!("source_artifact.{part}.{format}.sha256"),
            sha256,
        )?,
        Some(_) => {
            return Err(standards_shape(
                path,
                format!("source_artifact.{part}.{format}.sha256 must be a string or null"),
            ));
        }
        None => {
            return Err(standards_shape(
                path,
                format!("source_artifact.{part}.{format}.sha256 is missing"),
            ));
        }
    }
    Ok(())
}

fn validate_verification_query(path: &Path, query: &Value) -> Result<(), StandardsError> {
    for field in ["source", "edition", "query", "result", "official_url"] {
        require_non_empty_standards_str(path, query, &format!("/{field}"))?;
    }
    require_standards_value(
        path,
        "verification_queries[].edition",
        &required_standards_str(path, query, "/edition")?,
        "2026b",
    )
}

fn standards_shape(path: &Path, message: impl Into<String>) -> StandardsError {
    StandardsError::MetadataShape {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn build_generation_manifest(
    run: &PreparedGenerationRun,
    standards_lock: &Value,
    standards_lock_bytes: &[u8],
    cargo_lock: &[u8],
    registry: &Value,
    generated_files: Vec<generator::GeneratedFile>,
    generated_case_ids: &[String],
) -> Result<Value, GenerateError> {
    let skipped_cases = skipped_cases_for_run(registry, run, generated_case_ids)?;
    let dicom_standard_kb = standards_lock
        .get("dicom_standard_kb")
        .cloned()
        .unwrap_or(Value::Null);
    let file_entries: Vec<Value> = generated_files
        .into_iter()
        .map(|file| file_manifest_entry_with_reference_defaults(file.manifest_entry))
        .collect();

    Ok(serde_json::json!({
        "manifest_schema_version": "0.1.0",
        "generated_at": "19700101000000.000000+0000",
        "generator": {
            "name": PACKAGE_NAME,
            "version": PACKAGE_VERSION,
            "git_sha": Value::Null,
            "rustc_version": RUSTC_VERSION,
            "target_triple": TARGET_TRIPLE,
            "cargo_lock_sha256": sha256_hex(cargo_lock),
            "feature_flags": ACTIVE_FEATURE_FLAGS
        },
        "standards": {
            "dicom_base_edition": standards_lock.get("dicom_base_edition").and_then(Value::as_str).unwrap_or("2026b"),
            "include_final_text_after_base": standards_lock.get("include_final_text_after_base").and_then(Value::as_bool).unwrap_or(false),
            "standards_lock_sha256": sha256_hex(&standards_lock_bytes),
            "dicom_standard_kb": {
                "commit": dicom_standard_kb.get("commit").cloned().unwrap_or(Value::Null),
                "db_edition": dicom_standard_kb.get("db_edition").and_then(Value::as_str).unwrap_or("2026b"),
                "db_sha256": dicom_standard_kb.get("db_sha256").cloned().unwrap_or(Value::Null),
                "source_manifest_sha256": dicom_standard_kb.get("source_manifest_sha256").cloned().unwrap_or(Value::Null)
            }
        },
        "dependencies": {
            "dicom_rs_versions": {
                "dicom-core": "0.9.1",
                "dicom-dictionary-std": "0.9.0",
                "dicom-object": "0.9.1",
                "dicom-transfer-syntax-registry": "0.9.1"
            },
            "codec_versions": {}
        },
        "run": {
            "profile": run.profile,
            "seed": run.seed,
            "include_stress": run.include_stress
        },
        "files": file_entries,
        "skipped_cases": skipped_cases
    }))
}

fn file_manifest_entry_with_reference_defaults(mut file: Value) -> Value {
    if let Some(object) = file.as_object_mut() {
        object
            .entry("references")
            .or_insert_with(|| serde_json::json!([]));
    }
    file
}

fn skipped_cases_for_run(
    registry: &Value,
    run: &PreparedGenerationRun,
    generated_case_ids: &[String],
) -> Result<Vec<Value>, GenerateError> {
    let cases =
        registry
            .get("cases")
            .and_then(Value::as_array)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "missing cases array",
            })?;

    let mut skipped = Vec::new();
    for case in cases {
        let profiles =
            string_array(case.get("profiles")).map_err(|_| GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "case profiles must be a string array",
            })?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        let case_id = required_str(case, "case_id").map_err(|_| GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case_id must be a string",
        })?;
        if generated_case_ids
            .iter()
            .any(|generated_case_id| generated_case_id == case_id)
        {
            continue;
        }

        let status = required_str(case, "status").map_err(|_| GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case status must be a string",
        })?;

        match status {
            "implemented" if case_missing_required_features(case)? => {
                skipped.push(feature_gated_skipped_case_from_registry(case_id, case)?);
            }
            "implemented" => skipped.push(serde_json::json!({
                "case_id": case_id,
                "status": "unavailable",
                "reason_code": "generator_not_implemented",
                "message": "This implemented registry case does not have a generator recipe.",
                "recheck_phase": "remediation-r1",
                "standards_evidence": case.get("standards_evidence").cloned().unwrap_or_else(|| serde_json::json!([]))
            })),
            "planned" => skipped.push(planned_skipped_case_from_registry(case_id, case)?),
            "skipped" | "blocked" => skipped.push(skipped_case_from_registry(case_id, status, case)?),
            "deprecated" => {}
            _ => {
                return Err(GenerateError::MetadataShape {
                    path: PathBuf::from("cases/registry.json"),
                    message: "case status must be planned, implemented, skipped, blocked, or deprecated",
                });
            }
        }
    }

    Ok(skipped)
}

fn planned_skipped_case_from_registry(case_id: &str, case: &Value) -> Result<Value, GenerateError> {
    let required_features = string_array(case.pointer("/requirements/features")).map_err(|_| {
        GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case requirements.features must be a string array",
        }
    })?;
    let standards_evidence = case
        .get("standards_evidence")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));

    if required_features.is_empty() {
        return Ok(serde_json::json!({
            "case_id": case_id,
            "status": "unavailable",
            "reason_code": "case_planned",
            "message": "This planned registry case does not have an implemented generator recipe yet.",
            "recheck_phase": planned_recheck_phase(case_id),
            "standards_evidence": standards_evidence
        }));
    }

    let missing_features = missing_required_features(&required_features);
    let features = required_features.join(", ");
    let missing = if missing_features.is_empty() {
        "no required features are missing in this build".to_string()
    } else {
        format!("missing active feature(s): {}", missing_features.join(", "))
    };

    Ok(serde_json::json!({
        "case_id": case_id,
        "status": "unavailable",
        "reason_code": "feature_gated_case_planned",
        "message": format!("This planned registry case requires Cargo feature(s) {features}; {missing}; deflated dataset generation remains unavailable until write/read validation and reproducibility are implemented."),
        "recheck_phase": planned_recheck_phase(case_id),
        "standards_evidence": standards_evidence
    }))
}

fn feature_gated_skipped_case_from_registry(
    case_id: &str,
    case: &Value,
) -> Result<Value, GenerateError> {
    let required_features = string_array(case.pointer("/requirements/features")).map_err(|_| {
        GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case requirements.features must be a string array",
        }
    })?;
    let missing_features = missing_required_features(&required_features);
    let features = required_features.join(", ");
    let missing = if missing_features.is_empty() {
        "no required features are missing in this build".to_string()
    } else {
        format!("missing active feature(s): {}", missing_features.join(", "))
    };

    Ok(serde_json::json!({
        "case_id": case_id,
        "status": "unavailable",
        "reason_code": "feature_gated_case_unavailable",
        "message": format!("This implemented registry case requires Cargo feature(s) {features}; {missing}."),
        "recheck_phase": planned_recheck_phase(case_id),
        "standards_evidence": case.get("standards_evidence").cloned().unwrap_or_else(|| serde_json::json!([]))
    }))
}

fn case_missing_required_features(case: &Value) -> Result<bool, GenerateError> {
    let required_features = string_array(case.pointer("/requirements/features")).map_err(|_| {
        GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case requirements.features must be a string array",
        }
    })?;
    Ok(!missing_required_features(&required_features).is_empty())
}

fn missing_required_features(required_features: &[String]) -> Vec<String> {
    required_features
        .iter()
        .filter(|feature| !ACTIVE_FEATURE_FLAGS.contains(&feature.as_str()))
        .cloned()
        .collect()
}

fn skipped_case_from_registry(
    case_id: &str,
    status: &str,
    case: &Value,
) -> Result<Value, GenerateError> {
    let skip = case
        .get("skip")
        .and_then(Value::as_object)
        .ok_or(GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "skipped or blocked cases must include a skip object",
        })?;
    let reason_code =
        skip.get("reason_code")
            .and_then(Value::as_str)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "skip reason_code must be a string",
            })?;
    let message =
        skip.get("message")
            .and_then(Value::as_str)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "skip message must be a string",
            })?;
    let recheck_phase = skip.get("recheck_phase").cloned().unwrap_or(Value::Null);

    Ok(serde_json::json!({
        "case_id": case_id,
        "status": status,
        "reason_code": reason_code,
        "message": message,
        "recheck_phase": recheck_phase,
        "standards_evidence": case.get("standards_evidence").cloned().unwrap_or_else(|| serde_json::json!([]))
    }))
}

fn planned_recheck_phase(case_id: &str) -> &'static str {
    if case_id.contains("deflated") {
        "phase-6"
    } else if case_id.starts_with("derived/") {
        "phase-5"
    } else if case_id.starts_with("vl/") {
        "phase-7"
    } else {
        "phase-5"
    }
}

fn case_matches_profile(profiles: &[String], requested: &str, include_stress: bool) -> bool {
    match requested {
        "all" => profiles.iter().any(|profile| {
            matches!(profile.as_str(), "smoke" | "core" | "extended")
                || (include_stress && profile == "stress")
        }),
        profile => profiles.iter().any(|case_profile| case_profile == profile),
    }
}

fn read_json_metadata(path: impl AsRef<Path>) -> Result<Value, GenerateError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| GenerateError::ReadMetadata {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| GenerateError::ParseMetadata {
        path: path.to_path_buf(),
        source,
    })
}

fn read_bytes_metadata(path: impl AsRef<Path>) -> Result<Vec<u8>, GenerateError> {
    let path = path.as_ref();
    fs::read(path).map_err(|source| GenerateError::ReadMetadata {
        path: path.to_path_buf(),
        source,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    let mut h = H0;
    for chunk in data.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let start = i * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[derive(Debug)]
pub enum CaseRegistryError {
    Read {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: serde_json::Error,
    },
    InvalidProfile(String),
    InvalidStatus(String),
    Shape(&'static str),
}

impl fmt::Display for CaseRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read case registry {path}: {source}")
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse case registry {path}: {source}")
            }
            Self::InvalidProfile(profile) => write!(
                f,
                "unsupported profile {profile}; expected one of {}",
                SUPPORTED_PROFILES.join(", ")
            ),
            Self::InvalidStatus(status) => write!(
                f,
                "unsupported case status {status}; expected one of {}",
                SUPPORTED_CASE_STATUSES.join(", ")
            ),
            Self::Shape(message) => write!(f, "invalid case registry shape: {message}"),
        }
    }
}

impl Error for CaseRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::InvalidProfile(_) => None,
            Self::InvalidStatus(_) => None,
            Self::Shape(_) => None,
        }
    }
}

pub fn list_cases_from_registry_path(
    registry_path: impl AsRef<Path>,
    profile_filter: Option<&str>,
    status_filter: Option<&str>,
) -> Result<String, CaseRegistryError> {
    let registry_path = registry_path.as_ref();
    let path_display = registry_path.display().to_string();
    let contents = fs::read_to_string(registry_path).map_err(|source| CaseRegistryError::Read {
        path: path_display.clone(),
        source,
    })?;
    let registry: Value =
        serde_json::from_str(&contents).map_err(|source| CaseRegistryError::Parse {
            path: path_display,
            source,
        })?;

    list_cases_from_registry_value(&registry, profile_filter, status_filter)
}

pub fn standards_gaps_from_registry_path(
    registry_path: impl AsRef<Path>,
    profile_filter: &str,
) -> Result<String, CaseRegistryError> {
    if !SUPPORTED_PROFILES.contains(&profile_filter) {
        return Err(CaseRegistryError::InvalidProfile(
            profile_filter.to_string(),
        ));
    }

    let registry_path = registry_path.as_ref();
    let path_display = registry_path.display().to_string();
    let contents = fs::read_to_string(registry_path).map_err(|source| CaseRegistryError::Read {
        path: path_display.clone(),
        source,
    })?;
    let registry: Value =
        serde_json::from_str(&contents).map_err(|source| CaseRegistryError::Parse {
            path: path_display,
            source,
        })?;

    standards_gaps_from_registry_value(&registry, profile_filter)
}

pub fn list_cases_from_registry_value(
    registry: &Value,
    profile_filter: Option<&str>,
    status_filter: Option<&str>,
) -> Result<String, CaseRegistryError> {
    if let Some(status_filter) = status_filter {
        if !SUPPORTED_CASE_STATUSES.contains(&status_filter) {
            return Err(CaseRegistryError::InvalidStatus(status_filter.to_string()));
        }
    }

    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .ok_or(CaseRegistryError::Shape("missing cases array"))?;

    let mut output = String::from(
        "case_id\tstatus\tprofiles\tsop_class_uid\ttransfer_syntax_uid\tstandards_evidence\n",
    );

    for case in cases {
        let profiles = string_array(case.get("profiles"))?;
        if let Some(profile_filter) = profile_filter {
            if !profiles.iter().any(|profile| profile == profile_filter) {
                continue;
            }
        }

        let case_id = required_str(case, "case_id")?;
        let status = required_str(case, "status")?;
        if let Some(status_filter) = status_filter {
            if status != status_filter {
                continue;
            }
        }
        let sop_class_uid = required_str(case, "sop_class_uid")?;
        let transfer_syntax_uid = required_str(case, "transfer_syntax_uid")?;
        let evidence = case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .ok_or(CaseRegistryError::Shape("missing standards_evidence array"))?;
        let covered = evidence
            .iter()
            .filter(|entry| entry.get("covered").and_then(Value::as_bool) == Some(true))
            .count();

        output.push_str(&format!(
            "{case_id}\t{status}\t{}\t{sop_class_uid}\t{transfer_syntax_uid}\t{covered}/{} covered\n",
            profiles.join(","),
            evidence.len()
        ));
    }

    Ok(output)
}

fn standards_gaps_from_registry_value(
    registry: &Value,
    profile_filter: &str,
) -> Result<String, CaseRegistryError> {
    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .ok_or(CaseRegistryError::Shape("missing cases array"))?;

    let mut output = String::from("case_id\tstatus\tprofiles\tgap_kind\treason\n");
    for case in cases {
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, profile_filter, true) {
            continue;
        }
        let case_id = required_str(case, "case_id")?;
        let status = required_str(case, "status")?;
        for gap in standards_gaps_for_case(case, status)? {
            output.push_str(&format!(
                "{case_id}\t{status}\t{}\t{}\t{}\n",
                profiles.join(","),
                gap.kind,
                gap.reason
            ));
        }
    }

    Ok(output)
}

struct StandardsGap {
    kind: String,
    reason: String,
}

fn standards_gaps_for_case(
    case: &Value,
    status: &str,
) -> Result<Vec<StandardsGap>, CaseRegistryError> {
    let mut gaps = Vec::new();
    if status == "blocked" || status == "skipped" {
        let reason = case
            .get("skip")
            .and_then(Value::as_object)
            .and_then(|skip| skip.get("reason_code"))
            .and_then(Value::as_str)
            .unwrap_or("status_requires_skip_metadata")
            .to_string();
        gaps.push(StandardsGap {
            kind: status.to_string(),
            reason,
        });
    }

    let evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .ok_or(CaseRegistryError::Shape("missing standards_evidence array"))?;
    if evidence.is_empty() {
        gaps.push(StandardsGap {
            kind: "missing_standards_evidence".to_string(),
            reason: "case has no standards evidence entries".to_string(),
        });
        return Ok(gaps);
    }

    let covered_count = evidence
        .iter()
        .filter(|entry| entry.get("covered").and_then(Value::as_bool) == Some(true))
        .count();
    if covered_count == 0 {
        gaps.push(StandardsGap {
            kind: "incomplete_standards_evidence".to_string(),
            reason: "case has no covered standards evidence entries".to_string(),
        });
    }

    for entry in evidence {
        let source = entry.get("source").and_then(Value::as_str).unwrap_or("");
        let query = entry.get("query").and_then(Value::as_str).unwrap_or("");
        if entry.get("covered").and_then(Value::as_bool) != Some(true) {
            gaps.push(StandardsGap {
                kind: "uncovered_standards_evidence".to_string(),
                reason: format!("{} is not covered", evidence_label(source, query)),
            });
        }
        if source.contains("source-note") || source.contains("local-source") {
            gaps.push(StandardsGap {
                kind: "source_note_backed".to_string(),
                reason: evidence_label(source, query),
            });
        }
    }

    Ok(gaps)
}

fn evidence_label(source: &str, query: &str) -> String {
    if query.is_empty() {
        source.to_string()
    } else {
        format!("{source}:{query}")
    }
}

fn required_str<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, CaseRegistryError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(CaseRegistryError::Shape(field))
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, CaseRegistryError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(CaseRegistryError::Shape("missing string array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(CaseRegistryError::Shape("array item is not a string"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_dictionary_std::uids;
    use dicom_object::InMemDicomObject;
    use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

    #[test]
    fn version_banner_uses_package_metadata() {
        assert_eq!(version_banner(), "dicom-test-suite 0.1.0");
    }

    #[test]
    fn pinned_dicom_rs_crates_expose_phase_one_primitives() {
        let _obj = InMemDicomObject::new_empty();

        let explicit_vr_le = TransferSyntaxRegistry
            .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .expect("Explicit VR Little Endian must be available for Part 10 smoke cases");

        assert_eq!(explicit_vr_le.uid(), uids::EXPLICIT_VR_LITTLE_ENDIAN);
        assert_eq!(uids::VERIFICATION, "1.2.840.10008.1.1");
    }

    #[test]
    fn list_cases_shows_committed_smoke_case_status_and_evidence() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("smoke"), None)
            .expect("smoke case registry should list");

        assert!(
            output.contains(
                "classic/sc/mono2_u8_explicit_le\timplemented\tsmoke\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t2/2 covered"
            ),
            "list-cases output must show smoke status and standards evidence coverage"
        );
    }

    #[test]
    fn list_cases_shows_committed_core_case_status_and_evidence() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("core"), None)
            .expect("core case registry should list");

        assert!(
            output.contains(
                "classic/ct/mono2_i16_rescale_12bit_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.2\t1.2.840.10008.1.2.1\t11/11 covered"
            ),
            "list-cases output must show core status and standards evidence coverage"
        );
        assert!(
            output.contains(
                "classic/mg/for_presentation_mono1_u16_12bit_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1.2\t1.2.840.10008.1.2.1\t14/14 covered"
            ),
            "list-cases output must show implemented MG core status"
        );
        assert!(
            output.contains(
                "classic/mg/for_processing_mono2_u16_12bit_implicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1.2.1\t1.2.840.10008.1.2\t15/15 covered"
            ),
            "list-cases output must show implemented MG For Processing core status"
        );
        assert!(
            output.contains(
                "classic/cr/overlay_modality_voi_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1\t1.2.840.10008.1.2.1\t14/14 covered"
            ),
            "list-cases output must show implemented CR overlay/LUT core status"
        );
        assert!(
            output.contains(
                "classic/mr/multislice_oblique_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.4\t1.2.840.10008.1.2.1\t11/11 covered"
            ),
            "list-cases output must show implemented MR multi-slice core status"
        );
        assert!(
            output.contains(
                "classic/dx/display_shutter_mono2_u16_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1.1\t1.2.840.10008.1.2.1\t16/16 covered"
            ),
            "list-cases output must show implemented DX display shutter core status"
        );
        assert!(
            output.contains(
                "classic/us/mono2_u8_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.6.1\t1.2.840.10008.1.2.1\t9/9 covered"
            ),
            "list-cases output must show implemented US core status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t6/6 covered"
            ),
            "list-cases output must show implemented core native pixel status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_i16_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t7/7 covered"
            ),
            "list-cases output must show implemented signed core native pixel status"
        );
        assert!(
            output.contains(
                "classic/sc/rgb_planar1_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
            ),
            "list-cases output must show implemented RGB planar1 core status"
        );
        assert!(
            output.contains(
                "classic/sc/palette_color_u8_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t10/10 covered"
            ),
            "list-cases output must show implemented PALETTE COLOR core status"
        );
        assert!(
            output.contains(
                "classic/sc/ybr_full_planar0_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t6/6 covered"
            ),
            "list-cases output must show implemented YBR_FULL core status"
        );
        assert!(
            output.contains(
                "classic/sc/ybr_full_422_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t7/7 covered"
            ),
            "list-cases output must show implemented YBR_FULL_422 core status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_odd_3x3_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
            ),
            "list-cases output must show implemented odd-dimension core status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_rect_2x3_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
            ),
            "list-cases output must show implemented rectangular core status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_tiny_1x1_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
            ),
            "list-cases output must show implemented tiny-image core status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_padding_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t7/7 covered"
            ),
            "list-cases output must show implemented pixel-padding core status"
        );
    }

    #[test]
    fn list_cases_shows_committed_extended_case_status_and_evidence() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("extended"), None)
            .expect("extended case registry should list");

        assert!(
            output.contains(
                "enhanced/ct/multiframe_shared_perframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.2.1\t1.2.840.10008.1.2.1\t16/16 covered"
            ),
            "list-cases output must show implemented Enhanced CT extended status"
        );
        assert!(
            output.contains(
                "enhanced/ct/concatenation_two_part_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.2.1\t1.2.840.10008.1.2.1\t14/14 covered"
            ),
            "list-cases output must show implemented Enhanced CT concatenation extended status"
        );
        assert!(
            output.contains(
                "enhanced/mr/multiframe_echo_perframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t20/20 covered"
            ),
            "list-cases output must show implemented Enhanced MR extended status"
        );
        assert!(
            output.contains(
                "enhanced/mr/multiframe_temporal_position_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t23/23 covered"
            ),
            "list-cases output must show implemented Enhanced MR temporal extended status"
        );
        assert!(
            output.contains(
                "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t27/27 covered"
            ),
            "list-cases output must show implemented Enhanced MR phase extended status"
        );
        assert!(
            output.contains(
                "derived/seg/binary_multiframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.1\t8/8 covered"
            ),
            "list-cases output must show implemented SEG extended status"
        );
        assert!(
            output.contains(
                "derived/seg/binary_multiframe_deflated_image_frame\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.8.1\t10/10 covered"
            ),
            "list-cases output must show implemented Deflated Image Frame SEG extended status"
        );
        assert!(
            output.contains(
                "derived/seg/fractional_probability_multiframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.1\t8/8 covered"
            ),
            "list-cases output must show implemented fractional SEG extended status"
        );
        assert!(
            output.contains(
                "derived/seg/labelmap_multiframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.7\t1.2.840.10008.1.2.1\t7/7 covered"
            ),
            "list-cases output must show implemented LABELMAP SEG extended status"
        );
        assert!(
            output.contains(
                "non-image/encapsulated-document/pdf_minimal_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.104.1\t1.2.840.10008.1.2.1\t7/7 covered"
            ),
            "list-cases output must show implemented Encapsulated PDF extended status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u8_deflated_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1.99\t2/2 covered"
            ),
            "list-cases output must show implemented deflated transfer syntax status"
        );
    }

    #[test]
    fn list_cases_filters_by_profile_and_status() {
        let output =
            list_cases_from_registry_path("cases/registry.json", Some("extended"), Some("planned"))
                .expect("extended planned cases should list");

        assert!(
            !output.contains("derived/seg/binary_multiframe_explicit_le"),
            "planned status filter should not include implemented SEG"
        );
        assert!(
            !output.contains("derived/seg/binary_multiframe_deflated_image_frame"),
            "planned status filter should not include implemented Deflated Image Frame SEG"
        );
        assert!(
            !output.contains("derived/seg/fractional_probability_multiframe_explicit_le"),
            "planned status filter should not include implemented fractional SEG"
        );
        assert!(
            !output.contains("derived/seg/labelmap_multiframe_explicit_le"),
            "planned status filter should not include implemented LABELMAP SEG"
        );
        assert!(
            !output.contains("derived/sr/basic_text_observation_explicit_le"),
            "planned status filter should not include implemented Basic Text SR"
        );
        assert!(
            !output.contains("derived/sr/comprehensive_measurement_explicit_le"),
            "planned status filter should not include implemented Comprehensive SR"
        );
        assert!(
            !output.contains("derived/sr/key_object_selection_explicit_le"),
            "planned status filter should not include implemented KOS"
        );
        assert!(
            !output.contains("non-image/encapsulated-document/pdf_minimal_explicit_le"),
            "planned status filter should not include implemented Encapsulated PDF"
        );
        assert!(
            !output.contains("enhanced/ct/multiframe_shared_perframe_explicit_le"),
            "status filter should exclude implemented extended cases"
        );
        assert!(
            !output.contains("vl/photo/rgb_planar0_explicit_le"),
            "profile filter should still exclude planned core VL cases"
        );
        assert!(
            !output.contains("classic/sc/mono2_u8_deflated_explicit_le"),
            "planned status filter should exclude implemented feature-gated deflated cases"
        );
    }

    #[test]
    fn list_cases_rejects_unknown_status_filter() {
        let err = list_cases_from_registry_path("cases/registry.json", None, Some("unknown"))
            .expect_err("unknown status should fail");

        assert!(
            err.to_string().contains("unsupported case status unknown"),
            "error should name the unsupported status"
        );
    }

    #[test]
    fn prepare_generation_run_creates_output_root_and_manifest_path() {
        let out_dir = unique_temp_dir("prepare_generation_run");
        let prepared = prepare_generation_run(GenerateOptions {
            profile: "smoke".to_string(),
            out_dir: out_dir.clone(),
            seed: 1,
            include_stress: false,
        })
        .expect("generation run should prepare");

        assert!(out_dir.is_dir(), "prepare must create the output root");
        assert_eq!(prepared.profile, "smoke");
        assert_eq!(prepared.seed, 1);
        assert!(!prepared.include_stress);
        assert_eq!(prepared.manifest_path, out_dir.join("manifest.json"));
        assert!(
            !prepared.manifest_path.exists(),
            "preparing a run must not write a manifest before manifest construction"
        );

        fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
    }

    #[test]
    fn prepare_generation_run_rejects_unknown_profile() {
        let err = prepare_generation_run(GenerateOptions {
            profile: "unknown".to_string(),
            out_dir: unique_temp_dir("reject_unknown_profile"),
            seed: 1,
            include_stress: false,
        })
        .expect_err("unknown profile should be rejected");

        assert!(
            err.to_string().contains("unsupported profile unknown"),
            "error should name the rejected profile"
        );
    }

    #[test]
    fn write_generation_run_records_smoke_file_metadata() {
        let out_dir = unique_temp_dir("write_generation_run");
        let prepared = prepare_generation_run(GenerateOptions {
            profile: "smoke".to_string(),
            out_dir: out_dir.clone(),
            seed: 7,
            include_stress: false,
        })
        .expect("generation run should prepare");

        let summary = write_generation_run(&prepared).expect("manifest should write");

        assert_eq!(summary.files_written, 3);
        assert!(summary.manifest_written);

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(&prepared.manifest_path).expect("manifest should be readable"),
        )
        .expect("manifest should parse");

        assert_eq!(
            manifest
                .get("manifest_schema_version")
                .and_then(Value::as_str),
            Some("0.1.0")
        );
        assert_eq!(
            manifest.pointer("/run/profile").and_then(Value::as_str),
            Some("smoke")
        );
        assert_eq!(
            manifest.pointer("/run/seed").and_then(Value::as_u64),
            Some(7)
        );
        assert_eq!(
            manifest
                .pointer("/files")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
        let generated_case_ids: Vec<&str> = manifest
            .pointer("/files")
            .and_then(Value::as_array)
            .expect("manifest files should be an array")
            .iter()
            .map(|file| {
                file.get("case_id")
                    .and_then(Value::as_str)
                    .expect("file should have case_id")
            })
            .collect();
        assert_eq!(
            generated_case_ids,
            vec![
                "classic/sc/mono2_u8_explicit_le",
                "classic/sc/mono1_u8_explicit_le",
                "classic/sc/rgb_planar0_explicit_le"
            ]
        );
        assert_eq!(
            manifest.pointer("/files/2/path").and_then(Value::as_str),
            Some("classic/sc/rgb_planar0_explicit_le/instance.dcm")
        );
        assert_eq!(
            manifest
                .pointer("/files/0/dicom/sop_class_uid")
                .and_then(Value::as_str),
            Some(uids::SECONDARY_CAPTURE_IMAGE_STORAGE)
        );
        assert!(
            manifest
                .pointer("/skipped_cases")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "manifest should not skip smoke cases once all smoke recipes are generated"
        );

        fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
    }

    #[test]
    fn skipped_cases_use_registry_status_and_skip_metadata() {
        let run = PreparedGenerationRun {
            profile: "core".to_string(),
            out_dir: unique_temp_dir("skipped_status_metadata"),
            manifest_path: unique_temp_dir("skipped_status_metadata").join("manifest.json"),
            seed: 1,
            include_stress: false,
        };
        let registry = serde_json::json!({
            "cases": [
                {
                    "case_id": "classic/sc/generated_explicit_le",
                    "status": "implemented",
                    "profiles": ["core"],
                    "requirements": {
                        "features": [],
                        "external_codecs": [],
                        "external_validators": []
                    },
                    "skip": null,
                    "standards_evidence": []
                },
                {
                    "case_id": "classic/sc/missing_recipe_explicit_le",
                    "status": "implemented",
                    "profiles": ["core"],
                    "requirements": {
                        "features": [],
                        "external_codecs": [],
                        "external_validators": []
                    },
                    "skip": null,
                    "standards_evidence": [{"source": "dicom-standard-kb", "covered": true}]
                },
                {
                    "case_id": "vl/photo/rgb_planar0_explicit_le",
                    "status": "planned",
                    "profiles": ["core"],
                    "requirements": {
                        "features": [],
                        "external_codecs": [],
                        "external_validators": []
                    },
                    "skip": null,
                    "standards_evidence": [{"source": "dicom-standard-kb", "covered": true}]
                },
                {
                    "case_id": "classic/sc/mono2_u8_deflated_explicit_le",
                    "status": "implemented",
                    "profiles": ["core"],
                    "requirements": {
                        "features": ["deflate"],
                        "external_codecs": [],
                        "external_validators": []
                    },
                    "skip": null,
                    "standards_evidence": [{"source": "dicom-standard-kb", "covered": true}]
                },
                {
                    "case_id": "classic/sc/skipped_explicit_le",
                    "status": "skipped",
                    "profiles": ["core"],
                    "skip": {
                        "reason_code": "codec_unavailable",
                        "message": "Required codec is not available in this build.",
                        "recheck_phase": "phase-6"
                    },
                    "standards_evidence": [{"source": "local-source-note", "covered": false}]
                },
                {
                    "case_id": "classic/sc/blocked_explicit_le",
                    "status": "blocked",
                    "profiles": ["core"],
                    "skip": {
                        "reason_code": "standards_gap",
                        "message": "Standards evidence is not complete enough to generate.",
                        "recheck_phase": "remediation-r5"
                    },
                    "standards_evidence": [{"source": "local-source-note", "covered": false}]
                },
                {
                    "case_id": "classic/sc/deprecated_explicit_le",
                    "status": "deprecated",
                    "profiles": ["core"],
                    "skip": null,
                    "standards_evidence": []
                }
            ]
        });
        let generated_case_ids = vec!["classic/sc/generated_explicit_le".to_string()];

        let skipped = skipped_cases_for_run(&registry, &run, &generated_case_ids)
            .expect("registry statuses should build skipped rows");

        assert_eq!(
            skipped.len(),
            5,
            "implemented missing recipe, planned, feature-gated implemented, skipped, and blocked cases should be reported"
        );
        assert!(
            skipped
                .iter()
                .all(|case| case.get("case_id").and_then(Value::as_str)
                    != Some("classic/sc/deprecated_explicit_le"))
        );

        let implemented_missing =
            skipped_case_by_id(&skipped, "classic/sc/missing_recipe_explicit_le");
        assert_eq!(
            implemented_missing.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            implemented_missing
                .get("reason_code")
                .and_then(Value::as_str),
            Some("generator_not_implemented")
        );

        let planned = skipped_case_by_id(&skipped, "vl/photo/rgb_planar0_explicit_le");
        assert_eq!(
            planned.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            planned.get("reason_code").and_then(Value::as_str),
            Some("case_planned")
        );
        assert_eq!(
            planned.get("recheck_phase").and_then(Value::as_str),
            Some("phase-7")
        );
        assert!(
            !planned
                .get("message")
                .and_then(Value::as_str)
                .expect("planned row should have a message")
                .contains("Phase 1"),
            "planned unavailable text should not use the old hard-coded Phase 1 message"
        );

        let feature_gated =
            skipped_case_by_id(&skipped, "classic/sc/mono2_u8_deflated_explicit_le");
        assert_eq!(
            feature_gated.get("status").and_then(Value::as_str),
            Some("unavailable")
        );
        assert_eq!(
            feature_gated.get("reason_code").and_then(Value::as_str),
            Some(if cfg!(feature = "deflate") {
                "generator_not_implemented"
            } else {
                "feature_gated_case_unavailable"
            })
        );
        assert_eq!(
            feature_gated.get("recheck_phase").and_then(Value::as_str),
            Some(if cfg!(feature = "deflate") {
                "remediation-r1"
            } else {
                "phase-6"
            })
        );
        let feature_gated_message = feature_gated
            .get("message")
            .and_then(Value::as_str)
            .expect("feature-gated implemented row should have a message");
        if cfg!(feature = "deflate") {
            assert!(
                feature_gated_message.contains("does not have a generator recipe"),
                "feature-active fixture rows without generated case IDs should report missing recipes"
            );
        } else {
            assert!(
                feature_gated_message.contains("Cargo feature(s) deflate"),
                "feature-gated implemented rows should name required Cargo features"
            );
        }

        let skipped_row = skipped_case_by_id(&skipped, "classic/sc/skipped_explicit_le");
        assert_eq!(
            skipped_row.get("status").and_then(Value::as_str),
            Some("skipped")
        );
        assert_eq!(
            skipped_row.get("reason_code").and_then(Value::as_str),
            Some("codec_unavailable")
        );
        assert_eq!(
            skipped_row.get("recheck_phase").and_then(Value::as_str),
            Some("phase-6")
        );

        let blocked = skipped_case_by_id(&skipped, "classic/sc/blocked_explicit_le");
        assert_eq!(
            blocked.get("status").and_then(Value::as_str),
            Some("blocked")
        );
        assert_eq!(
            blocked.get("reason_code").and_then(Value::as_str),
            Some("standards_gap")
        );
    }

    #[test]
    fn blocked_registry_status_prevents_recipe_generation() {
        let out_dir = unique_temp_dir("blocked_registry_status");
        fs::create_dir_all(&out_dir).expect("temporary output root should be created");
        let run = PreparedGenerationRun {
            profile: "smoke".to_string(),
            manifest_path: out_dir.join("manifest.json"),
            out_dir: out_dir.clone(),
            seed: 1,
            include_stress: false,
        };
        let registry = serde_json::json!({
            "cases": [
                {
                    "case_id": "classic/sc/mono2_u8_explicit_le",
                    "status": "blocked",
                    "profiles": ["smoke"],
                    "skip": {
                        "reason_code": "standards_gap",
                        "message": "Temporarily blocked for regression coverage.",
                        "recheck_phase": "remediation-r1"
                    },
                    "standards_evidence": []
                }
            ]
        });

        let generated = crate::generator::write_supported_cases(
            &run,
            &registry,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("blocked registry case should not fail generation");

        assert!(
            generated.is_empty(),
            "blocked registry status must prevent matching recipes from writing files"
        );
        assert!(
            !out_dir
                .join("classic/sc/mono2_u8_explicit_le/instance.dcm")
                .exists(),
            "blocked recipe output should not be created"
        );

        fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "dicom-test-suite-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn skipped_case_by_id<'a>(skipped: &'a [Value], case_id: &str) -> &'a Value {
        skipped
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .expect("skipped case should be present")
    }
}
