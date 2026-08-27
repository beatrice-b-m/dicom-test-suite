use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::VR;
use dicom_dictionary_std::{StandardDataDictionary, tags, uids};
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
pub mod conformance;
pub mod coverage_gaps;
pub mod encapsulation;
pub mod generation_backends;
mod generator;
mod geometry;
mod metadata;
pub mod uid;
mod validation;
pub use coverage_gaps::{
    CoverageGapError, build_coverage_gap_report, render_coverage_gap_report_markdown,
};
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

    let generated =
        generator::write_supported_cases(run, &registry, &sha256_hex(&standards_lock_bytes))?;
    let files_written = generated.files.len();
    let generated_case_ids: Vec<String> = generated
        .files
        .iter()
        .map(|file| file.case_id.clone())
        .collect();
    let manifest = build_generation_manifest(
        run,
        &standards_lock,
        &standards_lock_bytes,
        &cargo_lock,
        &registry,
        generated.files,
        &generated_case_ids,
        &generated.unavailable_cases,
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
    geometry::validate_manifest_geometry(root_dir, files, &mut failures);
    metadata::validate_manifest_metadata_corpus(files, &mut failures);

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
    metadata::validate_manifest_metadata(
        relative_path,
        &bytes,
        expected_transfer_syntax,
        file,
        &obj,
        failures,
    );

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
    match file
        .pointer("/image/sample_type")
        .and_then(Value::as_str)
        .unwrap_or("integer")
    {
        "integer" => validate_integer_manifest_image_pixel_data(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        ),
        "float32" => validate_float32_manifest_image_pixel_data(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        ),
        _ => {
            failures.push(format!(
                "{relative_path}: image_sample_type: unsupported sample type"
            ));
            Ok(())
        }
    }
}

fn validate_integer_manifest_image_pixel_data(
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
                frame_hashes,
            )?;
            validate_u32_sc_manifest_pixel_contract(
                failures,
                relative_path,
                manifest_path,
                file,
                pixel_bytes.as_ref(),
            )?;
            validate_u1_sc_manifest_pixel_contract(
                failures,
                relative_path,
                manifest_path,
                file,
                &obj,
                pixel_bytes.as_ref(),
            )?;
            validate_icc_profile_manifest_contract(
                failures,
                relative_path,
                manifest_path,
                file,
                &obj,
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

fn validate_u32_sc_manifest_pixel_contract(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    pixel_bytes: &[u8],
) -> Result<(), ValidateError> {
    const CASE_ID: &str = "classic/sc/mono2_u32_explicit_le";
    const VALUES: [u64; 4] = [0, 65_535, 2_147_483_648, 4_294_967_295];
    const PIXEL_SHA256: &str = "56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41";

    let case_id = manifest_str(manifest_path, file, "/case_id", "case_id must be a string")?;
    let contract = file.get("expected_u32_pixels");
    if case_id != CASE_ID {
        if contract.is_some() {
            failures.push(format!(
                "{relative_path}: u32_pixel_contract_scope: expected_u32_pixels is reserved for {CASE_ID}"
            ));
        }
        return Ok(());
    }
    let contract = contract.ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "unsigned 32-bit SC file must define expected_u32_pixels",
    })?;

    for (pointer, expected, check) in [
        ("/image/rows", 2_u64, "u32_rows"),
        ("/image/columns", 2_u64, "u32_columns"),
        ("/image/frames", 1_u64, "u32_frames"),
        ("/image/samples_per_pixel", 1_u64, "u32_samples_per_pixel"),
        ("/image/bits_allocated", 32_u64, "u32_bits_allocated"),
        ("/image/bits_stored", 32_u64, "u32_bits_stored"),
        ("/image/high_bit", 31_u64, "u32_high_bit"),
        (
            "/image/pixel_representation",
            0_u64,
            "u32_pixel_representation",
        ),
        (
            "/pixel_data/value_length",
            16_u64,
            "u32_pixel_data_length_manifest",
        ),
        ("/pixel_data/frame_count", 1_u64, "u32_frame_count"),
        ("/expected_semantics/pixel_min", 0_u64, "u32_pixel_min"),
        (
            "/expected_semantics/pixel_max",
            4_294_967_295_u64,
            "u32_pixel_max",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "unsigned 32-bit numeric contract field must be an integer",
            )?,
            expected,
        );
    }
    for (pointer, expected, check) in [
        (
            "/image/photometric_interpretation",
            "MONOCHROME2",
            "u32_photometric_interpretation",
        ),
        ("/pixel_data/vr", "OW", "u32_pixel_data_vr"),
        (
            "/pixel_data/native_or_encapsulated",
            "native",
            "u32_pixel_data_layout",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_str(
                manifest_path,
                file,
                pointer,
                "unsigned 32-bit string contract field must be a string",
            )?,
            expected,
        );
    }
    if !file
        .pointer("/image/planar_configuration")
        .is_some_and(Value::is_null)
    {
        failures.push(format!(
            "{relative_path}: u32_planar_configuration_absent: expected null"
        ));
    }

    let expected_values = manifest_array(
        manifest_path,
        contract,
        "/stored_values",
        "expected_u32_pixels stored_values must be an array",
    )?;
    let recipe_values = manifest_array(
        manifest_path,
        file,
        "/recipe/recipe_parameters/pixel_values",
        "unsigned 32-bit recipe pixel_values must be an array",
    )?;
    for (actual, check) in [
        (expected_values, "u32_expected_stored_values"),
        (recipe_values, "u32_recipe_pixel_values"),
    ] {
        let actual = actual.iter().map(Value::as_u64).collect::<Option<Vec<_>>>();
        validate_equal_debug(
            failures,
            relative_path,
            check,
            actual,
            Some(VALUES.to_vec()),
        );
    }
    validate_equal(
        failures,
        relative_path,
        "u32_word_byte_order",
        manifest_str(
            manifest_path,
            contract,
            "/word_byte_order",
            "expected_u32_pixels word_byte_order must be a string",
        )?,
        "little_endian",
    );
    validate_equal(
        failures,
        relative_path,
        "u32_full_unsigned_range",
        manifest_bool(
            manifest_path,
            contract,
            "/full_unsigned_range",
            "expected_u32_pixels full_unsigned_range must be a boolean",
        )?,
        true,
    );
    let expected_hash = manifest_str(
        manifest_path,
        contract,
        "/pixel_data_sha256",
        "expected_u32_pixels pixel_data_sha256 must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "u32_declared_pixel_sha256",
        expected_hash,
        PIXEL_SHA256,
    );
    validate_equal(
        failures,
        relative_path,
        "u32_pixel_data_length",
        pixel_bytes.len(),
        16,
    );
    validate_equal(
        failures,
        relative_path,
        "u32_pixel_data_sha256",
        sha256_hex(pixel_bytes),
        expected_hash,
    );
    if pixel_bytes.len() == 16 {
        let decoded = pixel_bytes
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]) as u64)
            .collect::<Vec<_>>();
        validate_equal_debug(
            failures,
            relative_path,
            "u32_decoded_stored_values",
            decoded,
            VALUES.to_vec(),
        );
    }

    Ok(())
}

fn validate_u1_sc_manifest_pixel_contract(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
    pixel_bytes: &[u8],
) -> Result<(), ValidateError> {
    const CASE_ID: &str = "classic/sc/mono2_u1_native";
    const VALUES: [u64; 18] = [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0];
    const FRAME_HASHES: [&str; 2] = [
        "a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3",
        "c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae",
    ];
    const PIXEL_SHA256: &str = "9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b";

    let case_id = manifest_str(manifest_path, file, "/case_id", "case_id must be a string")?;
    let contract = file.get("expected_u1_pixels");
    if case_id != CASE_ID {
        if contract.is_some() {
            failures.push(format!(
                "{relative_path}: u1_pixel_contract_scope: expected_u1_pixels is reserved for {CASE_ID}"
            ));
        }
        return Ok(());
    }
    let contract = contract.ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "one-bit SC file must define expected_u1_pixels",
    })?;

    for (pointer, expected, check) in [
        ("/image/rows", 3, "u1_rows"),
        ("/image/columns", 3, "u1_columns"),
        ("/image/frames", 2, "u1_frames"),
        ("/image/samples_per_pixel", 1, "u1_samples_per_pixel"),
        ("/image/bits_allocated", 1, "u1_bits_allocated"),
        ("/image/bits_stored", 1, "u1_bits_stored"),
        ("/image/high_bit", 0, "u1_high_bit"),
        ("/image/pixel_representation", 0, "u1_pixel_representation"),
        (
            "/pixel_data/value_length",
            4,
            "u1_pixel_data_length_manifest",
        ),
        ("/pixel_data/frame_count", 2, "u1_frame_count"),
        ("/expected_semantics/pixel_min", 0, "u1_pixel_min"),
        ("/expected_semantics/pixel_max", 1, "u1_pixel_max"),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "u1 numeric field must be an integer",
            )?,
            expected,
        );
    }
    for (pointer, expected, check) in [
        (
            "/image/photometric_interpretation",
            "MONOCHROME2",
            "u1_photometric_interpretation",
        ),
        ("/pixel_data/vr", "OB", "u1_pixel_data_vr"),
        (
            "/pixel_data/native_or_encapsulated",
            "native",
            "u1_pixel_data_layout",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_str(
                manifest_path,
                file,
                pointer,
                "u1 string field must be a string",
            )?,
            expected,
        );
    }
    if !file
        .pointer("/image/planar_configuration")
        .is_some_and(Value::is_null)
    {
        failures.push(format!(
            "{relative_path}: u1_planar_configuration_absent: expected null"
        ));
    }

    let expected_values = manifest_array(
        manifest_path,
        contract,
        "/stored_values",
        "expected_u1_pixels stored_values must be an array",
    )?;
    let recipe_values = manifest_array(
        manifest_path,
        file,
        "/recipe/recipe_parameters/pixel_values",
        "u1 recipe pixel_values must be an array",
    )?;
    for (actual, check) in [
        (expected_values, "u1_expected_stored_values"),
        (recipe_values, "u1_recipe_pixel_values"),
    ] {
        validate_equal_debug(
            failures,
            relative_path,
            check,
            actual.iter().map(Value::as_u64).collect::<Option<Vec<_>>>(),
            Some(VALUES.to_vec()),
        );
    }
    for (pointer, expected, check) in [
        (
            "/packing_order",
            "least_significant_bit_first",
            "u1_packing_order",
        ),
        (
            "/frame_boundary_policy",
            "continuous_without_per_frame_padding",
            "u1_frame_boundary_policy",
        ),
        (
            "/pixel_data_sha256",
            PIXEL_SHA256,
            "u1_declared_pixel_sha256",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_str(
                manifest_path,
                contract,
                pointer,
                "u1 contract field must be a string",
            )?,
            expected,
        );
    }
    for (pointer, expected, check) in [
        ("/significant_bits", 18, "u1_significant_bits"),
        (
            "/significant_packed_bytes",
            3,
            "u1_significant_packed_bytes",
        ),
        ("/unused_high_bits", 6, "u1_unused_high_bits"),
        (
            "/value_field_padding_bytes",
            1,
            "u1_value_field_padding_bytes",
        ),
        ("/frame_two_bit_offset", 9, "u1_frame_two_bit_offset"),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_u64(
                manifest_path,
                contract,
                pointer,
                "u1 packing field must be an integer",
            )?,
            expected,
        );
    }
    let declared_frame_hashes = manifest_array(
        manifest_path,
        contract,
        "/decoded_frame_sha256",
        "expected_u1_pixels decoded_frame_sha256 must be an array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "u1_decoded_frame_hashes",
        declared_frame_hashes
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>(),
        Some(FRAME_HASHES.to_vec()),
    );

    validate_equal(
        failures,
        relative_path,
        "u1_pixel_data_length",
        pixel_bytes.len(),
        4,
    );
    validate_equal(
        failures,
        relative_path,
        "u1_pixel_data_sha256",
        sha256_hex(pixel_bytes),
        PIXEL_SHA256.to_string(),
    );
    if pixel_bytes.len() == 4 {
        let decoded = (0..18)
            .map(|bit| u64::from((pixel_bytes[bit / 8] >> (bit % 8)) & 1))
            .collect::<Vec<_>>();
        validate_equal_debug(
            failures,
            relative_path,
            "u1_decoded_stored_values",
            decoded,
            VALUES.to_vec(),
        );
        validate_equal(
            failures,
            relative_path,
            "u1_unused_high_bits_zero",
            pixel_bytes[2] & 0b1111_1100,
            0,
        );
        validate_equal(
            failures,
            relative_path,
            "u1_value_field_padding_zero",
            pixel_bytes[3],
            0,
        );
    }
    match element_tags_for_validate(obj, tags::FRAME_INCREMENT_POINTER) {
        Ok(actual_tags) => validate_equal_debug(
            failures,
            relative_path,
            "u1_frame_increment_pointer",
            actual_tags,
            vec![tags::PAGE_NUMBER_VECTOR],
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: u1_frame_increment_pointer: {err}"
        )),
    }
    match element_str_for_validate(obj, tags::PAGE_NUMBER_VECTOR) {
        Ok(value) => validate_equal(
            failures,
            relative_path,
            "u1_page_number_vector",
            value,
            "1\\2".to_string(),
        ),
        Err(err) => failures.push(format!("{relative_path}: u1_page_number_vector: {err}")),
    }

    Ok(())
}

fn validate_icc_profile_manifest_contract(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    const CASE_ID: &str = "vl/photo/rgb_icc_profile_explicit_le";
    const PROFILE_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
    const REQUIRED_TAGS: [&[u8; 4]; 9] = [
        b"desc", b"cprt", b"wtpt", b"rXYZ", b"gXYZ", b"bXYZ", b"rTRC", b"gTRC", b"bTRC",
    ];

    let case_id = manifest_str(manifest_path, file, "/case_id", "case_id must be a string")?;
    let contract = file.get("expected_icc_profile");
    if case_id != CASE_ID {
        if contract.is_some() {
            failures.push(format!(
                "{relative_path}: icc_profile_contract_scope: expected_icc_profile is reserved for {CASE_ID}"
            ));
        }
        return Ok(());
    }
    let contract = contract.ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "ICC VL Photographic file must define expected_icc_profile",
    })?;

    let expected_fields = [
        ("tag", Value::from("(0028,2000)")),
        ("vr", Value::from("OB")),
        ("profile_sha256", Value::from(PROFILE_SHA256)),
        ("profile_size_bytes", Value::from(736_u64)),
        ("declared_profile_size_bytes", Value::from(736_u64)),
        ("profile_version", Value::from("2.1.0")),
        ("device_class", Value::from("scnr")),
        ("data_color_space", Value::from("RGB")),
        ("profile_connection_space", Value::from("XYZ")),
        ("profile_signature", Value::from("acsp")),
        ("rendering_intent", Value::from("perceptual")),
        ("rendering_intent_code", Value::from(0_u64)),
        ("tag_count", Value::from(9_u64)),
        ("color_space", Value::from("SRGB")),
        ("profile_description", Value::from("sRGB")),
        ("copyright", Value::from("CC0")),
        (
            "source_identity",
            Value::from("DCMTK 3.7.0 DCMTK_SRGB_ICC_SAMPLE"),
        ),
    ];
    for (field, expected) in expected_fields {
        validate_equal_debug(
            failures,
            relative_path,
            &format!("icc_manifest_{field}"),
            contract.get(field),
            Some(&expected),
        );
    }
    for (pointer, expected, check) in [
        ("/image/rows", 2, "icc_rows"),
        ("/image/columns", 2, "icc_columns"),
        ("/image/frames", 1, "icc_frames"),
        ("/image/samples_per_pixel", 3, "icc_samples_per_pixel"),
        ("/image/bits_allocated", 8, "icc_bits_allocated"),
        ("/image/bits_stored", 8, "icc_bits_stored"),
        ("/image/high_bit", 7, "icc_high_bit"),
        ("/image/pixel_representation", 0, "icc_pixel_representation"),
        ("/image/planar_configuration", 0, "icc_planar_configuration"),
        ("/pixel_data/value_length", 12, "icc_pixel_data_length"),
    ] {
        validate_equal(
            failures,
            relative_path,
            check,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "ICC image field must be an integer",
            )?,
            expected,
        );
    }
    validate_equal(
        failures,
        relative_path,
        "icc_photometric_interpretation",
        manifest_str(
            manifest_path,
            file,
            "/image/photometric_interpretation",
            "ICC photometric interpretation must be a string",
        )?,
        "RGB",
    );

    match element_str_for_validate(obj, tags::COLOR_SPACE) {
        Ok(value) => validate_equal(
            failures,
            relative_path,
            "icc_color_space",
            value,
            "SRGB".to_string(),
        ),
        Err(err) => failures.push(format!("{relative_path}: icc_color_space: {err}")),
    }
    let profile = match obj.element(tags::ICC_PROFILE) {
        Ok(profile) => profile,
        Err(err) => {
            failures.push(format!("{relative_path}: icc_profile: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "icc_profile_vr",
        vr_name(profile.vr()),
        "OB",
    );
    let bytes = match profile.value().to_bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            failures.push(format!("{relative_path}: icc_profile_bytes: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "icc_profile_size",
        bytes.len(),
        736,
    );
    validate_equal(
        failures,
        relative_path,
        "icc_profile_sha256",
        sha256_hex(bytes.as_ref()),
        PROFILE_SHA256.to_string(),
    );
    let bytes = bytes.as_ref();
    if bytes.len() < 240 {
        failures.push(format!(
            "{relative_path}: icc_profile_header: profile is too short"
        ));
        return Ok(());
    }
    for (offset, expected, check) in [
        (8, &b"\x02\x10\x00\x00"[..], "icc_profile_version"),
        (12, &b"scnr"[..], "icc_device_class"),
        (16, &b"RGB "[..], "icc_data_color_space"),
        (20, &b"XYZ "[..], "icc_profile_connection_space"),
        (36, &b"acsp"[..], "icc_profile_signature"),
    ] {
        validate_equal_debug(
            failures,
            relative_path,
            check,
            bytes.get(offset..offset + expected.len()),
            Some(expected),
        );
    }
    validate_equal_debug(
        failures,
        relative_path,
        "icc_declared_profile_size",
        icc_be_u32(bytes, 0),
        Some(736),
    );
    validate_equal_debug(
        failures,
        relative_path,
        "icc_rendering_intent",
        icc_be_u32(bytes, 64),
        Some(0),
    );
    validate_equal_debug(
        failures,
        relative_path,
        "icc_tag_count",
        icc_be_u32(bytes, 128),
        Some(9),
    );
    for (index, expected_signature) in REQUIRED_TAGS.into_iter().enumerate() {
        let record = 132 + index * 12;
        validate_equal_debug(
            failures,
            relative_path,
            &format!("icc_tag_{index}_signature"),
            bytes.get(record..record + 4),
            Some(expected_signature.as_slice()),
        );
        let offset = icc_be_u32(bytes, record + 4).map(|value| value as usize);
        let size = icc_be_u32(bytes, record + 8).map(|value| value as usize);
        match (offset, size) {
            (Some(offset), Some(size)) => {
                if offset % 4 != 0 || offset < 240 || offset.saturating_add(size) > bytes.len() {
                    failures.push(format!(
                        "{relative_path}: icc_tag_{index}_bounds: offset {offset}, size {size}"
                    ));
                }
            }
            _ => failures.push(format!(
                "{relative_path}: icc_tag_{index}_bounds: truncated record"
            )),
        }
    }

    Ok(())
}

fn icc_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn validate_float32_manifest_image_pixel_data(
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
        obj,
        "/image/rows",
        tags::ROWS,
        "rows",
    )?;
    let columns = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        obj,
        "/image/columns",
        tags::COLUMNS,
        "columns",
    )?;
    let samples_per_pixel = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        obj,
        "/image/samples_per_pixel",
        tags::SAMPLES_PER_PIXEL,
        "samples_per_pixel",
    )?;
    validate_equal(
        failures,
        relative_path,
        "float32_samples_per_pixel",
        samples_per_pixel,
        1,
    );
    let bits_allocated = validate_u16_from_manifest_and_dataset(
        failures,
        relative_path,
        manifest_path,
        file,
        obj,
        "/image/bits_allocated",
        tags::BITS_ALLOCATED,
        "bits_allocated",
    )?;
    validate_equal(
        failures,
        relative_path,
        "float32_bits_allocated",
        bits_allocated,
        32,
    );
    let photometric_interpretation = manifest_str(
        manifest_path,
        file,
        "/image/photometric_interpretation",
        "photometric_interpretation must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "float32_photometric_interpretation",
        photometric_interpretation,
        "MONOCHROME2",
    );
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        "photometric_interpretation",
        photometric_interpretation,
    );
    match file.pointer("/image/planar_configuration") {
        Some(Value::Null) => {
            validate_element_absent(
                failures,
                relative_path,
                obj,
                tags::PLANAR_CONFIGURATION,
                "planar_configuration_absent",
            );
        }
        Some(_) => failures.push(format!(
            "{relative_path}: planar_configuration_absent: float32 single-sample pixels require null manifest planar configuration"
        )),
        None => {
            return Err(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "image planar_configuration is missing",
            });
        }
    }

    for (pointer, tag, name) in [
        (
            "/image/bits_stored",
            tags::BITS_STORED,
            "bits_stored_absent",
        ),
        ("/image/high_bit", tags::HIGH_BIT, "high_bit_absent"),
        (
            "/image/pixel_representation",
            tags::PIXEL_REPRESENTATION,
            "pixel_representation_absent",
        ),
    ] {
        if file.pointer(pointer).is_some() {
            failures.push(format!(
                "{relative_path}: {name}: float32 manifest must omit integer-only metadata"
            ));
        }
        validate_element_absent(failures, relative_path, obj, tag, name);
    }

    let frames = validate_frames(failures, relative_path, manifest_path, file, obj)?;
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::PIXEL_DATA,
        "integer_pixel_data_absent",
    );
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::DOUBLE_FLOAT_PIXEL_DATA,
        "double_float_pixel_data_absent",
    );
    let pixel_element = match obj.element(tags::FLOAT_PIXEL_DATA) {
        Ok(element) => element,
        Err(err) => {
            failures.push(format!("{relative_path}: float_pixel_data: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "pixel_data_vr",
        vr_name(pixel_element.vr()),
        "OF",
    );
    validate_equal(
        failures,
        relative_path,
        "pixel_data_manifest_vr",
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/vr",
            "pixel_data vr must be a string",
        )?,
        "OF",
    );
    validate_equal(
        failures,
        relative_path,
        "pixel_data_layout",
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/native_or_encapsulated",
            "pixel_data native_or_encapsulated must be a string",
        )?,
        "native",
    );

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
        usize::from(frames),
    );

    let pixel_bytes = match pixel_element.value().to_bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            failures.push(format!("{relative_path}: float_pixel_data_bytes: {err}"));
            return Ok(());
        }
    };
    let frame_length =
        usize::from(rows) * usize::from(columns) * usize::from(samples_per_pixel) * 4;
    let expected_value_length = frame_length * usize::from(frames);
    validate_equal(
        failures,
        relative_path,
        "float_pixel_data_length",
        pixel_bytes.len(),
        expected_value_length,
    );
    validate_equal(
        failures,
        relative_path,
        "pixel_data_manifest_length",
        manifest_u64(
            manifest_path,
            file,
            "/pixel_data/value_length",
            "pixel_data value_length must be an integer",
        )?,
        expected_value_length as u64,
    );
    if pixel_bytes.len() == expected_value_length {
        for (frame_index, frame) in pixel_bytes.chunks_exact(frame_length).enumerate() {
            let expected_hash = frame_hashes
                .get(frame_index)
                .and_then(Value::as_str)
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "pixel_data frame_hashes items must be strings",
                })?;
            validate_equal(
                failures,
                relative_path,
                "float_pixel_data_frame_hash",
                sha256_hex(frame),
                expected_hash,
            );
        }
    }

    Ok(())
}

fn validate_element_absent(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &OpenedObject,
    tag: dicom_core::Tag,
    name: &str,
) {
    if matches!(obj.element_opt(tag), Ok(Some(_))) {
        failures.push(format!("{relative_path}: {name}: expected absent"));
    }
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
    frame_hashes: &[Value],
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
        let value_bits = usize::from(rows)
            * usize::from(columns)
            * usize::from(frames)
            * usize::from(samples_per_pixel);
        let value_length = value_bits.div_ceil(8);
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

    if bits_allocated != 1 && expected_native_length == pixel_bytes.len() && frames > 0 {
        let frame_length = expected_native_length / usize::from(frames);
        for (frame_index, frame) in pixel_bytes.chunks_exact(frame_length).enumerate() {
            let expected_hash = frame_hashes
                .get(frame_index)
                .and_then(Value::as_str)
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "pixel_data frame_hashes items must be strings",
                })?;
            validate_equal(
                failures,
                relative_path,
                "native_pixel_data_frame_hash",
                sha256_hex(frame),
                expected_hash,
            );
        }
    }

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
    let expected_fragment_count = fragments_per_frame.iter().sum::<usize>();
    if pixel_fragments.len() != expected_fragment_count {
        failures.push(format!(
            "{relative_path}: jpeg_baseline_fragment_count: expected {} fragment(s), got {}",
            expected_fragment_count,
            pixel_fragments.len()
        ));
        return Ok(());
    }
    let mut fragment_cursor = 0usize;
    let compressed_frames = fragments_per_frame
        .iter()
        .map(|fragment_count| {
            let frame_end = fragment_cursor + fragment_count;
            let frame = pixel_fragments[fragment_cursor..frame_end].concat();
            fragment_cursor = frame_end;
            frame
        })
        .collect::<Vec<_>>();

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
        for compressed_frame in &compressed_frames {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: compressed_frame,
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
            compressed_frames,
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

fn manifest_f64(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<f64, ValidateError> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
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

fn manifest_string_array(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<Vec<String>, ValidateError> {
    manifest_array(manifest_path, value, pointer, message)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message,
                })
        })
        .collect()
}

fn manifest_u16_array(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<Vec<u16>, ValidateError> {
    manifest_array(manifest_path, value, pointer, message)?
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|number| u16::try_from(number).ok())
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message,
                })
        })
        .collect()
}

fn manifest_u32_array(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<Vec<u32>, ValidateError> {
    manifest_array(manifest_path, value, pointer, message)?
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|number| u32::try_from(number).ok())
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message,
                })
        })
        .collect()
}

fn manifest_f64_array(
    manifest_path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<Vec<f64>, ValidateError> {
    manifest_array(manifest_path, value, pointer, message)?
        .iter()
        .map(|item| {
            item.as_f64().ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message,
            })
        })
        .collect()
}

fn parse_manifest_tag(manifest_path: &Path, value: &str) -> Result<dicom_core::Tag, ValidateError> {
    let (group, element) = value.split_once(',').ok_or(ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "NM frame increment pointer must use GGGG,EEEE form",
    })?;
    let group = u16::from_str_radix(group, 16).map_err(|_| ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "NM frame increment pointer group must be hexadecimal",
    })?;
    let element = u16::from_str_radix(element, 16).map_err(|_| ValidateError::ManifestShape {
        path: manifest_path.to_path_buf(),
        message: "NM frame increment pointer element must be hexadecimal",
    })?;
    Ok(dicom_core::Tag(group, element))
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
        "Ultrasound Multi-frame Image" => validate_ultrasound_multiframe_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "Nuclear Medicine Image" => validate_nuclear_medicine_standard_elements(
            failures,
            relative_path,
            manifest_path,
            file,
            obj,
        )?,
        "PET Image" => {
            validate_pet_image_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
        "X-Ray Angiographic Image" => {
            validate_xa_image_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
        "X-Ray Radiofluoroscopic Image" => {
            validate_xrf_image_standard_elements(failures, relative_path, manifest_path, file, obj)?
        }
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
        "Enhanced PET Image" => validate_enhanced_pet_image_standard_elements(
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

fn validate_ultrasound_multiframe_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    const LOCKED_IMAGE_TYPE: [&str; 4] = ["ORIGINAL", "PRIMARY", "ABDOMINAL", "0001"];
    const LOCKED_FRAME_HASHES: [&str; 4] = [
        "be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7",
        "303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee",
        "7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb",
        "8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650",
    ];
    const LOCKED_FRAMES: [[u16; 16]; 4] = [
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 255, 80, 48, 64, 80, 64,
        ],
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 255, 48, 64, 80, 80,
        ],
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 64, 255, 80,
        ],
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 255, 80, 64,
        ],
    ];

    let expected = file
        .pointer("/expected_us_multiframe")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Ultrasound Multi-frame Image must define expected_us_multiframe",
        })?;

    let image_type = manifest_string_array(
        manifest_path,
        expected,
        "/image_type",
        "US multi-frame image_type must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "us_multiframe_image_type_manifest_contract",
        image_type.clone(),
        LOCKED_IMAGE_TYPE.map(str::to_string).to_vec(),
    );
    let image_type_string = image_type.join("\\");
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_image_type_semantics_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/image_type",
            "US multi-frame expected_semantics image_type must be a string",
        )?,
        image_type_string.as_str(),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "us_multiframe_image_type",
        &image_type_string,
    );

    let body_part_examined = manifest_str(
        manifest_path,
        file,
        "/expected_semantics/body_part_examined",
        "US multi-frame expected_semantics body_part_examined must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_body_part_examined_manifest_contract",
        body_part_examined,
        "ABDOMEN",
    );
    match element_str_for_validate(obj, tags::BODY_PART_EXAMINED) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "us_multiframe_body_part_examined",
            actual.as_str(),
            body_part_examined,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: us_multiframe_body_part_examined: {err}"
        )),
    }
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::LATERALITY,
        "us_multiframe_laterality_absent",
    );

    let frame_count = manifest_u64(
        manifest_path,
        expected,
        "/frame_count",
        "US multi-frame frame_count must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_frame_count_manifest_contract",
        frame_count,
        4,
    );
    for (pointer, name) in [
        (
            "/image/frames",
            "us_multiframe_image_frame_count_manifest_contract",
        ),
        (
            "/pixel_data/frame_count",
            "us_multiframe_pixel_data_frame_count_manifest_contract",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "US multi-frame frame count must be an integer",
            )?,
            frame_count,
        );
    }
    validate_type1_u16_element(
        failures,
        relative_path,
        obj,
        tags::NUMBER_OF_FRAMES,
        "us_multiframe_number_of_frames",
        frame_count as u16,
    );

    let pointer = manifest_str(
        manifest_path,
        expected,
        "/frame_increment_pointer",
        "US multi-frame frame_increment_pointer must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_frame_increment_pointer_manifest_contract",
        pointer,
        "0018,1063",
    );
    match element_tags_for_validate(obj, tags::FRAME_INCREMENT_POINTER) {
        Ok(actual) => validate_equal_debug(
            failures,
            relative_path,
            "us_multiframe_frame_increment_pointer",
            actual,
            vec![tags::FRAME_TIME],
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: us_multiframe_frame_increment_pointer: {err}"
        )),
    }
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::FRAME_TIME_VECTOR,
        "us_multiframe_frame_time_vector_absent",
    );

    let frame_time = manifest_f64(
        manifest_path,
        expected,
        "/frame_time_ms",
        "US multi-frame frame_time_ms must be numeric",
    )?;
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_frame_time_manifest_contract",
        frame_time,
        100.0,
    );
    match element_f64_for_validate(obj, tags::FRAME_TIME) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "us_multiframe_frame_time",
            actual,
            frame_time,
        ),
        Err(err) => failures.push(format!("{relative_path}: us_multiframe_frame_time: {err}")),
    }
    let relative_times = manifest_f64_array(
        manifest_path,
        expected,
        "/frame_relative_times_ms",
        "US multi-frame frame_relative_times_ms must be numeric",
    )?;
    let derived_times = (0..frame_count)
        .map(|index| index as f64 * frame_time)
        .collect::<Vec<_>>();
    validate_equal_debug(
        failures,
        relative_path,
        "us_multiframe_relative_times_manifest_contract",
        relative_times,
        derived_times,
    );

    let lossy = manifest_str(
        manifest_path,
        expected,
        "/lossy_image_compression",
        "US multi-frame lossy_image_compression must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_lossy_image_compression_manifest_contract",
        lossy,
        "00",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::LOSSY_IMAGE_COMPRESSION,
        "us_multiframe_lossy_image_compression",
        lossy,
    );
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::LOSSY_IMAGE_COMPRESSION_RATIO,
        "us_multiframe_lossy_image_compression_ratio_absent",
    );
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::LOSSY_IMAGE_COMPRESSION_METHOD,
        "us_multiframe_lossy_image_compression_method_absent",
    );

    for (pointer, name) in [
        (
            "/color_data_present",
            "us_multiframe_color_data_present_manifest_contract",
        ),
        (
            "/spatially_related_frames",
            "us_multiframe_spatially_related_frames_manifest_contract",
        ),
        (
            "/region_calibrated",
            "us_multiframe_region_calibrated_manifest_contract",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_bool(
                manifest_path,
                expected,
                pointer,
                "US multi-frame boolean contract must be a boolean",
            )?,
            false,
        );
    }
    validate_type1_u16_element(
        failures,
        relative_path,
        obj,
        tags::ULTRASOUND_COLOR_DATA_PRESENT,
        "us_multiframe_color_data_present",
        0,
    );
    for (tag, name) in [
        (
            tags::FRAME_OF_REFERENCE_UID,
            "us_multiframe_frame_of_reference_absent",
        ),
        (
            tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
            "us_multiframe_region_calibration_absent",
        ),
    ] {
        validate_element_absent(failures, relative_path, obj, tag, name);
    }
    if !file
        .pointer("/uids/frame_of_reference_uid")
        .is_some_and(Value::is_null)
    {
        failures.push(format!(
            "{relative_path}: us_multiframe_frame_of_reference_manifest_contract: expected null"
        ));
    }

    for (pointer, name, locked) in [
        ("/image/rows", "us_multiframe_rows_manifest_contract", 4),
        (
            "/image/columns",
            "us_multiframe_columns_manifest_contract",
            4,
        ),
        (
            "/pixel_data/value_length",
            "us_multiframe_value_length_manifest_contract",
            64,
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "US multi-frame image contract must be an integer",
            )?,
            locked,
        );
    }
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_native_encoding_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/native_or_encapsulated",
            "US multi-frame native_or_encapsulated must be a string",
        )?,
        "native",
    );
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_pixel_vr_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/vr",
            "US multi-frame pixel_data vr must be a string",
        )?,
        "OB",
    );

    let manifest_hashes = manifest_string_array(
        manifest_path,
        file,
        "/pixel_data/frame_hashes",
        "US multi-frame pixel_data frame_hashes must be strings",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "us_multiframe_frame_hashes_manifest_contract",
        manifest_hashes.clone(),
        LOCKED_FRAME_HASHES.map(str::to_string).to_vec(),
    );
    let frames = manifest_array(
        manifest_path,
        expected,
        "/frames",
        "US multi-frame frames must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_frames_manifest_contract",
        frames.len(),
        frame_count as usize,
    );

    let pixel_bytes = match obj.element(tags::PIXEL_DATA) {
        Ok(element) => match element.value().to_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                failures.push(format!("{relative_path}: us_multiframe_pixel_bytes: {err}"));
                return Ok(());
            }
        },
        Err(err) => {
            failures.push(format!("{relative_path}: us_multiframe_pixel_bytes: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_pixel_byte_length",
        pixel_bytes.len(),
        64,
    );
    for (index, frame) in frames.iter().enumerate() {
        validate_equal(
            failures,
            relative_path,
            "us_multiframe_frame_number_manifest_contract",
            manifest_u64(
                manifest_path,
                frame,
                "/frame_number",
                "US multi-frame frame_number must be an integer",
            )?,
            (index + 1) as u64,
        );
        let values = manifest_u16_array(
            manifest_path,
            frame,
            "/pixel_values",
            "US multi-frame pixel_values must be unsigned integers",
        )?;
        let locked_values = LOCKED_FRAMES
            .get(index)
            .map(|values| values.to_vec())
            .unwrap_or_default();
        validate_equal_debug(
            failures,
            relative_path,
            "us_multiframe_pixel_values_manifest_contract",
            values.clone(),
            locked_values,
        );
        let frame_hash = manifest_str(
            manifest_path,
            frame,
            "/frame_sha256",
            "US multi-frame frame_sha256 must be a string",
        )?;
        if let Some(pixel_hash) = manifest_hashes.get(index) {
            validate_equal(
                failures,
                relative_path,
                "us_multiframe_frame_hash_manifest_contract",
                frame_hash,
                pixel_hash,
            );
        }
        if let Some(actual) = pixel_bytes.chunks_exact(16).nth(index) {
            validate_equal_debug(
                failures,
                relative_path,
                "us_multiframe_pixel_values",
                actual
                    .iter()
                    .map(|value| u16::from(*value))
                    .collect::<Vec<_>>(),
                values,
            );
            validate_equal(
                failures,
                relative_path,
                "us_multiframe_frame_hash",
                sha256_hex(actual),
                frame_hash,
            );
        }
    }

    validate_equal(
        failures,
        relative_path,
        "us_multiframe_payload_hash_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/recipe/recipe_parameters/payload_sha256",
            "US multi-frame payload_sha256 must be a string",
        )?,
        "060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9",
    );
    validate_equal(
        failures,
        relative_path,
        "us_multiframe_payload_hash",
        sha256_hex(pixel_bytes.as_ref()),
        "060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9",
    );

    Ok(())
}

fn validate_nuclear_medicine_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let expected = file
        .pointer("/expected_nm_multiframe")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Nuclear Medicine file must define expected_nm_multiframe",
        })?;
    let image_type = manifest_string_array(
        manifest_path,
        expected,
        "/image_type",
        "NM image_type must be a string array",
    )?;
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "nm_image_type_type1",
        &image_type.join("\\"),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::BODY_PART_EXAMINED,
        "nm_body_part_examined",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/body_part_examined",
            "NM body_part_examined must be a string",
        )?,
    );
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::LATERALITY,
        "nm_laterality_absent_for_unpaired_head",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::ACTUAL_FRAME_DURATION,
        "nm_actual_frame_duration_type1c",
        &manifest_u64(
            manifest_path,
            expected,
            "/actual_frame_duration_ms",
            "NM actual_frame_duration_ms must be an integer",
        )?
        .to_string(),
    );
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::COUNTS_ACCUMULATED,
        "nm_counts_accumulated_type2",
        &manifest_u64(
            manifest_path,
            expected,
            "/counts_accumulated",
            "NM counts_accumulated must be an integer",
        )?
        .to_string(),
    );
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::PIXEL_SPACING,
        "nm_pixel_spacing_type2",
        "4\\4",
    );

    let pointer_strings = manifest_string_array(
        manifest_path,
        expected,
        "/frame_increment_pointers",
        "NM frame_increment_pointers must be a string array",
    )?;
    let expected_pointers = pointer_strings
        .iter()
        .map(|tag| parse_manifest_tag(manifest_path, tag))
        .collect::<Result<Vec<_>, _>>()?;
    match element_tags_for_validate(obj, tags::FRAME_INCREMENT_POINTER) {
        Ok(actual) => validate_equal_debug(
            failures,
            relative_path,
            "nm_frame_increment_pointers",
            actual,
            expected_pointers,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: nm_frame_increment_pointers: {err}"
        )),
    }

    let energy_vector = validate_nm_index_vector(
        failures,
        relative_path,
        manifest_path,
        expected,
        obj,
        "/energy_window_vector",
        tags::ENERGY_WINDOW_VECTOR,
        "nm_energy_window_vector",
        manifest_u64(
            manifest_path,
            expected,
            "/number_of_energy_windows",
            "NM number_of_energy_windows must be an integer",
        )? as u16,
    )?;
    let detector_vector = validate_nm_index_vector(
        failures,
        relative_path,
        manifest_path,
        expected,
        obj,
        "/detector_vector",
        tags::DETECTOR_VECTOR,
        "nm_detector_vector",
        manifest_u64(
            manifest_path,
            expected,
            "/number_of_detectors",
            "NM number_of_detectors must be an integer",
        )? as u16,
    )?;
    let frames = manifest_u64(
        manifest_path,
        file,
        "/image/frames",
        "NM image frames must be an integer",
    )? as usize;
    validate_equal(
        failures,
        relative_path,
        "nm_energy_window_vector_frame_count",
        energy_vector.len(),
        frames,
    );
    validate_equal(
        failures,
        relative_path,
        "nm_detector_vector_frame_count",
        detector_vector.len(),
        frames,
    );

    validate_nm_energy_windows(failures, relative_path, manifest_path, expected, obj)?;
    validate_nm_detectors(failures, relative_path, manifest_path, expected, obj)?;
    for (name, tag) in [
        (
            "nm_radiopharmaceutical_information_empty",
            tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
        ),
        (
            "nm_patient_orientation_code_empty",
            tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
        ),
        (
            "nm_patient_gantry_relationship_code_empty",
            tags::PATIENT_GANTRY_RELATIONSHIP_CODE_SEQUENCE,
        ),
    ] {
        match sequence_item_count_for_validate(obj, tag) {
            Ok(count) => validate_equal(failures, relative_path, name, count, 0),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }

    let frame_dimensions = manifest_array(
        manifest_path,
        expected,
        "/frame_dimensions",
        "NM frame_dimensions must be an array",
    )?;
    let frame_hashes = manifest_array(
        manifest_path,
        file,
        "/pixel_data/frame_hashes",
        "NM pixel_data frame_hashes must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "nm_frame_dimension_count",
        frame_dimensions.len(),
        frames,
    );
    for (index, dimension) in frame_dimensions.iter().enumerate() {
        let frame_number = manifest_u64(
            manifest_path,
            dimension,
            "/frame_number",
            "NM frame_number must be an integer",
        )? as usize;
        let energy_index = manifest_u64(
            manifest_path,
            dimension,
            "/energy_window_index",
            "NM energy_window_index must be an integer",
        )? as u16;
        let detector_index = manifest_u64(
            manifest_path,
            dimension,
            "/detector_index",
            "NM detector_index must be an integer",
        )? as u16;
        validate_equal(
            failures,
            relative_path,
            "nm_frame_number_order",
            frame_number,
            index + 1,
        );
        if let Some(actual) = energy_vector.get(index) {
            validate_equal(
                failures,
                relative_path,
                "nm_frame_energy_window_index",
                energy_index,
                *actual,
            );
        }
        if let Some(actual) = detector_vector.get(index) {
            validate_equal(
                failures,
                relative_path,
                "nm_frame_detector_index",
                detector_index,
                *actual,
            );
        }
        validate_equal(
            failures,
            relative_path,
            "nm_frame_dimension_hash",
            manifest_str(
                manifest_path,
                dimension,
                "/frame_sha256",
                "NM frame_sha256 must be a string",
            )?,
            frame_hashes.get(index).and_then(Value::as_str).ok_or(
                ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "NM pixel_data frame_hashes items must be strings",
                },
            )?,
        );
    }

    Ok(())
}

fn validate_pet_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let expected = file
        .pointer("/expected_pet_activity")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "PET Image file must define expected_pet_activity",
        })?;

    let image_type = manifest_string_array(
        manifest_path,
        expected,
        "/image_type",
        "PET image_type must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "pet_image_type_manifest_contract",
        image_type.clone(),
        vec!["ORIGINAL".to_string(), "PRIMARY".to_string()],
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "pet_image_type",
        &image_type.join("\\"),
    );

    for (pointer, tag, name, locked) in [
        ("/units", tags::UNITS, "pet_units", "BQML"),
        (
            "/counts_source",
            tags::COUNTS_SOURCE,
            "pet_counts_source",
            "EMISSION",
        ),
        (
            "/decay_correction",
            tags::DECAY_CORRECTION,
            "pet_decay_correction",
            "NONE",
        ),
    ] {
        let manifest_value = manifest_str(
            manifest_path,
            expected,
            pointer,
            "PET coded scalar must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked,
        );
        validate_type1_str_element(failures, relative_path, obj, tag, name, manifest_value);
    }
    let series_type = manifest_string_array(
        manifest_path,
        expected,
        "/series_type",
        "PET series_type must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "pet_series_type_manifest_contract",
        series_type.clone(),
        vec!["STATIC".to_string(), "IMAGE".to_string()],
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::SERIES_TYPE,
        "pet_series_type",
        &series_type.join("\\"),
    );

    let corrected_image = manifest_string_array(
        manifest_path,
        expected,
        "/corrected_image",
        "PET corrected_image must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "pet_corrected_image_manifest_contract",
        corrected_image.clone(),
        vec!["DCAL".to_string()],
    );
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::CORRECTED_IMAGE,
        "pet_corrected_image",
        &corrected_image.join("\\"),
    );

    let number_of_slices = manifest_u64(
        manifest_path,
        expected,
        "/number_of_slices",
        "PET number_of_slices must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "pet_number_of_slices_manifest_contract",
        number_of_slices,
        1,
    );
    validate_type1_u16_element(
        failures,
        relative_path,
        obj,
        tags::NUMBER_OF_SLICES,
        "pet_number_of_slices",
        number_of_slices as u16,
    );

    for (pointer, tag, name, locked_number, locked_encoded) in [
        (
            "/dose_calibration_factor",
            tags::DOSE_CALIBRATION_FACTOR,
            "pet_dose_calibration_factor",
            1.0,
            "1",
        ),
        (
            "/rescale_intercept",
            tags::RESCALE_INTERCEPT,
            "pet_rescale_intercept",
            0.0,
            "0",
        ),
        (
            "/rescale_slope",
            tags::RESCALE_SLOPE,
            "pet_rescale_slope",
            2.5,
            "2.5",
        ),
        (
            "/frame_reference_time_ms",
            tags::FRAME_REFERENCE_TIME,
            "pet_frame_reference_time",
            30_000.0,
            "30000",
        ),
    ] {
        let manifest_value = manifest_f64(
            manifest_path,
            expected,
            pointer,
            "PET quantitative scalar must be numeric",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked_number,
        );
        validate_type1_str_element(failures, relative_path, obj, tag, name, locked_encoded);
    }

    let actual_frame_duration = manifest_u64(
        manifest_path,
        expected,
        "/actual_frame_duration_ms",
        "PET actual_frame_duration_ms must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "pet_actual_frame_duration_manifest_contract",
        actual_frame_duration,
        60_000,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::ACTUAL_FRAME_DURATION,
        "pet_actual_frame_duration",
        &actual_frame_duration.to_string(),
    );

    let image_index = manifest_u64(
        manifest_path,
        expected,
        "/image_index",
        "PET image_index must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "pet_image_index_manifest_contract",
        image_index,
        1,
    );
    validate_type1_u16_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_INDEX,
        "pet_image_index",
        image_index as u16,
    );

    for (name, tag, expected_count) in [
        (
            "pet_radiopharmaceutical_information_empty",
            tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
            manifest_u64(
                manifest_path,
                expected,
                "/radiopharmaceutical_information_item_count",
                "PET radiopharmaceutical item count must be an integer",
            )? as usize,
        ),
        (
            "pet_patient_orientation_code_empty",
            tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
            0,
        ),
        (
            "pet_patient_gantry_relationship_code_empty",
            tags::PATIENT_GANTRY_RELATIONSHIP_CODE_SEQUENCE,
            0,
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            expected_count,
            0,
        );
        match sequence_item_count_for_validate(obj, tag) {
            Ok(actual) => validate_equal(failures, relative_path, name, actual, expected_count),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }

    for (tag, name, locked) in [
        (tags::PIXEL_SPACING, "pet_pixel_spacing", "4\\4"),
        (
            tags::IMAGE_ORIENTATION_PATIENT,
            "pet_image_orientation_patient",
            "1\\0\\0\\0\\1\\0",
        ),
        (
            tags::IMAGE_POSITION_PATIENT,
            "pet_image_position_patient",
            "0\\0\\0",
        ),
        (tags::SLICE_THICKNESS, "pet_slice_thickness", "4"),
    ] {
        validate_type1_str_element(failures, relative_path, obj, tag, name, locked);
    }
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "pet_frame_of_reference_uid",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "PET frame_of_reference_uid must be a string",
        )?,
    );
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::POSITION_REFERENCE_INDICATOR,
        "pet_position_reference_indicator",
        "",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::BODY_PART_EXAMINED,
        "pet_body_part_examined",
        "HEAD",
    );
    validate_element_absent(
        failures,
        relative_path,
        obj,
        tags::LATERALITY,
        "pet_laterality_absent_for_unpaired_head",
    );

    let stored_values = manifest_u16_array(
        manifest_path,
        expected,
        "/stored_values",
        "PET stored_values must be an unsigned integer array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "pet_stored_values_manifest_contract",
        stored_values.clone(),
        vec![0, 100, 200, 400],
    );
    let activity_values = manifest_f64_array(
        manifest_path,
        expected,
        "/activity_values_bqml",
        "PET activity_values_bqml must be a numeric array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "pet_activity_values_bqml_manifest_contract",
        activity_values.clone(),
        vec![0.0, 250.0, 500.0, 1_000.0],
    );

    validate_equal(
        failures,
        relative_path,
        "pet_native_pixel_encoding",
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/native_or_encapsulated",
            "PET pixel_data native_or_encapsulated must be a string",
        )?,
        "native",
    );
    validate_equal(
        failures,
        relative_path,
        "pet_pixel_data_vr",
        manifest_str(
            manifest_path,
            file,
            "/pixel_data/vr",
            "PET pixel_data vr must be a string",
        )?,
        "OW",
    );

    let pixel_bytes = match obj.element(tags::PIXEL_DATA) {
        Ok(element) => match element.value().to_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                failures.push(format!("{relative_path}: pet_pixel_bytes: {err}"));
                return Ok(());
            }
        },
        Err(err) => {
            failures.push(format!("{relative_path}: pet_pixel_bytes: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "pet_native_pixel_byte_length",
        pixel_bytes.len(),
        stored_values.len() * 2,
    );
    let decoded_stored = pixel_bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    validate_equal_debug(
        failures,
        relative_path,
        "pet_stored_values",
        decoded_stored.clone(),
        stored_values,
    );

    let intercept = manifest_f64(
        manifest_path,
        expected,
        "/rescale_intercept",
        "PET rescale_intercept must be numeric",
    )?;
    let slope = manifest_f64(
        manifest_path,
        expected,
        "/rescale_slope",
        "PET rescale_slope must be numeric",
    )?;
    let derived_activity = decoded_stored
        .iter()
        .map(|stored| f64::from(*stored) * slope + intercept)
        .collect::<Vec<_>>();
    validate_equal_debug(
        failures,
        relative_path,
        "pet_activity_values_bqml",
        derived_activity,
        activity_values,
    );

    let frame_hashes = manifest_array(
        manifest_path,
        file,
        "/pixel_data/frame_hashes",
        "PET pixel_data frame_hashes must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "pet_pixel_frame_hash_count",
        frame_hashes.len(),
        1,
    );
    let expected_hash =
        frame_hashes
            .first()
            .and_then(Value::as_str)
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "PET pixel_data frame_hashes must contain one string",
            })?;
    validate_equal(
        failures,
        relative_path,
        "pet_pixel_frame_hash",
        sha256_hex(pixel_bytes.as_ref()),
        expected_hash,
    );

    Ok(())
}

fn validate_xa_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let expected = file
        .pointer("/expected_xa_projection")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "X-Ray Angiographic Image must define expected_xa_projection",
        })?;
    let recipe_expected = file
        .pointer("/recipe/recipe_parameters/xa_projection")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "XA recipe parameters must define xa_projection",
        })?;
    validate_equal_debug(
        failures,
        relative_path,
        "xa_projection_recipe_manifest_contract",
        recipe_expected,
        expected,
    );

    let image_type = manifest_string_array(
        manifest_path,
        expected,
        "/image_type",
        "XA image_type must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "xa_image_type_manifest_contract",
        image_type.clone(),
        vec![
            "ORIGINAL".to_string(),
            "PRIMARY".to_string(),
            "SINGLE PLANE".to_string(),
        ],
    );
    let image_type_string = image_type.join("\\");
    validate_equal(
        failures,
        relative_path,
        "xa_image_type_semantics_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/image_type",
            "XA expected_semantics image_type must be a string",
        )?,
        image_type_string.as_str(),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "xa_image_type",
        &image_type_string,
    );

    let body_part = manifest_str(
        manifest_path,
        expected,
        "/body_part_examined",
        "XA body_part_examined must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "xa_body_part_examined_manifest_contract",
        body_part,
        "HEART",
    );
    validate_equal(
        failures,
        relative_path,
        "xa_body_part_semantics_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/body_part_examined",
            "XA expected_semantics body_part_examined must be a string",
        )?,
        body_part,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::BODY_PART_EXAMINED,
        "xa_body_part_examined",
        body_part,
    );

    for (pointer, name, locked) in [
        (
            "/patient_orientation_empty",
            "xa_patient_orientation_empty_manifest_contract",
            true,
        ),
        (
            "/laterality_present",
            "xa_laterality_present_manifest_contract",
            false,
        ),
        (
            "/multiframe_cine",
            "xa_multiframe_cine_manifest_contract",
            false,
        ),
        (
            "/biplane_data_present",
            "xa_biplane_data_present_manifest_contract",
            false,
        ),
        (
            "/contrast_used",
            "xa_contrast_used_manifest_contract",
            false,
        ),
        (
            "/subtraction_applied",
            "xa_subtraction_applied_manifest_contract",
            false,
        ),
        (
            "/table_motion_present",
            "xa_table_motion_present_manifest_contract",
            false,
        ),
        (
            "/patient_space_geometry_present",
            "xa_patient_space_geometry_present_manifest_contract",
            false,
        ),
        (
            "/pixel_spacing_calibrated",
            "xa_pixel_spacing_calibrated_manifest_contract",
            false,
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_bool(
                manifest_path,
                expected,
                pointer,
                "XA projection flag must be a boolean",
            )?,
            locked,
        );
    }
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::PATIENT_ORIENTATION,
        "xa_patient_orientation_empty",
        "",
    );

    for (pointer, tag, name, locked) in [
        (
            "/pixel_intensity_relationship",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            "xa_pixel_intensity_relationship",
            "LIN",
        ),
        (
            "/radiation_setting",
            tags::RADIATION_SETTING,
            "xa_radiation_setting",
            "GR",
        ),
        (
            "/lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "xa_lossy_image_compression",
            "00",
        ),
    ] {
        let manifest_value = manifest_str(
            manifest_path,
            expected,
            pointer,
            "XA coded projection value must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked,
        );
        validate_type1_str_element(failures, relative_path, obj, tag, name, manifest_value);
    }

    for (pointer, tag, name, locked) in [
        ("/kvp", tags::KVP, "xa_kvp", 80.0),
        (
            "/positioner_primary_angle_degrees",
            tags::POSITIONER_PRIMARY_ANGLE,
            "xa_positioner_primary_angle",
            15.0,
        ),
        (
            "/positioner_secondary_angle_degrees",
            tags::POSITIONER_SECONDARY_ANGLE,
            "xa_positioner_secondary_angle",
            -10.0,
        ),
        (
            "/distance_source_to_detector_mm",
            tags::DISTANCE_SOURCE_TO_DETECTOR,
            "xa_distance_source_to_detector",
            1200.0,
        ),
        (
            "/distance_source_to_patient_mm",
            tags::DISTANCE_SOURCE_TO_PATIENT,
            "xa_distance_source_to_patient",
            800.0,
        ),
        (
            "/estimated_radiographic_magnification_factor",
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            "xa_estimated_magnification",
            1.5,
        ),
    ] {
        let manifest_value = manifest_f64(
            manifest_path,
            expected,
            pointer,
            "XA projection scalar must be numeric",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked,
        );
        match element_f64_for_validate(obj, tag) {
            Ok(actual) => validate_equal(failures, relative_path, name, actual, manifest_value),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }

    let exposure = manifest_u64(
        manifest_path,
        expected,
        "/exposure_mas",
        "XA exposure_mas must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "xa_exposure_manifest_contract",
        exposure,
        4,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::EXPOSURE,
        "xa_exposure",
        &exposure.to_string(),
    );

    let imager_spacing = manifest_f64_array(
        manifest_path,
        expected,
        "/imager_pixel_spacing_mm",
        "XA imager_pixel_spacing_mm must be numeric",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "xa_imager_pixel_spacing_manifest_contract",
        imager_spacing.clone(),
        vec![0.2, 0.2],
    );
    match element_f64_values_for_validate(obj, tags::IMAGER_PIXEL_SPACING) {
        Ok(actual) => validate_equal_debug(
            failures,
            relative_path,
            "xa_imager_pixel_spacing",
            actual,
            imager_spacing,
        ),
        Err(err) => failures.push(format!("{relative_path}: xa_imager_pixel_spacing: {err}")),
    }

    validate_equal(
        failures,
        relative_path,
        "xa_frame_count_manifest_contract",
        manifest_u64(
            manifest_path,
            expected,
            "/frame_count",
            "XA frame_count must be an integer",
        )?,
        1,
    );
    for (pointer, name) in [
        ("/image/frames", "xa_image_frame_count_manifest_contract"),
        (
            "/pixel_data/frame_count",
            "xa_pixel_frame_count_manifest_contract",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "XA frame count must be an integer",
            )?,
            1,
        );
    }

    let sid = element_f64_for_validate(obj, tags::DISTANCE_SOURCE_TO_DETECTOR);
    let sod = element_f64_for_validate(obj, tags::DISTANCE_SOURCE_TO_PATIENT);
    let magnification =
        element_f64_for_validate(obj, tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR);
    if let (Ok(sid), Ok(sod), Ok(magnification)) = (sid, sod, magnification) {
        validate_equal(
            failures,
            relative_path,
            "xa_sid_sod_magnification_relation",
            sid / sod,
            magnification,
        );
    }

    for (tag, name) in [
        (tags::LATERALITY, "xa_laterality_absent"),
        (tags::NUMBER_OF_FRAMES, "xa_number_of_frames_absent"),
        (
            tags::FRAME_INCREMENT_POINTER,
            "xa_frame_increment_pointer_absent",
        ),
        (tags::FRAME_TIME, "xa_frame_time_absent"),
        (tags::FRAME_TIME_VECTOR, "xa_frame_time_vector_absent"),
        (tags::POSITIONER_MOTION, "xa_positioner_motion_absent"),
        (
            tags::POSITIONER_PRIMARY_ANGLE_INCREMENT,
            "xa_primary_angle_increment_absent",
        ),
        (
            tags::POSITIONER_SECONDARY_ANGLE_INCREMENT,
            "xa_secondary_angle_increment_absent",
        ),
        (
            tags::REFERENCED_IMAGE_SEQUENCE,
            "xa_biplane_reference_absent",
        ),
        (tags::CONTRAST_BOLUS_AGENT, "xa_contrast_agent_absent"),
        (
            tags::MASK_SUBTRACTION_SEQUENCE,
            "xa_mask_subtraction_absent",
        ),
        (tags::TABLE_MOTION, "xa_table_motion_absent"),
        (tags::FRAME_OF_REFERENCE_UID, "xa_frame_of_reference_absent"),
        (
            tags::IMAGE_ORIENTATION_PATIENT,
            "xa_image_orientation_patient_absent",
        ),
        (
            tags::IMAGE_POSITION_PATIENT,
            "xa_image_position_patient_absent",
        ),
        (tags::PIXEL_SPACING, "xa_pixel_spacing_absent"),
        (tags::MODALITY_LUT_SEQUENCE, "xa_modality_lut_absent"),
        (tags::VOILUT_SEQUENCE, "xa_voi_lut_absent"),
        (tags::CALIBRATION_IMAGE, "xa_calibration_image_absent"),
        (
            tags::LOSSY_IMAGE_COMPRESSION_RATIO,
            "xa_lossy_image_compression_ratio_absent",
        ),
        (
            tags::LOSSY_IMAGE_COMPRESSION_METHOD,
            "xa_lossy_image_compression_method_absent",
        ),
    ] {
        validate_element_absent(failures, relative_path, obj, tag, name);
    }
    if !file
        .pointer("/uids/frame_of_reference_uid")
        .is_some_and(Value::is_null)
    {
        failures.push(format!(
            "{relative_path}: xa_frame_of_reference_manifest_contract: expected null"
        ));
    }

    let payload_hash = manifest_str(
        manifest_path,
        file,
        "/recipe/recipe_parameters/payload_sha256",
        "XA payload_sha256 must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "xa_payload_hash_manifest_contract",
        payload_hash,
        "0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e",
    );
    match obj.element(tags::PIXEL_DATA) {
        Ok(element) => match element.value().to_bytes() {
            Ok(bytes) => validate_equal(
                failures,
                relative_path,
                "xa_payload_hash",
                sha256_hex(bytes.as_ref()),
                payload_hash,
            ),
            Err(err) => failures.push(format!("{relative_path}: xa_payload_hash: {err}")),
        },
        Err(err) => failures.push(format!("{relative_path}: xa_payload_hash: {err}")),
    }

    Ok(())
}

fn validate_xrf_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let expected =
        file.pointer("/expected_xrf_projection")
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "X-Ray Radiofluoroscopic Image must define expected_xrf_projection",
            })?;
    let recipe_expected = file
        .pointer("/recipe/recipe_parameters/xrf_projection")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "XRF recipe parameters must define xrf_projection",
        })?;
    validate_equal_debug(
        failures,
        relative_path,
        "xrf_projection_recipe_manifest_contract",
        recipe_expected,
        expected,
    );

    let image_type = manifest_string_array(
        manifest_path,
        expected,
        "/image_type",
        "XRF image_type must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "xrf_image_type_manifest_contract",
        image_type.clone(),
        vec![
            "ORIGINAL".to_string(),
            "PRIMARY".to_string(),
            "SINGLE PLANE".to_string(),
        ],
    );
    let image_type_string = image_type.join("\\");
    validate_equal(
        failures,
        relative_path,
        "xrf_image_type_semantics_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/image_type",
            "XRF expected_semantics image_type must be a string",
        )?,
        image_type_string.as_str(),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "xrf_image_type",
        &image_type_string,
    );

    let body_part = manifest_str(
        manifest_path,
        expected,
        "/body_part_examined",
        "XRF body_part_examined must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "xrf_body_part_examined_manifest_contract",
        body_part,
        "ABDOMEN",
    );
    validate_equal(
        failures,
        relative_path,
        "xrf_body_part_semantics_manifest_contract",
        manifest_str(
            manifest_path,
            file,
            "/expected_semantics/body_part_examined",
            "XRF expected_semantics body_part_examined must be a string",
        )?,
        body_part,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::BODY_PART_EXAMINED,
        "xrf_body_part_examined",
        body_part,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "xrf_modality",
        "RF",
    );

    for (pointer, name, locked) in [
        (
            "/patient_orientation_empty",
            "xrf_patient_orientation_empty_manifest_contract",
            true,
        ),
        (
            "/laterality_present",
            "xrf_laterality_present_manifest_contract",
            false,
        ),
        (
            "/multiframe_cine",
            "xrf_multiframe_cine_manifest_contract",
            false,
        ),
        (
            "/biplane_data_present",
            "xrf_biplane_data_present_manifest_contract",
            false,
        ),
        (
            "/contrast_used",
            "xrf_contrast_used_manifest_contract",
            false,
        ),
        (
            "/subtraction_applied",
            "xrf_subtraction_applied_manifest_contract",
            false,
        ),
        (
            "/table_position_present",
            "xrf_table_position_present_manifest_contract",
            false,
        ),
        (
            "/table_motion_present",
            "xrf_table_motion_present_manifest_contract",
            false,
        ),
        (
            "/table_tilt_present",
            "xrf_table_tilt_present_manifest_contract",
            false,
        ),
        (
            "/tomography_present",
            "xrf_tomography_present_manifest_contract",
            false,
        ),
        (
            "/patient_space_geometry_present",
            "xrf_patient_space_geometry_present_manifest_contract",
            false,
        ),
        (
            "/pixel_spacing_calibrated",
            "xrf_pixel_spacing_calibrated_manifest_contract",
            false,
        ),
        (
            "/xa_positioner_angles_present",
            "xrf_xa_positioner_angles_present_manifest_contract",
            false,
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_bool(
                manifest_path,
                expected,
                pointer,
                "XRF projection flag must be a boolean",
            )?,
            locked,
        );
    }
    validate_str_element(
        failures,
        relative_path,
        obj,
        tags::PATIENT_ORIENTATION,
        "xrf_patient_orientation_empty",
        "",
    );

    for (pointer, tag, name, locked) in [
        (
            "/pixel_intensity_relationship",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            "xrf_pixel_intensity_relationship",
            "LIN",
        ),
        (
            "/radiation_setting",
            tags::RADIATION_SETTING,
            "xrf_radiation_setting",
            "SC",
        ),
        (
            "/lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "xrf_lossy_image_compression",
            "00",
        ),
    ] {
        let manifest_value = manifest_str(
            manifest_path,
            expected,
            pointer,
            "XRF coded projection value must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked,
        );
        validate_type1_str_element(failures, relative_path, obj, tag, name, manifest_value);
    }

    for (pointer, tag, name, locked) in [
        ("/kvp", tags::KVP, "xrf_kvp", 70.0),
        (
            "/distance_source_to_detector_mm",
            tags::DISTANCE_SOURCE_TO_DETECTOR,
            "xrf_distance_source_to_detector",
            1200.0,
        ),
        (
            "/distance_source_to_patient_mm",
            tags::DISTANCE_SOURCE_TO_PATIENT,
            "xrf_distance_source_to_patient",
            800.0,
        ),
        (
            "/estimated_radiographic_magnification_factor",
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            "xrf_estimated_magnification",
            1.5,
        ),
        (
            "/column_angulation_degrees",
            tags::COLUMN_ANGULATION,
            "xrf_column_angulation",
            10.0,
        ),
    ] {
        let manifest_value = manifest_f64(
            manifest_path,
            expected,
            pointer,
            "XRF projection scalar must be numeric",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked,
        );
        match element_f64_for_validate(obj, tag) {
            Ok(actual) => validate_equal(failures, relative_path, name, actual, manifest_value),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }

    let exposure = manifest_u64(
        manifest_path,
        expected,
        "/exposure_mas",
        "XRF exposure_mas must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "xrf_exposure_manifest_contract",
        exposure,
        1,
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::EXPOSURE,
        "xrf_exposure",
        &exposure.to_string(),
    );

    let imager_spacing = manifest_f64_array(
        manifest_path,
        expected,
        "/imager_pixel_spacing_mm",
        "XRF imager_pixel_spacing_mm must be numeric",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "xrf_imager_pixel_spacing_manifest_contract",
        imager_spacing.clone(),
        vec![0.2, 0.2],
    );
    match element_f64_values_for_validate(obj, tags::IMAGER_PIXEL_SPACING) {
        Ok(actual) => validate_equal_debug(
            failures,
            relative_path,
            "xrf_imager_pixel_spacing",
            actual,
            imager_spacing,
        ),
        Err(err) => failures.push(format!("{relative_path}: xrf_imager_pixel_spacing: {err}")),
    }

    validate_equal(
        failures,
        relative_path,
        "xrf_frame_count_manifest_contract",
        manifest_u64(
            manifest_path,
            expected,
            "/frame_count",
            "XRF frame_count must be an integer",
        )?,
        1,
    );
    for (pointer, name) in [
        ("/image/frames", "xrf_image_frame_count_manifest_contract"),
        (
            "/pixel_data/frame_count",
            "xrf_pixel_frame_count_manifest_contract",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "XRF frame count must be an integer",
            )?,
            1,
        );
    }

    let sid = element_f64_for_validate(obj, tags::DISTANCE_SOURCE_TO_DETECTOR);
    let sod = element_f64_for_validate(obj, tags::DISTANCE_SOURCE_TO_PATIENT);
    let magnification =
        element_f64_for_validate(obj, tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR);
    if let (Ok(sid), Ok(sod), Ok(magnification)) = (sid, sod, magnification) {
        validate_equal(
            failures,
            relative_path,
            "xrf_sid_sod_magnification_relation",
            sid / sod,
            magnification,
        );
    }

    for (tag, expected_vr, name) in [
        (tags::IMAGE_TYPE, dicom_core::VR::CS, "xrf_image_type_vr"),
        (tags::MODALITY, dicom_core::VR::CS, "xrf_modality_vr"),
        (
            tags::BODY_PART_EXAMINED,
            dicom_core::VR::CS,
            "xrf_body_part_examined_vr",
        ),
        (
            tags::PATIENT_ORIENTATION,
            dicom_core::VR::CS,
            "xrf_patient_orientation_vr",
        ),
        (
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            dicom_core::VR::CS,
            "xrf_pixel_intensity_relationship_vr",
        ),
        (
            tags::LOSSY_IMAGE_COMPRESSION,
            dicom_core::VR::CS,
            "xrf_lossy_image_compression_vr",
        ),
        (
            tags::RADIATION_SETTING,
            dicom_core::VR::CS,
            "xrf_radiation_setting_vr",
        ),
        (tags::KVP, dicom_core::VR::DS, "xrf_kvp_vr"),
        (tags::EXPOSURE, dicom_core::VR::IS, "xrf_exposure_vr"),
        (
            tags::IMAGER_PIXEL_SPACING,
            dicom_core::VR::DS,
            "xrf_imager_pixel_spacing_vr",
        ),
        (
            tags::DISTANCE_SOURCE_TO_DETECTOR,
            dicom_core::VR::DS,
            "xrf_distance_source_to_detector_vr",
        ),
        (
            tags::DISTANCE_SOURCE_TO_PATIENT,
            dicom_core::VR::DS,
            "xrf_distance_source_to_patient_vr",
        ),
        (
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            dicom_core::VR::DS,
            "xrf_estimated_magnification_vr",
        ),
        (
            tags::COLUMN_ANGULATION,
            dicom_core::VR::DS,
            "xrf_column_angulation_vr",
        ),
    ] {
        match obj.element(tag) {
            Ok(element) => validate_equal(failures, relative_path, name, element.vr(), expected_vr),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }

    for (tag, name) in [
        (tags::LATERALITY, "xrf_laterality_absent"),
        (tags::EXPOSURE_TIME_INU_S, "xrf_exposure_time_us_absent"),
        (
            tags::X_RAY_TUBE_CURRENT_INU_A,
            "xrf_x_ray_tube_current_ua_absent",
        ),
        (tags::EXPOSURE_INU_AS, "xrf_exposure_uas_absent"),
        (
            dicom_core::Tag(0x0018, 0x1495),
            "xrf_number_of_tomosynthesis_source_images_absent",
        ),
        (tags::POSITIONER_MOTION, "xrf_positioner_motion_absent"),
        (tags::NUMBER_OF_FRAMES, "xrf_number_of_frames_absent"),
        (
            tags::FRAME_INCREMENT_POINTER,
            "xrf_frame_increment_pointer_absent",
        ),
        (tags::FRAME_TIME, "xrf_frame_time_absent"),
        (tags::FRAME_TIME_VECTOR, "xrf_frame_time_vector_absent"),
        (
            tags::REFERENCED_IMAGE_SEQUENCE,
            "xrf_biplane_reference_absent",
        ),
        (tags::CONTRAST_BOLUS_AGENT, "xrf_contrast_agent_absent"),
        (
            tags::MASK_SUBTRACTION_SEQUENCE,
            "xrf_mask_subtraction_absent",
        ),
        (tags::TABLE_HEIGHT, "xrf_table_height_absent"),
        (tags::TABLE_TRAVERSE, "xrf_table_traverse_absent"),
        (tags::TABLE_MOTION, "xrf_table_motion_absent"),
        (
            tags::TABLE_VERTICAL_INCREMENT,
            "xrf_table_vertical_increment_absent",
        ),
        (
            tags::TABLE_LATERAL_INCREMENT,
            "xrf_table_lateral_increment_absent",
        ),
        (
            tags::TABLE_LONGITUDINAL_INCREMENT,
            "xrf_table_longitudinal_increment_absent",
        ),
        (tags::TABLE_ANGLE, "xrf_table_angle_absent"),
        (
            tags::GANTRY_DETECTOR_TILT,
            "xrf_gantry_detector_tilt_absent",
        ),
        (tags::SCAN_OPTIONS, "xrf_scan_options_absent"),
        (tags::TOMO_LAYER_HEIGHT, "xrf_tomo_layer_height_absent"),
        (tags::TOMO_ANGLE, "xrf_tomo_angle_absent"),
        (tags::TOMO_TIME, "xrf_tomo_time_absent"),
        (tags::TOMO_TYPE, "xrf_tomo_type_absent"),
        (tags::TOMO_CLASS, "xrf_tomo_class_absent"),
        (
            tags::FRAME_OF_REFERENCE_UID,
            "xrf_frame_of_reference_absent",
        ),
        (
            tags::IMAGE_ORIENTATION_PATIENT,
            "xrf_image_orientation_patient_absent",
        ),
        (
            tags::IMAGE_POSITION_PATIENT,
            "xrf_image_position_patient_absent",
        ),
        (tags::PIXEL_SPACING, "xrf_pixel_spacing_absent"),
        (tags::CALIBRATION_IMAGE, "xrf_calibration_image_absent"),
        (
            tags::POSITIONER_PRIMARY_ANGLE,
            "xrf_positioner_primary_angle_absent",
        ),
        (
            tags::POSITIONER_SECONDARY_ANGLE,
            "xrf_positioner_secondary_angle_absent",
        ),
        (
            tags::POSITIONER_PRIMARY_ANGLE_INCREMENT,
            "xrf_primary_angle_increment_absent",
        ),
        (
            tags::POSITIONER_SECONDARY_ANGLE_INCREMENT,
            "xrf_secondary_angle_increment_absent",
        ),
        (tags::MODALITY_LUT_SEQUENCE, "xrf_modality_lut_absent"),
        (tags::VOILUT_SEQUENCE, "xrf_voi_lut_absent"),
        (
            tags::PRESENTATION_LUT_SHAPE,
            "xrf_presentation_lut_shape_absent",
        ),
        (tags::WINDOW_CENTER, "xrf_window_center_absent"),
        (tags::WINDOW_WIDTH, "xrf_window_width_absent"),
        (tags::SHUTTER_SHAPE, "xrf_shutter_shape_absent"),
        (
            tags::SHUTTER_LEFT_VERTICAL_EDGE,
            "xrf_shutter_left_vertical_edge_absent",
        ),
        (
            tags::SHUTTER_RIGHT_VERTICAL_EDGE,
            "xrf_shutter_right_vertical_edge_absent",
        ),
        (
            tags::SHUTTER_UPPER_HORIZONTAL_EDGE,
            "xrf_shutter_upper_horizontal_edge_absent",
        ),
        (
            tags::SHUTTER_LOWER_HORIZONTAL_EDGE,
            "xrf_shutter_lower_horizontal_edge_absent",
        ),
        (dicom_core::Tag(0x6000, 0x0010), "xrf_overlay_rows_absent"),
        (dicom_core::Tag(0x6000, 0x3000), "xrf_overlay_data_absent"),
        (tags::COLLIMATOR_SHAPE, "xrf_collimator_shape_absent"),
        (
            tags::COLLIMATOR_LEFT_VERTICAL_EDGE,
            "xrf_collimator_left_vertical_edge_absent",
        ),
        (
            tags::COLLIMATOR_RIGHT_VERTICAL_EDGE,
            "xrf_collimator_right_vertical_edge_absent",
        ),
        (
            tags::COLLIMATOR_UPPER_HORIZONTAL_EDGE,
            "xrf_collimator_upper_horizontal_edge_absent",
        ),
        (
            tags::COLLIMATOR_LOWER_HORIZONTAL_EDGE,
            "xrf_collimator_lower_horizontal_edge_absent",
        ),
        (
            tags::IMAGE_AND_FLUOROSCOPY_AREA_DOSE_PRODUCT,
            "xrf_area_dose_product_absent",
        ),
        (
            tags::LOSSY_IMAGE_COMPRESSION_RATIO,
            "xrf_lossy_image_compression_ratio_absent",
        ),
        (
            tags::LOSSY_IMAGE_COMPRESSION_METHOD,
            "xrf_lossy_image_compression_method_absent",
        ),
        (tags::RADIATION_MODE, "xrf_radiation_mode_absent"),
        (tags::AVERAGE_PULSE_WIDTH, "xrf_average_pulse_width_absent"),
        (tags::EXPOSURE_TIME, "xrf_exposure_time_absent"),
        (tags::X_RAY_TUBE_CURRENT, "xrf_x_ray_tube_current_absent"),
        (tags::DETECTOR_TYPE, "xrf_detector_type_absent"),
        (
            tags::DETECTOR_CONFIGURATION,
            "xrf_detector_configuration_absent",
        ),
        (
            tags::DETECTOR_DESCRIPTION,
            "xrf_detector_description_absent",
        ),
        (tags::DETECTOR_ID, "xrf_detector_id_absent"),
        (
            tags::DETECTOR_ELEMENT_PHYSICAL_SIZE,
            "xrf_detector_element_physical_size_absent",
        ),
        (
            tags::DETECTOR_ELEMENT_SPACING,
            "xrf_detector_element_spacing_absent",
        ),
        (
            tags::DETECTOR_ACTIVE_SHAPE,
            "xrf_detector_active_shape_absent",
        ),
        (
            tags::DETECTOR_ACTIVE_DIMENSIONS,
            "xrf_detector_active_dimensions_absent",
        ),
        (
            tags::DETECTOR_ACTIVE_ORIGIN,
            "xrf_detector_active_origin_absent",
        ),
        (
            tags::FIELD_OF_VIEW_ORIGIN,
            "xrf_field_of_view_origin_absent",
        ),
        (
            tags::FIELD_OF_VIEW_ROTATION,
            "xrf_field_of_view_rotation_absent",
        ),
        (
            tags::FIELD_OF_VIEW_HORIZONTAL_FLIP,
            "xrf_field_of_view_horizontal_flip_absent",
        ),
    ] {
        validate_element_absent(failures, relative_path, obj, tag, name);
    }
    if !file
        .pointer("/uids/frame_of_reference_uid")
        .is_some_and(Value::is_null)
    {
        failures.push(format!(
            "{relative_path}: xrf_frame_of_reference_manifest_contract: expected null"
        ));
    }

    let payload_hash = manifest_str(
        manifest_path,
        file,
        "/recipe/recipe_parameters/payload_sha256",
        "XRF payload_sha256 must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "xrf_payload_hash_manifest_contract",
        payload_hash,
        "0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e",
    );
    match obj.element(tags::PIXEL_DATA) {
        Ok(element) => match element.value().to_bytes() {
            Ok(bytes) => validate_equal(
                failures,
                relative_path,
                "xrf_payload_hash",
                sha256_hex(bytes.as_ref()),
                payload_hash,
            ),
            Err(err) => failures.push(format!("{relative_path}: xrf_payload_hash: {err}")),
        },
        Err(err) => failures.push(format!("{relative_path}: xrf_payload_hash: {err}")),
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_nm_index_vector(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    obj: &OpenedObject,
    manifest_pointer: &str,
    tag: dicom_core::Tag,
    name: &str,
    declared_count: u16,
) -> Result<Vec<u16>, ValidateError> {
    let expected_vector = manifest_u16_array(
        manifest_path,
        expected,
        manifest_pointer,
        "NM dimension vector must be an unsigned integer array",
    )?;
    match element_u16_values_for_validate(obj, tag) {
        Ok(actual) => validate_equal_debug(
            failures,
            relative_path,
            name,
            actual,
            expected_vector.clone(),
        ),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
    for value in &expected_vector {
        if *value == 0 || *value > declared_count {
            failures.push(format!(
                "{relative_path}: {name}_bounds: index {value} is outside 1..={declared_count}"
            ));
        }
    }
    Ok(expected_vector)
}

fn validate_nm_energy_windows(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let declared = manifest_u64(
        manifest_path,
        expected,
        "/number_of_energy_windows",
        "NM number_of_energy_windows must be an integer",
    )? as usize;
    match element_u16_for_validate(obj, tags::NUMBER_OF_ENERGY_WINDOWS) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "nm_number_of_energy_windows",
            usize::from(actual),
            declared,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: nm_number_of_energy_windows: {err}"
        )),
    }
    match sequence_item_count_for_validate(obj, tags::ENERGY_WINDOW_INFORMATION_SEQUENCE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "nm_energy_window_information_count",
            actual,
            declared,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: nm_energy_window_information_count: {err}"
        )),
    }
    let windows = manifest_array(
        manifest_path,
        expected,
        "/energy_windows",
        "NM energy_windows must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "nm_energy_window_manifest_count",
        windows.len(),
        declared,
    );
    for (index, window) in windows.iter().enumerate() {
        validate_equal(
            failures,
            relative_path,
            "nm_energy_window_index_order",
            manifest_u64(
                manifest_path,
                window,
                "/index",
                "NM energy window index must be an integer",
            )? as usize,
            index + 1,
        );
        let Ok(item) = top_level_sequence_item_for_validate(
            obj,
            tags::ENERGY_WINDOW_INFORMATION_SEQUENCE,
            index,
        ) else {
            continue;
        };
        validate_item_type1_str_element(
            failures,
            relative_path,
            item,
            tags::ENERGY_WINDOW_NAME,
            "nm_energy_window_name",
            manifest_str(
                manifest_path,
                window,
                "/name",
                "NM energy window name must be a string",
            )?,
        );
        match item_sequence_item_count_for_validate(item, tags::ENERGY_WINDOW_RANGE_SEQUENCE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "nm_energy_window_range_count",
                actual,
                1,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: nm_energy_window_range_count: {err}"
            )),
        }
        let Ok(range) =
            item_sequence_item_for_validate(item, tags::ENERGY_WINDOW_RANGE_SEQUENCE, 0)
        else {
            continue;
        };
        for (name, tag, pointer) in [
            (
                "nm_energy_window_lower_limit",
                tags::ENERGY_WINDOW_LOWER_LIMIT,
                "/lower_limit_kev",
            ),
            (
                "nm_energy_window_upper_limit",
                tags::ENERGY_WINDOW_UPPER_LIMIT,
                "/upper_limit_kev",
            ),
        ] {
            match item_f64_for_validate(range, tag) {
                Ok(actual) => validate_equal_debug(
                    failures,
                    relative_path,
                    name,
                    actual,
                    manifest_f64(
                        manifest_path,
                        window,
                        pointer,
                        "NM energy window limit must be numeric",
                    )?,
                ),
                Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
            }
        }
    }
    Ok(())
}

fn validate_nm_detectors(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let declared = manifest_u64(
        manifest_path,
        expected,
        "/number_of_detectors",
        "NM number_of_detectors must be an integer",
    )? as usize;
    match element_u16_for_validate(obj, tags::NUMBER_OF_DETECTORS) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "nm_number_of_detectors",
            usize::from(actual),
            declared,
        ),
        Err(err) => failures.push(format!("{relative_path}: nm_number_of_detectors: {err}")),
    }
    match sequence_item_count_for_validate(obj, tags::DETECTOR_INFORMATION_SEQUENCE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "nm_detector_information_count",
            actual,
            declared,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: nm_detector_information_count: {err}"
        )),
    }
    let detectors = manifest_array(
        manifest_path,
        expected,
        "/detectors",
        "NM detectors must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "nm_detector_manifest_count",
        detectors.len(),
        declared,
    );
    for (index, detector) in detectors.iter().enumerate() {
        validate_equal(
            failures,
            relative_path,
            "nm_detector_index_order",
            manifest_u64(
                manifest_path,
                detector,
                "/index",
                "NM detector index must be an integer",
            )? as usize,
            index + 1,
        );
        let Ok(item) =
            top_level_sequence_item_for_validate(obj, tags::DETECTOR_INFORMATION_SEQUENCE, index)
        else {
            continue;
        };
        validate_item_type1_str_element(
            failures,
            relative_path,
            item,
            tags::COLLIMATOR_TYPE,
            "nm_detector_collimator_type",
            manifest_str(
                manifest_path,
                detector,
                "/collimator_type",
                "NM detector collimator_type must be a string",
            )?,
        );
        for (name, tag, pointer) in [
            (
                "nm_detector_focal_distance",
                tags::FOCAL_DISTANCE,
                "/focal_distance_mm",
            ),
            (
                "nm_detector_start_angle",
                tags::START_ANGLE,
                "/start_angle_degrees",
            ),
        ] {
            match item_f64_for_validate(item, tag) {
                Ok(actual) => validate_equal_debug(
                    failures,
                    relative_path,
                    name,
                    actual,
                    manifest_f64(
                        manifest_path,
                        detector,
                        pointer,
                        "NM detector scalar must be numeric",
                    )?,
                ),
                Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
            }
        }
        for (name, tag, pointer) in [
            (
                "nm_detector_image_orientation_patient",
                tags::IMAGE_ORIENTATION_PATIENT,
                "/image_orientation_patient",
            ),
            (
                "nm_detector_image_position_patient",
                tags::IMAGE_POSITION_PATIENT,
                "/image_position_patient",
            ),
        ] {
            match item_f64_values_for_validate(item, tag) {
                Ok(actual) => validate_equal_debug(
                    failures,
                    relative_path,
                    name,
                    actual,
                    manifest_f64_array(
                        manifest_path,
                        detector,
                        pointer,
                        "NM detector geometry must be a numeric array",
                    )?,
                ),
                Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
            }
        }
    }
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

    for (tag, name, pointer, message, type1) in [
        (
            tags::PATIENT_POSITION,
            "enhanced_mr_patient_position_type2c",
            "/expected_semantics/patient_position",
            "expected Enhanced MR patient position must be a string",
            false,
        ),
        (
            tags::CONTENT_QUALIFICATION,
            "enhanced_mr_content_qualification_type1c",
            "/expected_semantics/content_qualification",
            "expected Enhanced MR content qualification must be a string",
            true,
        ),
        (
            tags::APPLICABLE_SAFETY_STANDARD_AGENCY,
            "enhanced_mr_applicable_safety_standard_agency_type1c",
            "/expected_semantics/applicable_safety_standard_agency",
            "expected Enhanced MR safety standard agency must be a string",
            true,
        ),
        (
            tags::COMPLEX_IMAGE_COMPONENT,
            "enhanced_mr_complex_image_component_image_level_type1c",
            "/expected_semantics/complex_image_component",
            "expected Enhanced MR complex image component must be a string",
            true,
        ),
        (
            tags::ACQUISITION_CONTRAST,
            "enhanced_mr_acquisition_contrast_image_level_type1c",
            "/expected_semantics/acquisition_contrast",
            "expected Enhanced MR acquisition contrast must be a string",
            true,
        ),
        (
            tags::BURNED_IN_ANNOTATION,
            "enhanced_mr_burned_in_annotation_type1c",
            "/expected_semantics/burned_in_annotation",
            "expected Enhanced MR burned-in annotation state must be a string",
            true,
        ),
        (
            tags::LOSSY_IMAGE_COMPRESSION,
            "enhanced_mr_lossy_image_compression_type1c",
            "/expected_semantics/lossy_image_compression",
            "expected Enhanced MR lossy compression state must be a string",
            true,
        ),
        (
            tags::PRESENTATION_LUT_SHAPE,
            "enhanced_mr_presentation_lut_shape_type1c",
            "/expected_semantics/presentation_lut_shape",
            "expected Enhanced MR presentation LUT shape must be a string",
            true,
        ),
    ] {
        let expected = manifest_str(manifest_path, file, pointer, message)?;
        if type1 {
            validate_type1_str_element(failures, relative_path, obj, tag, name, expected);
        } else {
            validate_str_element(failures, relative_path, obj, tag, name, expected);
        }
    }

    let shared =
        match top_level_sequence_item_for_validate(obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)
        {
            Ok(shared) => shared,
            Err(err) => {
                failures.push(format!(
                    "{relative_path}: enhanced_mr_shared_functional_groups_content: {err}"
                ));
                return Ok(());
            }
        };
    let frame_type =
        match item_sequence_item_for_validate(shared, tags::MR_IMAGE_FRAME_TYPE_SEQUENCE, 0) {
            Ok(frame_type) => frame_type,
            Err(err) => {
                failures.push(format!(
                    "{relative_path}: enhanced_mr_image_frame_type_sequence_type1: {err}"
                ));
                return Ok(());
            }
        };
    for (tag, name, pointer, message) in [
        (
            tags::COMPLEX_IMAGE_COMPONENT,
            "enhanced_mr_complex_image_component_frame_level_type1c",
            "/expected_semantics/complex_image_component",
            "expected Enhanced MR complex image component must be a string",
        ),
        (
            tags::ACQUISITION_CONTRAST,
            "enhanced_mr_acquisition_contrast_frame_level_type1c",
            "/expected_semantics/acquisition_contrast",
            "expected Enhanced MR acquisition contrast must be a string",
        ),
    ] {
        validate_item_type1_str_element(
            failures,
            relative_path,
            frame_type,
            tag,
            name,
            manifest_str(manifest_path, file, pointer, message)?,
        );
    }

    validate_enhanced_mr_timing_safety(failures, relative_path, manifest_path, file, shared)?;

    Ok(())
}

fn validate_enhanced_pet_image_standard_elements(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let expected = file
        .pointer("/expected_enhanced_pet")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Enhanced PET Image file must define expected_enhanced_pet",
        })?;
    let recipe_expected = file
        .pointer("/recipe/recipe_parameters/enhanced_pet")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Enhanced PET recipe parameters must define enhanced_pet",
        })?;
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_recipe_manifest_contract",
        recipe_expected,
        expected,
    );

    let image_type = manifest_string_array(
        manifest_path,
        expected,
        "/image_type",
        "Enhanced PET image_type must be a string array",
    )?;
    let frame_type = manifest_string_array(
        manifest_path,
        expected,
        "/frame_type",
        "Enhanced PET frame_type must be a string array",
    )?;
    let locked_type = vec!["DERIVED", "PRIMARY", "STATIC", "MULTIPLICATION"];
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_image_type_manifest_contract",
        image_type.clone(),
        locked_type.iter().map(ToString::to_string).collect(),
    );
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_frame_type_manifest_contract",
        frame_type.clone(),
        locked_type.iter().map(ToString::to_string).collect(),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::IMAGE_TYPE,
        "enhanced_pet_image_type",
        &image_type.join("\\"),
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::MODALITY,
        "enhanced_pet_modality",
        "PT",
    );
    validate_type1_str_element(
        failures,
        relative_path,
        obj,
        tags::FRAME_OF_REFERENCE_UID,
        "enhanced_pet_frame_of_reference_uid",
        manifest_str(
            manifest_path,
            file,
            "/uids/frame_of_reference_uid",
            "Enhanced PET frame_of_reference_uid must be a string",
        )?,
    );
    validate_type2_element(
        failures,
        relative_path,
        obj,
        tags::POSITION_REFERENCE_INDICATOR,
        "enhanced_pet_position_reference_indicator_type2",
    );
    for (tag, name, locked) in [
        (
            tags::MANUFACTURER,
            "enhanced_pet_manufacturer_type1",
            "dicom-test-suite",
        ),
        (
            tags::MANUFACTURER_MODEL_NAME,
            "enhanced_pet_model_name_type1",
            "enhanced_pet_multiframe_explicit_le",
        ),
        (
            tags::DEVICE_SERIAL_NUMBER,
            "enhanced_pet_device_serial_number_type1",
            "DTS-EPET-0001",
        ),
        (
            tags::SOFTWARE_VERSIONS,
            "enhanced_pet_software_versions_type1",
            PACKAGE_VERSION,
        ),
    ] {
        validate_type1_str_element(failures, relative_path, obj, tag, name, locked);
    }

    for (pointer, tag, name, locked) in [
        (
            "/pixel_presentation",
            tags::PIXEL_PRESENTATION,
            "enhanced_pet_pixel_presentation",
            "MONOCHROME",
        ),
        (
            "/volumetric_properties",
            tags::VOLUMETRIC_PROPERTIES,
            "enhanced_pet_volumetric_properties",
            "VOLUME",
        ),
        (
            "/volume_based_calculation_technique",
            tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
            "enhanced_pet_volume_based_calculation_technique",
            "NONE",
        ),
        (
            "/content_qualification",
            tags::CONTENT_QUALIFICATION,
            "enhanced_pet_content_qualification",
            "RESEARCH",
        ),
        (
            "/burned_in_annotation",
            tags::BURNED_IN_ANNOTATION,
            "enhanced_pet_burned_in_annotation",
            "NO",
        ),
        (
            "/lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "enhanced_pet_lossy_image_compression",
            "00",
        ),
        (
            "/presentation_lut_shape",
            tags::PRESENTATION_LUT_SHAPE,
            "enhanced_pet_presentation_lut_shape",
            "IDENTITY",
        ),
        (
            "/table_motion",
            tags::TABLE_MOTION,
            "enhanced_pet_table_motion",
            "STATIC",
        ),
        (
            "/time_of_flight_information_used",
            tags::TIME_OF_FLIGHT_INFORMATION_USED,
            "enhanced_pet_time_of_flight_information_used",
            "FALSE",
        ),
        (
            "/counts_source",
            tags::COUNTS_SOURCE,
            "enhanced_pet_counts_source",
            "EMISSION",
        ),
    ] {
        let manifest_value = manifest_str(
            manifest_path,
            expected,
            pointer,
            "Enhanced PET coded scalar must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            manifest_value,
            locked,
        );
        validate_type1_str_element(failures, relative_path, obj, tag, name, manifest_value);
    }
    for (tag, name) in [
        (tags::IMAGE_TYPE, "enhanced_pet_image_type"),
        (tags::MODALITY, "enhanced_pet_modality"),
        (tags::PIXEL_PRESENTATION, "enhanced_pet_pixel_presentation"),
        (
            tags::VOLUMETRIC_PROPERTIES,
            "enhanced_pet_volumetric_properties",
        ),
        (
            tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
            "enhanced_pet_volume_based_calculation_technique",
        ),
        (
            tags::CONTENT_QUALIFICATION,
            "enhanced_pet_content_qualification",
        ),
        (
            tags::BURNED_IN_ANNOTATION,
            "enhanced_pet_burned_in_annotation",
        ),
        (
            tags::LOSSY_IMAGE_COMPRESSION,
            "enhanced_pet_lossy_image_compression",
        ),
        (
            tags::PRESENTATION_LUT_SHAPE,
            "enhanced_pet_presentation_lut_shape",
        ),
        (tags::TABLE_MOTION, "enhanced_pet_table_motion"),
        (
            tags::TIME_OF_FLIGHT_INFORMATION_USED,
            "enhanced_pet_time_of_flight_information_used",
        ),
        (tags::COUNTS_SOURCE, "enhanced_pet_counts_source"),
    ] {
        match obj.element(tag) {
            Ok(element) => validate_equal(
                failures,
                relative_path,
                &format!("{name}_vr"),
                element.vr(),
                dicom_core::VR::CS,
            ),
            Err(err) => failures.push(format!("{relative_path}: {name}_vr: {err}")),
        }
    }

    validate_enhanced_pet_code(
        failures,
        relative_path,
        manifest_path,
        expected
            .pointer("/view_code")
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "Enhanced PET view_code must be an object",
            })?,
        obj,
        tags::VIEW_CODE_SEQUENCE,
        "enhanced_pet_view_code",
        ("24422004", "SCT", "Axial"),
    )?;
    let modifier_count = manifest_u64(
        manifest_path,
        expected,
        "/view_modifier_item_count",
        "Enhanced PET view_modifier_item_count must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_view_modifier_count_manifest_contract",
        modifier_count,
        0,
    );
    if let Ok(view) = top_level_sequence_item_for_validate(obj, tags::VIEW_CODE_SEQUENCE, 0) {
        validate_item_absent(
            failures,
            relative_path,
            view,
            tags::VIEW_MODIFIER_CODE_SEQUENCE,
            "enhanced_pet_view_modifier_absent",
        );
    }
    let slice_progression_present = manifest_bool(
        manifest_path,
        expected,
        "/slice_progression_direction_present",
        "Enhanced PET slice_progression_direction_present must be a boolean",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_slice_progression_manifest_contract",
        slice_progression_present,
        false,
    );
    validate_item_absent(
        failures,
        relative_path,
        obj,
        tags::SLICE_PROGRESSION_DIRECTION,
        "enhanced_pet_slice_progression_direction_absent",
    );

    let frame_count = manifest_u64(
        manifest_path,
        expected,
        "/frame_count",
        "Enhanced PET frame_count must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_frame_count_manifest_contract",
        frame_count,
        2,
    );
    for (pointer, name) in [
        (
            "/image/frames",
            "enhanced_pet_image_frame_count_manifest_contract",
        ),
        (
            "/pixel_data/frame_count",
            "enhanced_pet_pixel_frame_count_manifest_contract",
        ),
    ] {
        validate_equal(
            failures,
            relative_path,
            name,
            manifest_u64(
                manifest_path,
                file,
                pointer,
                "Enhanced PET frame count must be an integer",
            )?,
            frame_count,
        );
    }

    for (pointer, tag, name, locked) in [
        (
            "/shared_functional_groups_item_count",
            tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            "enhanced_pet_shared_functional_groups",
            1_u64,
        ),
        (
            "/per_frame_functional_groups_item_count",
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            "enhanced_pet_per_frame_functional_groups",
            2,
        ),
        (
            "/dimension_organization_item_count",
            tags::DIMENSION_ORGANIZATION_SEQUENCE,
            "enhanced_pet_dimension_organization",
            1,
        ),
        (
            "/dimension_index_item_count",
            tags::DIMENSION_INDEX_SEQUENCE,
            "enhanced_pet_dimension_index",
            1,
        ),
        (
            "/acquisition_context_item_count",
            tags::ACQUISITION_CONTEXT_SEQUENCE,
            "enhanced_pet_acquisition_context_empty",
            0,
        ),
    ] {
        let declared = manifest_u64(
            manifest_path,
            expected,
            pointer,
            "Enhanced PET sequence count must be an integer",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            declared,
            locked,
        );
        match sequence_item_count_for_validate(obj, tag) {
            Ok(actual) => validate_equal(failures, relative_path, name, actual, declared),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }

    validate_enhanced_pet_dimensions(failures, relative_path, manifest_path, file, expected, obj)?;
    validate_enhanced_pet_shared_groups(
        failures,
        relative_path,
        manifest_path,
        expected,
        obj,
        &frame_type,
    )?;
    validate_enhanced_pet_per_frame_groups(failures, relative_path, manifest_path, expected, obj)?;
    validate_enhanced_pet_isotope_and_corrections(
        failures,
        relative_path,
        manifest_path,
        expected,
        obj,
    )?;
    validate_enhanced_pet_pixels_and_nonclaims(
        failures,
        relative_path,
        manifest_path,
        file,
        expected,
        obj,
    )?;

    Ok(())
}

fn validate_enhanced_pet_dimensions(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    expected: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let organization_uid = manifest_str(
        manifest_path,
        file,
        "/uids/dimension_organization_uid",
        "Enhanced PET dimension_organization_uid must be a string",
    )?;
    if let Ok(item) =
        top_level_sequence_item_for_validate(obj, tags::DIMENSION_ORGANIZATION_SEQUENCE, 0)
    {
        validate_item_type1_str_element(
            failures,
            relative_path,
            item,
            tags::DIMENSION_ORGANIZATION_UID,
            "enhanced_pet_dimension_organization_uid",
            organization_uid,
        );
    }
    if let Ok(item) = top_level_sequence_item_for_validate(obj, tags::DIMENSION_INDEX_SEQUENCE, 0) {
        validate_item_type1_str_element(
            failures,
            relative_path,
            item,
            tags::DIMENSION_ORGANIZATION_UID,
            "enhanced_pet_dimension_index_organization_uid",
            organization_uid,
        );
        for (pointer, tag, name, locked) in [
            (
                "/dimension_index_pointer",
                tags::DIMENSION_INDEX_POINTER,
                "enhanced_pet_dimension_index_pointer",
                tags::IN_STACK_POSITION_NUMBER,
            ),
            (
                "/functional_group_pointer",
                tags::FUNCTIONAL_GROUP_POINTER,
                "enhanced_pet_functional_group_pointer",
                tags::FRAME_CONTENT_SEQUENCE,
            ),
        ] {
            let declared = parse_manifest_tag(
                manifest_path,
                manifest_str(
                    manifest_path,
                    expected,
                    pointer,
                    "Enhanced PET dimension pointer must be GGGG,EEEE",
                )?,
            )?;
            validate_equal(
                failures,
                relative_path,
                &format!("{name}_manifest_contract"),
                declared,
                locked,
            );
            match item_tag_for_validate(item, tag) {
                Ok(actual) => validate_equal(failures, relative_path, name, actual, declared),
                Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
            }
            validate_item_vr(failures, relative_path, item, tag, name, dicom_core::VR::AT);
        }
    }
    Ok(())
}

fn validate_enhanced_pet_shared_groups(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    obj: &OpenedObject,
    frame_type_expected: &[String],
) -> Result<(), ValidateError> {
    let shared =
        match top_level_sequence_item_for_validate(obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)
        {
            Ok(item) => item,
            Err(err) => {
                failures.push(format!(
                    "{relative_path}: enhanced_pet_shared_functional_groups_content: {err}"
                ));
                return Ok(());
            }
        };
    for (tag, name) in [
        (
            tags::FRAME_CONTENT_SEQUENCE,
            "enhanced_pet_frame_content_not_shared",
        ),
        (
            tags::PLANE_POSITION_SEQUENCE,
            "enhanced_pet_plane_position_not_shared",
        ),
    ] {
        validate_item_absent(failures, relative_path, shared, tag, name);
    }
    for (tag, name) in [
        (
            tags::CARDIAC_SYNCHRONIZATION_SEQUENCE,
            "enhanced_pet_cardiac_synchronization_not_shared",
        ),
        (
            tags::RESPIRATORY_SYNCHRONIZATION_SEQUENCE,
            "enhanced_pet_respiratory_synchronization_not_shared",
        ),
        (
            tags::PET_FRAME_ACQUISITION_SEQUENCE,
            "enhanced_pet_frame_acquisition_not_shared",
        ),
        (
            tags::PET_DETECTOR_MOTION_DETAILS_SEQUENCE,
            "enhanced_pet_detector_motion_not_shared",
        ),
        (
            tags::PET_TABLE_DYNAMICS_SEQUENCE,
            "enhanced_pet_table_dynamics_not_shared",
        ),
        (
            tags::PET_POSITION_SEQUENCE,
            "enhanced_pet_position_not_shared",
        ),
        (
            tags::PET_FRAME_CORRECTION_FACTORS_SEQUENCE,
            "enhanced_pet_correction_factors_not_shared",
        ),
        (
            tags::PET_RECONSTRUCTION_SEQUENCE,
            "enhanced_pet_reconstruction_not_shared",
        ),
        (
            tags::PATIENT_PHYSIOLOGICAL_STATE_SEQUENCE,
            "enhanced_pet_physiological_state_not_shared",
        ),
    ] {
        validate_item_absent(failures, relative_path, shared, tag, name);
    }
    for (tag, name) in [
        (tags::PIXEL_MEASURES_SEQUENCE, "enhanced_pet_pixel_measures"),
        (
            tags::PLANE_ORIENTATION_SEQUENCE,
            "enhanced_pet_plane_orientation",
        ),
        (tags::FRAME_ANATOMY_SEQUENCE, "enhanced_pet_frame_anatomy"),
        (
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            "enhanced_pet_pixel_value_transformation",
        ),
        (tags::FRAME_VOILUT_SEQUENCE, "enhanced_pet_frame_voi_lut"),
        (
            tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE,
            "enhanced_pet_real_world_value_mapping",
        ),
        (
            tags::RADIOPHARMACEUTICAL_USAGE_SEQUENCE,
            "enhanced_pet_radiopharmaceutical_usage",
        ),
        (tags::PET_FRAME_TYPE_SEQUENCE, "enhanced_pet_pet_frame_type"),
    ] {
        match item_sequence_item_count_for_validate(shared, tag) {
            Ok(actual) => validate_equal(failures, relative_path, name, actual, 1),
            Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
        }
    }
    let derivation_count = manifest_u64(
        manifest_path,
        expected,
        "/derivation_image_item_count",
        "Enhanced PET derivation item count must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_derivation_manifest_contract",
        derivation_count,
        0,
    );
    match item_sequence_item_count_for_validate(shared, tags::DERIVATION_IMAGE_SEQUENCE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "enhanced_pet_derivation_image_empty",
            actual,
            derivation_count,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: enhanced_pet_derivation_image_empty: {err}"
        )),
    }

    if let Ok(pixel_measures) =
        item_sequence_item_for_validate(shared, tags::PIXEL_MEASURES_SEQUENCE, 0)
    {
        for (tag, name) in [
            (tags::PIXEL_SPACING, "enhanced_pet_pixel_spacing"),
            (tags::SLICE_THICKNESS, "enhanced_pet_slice_thickness"),
            (
                tags::SPACING_BETWEEN_SLICES,
                "enhanced_pet_spacing_between_slices",
            ),
        ] {
            validate_item_vr(
                failures,
                relative_path,
                pixel_measures,
                tag,
                name,
                dicom_core::VR::DS,
            );
        }
        for (pointer, tag, name, locked) in [
            (
                "/slice_thickness_mm",
                tags::SLICE_THICKNESS,
                "enhanced_pet_slice_thickness",
                5.0,
            ),
            (
                "/spacing_between_slices_mm",
                tags::SPACING_BETWEEN_SLICES,
                "enhanced_pet_spacing_between_slices",
                5.0,
            ),
        ] {
            let declared = manifest_f64(
                manifest_path,
                expected,
                pointer,
                "Enhanced PET spacing must be numeric",
            )?;
            validate_equal(
                failures,
                relative_path,
                &format!("{name}_manifest_contract"),
                declared,
                locked,
            );
            validate_item_f64(failures, relative_path, pixel_measures, tag, name, declared);
        }
        let spacing = manifest_f64_array(
            manifest_path,
            expected,
            "/pixel_spacing_mm",
            "Enhanced PET pixel_spacing_mm must be numeric",
        )?;
        validate_equal_debug(
            failures,
            relative_path,
            "enhanced_pet_pixel_spacing_manifest_contract",
            spacing.clone(),
            vec![2.0, 2.0],
        );
        validate_item_f64_array(
            failures,
            relative_path,
            pixel_measures,
            tags::PIXEL_SPACING,
            "enhanced_pet_pixel_spacing",
            spacing,
        );
    }
    if let Ok(orientation) =
        item_sequence_item_for_validate(shared, tags::PLANE_ORIENTATION_SEQUENCE, 0)
    {
        validate_item_vr(
            failures,
            relative_path,
            orientation,
            tags::IMAGE_ORIENTATION_PATIENT,
            "enhanced_pet_image_orientation_patient",
            dicom_core::VR::DS,
        );
        let declared = manifest_f64_array(
            manifest_path,
            expected,
            "/image_orientation_patient",
            "Enhanced PET orientation must be numeric",
        )?;
        validate_equal_debug(
            failures,
            relative_path,
            "enhanced_pet_orientation_manifest_contract",
            declared.clone(),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        );
        validate_item_f64_array(
            failures,
            relative_path,
            orientation,
            tags::IMAGE_ORIENTATION_PATIENT,
            "enhanced_pet_image_orientation_patient",
            declared,
        );
    }
    if let Ok(anatomy) = item_sequence_item_for_validate(shared, tags::FRAME_ANATOMY_SEQUENCE, 0) {
        validate_item_vr(
            failures,
            relative_path,
            anatomy,
            tags::FRAME_LATERALITY,
            "enhanced_pet_frame_laterality",
            dicom_core::VR::CS,
        );
        let laterality = manifest_str(
            manifest_path,
            expected,
            "/frame_laterality",
            "Enhanced PET frame_laterality must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            "enhanced_pet_frame_laterality_manifest_contract",
            laterality,
            "U",
        );
        validate_item_type1_str_element(
            failures,
            relative_path,
            anatomy,
            tags::FRAME_LATERALITY,
            "enhanced_pet_frame_laterality",
            laterality,
        );
        validate_enhanced_pet_code(
            failures,
            relative_path,
            manifest_path,
            expected
                .pointer("/anatomic_region")
                .ok_or(ValidateError::ManifestShape {
                    path: manifest_path.to_path_buf(),
                    message: "Enhanced PET anatomic_region must be an object",
                })?,
            anatomy,
            tags::ANATOMIC_REGION_SEQUENCE,
            "enhanced_pet_anatomic_region",
            ("69536005", "SCT", "Head"),
        )?;
    }
    if let Ok(transform) =
        item_sequence_item_for_validate(shared, tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE, 0)
    {
        for (tag, name, vr) in [
            (
                tags::RESCALE_INTERCEPT,
                "enhanced_pet_rescale_intercept",
                dicom_core::VR::DS,
            ),
            (
                tags::RESCALE_SLOPE,
                "enhanced_pet_rescale_slope",
                dicom_core::VR::DS,
            ),
            (
                tags::RESCALE_TYPE,
                "enhanced_pet_rescale_type",
                dicom_core::VR::LO,
            ),
        ] {
            validate_item_vr(failures, relative_path, transform, tag, name, vr);
        }
        for (pointer, tag, name, locked) in [
            (
                "/rescale_intercept",
                tags::RESCALE_INTERCEPT,
                "enhanced_pet_rescale_intercept",
                0.0,
            ),
            (
                "/rescale_slope",
                tags::RESCALE_SLOPE,
                "enhanced_pet_rescale_slope",
                2.5,
            ),
        ] {
            let declared = manifest_f64(
                manifest_path,
                expected,
                pointer,
                "Enhanced PET rescale scalar must be numeric",
            )?;
            validate_equal(
                failures,
                relative_path,
                &format!("{name}_manifest_contract"),
                declared,
                locked,
            );
            validate_item_f64(failures, relative_path, transform, tag, name, declared);
        }
        let rescale_type = manifest_str(
            manifest_path,
            expected,
            "/rescale_type",
            "Enhanced PET rescale_type must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            "enhanced_pet_rescale_type_manifest_contract",
            rescale_type,
            "US",
        );
        validate_item_type1_str_element(
            failures,
            relative_path,
            transform,
            tags::RESCALE_TYPE,
            "enhanced_pet_rescale_type",
            rescale_type,
        );
    }
    if let Ok(voi) = item_sequence_item_for_validate(shared, tags::FRAME_VOILUT_SEQUENCE, 0) {
        for (tag, name) in [
            (tags::WINDOW_CENTER, "enhanced_pet_window_center"),
            (tags::WINDOW_WIDTH, "enhanced_pet_window_width"),
        ] {
            validate_item_vr(failures, relative_path, voi, tag, name, dicom_core::VR::DS);
        }
        for (pointer, tag, name, locked) in [
            (
                "/window_center",
                tags::WINDOW_CENTER,
                "enhanced_pet_window_center",
                500.0,
            ),
            (
                "/window_width",
                tags::WINDOW_WIDTH,
                "enhanced_pet_window_width",
                1000.0,
            ),
        ] {
            let declared = manifest_f64(
                manifest_path,
                expected,
                pointer,
                "Enhanced PET VOI scalar must be numeric",
            )?;
            validate_equal(
                failures,
                relative_path,
                &format!("{name}_manifest_contract"),
                declared,
                locked,
            );
            validate_item_f64(failures, relative_path, voi, tag, name, declared);
        }
    }
    if let Ok(frame_type) =
        item_sequence_item_for_validate(shared, tags::PET_FRAME_TYPE_SEQUENCE, 0)
    {
        for (tag, name) in [
            (tags::FRAME_TYPE, "enhanced_pet_frame_type"),
            (
                tags::PIXEL_PRESENTATION,
                "enhanced_pet_frame_pixel_presentation",
            ),
            (
                tags::VOLUMETRIC_PROPERTIES,
                "enhanced_pet_frame_volumetric_properties",
            ),
            (
                tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
                "enhanced_pet_frame_volume_based_calculation_technique",
            ),
        ] {
            validate_item_vr(
                failures,
                relative_path,
                frame_type,
                tag,
                name,
                dicom_core::VR::CS,
            );
        }
        validate_item_type1_str_element(
            failures,
            relative_path,
            frame_type,
            tags::FRAME_TYPE,
            "enhanced_pet_frame_type",
            &frame_type_expected.join("\\"),
        );
        for (tag, name, expected_value) in [
            (
                tags::PIXEL_PRESENTATION,
                "enhanced_pet_frame_pixel_presentation",
                "MONOCHROME",
            ),
            (
                tags::VOLUMETRIC_PROPERTIES,
                "enhanced_pet_frame_volumetric_properties",
                "VOLUME",
            ),
            (
                tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
                "enhanced_pet_frame_volume_based_calculation_technique",
                "NONE",
            ),
        ] {
            validate_item_type1_str_element(
                failures,
                relative_path,
                frame_type,
                tag,
                name,
                expected_value,
            );
        }
    }
    validate_enhanced_pet_rwvm(failures, relative_path, manifest_path, expected, shared)?;
    if let Ok(usage) =
        item_sequence_item_for_validate(shared, tags::RADIOPHARMACEUTICAL_USAGE_SEQUENCE, 0)
    {
        validate_item_vr(
            failures,
            relative_path,
            usage,
            tags::RADIOPHARMACEUTICAL_AGENT_NUMBER,
            "enhanced_pet_usage_agent_number",
            dicom_core::VR::US,
        );
        let declared = manifest_u64(
            manifest_path,
            expected,
            "/radiopharmaceutical_usage_agent_number",
            "Enhanced PET usage agent number must be an integer",
        )? as u16;
        validate_equal(
            failures,
            relative_path,
            "enhanced_pet_usage_agent_manifest_contract",
            declared,
            1,
        );
        validate_item_u16(
            failures,
            relative_path,
            usage,
            tags::RADIOPHARMACEUTICAL_AGENT_NUMBER,
            "enhanced_pet_usage_agent_number",
            declared,
        );
    }
    Ok(())
}

fn validate_enhanced_pet_per_frame_groups(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let stack_ids = manifest_string_array(
        manifest_path,
        expected,
        "/stack_ids",
        "Enhanced PET stack_ids must be a string array",
    )?;
    let in_stack = manifest_u32_array(
        manifest_path,
        expected,
        "/in_stack_position_numbers",
        "Enhanced PET in-stack positions must be integers",
    )?;
    let dimensions = manifest_u32_array(
        manifest_path,
        expected,
        "/dimension_index_values",
        "Enhanced PET dimension values must be integers",
    )?;
    let temporal = manifest_u32_array(
        manifest_path,
        expected,
        "/temporal_position_indices",
        "Enhanced PET temporal positions must be integers",
    )?;
    let positions = manifest_array(
        manifest_path,
        expected,
        "/image_positions_patient_mm",
        "Enhanced PET image positions must be arrays",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_stack_ids_manifest_contract",
        stack_ids.clone(),
        vec!["1".to_string(), "1".to_string()],
    );
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_in_stack_positions_manifest_contract",
        in_stack.clone(),
        vec![1, 2],
    );
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_dimension_values_manifest_contract",
        dimensions.clone(),
        vec![1, 2],
    );
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_temporal_positions_manifest_contract",
        temporal.clone(),
        vec![1, 1],
    );
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_position_count_manifest_contract",
        positions.len(),
        2,
    );
    if stack_ids.len() != 2
        || in_stack.len() != 2
        || dimensions.len() != 2
        || temporal.len() != 2
        || positions.len() != 2
    {
        return Ok(());
    }

    let shared_only = [
        tags::PIXEL_MEASURES_SEQUENCE,
        tags::PLANE_ORIENTATION_SEQUENCE,
        tags::FRAME_ANATOMY_SEQUENCE,
        tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
        tags::FRAME_VOILUT_SEQUENCE,
        tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE,
        tags::RADIOPHARMACEUTICAL_USAGE_SEQUENCE,
        tags::PET_FRAME_TYPE_SEQUENCE,
        tags::DERIVATION_IMAGE_SEQUENCE,
    ];
    for index in 0..2 {
        let frame = match top_level_sequence_item_for_validate(
            obj,
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            index,
        ) {
            Ok(frame) => frame,
            Err(err) => {
                failures.push(format!(
                    "{relative_path}: enhanced_pet_per_frame_item[{index}]: {err}"
                ));
                continue;
            }
        };
        for tag in shared_only {
            validate_item_absent(
                failures,
                relative_path,
                frame,
                tag,
                &format!("enhanced_pet_shared_macro_not_per_frame[{index}]"),
            );
        }
        for (tag, name) in [
            (
                tags::CARDIAC_SYNCHRONIZATION_SEQUENCE,
                "cardiac_synchronization",
            ),
            (
                tags::RESPIRATORY_SYNCHRONIZATION_SEQUENCE,
                "respiratory_synchronization",
            ),
            (tags::PET_FRAME_ACQUISITION_SEQUENCE, "frame_acquisition"),
            (
                tags::PET_DETECTOR_MOTION_DETAILS_SEQUENCE,
                "detector_motion",
            ),
            (tags::PET_TABLE_DYNAMICS_SEQUENCE, "table_dynamics"),
            (tags::PET_POSITION_SEQUENCE, "position"),
            (
                tags::PET_FRAME_CORRECTION_FACTORS_SEQUENCE,
                "correction_factors",
            ),
            (tags::PET_RECONSTRUCTION_SEQUENCE, "reconstruction"),
            (
                tags::PATIENT_PHYSIOLOGICAL_STATE_SEQUENCE,
                "physiological_state",
            ),
        ] {
            validate_item_absent(
                failures,
                relative_path,
                frame,
                tag,
                &format!("enhanced_pet_{name}_not_per_frame[{index}]"),
            );
        }
        for (tag, name) in [
            (tags::FRAME_CONTENT_SEQUENCE, "enhanced_pet_frame_content"),
            (tags::PLANE_POSITION_SEQUENCE, "enhanced_pet_plane_position"),
        ] {
            match item_sequence_item_count_for_validate(frame, tag) {
                Ok(actual) => validate_equal(
                    failures,
                    relative_path,
                    &format!("{name}[{index}]"),
                    actual,
                    1,
                ),
                Err(err) => failures.push(format!("{relative_path}: {name}[{index}]: {err}")),
            }
        }
        if let Ok(content) = item_sequence_item_for_validate(frame, tags::FRAME_CONTENT_SEQUENCE, 0)
        {
            validate_item_vr(
                failures,
                relative_path,
                content,
                tags::STACK_ID,
                &format!("enhanced_pet_stack_id[{index}]"),
                dicom_core::VR::SH,
            );
            for (tag, name) in [
                (tags::IN_STACK_POSITION_NUMBER, "in_stack_position"),
                (tags::DIMENSION_INDEX_VALUES, "dimension_index_value"),
                (tags::TEMPORAL_POSITION_INDEX, "temporal_position_index"),
            ] {
                validate_item_vr(
                    failures,
                    relative_path,
                    content,
                    tag,
                    &format!("enhanced_pet_{name}[{index}]"),
                    dicom_core::VR::UL,
                );
            }
            validate_item_type1_str_element(
                failures,
                relative_path,
                content,
                tags::STACK_ID,
                &format!("enhanced_pet_stack_id[{index}]"),
                &stack_ids[index],
            );
            for (tag, name, value) in [
                (
                    tags::IN_STACK_POSITION_NUMBER,
                    "enhanced_pet_in_stack_position",
                    in_stack[index],
                ),
                (
                    tags::DIMENSION_INDEX_VALUES,
                    "enhanced_pet_dimension_index_value",
                    dimensions[index],
                ),
                (
                    tags::TEMPORAL_POSITION_INDEX,
                    "enhanced_pet_temporal_position_index",
                    temporal[index],
                ),
            ] {
                validate_item_u32(
                    failures,
                    relative_path,
                    content,
                    tag,
                    &format!("{name}[{index}]"),
                    value,
                );
            }
        }
        if let Ok(position) =
            item_sequence_item_for_validate(frame, tags::PLANE_POSITION_SEQUENCE, 0)
        {
            validate_item_vr(
                failures,
                relative_path,
                position,
                tags::IMAGE_POSITION_PATIENT,
                &format!("enhanced_pet_image_position_patient[{index}]"),
                dicom_core::VR::DS,
            );
            let declared = manifest_f64_array(
                manifest_path,
                &positions[index],
                "",
                "Enhanced PET image position must be numeric",
            )?;
            let locked = if index == 0 {
                vec![0.0, 0.0, 0.0]
            } else {
                vec![0.0, 0.0, 5.0]
            };
            validate_equal_debug(
                failures,
                relative_path,
                &format!("enhanced_pet_position_manifest_contract[{index}]"),
                declared.clone(),
                locked,
            );
            validate_item_f64_array(
                failures,
                relative_path,
                position,
                tags::IMAGE_POSITION_PATIENT,
                &format!("enhanced_pet_image_position_patient[{index}]"),
                declared,
            );
        }
    }
    Ok(())
}

fn validate_enhanced_pet_rwvm(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    shared: &DatasetObject,
) -> Result<(), ValidateError> {
    let rwvm_expected =
        expected
            .pointer("/real_world_value_mapping")
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "Enhanced PET real_world_value_mapping must be an object",
            })?;
    let rwvm =
        match item_sequence_item_for_validate(shared, tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE, 0) {
            Ok(rwvm) => rwvm,
            Err(err) => {
                failures.push(format!("{relative_path}: enhanced_pet_rwvm_content: {err}"));
                return Ok(());
            }
        };
    for (pointer, tag, name, locked) in [
        (
            "/first_value_mapped",
            tags::REAL_WORLD_VALUE_FIRST_VALUE_MAPPED,
            "enhanced_pet_rwvm_first_value",
            0_u64,
        ),
        (
            "/last_value_mapped",
            tags::REAL_WORLD_VALUE_LAST_VALUE_MAPPED,
            "enhanced_pet_rwvm_last_value",
            400,
        ),
    ] {
        let declared = manifest_u64(
            manifest_path,
            rwvm_expected,
            pointer,
            "Enhanced PET RWVM stored bound must be an integer",
        )? as u16;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            u64::from(declared),
            locked,
        );
        validate_item_u16(failures, relative_path, rwvm, tag, name, declared);
        validate_item_vr(failures, relative_path, rwvm, tag, name, dicom_core::VR::US);
    }
    for (pointer, tag, name, locked) in [
        (
            "/intercept",
            tags::REAL_WORLD_VALUE_INTERCEPT,
            "enhanced_pet_rwvm_intercept",
            0.0,
        ),
        (
            "/slope",
            tags::REAL_WORLD_VALUE_SLOPE,
            "enhanced_pet_rwvm_slope",
            2.5,
        ),
    ] {
        let declared = manifest_f64(
            manifest_path,
            rwvm_expected,
            pointer,
            "Enhanced PET RWVM scalar must be numeric",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            declared,
            locked,
        );
        validate_item_f64(failures, relative_path, rwvm, tag, name, declared);
        validate_item_vr(failures, relative_path, rwvm, tag, name, dicom_core::VR::FD);
    }
    for (pointer, tag, name, locked) in [
        (
            "/lut_label",
            tags::LUT_LABEL,
            "enhanced_pet_rwvm_lut_label",
            "BQML",
        ),
        (
            "/lut_explanation",
            tags::LUT_EXPLANATION,
            "enhanced_pet_rwvm_lut_explanation",
            "Activity concentration",
        ),
    ] {
        let declared = manifest_str(
            manifest_path,
            rwvm_expected,
            pointer,
            "Enhanced PET RWVM label must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            declared,
            locked,
        );
        validate_item_type1_str_element(failures, relative_path, rwvm, tag, name, declared);
        validate_item_vr(
            failures,
            relative_path,
            rwvm,
            tag,
            name,
            if tag == tags::LUT_LABEL {
                dicom_core::VR::SH
            } else {
                dicom_core::VR::LO
            },
        );
    }
    validate_enhanced_pet_code(
        failures,
        relative_path,
        manifest_path,
        rwvm_expected
            .pointer("/measurement_units")
            .ok_or(ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "Enhanced PET measurement_units must be an object",
            })?,
        rwvm,
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        "enhanced_pet_rwvm_measurement_units",
        ("Bq/ml", "UCUM", "Becquerels/milliliter"),
    )?;
    Ok(())
}

fn validate_enhanced_pet_isotope_and_corrections(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let isotope_expected = expected.pointer("/radiopharmaceutical_information").ok_or(
        ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Enhanced PET radiopharmaceutical_information must be an object",
        },
    )?;
    let isotope_count = manifest_u64(
        manifest_path,
        isotope_expected,
        "/item_count",
        "Enhanced PET isotope item_count must be an integer",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_isotope_count_manifest_contract",
        isotope_count,
        1,
    );
    match sequence_item_count_for_validate(obj, tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            "enhanced_pet_isotope_item_count",
            actual,
            isotope_count,
        ),
        Err(err) => failures.push(format!(
            "{relative_path}: enhanced_pet_isotope_item_count: {err}"
        )),
    }
    if let Ok(item) =
        top_level_sequence_item_for_validate(obj, tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE, 0)
    {
        validate_item_vr(
            failures,
            relative_path,
            item,
            tags::RADIOPHARMACEUTICAL_AGENT_NUMBER,
            "enhanced_pet_isotope_agent_number",
            dicom_core::VR::US,
        );
        let agent = manifest_u64(
            manifest_path,
            isotope_expected,
            "/agent_number",
            "Enhanced PET isotope agent_number must be an integer",
        )? as u16;
        validate_equal(
            failures,
            relative_path,
            "enhanced_pet_isotope_agent_manifest_contract",
            agent,
            1,
        );
        validate_item_u16(
            failures,
            relative_path,
            item,
            tags::RADIOPHARMACEUTICAL_AGENT_NUMBER,
            "enhanced_pet_isotope_agent_number",
            agent,
        );
        for (pointer, sequence, name, locked) in [
            (
                "/radionuclide",
                tags::RADIONUCLIDE_CODE_SEQUENCE,
                "enhanced_pet_radionuclide",
                ("77004003", "SCT", "^18^Fluorine"),
            ),
            (
                "/administration_route",
                tags::ADMINISTRATION_ROUTE_CODE_SEQUENCE,
                "enhanced_pet_administration_route",
                ("47625008", "SCT", "Intravenous route"),
            ),
            (
                "/radiopharmaceutical",
                tags::RADIOPHARMACEUTICAL_CODE_SEQUENCE,
                "enhanced_pet_radiopharmaceutical",
                ("35321007", "SCT", "Fluorodeoxyglucose F^18^"),
            ),
        ] {
            validate_enhanced_pet_code(
                failures,
                relative_path,
                manifest_path,
                isotope_expected
                    .pointer(pointer)
                    .ok_or(ValidateError::ManifestShape {
                        path: manifest_path.to_path_buf(),
                        message: "Enhanced PET isotope code must be an object",
                    })?,
                item,
                sequence,
                name,
                locked,
            )?;
        }
        let start = manifest_str(
            manifest_path,
            isotope_expected,
            "/start_datetime",
            "Enhanced PET start_datetime must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            "enhanced_pet_start_datetime_manifest_contract",
            start,
            "20260101000000",
        );
        validate_item_type1_str_element(
            failures,
            relative_path,
            item,
            tags::RADIOPHARMACEUTICAL_START_DATE_TIME,
            "enhanced_pet_start_datetime",
            start,
        );
        validate_item_vr(
            failures,
            relative_path,
            item,
            tags::RADIOPHARMACEUTICAL_START_DATE_TIME,
            "enhanced_pet_start_datetime",
            dicom_core::VR::DT,
        );
        let total_dose_present_empty = manifest_bool(
            manifest_path,
            isotope_expected,
            "/total_dose_present_empty",
            "Enhanced PET total_dose_present_empty must be a boolean",
        )?;
        validate_equal(
            failures,
            relative_path,
            "enhanced_pet_total_dose_manifest_contract",
            total_dose_present_empty,
            true,
        );
        validate_item_vr(
            failures,
            relative_path,
            item,
            tags::RADIONUCLIDE_TOTAL_DOSE,
            "enhanced_pet_total_dose_present_empty",
            dicom_core::VR::DS,
        );
        match item_str_for_validate(item, tags::RADIONUCLIDE_TOTAL_DOSE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "enhanced_pet_total_dose_present_empty",
                actual,
                String::new(),
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: enhanced_pet_total_dose_present_empty: {err}"
            )),
        }
        for (pointer, tag, name, locked) in [
            (
                "/half_life_seconds",
                tags::RADIONUCLIDE_HALF_LIFE,
                "enhanced_pet_half_life",
                6586.2,
            ),
            (
                "/positron_fraction",
                tags::RADIONUCLIDE_POSITRON_FRACTION,
                "enhanced_pet_positron_fraction",
                0.967,
            ),
        ] {
            let declared = manifest_f64(
                manifest_path,
                isotope_expected,
                pointer,
                "Enhanced PET isotope scalar must be numeric",
            )?;
            validate_equal(
                failures,
                relative_path,
                &format!("{name}_manifest_contract"),
                declared,
                locked,
            );
            validate_item_f64(failures, relative_path, item, tag, name, declared);
            validate_item_vr(failures, relative_path, item, tag, name, dicom_core::VR::DS);
        }
    }
    let corrections = expected
        .pointer("/corrections")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Enhanced PET corrections must be an object",
        })?;
    for (pointer, tag, name) in [
        (
            "/decay",
            tags::DECAY_CORRECTED,
            "enhanced_pet_decay_corrected",
        ),
        (
            "/attenuation",
            tags::ATTENUATION_CORRECTED,
            "enhanced_pet_attenuation_corrected",
        ),
        (
            "/scatter",
            tags::SCATTER_CORRECTED,
            "enhanced_pet_scatter_corrected",
        ),
        (
            "/dead_time",
            tags::DEAD_TIME_CORRECTED,
            "enhanced_pet_dead_time_corrected",
        ),
        (
            "/gantry_motion",
            tags::GANTRY_MOTION_CORRECTED,
            "enhanced_pet_gantry_motion_corrected",
        ),
        (
            "/patient_motion",
            tags::PATIENT_MOTION_CORRECTED,
            "enhanced_pet_patient_motion_corrected",
        ),
        (
            "/count_loss_normalization",
            tags::COUNT_LOSS_NORMALIZATION_CORRECTED,
            "enhanced_pet_count_loss_normalization_corrected",
        ),
        (
            "/randoms",
            tags::RANDOMS_CORRECTED,
            "enhanced_pet_randoms_corrected",
        ),
        (
            "/non_uniform_radial_sampling",
            tags::NON_UNIFORM_RADIAL_SAMPLING_CORRECTED,
            "enhanced_pet_non_uniform_radial_sampling_corrected",
        ),
        (
            "/sensitivity_calibration",
            tags::SENSITIVITY_CALIBRATED,
            "enhanced_pet_sensitivity_calibrated",
        ),
        (
            "/detector_normalization",
            tags::DETECTOR_NORMALIZATION_CORRECTION,
            "enhanced_pet_detector_normalization_correction",
        ),
    ] {
        let declared = manifest_str(
            manifest_path,
            corrections,
            pointer,
            "Enhanced PET correction flag must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_manifest_contract"),
            declared,
            "NO",
        );
        validate_type1_str_element(failures, relative_path, obj, tag, name, declared);
        match obj.element(tag) {
            Ok(element) => validate_equal(
                failures,
                relative_path,
                &format!("{name}_vr"),
                element.vr(),
                dicom_core::VR::CS,
            ),
            Err(err) => failures.push(format!("{relative_path}: {name}_vr: {err}")),
        }
    }
    Ok(())
}

fn validate_enhanced_pet_code(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected: &Value,
    parent: &DatasetObject,
    sequence_tag: dicom_core::Tag,
    name: &str,
    locked: (&str, &str, &str),
) -> Result<(), ValidateError> {
    match item_sequence_item_count_for_validate(parent, sequence_tag) {
        Ok(actual) => validate_equal(
            failures,
            relative_path,
            &format!("{name}_item_count"),
            actual,
            1,
        ),
        Err(err) => {
            failures.push(format!("{relative_path}: {name}_item_count: {err}"));
            return Ok(());
        }
    }
    let item = match item_sequence_item_for_validate(parent, sequence_tag, 0) {
        Ok(item) => item,
        Err(_) => return Ok(()),
    };
    for (pointer, tag, suffix, locked_value) in [
        ("/code_value", tags::CODE_VALUE, "code_value", locked.0),
        (
            "/coding_scheme_designator",
            tags::CODING_SCHEME_DESIGNATOR,
            "coding_scheme_designator",
            locked.1,
        ),
        (
            "/code_meaning",
            tags::CODE_MEANING,
            "code_meaning",
            locked.2,
        ),
    ] {
        let declared = manifest_str(
            manifest_path,
            expected,
            pointer,
            "Enhanced PET code field must be a string",
        )?;
        validate_equal(
            failures,
            relative_path,
            &format!("{name}_{suffix}_manifest_contract"),
            declared,
            locked_value,
        );
        validate_item_type1_str_element(
            failures,
            relative_path,
            item,
            tag,
            &format!("{name}_{suffix}"),
            declared,
        );
        validate_item_vr(
            failures,
            relative_path,
            item,
            tag,
            &format!("{name}_{suffix}"),
            if tag == tags::CODE_MEANING {
                dicom_core::VR::LO
            } else {
                dicom_core::VR::SH
            },
        );
    }
    Ok(())
}

fn validate_enhanced_pet_pixels_and_nonclaims(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    expected: &Value,
    obj: &OpenedObject,
) -> Result<(), ValidateError> {
    let frame_hashes = manifest_string_array(
        manifest_path,
        expected,
        "/frame_sha256",
        "Enhanced PET frame_sha256 must be a string array",
    )?;
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_frame_hash_manifest_contract",
        frame_hashes.clone(),
        vec!["03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784".to_string(); 2],
    );
    let pixel_hash = manifest_str(
        manifest_path,
        expected,
        "/pixel_data_sha256",
        "Enhanced PET pixel_data_sha256 must be a string",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_pixel_hash_manifest_contract",
        pixel_hash,
        "3a43b45e2f6d4d04fe4fc357dfc0efaa21caa5415ffc5db96fc19428d34a7bb5",
    );
    validate_equal_debug(
        failures,
        relative_path,
        "enhanced_pet_frame_hash_pixel_manifest_contract",
        manifest_string_array(
            manifest_path,
            file,
            "/pixel_data/frame_hashes",
            "Enhanced PET pixel_data frame_hashes must be a string array",
        )?,
        frame_hashes.clone(),
    );
    let pixel_bytes = match obj.element(tags::PIXEL_DATA) {
        Ok(element) => match element.value().to_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                failures.push(format!("{relative_path}: enhanced_pet_pixel_bytes: {err}"));
                return Ok(());
            }
        },
        Err(err) => {
            failures.push(format!("{relative_path}: enhanced_pet_pixel_bytes: {err}"));
            return Ok(());
        }
    };
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_pixel_byte_length",
        pixel_bytes.len(),
        16,
    );
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_pixel_data_sha256",
        sha256_hex(pixel_bytes.as_ref()),
        pixel_hash,
    );
    let stored_by_frame = manifest_array(
        manifest_path,
        expected,
        "/stored_values_by_frame",
        "Enhanced PET stored_values_by_frame must be an array",
    )?;
    let activity_by_frame = manifest_array(
        manifest_path,
        expected,
        "/activity_values_bqml_by_frame",
        "Enhanced PET activity_values_bqml_by_frame must be an array",
    )?;
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_stored_frame_count",
        stored_by_frame.len(),
        2,
    );
    validate_equal(
        failures,
        relative_path,
        "enhanced_pet_activity_frame_count",
        activity_by_frame.len(),
        2,
    );
    if pixel_bytes.len() == 16
        && frame_hashes.len() == 2
        && stored_by_frame.len() == 2
        && activity_by_frame.len() == 2
    {
        for (index, frame) in pixel_bytes.chunks_exact(8).enumerate() {
            validate_equal(
                failures,
                relative_path,
                &format!("enhanced_pet_frame_sha256[{index}]"),
                sha256_hex(frame),
                &frame_hashes[index],
            );
            let actual_stored = frame
                .chunks_exact(2)
                .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                .collect::<Vec<_>>();
            let declared_stored = manifest_u16_array(
                manifest_path,
                &stored_by_frame[index],
                "",
                "Enhanced PET stored frame must contain u16 values",
            )?;
            validate_equal_debug(
                failures,
                relative_path,
                &format!("enhanced_pet_stored_values_manifest_contract[{index}]"),
                declared_stored.clone(),
                vec![0, 100, 200, 400],
            );
            validate_equal_debug(
                failures,
                relative_path,
                &format!("enhanced_pet_stored_values[{index}]"),
                actual_stored.clone(),
                declared_stored,
            );
            let declared_activity = manifest_f64_array(
                manifest_path,
                &activity_by_frame[index],
                "",
                "Enhanced PET activity frame must contain numbers",
            )?;
            validate_equal_debug(
                failures,
                relative_path,
                &format!("enhanced_pet_activity_values_manifest_contract[{index}]"),
                declared_activity.clone(),
                vec![0.0, 250.0, 500.0, 1000.0],
            );
            let recomputed = actual_stored
                .iter()
                .map(|stored| f64::from(*stored) * 2.5)
                .collect::<Vec<_>>();
            validate_equal_debug(
                failures,
                relative_path,
                &format!("enhanced_pet_activity_recomputed[{index}]"),
                recomputed,
                declared_activity,
            );
        }
    }

    let nonclaims = expected
        .pointer("/nonclaims")
        .ok_or(ValidateError::ManifestShape {
            path: manifest_path.to_path_buf(),
            message: "Enhanced PET nonclaims must be an object",
        })?;
    for field in [
        "suv",
        "body_weight_normalization",
        "body_surface_area_normalization",
        "decay_corrected",
        "clinically_calibrated",
        "acquisition_counts",
        "actual_clinical_dose",
        "gating",
        "detector_motion",
        "time_of_flight_processing",
        "reconstruction",
    ] {
        validate_equal(
            failures,
            relative_path,
            &format!("enhanced_pet_nonclaim_{field}_manifest_contract"),
            manifest_bool(
                manifest_path,
                nonclaims,
                &format!("/{field}"),
                "Enhanced PET nonclaim must be boolean",
            )?,
            false,
        );
    }
    for (tag, name) in [
        (tags::UNITS, "enhanced_pet_suv_units_absent"),
        (tags::PATIENT_WEIGHT, "enhanced_pet_body_weight_absent"),
        (
            tags::DOSE_CALIBRATION_FACTOR,
            "enhanced_pet_clinical_calibration_absent",
        ),
        (
            tags::DECAY_CORRECTION,
            "enhanced_pet_decay_correction_absent",
        ),
        (tags::CORRECTED_IMAGE, "enhanced_pet_corrected_image_absent"),
        (
            tags::CARDIAC_SYNCHRONIZATION_SEQUENCE,
            "enhanced_pet_gating_absent",
        ),
        (
            tags::PET_DETECTOR_MOTION_DETAILS_SEQUENCE,
            "enhanced_pet_detector_motion_absent",
        ),
        (
            tags::PET_RECONSTRUCTION_SEQUENCE,
            "enhanced_pet_reconstruction_absent",
        ),
        (
            tags::GATED_INFORMATION_SEQUENCE,
            "enhanced_pet_gated_information_absent",
        ),
        (
            tags::COUNTS_ACCUMULATED,
            "enhanced_pet_acquisition_counts_absent",
        ),
        (
            tags::PRIMARY_PROMPTS_COUNTS_ACCUMULATED,
            "enhanced_pet_primary_counts_absent",
        ),
        (
            tags::SECONDARY_COUNTS_ACCUMULATED,
            "enhanced_pet_secondary_counts_absent",
        ),
    ] {
        validate_element_absent(failures, relative_path, obj, tag, name);
    }
    Ok(())
}

fn validate_enhanced_mr_timing_safety(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    file: &Value,
    shared: &DatasetObject,
) -> Result<(), ValidateError> {
    let timing_pointer = "/recipe/recipe_parameters/shared_functional_groups/mr_timing";
    let Some(expected_timing) = file.pointer(timing_pointer) else {
        return Ok(());
    };
    let timing = match item_sequence_item_for_validate(
        shared,
        tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
        0,
    ) {
        Ok(timing) => timing,
        Err(err) => {
            failures.push(format!(
                "{relative_path}: enhanced_mr_timing_sequence_type1c: {err}"
            ));
            return Ok(());
        }
    };

    if let Some(expected_sar) = expected_timing.get("specific_absorption_rate") {
        let sar = match item_sequence_item_for_validate(
            timing,
            tags::SPECIFIC_ABSORPTION_RATE_SEQUENCE,
            0,
        ) {
            Ok(sar) => sar,
            Err(err) => {
                failures.push(format!(
                    "{relative_path}: enhanced_mr_specific_absorption_rate_sequence_type1c: {err}"
                ));
                // Keep checking the independently required Operating Mode Sequence.
                return validate_enhanced_mr_operating_modes(
                    failures,
                    relative_path,
                    manifest_path,
                    expected_timing,
                    timing,
                );
            }
        };
        validate_item_type1_str_element(
            failures,
            relative_path,
            sar,
            tags::SPECIFIC_ABSORPTION_RATE_DEFINITION,
            "enhanced_mr_specific_absorption_rate_definition_type1",
            manifest_str(
                manifest_path,
                expected_sar,
                "/definition",
                "expected Enhanced MR SAR definition must be a string",
            )?,
        );
        let expected_value = expected_sar.get("value").and_then(Value::as_f64).ok_or(
            ValidateError::ManifestShape {
                path: manifest_path.to_path_buf(),
                message: "expected Enhanced MR SAR value must be a number",
            },
        )?;
        match item_f64_for_validate(sar, tags::SPECIFIC_ABSORPTION_RATE_VALUE) {
            Ok(actual) => validate_equal(
                failures,
                relative_path,
                "enhanced_mr_specific_absorption_rate_value_type1",
                actual,
                expected_value,
            ),
            Err(err) => failures.push(format!(
                "{relative_path}: enhanced_mr_specific_absorption_rate_value_type1: {err}"
            )),
        }
    }

    validate_enhanced_mr_operating_modes(
        failures,
        relative_path,
        manifest_path,
        expected_timing,
        timing,
    )
}

fn validate_enhanced_mr_operating_modes(
    failures: &mut Vec<String>,
    relative_path: &str,
    manifest_path: &Path,
    expected_timing: &Value,
    timing: &DatasetObject,
) -> Result<(), ValidateError> {
    if let Some(expected_modes) = expected_timing
        .get("operating_modes")
        .and_then(Value::as_array)
    {
        let actual_modes = match timing.element(tags::OPERATING_MODE_SEQUENCE) {
            Ok(element) => match element.items() {
                Some(items) => items,
                None => {
                    failures.push(format!(
                        "{relative_path}: enhanced_mr_operating_mode_sequence_type1c: element is not a sequence"
                    ));
                    return Ok(());
                }
            },
            Err(err) => {
                failures.push(format!(
                    "{relative_path}: enhanced_mr_operating_mode_sequence_type1c: {err}"
                ));
                return Ok(());
            }
        };
        validate_equal(
            failures,
            relative_path,
            "enhanced_mr_operating_mode_sequence_type1c",
            actual_modes.len(),
            expected_modes.len(),
        );
        for (index, (actual, expected)) in actual_modes.iter().zip(expected_modes).enumerate() {
            for (tag, field, name) in [
                (
                    tags::OPERATING_MODE_TYPE,
                    "type",
                    "enhanced_mr_operating_mode_type_type1",
                ),
                (
                    tags::OPERATING_MODE,
                    "mode",
                    "enhanced_mr_operating_mode_type1",
                ),
            ] {
                let expected = manifest_str(
                    manifest_path,
                    expected,
                    &format!("/{field}"),
                    "expected Enhanced MR operating mode field must be a string",
                )?;
                validate_item_type1_str_element(
                    failures,
                    relative_path,
                    actual,
                    tag,
                    &format!("{name}[{index}]"),
                    expected,
                );
            }
        }
    }

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

fn validate_item_type1_str_element(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: &str,
) {
    match item_str_for_validate(obj, tag) {
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

fn sequence_item_count_for_validate(
    obj: &OpenedObject,
    tag: dicom_core::Tag,
) -> Result<usize, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .items()
        .map(|items| items.len())
        .ok_or_else(|| format!("attribute {tag} is not a sequence"))
}

fn item_sequence_item_count_for_validate(
    obj: &DatasetObject,
    tag: dicom_core::Tag,
) -> Result<usize, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .items()
        .map(|items| items.len())
        .ok_or_else(|| format!("attribute {tag} is not a sequence"))
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

fn item_f64_for_validate(obj: &DatasetObject, tag: dicom_core::Tag) -> Result<f64, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_float64()
        .map_err(|err| err.to_string())
}

fn item_f64_values_for_validate(
    obj: &DatasetObject,
    tag: dicom_core::Tag,
) -> Result<Vec<f64>, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_multi_float64()
        .map_err(|err| err.to_string())
}

fn item_tag_for_validate(
    obj: &DatasetObject,
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

fn validate_item_absent(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
) {
    if matches!(obj.element_opt(tag), Ok(Some(_))) {
        failures.push(format!("{relative_path}: {name}: expected absent"));
    }
}

fn validate_item_vr(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: dicom_core::VR,
) {
    match obj.element(tag) {
        Ok(element) => validate_equal(
            failures,
            relative_path,
            &format!("{name}_vr"),
            element.vr(),
            expected,
        ),
        Err(err) => failures.push(format!("{relative_path}: {name}_vr: {err}")),
    }
}

fn validate_item_u16(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: u16,
) {
    match obj
        .element(tag)
        .map_err(|err| err.to_string())
        .and_then(|element| {
            element
                .value()
                .to_int::<u16>()
                .map_err(|err| err.to_string())
        }) {
        Ok(actual) => validate_equal(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn validate_item_u32(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: u32,
) {
    match obj
        .element(tag)
        .map_err(|err| err.to_string())
        .and_then(|element| {
            element
                .value()
                .to_int::<u32>()
                .map_err(|err| err.to_string())
        }) {
        Ok(actual) => validate_equal(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn validate_item_f64(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: f64,
) {
    match item_f64_for_validate(obj, tag) {
        Ok(actual) => validate_equal(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn validate_item_f64_array(
    failures: &mut Vec<String>,
    relative_path: &str,
    obj: &DatasetObject,
    tag: dicom_core::Tag,
    name: &str,
    expected: Vec<f64>,
) {
    match item_f64_values_for_validate(obj, tag) {
        Ok(actual) => validate_equal_debug(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
}

fn element_str_for_validate(obj: &OpenedObject, tag: dicom_core::Tag) -> Result<String, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_str()
        .map_err(|err| err.to_string())
        .map(|value| value.trim_matches('\0').trim().to_string())
}

fn element_f64_for_validate(obj: &OpenedObject, tag: dicom_core::Tag) -> Result<f64, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_float64()
        .map_err(|err| err.to_string())
}

fn element_f64_values_for_validate(
    obj: &OpenedObject,
    tag: dicom_core::Tag,
) -> Result<Vec<f64>, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_multi_float64()
        .map_err(|err| err.to_string())
}

fn element_u16_for_validate(obj: &OpenedObject, tag: dicom_core::Tag) -> Result<u16, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_int::<u16>()
        .map_err(|err| err.to_string())
}

fn element_u16_values_for_validate(
    obj: &OpenedObject,
    tag: dicom_core::Tag,
) -> Result<Vec<u16>, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .to_multi_int::<u16>()
        .map_err(|err| err.to_string())
}

fn element_tags_for_validate(
    obj: &OpenedObject,
    tag: dicom_core::Tag,
) -> Result<Vec<dicom_core::Tag>, String> {
    obj.element(tag)
        .map_err(|err| err.to_string())?
        .value()
        .tags()
        .map(|tags| tags.to_vec())
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

fn validate_equal_debug<A: fmt::Debug + PartialEq>(
    failures: &mut Vec<String>,
    relative_path: &str,
    name: &str,
    actual: A,
    expected: A,
) {
    if actual != expected {
        failures.push(format!(
            "{relative_path}: {name}: actual {actual:?}, expected {expected:?}"
        ));
    }
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
    append_count_map_section(
        &mut output,
        report,
        "Profile Memberships",
        "/grouped_coverage/profile_memberships",
    );
    append_count_map_section(
        &mut output,
        report,
        "Statuses",
        "/grouped_coverage/statuses",
    );
    append_count_map_section(&mut output, report, "IODs", "/grouped_coverage/iods");
    append_count_map_section(
        &mut output,
        report,
        "SOP Classes",
        "/grouped_coverage/sop_classes",
    );
    append_count_map_section(
        &mut output,
        report,
        "SOP Class Names",
        "/grouped_coverage/sop_class_names",
    );
    append_count_map_section(
        &mut output,
        report,
        "Modalities",
        "/grouped_coverage/modalities",
    );
    append_count_map_section(
        &mut output,
        report,
        "Transfer Syntaxes",
        "/grouped_coverage/transfer_syntaxes",
    );
    append_count_map_section(
        &mut output,
        report,
        "Transfer Syntax Names",
        "/grouped_coverage/transfer_syntax_names",
    );
    append_count_map_section(
        &mut output,
        report,
        "Codec Families",
        "/grouped_coverage/codec_families",
    );
    append_count_map_section(
        &mut output,
        report,
        "Codec Backends",
        "/grouped_coverage/codec_backends",
    );
    append_count_map_section(
        &mut output,
        report,
        "Codec Backend Kinds",
        "/grouped_coverage/codec_backend_kinds",
    );
    append_count_map_section(
        &mut output,
        report,
        "Codec Feature Gates",
        "/grouped_coverage/codec_feature_gates",
    );
    append_count_map_section(
        &mut output,
        report,
        "Generation Backends",
        "/grouped_coverage/generation_backends",
    );
    append_count_map_section(
        &mut output,
        report,
        "Determinism",
        "/grouped_coverage/determinism",
    );
    append_count_map_section(
        &mut output,
        report,
        "Validation Statuses",
        "/grouped_coverage/validation_statuses",
    );
    append_count_map_section(
        &mut output,
        report,
        "Unavailable Reasons",
        "/grouped_coverage/unavailable_reasons",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Specific Character Sets",
        "/grouped_coverage/metadata_specific_character_sets",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Person Names",
        "/grouped_coverage/metadata_person_names",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Person Name Component Groups",
        "/grouped_coverage/metadata_person_name_component_groups",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Person Name Component Group Counts",
        "/grouped_coverage/metadata_person_name_component_group_counts",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Person Name Encoded SHA-256 Values",
        "/grouped_coverage/metadata_person_name_encoded_sha256_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Person Name Encoded Byte Lengths",
        "/grouped_coverage/metadata_person_name_encoded_length_bytes",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Temporal Boundary IDs",
        "/grouped_coverage/metadata_temporal_boundary_ids",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Timezone Offsets From UTC",
        "/grouped_coverage/metadata_timezone_offsets_from_utc",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Empty Type 2 Attributes",
        "/grouped_coverage/metadata_empty_type2_attributes",
    );
    append_count_map_section(
        &mut output,
        report,
        "Metadata Empty Type 2 Attribute Counts",
        "/grouped_coverage/metadata_empty_type2_attribute_counts",
    );
    for (title, pointer) in [
        (
            "Metadata String Tags",
            "/grouped_coverage/metadata_string_tags",
        ),
        (
            "Metadata String VRs",
            "/grouped_coverage/metadata_string_vrs",
        ),
        (
            "Metadata String Value Multiplicities",
            "/grouped_coverage/metadata_string_value_multiplicities",
        ),
        (
            "Metadata String Maximum Component Encoded Byte Lengths",
            "/grouped_coverage/metadata_string_max_component_encoded_length_bytes",
        ),
        (
            "Metadata String Raw Value Lengths",
            "/grouped_coverage/metadata_string_raw_value_lengths",
        ),
        (
            "Metadata String Raw SHA-256 Values",
            "/grouped_coverage/metadata_string_raw_sha256_values",
        ),
        (
            "Metadata Private Creator Tags",
            "/grouped_coverage/metadata_private_creator_tags",
        ),
        (
            "Metadata Private Creator IDs",
            "/grouped_coverage/metadata_private_creator_ids",
        ),
        (
            "Metadata Private Block Ranges",
            "/grouped_coverage/metadata_private_block_ranges",
        ),
        (
            "Metadata Private Creator Raw SHA-256 Values",
            "/grouped_coverage/metadata_private_creator_raw_sha256_values",
        ),
        (
            "Metadata Private Element Tags",
            "/grouped_coverage/metadata_private_element_tags",
        ),
        (
            "Metadata Private Element VRs",
            "/grouped_coverage/metadata_private_element_vrs",
        ),
        (
            "Metadata Private Element Raw SHA-256 Values",
            "/grouped_coverage/metadata_private_element_raw_sha256_values",
        ),
        (
            "Metadata Sequence Length Variants",
            "/grouped_coverage/metadata_sequence_length_variants",
        ),
        (
            "Metadata Sequence Length Field Hex Values",
            "/grouped_coverage/metadata_sequence_length_field_hex_values",
        ),
        (
            "Metadata Sequence Delimitation States",
            "/grouped_coverage/metadata_sequence_delimitation_states",
        ),
        (
            "Metadata Sequence Item Length Encodings",
            "/grouped_coverage/metadata_sequence_item_length_encodings",
        ),
    ] {
        append_count_map_section(&mut output, report, title, pointer);
    }
    for (title, pointer) in [
        ("PET Units", "/grouped_coverage/pet_units"),
        ("PET Counts Sources", "/grouped_coverage/pet_counts_sources"),
        ("PET Series Types", "/grouped_coverage/pet_series_types"),
        (
            "PET Corrected Images",
            "/grouped_coverage/pet_corrected_images",
        ),
        (
            "PET Decay Corrections",
            "/grouped_coverage/pet_decay_corrections",
        ),
        (
            "PET Dose Calibration Factors",
            "/grouped_coverage/pet_dose_calibration_factors",
        ),
        (
            "PET Rescale Intercepts",
            "/grouped_coverage/pet_rescale_intercepts",
        ),
        ("PET Rescale Slopes", "/grouped_coverage/pet_rescale_slopes"),
        ("PET Stored Values", "/grouped_coverage/pet_stored_values"),
        (
            "PET Activity Values (BQML)",
            "/grouped_coverage/pet_activity_values_bqml",
        ),
        (
            "PET Frame Reference Times (ms)",
            "/grouped_coverage/pet_frame_reference_times_ms",
        ),
        (
            "PET Actual Frame Durations (ms)",
            "/grouped_coverage/pet_actual_frame_durations_ms",
        ),
        ("PET Image Indices", "/grouped_coverage/pet_image_indices"),
        (
            "PET Radiopharmaceutical Information Item Counts",
            "/grouped_coverage/pet_radiopharmaceutical_information_item_counts",
        ),
    ] {
        append_count_map_section(&mut output, report, title, pointer);
    }
    for (title, pointer) in [
        ("US Image Types", "/grouped_coverage/us_image_types"),
        (
            "US Frame Increment Pointers",
            "/grouped_coverage/us_frame_increment_pointers",
        ),
        ("US Frame Times (ms)", "/grouped_coverage/us_frame_times_ms"),
        ("US Frame Counts", "/grouped_coverage/us_frame_counts"),
        (
            "US Spatially Related Frame States",
            "/grouped_coverage/us_spatially_related_frames",
        ),
        (
            "US Color Data Present States",
            "/grouped_coverage/us_color_data_present",
        ),
        (
            "US Region Calibration States",
            "/grouped_coverage/us_region_calibrated",
        ),
        (
            "US Lossy Image Compression History",
            "/grouped_coverage/us_lossy_image_compressions",
        ),
    ] {
        append_count_map_section(&mut output, report, title, pointer);
    }
    for (title, pointer) in [
        ("XA Image Types", "/grouped_coverage/xa_image_types"),
        ("XA Frame Counts", "/grouped_coverage/xa_frame_counts"),
        (
            "XA Body Parts Examined",
            "/grouped_coverage/xa_body_parts_examined",
        ),
        (
            "XA Patient Orientation Empty States",
            "/grouped_coverage/xa_patient_orientation_empty_states",
        ),
        (
            "XA Laterality Present States",
            "/grouped_coverage/xa_laterality_present_states",
        ),
        (
            "XA Pixel Intensity Relationships",
            "/grouped_coverage/xa_pixel_intensity_relationships",
        ),
        (
            "XA Radiation Settings",
            "/grouped_coverage/xa_radiation_settings",
        ),
        ("XA KVPs", "/grouped_coverage/xa_kvps"),
        ("XA Exposures (mAs)", "/grouped_coverage/xa_exposures_mas"),
        (
            "XA Imager Pixel Spacings (mm)",
            "/grouped_coverage/xa_imager_pixel_spacings_mm",
        ),
        (
            "XA Positioner Primary Angles (degrees)",
            "/grouped_coverage/xa_positioner_primary_angles_degrees",
        ),
        (
            "XA Positioner Secondary Angles (degrees)",
            "/grouped_coverage/xa_positioner_secondary_angles_degrees",
        ),
        (
            "XA Source-to-Detector Distances (mm)",
            "/grouped_coverage/xa_distances_source_to_detector_mm",
        ),
        (
            "XA Source-to-Patient Distances (mm)",
            "/grouped_coverage/xa_distances_source_to_patient_mm",
        ),
        (
            "XA Estimated Radiographic Magnification Factors",
            "/grouped_coverage/xa_estimated_radiographic_magnification_factors",
        ),
        (
            "XA Lossy Image Compression History",
            "/grouped_coverage/xa_lossy_image_compressions",
        ),
        (
            "XA Multi-frame Cine States",
            "/grouped_coverage/xa_multiframe_cine_states",
        ),
        (
            "XA Biplane Data Present States",
            "/grouped_coverage/xa_biplane_data_present_states",
        ),
        (
            "XA Contrast Used States",
            "/grouped_coverage/xa_contrast_used_states",
        ),
        (
            "XA Subtraction Applied States",
            "/grouped_coverage/xa_subtraction_applied_states",
        ),
        (
            "XA Table Motion Present States",
            "/grouped_coverage/xa_table_motion_present_states",
        ),
        (
            "XA Patient-space Geometry Present States",
            "/grouped_coverage/xa_patient_space_geometry_present_states",
        ),
        (
            "XA Pixel Spacing Calibrated States",
            "/grouped_coverage/xa_pixel_spacing_calibrated_states",
        ),
    ] {
        append_count_map_section(&mut output, report, title, pointer);
    }
    for (title, pointer) in [
        ("XRF Image Types", "/grouped_coverage/xrf_image_types"),
        ("XRF Frame Counts", "/grouped_coverage/xrf_frame_counts"),
        (
            "XRF Body Parts Examined",
            "/grouped_coverage/xrf_body_parts_examined",
        ),
        (
            "XRF Patient Orientation Empty States",
            "/grouped_coverage/xrf_patient_orientation_empty_states",
        ),
        (
            "XRF Laterality Present States",
            "/grouped_coverage/xrf_laterality_present_states",
        ),
        (
            "XRF Pixel Intensity Relationships",
            "/grouped_coverage/xrf_pixel_intensity_relationships",
        ),
        (
            "XRF Radiation Settings",
            "/grouped_coverage/xrf_radiation_settings",
        ),
        ("XRF KVPs", "/grouped_coverage/xrf_kvps"),
        ("XRF Exposures (mAs)", "/grouped_coverage/xrf_exposures_mas"),
        (
            "XRF Imager Pixel Spacings (mm)",
            "/grouped_coverage/xrf_imager_pixel_spacings_mm",
        ),
        (
            "XRF Source-to-Detector Distances (mm)",
            "/grouped_coverage/xrf_distances_source_to_detector_mm",
        ),
        (
            "XRF Source-to-Patient Distances (mm)",
            "/grouped_coverage/xrf_distances_source_to_patient_mm",
        ),
        (
            "XRF Estimated Radiographic Magnification Factors",
            "/grouped_coverage/xrf_estimated_radiographic_magnification_factors",
        ),
        (
            "XRF Column Angulations (degrees)",
            "/grouped_coverage/xrf_column_angulations_degrees",
        ),
        (
            "XRF Lossy Image Compression History",
            "/grouped_coverage/xrf_lossy_image_compressions",
        ),
        (
            "XRF Multi-frame Cine States",
            "/grouped_coverage/xrf_multiframe_cine_states",
        ),
        (
            "XRF Biplane Data Present States",
            "/grouped_coverage/xrf_biplane_data_present_states",
        ),
        (
            "XRF Contrast Used States",
            "/grouped_coverage/xrf_contrast_used_states",
        ),
        (
            "XRF Subtraction Applied States",
            "/grouped_coverage/xrf_subtraction_applied_states",
        ),
        (
            "XRF Table Position Present States",
            "/grouped_coverage/xrf_table_position_present_states",
        ),
        (
            "XRF Table Motion Present States",
            "/grouped_coverage/xrf_table_motion_present_states",
        ),
        (
            "XRF Table Tilt Present States",
            "/grouped_coverage/xrf_table_tilt_present_states",
        ),
        (
            "XRF Tomography Present States",
            "/grouped_coverage/xrf_tomography_present_states",
        ),
        (
            "XRF Patient-space Geometry Present States",
            "/grouped_coverage/xrf_patient_space_geometry_present_states",
        ),
        (
            "XRF Pixel Spacing Calibrated States",
            "/grouped_coverage/xrf_pixel_spacing_calibrated_states",
        ),
        (
            "XRF XA Positioner Angles Present States",
            "/grouped_coverage/xrf_xa_positioner_angles_present_states",
        ),
    ] {
        append_count_map_section(&mut output, report, title, pointer);
    }
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
        "Bits Allocated",
        "/grouped_coverage/bits_allocated",
    );
    append_count_map_section(
        &mut output,
        report,
        "Bits Stored",
        "/grouped_coverage/bits_stored",
    );
    append_count_map_section(
        &mut output,
        report,
        "High Bits",
        "/grouped_coverage/high_bits",
    );
    append_count_map_section(
        &mut output,
        report,
        "Pixel Representations",
        "/grouped_coverage/pixel_representations",
    );
    append_count_map_section(
        &mut output,
        report,
        "Samples Per Pixel",
        "/grouped_coverage/samples_per_pixel",
    );
    append_count_map_section(
        &mut output,
        report,
        "Planar Configurations",
        "/grouped_coverage/planar_configurations",
    );
    append_count_map_section(
        &mut output,
        report,
        "Pixel Data VRs",
        "/grouped_coverage/pixel_data_vrs",
    );
    append_count_map_section(
        &mut output,
        report,
        "Pixel Data Layouts",
        "/grouped_coverage/pixel_data_layouts",
    );
    append_count_map_section(
        &mut output,
        report,
        "Unsigned 32-bit Stored Value Sets",
        "/grouped_coverage/u32_stored_value_sets",
    );
    append_count_map_section(
        &mut output,
        report,
        "Unsigned 32-bit Pixel Data SHA-256 Values",
        "/grouped_coverage/u32_pixel_data_sha256_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Unsigned 32-bit Word Byte Orders",
        "/grouped_coverage/u32_word_byte_orders",
    );
    append_count_map_section(
        &mut output,
        report,
        "Unsigned 32-bit Full-range States",
        "/grouped_coverage/u32_full_unsigned_range_states",
    );
    append_count_map_section(
        &mut output,
        report,
        "One-bit Stored Value Sets",
        "/grouped_coverage/u1_stored_value_sets",
    );
    append_count_map_section(
        &mut output,
        report,
        "One-bit Pixel Data SHA-256 Values",
        "/grouped_coverage/u1_pixel_data_sha256_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "One-bit Packing Orders",
        "/grouped_coverage/u1_packing_orders",
    );
    append_count_map_section(
        &mut output,
        report,
        "One-bit Frame Boundary Policies",
        "/grouped_coverage/u1_frame_boundary_policies",
    );
    append_count_map_section(
        &mut output,
        report,
        "One-bit Value Field Padding Byte Counts",
        "/grouped_coverage/u1_value_field_padding_byte_counts",
    );
    append_count_map_section(
        &mut output,
        report,
        "Basic Offset Tables",
        "/grouped_coverage/basic_offset_tables",
    );
    append_count_map_section(
        &mut output,
        report,
        "Encapsulated Fragment Layouts",
        "/grouped_coverage/encapsulated_fragment_layouts",
    );
    append_count_map_section(
        &mut output,
        report,
        "Extended Offset Tables",
        "/grouped_coverage/extended_offset_tables",
    );
    append_count_map_section(
        &mut output,
        report,
        "Frame Counts",
        "/grouped_coverage/frame_counts",
    );
    append_count_map_section(
        &mut output,
        report,
        "Geometries",
        "/grouped_coverage/geometries",
    );
    append_count_map_section(
        &mut output,
        report,
        "Pixel Spacings",
        "/grouped_coverage/pixel_spacings",
    );
    append_count_map_section(
        &mut output,
        report,
        "Imager Pixel Spacings",
        "/grouped_coverage/imager_pixel_spacings",
    );
    append_count_map_section(
        &mut output,
        report,
        "Image Orientations Patient",
        "/grouped_coverage/image_orientations_patient",
    );
    append_count_map_section(
        &mut output,
        report,
        "Image Positions Patient",
        "/grouped_coverage/image_positions_patient",
    );
    append_count_map_section(
        &mut output,
        report,
        "Slice Thicknesses",
        "/grouped_coverage/slice_thicknesses",
    );
    append_count_map_section(
        &mut output,
        report,
        "Spacing Between Slices",
        "/grouped_coverage/spacing_between_slices",
    );
    append_count_map_section(
        &mut output,
        report,
        "Slice Locations",
        "/grouped_coverage/slice_locations",
    );
    append_count_map_section(
        &mut output,
        report,
        "Object Types",
        "/grouped_coverage/object_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Derived Reference States",
        "/grouped_coverage/derived_reference_states",
    );
    append_count_map_section(
        &mut output,
        report,
        "Derived Reference Relationships",
        "/grouped_coverage/derived_reference_relationships",
    );
    append_count_map_section(
        &mut output,
        report,
        "Derived Reference Targets",
        "/grouped_coverage/derived_reference_targets",
    );
    append_count_map_section(
        &mut output,
        report,
        "Derived Reference SOP Class UIDs",
        "/grouped_coverage/derived_reference_sop_class_uids",
    );
    append_count_map_section(
        &mut output,
        report,
        "Derived Reference SOP Instance UID Roots",
        "/grouped_coverage/derived_reference_sop_instance_uid_roots",
    );
    append_count_map_section(
        &mut output,
        report,
        "Synthetic Data",
        "/grouped_coverage/synthetic_data",
    );
    append_count_map_section(
        &mut output,
        report,
        "Lossy Image Compression",
        "/grouped_coverage/lossy_image_compression",
    );
    append_count_map_section(
        &mut output,
        report,
        "Lossy Image Compression Ratios",
        "/grouped_coverage/lossy_image_compression_ratios",
    );
    append_count_map_section(
        &mut output,
        report,
        "Lossy Image Compression Methods",
        "/grouped_coverage/lossy_image_compression_methods",
    );
    append_count_map_section(
        &mut output,
        report,
        "Image Types",
        "/grouped_coverage/image_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Conversion Types",
        "/grouped_coverage/conversion_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Presentation LUT Shapes",
        "/grouped_coverage/presentation_lut_shapes",
    );
    append_count_map_section(
        &mut output,
        report,
        "Window Centers",
        "/grouped_coverage/window_centers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Window Widths",
        "/grouped_coverage/window_widths",
    );
    append_count_map_section(&mut output, report, "KVPs", "/grouped_coverage/kvps");
    append_count_map_section(
        &mut output,
        report,
        "CT Acquisition Numbers",
        "/grouped_coverage/ct_acquisition_numbers",
    );
    append_count_map_section(
        &mut output,
        report,
        "CT Rescale Intercepts",
        "/grouped_coverage/ct_rescale_intercepts",
    );
    append_count_map_section(
        &mut output,
        report,
        "CT Rescale Slopes",
        "/grouped_coverage/ct_rescale_slopes",
    );
    append_count_map_section(
        &mut output,
        report,
        "CT Rescale Types",
        "/grouped_coverage/ct_rescale_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced CT Dimension Index Values",
        "/grouped_coverage/enhanced_ct_dimension_index_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced CT In-concatenation Numbers",
        "/grouped_coverage/enhanced_ct_in_concatenation_numbers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced CT In-concatenation Total Numbers",
        "/grouped_coverage/enhanced_ct_in_concatenation_total_numbers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced CT Concatenation Frame Offset Numbers",
        "/grouped_coverage/enhanced_ct_concatenation_frame_offset_numbers",
    );
    for (title, pointer) in [
        (
            "NM Frame Increment Pointers",
            "/grouped_coverage/nm_frame_increment_pointers",
        ),
        (
            "NM Energy Window Vectors",
            "/grouped_coverage/nm_energy_window_vectors",
        ),
        (
            "NM Detector Vectors",
            "/grouped_coverage/nm_detector_vectors",
        ),
        (
            "NM Energy Window Names",
            "/grouped_coverage/nm_energy_window_names",
        ),
        (
            "NM Detector Start Angles (degrees)",
            "/grouped_coverage/nm_detector_start_angles_degrees",
        ),
        (
            "NM Frame Dimension Tuples",
            "/grouped_coverage/nm_frame_dimension_tuples",
        ),
    ] {
        append_count_map_section(&mut output, report, title, pointer);
    }
    append_count_map_section(
        &mut output,
        report,
        "MR Scanning Sequences",
        "/grouped_coverage/mr_scanning_sequences",
    );
    append_count_map_section(
        &mut output,
        report,
        "MR Sequence Variants",
        "/grouped_coverage/mr_sequence_variants",
    );
    append_count_map_section(
        &mut output,
        report,
        "MR Acquisition Types",
        "/grouped_coverage/mr_acquisition_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "MR Repetition Times",
        "/grouped_coverage/mr_repetition_times",
    );
    append_count_map_section(
        &mut output,
        report,
        "MR Echo Times",
        "/grouped_coverage/mr_echo_times",
    );
    append_count_map_section(
        &mut output,
        report,
        "MR Echo Train Lengths",
        "/grouped_coverage/mr_echo_train_lengths",
    );
    append_count_map_section(
        &mut output,
        report,
        "MR Magnetic Field Strengths",
        "/grouped_coverage/mr_magnetic_field_strengths",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Effective Echo Times",
        "/grouped_coverage/enhanced_mr_effective_echo_times",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Temporal Position Time Offsets (seconds)",
        "/grouped_coverage/enhanced_mr_temporal_position_time_offsets",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Temporal Position Indices",
        "/grouped_coverage/enhanced_mr_temporal_position_indices",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Dimension Index Values",
        "/grouped_coverage/enhanced_mr_dimension_index_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Frame Acquisition Numbers",
        "/grouped_coverage/enhanced_mr_frame_acquisition_numbers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Dimension Index Pointers",
        "/grouped_coverage/enhanced_mr_dimension_index_pointers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Functional Group Pointers",
        "/grouped_coverage/enhanced_mr_functional_group_pointers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Temporal Position Time Offset Units",
        "/grouped_coverage/enhanced_mr_temporal_position_time_offset_units",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Velocity Encoding Minimum Values",
        "/grouped_coverage/enhanced_mr_velocity_encoding_minimum_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Enhanced MR Velocity Encoding Maximum Values",
        "/grouped_coverage/enhanced_mr_velocity_encoding_maximum_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Segmentation Types",
        "/grouped_coverage/segmentation_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Segmentation Fractional Types",
        "/grouped_coverage/segmentation_fractional_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Segmentation Maximum Fractional Values",
        "/grouped_coverage/segmentation_maximum_fractional_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Content Labels",
        "/grouped_coverage/gsps_content_labels",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Content Descriptions",
        "/grouped_coverage/gsps_content_descriptions",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Presentation Size Modes",
        "/grouped_coverage/gsps_presentation_size_modes",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Presentation Pixel Aspect Ratios",
        "/grouped_coverage/gsps_presentation_pixel_aspect_ratios",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Window Centers",
        "/grouped_coverage/gsps_window_centers",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Window Widths",
        "/grouped_coverage/gsps_window_widths",
    );
    append_count_map_section(
        &mut output,
        report,
        "GSPS Presentation LUT Shapes",
        "/grouped_coverage/gsps_presentation_lut_shapes",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Content Labels",
        "/grouped_coverage/rwvm_content_labels",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM LUT Labels",
        "/grouped_coverage/rwvm_lut_labels",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM First Values Mapped",
        "/grouped_coverage/rwvm_first_values_mapped",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Last Values Mapped",
        "/grouped_coverage/rwvm_last_values_mapped",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Intercepts",
        "/grouped_coverage/rwvm_intercepts",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Slopes",
        "/grouped_coverage/rwvm_slopes",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Units Code Values",
        "/grouped_coverage/rwvm_units_code_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Units Coding Scheme Designators",
        "/grouped_coverage/rwvm_units_coding_scheme_designators",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Units Code Meanings",
        "/grouped_coverage/rwvm_units_code_meanings",
    );
    append_count_map_section(
        &mut output,
        report,
        "RWVM Referenced Frame Numbers",
        "/grouped_coverage/rwvm_referenced_frame_numbers",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Dose Units",
        "/grouped_coverage/rt_dose_units",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Dose Types",
        "/grouped_coverage/rt_dose_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Dose Summation Types",
        "/grouped_coverage/rt_dose_summation_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Dose Grid Scalings",
        "/grouped_coverage/rt_dose_grid_scalings",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Structure Set Labels",
        "/grouped_coverage/rt_structure_set_labels",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Structure Set ROI Names",
        "/grouped_coverage/rt_structure_set_roi_names",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT ROI Generation Algorithms",
        "/grouped_coverage/rt_roi_generation_algorithms",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Contour Geometric Types",
        "/grouped_coverage/rt_contour_geometric_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT Contour Points",
        "/grouped_coverage/rt_contour_points",
    );
    append_count_map_section(
        &mut output,
        report,
        "RT ROI Interpreted Types",
        "/grouped_coverage/rt_roi_interpreted_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Encapsulated Document Burned In Annotations",
        "/grouped_coverage/encapsulated_document_burned_in_annotations",
    );
    append_count_map_section(
        &mut output,
        report,
        "Encapsulated Document Recognizable Visual Features",
        "/grouped_coverage/encapsulated_document_recognizable_visual_features",
    );
    append_count_map_section(
        &mut output,
        report,
        "Encapsulated Document Titles",
        "/grouped_coverage/encapsulated_document_titles",
    );
    append_count_map_section(
        &mut output,
        report,
        "Encapsulated Document MIME Types",
        "/grouped_coverage/encapsulated_document_mime_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Encapsulated Document Lengths",
        "/grouped_coverage/encapsulated_document_lengths",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Completion Flags",
        "/grouped_coverage/sr_completion_flags",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Verification Flags",
        "/grouped_coverage/sr_verification_flags",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Root Value Types",
        "/grouped_coverage/sr_root_value_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Root Continuity Of Content",
        "/grouped_coverage/sr_root_continuity_of_content",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Content Sequence Item Counts",
        "/grouped_coverage/sr_content_sequence_item_counts",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Observation Texts",
        "/grouped_coverage/sr_observation_texts",
    );
    append_count_map_section(
        &mut output,
        report,
        "SR Measurement Numeric Values",
        "/grouped_coverage/sr_measurement_numeric_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "KOS Document Titles",
        "/grouped_coverage/kos_document_titles",
    );
    append_count_map_section(
        &mut output,
        report,
        "KOS Key Object Counts",
        "/grouped_coverage/kos_key_object_counts",
    );
    append_count_map_section(
        &mut output,
        report,
        "KOS Key Object Relationship Types",
        "/grouped_coverage/kos_key_object_relationship_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "KOS Key Object Value Types",
        "/grouped_coverage/kos_key_object_value_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "KOS Referenced Frame Numbers",
        "/grouped_coverage/kos_referenced_frame_numbers",
    );
    append_count_map_section(
        &mut output,
        report,
        "Modality LUT Descriptors",
        "/grouped_coverage/modality_lut_descriptors",
    );
    append_count_map_section(
        &mut output,
        report,
        "Modality LUT Types",
        "/grouped_coverage/modality_lut_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Modality LUT Data Value Lengths",
        "/grouped_coverage/modality_lut_data_value_lengths",
    );
    append_count_map_section(
        &mut output,
        report,
        "VOI LUT Descriptors",
        "/grouped_coverage/voi_lut_descriptors",
    );
    append_count_map_section(
        &mut output,
        report,
        "VOI LUT Data Value Lengths",
        "/grouped_coverage/voi_lut_data_value_lengths",
    );
    append_count_map_section(
        &mut output,
        report,
        "Overlay Geometries",
        "/grouped_coverage/overlay_geometries",
    );
    append_count_map_section(
        &mut output,
        report,
        "Overlay Types",
        "/grouped_coverage/overlay_types",
    );
    append_count_map_section(
        &mut output,
        report,
        "Overlay Origins",
        "/grouped_coverage/overlay_origins",
    );
    append_count_map_section(
        &mut output,
        report,
        "Overlay Bits Allocated",
        "/grouped_coverage/overlay_bits_allocated",
    );
    append_count_map_section(
        &mut output,
        report,
        "Overlay Bit Positions",
        "/grouped_coverage/overlay_bit_positions",
    );
    append_count_map_section(
        &mut output,
        report,
        "Overlay Data Value Lengths",
        "/grouped_coverage/overlay_data_value_lengths",
    );
    append_count_map_section(
        &mut output,
        report,
        "Display Shutter Shapes",
        "/grouped_coverage/display_shutter_shapes",
    );
    append_count_map_section(
        &mut output,
        report,
        "Display Shutter Presentation Values",
        "/grouped_coverage/display_shutter_presentation_values",
    );
    append_count_map_section(
        &mut output,
        report,
        "Body Parts Examined",
        "/grouped_coverage/body_parts_examined",
    );
    append_count_map_section(
        &mut output,
        report,
        "View Positions",
        "/grouped_coverage/view_positions",
    );
    append_count_map_section(
        &mut output,
        report,
        "Study Instance UID Roots",
        "/grouped_coverage/study_instance_uid_roots",
    );
    append_count_map_section(
        &mut output,
        report,
        "Series Instance UID Roots",
        "/grouped_coverage/series_instance_uid_roots",
    );
    append_count_map_section(
        &mut output,
        report,
        "SOP Instance UID Roots",
        "/grouped_coverage/sop_instance_uid_roots",
    );
    append_count_map_section(
        &mut output,
        report,
        "Known Stressors",
        "/grouped_coverage/known_stressors",
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

    let nm_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["nm_frame_increment_pointers"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !nm_rows.is_empty() {
        output.push_str("## Nuclear Medicine Multi-frame Expectations\n\n");
        output.push_str("| Case ID | Frame increment pointers | Energy vector | Detector vector | Energy window names | Detector start angles (degrees) | Frame tuples (frame:window:detector) |\n");
        output.push_str("|---|---|---|---|---|---|---|\n");
        for row in nm_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("nm_frame_increment_pointers")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("nm_energy_window_vector").and_then(Value::as_str)),
                markdown_cell(row.get("nm_detector_vector").and_then(Value::as_str)),
                markdown_cell(row.get("nm_energy_window_names").and_then(Value::as_str)),
                markdown_cell(
                    row.get("nm_detector_start_angles_degrees")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("nm_frame_dimension_tuples").and_then(Value::as_str))
            ));
        }
        output.push('\n');
    }

    let enhanced_pet_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["enhanced_pet_image_type"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !enhanced_pet_rows.is_empty() {
        output.push_str("## Enhanced PET Multi-frame Expectations\n\n");
        output.push_str("| Case ID | Image type | Frame type | View | View modifiers | Slice progression present | In-stack positions | Dimension values | Image positions (mm) | Stored values by frame | Activity values (BQML) by frame | RWVM intercept | RWVM slope | RWVM units | Corrections |\n");
        output.push_str("|---|---|---|---|---:|---|---|---|---|---|---|---:|---:|---|---|\n");
        for row in enhanced_pet_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("enhanced_pet_image_type").and_then(Value::as_str)),
                markdown_cell(row.get("enhanced_pet_frame_type").and_then(Value::as_str)),
                markdown_cell(row.get("enhanced_pet_view_code").and_then(Value::as_str)),
                markdown_number(row.get("enhanced_pet_view_modifier_item_count")),
                markdown_bool(row.get("enhanced_pet_slice_progression_direction_present")),
                markdown_cell(
                    row.get("enhanced_pet_in_stack_position_numbers")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_pet_dimension_index_values")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_pet_image_positions_patient_mm")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_pet_stored_values_by_frame")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_pet_activity_values_bqml_by_frame")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("enhanced_pet_rwvm_intercept")),
                markdown_number(row.get("enhanced_pet_rwvm_slope")),
                markdown_cell(
                    row.get("enhanced_pet_rwvm_measurement_units")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("enhanced_pet_corrections").and_then(Value::as_str)),
            ));
        }
        output.push('\n');
    }

    let pet_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["pet_units"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !pet_rows.is_empty() {
        output.push_str("## PET Activity Expectations\n\n");
        output.push_str("| Case ID | Units | Counts source | Series type | Corrected image | Decay correction | Dose calibration factor | Rescale intercept | Rescale slope | Stored values | Activity values (BQML) | Frame reference time (ms) | Actual frame duration (ms) | Image index | Radiopharmaceutical items |\n");
        output.push_str("|---|---|---|---|---|---|---:|---:|---:|---|---|---:|---:|---:|---:|\n");
        for row in pet_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("pet_units").and_then(Value::as_str)),
                markdown_cell(row.get("pet_counts_source").and_then(Value::as_str)),
                markdown_cell(row.get("pet_series_type").and_then(Value::as_str)),
                markdown_cell(row.get("pet_corrected_image").and_then(Value::as_str)),
                markdown_cell(row.get("pet_decay_correction").and_then(Value::as_str)),
                markdown_number(row.get("pet_dose_calibration_factor")),
                markdown_number(row.get("pet_rescale_intercept")),
                markdown_number(row.get("pet_rescale_slope")),
                markdown_cell(row.get("pet_stored_values").and_then(Value::as_str)),
                markdown_cell(row.get("pet_activity_values_bqml").and_then(Value::as_str)),
                markdown_number(row.get("pet_frame_reference_time_ms")),
                markdown_number(row.get("pet_actual_frame_duration_ms")),
                markdown_number(row.get("pet_image_index")),
                markdown_number(row.get("pet_radiopharmaceutical_information_item_count"))
            ));
        }
        output.push('\n');
    }

    let us_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["us_image_type"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !us_rows.is_empty() {
        output.push_str("## Ultrasound Multi-frame Expectations\n\n");
        output.push_str("| Case ID | Image type | Frame increment pointer | Frame time (ms) | Relative times (ms) | Frame count | Ordered frame SHA-256 values | Spatially related | Color data present | Region calibrated | Lossy image compression |\n");
        output.push_str("|---|---|---|---:|---|---:|---|---|---|---|---|\n");
        for row in us_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("us_image_type").and_then(Value::as_str)),
                markdown_cell(
                    row.get("us_frame_increment_pointer")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("us_frame_time_ms")),
                markdown_cell(
                    row.get("us_frame_relative_times_ms")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("us_frame_count")),
                markdown_cell(row.get("us_ordered_frame_hashes").and_then(Value::as_str)),
                markdown_bool(row.get("us_spatially_related_frames")),
                markdown_bool(row.get("us_color_data_present")),
                markdown_bool(row.get("us_region_calibrated")),
                markdown_cell(
                    row.get("us_lossy_image_compression")
                        .and_then(Value::as_str)
                )
            ));
        }
        output.push('\n');
    }

    let xa_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["xa_image_type"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !xa_rows.is_empty() {
        output.push_str("## X-Ray Angiographic Projection Expectations\n\n");
        output.push_str("| Case ID | Image type | Frames | Body part | Patient orientation empty | Laterality present | Pixel intensity relationship | Radiation setting | KVP | Exposure (mAs) | Imager pixel spacing (mm) | Primary angle (degrees) | Secondary angle (degrees) | SID (mm) | Source-to-patient distance (mm) | Estimated magnification | Lossy compression | Multi-frame cine | Biplane data | Contrast used | Subtraction applied | Table motion | Patient-space geometry | Pixel spacing calibrated |\n");
        output.push_str("|---|---|---:|---|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---:|---|---|---|---|---|---|---|---|\n");
        for row in xa_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("xa_image_type").and_then(Value::as_str)),
                markdown_number(row.get("xa_frame_count")),
                markdown_cell(row.get("xa_body_part_examined").and_then(Value::as_str)),
                markdown_bool(row.get("xa_patient_orientation_empty")),
                markdown_bool(row.get("xa_laterality_present")),
                markdown_cell(
                    row.get("xa_pixel_intensity_relationship")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("xa_radiation_setting").and_then(Value::as_str)),
                markdown_number(row.get("xa_kvp")),
                markdown_number(row.get("xa_exposure_mas")),
                markdown_cell(
                    row.get("xa_imager_pixel_spacing_mm")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("xa_positioner_primary_angle_degrees")),
                markdown_number(row.get("xa_positioner_secondary_angle_degrees")),
                markdown_number(row.get("xa_distance_source_to_detector_mm")),
                markdown_number(row.get("xa_distance_source_to_patient_mm")),
                markdown_number(
                    row.get("xa_estimated_radiographic_magnification_factor")
                ),
                markdown_cell(
                    row.get("xa_lossy_image_compression")
                        .and_then(Value::as_str)
                ),
                markdown_bool(row.get("xa_multiframe_cine")),
                markdown_bool(row.get("xa_biplane_data_present")),
                markdown_bool(row.get("xa_contrast_used")),
                markdown_bool(row.get("xa_subtraction_applied")),
                markdown_bool(row.get("xa_table_motion_present")),
                markdown_bool(row.get("xa_patient_space_geometry_present")),
                markdown_bool(row.get("xa_pixel_spacing_calibrated"))
            ));
        }
        output.push('\n');
    }

    let xrf_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["xrf_image_type"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !xrf_rows.is_empty() {
        output.push_str("## X-Ray Radiofluoroscopic Projection Expectations\n\n");
        output.push_str("| Case ID | Image type | Frames | Body part | Patient orientation empty | Laterality present | Pixel intensity relationship | Radiation setting | KVP | Exposure (mAs) | Imager pixel spacing (mm) | SID (mm) | Source-to-patient distance (mm) | Estimated magnification | Column angulation (degrees) | Lossy compression | Multi-frame cine | Biplane data | Contrast used | Subtraction applied | Table position | Table motion | Table tilt | Tomography | Patient-space geometry | Pixel spacing calibrated | XA positioner angles |\n");
        output.push_str("|---|---|---:|---|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        for row in xrf_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("xrf_image_type").and_then(Value::as_str)),
                markdown_number(row.get("xrf_frame_count")),
                markdown_cell(row.get("xrf_body_part_examined").and_then(Value::as_str)),
                markdown_bool(row.get("xrf_patient_orientation_empty")),
                markdown_bool(row.get("xrf_laterality_present")),
                markdown_cell(row.get("xrf_pixel_intensity_relationship").and_then(Value::as_str)),
                markdown_cell(row.get("xrf_radiation_setting").and_then(Value::as_str)),
                markdown_number(row.get("xrf_kvp")),
                markdown_number(row.get("xrf_exposure_mas")),
                markdown_cell(row.get("xrf_imager_pixel_spacing_mm").and_then(Value::as_str)),
                markdown_number(row.get("xrf_distance_source_to_detector_mm")),
                markdown_number(row.get("xrf_distance_source_to_patient_mm")),
                markdown_number(row.get("xrf_estimated_radiographic_magnification_factor")),
                markdown_number(row.get("xrf_column_angulation_degrees")),
                markdown_cell(row.get("xrf_lossy_image_compression").and_then(Value::as_str)),
                markdown_bool(row.get("xrf_multiframe_cine")),
                markdown_bool(row.get("xrf_biplane_data_present")),
                markdown_bool(row.get("xrf_contrast_used")),
                markdown_bool(row.get("xrf_subtraction_applied")),
                markdown_bool(row.get("xrf_table_position_present")),
                markdown_bool(row.get("xrf_table_motion_present")),
                markdown_bool(row.get("xrf_table_tilt_present")),
                markdown_bool(row.get("xrf_tomography_present")),
                markdown_bool(row.get("xrf_patient_space_geometry_present")),
                markdown_bool(row.get("xrf_pixel_spacing_calibrated")),
                markdown_bool(row.get("xrf_xa_positioner_angles_present"))
            ));
        }
        output.push('\n');
    }

    let enhanced_mr_temporal_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["enhanced_mr_temporal_position_time_offsets"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !enhanced_mr_temporal_rows.is_empty() {
        output.push_str("## Enhanced MR Temporal Expectations\n\n");
        output.push_str("| Case ID | Temporal indices | Dimension indices | Frame acquisition numbers | Time offsets (s) | Dimension pointer | Functional-group pointer |\n");
        output.push_str("|---|---|---|---|---|---|---|\n");
        for row in enhanced_mr_temporal_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("enhanced_mr_temporal_position_indices")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_mr_dimension_index_values")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_mr_frame_acquisition_numbers")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_mr_temporal_position_time_offsets")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_mr_dimension_index_pointer")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("enhanced_mr_functional_group_pointer")
                        .and_then(Value::as_str)
                )
            ));
        }
        output.push('\n');
    }

    let metadata_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["metadata_specific_character_sets"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !metadata_rows.is_empty() {
        output.push_str("## Metadata and VR Expectations\n\n");
        output.push_str("| Case ID | Specific Character Set | Person Name | Component groups | Group count | Encoded SHA-256 | Encoded bytes |\n");
        output.push_str("|---|---|---|---|---:|---|---:|\n");
        for row in metadata_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("metadata_specific_character_sets")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("metadata_person_name").and_then(Value::as_str)),
                markdown_cell(
                    row.get("metadata_person_name_component_groups")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("metadata_person_name_component_group_count")),
                markdown_cell(
                    row.get("metadata_person_name_encoded_sha256")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("metadata_person_name_encoded_length_bytes")),
            ));
        }
        output.push('\n');
    }

    let temporal_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["metadata_temporal_boundary_id"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !temporal_rows.is_empty() {
        output.push_str("## Temporal Metadata Expectations\n\n");
        output
            .push_str("| Case ID | Boundary | DA | TM | DT | Timezone offset | Normalized UTC |\n");
        output.push_str("|---|---|---|---|---|---|---|\n");
        for row in temporal_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("metadata_temporal_boundary_id")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("metadata_da_values").and_then(Value::as_str)),
                markdown_cell(row.get("metadata_tm_values").and_then(Value::as_str)),
                markdown_cell(row.get("metadata_dt_values").and_then(Value::as_str)),
                markdown_cell(
                    row.get("metadata_timezone_offset_from_utc")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("metadata_temporal_normalized_utc")
                        .and_then(Value::as_str)
                ),
            ));
        }
        output.push('\n');
    }

    let empty_type2_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["metadata_empty_type2_attributes"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !empty_type2_rows.is_empty() {
        output.push_str("## Empty Type 2 Metadata Expectations\n\n");
        output.push_str("| Case ID | Attributes | Count |\n");
        output.push_str("|---|---|---:|\n");
        for row in empty_type2_rows {
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("metadata_empty_type2_attributes")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("metadata_empty_type2_attribute_count")),
            ));
        }
        output.push('\n');
    }

    let string_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["metadata_string_tags"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !string_rows.is_empty() {
        output.push_str("## String VR Boundary Expectations\n\n");
        output.push_str(
            "| Case ID | Tags | VRs | VMs | Max component bytes | Raw VLs | Raw SHA-256 values |\n",
        );
        output.push_str("|---|---|---|---|---|---|---|\n");
        for row in string_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_value_array(row.get("metadata_string_tags")),
                markdown_value_array(row.get("metadata_string_vrs")),
                markdown_value_array(row.get("metadata_string_value_multiplicities")),
                markdown_value_array(row.get("metadata_string_max_component_encoded_length_bytes")),
                markdown_value_array(row.get("metadata_string_raw_value_lengths")),
                markdown_value_array(row.get("metadata_string_raw_sha256_values")),
            ));
        }
        output.push('\n');
    }

    let private_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["metadata_private_creator_tags"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !private_rows.is_empty() {
        output.push_str("## Private Creator Block Expectations\n\n");
        output.push_str("| Case ID | Creator tags | Creator IDs | Block ranges | Creator raw SHA-256 values | Element tags | Element VRs | Element raw SHA-256 values |\n");
        output.push_str("|---|---|---|---|---|---|---|---|\n");
        for row in private_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_value_array(row.get("metadata_private_creator_tags")),
                markdown_value_array(row.get("metadata_private_creator_ids")),
                markdown_value_array(row.get("metadata_private_block_ranges")),
                markdown_value_array(row.get("metadata_private_creator_raw_sha256_values")),
                markdown_value_array(row.get("metadata_private_element_tags")),
                markdown_value_array(row.get("metadata_private_element_vrs")),
                markdown_value_array(row.get("metadata_private_element_raw_sha256_values")),
            ));
        }
        output.push('\n');
    }

    let sequence_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["metadata_sequence_length_variant"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !sequence_rows.is_empty() {
        output.push_str("## Sequence Length Encoding Expectations\n\n");
        output.push_str("| Case ID | Variant | Sequence tag | SQ VL | Length field | Sequence delimiter | Item length | Item delimiter | Decoded code |\n");
        output.push_str("|---|---|---|---:|---|---|---|---|---|\n");
        for row in sequence_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("metadata_sequence_length_variant")
                        .and_then(Value::as_str)
                ),
                markdown_cell(row.get("metadata_sequence_tag").and_then(Value::as_str)),
                markdown_number(row.get("metadata_sequence_value_length")),
                markdown_cell(
                    row.get("metadata_sequence_length_field_hex")
                        .and_then(Value::as_str)
                ),
                markdown_bool(row.get("metadata_sequence_delimitation_present")),
                markdown_cell(
                    row.get("metadata_sequence_item_length_encoding")
                        .and_then(Value::as_str)
                ),
                markdown_bool(row.get("metadata_sequence_item_delimitation_present")),
                markdown_cell(
                    row.get("metadata_sequence_decoded_code")
                        .and_then(Value::as_str)
                ),
            ));
        }
        output.push('\n');
    }

    let geometry_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["geometry_sort_basis"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !geometry_rows.is_empty() {
        output.push_str("## Geometry Sorting Expectations\n\n");
        output.push_str("| Case ID | Position along normal (mm) | Geometric rank | Instance Number state | Instance Number | Instance rank | Adjacent spacing (mm) | Uniform spacing | Gantry tilt (degrees) | Basis | Direction | Conflict expected |\n");
        output.push_str("|---|---:|---:|---|---:|---:|---|---|---:|---|---|---|\n");
        for row in geometry_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_number(row.get("geometry_position_along_normal_mm")),
                markdown_number(row.get("geometry_geometric_order_index")),
                markdown_cell(
                    row.get("geometry_instance_number_state")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("geometry_instance_number")),
                markdown_number(row.get("geometry_instance_number_order_index")),
                markdown_number_list(row.get("geometry_adjacent_spacing_mm")),
                markdown_bool(row.get("geometry_spacing_uniform")),
                markdown_number(row.get("geometry_gantry_detector_tilt_degrees")),
                markdown_cell(row.get("geometry_sort_basis").and_then(Value::as_str)),
                markdown_cell(row.get("geometry_sort_direction").and_then(Value::as_str)),
                markdown_bool(row.get("geometry_sorting_conflict_expected"))
            ));
        }
        output.push('\n');
    }

    let series_organization_rows = report
        .get("coverage_matrix")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["series_organization_group_id"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !series_organization_rows.is_empty() {
        output.push_str("## Cross-Series Organization Expectations\n\n");
        output.push_str("| Case ID | Group | Study series count | Series ordinal | Instances in series | Shared study UID | Shared frame of reference UID | Distinct series UIDs |\n");
        output.push_str("|---|---|---:|---:|---:|---|---|---|\n");
        for row in series_organization_rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("series_organization_group_id")
                        .and_then(Value::as_str)
                ),
                markdown_number(row.get("study_series_count")),
                markdown_number(row.get("series_ordinal")),
                markdown_number(row.get("series_organization_instance_count")),
                markdown_bool(row.get("shared_study_instance_uid_expected")),
                markdown_bool(row.get("shared_frame_of_reference_uid_expected")),
                markdown_bool(row.get("distinct_series_instance_uids_expected"))
            ));
        }
        output.push('\n');
    }

    output.push_str("## Coverage Matrix\n\n");
    output.push_str("| Case ID | Status | Profile | IOD | Transfer Syntax | Photometric | Bits | Frames | Generation Backend | Backend Version | Backend Determinism | Validation |\n");
    output.push_str("|---|---|---|---|---|---|---:|---:|---|---|---|---|\n");
    if let Some(rows) = report.get("coverage_matrix").and_then(Value::as_array) {
        for row in rows {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(row.get("case_id").and_then(Value::as_str)),
                markdown_cell(row.get("status").and_then(Value::as_str)),
                markdown_cell(row.get("profile").and_then(Value::as_str)),
                markdown_cell(row.get("iod").and_then(Value::as_str)),
                markdown_cell(row.get("transfer_syntax").and_then(Value::as_str)),
                markdown_cell(row.get("photometric").and_then(Value::as_str)),
                markdown_number(row.get("bits")),
                markdown_number(row.get("frames")),
                markdown_cell(row.get("generation_backend_id").and_then(Value::as_str)),
                markdown_cell(
                    row.get("generation_backend_version")
                        .and_then(Value::as_str)
                ),
                markdown_cell(
                    row.get("generation_backend_determinism")
                        .and_then(Value::as_str)
                ),
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
    let derived_reference_relationships = manifest_reference_relationships(manifest_path, file)?;
    let derived_reference_sop_class_uids = manifest_reference_sop_class_uids(manifest_path, file)?;
    let derived_reference_sop_instance_uid_roots =
        manifest_reference_sop_instance_uid_roots(manifest_path, file)?;
    let transfer_syntax = report_str(
        manifest_path,
        file,
        "/dicom/transfer_syntax_uid",
        "dicom transfer_syntax_uid must be a string",
    )?;
    let transfer_syntax_name = report_str(
        manifest_path,
        file,
        "/dicom/transfer_syntax_name",
        "dicom transfer_syntax_name must be a string",
    )?;
    let codec = file.pointer("/pixel_data/codec");
    let generation_backend = file.pointer("/generation_backend");
    let metadata = metadata_report_fields(file);
    let nm = nm_multiframe_report_fields(file);
    let pet = pet_activity_report_fields(file);
    let enhanced_pet = enhanced_pet_report_fields(manifest_path, file)?;
    let us = us_multiframe_report_fields(manifest_path, file)?;
    let xa = xa_projection_report_fields(manifest_path, file)?;
    let xrf = xrf_projection_report_fields(manifest_path, file)?;
    let u32_pixels = u32_pixel_report_fields(manifest_path, file)?;
    let u1_pixels = u1_pixel_report_fields(manifest_path, file)?;
    let mut row = serde_json::json!({
        "case_id": report_str(manifest_path, file, "/case_id", "file case_id must be a string")?,
        "profile": run_profile,
        "profile_membership": report_string_array(manifest_path, file, "/profile_membership", "file profile_membership must be a string array")?,
        "status": "generated",
        "iod": report_str(manifest_path, file, "/dicom/iod_name", "dicom iod_name must be a string")?,
        "modality": file.pointer("/dicom/modality").and_then(Value::as_str),
        "sop_class_uid": report_str(manifest_path, file, "/dicom/sop_class_uid", "dicom sop_class_uid must be a string")?,
        "transfer_syntax": transfer_syntax,
        "transfer_syntax_name": transfer_syntax_name,
        "codec_family": compressed_codec_family(transfer_syntax),
        "codec_backend_id": codec.and_then(|codec| codec.get("backend_id")).and_then(Value::as_str),
        "codec_backend_kind": codec.and_then(|codec| codec.get("backend_kind")).and_then(Value::as_str),
        "codec_feature_gate": codec.and_then(|codec| codec.get("feature_gate")).and_then(Value::as_str),
        "reason_code": Value::Null,
        "photometric": file.pointer("/image/photometric_interpretation").and_then(Value::as_str),
        "bits": file.pointer("/image/bits_stored").and_then(Value::as_u64),
        "bits_allocated": file.pointer("/image/bits_allocated").and_then(Value::as_u64),
        "bits_stored": file.pointer("/image/bits_stored").and_then(Value::as_u64),
        "high_bit": file.pointer("/image/high_bit").and_then(Value::as_u64),
        "pixel_representation": file.pointer("/image/pixel_representation").and_then(Value::as_u64),
        "samples_per_pixel": file.pointer("/image/samples_per_pixel").and_then(Value::as_u64),
        "planar_configuration": file.pointer("/image/planar_configuration").and_then(Value::as_u64),
        "pixel_data_vr": file.pointer("/pixel_data/vr").and_then(Value::as_str),
        "pixel_data_layout": file.pointer("/pixel_data/native_or_encapsulated").and_then(Value::as_str),
        "basic_offset_table": basic_offset_table_state(file),
        "encapsulated_fragment_layout": encapsulated_fragment_layout(file),
        "extended_offset_table": extended_offset_table_state(file),
        "frames": file.pointer("/image/frames").and_then(Value::as_u64),
        "geometry": {
            "rows": file.pointer("/image/rows").and_then(Value::as_u64),
            "columns": file.pointer("/image/columns").and_then(Value::as_u64),
            "spacing": Value::Null,
            "orientation": Value::Null
        },
        "derived_refs": derived_refs.clone(),
        "derived_reference_relationships": derived_reference_relationships,
        "validation_status": file.pointer("/validation/status").and_then(Value::as_str).unwrap_or("not_run"),
        "determinism": report_str(manifest_path, file, "/determinism", "determinism must be a string")?,
        "object_type": file.get("case_id").and_then(Value::as_str).and_then(|case_id| case_id.split('/').next()),
        "synthetic_data": file.pointer("/expected_semantics/synthetic_data").and_then(Value::as_str),
        "image_type": file.pointer("/expected_semantics/image_type").and_then(Value::as_str),
        "conversion_type": file.pointer("/expected_semantics/conversion_type").and_then(Value::as_str),
        "presentation_lut_shape": file.pointer("/recipe/recipe_parameters/presentation_lut_shape").and_then(Value::as_str),
        "lossy_image_compression": file.pointer("/expected_semantics/lossy_image_compression").and_then(Value::as_str),
        "lossy_image_compression_ratio": file.pointer("/expected_semantics/lossy_image_compression_ratio").and_then(Value::as_str),
        "lossy_image_compression_method": file.pointer("/expected_semantics/lossy_image_compression_method").and_then(Value::as_str),
    });
    let row_object = row
        .as_object_mut()
        .expect("generated coverage row literal must be an object");
    for (field, value) in [
        (
            "u32_stored_values",
            u32_pixels.stored_values.map(Value::from),
        ),
        (
            "u32_pixel_data_sha256",
            u32_pixels.pixel_data_sha256.map(Value::from),
        ),
        (
            "u32_word_byte_order",
            u32_pixels.word_byte_order.map(Value::from),
        ),
        (
            "u32_full_unsigned_range",
            u32_pixels.full_unsigned_range.map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, value) in [
        ("u1_stored_values", u1_pixels.stored_values.map(Value::from)),
        (
            "u1_decoded_frame_sha256",
            u1_pixels.decoded_frame_sha256.map(Value::from),
        ),
        (
            "u1_pixel_data_sha256",
            u1_pixels.pixel_data_sha256.map(Value::from),
        ),
        ("u1_packing_order", u1_pixels.packing_order.map(Value::from)),
        (
            "u1_frame_boundary_policy",
            u1_pixels.frame_boundary_policy.map(Value::from),
        ),
        (
            "u1_significant_bits",
            u1_pixels.significant_bits.map(Value::from),
        ),
        (
            "u1_unused_high_bits",
            u1_pixels.unused_high_bits.map(Value::from),
        ),
        (
            "u1_value_field_padding_bytes",
            u1_pixels.value_field_padding_bytes.map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, value) in [
        (
            "metadata_specific_character_sets",
            metadata.specific_character_sets.map(Value::from),
        ),
        (
            "metadata_person_name",
            metadata.person_name.map(Value::from),
        ),
        (
            "metadata_person_name_component_groups",
            metadata.person_name_component_groups.map(Value::from),
        ),
        (
            "metadata_person_name_component_group_count",
            metadata.person_name_component_group_count.map(Value::from),
        ),
        (
            "metadata_person_name_encoded_sha256",
            metadata.person_name_encoded_sha256.map(Value::from),
        ),
        (
            "metadata_person_name_encoded_length_bytes",
            metadata.person_name_encoded_length_bytes.map(Value::from),
        ),
        (
            "metadata_temporal_boundary_id",
            metadata.temporal_boundary_id.map(Value::from),
        ),
        (
            "metadata_timezone_offset_from_utc",
            metadata.timezone_offset_from_utc.map(Value::from),
        ),
        ("metadata_da_values", metadata.date_values.map(Value::from)),
        ("metadata_tm_values", metadata.time_values.map(Value::from)),
        (
            "metadata_dt_values",
            metadata.date_time_values.map(Value::from),
        ),
        (
            "metadata_temporal_normalized_utc",
            metadata.temporal_normalized_utc.map(Value::from),
        ),
        (
            "metadata_empty_type2_attributes",
            metadata.empty_type2_attributes.map(Value::from),
        ),
        (
            "metadata_empty_type2_attribute_count",
            metadata.empty_type2_attribute_count.map(Value::from),
        ),
        (
            "metadata_string_tags",
            metadata.string_tags.map(Value::from),
        ),
        ("metadata_string_vrs", metadata.string_vrs.map(Value::from)),
        (
            "metadata_string_value_multiplicities",
            metadata.string_value_multiplicities.map(Value::from),
        ),
        (
            "metadata_string_max_component_encoded_length_bytes",
            metadata
                .string_max_component_encoded_length_bytes
                .map(Value::from),
        ),
        (
            "metadata_string_raw_value_lengths",
            metadata.string_raw_value_lengths.map(Value::from),
        ),
        (
            "metadata_string_raw_sha256_values",
            metadata.string_raw_sha256_values.map(Value::from),
        ),
        (
            "metadata_private_creator_tags",
            metadata.private_creator_tags.map(Value::from),
        ),
        (
            "metadata_private_creator_ids",
            metadata.private_creator_ids.map(Value::from),
        ),
        (
            "metadata_private_block_ranges",
            metadata.private_block_ranges.map(Value::from),
        ),
        (
            "metadata_private_creator_raw_sha256_values",
            metadata.private_creator_raw_sha256_values.map(Value::from),
        ),
        (
            "metadata_private_element_tags",
            metadata.private_element_tags.map(Value::from),
        ),
        (
            "metadata_private_element_vrs",
            metadata.private_element_vrs.map(Value::from),
        ),
        (
            "metadata_private_element_raw_sha256_values",
            metadata.private_element_raw_sha256_values.map(Value::from),
        ),
        (
            "metadata_sequence_length_variant",
            metadata.sequence_length_variant.map(Value::from),
        ),
        (
            "metadata_sequence_tag",
            metadata.sequence_tag.map(Value::from),
        ),
        (
            "metadata_sequence_value_length",
            metadata.sequence_value_length.map(Value::from),
        ),
        (
            "metadata_sequence_length_field_hex",
            metadata.sequence_length_field_hex.map(Value::from),
        ),
        (
            "metadata_sequence_delimitation_present",
            metadata.sequence_delimitation_present.map(Value::from),
        ),
        (
            "metadata_sequence_item_length_encoding",
            metadata.sequence_item_length_encoding.map(Value::from),
        ),
        (
            "metadata_sequence_item_delimitation_present",
            metadata.sequence_item_delimitation_present.map(Value::from),
        ),
        (
            "metadata_sequence_decoded_code",
            metadata.sequence_decoded_code.map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, value) in [
        ("xa_image_type", xa.image_type.map(Value::from)),
        ("xa_frame_count", xa.frame_count.map(Value::from)),
        (
            "xa_body_part_examined",
            xa.body_part_examined.map(Value::from),
        ),
        (
            "xa_patient_orientation_empty",
            xa.patient_orientation_empty.map(Value::from),
        ),
        (
            "xa_laterality_present",
            xa.laterality_present.map(Value::from),
        ),
        (
            "xa_pixel_intensity_relationship",
            xa.pixel_intensity_relationship.map(Value::from),
        ),
        (
            "xa_radiation_setting",
            xa.radiation_setting.map(Value::from),
        ),
        ("xa_kvp", xa.kvp.map(Value::from)),
        ("xa_exposure_mas", xa.exposure_mas.map(Value::from)),
        (
            "xa_imager_pixel_spacing_mm",
            xa.imager_pixel_spacing_mm.map(Value::from),
        ),
        (
            "xa_positioner_primary_angle_degrees",
            xa.positioner_primary_angle_degrees.map(Value::from),
        ),
        (
            "xa_positioner_secondary_angle_degrees",
            xa.positioner_secondary_angle_degrees.map(Value::from),
        ),
        (
            "xa_distance_source_to_detector_mm",
            xa.distance_source_to_detector_mm.map(Value::from),
        ),
        (
            "xa_distance_source_to_patient_mm",
            xa.distance_source_to_patient_mm.map(Value::from),
        ),
        (
            "xa_estimated_radiographic_magnification_factor",
            xa.estimated_radiographic_magnification_factor
                .map(Value::from),
        ),
        (
            "xa_lossy_image_compression",
            xa.lossy_image_compression.map(Value::from),
        ),
        ("xa_multiframe_cine", xa.multiframe_cine.map(Value::from)),
        (
            "xa_biplane_data_present",
            xa.biplane_data_present.map(Value::from),
        ),
        ("xa_contrast_used", xa.contrast_used.map(Value::from)),
        (
            "xa_subtraction_applied",
            xa.subtraction_applied.map(Value::from),
        ),
        (
            "xa_table_motion_present",
            xa.table_motion_present.map(Value::from),
        ),
        (
            "xa_patient_space_geometry_present",
            xa.patient_space_geometry_present.map(Value::from),
        ),
        (
            "xa_pixel_spacing_calibrated",
            xa.pixel_spacing_calibrated.map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, value) in [
        ("xrf_image_type", xrf.image_type.map(Value::from)),
        ("xrf_frame_count", xrf.frame_count.map(Value::from)),
        (
            "xrf_body_part_examined",
            xrf.body_part_examined.map(Value::from),
        ),
        (
            "xrf_patient_orientation_empty",
            xrf.patient_orientation_empty.map(Value::from),
        ),
        (
            "xrf_laterality_present",
            xrf.laterality_present.map(Value::from),
        ),
        (
            "xrf_pixel_intensity_relationship",
            xrf.pixel_intensity_relationship.map(Value::from),
        ),
        (
            "xrf_radiation_setting",
            xrf.radiation_setting.map(Value::from),
        ),
        ("xrf_kvp", xrf.kvp.map(Value::from)),
        ("xrf_exposure_mas", xrf.exposure_mas.map(Value::from)),
        (
            "xrf_imager_pixel_spacing_mm",
            xrf.imager_pixel_spacing_mm.map(Value::from),
        ),
        (
            "xrf_distance_source_to_detector_mm",
            xrf.distance_source_to_detector_mm.map(Value::from),
        ),
        (
            "xrf_distance_source_to_patient_mm",
            xrf.distance_source_to_patient_mm.map(Value::from),
        ),
        (
            "xrf_estimated_radiographic_magnification_factor",
            xrf.estimated_radiographic_magnification_factor
                .map(Value::from),
        ),
        (
            "xrf_column_angulation_degrees",
            xrf.column_angulation_degrees.map(Value::from),
        ),
        (
            "xrf_lossy_image_compression",
            xrf.lossy_image_compression.map(Value::from),
        ),
        ("xrf_multiframe_cine", xrf.multiframe_cine.map(Value::from)),
        (
            "xrf_biplane_data_present",
            xrf.biplane_data_present.map(Value::from),
        ),
        ("xrf_contrast_used", xrf.contrast_used.map(Value::from)),
        (
            "xrf_subtraction_applied",
            xrf.subtraction_applied.map(Value::from),
        ),
        (
            "xrf_table_position_present",
            xrf.table_position_present.map(Value::from),
        ),
        (
            "xrf_table_motion_present",
            xrf.table_motion_present.map(Value::from),
        ),
        (
            "xrf_table_tilt_present",
            xrf.table_tilt_present.map(Value::from),
        ),
        (
            "xrf_tomography_present",
            xrf.tomography_present.map(Value::from),
        ),
        (
            "xrf_patient_space_geometry_present",
            xrf.patient_space_geometry_present.map(Value::from),
        ),
        (
            "xrf_pixel_spacing_calibrated",
            xrf.pixel_spacing_calibrated.map(Value::from),
        ),
        (
            "xrf_xa_positioner_angles_present",
            xrf.xa_positioner_angles_present.map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, value) in [
        ("us_image_type", us.image_type.map(Value::from)),
        (
            "us_frame_increment_pointer",
            us.frame_increment_pointer.map(Value::from),
        ),
        ("us_frame_time_ms", us.frame_time_ms.map(Value::from)),
        (
            "us_frame_relative_times_ms",
            us.frame_relative_times_ms.map(Value::from),
        ),
        ("us_frame_count", us.frame_count.map(Value::from)),
        (
            "us_ordered_frame_hashes",
            us.ordered_frame_hashes.map(Value::from),
        ),
        (
            "us_spatially_related_frames",
            us.spatially_related_frames.map(Value::from),
        ),
        (
            "us_color_data_present",
            us.color_data_present.map(Value::from),
        ),
        (
            "us_region_calibrated",
            us.region_calibrated.map(Value::from),
        ),
        (
            "us_lossy_image_compression",
            us.lossy_image_compression.map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, value) in [
        ("nm_frame_increment_pointers", nm.frame_increment_pointers),
        ("nm_energy_window_vector", nm.energy_window_vector),
        ("nm_detector_vector", nm.detector_vector),
        ("nm_energy_window_names", nm.energy_window_names),
        (
            "nm_detector_start_angles_degrees",
            nm.detector_start_angles_degrees,
        ),
        ("nm_frame_dimension_tuples", nm.frame_dimension_tuples),
    ] {
        row_object.insert(
            field.to_string(),
            value.map(Value::from).unwrap_or(Value::Null),
        );
    }
    for (field, value) in [
        (
            "enhanced_pet_image_type",
            enhanced_pet.image_type.map(Value::from),
        ),
        (
            "enhanced_pet_frame_type",
            enhanced_pet.frame_type.map(Value::from),
        ),
        (
            "enhanced_pet_view_code",
            enhanced_pet.view_code.map(Value::from),
        ),
        (
            "enhanced_pet_view_modifier_item_count",
            enhanced_pet.view_modifier_item_count.map(Value::from),
        ),
        (
            "enhanced_pet_slice_progression_direction_present",
            enhanced_pet
                .slice_progression_direction_present
                .map(Value::from),
        ),
        (
            "enhanced_pet_stack_ids",
            enhanced_pet.stack_ids.map(Value::from),
        ),
        (
            "enhanced_pet_in_stack_position_numbers",
            enhanced_pet.in_stack_position_numbers.map(Value::from),
        ),
        (
            "enhanced_pet_dimension_index_values",
            enhanced_pet.dimension_index_values.map(Value::from),
        ),
        (
            "enhanced_pet_temporal_position_indices",
            enhanced_pet.temporal_position_indices.map(Value::from),
        ),
        (
            "enhanced_pet_image_positions_patient_mm",
            enhanced_pet.image_positions_patient_mm.map(Value::from),
        ),
        (
            "enhanced_pet_stored_values_by_frame",
            enhanced_pet.stored_values_by_frame.map(Value::from),
        ),
        (
            "enhanced_pet_activity_values_bqml_by_frame",
            enhanced_pet.activity_values_bqml_by_frame.map(Value::from),
        ),
        (
            "enhanced_pet_rwvm_intercept",
            enhanced_pet.rwvm_intercept.map(Value::from),
        ),
        (
            "enhanced_pet_rwvm_slope",
            enhanced_pet.rwvm_slope.map(Value::from),
        ),
        (
            "enhanced_pet_rwvm_measurement_units",
            enhanced_pet.rwvm_measurement_units.map(Value::from),
        ),
        (
            "enhanced_pet_corrections",
            enhanced_pet.corrections.map(Value::from),
        ),
        ("pet_units", pet.units.map(Value::from)),
        ("pet_counts_source", pet.counts_source.map(Value::from)),
        ("pet_series_type", pet.series_type.map(Value::from)),
        ("pet_corrected_image", pet.corrected_image.map(Value::from)),
        (
            "pet_decay_correction",
            pet.decay_correction.map(Value::from),
        ),
        (
            "pet_dose_calibration_factor",
            pet.dose_calibration_factor.map(Value::from),
        ),
        (
            "pet_rescale_intercept",
            pet.rescale_intercept.map(Value::from),
        ),
        ("pet_rescale_slope", pet.rescale_slope.map(Value::from)),
        ("pet_stored_values", pet.stored_values.map(Value::from)),
        (
            "pet_activity_values_bqml",
            pet.activity_values_bqml.map(Value::from),
        ),
        (
            "pet_frame_reference_time_ms",
            pet.frame_reference_time_ms.map(Value::from),
        ),
        (
            "pet_actual_frame_duration_ms",
            pet.actual_frame_duration_ms.map(Value::from),
        ),
        ("pet_image_index", pet.image_index.map(Value::from)),
        (
            "pet_radiopharmaceutical_information_item_count",
            pet.radiopharmaceutical_information_item_count
                .map(Value::from),
        ),
    ] {
        row_object.insert(field.to_string(), value.unwrap_or(Value::Null));
    }
    for (field, backend_field) in [
        ("generation_backend_id", "backend_id"),
        ("generation_backend_version", "version"),
        ("generation_backend_determinism", "determinism"),
    ] {
        row_object.insert(
            field.to_string(),
            generation_backend
                .and_then(|backend| backend.get(backend_field))
                .and_then(Value::as_str)
                .map(Value::from)
                .unwrap_or(Value::Null),
        );
    }
    row_object.insert(
        "sop_class_name".to_string(),
        file.pointer("/dicom/sop_class_name")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "derived_reference_targets".to_string(),
        serde_json::to_value(derived_refs).expect("derived reference targets must serialize"),
    );
    row_object.insert(
        "derived_reference_sop_class_uids".to_string(),
        serde_json::to_value(derived_reference_sop_class_uids)
            .expect("derived reference SOP Class UID values must serialize"),
    );
    row_object.insert(
        "derived_reference_sop_instance_uid_roots".to_string(),
        serde_json::to_value(derived_reference_sop_instance_uid_roots)
            .expect("derived reference SOP Instance UID root values must serialize"),
    );
    row_object.insert(
        "known_stressors".to_string(),
        file.get("known_stressors")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    let pixel_spacing = report_patient_pixel_spacing(file);
    let image_orientation_patient = report_image_orientation_patient(file);
    if let Some(geometry) = row_object
        .get_mut("geometry")
        .and_then(Value::as_object_mut)
    {
        geometry.insert(
            "spacing".to_string(),
            pixel_spacing
                .and_then(report_backslash_number_values)
                .map(|values| {
                    serde_json::to_value(values)
                        .expect("patient pixel spacing values must serialize")
                })
                .unwrap_or(Value::Null),
        );
        geometry.insert(
            "orientation".to_string(),
            image_orientation_patient
                .map(Value::from)
                .unwrap_or(Value::Null),
        );
    }
    row_object.insert(
        "pixel_spacing".to_string(),
        pixel_spacing.map(Value::from).unwrap_or(Value::Null),
    );
    row_object.insert(
        "imager_pixel_spacing".to_string(),
        file.pointer("/recipe/recipe_parameters/imager_pixel_spacing")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "image_orientation_patient".to_string(),
        image_orientation_patient
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "image_position_patient".to_string(),
        report_image_position_patient(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "slice_thickness".to_string(),
        report_slice_thickness(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "spacing_between_slices".to_string(),
        report_spacing_between_slices(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "slice_location".to_string(),
        report_slice_location(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    for (field, pointer) in [
        ("geometry_sort_basis", "/expected_geometry/sort_basis"),
        (
            "geometry_sort_direction",
            "/expected_geometry/sort_direction",
        ),
        (
            "geometry_position_along_normal_mm",
            "/expected_geometry/position_along_normal_mm",
        ),
        (
            "geometry_geometric_order_index",
            "/expected_geometry/geometric_order_index",
        ),
        (
            "geometry_instance_number",
            "/expected_geometry/instance_number",
        ),
        (
            "geometry_instance_number_order_index",
            "/expected_geometry/instance_number_order_index",
        ),
        (
            "geometry_sorting_conflict_expected",
            "/expected_geometry/sorting_conflict_expected",
        ),
        (
            "geometry_instance_number_state",
            "/expected_geometry/instance_number_state",
        ),
        (
            "geometry_adjacent_spacing_mm",
            "/expected_geometry/adjacent_spacing_mm",
        ),
        (
            "geometry_spacing_uniform",
            "/expected_geometry/spacing_uniform",
        ),
        (
            "geometry_gantry_detector_tilt_degrees",
            "/expected_geometry/gantry_detector_tilt_degrees",
        ),
    ] {
        row_object.insert(
            field.to_string(),
            file.pointer(pointer).cloned().unwrap_or(Value::Null),
        );
    }
    for (field, pointer) in [
        (
            "series_organization_group_id",
            "/expected_series_organization/group_id",
        ),
        (
            "study_series_count",
            "/expected_series_organization/study_series_count",
        ),
        (
            "series_ordinal",
            "/expected_series_organization/series_ordinal",
        ),
        (
            "series_organization_instance_count",
            "/expected_series_organization/series_instance_count",
        ),
        (
            "shared_study_instance_uid_expected",
            "/expected_series_organization/shared_study_instance_uid_expected",
        ),
        (
            "shared_frame_of_reference_uid_expected",
            "/expected_series_organization/shared_frame_of_reference_uid_expected",
        ),
        (
            "distinct_series_instance_uids_expected",
            "/expected_series_organization/distinct_series_instance_uids_expected",
        ),
    ] {
        row_object.insert(
            field.to_string(),
            file.pointer(pointer).cloned().unwrap_or(Value::Null),
        );
    }
    row_object.insert(
        "window_center".to_string(),
        report_window_center(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "window_width".to_string(),
        report_window_width(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "kvp".to_string(),
        file.pointer("/recipe/recipe_parameters/kvp")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "ct_acquisition_number".to_string(),
        file.pointer("/recipe/recipe_parameters/acquisition_number")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "ct_rescale_intercept".to_string(),
        file.pointer("/recipe/recipe_parameters/rescale/intercept")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "ct_rescale_slope".to_string(),
        file.pointer("/recipe/recipe_parameters/rescale/slope")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "ct_rescale_type".to_string(),
        file.pointer("/recipe/recipe_parameters/rescale/type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    let is_enhanced_ct = file.pointer("/dicom/sop_class_uid").and_then(Value::as_str)
        == Some(uids::ENHANCED_CT_IMAGE_STORAGE);
    row_object.insert(
        "enhanced_ct_dimension_index_values".to_string(),
        is_enhanced_ct
            .then(|| {
                report_string_or_number_array(file, "/expected_semantics/dimension_index_values")
            })
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "enhanced_ct_in_concatenation_number".to_string(),
        is_enhanced_ct
            .then(|| {
                file.pointer("/expected_semantics/concatenation/in_concatenation_number")
                    .and_then(Value::as_u64)
            })
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "enhanced_ct_in_concatenation_total_number".to_string(),
        is_enhanced_ct
            .then(|| {
                file.pointer("/expected_semantics/concatenation/in_concatenation_total_number")
                    .and_then(Value::as_u64)
            })
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "enhanced_ct_concatenation_frame_offset_number".to_string(),
        is_enhanced_ct
            .then(|| {
                file.pointer("/expected_semantics/concatenation/concatenation_frame_offset_number")
                    .and_then(Value::as_u64)
            })
            .flatten()
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_scanning_sequence".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/scanning_sequence")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_sequence_variant".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/sequence_variant")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_acquisition_type".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/mr_acquisition_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_repetition_time".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/repetition_time")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_echo_time".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/echo_time")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_echo_train_length".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/echo_train_length")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "mr_magnetic_field_strength".to_string(),
        file.pointer("/recipe/recipe_parameters/mr/magnetic_field_strength")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "enhanced_mr_effective_echo_times".to_string(),
        report_string_or_number_array(
            file,
            "/recipe/recipe_parameters/per_frame_functional_groups/effective_echo_time",
        )
        .map(Value::from)
        .unwrap_or(Value::Null),
    );
    let enhanced_mr_temporal = enhanced_mr_temporal_report_fields(manifest_path, file)?;
    row_object.insert(
        "enhanced_mr_temporal_position_time_offsets".to_string(),
        enhanced_mr_temporal
            .time_offsets
            .clone()
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    for (field, value) in [
        (
            "enhanced_mr_temporal_position_indices",
            enhanced_mr_temporal.temporal_position_indices,
        ),
        (
            "enhanced_mr_dimension_index_values",
            enhanced_mr_temporal.dimension_index_values,
        ),
        (
            "enhanced_mr_frame_acquisition_numbers",
            enhanced_mr_temporal.frame_acquisition_numbers,
        ),
        (
            "enhanced_mr_dimension_index_pointer",
            enhanced_mr_temporal.dimension_index_pointer,
        ),
        (
            "enhanced_mr_functional_group_pointer",
            enhanced_mr_temporal.functional_group_pointer,
        ),
        (
            "enhanced_mr_temporal_position_time_offset_unit",
            enhanced_mr_temporal.time_offset_unit,
        ),
    ] {
        row_object.insert(
            field.to_string(),
            value.map(Value::from).unwrap_or(Value::Null),
        );
    }
    row_object.insert(
        "enhanced_mr_velocity_encoding_minimum_value".to_string(),
        file.pointer(
            "/recipe/recipe_parameters/per_frame_functional_groups/velocity_encoding_minimum_value",
        )
        .and_then(report_scalar_label)
        .map(Value::from)
        .unwrap_or(Value::Null),
    );
    row_object.insert(
        "enhanced_mr_velocity_encoding_maximum_value".to_string(),
        file.pointer(
            "/recipe/recipe_parameters/per_frame_functional_groups/velocity_encoding_maximum_value",
        )
        .and_then(report_scalar_label)
        .map(Value::from)
        .unwrap_or(Value::Null),
    );
    row_object.insert(
        "segmentation_type".to_string(),
        file.pointer("/recipe/recipe_parameters/segmentation_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "segmentation_fractional_type".to_string(),
        file.pointer("/recipe/recipe_parameters/segmentation_fractional_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "segmentation_maximum_fractional_value".to_string(),
        file.pointer("/recipe/recipe_parameters/maximum_fractional_value")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    let presentation_state = file.pointer("/expected_semantics/presentation_state");
    row_object.insert(
        "gsps_content_label".to_string(),
        presentation_state
            .and_then(|_| {
                file.pointer("/recipe/recipe_parameters/content_label")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "gsps_content_description".to_string(),
        presentation_state
            .and_then(|_| {
                file.pointer("/recipe/recipe_parameters/content_description")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "gsps_presentation_size_mode".to_string(),
        presentation_state
            .and_then(|_| {
                file.pointer("/expected_semantics/presentation_state/presentation_size_mode")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "gsps_presentation_pixel_aspect_ratio".to_string(),
        presentation_state
            .and_then(|_| {
                report_i64_array(
                    file,
                    "/expected_semantics/presentation_state/presentation_pixel_aspect_ratio",
                )
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "gsps_window_center".to_string(),
        presentation_state
            .and_then(|_| {
                file.pointer("/expected_semantics/presentation_state/window_center")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "gsps_window_width".to_string(),
        presentation_state
            .and_then(|_| {
                file.pointer("/expected_semantics/presentation_state/window_width")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "gsps_presentation_lut_shape".to_string(),
        presentation_state
            .and_then(|_| {
                file.pointer("/expected_semantics/presentation_state/presentation_lut_shape")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    let rwvm = file.pointer("/expected_semantics/real_world_value_mapping");
    row_object.insert(
        "rwvm_content_label".to_string(),
        rwvm.and_then(|_| {
            file.pointer("/recipe/recipe_parameters/content_label")
                .and_then(Value::as_str)
        })
        .map(Value::from)
        .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_lut_label".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/lut_label")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_first_value_mapped".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/first_value_mapped")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_last_value_mapped".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/last_value_mapped")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_intercept".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/intercept")
            .and_then(report_scalar_label)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_slope".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/slope")
            .and_then(report_scalar_label)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_units_code_value".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/units/code_value")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_units_coding_scheme_designator".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/units/coding_scheme_designator")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_units_code_meaning".to_string(),
        file.pointer("/expected_semantics/real_world_value_mapping/units/code_meaning")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rwvm_referenced_frame_numbers".to_string(),
        report_string_or_number_array(
            file,
            "/expected_semantics/real_world_value_mapping/referenced_frame_numbers",
        )
        .map(Value::from)
        .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_dose_units".to_string(),
        file.pointer("/expected_semantics/rt_dose/dose_units")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_dose_type".to_string(),
        file.pointer("/expected_semantics/rt_dose/dose_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_dose_summation_type".to_string(),
        file.pointer("/expected_semantics/rt_dose/dose_summation_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_dose_grid_scaling".to_string(),
        file.pointer("/expected_semantics/rt_dose/dose_grid_scaling")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_structure_set_label".to_string(),
        file.pointer("/expected_semantics/rt_structure_set/structure_set_label")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_structure_set_roi_name".to_string(),
        file.pointer("/expected_semantics/rt_structure_set/roi_name")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_roi_generation_algorithm".to_string(),
        file.pointer("/expected_semantics/rt_structure_set/roi_generation_algorithm")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_contour_geometric_type".to_string(),
        file.pointer("/expected_semantics/rt_structure_set/contour_geometric_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_contour_points".to_string(),
        file.pointer("/expected_semantics/rt_structure_set/contour_points")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "rt_roi_interpreted_type".to_string(),
        file.pointer("/expected_semantics/rt_structure_set/roi_interpreted_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "encapsulated_document_burned_in_annotation".to_string(),
        file.pointer("/expected_semantics/encapsulated_document/burned_in_annotation")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "encapsulated_document_recognizable_visual_features".to_string(),
        file.pointer("/expected_semantics/encapsulated_document/recognizable_visual_features")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "encapsulated_document_title".to_string(),
        file.pointer("/expected_semantics/encapsulated_document/document_title")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "encapsulated_document_mime_type".to_string(),
        file.pointer("/expected_semantics/encapsulated_document/mime_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "encapsulated_document_length".to_string(),
        file.pointer("/expected_semantics/encapsulated_document/document_length")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_completion_flag".to_string(),
        file.pointer("/expected_semantics/structured_report/completion_flag")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_verification_flag".to_string(),
        file.pointer("/expected_semantics/structured_report/verification_flag")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_root_value_type".to_string(),
        file.pointer("/expected_semantics/structured_report/root_value_type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_root_continuity_of_content".to_string(),
        file.pointer("/expected_semantics/structured_report/root_continuity_of_content")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_content_sequence_items".to_string(),
        file.pointer("/expected_semantics/structured_report/content_sequence_items")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_observation_text".to_string(),
        file.pointer("/expected_semantics/structured_report/observation_text")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sr_measurement_numeric_value".to_string(),
        file.pointer("/expected_semantics/structured_report/measurement/numeric_value")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    let key_objects = file.pointer("/expected_semantics/structured_report/key_objects");
    row_object.insert(
        "kos_document_title".to_string(),
        key_objects
            .and_then(|_| {
                file.pointer("/recipe/recipe_parameters/document_title/code_meaning")
                    .and_then(Value::as_str)
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "kos_key_object_count".to_string(),
        key_objects
            .and_then(Value::as_array)
            .map(|items| Value::from(items.len() as u64))
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "kos_key_object_relationship_types".to_string(),
        key_objects
            .and_then(|_| {
                report_object_array_string_values(
                    file,
                    "/expected_semantics/structured_report/key_objects",
                    "relationship_type",
                )
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "kos_key_object_value_types".to_string(),
        key_objects
            .and_then(|_| {
                report_object_array_string_values(
                    file,
                    "/expected_semantics/structured_report/key_objects",
                    "value_type",
                )
            })
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "kos_referenced_frame_numbers".to_string(),
        key_objects
            .and_then(|_| report_key_object_referenced_frame_numbers(file))
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "display_shutter_shape".to_string(),
        report_display_shutter_shape(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "display_shutter_presentation_value".to_string(),
        report_display_shutter_presentation_value(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "body_part_examined".to_string(),
        report_body_part_examined(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "view_position".to_string(),
        report_view_position(file)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "modality_lut_descriptor".to_string(),
        report_lut_descriptor(file, "/recipe/recipe_parameters/modality_lut/descriptor")
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "modality_lut_type".to_string(),
        file.pointer("/recipe/recipe_parameters/modality_lut/type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "modality_lut_data_value_length".to_string(),
        file.pointer("/recipe/recipe_parameters/modality_lut/data_value_length")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "voi_lut_descriptor".to_string(),
        report_lut_descriptor(file, "/recipe/recipe_parameters/voi_lut/descriptor")
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "voi_lut_data_value_length".to_string(),
        file.pointer("/recipe/recipe_parameters/voi_lut/data_value_length")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_rows".to_string(),
        file.pointer("/recipe/recipe_parameters/overlay/rows")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_columns".to_string(),
        file.pointer("/recipe/recipe_parameters/overlay/columns")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_type".to_string(),
        file.pointer("/recipe/recipe_parameters/overlay/type")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_origin".to_string(),
        report_i64_array(file, "/recipe/recipe_parameters/overlay/origin")
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_bits_allocated".to_string(),
        file.pointer("/recipe/recipe_parameters/overlay/bits_allocated")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_bit_position".to_string(),
        file.pointer("/recipe/recipe_parameters/overlay/bit_position")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "overlay_data_value_length".to_string(),
        file.pointer("/recipe/recipe_parameters/overlay/value_length")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "study_instance_uid_root".to_string(),
        file.pointer("/uids/study_instance_uid")
            .and_then(Value::as_str)
            .and_then(uid_root_bucket)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "series_instance_uid_root".to_string(),
        file.pointer("/uids/series_instance_uid")
            .and_then(Value::as_str)
            .and_then(uid_root_bucket)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert(
        "sop_instance_uid_root".to_string(),
        file.pointer("/uids/sop_instance_uid")
            .and_then(Value::as_str)
            .and_then(uid_root_bucket)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    Ok(row)
}

#[derive(Default)]
struct NmMultiframeReportFields {
    frame_increment_pointers: Option<String>,
    energy_window_vector: Option<String>,
    detector_vector: Option<String>,
    energy_window_names: Option<String>,
    detector_start_angles_degrees: Option<String>,
    frame_dimension_tuples: Option<String>,
}

#[derive(Default)]
struct PetActivityReportFields {
    units: Option<String>,
    counts_source: Option<String>,
    series_type: Option<String>,
    corrected_image: Option<String>,
    decay_correction: Option<String>,
    dose_calibration_factor: Option<f64>,
    rescale_intercept: Option<f64>,
    rescale_slope: Option<f64>,
    stored_values: Option<String>,
    activity_values_bqml: Option<String>,
    frame_reference_time_ms: Option<f64>,
    actual_frame_duration_ms: Option<u64>,
    image_index: Option<u64>,
    radiopharmaceutical_information_item_count: Option<u64>,
}

#[derive(Debug, Default, PartialEq)]
struct EnhancedPetReportFields {
    image_type: Option<String>,
    frame_type: Option<String>,
    view_code: Option<String>,
    view_modifier_item_count: Option<u64>,
    slice_progression_direction_present: Option<bool>,
    stack_ids: Option<String>,
    in_stack_position_numbers: Option<String>,
    dimension_index_values: Option<String>,
    temporal_position_indices: Option<String>,
    image_positions_patient_mm: Option<String>,
    stored_values_by_frame: Option<String>,
    activity_values_bqml_by_frame: Option<String>,
    rwvm_intercept: Option<f64>,
    rwvm_slope: Option<f64>,
    rwvm_measurement_units: Option<String>,
    corrections: Option<String>,
}

#[derive(Debug, Default, PartialEq)]
struct XaProjectionReportFields {
    image_type: Option<String>,
    frame_count: Option<u64>,
    body_part_examined: Option<String>,
    patient_orientation_empty: Option<bool>,
    laterality_present: Option<bool>,
    pixel_intensity_relationship: Option<String>,
    radiation_setting: Option<String>,
    kvp: Option<f64>,
    exposure_mas: Option<u64>,
    imager_pixel_spacing_mm: Option<String>,
    positioner_primary_angle_degrees: Option<f64>,
    positioner_secondary_angle_degrees: Option<f64>,
    distance_source_to_detector_mm: Option<f64>,
    distance_source_to_patient_mm: Option<f64>,
    estimated_radiographic_magnification_factor: Option<f64>,
    lossy_image_compression: Option<String>,
    multiframe_cine: Option<bool>,
    biplane_data_present: Option<bool>,
    contrast_used: Option<bool>,
    subtraction_applied: Option<bool>,
    table_motion_present: Option<bool>,
    patient_space_geometry_present: Option<bool>,
    pixel_spacing_calibrated: Option<bool>,
}

#[derive(Debug, Default, PartialEq)]
struct XrfProjectionReportFields {
    image_type: Option<String>,
    frame_count: Option<u64>,
    body_part_examined: Option<String>,
    patient_orientation_empty: Option<bool>,
    laterality_present: Option<bool>,
    pixel_intensity_relationship: Option<String>,
    radiation_setting: Option<String>,
    kvp: Option<f64>,
    exposure_mas: Option<u64>,
    imager_pixel_spacing_mm: Option<String>,
    distance_source_to_detector_mm: Option<f64>,
    distance_source_to_patient_mm: Option<f64>,
    estimated_radiographic_magnification_factor: Option<f64>,
    column_angulation_degrees: Option<f64>,
    lossy_image_compression: Option<String>,
    multiframe_cine: Option<bool>,
    biplane_data_present: Option<bool>,
    contrast_used: Option<bool>,
    subtraction_applied: Option<bool>,
    table_position_present: Option<bool>,
    table_motion_present: Option<bool>,
    table_tilt_present: Option<bool>,
    tomography_present: Option<bool>,
    patient_space_geometry_present: Option<bool>,
    pixel_spacing_calibrated: Option<bool>,
    xa_positioner_angles_present: Option<bool>,
}

#[derive(Debug, Default)]
struct UsMultiframeReportFields {
    image_type: Option<String>,
    frame_increment_pointer: Option<String>,
    frame_time_ms: Option<f64>,
    frame_relative_times_ms: Option<String>,
    frame_count: Option<u64>,
    ordered_frame_hashes: Option<String>,
    spatially_related_frames: Option<bool>,
    color_data_present: Option<bool>,
    region_calibrated: Option<bool>,
    lossy_image_compression: Option<String>,
}

fn us_multiframe_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<UsMultiframeReportFields, ReportError> {
    let Some(expected) = file.get("expected_us_multiframe") else {
        return Ok(UsMultiframeReportFields::default());
    };
    let expected = expected.as_object().ok_or(ReportError::MetadataShape {
        path: manifest_path.to_path_buf(),
        message: "expected_us_multiframe must be an object",
    })?;
    let string_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| values.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .map(|values| values.join("; "))
    };
    let scalar_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| {
                values
                    .iter()
                    .map(report_scalar_label)
                    .collect::<Option<Vec<_>>>()
            })
            .map(|values| values.join("; "))
    };
    let ordered_frame_hashes = expected
        .get("frames")
        .and_then(Value::as_array)
        .and_then(|frames| {
            frames
                .iter()
                .map(|frame| frame.get("frame_sha256").and_then(Value::as_str))
                .collect::<Option<Vec<_>>>()
        })
        .map(|hashes| hashes.join("; "));

    let fields = UsMultiframeReportFields {
        image_type: string_array("image_type"),
        frame_increment_pointer: expected
            .get("frame_increment_pointer")
            .and_then(Value::as_str)
            .map(str::to_string),
        frame_time_ms: expected.get("frame_time_ms").and_then(Value::as_f64),
        frame_relative_times_ms: scalar_array("frame_relative_times_ms"),
        frame_count: expected.get("frame_count").and_then(Value::as_u64),
        ordered_frame_hashes,
        spatially_related_frames: expected
            .get("spatially_related_frames")
            .and_then(Value::as_bool),
        color_data_present: expected.get("color_data_present").and_then(Value::as_bool),
        region_calibrated: expected.get("region_calibrated").and_then(Value::as_bool),
        lossy_image_compression: expected
            .get("lossy_image_compression")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    if fields.image_type.is_none()
        || fields.frame_increment_pointer.is_none()
        || fields.frame_time_ms.is_none()
        || fields.frame_relative_times_ms.is_none()
        || fields.frame_count.is_none()
        || fields.ordered_frame_hashes.is_none()
        || fields.spatially_related_frames.is_none()
        || fields.color_data_present.is_none()
        || fields.region_calibrated.is_none()
        || fields.lossy_image_compression.is_none()
        || expected
            .get("frame_relative_times_ms")
            .and_then(Value::as_array)
            .zip(fields.frame_count)
            .is_none_or(|(times, count)| times.len() as u64 != count)
        || expected
            .get("frames")
            .and_then(Value::as_array)
            .zip(fields.frame_count)
            .is_none_or(|(frames, count)| frames.len() as u64 != count)
    {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "expected_us_multiframe must define the complete report contract",
        });
    }
    Ok(fields)
}

fn xa_projection_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<XaProjectionReportFields, ReportError> {
    let Some(expected) = file.get("expected_xa_projection") else {
        if file.pointer("/dicom/modality").and_then(Value::as_str) == Some("XA") {
            return Err(ReportError::MetadataShape {
                path: manifest_path.to_path_buf(),
                message: "XA file must define expected_xa_projection",
            });
        }
        return Ok(XaProjectionReportFields::default());
    };
    let expected = expected.as_object().ok_or(ReportError::MetadataShape {
        path: manifest_path.to_path_buf(),
        message: "expected_xa_projection must be an object",
    })?;
    let string_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| values.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .map(|values| values.join("; "))
    };

    let fields = XaProjectionReportFields {
        image_type: string_array("image_type"),
        frame_count: expected.get("frame_count").and_then(Value::as_u64),
        body_part_examined: expected
            .get("body_part_examined")
            .and_then(Value::as_str)
            .map(str::to_string),
        patient_orientation_empty: expected
            .get("patient_orientation_empty")
            .and_then(Value::as_bool),
        laterality_present: expected.get("laterality_present").and_then(Value::as_bool),
        pixel_intensity_relationship: expected
            .get("pixel_intensity_relationship")
            .and_then(Value::as_str)
            .map(str::to_string),
        radiation_setting: expected
            .get("radiation_setting")
            .and_then(Value::as_str)
            .map(str::to_string),
        kvp: expected.get("kvp").and_then(Value::as_f64),
        exposure_mas: expected.get("exposure_mas").and_then(Value::as_u64),
        imager_pixel_spacing_mm: expected
            .get("imager_pixel_spacing_mm")
            .and_then(report_value_array_label),
        positioner_primary_angle_degrees: expected
            .get("positioner_primary_angle_degrees")
            .and_then(Value::as_f64),
        positioner_secondary_angle_degrees: expected
            .get("positioner_secondary_angle_degrees")
            .and_then(Value::as_f64),
        distance_source_to_detector_mm: expected
            .get("distance_source_to_detector_mm")
            .and_then(Value::as_f64),
        distance_source_to_patient_mm: expected
            .get("distance_source_to_patient_mm")
            .and_then(Value::as_f64),
        estimated_radiographic_magnification_factor: expected
            .get("estimated_radiographic_magnification_factor")
            .and_then(Value::as_f64),
        lossy_image_compression: expected
            .get("lossy_image_compression")
            .and_then(Value::as_str)
            .map(str::to_string),
        multiframe_cine: expected.get("multiframe_cine").and_then(Value::as_bool),
        biplane_data_present: expected
            .get("biplane_data_present")
            .and_then(Value::as_bool),
        contrast_used: expected.get("contrast_used").and_then(Value::as_bool),
        subtraction_applied: expected.get("subtraction_applied").and_then(Value::as_bool),
        table_motion_present: expected
            .get("table_motion_present")
            .and_then(Value::as_bool),
        patient_space_geometry_present: expected
            .get("patient_space_geometry_present")
            .and_then(Value::as_bool),
        pixel_spacing_calibrated: expected
            .get("pixel_spacing_calibrated")
            .and_then(Value::as_bool),
    };
    if fields.image_type.is_none()
        || fields.frame_count.is_none()
        || fields.body_part_examined.is_none()
        || fields.patient_orientation_empty.is_none()
        || fields.laterality_present.is_none()
        || fields.pixel_intensity_relationship.is_none()
        || fields.radiation_setting.is_none()
        || fields.kvp.is_none()
        || fields.exposure_mas.is_none()
        || fields.imager_pixel_spacing_mm.is_none()
        || fields.positioner_primary_angle_degrees.is_none()
        || fields.positioner_secondary_angle_degrees.is_none()
        || fields.distance_source_to_detector_mm.is_none()
        || fields.distance_source_to_patient_mm.is_none()
        || fields.estimated_radiographic_magnification_factor.is_none()
        || fields.lossy_image_compression.is_none()
        || fields.multiframe_cine.is_none()
        || fields.biplane_data_present.is_none()
        || fields.contrast_used.is_none()
        || fields.subtraction_applied.is_none()
        || fields.table_motion_present.is_none()
        || fields.patient_space_geometry_present.is_none()
        || fields.pixel_spacing_calibrated.is_none()
    {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "expected_xa_projection must define the complete report contract",
        });
    }
    Ok(fields)
}

fn xrf_projection_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<XrfProjectionReportFields, ReportError> {
    let Some(expected) = file.get("expected_xrf_projection") else {
        if file.pointer("/dicom/modality").and_then(Value::as_str) == Some("RF") {
            return Err(ReportError::MetadataShape {
                path: manifest_path.to_path_buf(),
                message: "RF file must define expected_xrf_projection",
            });
        }
        return Ok(XrfProjectionReportFields::default());
    };
    let expected = expected.as_object().ok_or(ReportError::MetadataShape {
        path: manifest_path.to_path_buf(),
        message: "expected_xrf_projection must be an object",
    })?;
    let string_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| values.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .map(|values| values.join("; "))
    };

    let fields = XrfProjectionReportFields {
        image_type: string_array("image_type"),
        frame_count: expected.get("frame_count").and_then(Value::as_u64),
        body_part_examined: expected
            .get("body_part_examined")
            .and_then(Value::as_str)
            .map(str::to_string),
        patient_orientation_empty: expected
            .get("patient_orientation_empty")
            .and_then(Value::as_bool),
        laterality_present: expected.get("laterality_present").and_then(Value::as_bool),
        pixel_intensity_relationship: expected
            .get("pixel_intensity_relationship")
            .and_then(Value::as_str)
            .map(str::to_string),
        radiation_setting: expected
            .get("radiation_setting")
            .and_then(Value::as_str)
            .map(str::to_string),
        kvp: expected.get("kvp").and_then(Value::as_f64),
        exposure_mas: expected.get("exposure_mas").and_then(Value::as_u64),
        imager_pixel_spacing_mm: expected
            .get("imager_pixel_spacing_mm")
            .and_then(report_value_array_label),
        distance_source_to_detector_mm: expected
            .get("distance_source_to_detector_mm")
            .and_then(Value::as_f64),
        distance_source_to_patient_mm: expected
            .get("distance_source_to_patient_mm")
            .and_then(Value::as_f64),
        estimated_radiographic_magnification_factor: expected
            .get("estimated_radiographic_magnification_factor")
            .and_then(Value::as_f64),
        column_angulation_degrees: expected
            .get("column_angulation_degrees")
            .and_then(Value::as_f64),
        lossy_image_compression: expected
            .get("lossy_image_compression")
            .and_then(Value::as_str)
            .map(str::to_string),
        multiframe_cine: expected.get("multiframe_cine").and_then(Value::as_bool),
        biplane_data_present: expected
            .get("biplane_data_present")
            .and_then(Value::as_bool),
        contrast_used: expected.get("contrast_used").and_then(Value::as_bool),
        subtraction_applied: expected.get("subtraction_applied").and_then(Value::as_bool),
        table_position_present: expected
            .get("table_position_present")
            .and_then(Value::as_bool),
        table_motion_present: expected
            .get("table_motion_present")
            .and_then(Value::as_bool),
        table_tilt_present: expected.get("table_tilt_present").and_then(Value::as_bool),
        tomography_present: expected.get("tomography_present").and_then(Value::as_bool),
        patient_space_geometry_present: expected
            .get("patient_space_geometry_present")
            .and_then(Value::as_bool),
        pixel_spacing_calibrated: expected
            .get("pixel_spacing_calibrated")
            .and_then(Value::as_bool),
        xa_positioner_angles_present: expected
            .get("xa_positioner_angles_present")
            .and_then(Value::as_bool),
    };
    if fields.image_type.is_none()
        || fields.frame_count.is_none()
        || fields.body_part_examined.is_none()
        || fields.patient_orientation_empty.is_none()
        || fields.laterality_present.is_none()
        || fields.pixel_intensity_relationship.is_none()
        || fields.radiation_setting.is_none()
        || fields.kvp.is_none()
        || fields.exposure_mas.is_none()
        || fields.imager_pixel_spacing_mm.is_none()
        || fields.distance_source_to_detector_mm.is_none()
        || fields.distance_source_to_patient_mm.is_none()
        || fields.estimated_radiographic_magnification_factor.is_none()
        || fields.column_angulation_degrees.is_none()
        || fields.lossy_image_compression.is_none()
        || fields.multiframe_cine.is_none()
        || fields.biplane_data_present.is_none()
        || fields.contrast_used.is_none()
        || fields.subtraction_applied.is_none()
        || fields.table_position_present.is_none()
        || fields.table_motion_present.is_none()
        || fields.table_tilt_present.is_none()
        || fields.tomography_present.is_none()
        || fields.patient_space_geometry_present.is_none()
        || fields.pixel_spacing_calibrated.is_none()
        || fields.xa_positioner_angles_present.is_none()
    {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "expected_xrf_projection must define the complete report contract",
        });
    }
    Ok(fields)
}

fn pet_activity_report_fields(file: &Value) -> PetActivityReportFields {
    let Some(expected) = file.get("expected_pet_activity") else {
        return PetActivityReportFields::default();
    };
    let joined = |field: &str| expected.get(field).and_then(report_value_array_label);

    PetActivityReportFields {
        units: expected
            .get("units")
            .and_then(Value::as_str)
            .map(str::to_string),
        counts_source: expected
            .get("counts_source")
            .and_then(Value::as_str)
            .map(str::to_string),
        series_type: joined("series_type"),
        corrected_image: joined("corrected_image"),
        decay_correction: expected
            .get("decay_correction")
            .and_then(Value::as_str)
            .map(str::to_string),
        dose_calibration_factor: expected
            .get("dose_calibration_factor")
            .and_then(Value::as_f64),
        rescale_intercept: expected.get("rescale_intercept").and_then(Value::as_f64),
        rescale_slope: expected.get("rescale_slope").and_then(Value::as_f64),
        stored_values: joined("stored_values"),
        activity_values_bqml: joined("activity_values_bqml"),
        frame_reference_time_ms: expected
            .get("frame_reference_time_ms")
            .and_then(Value::as_f64),
        actual_frame_duration_ms: expected
            .get("actual_frame_duration_ms")
            .and_then(Value::as_u64),
        image_index: expected.get("image_index").and_then(Value::as_u64),
        radiopharmaceutical_information_item_count: expected
            .get("radiopharmaceutical_information_item_count")
            .and_then(Value::as_u64),
    }
}

fn enhanced_pet_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<EnhancedPetReportFields, ReportError> {
    let Some(expected) = file.get("expected_enhanced_pet") else {
        return Ok(EnhancedPetReportFields::default());
    };
    let expected = expected.as_object().ok_or(ReportError::MetadataShape {
        path: manifest_path.to_path_buf(),
        message: "expected_enhanced_pet must be an object",
    })?;
    let string_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| values.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .map(|values| values.join("; "))
    };
    let scalar_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| {
                values
                    .iter()
                    .map(report_scalar_label)
                    .collect::<Option<Vec<_>>>()
            })
            .map(|values| values.join("; "))
    };
    let nested_scalar_array = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|frames| {
                frames
                    .iter()
                    .map(report_value_array_label)
                    .collect::<Option<Vec<_>>>()
            })
            .map(|frames| frames.join(" | "))
    };
    let code_label = |value: Option<&Value>| {
        let code = value?.as_object()?;
        Some(format!(
            "{}|{}|{}",
            code.get("code_value")?.as_str()?,
            code.get("coding_scheme_designator")?.as_str()?,
            code.get("code_meaning")?.as_str()?
        ))
    };
    let corrections = expected
        .get("corrections")
        .and_then(Value::as_object)
        .and_then(|values| {
            [
                "decay",
                "attenuation",
                "scatter",
                "dead_time",
                "gantry_motion",
                "patient_motion",
                "count_loss_normalization",
                "randoms",
                "non_uniform_radial_sampling",
                "sensitivity_calibration",
                "detector_normalization",
            ]
            .iter()
            .map(|name| {
                values
                    .get(*name)
                    .and_then(Value::as_str)
                    .map(|value| format!("{name}={value}"))
            })
            .collect::<Option<Vec<_>>>()
        })
        .map(|values| values.join("; "));
    let rwvm = expected
        .get("real_world_value_mapping")
        .and_then(Value::as_object);

    let fields = EnhancedPetReportFields {
        image_type: string_array("image_type"),
        frame_type: string_array("frame_type"),
        view_code: code_label(expected.get("view_code")),
        view_modifier_item_count: expected
            .get("view_modifier_item_count")
            .and_then(Value::as_u64),
        slice_progression_direction_present: expected
            .get("slice_progression_direction_present")
            .and_then(Value::as_bool),
        stack_ids: string_array("stack_ids"),
        in_stack_position_numbers: scalar_array("in_stack_position_numbers"),
        dimension_index_values: scalar_array("dimension_index_values"),
        temporal_position_indices: scalar_array("temporal_position_indices"),
        image_positions_patient_mm: nested_scalar_array("image_positions_patient_mm"),
        stored_values_by_frame: nested_scalar_array("stored_values_by_frame"),
        activity_values_bqml_by_frame: nested_scalar_array("activity_values_bqml_by_frame"),
        rwvm_intercept: rwvm
            .and_then(|mapping| mapping.get("intercept"))
            .and_then(Value::as_f64),
        rwvm_slope: rwvm
            .and_then(|mapping| mapping.get("slope"))
            .and_then(Value::as_f64),
        rwvm_measurement_units: code_label(
            rwvm.and_then(|mapping| mapping.get("measurement_units")),
        ),
        corrections,
    };
    if fields.image_type.is_none()
        || fields.frame_type.is_none()
        || fields.view_code.is_none()
        || fields.view_modifier_item_count.is_none()
        || fields.slice_progression_direction_present.is_none()
        || fields.stack_ids.is_none()
        || fields.in_stack_position_numbers.is_none()
        || fields.dimension_index_values.is_none()
        || fields.temporal_position_indices.is_none()
        || fields.image_positions_patient_mm.is_none()
        || fields.stored_values_by_frame.is_none()
        || fields.activity_values_bqml_by_frame.is_none()
        || fields.rwvm_intercept.is_none()
        || fields.rwvm_slope.is_none()
        || fields.rwvm_measurement_units.is_none()
        || fields.corrections.is_none()
    {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "expected_enhanced_pet must define the complete report contract",
        });
    }
    Ok(fields)
}

fn nm_multiframe_report_fields(file: &Value) -> NmMultiframeReportFields {
    let Some(expected) = file
        .get("expected_nm_multiframe")
        .and_then(Value::as_object)
    else {
        return NmMultiframeReportFields::default();
    };

    let joined_scalars = |field: &str| {
        expected
            .get(field)
            .and_then(Value::as_array)
            .and_then(|values| {
                values
                    .iter()
                    .map(report_scalar_label)
                    .collect::<Option<Vec<_>>>()
            })
            .map(|values| values.join("; "))
    };
    let energy_window_names = expected
        .get("energy_windows")
        .and_then(Value::as_array)
        .and_then(|windows| {
            windows
                .iter()
                .map(|window| window.get("name").and_then(Value::as_str))
                .collect::<Option<Vec<_>>>()
        })
        .map(|names| names.join("; "));
    let detector_start_angles_degrees = expected
        .get("detectors")
        .and_then(Value::as_array)
        .and_then(|detectors| {
            detectors
                .iter()
                .map(|detector| {
                    detector
                        .get("start_angle_degrees")
                        .and_then(report_scalar_label)
                })
                .collect::<Option<Vec<_>>>()
        })
        .map(|angles| angles.join("; "));
    let frame_dimension_tuples = expected
        .get("frame_dimensions")
        .and_then(Value::as_array)
        .and_then(|frames| {
            frames
                .iter()
                .map(|frame| {
                    Some(format!(
                        "{}:{}:{}",
                        frame.get("frame_number")?.as_u64()?,
                        frame.get("energy_window_index")?.as_u64()?,
                        frame.get("detector_index")?.as_u64()?,
                    ))
                })
                .collect::<Option<Vec<_>>>()
        })
        .map(|tuples| tuples.join("; "));

    NmMultiframeReportFields {
        frame_increment_pointers: joined_scalars("frame_increment_pointers"),
        energy_window_vector: joined_scalars("energy_window_vector"),
        detector_vector: joined_scalars("detector_vector"),
        energy_window_names,
        detector_start_angles_degrees,
        frame_dimension_tuples,
    }
}

#[derive(Default)]
struct MetadataReportFields {
    specific_character_sets: Option<String>,
    person_name: Option<String>,
    person_name_component_groups: Option<String>,
    person_name_component_group_count: Option<u64>,
    person_name_encoded_sha256: Option<String>,
    person_name_encoded_length_bytes: Option<u64>,
    temporal_boundary_id: Option<String>,
    timezone_offset_from_utc: Option<String>,
    date_values: Option<String>,
    time_values: Option<String>,
    date_time_values: Option<String>,
    temporal_normalized_utc: Option<String>,
    empty_type2_attributes: Option<String>,
    empty_type2_attribute_count: Option<u64>,
    string_tags: Option<Vec<String>>,
    string_vrs: Option<Vec<String>>,
    string_value_multiplicities: Option<Vec<u64>>,
    string_max_component_encoded_length_bytes: Option<Vec<u64>>,
    string_raw_value_lengths: Option<Vec<u64>>,
    string_raw_sha256_values: Option<Vec<String>>,
    private_creator_tags: Option<Vec<String>>,
    private_creator_ids: Option<Vec<String>>,
    private_block_ranges: Option<Vec<String>>,
    private_creator_raw_sha256_values: Option<Vec<String>>,
    private_element_tags: Option<Vec<String>>,
    private_element_vrs: Option<Vec<String>>,
    private_element_raw_sha256_values: Option<Vec<String>>,
    sequence_length_variant: Option<String>,
    sequence_tag: Option<String>,
    sequence_value_length: Option<u64>,
    sequence_length_field_hex: Option<String>,
    sequence_delimitation_present: Option<bool>,
    sequence_item_length_encoding: Option<String>,
    sequence_item_delimitation_present: Option<bool>,
    sequence_decoded_code: Option<String>,
}

fn metadata_report_fields(file: &Value) -> MetadataReportFields {
    let specific_character_sets = file
        .pointer("/expected_metadata/specific_character_sets")
        .and_then(Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .map(|value| value.as_str())
                .collect::<Option<Vec<_>>>()
        })
        .map(|values| values.join("\\"));
    let person_name = file.pointer("/expected_metadata/person_names/0");
    let component_groups = person_name
        .and_then(|person_name| person_name.get("component_groups"))
        .and_then(Value::as_array);
    let person_name_component_groups = component_groups.and_then(|groups| {
        groups
            .iter()
            .map(|group| {
                Some(format!(
                    "{}:{}",
                    group.get("kind")?.as_str()?,
                    group.get("decoded_value")?.as_str()?
                ))
            })
            .collect::<Option<Vec<_>>>()
            .map(|groups| groups.join(" | "))
    });
    let temporal = file.pointer("/expected_metadata/temporal");
    let empty_type2_attributes = file
        .pointer("/expected_metadata/empty_type2_attributes")
        .and_then(Value::as_array);
    let mut string_elements = file
        .pointer("/expected_metadata/string_elements")
        .and_then(Value::as_array)
        .map(|elements| elements.iter().collect::<Vec<_>>());
    if let Some(elements) = string_elements.as_mut() {
        elements.sort_by_key(|element| element.get("tag").and_then(Value::as_str));
    }
    let mut private_blocks = file
        .pointer("/expected_metadata/private_creator_blocks")
        .and_then(Value::as_array)
        .map(|blocks| blocks.iter().collect::<Vec<_>>());
    if let Some(blocks) = private_blocks.as_mut() {
        blocks.sort_by_key(|block| block.get("creator_tag").and_then(Value::as_str));
    }
    let private_elements = private_blocks.as_ref().map(|blocks| {
        let mut elements = blocks
            .iter()
            .flat_map(|block| {
                block
                    .get("elements")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .collect::<Vec<_>>();
        elements.sort_by_key(|element| element.get("tag").and_then(Value::as_str));
        elements
    });
    let sequence = file.pointer("/expected_metadata/sequence_length_encoding");

    MetadataReportFields {
        specific_character_sets,
        person_name: person_name
            .and_then(|value| value.get("decoded_value"))
            .and_then(Value::as_str)
            .map(str::to_string),
        person_name_component_groups,
        person_name_component_group_count: component_groups.map(|groups| groups.len() as u64),
        person_name_encoded_sha256: person_name
            .and_then(|value| value.get("raw_value_sha256"))
            .and_then(Value::as_str)
            .map(str::to_string),
        person_name_encoded_length_bytes: person_name
            .and_then(|value| value.get("raw_value_byte_length"))
            .and_then(Value::as_u64),
        temporal_boundary_id: temporal
            .and_then(|value| value.get("boundary_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        timezone_offset_from_utc: temporal
            .and_then(|value| value.pointer("/timezone_offset_from_utc/decoded_value"))
            .and_then(Value::as_str)
            .map(str::to_string),
        date_values: temporal
            .and_then(|value| value.get("date_values"))
            .and_then(metadata_decoded_values),
        time_values: temporal
            .and_then(|value| value.get("time_values"))
            .and_then(metadata_decoded_values),
        date_time_values: temporal
            .and_then(|value| value.get("date_time_values"))
            .and_then(metadata_decoded_values),
        temporal_normalized_utc: temporal
            .and_then(|value| value.get("combined_da_tm_utc"))
            .and_then(Value::as_str)
            .map(str::to_string),
        empty_type2_attributes: empty_type2_attributes.and_then(|attributes| {
            attributes
                .iter()
                .map(|attribute| {
                    Some(format!(
                        "{} {} {} VL={}",
                        attribute.get("tag")?.as_str()?,
                        attribute.get("keyword")?.as_str()?,
                        attribute.get("vr")?.as_str()?,
                        attribute.get("value_length")?.as_u64()?
                    ))
                })
                .collect::<Option<Vec<_>>>()
                .map(|attributes| attributes.join("; "))
        }),
        empty_type2_attribute_count: empty_type2_attributes
            .map(|attributes| attributes.len() as u64),
        string_tags: metadata_string_field(&string_elements, "tag"),
        string_vrs: metadata_string_field(&string_elements, "vr"),
        string_value_multiplicities: metadata_u64_field(&string_elements, "value_multiplicity"),
        string_max_component_encoded_length_bytes: string_elements.as_ref().and_then(|elements| {
            elements
                .iter()
                .map(|element| {
                    element
                        .get("decoded_value_lengths")?
                        .as_array()?
                        .iter()
                        .filter_map(Value::as_u64)
                        .max()
                })
                .collect::<Option<Vec<_>>>()
        }),
        string_raw_value_lengths: metadata_u64_field(&string_elements, "raw_value_byte_length"),
        string_raw_sha256_values: metadata_string_field(&string_elements, "raw_value_sha256"),
        private_creator_tags: metadata_string_field(&private_blocks, "creator_tag"),
        private_creator_ids: metadata_string_field(&private_blocks, "creator_id"),
        private_block_ranges: private_blocks.as_ref().and_then(|blocks| {
            blocks
                .iter()
                .map(|block| {
                    Some(format!(
                        "{}-{}",
                        block.get("block_start_tag")?.as_str()?,
                        block.get("block_end_tag")?.as_str()?
                    ))
                })
                .collect()
        }),
        private_creator_raw_sha256_values: metadata_string_field(
            &private_blocks,
            "raw_value_sha256",
        ),
        private_element_tags: metadata_string_field(&private_elements, "tag"),
        private_element_vrs: metadata_string_field(&private_elements, "vr"),
        private_element_raw_sha256_values: metadata_string_field(
            &private_elements,
            "raw_value_sha256",
        ),
        sequence_length_variant: sequence
            .and_then(|value| value.get("variant_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        sequence_tag: sequence
            .and_then(|value| value.get("sequence_tag"))
            .and_then(Value::as_str)
            .map(str::to_string),
        sequence_value_length: sequence
            .and_then(|value| value.get("sequence_value_length"))
            .and_then(Value::as_u64),
        sequence_length_field_hex: sequence
            .and_then(|value| value.get("sequence_length_field_hex"))
            .and_then(Value::as_str)
            .map(str::to_string),
        sequence_delimitation_present: sequence
            .and_then(|value| value.get("sequence_delimitation_present"))
            .and_then(Value::as_bool),
        sequence_item_length_encoding: sequence
            .and_then(|value| value.get("item_length_encoding"))
            .and_then(Value::as_str)
            .map(str::to_string),
        sequence_item_delimitation_present: sequence
            .and_then(|value| value.get("item_delimitation_present"))
            .and_then(Value::as_bool),
        sequence_decoded_code: sequence
            .and_then(|value| value.pointer("/decoded_items/0"))
            .and_then(|item| {
                Some(format!(
                    "{}|{}|{}",
                    item.get("code_value")?.as_str()?,
                    item.get("coding_scheme_designator")?.as_str()?,
                    item.get("code_meaning")?.as_str()?
                ))
            }),
    }
}

fn metadata_string_field(elements: &Option<Vec<&Value>>, field: &str) -> Option<Vec<String>> {
    elements
        .as_ref()?
        .iter()
        .map(|element| element.get(field)?.as_str().map(str::to_string))
        .collect()
}

fn metadata_u64_field(elements: &Option<Vec<&Value>>, field: &str) -> Option<Vec<u64>> {
    elements
        .as_ref()?
        .iter()
        .map(|element| element.get(field)?.as_u64())
        .collect()
}

fn metadata_decoded_values(values: &Value) -> Option<String> {
    values
        .as_array()?
        .iter()
        .map(|value| value.get("decoded_value")?.as_str())
        .collect::<Option<Vec<_>>>()
        .map(|values| values.join("\\"))
}

fn report_lut_descriptor(file: &Value, pointer: &str) -> Option<String> {
    let descriptor = file.pointer(pointer)?.as_array()?;
    let values = descriptor
        .iter()
        .map(|value| value.as_u64().map(|value| value.to_string()))
        .collect::<Option<Vec<_>>>()?;
    Some(values.join("\\"))
}

fn report_i64_array(file: &Value, pointer: &str) -> Option<String> {
    let values = file.pointer(pointer)?.as_array()?;
    let values = values
        .iter()
        .map(|value| value.as_i64().map(|value| value.to_string()))
        .collect::<Option<Vec<_>>>()?;
    Some(values.join("\\"))
}

fn report_backslash_number_values(value: &str) -> Option<Vec<f64>> {
    value.split('\\').map(|part| part.parse().ok()).collect()
}

fn report_patient_pixel_spacing(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/geometry/pixel_spacing")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer(
                "/recipe/recipe_parameters/shared_functional_groups/pixel_measures/pixel_spacing",
            )
            .and_then(Value::as_str)
        })
        .or_else(|| {
            file.pointer("/recipe/recipe_parameters/pixel_spacing")
                .and_then(Value::as_str)
        })
}

fn report_image_orientation_patient(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/geometry/image_orientation_patient")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer(
                "/recipe/recipe_parameters/shared_functional_groups/plane_orientation_patient",
            )
            .and_then(Value::as_str)
        })
        .or_else(|| {
            file.pointer("/recipe/recipe_parameters/image_orientation_patient")
                .and_then(Value::as_str)
        })
}

fn report_image_position_patient(file: &Value) -> Option<String> {
    report_string_or_string_array(
        file,
        "/recipe/recipe_parameters/geometry/image_position_patient",
    )
    .or_else(|| {
        report_string_or_string_array(
            file,
            "/recipe/recipe_parameters/per_frame_functional_groups/image_position_patient",
        )
    })
    .or_else(|| {
        report_string_or_string_array(file, "/recipe/recipe_parameters/image_position_patient")
    })
}

fn report_string_or_string_array(file: &Value, pointer: &str) -> Option<String> {
    match file.pointer(pointer)? {
        Value::String(value) => Some(value.clone()),
        Value::Array(values) => values
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .map(|values| values.join("; ")),
        _ => None,
    }
}

fn report_string_or_number_array(file: &Value, pointer: &str) -> Option<String> {
    match file.pointer(pointer)? {
        Value::String(value) => Some(value.clone()),
        Value::Array(values) => values
            .iter()
            .map(report_scalar_label)
            .collect::<Option<Vec<_>>>()
            .map(|values| values.join("; ")),
        value => report_scalar_label(value),
    }
}

#[derive(Default)]
struct EnhancedMrTemporalReportFields {
    time_offsets: Option<String>,
    temporal_position_indices: Option<String>,
    dimension_index_values: Option<String>,
    frame_acquisition_numbers: Option<String>,
    dimension_index_pointer: Option<String>,
    functional_group_pointer: Option<String>,
    time_offset_unit: Option<String>,
}

fn enhanced_mr_temporal_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<EnhancedMrTemporalReportFields, ReportError> {
    const OFFSETS: &str = "/expected_semantics/temporal_position_time_offset";
    const TEMPORAL_INDICES: &str = "/expected_semantics/temporal_position_indices";
    const DIMENSION_INDICES: &str = "/expected_semantics/dimension_index_values";
    const ACQUISITION_NUMBERS: &str = "/expected_semantics/frame_acquisition_numbers";
    const DIMENSION_POINTER: &str =
        "/recipe/recipe_parameters/dimension_index/dimension_index_pointer";
    const FUNCTIONAL_GROUP_POINTER: &str =
        "/recipe/recipe_parameters/dimension_index/functional_group_pointer";
    const UNIT: &str = "/expected_semantics/temporal_position_time_offset_unit";

    let is_temporal = [
        OFFSETS,
        UNIT,
        "/recipe/recipe_parameters/per_frame_functional_groups/temporal_position_time_offset",
    ]
    .iter()
    .any(|pointer| file.pointer(pointer).is_some())
        || file.pointer(DIMENSION_POINTER).and_then(Value::as_str)
            == Some("TemporalPositionTimeOffset")
        || file
            .pointer(FUNCTIONAL_GROUP_POINTER)
            .and_then(Value::as_str)
            == Some("TemporalPositionSequence");
    if !is_temporal {
        return Ok(EnhancedMrTemporalReportFields::default());
    }

    let offsets = strict_number_array(
        manifest_path,
        file,
        OFFSETS,
        "temporal expected semantics must define a numeric temporal_position_time_offset array",
        false,
    )?;
    let temporal_indices = strict_number_array(
        manifest_path,
        file,
        TEMPORAL_INDICES,
        "temporal expected semantics must define an integer temporal_position_indices array",
        true,
    )?;
    let dimension_indices = strict_number_array(
        manifest_path,
        file,
        DIMENSION_INDICES,
        "temporal expected semantics must define an integer dimension_index_values array",
        true,
    )?;
    let acquisition_numbers = strict_number_array(
        manifest_path,
        file,
        ACQUISITION_NUMBERS,
        "temporal expected semantics must define an integer frame_acquisition_numbers array",
        true,
    )?;
    if offsets.0 == 0
        || offsets.0 != temporal_indices.0
        || offsets.0 != dimension_indices.0
        || offsets.0 != acquisition_numbers.0
    {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "temporal offsets, indices, dimension values, and acquisition numbers must be non-empty arrays of equal length",
        });
    }
    let dimension_index_pointer = report_str(
        manifest_path,
        file,
        DIMENSION_POINTER,
        "temporal dimension_index_pointer must be TemporalPositionTimeOffset",
    )?;
    if dimension_index_pointer != "TemporalPositionTimeOffset" {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "temporal dimension_index_pointer must be TemporalPositionTimeOffset",
        });
    }
    let functional_group_pointer = report_str(
        manifest_path,
        file,
        FUNCTIONAL_GROUP_POINTER,
        "temporal functional_group_pointer must be TemporalPositionSequence",
    )?;
    if functional_group_pointer != "TemporalPositionSequence" {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "temporal functional_group_pointer must be TemporalPositionSequence",
        });
    }
    let unit = report_str(
        manifest_path,
        file,
        UNIT,
        "temporal_position_time_offset_unit must be seconds",
    )?;
    if unit != "seconds" {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "temporal_position_time_offset_unit must be seconds",
        });
    }

    Ok(EnhancedMrTemporalReportFields {
        time_offsets: Some(offsets.1),
        temporal_position_indices: Some(temporal_indices.1),
        dimension_index_values: Some(dimension_indices.1),
        frame_acquisition_numbers: Some(acquisition_numbers.1),
        dimension_index_pointer: Some(dimension_index_pointer.to_string()),
        functional_group_pointer: Some(functional_group_pointer.to_string()),
        time_offset_unit: Some(unit.to_string()),
    })
}

fn strict_number_array(
    path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
    integers_only: bool,
) -> Result<(usize, String), ReportError> {
    let values =
        value
            .pointer(pointer)
            .and_then(Value::as_array)
            .ok_or(ReportError::MetadataShape {
                path: path.to_path_buf(),
                message,
            })?;
    if values
        .iter()
        .any(|value| !value.is_number() || (integers_only && value.as_u64().is_none()))
    {
        return Err(ReportError::MetadataShape {
            path: path.to_path_buf(),
            message,
        });
    }
    Ok((
        values.len(),
        values
            .iter()
            .map(|value| {
                value
                    .as_number()
                    .expect("numbers checked above")
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("; "),
    ))
}

fn report_object_array_string_values(file: &Value, pointer: &str, field: &str) -> Option<String> {
    let labels = file
        .pointer(pointer)?
        .as_array()?
        .iter()
        .filter_map(|value| value.get(field).and_then(Value::as_str))
        .collect::<Vec<_>>();
    (!labels.is_empty()).then(|| labels.join("; "))
}

fn report_key_object_referenced_frame_numbers(file: &Value) -> Option<String> {
    let labels = file
        .pointer("/expected_semantics/structured_report/key_objects")?
        .as_array()?
        .iter()
        .filter_map(|value| {
            value
                .get("referenced_frame_numbers")
                .and_then(report_value_array_label)
        })
        .collect::<Vec<_>>();
    (!labels.is_empty()).then(|| labels.join("; "))
}

fn report_value_array_label(value: &Value) -> Option<String> {
    value
        .as_array()?
        .iter()
        .map(report_scalar_label)
        .collect::<Option<Vec<_>>>()
        .map(|values| values.join("; "))
}

#[derive(Default)]
struct U32PixelReportFields {
    stored_values: Option<String>,
    pixel_data_sha256: Option<String>,
    word_byte_order: Option<String>,
    full_unsigned_range: Option<bool>,
}

#[derive(Default)]
struct U1PixelReportFields {
    stored_values: Option<String>,
    decoded_frame_sha256: Option<String>,
    pixel_data_sha256: Option<String>,
    packing_order: Option<String>,
    frame_boundary_policy: Option<String>,
    significant_bits: Option<u64>,
    unused_high_bits: Option<u64>,
    value_field_padding_bytes: Option<u64>,
}

fn u32_pixel_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<U32PixelReportFields, ReportError> {
    if file.get("case_id").and_then(Value::as_str) != Some("classic/sc/mono2_u32_explicit_le") {
        return Ok(U32PixelReportFields::default());
    }
    let expected = file
        .get("expected_u32_pixels")
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "u32 coverage row requires expected_u32_pixels",
        })?;
    let stored_values = report_value_array_label(&expected["stored_values"])
        .filter(|value| value == "0; 65535; 2147483648; 4294967295")
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "u32 coverage row requires the four locked unsigned boundary values",
        })?;
    let pixel_data_sha256 = expected["pixel_data_sha256"]
        .as_str()
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "u32 coverage row requires a lowercase SHA-256 pixel hash",
        })?;
    let word_byte_order = expected["word_byte_order"]
        .as_str()
        .filter(|value| *value == "little_endian")
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "u32 coverage row requires little-endian word order",
        })?;
    if expected["full_unsigned_range"].as_bool() != Some(true) {
        return Err(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "u32 coverage row requires full_unsigned_range true",
        });
    }
    Ok(U32PixelReportFields {
        stored_values: Some(stored_values),
        pixel_data_sha256: Some(pixel_data_sha256.to_string()),
        word_byte_order: Some(word_byte_order.to_string()),
        full_unsigned_range: Some(true),
    })
}

fn u1_pixel_report_fields(
    manifest_path: &Path,
    file: &Value,
) -> Result<U1PixelReportFields, ReportError> {
    if file.get("case_id").and_then(Value::as_str) != Some("classic/sc/mono2_u1_native") {
        return Ok(U1PixelReportFields::default());
    }
    let expected = file
        .get("expected_u1_pixels")
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "one-bit coverage row requires expected_u1_pixels",
        })?;
    let stored_values = report_value_array_label(&expected["stored_values"])
        .filter(|value| value == "1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0; 1; 0")
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "one-bit coverage row requires the locked two-frame checkerboard values",
        })?;
    let decoded_frame_sha256 = report_value_array_label(&expected["decoded_frame_sha256"])
        .filter(|value| value == "a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3; c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae")
        .ok_or(ReportError::MetadataShape {
            path: manifest_path.to_path_buf(),
            message: "one-bit coverage row requires both locked decoded-frame hashes",
        })?;
    let exact_string = |field: &'static str, expected_value: &'static str, message| {
        expected[field]
            .as_str()
            .filter(|value| *value == expected_value)
            .map(str::to_string)
            .ok_or(ReportError::MetadataShape {
                path: manifest_path.to_path_buf(),
                message,
            })
    };
    let exact_u64 = |field: &'static str, expected_value: u64, message| {
        expected[field]
            .as_u64()
            .filter(|value| *value == expected_value)
            .ok_or(ReportError::MetadataShape {
                path: manifest_path.to_path_buf(),
                message,
            })
    };
    Ok(U1PixelReportFields {
        stored_values: Some(stored_values),
        decoded_frame_sha256: Some(decoded_frame_sha256),
        pixel_data_sha256: Some(exact_string(
            "pixel_data_sha256",
            "9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b",
            "one-bit coverage row requires the locked Pixel Data SHA-256",
        )?),
        packing_order: Some(exact_string(
            "packing_order",
            "least_significant_bit_first",
            "one-bit coverage row requires least-significant-bit-first packing",
        )?),
        frame_boundary_policy: Some(exact_string(
            "frame_boundary_policy",
            "continuous_without_per_frame_padding",
            "one-bit coverage row requires continuous cross-frame packing",
        )?),
        significant_bits: Some(exact_u64(
            "significant_bits",
            18,
            "one-bit coverage row requires 18 significant bits",
        )?),
        unused_high_bits: Some(exact_u64(
            "unused_high_bits",
            6,
            "one-bit coverage row requires six unused high bits",
        )?),
        value_field_padding_bytes: Some(exact_u64(
            "value_field_padding_bytes",
            1,
            "one-bit coverage row requires one final Value Field padding byte",
        )?),
    })
}

fn report_scalar_label(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn report_slice_thickness(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/geometry/slice_thickness")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer(
                "/recipe/recipe_parameters/shared_functional_groups/pixel_measures/slice_thickness",
            )
            .and_then(Value::as_str)
        })
        .or_else(|| {
            file.pointer("/recipe/recipe_parameters/slice_thickness")
                .and_then(Value::as_str)
        })
}

fn report_spacing_between_slices(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/geometry/spacing_between_slices")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer(
                "/recipe/recipe_parameters/shared_functional_groups/pixel_measures/spacing_between_slices",
            )
            .and_then(Value::as_str)
        })
}

fn report_slice_location(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/geometry/slice_location")
        .and_then(Value::as_str)
}

fn report_window_center(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/window/center")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer("/recipe/recipe_parameters/window_center")
                .and_then(Value::as_str)
        })
}

fn report_window_width(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/window/width")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer("/recipe/recipe_parameters/window_width")
                .and_then(Value::as_str)
        })
}

fn report_display_shutter_shape(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/display_shutter/shape")
        .and_then(Value::as_str)
}

fn report_display_shutter_presentation_value(file: &Value) -> Option<u64> {
    file.pointer("/recipe/recipe_parameters/display_shutter/presentation_value")
        .and_then(Value::as_u64)
}

fn report_body_part_examined(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/body_part_examined")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer("/expected_semantics/body_part_examined")
                .and_then(Value::as_str)
        })
}

fn report_view_position(file: &Value) -> Option<&str> {
    file.pointer("/recipe/recipe_parameters/view_position")
        .and_then(Value::as_str)
        .or_else(|| {
            file.pointer("/expected_semantics/view_position")
                .and_then(Value::as_str)
        })
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

fn manifest_reference_relationships(
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
                .get("relationship")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| ReportError::MetadataShape {
                    path: manifest_path.to_path_buf(),
                    message: "file reference relationship must be a string",
                })
        })
        .collect()
}

fn manifest_reference_sop_class_uids(
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
                .get("sop_class_uid")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| ReportError::MetadataShape {
                    path: manifest_path.to_path_buf(),
                    message: "file reference sop_class_uid must be a string",
                })
        })
        .collect()
}

fn manifest_reference_sop_instance_uid_roots(
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
                .get("sop_instance_uid")
                .and_then(Value::as_str)
                .ok_or_else(|| ReportError::MetadataShape {
                    path: manifest_path.to_path_buf(),
                    message: "file reference sop_instance_uid must be a string",
                })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|uids| {
            uids.into_iter()
                .filter_map(uid_root_bucket)
                .map(str::to_string)
                .collect()
        })
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
    let transfer_syntax = registry_case
        .get("transfer_syntax_uid")
        .and_then(Value::as_str)
        .unwrap_or("");

    let mut row = serde_json::json!({
        "case_id": case_id,
        "profile": run_profile,
        "profile_membership": report_registry_string_array(registry_case, "profiles"),
        "status": status,
        "iod": registry_case.get("iod_name").and_then(Value::as_str).unwrap_or(""),
        "modality": registry_case.get("modality").and_then(Value::as_str),
        "sop_class_uid": registry_case.get("sop_class_uid").and_then(Value::as_str).unwrap_or(""),
        "transfer_syntax": transfer_syntax,
        "transfer_syntax_name": transfer_syntax_name(transfer_syntax),
        "codec_family": compressed_codec_family(transfer_syntax),
        "codec_backend_id": Value::Null,
        "codec_backend_kind": Value::Null,
        "codec_feature_gate": registry_case.pointer("/requirements/features/0").and_then(Value::as_str),
        "reason_code": skipped.get("reason_code").and_then(Value::as_str),
        "photometric": Value::Null,
        "bits": Value::Null,
        "bits_allocated": Value::Null,
        "bits_stored": Value::Null,
        "high_bit": Value::Null,
        "pixel_representation": Value::Null,
        "samples_per_pixel": Value::Null,
        "planar_configuration": Value::Null,
        "pixel_data_vr": Value::Null,
        "pixel_data_layout": Value::Null,
        "basic_offset_table": Value::Null,
        "encapsulated_fragment_layout": Value::Null,
        "extended_offset_table": Value::Null,
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
        "synthetic_data": Value::Null,
        "image_type": Value::Null,
        "conversion_type": Value::Null,
        "presentation_lut_shape": Value::Null,
        "lossy_image_compression": Value::Null,
        "lossy_image_compression_ratio": Value::Null,
        "lossy_image_compression_method": Value::Null,
        "known_stressors": []
    });
    let row_object = row
        .as_object_mut()
        .expect("skipped coverage row literal must be an object");
    for field in [
        "generation_backend_id",
        "generation_backend_version",
        "generation_backend_determinism",
        "metadata_specific_character_sets",
        "metadata_person_name",
        "metadata_person_name_component_groups",
        "metadata_person_name_component_group_count",
        "metadata_person_name_encoded_sha256",
        "metadata_person_name_encoded_length_bytes",
        "metadata_temporal_boundary_id",
        "metadata_timezone_offset_from_utc",
        "metadata_da_values",
        "metadata_tm_values",
        "metadata_dt_values",
        "metadata_temporal_normalized_utc",
        "metadata_empty_type2_attributes",
        "metadata_empty_type2_attribute_count",
        "metadata_string_tags",
        "metadata_string_vrs",
        "metadata_string_value_multiplicities",
        "metadata_string_max_component_encoded_length_bytes",
        "metadata_string_raw_value_lengths",
        "metadata_string_raw_sha256_values",
        "metadata_private_creator_tags",
        "metadata_private_creator_ids",
        "metadata_private_block_ranges",
        "metadata_private_creator_raw_sha256_values",
        "metadata_private_element_tags",
        "metadata_private_element_vrs",
        "metadata_private_element_raw_sha256_values",
        "metadata_sequence_length_variant",
        "metadata_sequence_tag",
        "metadata_sequence_value_length",
        "metadata_sequence_length_field_hex",
        "metadata_sequence_delimitation_present",
        "metadata_sequence_item_length_encoding",
        "metadata_sequence_item_delimitation_present",
        "metadata_sequence_decoded_code",
        "u32_stored_values",
        "u32_pixel_data_sha256",
        "u32_word_byte_order",
        "u32_full_unsigned_range",
        "u1_stored_values",
        "u1_decoded_frame_sha256",
        "u1_pixel_data_sha256",
        "u1_packing_order",
        "u1_frame_boundary_policy",
        "u1_significant_bits",
        "u1_unused_high_bits",
        "u1_value_field_padding_bytes",
        "nm_frame_increment_pointers",
        "nm_energy_window_vector",
        "nm_detector_vector",
        "nm_energy_window_names",
        "nm_detector_start_angles_degrees",
        "nm_frame_dimension_tuples",
        "enhanced_pet_image_type",
        "enhanced_pet_frame_type",
        "enhanced_pet_view_code",
        "enhanced_pet_view_modifier_item_count",
        "enhanced_pet_slice_progression_direction_present",
        "enhanced_pet_stack_ids",
        "enhanced_pet_in_stack_position_numbers",
        "enhanced_pet_dimension_index_values",
        "enhanced_pet_temporal_position_indices",
        "enhanced_pet_image_positions_patient_mm",
        "enhanced_pet_stored_values_by_frame",
        "enhanced_pet_activity_values_bqml_by_frame",
        "enhanced_pet_rwvm_intercept",
        "enhanced_pet_rwvm_slope",
        "enhanced_pet_rwvm_measurement_units",
        "enhanced_pet_corrections",
        "pet_units",
        "pet_counts_source",
        "pet_series_type",
        "pet_corrected_image",
        "pet_decay_correction",
        "pet_dose_calibration_factor",
        "pet_rescale_intercept",
        "pet_rescale_slope",
        "pet_stored_values",
        "pet_activity_values_bqml",
        "pet_frame_reference_time_ms",
        "pet_actual_frame_duration_ms",
        "pet_image_index",
        "pet_radiopharmaceutical_information_item_count",
        "us_image_type",
        "us_frame_increment_pointer",
        "us_frame_time_ms",
        "us_frame_relative_times_ms",
        "us_frame_count",
        "us_ordered_frame_hashes",
        "us_spatially_related_frames",
        "us_color_data_present",
        "us_region_calibrated",
        "us_lossy_image_compression",
        "xa_image_type",
        "xa_frame_count",
        "xa_body_part_examined",
        "xa_patient_orientation_empty",
        "xa_laterality_present",
        "xa_pixel_intensity_relationship",
        "xa_radiation_setting",
        "xa_kvp",
        "xa_exposure_mas",
        "xa_imager_pixel_spacing_mm",
        "xa_positioner_primary_angle_degrees",
        "xa_positioner_secondary_angle_degrees",
        "xa_distance_source_to_detector_mm",
        "xa_distance_source_to_patient_mm",
        "xa_estimated_radiographic_magnification_factor",
        "xa_lossy_image_compression",
        "xa_multiframe_cine",
        "xa_biplane_data_present",
        "xa_contrast_used",
        "xa_subtraction_applied",
        "xa_table_motion_present",
        "xa_patient_space_geometry_present",
        "xa_pixel_spacing_calibrated",
        "xrf_image_type",
        "xrf_frame_count",
        "xrf_body_part_examined",
        "xrf_patient_orientation_empty",
        "xrf_laterality_present",
        "xrf_pixel_intensity_relationship",
        "xrf_radiation_setting",
        "xrf_kvp",
        "xrf_exposure_mas",
        "xrf_imager_pixel_spacing_mm",
        "xrf_distance_source_to_detector_mm",
        "xrf_distance_source_to_patient_mm",
        "xrf_estimated_radiographic_magnification_factor",
        "xrf_column_angulation_degrees",
        "xrf_lossy_image_compression",
        "xrf_multiframe_cine",
        "xrf_biplane_data_present",
        "xrf_contrast_used",
        "xrf_subtraction_applied",
        "xrf_table_position_present",
        "xrf_table_motion_present",
        "xrf_table_tilt_present",
        "xrf_tomography_present",
        "xrf_patient_space_geometry_present",
        "xrf_pixel_spacing_calibrated",
        "xrf_xa_positioner_angles_present",
    ] {
        row_object.insert(field.to_string(), Value::Null);
    }
    row_object.insert("window_center".to_string(), Value::Null);
    row_object.insert("window_width".to_string(), Value::Null);
    row_object.insert(
        "derived_reference_relationships".to_string(),
        Value::Array(Vec::new()),
    );
    row_object.insert(
        "derived_reference_targets".to_string(),
        Value::Array(Vec::new()),
    );
    row_object.insert(
        "derived_reference_sop_class_uids".to_string(),
        Value::Array(Vec::new()),
    );
    row_object.insert(
        "derived_reference_sop_instance_uid_roots".to_string(),
        Value::Array(Vec::new()),
    );
    row_object.insert("kvp".to_string(), Value::Null);
    row_object.insert("ct_acquisition_number".to_string(), Value::Null);
    row_object.insert("ct_rescale_intercept".to_string(), Value::Null);
    row_object.insert("ct_rescale_slope".to_string(), Value::Null);
    row_object.insert("ct_rescale_type".to_string(), Value::Null);
    row_object.insert(
        "enhanced_ct_dimension_index_values".to_string(),
        Value::Null,
    );
    row_object.insert(
        "enhanced_ct_in_concatenation_number".to_string(),
        Value::Null,
    );
    row_object.insert(
        "enhanced_ct_in_concatenation_total_number".to_string(),
        Value::Null,
    );
    row_object.insert(
        "enhanced_ct_concatenation_frame_offset_number".to_string(),
        Value::Null,
    );
    row_object.insert("mr_scanning_sequence".to_string(), Value::Null);
    row_object.insert("mr_sequence_variant".to_string(), Value::Null);
    row_object.insert("mr_acquisition_type".to_string(), Value::Null);
    row_object.insert("mr_repetition_time".to_string(), Value::Null);
    row_object.insert("mr_echo_time".to_string(), Value::Null);
    row_object.insert("mr_echo_train_length".to_string(), Value::Null);
    row_object.insert("mr_magnetic_field_strength".to_string(), Value::Null);
    row_object.insert("enhanced_mr_effective_echo_times".to_string(), Value::Null);
    row_object.insert(
        "enhanced_mr_temporal_position_time_offsets".to_string(),
        Value::Null,
    );
    for field in [
        "enhanced_mr_temporal_position_indices",
        "enhanced_mr_dimension_index_values",
        "enhanced_mr_frame_acquisition_numbers",
        "enhanced_mr_dimension_index_pointer",
        "enhanced_mr_functional_group_pointer",
        "enhanced_mr_temporal_position_time_offset_unit",
    ] {
        row_object.insert(field.to_string(), Value::Null);
    }
    row_object.insert(
        "enhanced_mr_velocity_encoding_minimum_value".to_string(),
        Value::Null,
    );
    row_object.insert(
        "enhanced_mr_velocity_encoding_maximum_value".to_string(),
        Value::Null,
    );
    row_object.insert("segmentation_type".to_string(), Value::Null);
    row_object.insert("segmentation_fractional_type".to_string(), Value::Null);
    row_object.insert(
        "segmentation_maximum_fractional_value".to_string(),
        Value::Null,
    );
    row_object.insert("gsps_content_label".to_string(), Value::Null);
    row_object.insert("gsps_content_description".to_string(), Value::Null);
    row_object.insert("gsps_presentation_size_mode".to_string(), Value::Null);
    row_object.insert(
        "gsps_presentation_pixel_aspect_ratio".to_string(),
        Value::Null,
    );
    row_object.insert("gsps_window_center".to_string(), Value::Null);
    row_object.insert("gsps_window_width".to_string(), Value::Null);
    row_object.insert("gsps_presentation_lut_shape".to_string(), Value::Null);
    row_object.insert("rwvm_content_label".to_string(), Value::Null);
    row_object.insert("rwvm_lut_label".to_string(), Value::Null);
    row_object.insert("rwvm_first_value_mapped".to_string(), Value::Null);
    row_object.insert("rwvm_last_value_mapped".to_string(), Value::Null);
    row_object.insert("rwvm_intercept".to_string(), Value::Null);
    row_object.insert("rwvm_slope".to_string(), Value::Null);
    row_object.insert("rwvm_units_code_value".to_string(), Value::Null);
    row_object.insert(
        "rwvm_units_coding_scheme_designator".to_string(),
        Value::Null,
    );
    row_object.insert("rwvm_units_code_meaning".to_string(), Value::Null);
    row_object.insert("rwvm_referenced_frame_numbers".to_string(), Value::Null);
    row_object.insert("rt_dose_units".to_string(), Value::Null);
    row_object.insert("rt_dose_type".to_string(), Value::Null);
    row_object.insert("rt_dose_summation_type".to_string(), Value::Null);
    row_object.insert("rt_dose_grid_scaling".to_string(), Value::Null);
    row_object.insert("rt_structure_set_label".to_string(), Value::Null);
    row_object.insert("rt_structure_set_roi_name".to_string(), Value::Null);
    row_object.insert("rt_roi_generation_algorithm".to_string(), Value::Null);
    row_object.insert("rt_contour_geometric_type".to_string(), Value::Null);
    row_object.insert("rt_contour_points".to_string(), Value::Null);
    row_object.insert("rt_roi_interpreted_type".to_string(), Value::Null);
    row_object.insert(
        "encapsulated_document_burned_in_annotation".to_string(),
        Value::Null,
    );
    row_object.insert(
        "encapsulated_document_recognizable_visual_features".to_string(),
        Value::Null,
    );
    row_object.insert("encapsulated_document_title".to_string(), Value::Null);
    row_object.insert("encapsulated_document_mime_type".to_string(), Value::Null);
    row_object.insert("encapsulated_document_length".to_string(), Value::Null);
    row_object.insert("sr_completion_flag".to_string(), Value::Null);
    row_object.insert("sr_verification_flag".to_string(), Value::Null);
    row_object.insert("sr_root_value_type".to_string(), Value::Null);
    row_object.insert("sr_root_continuity_of_content".to_string(), Value::Null);
    row_object.insert("sr_content_sequence_items".to_string(), Value::Null);
    row_object.insert("sr_observation_text".to_string(), Value::Null);
    row_object.insert("sr_measurement_numeric_value".to_string(), Value::Null);
    row_object.insert("kos_document_title".to_string(), Value::Null);
    row_object.insert("kos_key_object_count".to_string(), Value::Null);
    row_object.insert("kos_key_object_relationship_types".to_string(), Value::Null);
    row_object.insert("kos_key_object_value_types".to_string(), Value::Null);
    row_object.insert("kos_referenced_frame_numbers".to_string(), Value::Null);
    row_object.insert("display_shutter_shape".to_string(), Value::Null);
    row_object.insert(
        "display_shutter_presentation_value".to_string(),
        Value::Null,
    );
    row_object.insert("body_part_examined".to_string(), Value::Null);
    row_object.insert("view_position".to_string(), Value::Null);
    row_object.insert("modality_lut_descriptor".to_string(), Value::Null);
    row_object.insert("modality_lut_type".to_string(), Value::Null);
    row_object.insert("modality_lut_data_value_length".to_string(), Value::Null);
    row_object.insert("voi_lut_descriptor".to_string(), Value::Null);
    row_object.insert("voi_lut_data_value_length".to_string(), Value::Null);
    row_object.insert("overlay_rows".to_string(), Value::Null);
    row_object.insert("overlay_columns".to_string(), Value::Null);
    row_object.insert("overlay_type".to_string(), Value::Null);
    row_object.insert("overlay_origin".to_string(), Value::Null);
    row_object.insert("overlay_bits_allocated".to_string(), Value::Null);
    row_object.insert("overlay_bit_position".to_string(), Value::Null);
    row_object.insert("overlay_data_value_length".to_string(), Value::Null);
    row_object.insert("pixel_spacing".to_string(), Value::Null);
    row_object.insert("imager_pixel_spacing".to_string(), Value::Null);
    row_object.insert("image_orientation_patient".to_string(), Value::Null);
    row_object.insert("image_position_patient".to_string(), Value::Null);
    row_object.insert("slice_thickness".to_string(), Value::Null);
    row_object.insert("spacing_between_slices".to_string(), Value::Null);
    row_object.insert("slice_location".to_string(), Value::Null);
    for field in [
        "geometry_sort_basis",
        "geometry_sort_direction",
        "geometry_position_along_normal_mm",
        "geometry_geometric_order_index",
        "geometry_instance_number",
        "geometry_instance_number_order_index",
        "geometry_sorting_conflict_expected",
        "geometry_instance_number_state",
        "geometry_adjacent_spacing_mm",
        "geometry_spacing_uniform",
        "geometry_gantry_detector_tilt_degrees",
        "series_organization_group_id",
        "study_series_count",
        "series_ordinal",
        "series_organization_instance_count",
        "shared_study_instance_uid_expected",
        "shared_frame_of_reference_uid_expected",
        "distinct_series_instance_uids_expected",
    ] {
        row_object.insert(field.to_string(), Value::Null);
    }
    row_object.insert(
        "sop_class_name".to_string(),
        registry_case
            .get("sop_class_name")
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    row_object.insert("study_instance_uid_root".to_string(), Value::Null);
    row_object.insert("series_instance_uid_root".to_string(), Value::Null);
    row_object.insert("sop_instance_uid_root".to_string(), Value::Null);
    Ok(row)
}

fn compressed_codec_family(transfer_syntax_uid: &str) -> Option<&'static str> {
    match transfer_syntax_uid {
        RLE_LOSSLESS_TRANSFER_SYNTAX_UID => Some("RLE Lossless"),
        JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID => Some("JPEG Baseline"),
        JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID => Some("JPEG-LS"),
        JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID => Some("JPEG XL"),
        JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID => Some("JPEG 2000"),
        HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID => Some("HTJ2K"),
        JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID | JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID => {
            Some("Legacy JPEG Lossless")
        }
        DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID => Some("Deflated Image Frame"),
        _ => None,
    }
}

fn transfer_syntax_name(transfer_syntax_uid: &str) -> Option<&'static str> {
    match transfer_syntax_uid {
        uids::IMPLICIT_VR_LITTLE_ENDIAN => Some("Implicit VR Little Endian"),
        uids::EXPLICIT_VR_LITTLE_ENDIAN => Some("Explicit VR Little Endian"),
        "1.2.840.10008.1.2.2" => Some("Explicit VR Big Endian"),
        uids::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN => Some("Deflated Explicit VR Little Endian"),
        RLE_LOSSLESS_TRANSFER_SYNTAX_UID => Some("RLE Lossless"),
        JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID => Some("JPEG Baseline (Process 1)"),
        JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID => Some("JPEG-LS Lossless"),
        JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID => Some("JPEG XL Lossless"),
        JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID => Some("JPEG 2000 Lossless"),
        HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID => Some("HTJ2K Lossless"),
        JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID => Some("JPEG Lossless Process 14"),
        JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID => Some("JPEG Lossless SV1"),
        DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID => Some("Deflated Image Frame Compression"),
        _ => None,
    }
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

fn report_string_array(
    path: &Path,
    value: &Value,
    pointer: &str,
    message: &'static str,
) -> Result<Vec<String>, ReportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError::MetadataShape {
            path: path.to_path_buf(),
            message,
        })?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| ReportError::MetadataShape {
                    path: path.to_path_buf(),
                    message,
                })
        })
        .collect()
}

fn report_registry_string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
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
    profile_memberships: BTreeMap<String, usize>,
    statuses: BTreeMap<String, usize>,
    iods: BTreeMap<String, usize>,
    modalities: BTreeMap<String, usize>,
    sop_classes: BTreeMap<String, usize>,
    sop_class_names: BTreeMap<String, usize>,
    transfer_syntaxes: BTreeMap<String, usize>,
    transfer_syntax_names: BTreeMap<String, usize>,
    codec_families: BTreeMap<String, usize>,
    codec_backends: BTreeMap<String, usize>,
    codec_backend_kinds: BTreeMap<String, usize>,
    codec_feature_gates: BTreeMap<String, usize>,
    generation_backends: BTreeMap<String, usize>,
    determinism: BTreeMap<String, usize>,
    validation_statuses: BTreeMap<String, usize>,
    unavailable_reasons: BTreeMap<String, usize>,
    metadata_specific_character_sets: BTreeMap<String, usize>,
    metadata_person_names: BTreeMap<String, usize>,
    metadata_person_name_component_groups: BTreeMap<String, usize>,
    metadata_person_name_component_group_counts: BTreeMap<String, usize>,
    metadata_person_name_encoded_sha256_values: BTreeMap<String, usize>,
    metadata_person_name_encoded_length_bytes: BTreeMap<String, usize>,
    metadata_temporal_boundary_ids: BTreeMap<String, usize>,
    metadata_timezone_offsets_from_utc: BTreeMap<String, usize>,
    metadata_empty_type2_attributes: BTreeMap<String, usize>,
    metadata_empty_type2_attribute_counts: BTreeMap<String, usize>,
    metadata_string_tags: BTreeMap<String, usize>,
    metadata_string_vrs: BTreeMap<String, usize>,
    metadata_string_value_multiplicities: BTreeMap<String, usize>,
    metadata_string_max_component_encoded_length_bytes: BTreeMap<String, usize>,
    metadata_string_raw_value_lengths: BTreeMap<String, usize>,
    metadata_string_raw_sha256_values: BTreeMap<String, usize>,
    metadata_private_creator_tags: BTreeMap<String, usize>,
    metadata_private_creator_ids: BTreeMap<String, usize>,
    metadata_private_block_ranges: BTreeMap<String, usize>,
    metadata_private_creator_raw_sha256_values: BTreeMap<String, usize>,
    metadata_private_element_tags: BTreeMap<String, usize>,
    metadata_private_element_vrs: BTreeMap<String, usize>,
    metadata_private_element_raw_sha256_values: BTreeMap<String, usize>,
    metadata_sequence_length_variants: BTreeMap<String, usize>,
    metadata_sequence_length_field_hex_values: BTreeMap<String, usize>,
    metadata_sequence_delimitation_states: BTreeMap<String, usize>,
    metadata_sequence_item_length_encodings: BTreeMap<String, usize>,
    photometric_interpretations: BTreeMap<String, usize>,
    bit_depths: BTreeMap<String, usize>,
    bits_allocated: BTreeMap<String, usize>,
    bits_stored: BTreeMap<String, usize>,
    high_bits: BTreeMap<String, usize>,
    pixel_representations: BTreeMap<String, usize>,
    samples_per_pixel: BTreeMap<String, usize>,
    planar_configurations: BTreeMap<String, usize>,
    pixel_data_vrs: BTreeMap<String, usize>,
    pixel_data_layouts: BTreeMap<String, usize>,
    u32_stored_value_sets: BTreeMap<String, usize>,
    u32_pixel_data_sha256_values: BTreeMap<String, usize>,
    u32_word_byte_orders: BTreeMap<String, usize>,
    u32_full_unsigned_range_states: BTreeMap<String, usize>,
    u1_stored_value_sets: BTreeMap<String, usize>,
    u1_pixel_data_sha256_values: BTreeMap<String, usize>,
    u1_packing_orders: BTreeMap<String, usize>,
    u1_frame_boundary_policies: BTreeMap<String, usize>,
    u1_value_field_padding_byte_counts: BTreeMap<String, usize>,
    frame_counts: BTreeMap<String, usize>,
    geometries: BTreeMap<String, usize>,
    pixel_spacings: BTreeMap<String, usize>,
    imager_pixel_spacings: BTreeMap<String, usize>,
    image_orientations_patient: BTreeMap<String, usize>,
    image_positions_patient: BTreeMap<String, usize>,
    slice_thicknesses: BTreeMap<String, usize>,
    spacing_between_slices: BTreeMap<String, usize>,
    slice_locations: BTreeMap<String, usize>,
    object_types: BTreeMap<String, usize>,
    derived_reference_states: BTreeMap<String, usize>,
    derived_reference_relationships: BTreeMap<String, usize>,
    derived_reference_targets: BTreeMap<String, usize>,
    derived_reference_sop_class_uids: BTreeMap<String, usize>,
    derived_reference_sop_instance_uid_roots: BTreeMap<String, usize>,
    synthetic_data: BTreeMap<String, usize>,
    image_types: BTreeMap<String, usize>,
    conversion_types: BTreeMap<String, usize>,
    presentation_lut_shapes: BTreeMap<String, usize>,
    window_centers: BTreeMap<String, usize>,
    window_widths: BTreeMap<String, usize>,
    kvps: BTreeMap<String, usize>,
    ct_acquisition_numbers: BTreeMap<String, usize>,
    ct_rescale_intercepts: BTreeMap<String, usize>,
    ct_rescale_slopes: BTreeMap<String, usize>,
    ct_rescale_types: BTreeMap<String, usize>,
    enhanced_ct_dimension_index_values: BTreeMap<String, usize>,
    enhanced_ct_in_concatenation_numbers: BTreeMap<String, usize>,
    enhanced_ct_in_concatenation_total_numbers: BTreeMap<String, usize>,
    enhanced_ct_concatenation_frame_offset_numbers: BTreeMap<String, usize>,
    nm_frame_increment_pointers: BTreeMap<String, usize>,
    nm_energy_window_vectors: BTreeMap<String, usize>,
    nm_detector_vectors: BTreeMap<String, usize>,
    nm_energy_window_names: BTreeMap<String, usize>,
    nm_detector_start_angles_degrees: BTreeMap<String, usize>,
    nm_frame_dimension_tuples: BTreeMap<String, usize>,
    enhanced_pet_image_types: BTreeMap<String, usize>,
    enhanced_pet_frame_types: BTreeMap<String, usize>,
    enhanced_pet_view_codes: BTreeMap<String, usize>,
    enhanced_pet_view_modifier_item_counts: BTreeMap<String, usize>,
    enhanced_pet_slice_progression_direction_states: BTreeMap<String, usize>,
    enhanced_pet_stack_ids: BTreeMap<String, usize>,
    enhanced_pet_in_stack_position_numbers: BTreeMap<String, usize>,
    enhanced_pet_dimension_index_values: BTreeMap<String, usize>,
    enhanced_pet_temporal_position_indices: BTreeMap<String, usize>,
    enhanced_pet_image_positions_patient_mm: BTreeMap<String, usize>,
    enhanced_pet_stored_values_by_frame: BTreeMap<String, usize>,
    enhanced_pet_activity_values_bqml_by_frame: BTreeMap<String, usize>,
    enhanced_pet_rwvm_intercepts: BTreeMap<String, usize>,
    enhanced_pet_rwvm_slopes: BTreeMap<String, usize>,
    enhanced_pet_rwvm_measurement_units: BTreeMap<String, usize>,
    enhanced_pet_corrections: BTreeMap<String, usize>,
    pet_units: BTreeMap<String, usize>,
    pet_counts_sources: BTreeMap<String, usize>,
    pet_series_types: BTreeMap<String, usize>,
    pet_corrected_images: BTreeMap<String, usize>,
    pet_decay_corrections: BTreeMap<String, usize>,
    pet_dose_calibration_factors: BTreeMap<String, usize>,
    pet_rescale_intercepts: BTreeMap<String, usize>,
    pet_rescale_slopes: BTreeMap<String, usize>,
    pet_stored_values: BTreeMap<String, usize>,
    pet_activity_values_bqml: BTreeMap<String, usize>,
    pet_frame_reference_times_ms: BTreeMap<String, usize>,
    pet_actual_frame_durations_ms: BTreeMap<String, usize>,
    pet_image_indices: BTreeMap<String, usize>,
    pet_radiopharmaceutical_information_item_counts: BTreeMap<String, usize>,
    us_image_types: BTreeMap<String, usize>,
    us_frame_increment_pointers: BTreeMap<String, usize>,
    us_frame_times_ms: BTreeMap<String, usize>,
    us_frame_counts: BTreeMap<String, usize>,
    us_spatially_related_frames: BTreeMap<String, usize>,
    us_color_data_present: BTreeMap<String, usize>,
    us_region_calibrated: BTreeMap<String, usize>,
    us_lossy_image_compressions: BTreeMap<String, usize>,
    xa_image_types: BTreeMap<String, usize>,
    xa_frame_counts: BTreeMap<String, usize>,
    xa_body_parts_examined: BTreeMap<String, usize>,
    xa_patient_orientation_empty_states: BTreeMap<String, usize>,
    xa_laterality_present_states: BTreeMap<String, usize>,
    xa_pixel_intensity_relationships: BTreeMap<String, usize>,
    xa_radiation_settings: BTreeMap<String, usize>,
    xa_kvps: BTreeMap<String, usize>,
    xa_exposures_mas: BTreeMap<String, usize>,
    xa_imager_pixel_spacings_mm: BTreeMap<String, usize>,
    xa_positioner_primary_angles_degrees: BTreeMap<String, usize>,
    xa_positioner_secondary_angles_degrees: BTreeMap<String, usize>,
    xa_distances_source_to_detector_mm: BTreeMap<String, usize>,
    xa_distances_source_to_patient_mm: BTreeMap<String, usize>,
    xa_estimated_radiographic_magnification_factors: BTreeMap<String, usize>,
    xa_lossy_image_compressions: BTreeMap<String, usize>,
    xa_multiframe_cine_states: BTreeMap<String, usize>,
    xa_biplane_data_present_states: BTreeMap<String, usize>,
    xa_contrast_used_states: BTreeMap<String, usize>,
    xa_subtraction_applied_states: BTreeMap<String, usize>,
    xa_table_motion_present_states: BTreeMap<String, usize>,
    xa_patient_space_geometry_present_states: BTreeMap<String, usize>,
    xa_pixel_spacing_calibrated_states: BTreeMap<String, usize>,
    xrf_image_types: BTreeMap<String, usize>,
    xrf_frame_counts: BTreeMap<String, usize>,
    xrf_body_parts_examined: BTreeMap<String, usize>,
    xrf_patient_orientation_empty_states: BTreeMap<String, usize>,
    xrf_laterality_present_states: BTreeMap<String, usize>,
    xrf_pixel_intensity_relationships: BTreeMap<String, usize>,
    xrf_radiation_settings: BTreeMap<String, usize>,
    xrf_kvps: BTreeMap<String, usize>,
    xrf_exposures_mas: BTreeMap<String, usize>,
    xrf_imager_pixel_spacings_mm: BTreeMap<String, usize>,
    xrf_distances_source_to_detector_mm: BTreeMap<String, usize>,
    xrf_distances_source_to_patient_mm: BTreeMap<String, usize>,
    xrf_estimated_radiographic_magnification_factors: BTreeMap<String, usize>,
    xrf_column_angulations_degrees: BTreeMap<String, usize>,
    xrf_lossy_image_compressions: BTreeMap<String, usize>,
    xrf_multiframe_cine_states: BTreeMap<String, usize>,
    xrf_biplane_data_present_states: BTreeMap<String, usize>,
    xrf_contrast_used_states: BTreeMap<String, usize>,
    xrf_subtraction_applied_states: BTreeMap<String, usize>,
    xrf_table_position_present_states: BTreeMap<String, usize>,
    xrf_table_motion_present_states: BTreeMap<String, usize>,
    xrf_table_tilt_present_states: BTreeMap<String, usize>,
    xrf_tomography_present_states: BTreeMap<String, usize>,
    xrf_patient_space_geometry_present_states: BTreeMap<String, usize>,
    xrf_pixel_spacing_calibrated_states: BTreeMap<String, usize>,
    xrf_xa_positioner_angles_present_states: BTreeMap<String, usize>,
    mr_scanning_sequences: BTreeMap<String, usize>,
    mr_sequence_variants: BTreeMap<String, usize>,
    mr_acquisition_types: BTreeMap<String, usize>,
    mr_repetition_times: BTreeMap<String, usize>,
    mr_echo_times: BTreeMap<String, usize>,
    mr_echo_train_lengths: BTreeMap<String, usize>,
    mr_magnetic_field_strengths: BTreeMap<String, usize>,
    enhanced_mr_effective_echo_times: BTreeMap<String, usize>,
    enhanced_mr_temporal_position_time_offsets: BTreeMap<String, usize>,
    enhanced_mr_temporal_position_indices: BTreeMap<String, usize>,
    enhanced_mr_dimension_index_values: BTreeMap<String, usize>,
    enhanced_mr_frame_acquisition_numbers: BTreeMap<String, usize>,
    enhanced_mr_dimension_index_pointers: BTreeMap<String, usize>,
    enhanced_mr_functional_group_pointers: BTreeMap<String, usize>,
    enhanced_mr_temporal_position_time_offset_units: BTreeMap<String, usize>,
    enhanced_mr_velocity_encoding_minimum_values: BTreeMap<String, usize>,
    enhanced_mr_velocity_encoding_maximum_values: BTreeMap<String, usize>,
    segmentation_types: BTreeMap<String, usize>,
    segmentation_fractional_types: BTreeMap<String, usize>,
    segmentation_maximum_fractional_values: BTreeMap<String, usize>,
    gsps_content_labels: BTreeMap<String, usize>,
    gsps_content_descriptions: BTreeMap<String, usize>,
    gsps_presentation_size_modes: BTreeMap<String, usize>,
    gsps_presentation_pixel_aspect_ratios: BTreeMap<String, usize>,
    gsps_window_centers: BTreeMap<String, usize>,
    gsps_window_widths: BTreeMap<String, usize>,
    gsps_presentation_lut_shapes: BTreeMap<String, usize>,
    rwvm_content_labels: BTreeMap<String, usize>,
    rwvm_lut_labels: BTreeMap<String, usize>,
    rwvm_first_values_mapped: BTreeMap<String, usize>,
    rwvm_last_values_mapped: BTreeMap<String, usize>,
    rwvm_intercepts: BTreeMap<String, usize>,
    rwvm_slopes: BTreeMap<String, usize>,
    rwvm_units_code_values: BTreeMap<String, usize>,
    rwvm_units_coding_scheme_designators: BTreeMap<String, usize>,
    rwvm_units_code_meanings: BTreeMap<String, usize>,
    rwvm_referenced_frame_numbers: BTreeMap<String, usize>,
    rt_dose_units: BTreeMap<String, usize>,
    rt_dose_types: BTreeMap<String, usize>,
    rt_dose_summation_types: BTreeMap<String, usize>,
    rt_dose_grid_scalings: BTreeMap<String, usize>,
    rt_structure_set_labels: BTreeMap<String, usize>,
    rt_structure_set_roi_names: BTreeMap<String, usize>,
    rt_roi_generation_algorithms: BTreeMap<String, usize>,
    rt_contour_geometric_types: BTreeMap<String, usize>,
    rt_contour_points: BTreeMap<String, usize>,
    rt_roi_interpreted_types: BTreeMap<String, usize>,
    encapsulated_document_burned_in_annotations: BTreeMap<String, usize>,
    encapsulated_document_recognizable_visual_features: BTreeMap<String, usize>,
    encapsulated_document_titles: BTreeMap<String, usize>,
    encapsulated_document_mime_types: BTreeMap<String, usize>,
    encapsulated_document_lengths: BTreeMap<String, usize>,
    sr_completion_flags: BTreeMap<String, usize>,
    sr_verification_flags: BTreeMap<String, usize>,
    sr_root_value_types: BTreeMap<String, usize>,
    sr_root_continuity_of_content: BTreeMap<String, usize>,
    sr_content_sequence_item_counts: BTreeMap<String, usize>,
    sr_observation_texts: BTreeMap<String, usize>,
    sr_measurement_numeric_values: BTreeMap<String, usize>,
    kos_document_titles: BTreeMap<String, usize>,
    kos_key_object_counts: BTreeMap<String, usize>,
    kos_key_object_relationship_types: BTreeMap<String, usize>,
    kos_key_object_value_types: BTreeMap<String, usize>,
    kos_referenced_frame_numbers: BTreeMap<String, usize>,
    modality_lut_descriptors: BTreeMap<String, usize>,
    modality_lut_types: BTreeMap<String, usize>,
    modality_lut_data_value_lengths: BTreeMap<String, usize>,
    voi_lut_descriptors: BTreeMap<String, usize>,
    voi_lut_data_value_lengths: BTreeMap<String, usize>,
    overlay_geometries: BTreeMap<String, usize>,
    overlay_types: BTreeMap<String, usize>,
    overlay_origins: BTreeMap<String, usize>,
    overlay_bits_allocated: BTreeMap<String, usize>,
    overlay_bit_positions: BTreeMap<String, usize>,
    overlay_data_value_lengths: BTreeMap<String, usize>,
    display_shutter_shapes: BTreeMap<String, usize>,
    display_shutter_presentation_values: BTreeMap<String, usize>,
    body_parts_examined: BTreeMap<String, usize>,
    view_positions: BTreeMap<String, usize>,
    study_instance_uid_roots: BTreeMap<String, usize>,
    series_instance_uid_roots: BTreeMap<String, usize>,
    sop_instance_uid_roots: BTreeMap<String, usize>,
    lossy_image_compression: BTreeMap<String, usize>,
    lossy_image_compression_ratios: BTreeMap<String, usize>,
    lossy_image_compression_methods: BTreeMap<String, usize>,
    known_stressors: BTreeMap<String, usize>,
    basic_offset_tables: BTreeMap<String, usize>,
    encapsulated_fragment_layouts: BTreeMap<String, usize>,
    extended_offset_tables: BTreeMap<String, usize>,
}

impl GroupedCoverage {
    fn record(&mut self, row: &Value) {
        increment_map(
            &mut self.profiles,
            row.get("profile").and_then(Value::as_str),
        );
        if let Some(profile_memberships) = row.get("profile_membership").and_then(Value::as_array) {
            for profile in profile_memberships {
                increment_map(&mut self.profile_memberships, profile.as_str());
            }
        }
        increment_map(
            &mut self.statuses,
            row.get("status").and_then(Value::as_str),
        );
        increment_map(&mut self.iods, row.get("iod").and_then(Value::as_str));
        increment_map(
            &mut self.modalities,
            row.get("modality").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sop_classes,
            row.get("sop_class_uid").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sop_class_names,
            row.get("sop_class_name").and_then(Value::as_str),
        );
        increment_map(
            &mut self.transfer_syntaxes,
            row.get("transfer_syntax").and_then(Value::as_str),
        );
        increment_map(
            &mut self.transfer_syntax_names,
            row.get("transfer_syntax_name").and_then(Value::as_str),
        );
        increment_map(
            &mut self.codec_families,
            row.get("codec_family").and_then(Value::as_str),
        );
        increment_map(
            &mut self.codec_backends,
            row.get("codec_backend_id").and_then(Value::as_str),
        );
        increment_map(
            &mut self.codec_backend_kinds,
            row.get("codec_backend_kind").and_then(Value::as_str),
        );
        increment_map(
            &mut self.codec_feature_gates,
            row.get("codec_feature_gate").and_then(Value::as_str),
        );
        increment_map(
            &mut self.generation_backends,
            row.get("generation_backend_id").and_then(Value::as_str),
        );
        increment_map(
            &mut self.determinism,
            row.get("determinism").and_then(Value::as_str),
        );
        increment_map(
            &mut self.validation_statuses,
            row.get("validation_status").and_then(Value::as_str),
        );
        if matches!(
            row.get("status").and_then(Value::as_str),
            Some("blocked" | "planned" | "skipped" | "unavailable")
        ) {
            increment_map(
                &mut self.unavailable_reasons,
                row.get("reason_code").and_then(Value::as_str),
            );
        }
        increment_map(
            &mut self.metadata_specific_character_sets,
            row.get("metadata_specific_character_sets")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.metadata_person_names,
            row.get("metadata_person_name").and_then(Value::as_str),
        );
        increment_map(
            &mut self.metadata_person_name_component_groups,
            row.get("metadata_person_name_component_groups")
                .and_then(Value::as_str),
        );
        if let Some(count) = row
            .get("metadata_person_name_component_group_count")
            .and_then(Value::as_u64)
        {
            *self
                .metadata_person_name_component_group_counts
                .entry(count.to_string())
                .or_default() += 1;
        }
        increment_string_array_map(
            &mut self.metadata_string_tags,
            row.get("metadata_string_tags"),
        );
        increment_string_array_map(
            &mut self.metadata_string_vrs,
            row.get("metadata_string_vrs"),
        );
        increment_u64_array_map(
            &mut self.metadata_string_value_multiplicities,
            row.get("metadata_string_value_multiplicities"),
        );
        increment_u64_array_map(
            &mut self.metadata_string_max_component_encoded_length_bytes,
            row.get("metadata_string_max_component_encoded_length_bytes"),
        );
        increment_u64_array_map(
            &mut self.metadata_string_raw_value_lengths,
            row.get("metadata_string_raw_value_lengths"),
        );
        increment_string_array_map(
            &mut self.metadata_string_raw_sha256_values,
            row.get("metadata_string_raw_sha256_values"),
        );
        for (map, field) in [
            (
                &mut self.metadata_private_creator_tags,
                "metadata_private_creator_tags",
            ),
            (
                &mut self.metadata_private_creator_ids,
                "metadata_private_creator_ids",
            ),
            (
                &mut self.metadata_private_block_ranges,
                "metadata_private_block_ranges",
            ),
            (
                &mut self.metadata_private_creator_raw_sha256_values,
                "metadata_private_creator_raw_sha256_values",
            ),
            (
                &mut self.metadata_private_element_tags,
                "metadata_private_element_tags",
            ),
            (
                &mut self.metadata_private_element_vrs,
                "metadata_private_element_vrs",
            ),
            (
                &mut self.metadata_private_element_raw_sha256_values,
                "metadata_private_element_raw_sha256_values",
            ),
        ] {
            increment_string_array_map(map, row.get(field));
        }
        increment_map(
            &mut self.metadata_sequence_length_variants,
            row.get("metadata_sequence_length_variant")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.metadata_sequence_length_field_hex_values,
            row.get("metadata_sequence_length_field_hex")
                .and_then(Value::as_str),
        );
        if let Some(value) = row
            .get("metadata_sequence_delimitation_present")
            .and_then(Value::as_bool)
        {
            *self
                .metadata_sequence_delimitation_states
                .entry(value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.metadata_sequence_item_length_encodings,
            row.get("metadata_sequence_item_length_encoding")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.metadata_person_name_encoded_sha256_values,
            row.get("metadata_person_name_encoded_sha256")
                .and_then(Value::as_str),
        );
        if let Some(length) = row
            .get("metadata_person_name_encoded_length_bytes")
            .and_then(Value::as_u64)
        {
            *self
                .metadata_person_name_encoded_length_bytes
                .entry(length.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.metadata_temporal_boundary_ids,
            row.get("metadata_temporal_boundary_id")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.metadata_timezone_offsets_from_utc,
            row.get("metadata_timezone_offset_from_utc")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.metadata_empty_type2_attributes,
            row.get("metadata_empty_type2_attributes")
                .and_then(Value::as_str),
        );
        if let Some(count) = row
            .get("metadata_empty_type2_attribute_count")
            .and_then(Value::as_u64)
        {
            *self
                .metadata_empty_type2_attribute_counts
                .entry(count.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.photometric_interpretations,
            row.get("photometric").and_then(Value::as_str),
        );
        if let Some(bits) = row.get("bits").and_then(Value::as_u64) {
            *self.bit_depths.entry(bits.to_string()).or_default() += 1;
        }
        if let Some(bits_allocated) = row.get("bits_allocated").and_then(Value::as_u64) {
            *self
                .bits_allocated
                .entry(bits_allocated.to_string())
                .or_default() += 1;
        }
        if let Some(bits_stored) = row.get("bits_stored").and_then(Value::as_u64) {
            *self.bits_stored.entry(bits_stored.to_string()).or_default() += 1;
        }
        if let Some(high_bit) = row.get("high_bit").and_then(Value::as_u64) {
            *self.high_bits.entry(high_bit.to_string()).or_default() += 1;
        }
        if let Some(pixel_representation) = row.get("pixel_representation").and_then(Value::as_u64)
        {
            *self
                .pixel_representations
                .entry(pixel_representation.to_string())
                .or_default() += 1;
        }
        if let Some(samples_per_pixel) = row.get("samples_per_pixel").and_then(Value::as_u64) {
            *self
                .samples_per_pixel
                .entry(samples_per_pixel.to_string())
                .or_default() += 1;
        }
        if let Some(planar_configuration) = row.get("planar_configuration").and_then(Value::as_u64)
        {
            *self
                .planar_configurations
                .entry(planar_configuration.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.pixel_data_vrs,
            row.get("pixel_data_vr").and_then(Value::as_str),
        );
        increment_map(
            &mut self.pixel_data_layouts,
            row.get("pixel_data_layout").and_then(Value::as_str),
        );
        increment_map(
            &mut self.u32_stored_value_sets,
            row.get("u32_stored_values").and_then(Value::as_str),
        );
        increment_map(
            &mut self.u32_pixel_data_sha256_values,
            row.get("u32_pixel_data_sha256").and_then(Value::as_str),
        );
        increment_map(
            &mut self.u32_word_byte_orders,
            row.get("u32_word_byte_order").and_then(Value::as_str),
        );
        if let Some(value) = row.get("u32_full_unsigned_range").and_then(Value::as_bool) {
            *self
                .u32_full_unsigned_range_states
                .entry(value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.u1_stored_value_sets,
            row.get("u1_stored_values").and_then(Value::as_str),
        );
        increment_map(
            &mut self.u1_pixel_data_sha256_values,
            row.get("u1_pixel_data_sha256").and_then(Value::as_str),
        );
        increment_map(
            &mut self.u1_packing_orders,
            row.get("u1_packing_order").and_then(Value::as_str),
        );
        increment_map(
            &mut self.u1_frame_boundary_policies,
            row.get("u1_frame_boundary_policy").and_then(Value::as_str),
        );
        if let Some(value) = row
            .get("u1_value_field_padding_bytes")
            .and_then(Value::as_u64)
        {
            *self
                .u1_value_field_padding_byte_counts
                .entry(value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.basic_offset_tables,
            row.get("basic_offset_table").and_then(Value::as_str),
        );
        increment_map(
            &mut self.encapsulated_fragment_layouts,
            row.get("encapsulated_fragment_layout")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.extended_offset_tables,
            row.get("extended_offset_table").and_then(Value::as_str),
        );
        if let Some(frames) = row.get("frames").and_then(Value::as_u64) {
            *self.frame_counts.entry(frames.to_string()).or_default() += 1;
        }
        if let Some(geometry) = geometry_bucket(row) {
            *self.geometries.entry(geometry).or_default() += 1;
        }
        for (map, field) in [
            (
                &mut self.nm_frame_increment_pointers,
                "nm_frame_increment_pointers",
            ),
            (
                &mut self.nm_energy_window_vectors,
                "nm_energy_window_vector",
            ),
            (&mut self.nm_detector_vectors, "nm_detector_vector"),
            (&mut self.nm_energy_window_names, "nm_energy_window_names"),
            (
                &mut self.nm_detector_start_angles_degrees,
                "nm_detector_start_angles_degrees",
            ),
            (
                &mut self.nm_frame_dimension_tuples,
                "nm_frame_dimension_tuples",
            ),
        ] {
            increment_map(map, row.get(field).and_then(Value::as_str));
        }
        for (map, field) in [
            (
                &mut self.enhanced_pet_image_types,
                "enhanced_pet_image_type",
            ),
            (
                &mut self.enhanced_pet_frame_types,
                "enhanced_pet_frame_type",
            ),
            (&mut self.enhanced_pet_view_codes, "enhanced_pet_view_code"),
            (&mut self.enhanced_pet_stack_ids, "enhanced_pet_stack_ids"),
            (
                &mut self.enhanced_pet_in_stack_position_numbers,
                "enhanced_pet_in_stack_position_numbers",
            ),
            (
                &mut self.enhanced_pet_dimension_index_values,
                "enhanced_pet_dimension_index_values",
            ),
            (
                &mut self.enhanced_pet_temporal_position_indices,
                "enhanced_pet_temporal_position_indices",
            ),
            (
                &mut self.enhanced_pet_image_positions_patient_mm,
                "enhanced_pet_image_positions_patient_mm",
            ),
            (
                &mut self.enhanced_pet_stored_values_by_frame,
                "enhanced_pet_stored_values_by_frame",
            ),
            (
                &mut self.enhanced_pet_activity_values_bqml_by_frame,
                "enhanced_pet_activity_values_bqml_by_frame",
            ),
            (
                &mut self.enhanced_pet_rwvm_measurement_units,
                "enhanced_pet_rwvm_measurement_units",
            ),
            (
                &mut self.enhanced_pet_corrections,
                "enhanced_pet_corrections",
            ),
        ] {
            increment_map(map, row.get(field).and_then(Value::as_str));
        }
        for (map, field) in [
            (
                &mut self.enhanced_pet_view_modifier_item_counts,
                "enhanced_pet_view_modifier_item_count",
            ),
            (
                &mut self.enhanced_pet_slice_progression_direction_states,
                "enhanced_pet_slice_progression_direction_present",
            ),
            (
                &mut self.enhanced_pet_rwvm_intercepts,
                "enhanced_pet_rwvm_intercept",
            ),
            (
                &mut self.enhanced_pet_rwvm_slopes,
                "enhanced_pet_rwvm_slope",
            ),
        ] {
            increment_scalar_map(map, row.get(field));
        }
        for (map, field) in [
            (&mut self.pet_units, "pet_units"),
            (&mut self.pet_counts_sources, "pet_counts_source"),
            (&mut self.pet_series_types, "pet_series_type"),
            (&mut self.pet_corrected_images, "pet_corrected_image"),
            (&mut self.pet_decay_corrections, "pet_decay_correction"),
            (&mut self.pet_stored_values, "pet_stored_values"),
            (
                &mut self.pet_activity_values_bqml,
                "pet_activity_values_bqml",
            ),
        ] {
            increment_map(map, row.get(field).and_then(Value::as_str));
        }
        for (map, field) in [
            (
                &mut self.pet_dose_calibration_factors,
                "pet_dose_calibration_factor",
            ),
            (&mut self.pet_rescale_intercepts, "pet_rescale_intercept"),
            (&mut self.pet_rescale_slopes, "pet_rescale_slope"),
            (
                &mut self.pet_frame_reference_times_ms,
                "pet_frame_reference_time_ms",
            ),
            (
                &mut self.pet_actual_frame_durations_ms,
                "pet_actual_frame_duration_ms",
            ),
            (&mut self.pet_image_indices, "pet_image_index"),
            (
                &mut self.pet_radiopharmaceutical_information_item_counts,
                "pet_radiopharmaceutical_information_item_count",
            ),
        ] {
            increment_scalar_map(map, row.get(field));
        }
        for (map, field) in [
            (&mut self.us_image_types, "us_image_type"),
            (
                &mut self.us_frame_increment_pointers,
                "us_frame_increment_pointer",
            ),
            (
                &mut self.us_lossy_image_compressions,
                "us_lossy_image_compression",
            ),
        ] {
            increment_map(map, row.get(field).and_then(Value::as_str));
        }
        for (map, field) in [
            (&mut self.us_frame_times_ms, "us_frame_time_ms"),
            (&mut self.us_frame_counts, "us_frame_count"),
        ] {
            increment_scalar_map(map, row.get(field));
        }
        for (map, field) in [
            (
                &mut self.us_spatially_related_frames,
                "us_spatially_related_frames",
            ),
            (&mut self.us_color_data_present, "us_color_data_present"),
            (&mut self.us_region_calibrated, "us_region_calibrated"),
        ] {
            if let Some(value) = row.get(field).and_then(Value::as_bool) {
                *map.entry(value.to_string()).or_default() += 1;
            }
        }
        for (map, field) in [
            (&mut self.xa_image_types, "xa_image_type"),
            (&mut self.xa_body_parts_examined, "xa_body_part_examined"),
            (
                &mut self.xa_pixel_intensity_relationships,
                "xa_pixel_intensity_relationship",
            ),
            (&mut self.xa_radiation_settings, "xa_radiation_setting"),
            (
                &mut self.xa_imager_pixel_spacings_mm,
                "xa_imager_pixel_spacing_mm",
            ),
            (
                &mut self.xa_lossy_image_compressions,
                "xa_lossy_image_compression",
            ),
        ] {
            increment_map(map, row.get(field).and_then(Value::as_str));
        }
        for (map, field) in [
            (&mut self.xa_frame_counts, "xa_frame_count"),
            (&mut self.xa_kvps, "xa_kvp"),
            (&mut self.xa_exposures_mas, "xa_exposure_mas"),
            (
                &mut self.xa_positioner_primary_angles_degrees,
                "xa_positioner_primary_angle_degrees",
            ),
            (
                &mut self.xa_positioner_secondary_angles_degrees,
                "xa_positioner_secondary_angle_degrees",
            ),
            (
                &mut self.xa_distances_source_to_detector_mm,
                "xa_distance_source_to_detector_mm",
            ),
            (
                &mut self.xa_distances_source_to_patient_mm,
                "xa_distance_source_to_patient_mm",
            ),
            (
                &mut self.xa_estimated_radiographic_magnification_factors,
                "xa_estimated_radiographic_magnification_factor",
            ),
        ] {
            increment_scalar_map(map, row.get(field));
        }
        for (map, field) in [
            (
                &mut self.xa_patient_orientation_empty_states,
                "xa_patient_orientation_empty",
            ),
            (
                &mut self.xa_laterality_present_states,
                "xa_laterality_present",
            ),
            (&mut self.xa_multiframe_cine_states, "xa_multiframe_cine"),
            (
                &mut self.xa_biplane_data_present_states,
                "xa_biplane_data_present",
            ),
            (&mut self.xa_contrast_used_states, "xa_contrast_used"),
            (
                &mut self.xa_subtraction_applied_states,
                "xa_subtraction_applied",
            ),
            (
                &mut self.xa_table_motion_present_states,
                "xa_table_motion_present",
            ),
            (
                &mut self.xa_patient_space_geometry_present_states,
                "xa_patient_space_geometry_present",
            ),
            (
                &mut self.xa_pixel_spacing_calibrated_states,
                "xa_pixel_spacing_calibrated",
            ),
        ] {
            if let Some(value) = row.get(field).and_then(Value::as_bool) {
                *map.entry(value.to_string()).or_default() += 1;
            }
        }
        for (map, field) in [
            (&mut self.xrf_image_types, "xrf_image_type"),
            (&mut self.xrf_body_parts_examined, "xrf_body_part_examined"),
            (
                &mut self.xrf_pixel_intensity_relationships,
                "xrf_pixel_intensity_relationship",
            ),
            (&mut self.xrf_radiation_settings, "xrf_radiation_setting"),
            (
                &mut self.xrf_imager_pixel_spacings_mm,
                "xrf_imager_pixel_spacing_mm",
            ),
            (
                &mut self.xrf_lossy_image_compressions,
                "xrf_lossy_image_compression",
            ),
        ] {
            increment_map(map, row.get(field).and_then(Value::as_str));
        }
        for (map, field) in [
            (&mut self.xrf_frame_counts, "xrf_frame_count"),
            (&mut self.xrf_kvps, "xrf_kvp"),
            (&mut self.xrf_exposures_mas, "xrf_exposure_mas"),
            (
                &mut self.xrf_distances_source_to_detector_mm,
                "xrf_distance_source_to_detector_mm",
            ),
            (
                &mut self.xrf_distances_source_to_patient_mm,
                "xrf_distance_source_to_patient_mm",
            ),
            (
                &mut self.xrf_estimated_radiographic_magnification_factors,
                "xrf_estimated_radiographic_magnification_factor",
            ),
            (
                &mut self.xrf_column_angulations_degrees,
                "xrf_column_angulation_degrees",
            ),
        ] {
            increment_scalar_map(map, row.get(field));
        }
        for (map, field) in [
            (
                &mut self.xrf_patient_orientation_empty_states,
                "xrf_patient_orientation_empty",
            ),
            (
                &mut self.xrf_laterality_present_states,
                "xrf_laterality_present",
            ),
            (&mut self.xrf_multiframe_cine_states, "xrf_multiframe_cine"),
            (
                &mut self.xrf_biplane_data_present_states,
                "xrf_biplane_data_present",
            ),
            (&mut self.xrf_contrast_used_states, "xrf_contrast_used"),
            (
                &mut self.xrf_subtraction_applied_states,
                "xrf_subtraction_applied",
            ),
            (
                &mut self.xrf_table_position_present_states,
                "xrf_table_position_present",
            ),
            (
                &mut self.xrf_table_motion_present_states,
                "xrf_table_motion_present",
            ),
            (
                &mut self.xrf_table_tilt_present_states,
                "xrf_table_tilt_present",
            ),
            (
                &mut self.xrf_tomography_present_states,
                "xrf_tomography_present",
            ),
            (
                &mut self.xrf_patient_space_geometry_present_states,
                "xrf_patient_space_geometry_present",
            ),
            (
                &mut self.xrf_pixel_spacing_calibrated_states,
                "xrf_pixel_spacing_calibrated",
            ),
            (
                &mut self.xrf_xa_positioner_angles_present_states,
                "xrf_xa_positioner_angles_present",
            ),
        ] {
            if let Some(value) = row.get(field).and_then(Value::as_bool) {
                *map.entry(value.to_string()).or_default() += 1;
            }
        }
        increment_map(
            &mut self.pixel_spacings,
            row.get("pixel_spacing").and_then(Value::as_str),
        );
        increment_map(
            &mut self.imager_pixel_spacings,
            row.get("imager_pixel_spacing").and_then(Value::as_str),
        );
        increment_map(
            &mut self.image_orientations_patient,
            row.get("image_orientation_patient").and_then(Value::as_str),
        );
        increment_map(
            &mut self.image_positions_patient,
            row.get("image_position_patient").and_then(Value::as_str),
        );
        increment_map(
            &mut self.slice_thicknesses,
            row.get("slice_thickness").and_then(Value::as_str),
        );
        increment_map(
            &mut self.spacing_between_slices,
            row.get("spacing_between_slices").and_then(Value::as_str),
        );
        increment_map(
            &mut self.slice_locations,
            row.get("slice_location").and_then(Value::as_str),
        );
        increment_map(
            &mut self.object_types,
            row.get("object_type").and_then(Value::as_str),
        );
        let derived_reference_state =
            row.get("derived_refs")
                .and_then(Value::as_array)
                .map(|derived_refs| {
                    if derived_refs.is_empty() {
                        "without_source_reference"
                    } else {
                        "with_source_reference"
                    }
                });
        increment_map(&mut self.derived_reference_states, derived_reference_state);
        if let Some(relationships) = row
            .get("derived_reference_relationships")
            .and_then(Value::as_array)
        {
            for relationship in relationships {
                increment_map(
                    &mut self.derived_reference_relationships,
                    relationship.as_str(),
                );
            }
        }
        if let Some(targets) = row
            .get("derived_reference_targets")
            .and_then(Value::as_array)
        {
            for target in targets {
                increment_map(&mut self.derived_reference_targets, target.as_str());
            }
        }
        if let Some(sop_class_uids) = row
            .get("derived_reference_sop_class_uids")
            .and_then(Value::as_array)
        {
            for sop_class_uid in sop_class_uids {
                increment_map(
                    &mut self.derived_reference_sop_class_uids,
                    sop_class_uid.as_str(),
                );
            }
        }
        if let Some(sop_instance_uid_roots) = row
            .get("derived_reference_sop_instance_uid_roots")
            .and_then(Value::as_array)
        {
            for sop_instance_uid_root in sop_instance_uid_roots {
                increment_map(
                    &mut self.derived_reference_sop_instance_uid_roots,
                    sop_instance_uid_root.as_str(),
                );
            }
        }
        increment_map(
            &mut self.synthetic_data,
            row.get("synthetic_data").and_then(Value::as_str),
        );
        increment_map(
            &mut self.image_types,
            row.get("image_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.conversion_types,
            row.get("conversion_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.presentation_lut_shapes,
            row.get("presentation_lut_shape").and_then(Value::as_str),
        );
        increment_map(
            &mut self.window_centers,
            row.get("window_center").and_then(Value::as_str),
        );
        increment_map(
            &mut self.window_widths,
            row.get("window_width").and_then(Value::as_str),
        );
        increment_map(&mut self.kvps, row.get("kvp").and_then(Value::as_str));
        increment_map(
            &mut self.ct_acquisition_numbers,
            row.get("ct_acquisition_number").and_then(Value::as_str),
        );
        increment_map(
            &mut self.ct_rescale_intercepts,
            row.get("ct_rescale_intercept").and_then(Value::as_str),
        );
        increment_map(
            &mut self.ct_rescale_slopes,
            row.get("ct_rescale_slope").and_then(Value::as_str),
        );
        increment_map(
            &mut self.ct_rescale_types,
            row.get("ct_rescale_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_ct_dimension_index_values,
            row.get("enhanced_ct_dimension_index_values")
                .and_then(Value::as_str),
        );
        if let Some(value) = row
            .get("enhanced_ct_in_concatenation_number")
            .and_then(Value::as_u64)
        {
            *self
                .enhanced_ct_in_concatenation_numbers
                .entry(value.to_string())
                .or_default() += 1;
        }
        if let Some(value) = row
            .get("enhanced_ct_in_concatenation_total_number")
            .and_then(Value::as_u64)
        {
            *self
                .enhanced_ct_in_concatenation_total_numbers
                .entry(value.to_string())
                .or_default() += 1;
        }
        if let Some(value) = row
            .get("enhanced_ct_concatenation_frame_offset_number")
            .and_then(Value::as_u64)
        {
            *self
                .enhanced_ct_concatenation_frame_offset_numbers
                .entry(value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.mr_scanning_sequences,
            row.get("mr_scanning_sequence").and_then(Value::as_str),
        );
        increment_map(
            &mut self.mr_sequence_variants,
            row.get("mr_sequence_variant").and_then(Value::as_str),
        );
        increment_map(
            &mut self.mr_acquisition_types,
            row.get("mr_acquisition_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.mr_repetition_times,
            row.get("mr_repetition_time").and_then(Value::as_str),
        );
        increment_map(
            &mut self.mr_echo_times,
            row.get("mr_echo_time").and_then(Value::as_str),
        );
        increment_map(
            &mut self.mr_echo_train_lengths,
            row.get("mr_echo_train_length").and_then(Value::as_str),
        );
        increment_map(
            &mut self.mr_magnetic_field_strengths,
            row.get("mr_magnetic_field_strength")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_effective_echo_times,
            row.get("enhanced_mr_effective_echo_times")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_temporal_position_time_offsets,
            row.get("enhanced_mr_temporal_position_time_offsets")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_temporal_position_indices,
            row.get("enhanced_mr_temporal_position_indices")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_dimension_index_values,
            row.get("enhanced_mr_dimension_index_values")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_frame_acquisition_numbers,
            row.get("enhanced_mr_frame_acquisition_numbers")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_dimension_index_pointers,
            row.get("enhanced_mr_dimension_index_pointer")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_functional_group_pointers,
            row.get("enhanced_mr_functional_group_pointer")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_temporal_position_time_offset_units,
            row.get("enhanced_mr_temporal_position_time_offset_unit")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_velocity_encoding_minimum_values,
            row.get("enhanced_mr_velocity_encoding_minimum_value")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.enhanced_mr_velocity_encoding_maximum_values,
            row.get("enhanced_mr_velocity_encoding_maximum_value")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.segmentation_types,
            row.get("segmentation_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.segmentation_fractional_types,
            row.get("segmentation_fractional_type")
                .and_then(Value::as_str),
        );
        if let Some(value) = row
            .get("segmentation_maximum_fractional_value")
            .and_then(Value::as_u64)
        {
            *self
                .segmentation_maximum_fractional_values
                .entry(value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.gsps_content_labels,
            row.get("gsps_content_label").and_then(Value::as_str),
        );
        increment_map(
            &mut self.gsps_content_descriptions,
            row.get("gsps_content_description").and_then(Value::as_str),
        );
        increment_map(
            &mut self.gsps_presentation_size_modes,
            row.get("gsps_presentation_size_mode")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.gsps_presentation_pixel_aspect_ratios,
            row.get("gsps_presentation_pixel_aspect_ratio")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.gsps_window_centers,
            row.get("gsps_window_center").and_then(Value::as_str),
        );
        increment_map(
            &mut self.gsps_window_widths,
            row.get("gsps_window_width").and_then(Value::as_str),
        );
        increment_map(
            &mut self.gsps_presentation_lut_shapes,
            row.get("gsps_presentation_lut_shape")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_content_labels,
            row.get("rwvm_content_label").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_lut_labels,
            row.get("rwvm_lut_label").and_then(Value::as_str),
        );
        if let Some(value) = row.get("rwvm_first_value_mapped").and_then(Value::as_u64) {
            *self
                .rwvm_first_values_mapped
                .entry(value.to_string())
                .or_default() += 1;
        }
        if let Some(value) = row.get("rwvm_last_value_mapped").and_then(Value::as_u64) {
            *self
                .rwvm_last_values_mapped
                .entry(value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.rwvm_intercepts,
            row.get("rwvm_intercept").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_slopes,
            row.get("rwvm_slope").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_units_code_values,
            row.get("rwvm_units_code_value").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_units_coding_scheme_designators,
            row.get("rwvm_units_coding_scheme_designator")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_units_code_meanings,
            row.get("rwvm_units_code_meaning").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rwvm_referenced_frame_numbers,
            row.get("rwvm_referenced_frame_numbers")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_dose_units,
            row.get("rt_dose_units").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_dose_types,
            row.get("rt_dose_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_dose_summation_types,
            row.get("rt_dose_summation_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_dose_grid_scalings,
            row.get("rt_dose_grid_scaling").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_structure_set_labels,
            row.get("rt_structure_set_label").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_structure_set_roi_names,
            row.get("rt_structure_set_roi_name").and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_roi_generation_algorithms,
            row.get("rt_roi_generation_algorithm")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.rt_contour_geometric_types,
            row.get("rt_contour_geometric_type").and_then(Value::as_str),
        );
        if let Some(value) = row.get("rt_contour_points").and_then(Value::as_u64) {
            *self.rt_contour_points.entry(value.to_string()).or_default() += 1;
        }
        increment_map(
            &mut self.rt_roi_interpreted_types,
            row.get("rt_roi_interpreted_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.encapsulated_document_burned_in_annotations,
            row.get("encapsulated_document_burned_in_annotation")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.encapsulated_document_recognizable_visual_features,
            row.get("encapsulated_document_recognizable_visual_features")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.encapsulated_document_titles,
            row.get("encapsulated_document_title")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.encapsulated_document_mime_types,
            row.get("encapsulated_document_mime_type")
                .and_then(Value::as_str),
        );
        if let Some(length) = row
            .get("encapsulated_document_length")
            .and_then(Value::as_u64)
        {
            *self
                .encapsulated_document_lengths
                .entry(length.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.sr_completion_flags,
            row.get("sr_completion_flag").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sr_verification_flags,
            row.get("sr_verification_flag").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sr_root_value_types,
            row.get("sr_root_value_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sr_root_continuity_of_content,
            row.get("sr_root_continuity_of_content")
                .and_then(Value::as_str),
        );
        if let Some(count) = row.get("sr_content_sequence_items").and_then(Value::as_u64) {
            *self
                .sr_content_sequence_item_counts
                .entry(count.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.sr_observation_texts,
            row.get("sr_observation_text").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sr_measurement_numeric_values,
            row.get("sr_measurement_numeric_value")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.kos_document_titles,
            row.get("kos_document_title").and_then(Value::as_str),
        );
        if let Some(count) = row.get("kos_key_object_count").and_then(Value::as_u64) {
            *self
                .kos_key_object_counts
                .entry(count.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.kos_key_object_relationship_types,
            row.get("kos_key_object_relationship_types")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.kos_key_object_value_types,
            row.get("kos_key_object_value_types")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.kos_referenced_frame_numbers,
            row.get("kos_referenced_frame_numbers")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.modality_lut_descriptors,
            row.get("modality_lut_descriptor").and_then(Value::as_str),
        );
        increment_map(
            &mut self.modality_lut_types,
            row.get("modality_lut_type").and_then(Value::as_str),
        );
        if let Some(length) = row
            .get("modality_lut_data_value_length")
            .and_then(Value::as_u64)
        {
            *self
                .modality_lut_data_value_lengths
                .entry(length.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.voi_lut_descriptors,
            row.get("voi_lut_descriptor").and_then(Value::as_str),
        );
        if let Some(length) = row.get("voi_lut_data_value_length").and_then(Value::as_u64) {
            *self
                .voi_lut_data_value_lengths
                .entry(length.to_string())
                .or_default() += 1;
        }
        if let Some(geometry) = overlay_geometry_bucket(row) {
            *self.overlay_geometries.entry(geometry).or_default() += 1;
        }
        increment_map(
            &mut self.overlay_types,
            row.get("overlay_type").and_then(Value::as_str),
        );
        increment_map(
            &mut self.overlay_origins,
            row.get("overlay_origin").and_then(Value::as_str),
        );
        if let Some(bits_allocated) = row.get("overlay_bits_allocated").and_then(Value::as_u64) {
            *self
                .overlay_bits_allocated
                .entry(bits_allocated.to_string())
                .or_default() += 1;
        }
        if let Some(bit_position) = row.get("overlay_bit_position").and_then(Value::as_u64) {
            *self
                .overlay_bit_positions
                .entry(bit_position.to_string())
                .or_default() += 1;
        }
        if let Some(length) = row.get("overlay_data_value_length").and_then(Value::as_u64) {
            *self
                .overlay_data_value_lengths
                .entry(length.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.display_shutter_shapes,
            row.get("display_shutter_shape").and_then(Value::as_str),
        );
        if let Some(presentation_value) = row
            .get("display_shutter_presentation_value")
            .and_then(Value::as_u64)
        {
            *self
                .display_shutter_presentation_values
                .entry(presentation_value.to_string())
                .or_default() += 1;
        }
        increment_map(
            &mut self.body_parts_examined,
            row.get("body_part_examined").and_then(Value::as_str),
        );
        increment_map(
            &mut self.view_positions,
            row.get("view_position").and_then(Value::as_str),
        );
        increment_map(
            &mut self.study_instance_uid_roots,
            row.get("study_instance_uid_root").and_then(Value::as_str),
        );
        increment_map(
            &mut self.series_instance_uid_roots,
            row.get("series_instance_uid_root").and_then(Value::as_str),
        );
        increment_map(
            &mut self.sop_instance_uid_roots,
            row.get("sop_instance_uid_root").and_then(Value::as_str),
        );
        increment_map(
            &mut self.lossy_image_compression,
            row.get("lossy_image_compression").and_then(Value::as_str),
        );
        increment_map(
            &mut self.lossy_image_compression_ratios,
            row.get("lossy_image_compression_ratio")
                .and_then(Value::as_str),
        );
        increment_map(
            &mut self.lossy_image_compression_methods,
            row.get("lossy_image_compression_method")
                .and_then(Value::as_str),
        );
        if let Some(stressors) = row.get("known_stressors").and_then(Value::as_array) {
            for stressor in stressors {
                increment_map(&mut self.known_stressors, stressor.as_str());
            }
        }
    }

    fn to_json(&self) -> Value {
        let mut grouped = serde_json::json!({
            "profiles": self.profiles,
            "profile_memberships": self.profile_memberships,
            "statuses": self.statuses,
            "iods": self.iods,
            "modalities": self.modalities,
            "sop_classes": self.sop_classes,
            "sop_class_names": self.sop_class_names,
            "transfer_syntaxes": self.transfer_syntaxes,
            "transfer_syntax_names": self.transfer_syntax_names,
            "codec_families": self.codec_families,
            "codec_backends": self.codec_backends,
            "codec_backend_kinds": self.codec_backend_kinds,
            "codec_feature_gates": self.codec_feature_gates,
            "determinism": self.determinism,
            "validation_statuses": self.validation_statuses,
            "unavailable_reasons": self.unavailable_reasons,
            "photometric_interpretations": self.photometric_interpretations,
            "bit_depths": self.bit_depths,
            "bits_allocated": self.bits_allocated,
            "bits_stored": self.bits_stored,
            "high_bits": self.high_bits,
            "pixel_representations": self.pixel_representations,
            "samples_per_pixel": self.samples_per_pixel,
            "planar_configurations": self.planar_configurations,
            "pixel_data_vrs": self.pixel_data_vrs,
            "pixel_data_layouts": self.pixel_data_layouts,
            "basic_offset_tables": self.basic_offset_tables,
            "encapsulated_fragment_layouts": self.encapsulated_fragment_layouts,
            "extended_offset_tables": self.extended_offset_tables,
            "frame_counts": self.frame_counts,
            "geometries": self.geometries,
            "object_types": self.object_types,
            "derived_reference_states": self.derived_reference_states,
            "synthetic_data": self.synthetic_data,
            "image_types": self.image_types,
            "conversion_types": self.conversion_types,
            "presentation_lut_shapes": self.presentation_lut_shapes,
            "lossy_image_compression": self.lossy_image_compression,
            "lossy_image_compression_ratios": self.lossy_image_compression_ratios,
            "lossy_image_compression_methods": self.lossy_image_compression_methods,
            "known_stressors": self.known_stressors
        });
        let grouped_object = grouped
            .as_object_mut()
            .expect("grouped coverage literal must be an object");
        for (field, map) in [
            ("u32_stored_value_sets", &self.u32_stored_value_sets),
            (
                "u32_pixel_data_sha256_values",
                &self.u32_pixel_data_sha256_values,
            ),
            ("u32_word_byte_orders", &self.u32_word_byte_orders),
            (
                "u32_full_unsigned_range_states",
                &self.u32_full_unsigned_range_states,
            ),
            ("u1_stored_value_sets", &self.u1_stored_value_sets),
            (
                "u1_pixel_data_sha256_values",
                &self.u1_pixel_data_sha256_values,
            ),
            ("u1_packing_orders", &self.u1_packing_orders),
            (
                "u1_frame_boundary_policies",
                &self.u1_frame_boundary_policies,
            ),
            (
                "u1_value_field_padding_byte_counts",
                &self.u1_value_field_padding_byte_counts,
            ),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("u32 coverage count map must serialize"),
            );
        }
        grouped_object.insert(
            "generation_backends".to_string(),
            serde_json::to_value(&self.generation_backends)
                .expect("generation backend count map must serialize"),
        );
        grouped_object.insert(
            "metadata_specific_character_sets".to_string(),
            serde_json::to_value(&self.metadata_specific_character_sets)
                .expect("metadata Specific Character Set count map must serialize"),
        );
        grouped_object.insert(
            "metadata_person_names".to_string(),
            serde_json::to_value(&self.metadata_person_names)
                .expect("metadata Person Name count map must serialize"),
        );
        grouped_object.insert(
            "metadata_person_name_component_groups".to_string(),
            serde_json::to_value(&self.metadata_person_name_component_groups)
                .expect("metadata Person Name component group count map must serialize"),
        );
        grouped_object.insert(
            "metadata_person_name_component_group_counts".to_string(),
            serde_json::to_value(&self.metadata_person_name_component_group_counts)
                .expect("metadata Person Name component group total map must serialize"),
        );
        grouped_object.insert(
            "metadata_person_name_encoded_sha256_values".to_string(),
            serde_json::to_value(&self.metadata_person_name_encoded_sha256_values)
                .expect("metadata Person Name encoded SHA-256 count map must serialize"),
        );
        grouped_object.insert(
            "metadata_person_name_encoded_length_bytes".to_string(),
            serde_json::to_value(&self.metadata_person_name_encoded_length_bytes)
                .expect("metadata Person Name encoded byte length count map must serialize"),
        );
        grouped_object.insert(
            "metadata_temporal_boundary_ids".to_string(),
            serde_json::to_value(&self.metadata_temporal_boundary_ids)
                .expect("metadata temporal boundary ID count map must serialize"),
        );
        grouped_object.insert(
            "metadata_timezone_offsets_from_utc".to_string(),
            serde_json::to_value(&self.metadata_timezone_offsets_from_utc)
                .expect("metadata timezone offset count map must serialize"),
        );
        grouped_object.insert(
            "metadata_empty_type2_attributes".to_string(),
            serde_json::to_value(&self.metadata_empty_type2_attributes)
                .expect("metadata empty Type 2 attribute count map must serialize"),
        );
        grouped_object.insert(
            "metadata_empty_type2_attribute_counts".to_string(),
            serde_json::to_value(&self.metadata_empty_type2_attribute_counts)
                .expect("metadata empty Type 2 attribute total map must serialize"),
        );
        for (field, value) in [
            (
                "metadata_string_tags",
                serde_json::to_value(&self.metadata_string_tags),
            ),
            (
                "metadata_string_vrs",
                serde_json::to_value(&self.metadata_string_vrs),
            ),
            (
                "metadata_string_value_multiplicities",
                serde_json::to_value(&self.metadata_string_value_multiplicities),
            ),
            (
                "metadata_string_max_component_encoded_length_bytes",
                serde_json::to_value(&self.metadata_string_max_component_encoded_length_bytes),
            ),
            (
                "metadata_string_raw_value_lengths",
                serde_json::to_value(&self.metadata_string_raw_value_lengths),
            ),
            (
                "metadata_string_raw_sha256_values",
                serde_json::to_value(&self.metadata_string_raw_sha256_values),
            ),
            (
                "metadata_private_creator_tags",
                serde_json::to_value(&self.metadata_private_creator_tags),
            ),
            (
                "metadata_private_creator_ids",
                serde_json::to_value(&self.metadata_private_creator_ids),
            ),
            (
                "metadata_private_block_ranges",
                serde_json::to_value(&self.metadata_private_block_ranges),
            ),
            (
                "metadata_private_creator_raw_sha256_values",
                serde_json::to_value(&self.metadata_private_creator_raw_sha256_values),
            ),
            (
                "metadata_private_element_tags",
                serde_json::to_value(&self.metadata_private_element_tags),
            ),
            (
                "metadata_private_element_vrs",
                serde_json::to_value(&self.metadata_private_element_vrs),
            ),
            (
                "metadata_private_element_raw_sha256_values",
                serde_json::to_value(&self.metadata_private_element_raw_sha256_values),
            ),
            (
                "metadata_sequence_length_variants",
                serde_json::to_value(&self.metadata_sequence_length_variants),
            ),
            (
                "metadata_sequence_length_field_hex_values",
                serde_json::to_value(&self.metadata_sequence_length_field_hex_values),
            ),
            (
                "metadata_sequence_delimitation_states",
                serde_json::to_value(&self.metadata_sequence_delimitation_states),
            ),
            (
                "metadata_sequence_item_length_encodings",
                serde_json::to_value(&self.metadata_sequence_item_length_encodings),
            ),
        ] {
            grouped_object.insert(
                field.to_string(),
                value.expect("metadata string boundary count map must serialize"),
            );
        }
        grouped_object.insert(
            "window_centers".to_string(),
            serde_json::to_value(&self.window_centers)
                .expect("window center count map must serialize"),
        );
        grouped_object.insert(
            "derived_reference_relationships".to_string(),
            serde_json::to_value(&self.derived_reference_relationships)
                .expect("derived reference relationship count map must serialize"),
        );
        grouped_object.insert(
            "derived_reference_targets".to_string(),
            serde_json::to_value(&self.derived_reference_targets)
                .expect("derived reference target count map must serialize"),
        );
        grouped_object.insert(
            "derived_reference_sop_class_uids".to_string(),
            serde_json::to_value(&self.derived_reference_sop_class_uids)
                .expect("derived reference SOP Class UID count map must serialize"),
        );
        grouped_object.insert(
            "derived_reference_sop_instance_uid_roots".to_string(),
            serde_json::to_value(&self.derived_reference_sop_instance_uid_roots)
                .expect("derived reference SOP Instance UID root count map must serialize"),
        );
        grouped_object.insert(
            "window_widths".to_string(),
            serde_json::to_value(&self.window_widths)
                .expect("window width count map must serialize"),
        );
        grouped_object.insert(
            "kvps".to_string(),
            serde_json::to_value(&self.kvps).expect("KVP count map must serialize"),
        );
        grouped_object.insert(
            "ct_acquisition_numbers".to_string(),
            serde_json::to_value(&self.ct_acquisition_numbers)
                .expect("CT acquisition number count map must serialize"),
        );
        grouped_object.insert(
            "ct_rescale_intercepts".to_string(),
            serde_json::to_value(&self.ct_rescale_intercepts)
                .expect("CT rescale intercept count map must serialize"),
        );
        grouped_object.insert(
            "ct_rescale_slopes".to_string(),
            serde_json::to_value(&self.ct_rescale_slopes)
                .expect("CT rescale slope count map must serialize"),
        );
        grouped_object.insert(
            "ct_rescale_types".to_string(),
            serde_json::to_value(&self.ct_rescale_types)
                .expect("CT rescale type count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_ct_dimension_index_values".to_string(),
            serde_json::to_value(&self.enhanced_ct_dimension_index_values)
                .expect("Enhanced CT dimension index value count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_ct_in_concatenation_numbers".to_string(),
            serde_json::to_value(&self.enhanced_ct_in_concatenation_numbers)
                .expect("Enhanced CT in-concatenation number count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_ct_in_concatenation_total_numbers".to_string(),
            serde_json::to_value(&self.enhanced_ct_in_concatenation_total_numbers)
                .expect("Enhanced CT in-concatenation total number count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_ct_concatenation_frame_offset_numbers".to_string(),
            serde_json::to_value(&self.enhanced_ct_concatenation_frame_offset_numbers)
                .expect("Enhanced CT concatenation frame offset number count map must serialize"),
        );
        for (field, map) in [
            (
                "nm_frame_increment_pointers",
                &self.nm_frame_increment_pointers,
            ),
            ("nm_energy_window_vectors", &self.nm_energy_window_vectors),
            ("nm_detector_vectors", &self.nm_detector_vectors),
            ("nm_energy_window_names", &self.nm_energy_window_names),
            (
                "nm_detector_start_angles_degrees",
                &self.nm_detector_start_angles_degrees,
            ),
            ("nm_frame_dimension_tuples", &self.nm_frame_dimension_tuples),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("NM coverage count map must serialize"),
            );
        }
        for (field, map) in [
            ("enhanced_pet_image_types", &self.enhanced_pet_image_types),
            ("enhanced_pet_frame_types", &self.enhanced_pet_frame_types),
            ("enhanced_pet_view_codes", &self.enhanced_pet_view_codes),
            (
                "enhanced_pet_view_modifier_item_counts",
                &self.enhanced_pet_view_modifier_item_counts,
            ),
            (
                "enhanced_pet_slice_progression_direction_states",
                &self.enhanced_pet_slice_progression_direction_states,
            ),
            ("enhanced_pet_stack_ids", &self.enhanced_pet_stack_ids),
            (
                "enhanced_pet_in_stack_position_numbers",
                &self.enhanced_pet_in_stack_position_numbers,
            ),
            (
                "enhanced_pet_dimension_index_values",
                &self.enhanced_pet_dimension_index_values,
            ),
            (
                "enhanced_pet_temporal_position_indices",
                &self.enhanced_pet_temporal_position_indices,
            ),
            (
                "enhanced_pet_image_positions_patient_mm",
                &self.enhanced_pet_image_positions_patient_mm,
            ),
            (
                "enhanced_pet_stored_values_by_frame",
                &self.enhanced_pet_stored_values_by_frame,
            ),
            (
                "enhanced_pet_activity_values_bqml_by_frame",
                &self.enhanced_pet_activity_values_bqml_by_frame,
            ),
            (
                "enhanced_pet_rwvm_intercepts",
                &self.enhanced_pet_rwvm_intercepts,
            ),
            ("enhanced_pet_rwvm_slopes", &self.enhanced_pet_rwvm_slopes),
            (
                "enhanced_pet_rwvm_measurement_units",
                &self.enhanced_pet_rwvm_measurement_units,
            ),
            ("enhanced_pet_corrections", &self.enhanced_pet_corrections),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("Enhanced PET coverage count map must serialize"),
            );
        }
        for (field, map) in [
            ("pet_units", &self.pet_units),
            ("pet_counts_sources", &self.pet_counts_sources),
            ("pet_series_types", &self.pet_series_types),
            ("pet_corrected_images", &self.pet_corrected_images),
            ("pet_decay_corrections", &self.pet_decay_corrections),
            (
                "pet_dose_calibration_factors",
                &self.pet_dose_calibration_factors,
            ),
            ("pet_rescale_intercepts", &self.pet_rescale_intercepts),
            ("pet_rescale_slopes", &self.pet_rescale_slopes),
            ("pet_stored_values", &self.pet_stored_values),
            ("pet_activity_values_bqml", &self.pet_activity_values_bqml),
            (
                "pet_frame_reference_times_ms",
                &self.pet_frame_reference_times_ms,
            ),
            (
                "pet_actual_frame_durations_ms",
                &self.pet_actual_frame_durations_ms,
            ),
            ("pet_image_indices", &self.pet_image_indices),
            (
                "pet_radiopharmaceutical_information_item_counts",
                &self.pet_radiopharmaceutical_information_item_counts,
            ),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("PET coverage count map must serialize"),
            );
        }
        for (field, map) in [
            ("us_image_types", &self.us_image_types),
            (
                "us_frame_increment_pointers",
                &self.us_frame_increment_pointers,
            ),
            ("us_frame_times_ms", &self.us_frame_times_ms),
            ("us_frame_counts", &self.us_frame_counts),
            (
                "us_spatially_related_frames",
                &self.us_spatially_related_frames,
            ),
            ("us_color_data_present", &self.us_color_data_present),
            ("us_region_calibrated", &self.us_region_calibrated),
            (
                "us_lossy_image_compressions",
                &self.us_lossy_image_compressions,
            ),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("US coverage count map must serialize"),
            );
        }
        for (field, map) in [
            ("xa_image_types", &self.xa_image_types),
            ("xa_frame_counts", &self.xa_frame_counts),
            ("xa_body_parts_examined", &self.xa_body_parts_examined),
            (
                "xa_patient_orientation_empty_states",
                &self.xa_patient_orientation_empty_states,
            ),
            (
                "xa_laterality_present_states",
                &self.xa_laterality_present_states,
            ),
            (
                "xa_pixel_intensity_relationships",
                &self.xa_pixel_intensity_relationships,
            ),
            ("xa_radiation_settings", &self.xa_radiation_settings),
            ("xa_kvps", &self.xa_kvps),
            ("xa_exposures_mas", &self.xa_exposures_mas),
            (
                "xa_imager_pixel_spacings_mm",
                &self.xa_imager_pixel_spacings_mm,
            ),
            (
                "xa_positioner_primary_angles_degrees",
                &self.xa_positioner_primary_angles_degrees,
            ),
            (
                "xa_positioner_secondary_angles_degrees",
                &self.xa_positioner_secondary_angles_degrees,
            ),
            (
                "xa_distances_source_to_detector_mm",
                &self.xa_distances_source_to_detector_mm,
            ),
            (
                "xa_distances_source_to_patient_mm",
                &self.xa_distances_source_to_patient_mm,
            ),
            (
                "xa_estimated_radiographic_magnification_factors",
                &self.xa_estimated_radiographic_magnification_factors,
            ),
            (
                "xa_lossy_image_compressions",
                &self.xa_lossy_image_compressions,
            ),
            ("xa_multiframe_cine_states", &self.xa_multiframe_cine_states),
            (
                "xa_biplane_data_present_states",
                &self.xa_biplane_data_present_states,
            ),
            ("xa_contrast_used_states", &self.xa_contrast_used_states),
            (
                "xa_subtraction_applied_states",
                &self.xa_subtraction_applied_states,
            ),
            (
                "xa_table_motion_present_states",
                &self.xa_table_motion_present_states,
            ),
            (
                "xa_patient_space_geometry_present_states",
                &self.xa_patient_space_geometry_present_states,
            ),
            (
                "xa_pixel_spacing_calibrated_states",
                &self.xa_pixel_spacing_calibrated_states,
            ),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("XA coverage count map must serialize"),
            );
        }
        for (field, map) in [
            ("xrf_image_types", &self.xrf_image_types),
            ("xrf_frame_counts", &self.xrf_frame_counts),
            ("xrf_body_parts_examined", &self.xrf_body_parts_examined),
            (
                "xrf_patient_orientation_empty_states",
                &self.xrf_patient_orientation_empty_states,
            ),
            (
                "xrf_laterality_present_states",
                &self.xrf_laterality_present_states,
            ),
            (
                "xrf_pixel_intensity_relationships",
                &self.xrf_pixel_intensity_relationships,
            ),
            ("xrf_radiation_settings", &self.xrf_radiation_settings),
            ("xrf_kvps", &self.xrf_kvps),
            ("xrf_exposures_mas", &self.xrf_exposures_mas),
            (
                "xrf_imager_pixel_spacings_mm",
                &self.xrf_imager_pixel_spacings_mm,
            ),
            (
                "xrf_distances_source_to_detector_mm",
                &self.xrf_distances_source_to_detector_mm,
            ),
            (
                "xrf_distances_source_to_patient_mm",
                &self.xrf_distances_source_to_patient_mm,
            ),
            (
                "xrf_estimated_radiographic_magnification_factors",
                &self.xrf_estimated_radiographic_magnification_factors,
            ),
            (
                "xrf_column_angulations_degrees",
                &self.xrf_column_angulations_degrees,
            ),
            (
                "xrf_lossy_image_compressions",
                &self.xrf_lossy_image_compressions,
            ),
            (
                "xrf_multiframe_cine_states",
                &self.xrf_multiframe_cine_states,
            ),
            (
                "xrf_biplane_data_present_states",
                &self.xrf_biplane_data_present_states,
            ),
            ("xrf_contrast_used_states", &self.xrf_contrast_used_states),
            (
                "xrf_subtraction_applied_states",
                &self.xrf_subtraction_applied_states,
            ),
            (
                "xrf_table_position_present_states",
                &self.xrf_table_position_present_states,
            ),
            (
                "xrf_table_motion_present_states",
                &self.xrf_table_motion_present_states,
            ),
            (
                "xrf_table_tilt_present_states",
                &self.xrf_table_tilt_present_states,
            ),
            (
                "xrf_tomography_present_states",
                &self.xrf_tomography_present_states,
            ),
            (
                "xrf_patient_space_geometry_present_states",
                &self.xrf_patient_space_geometry_present_states,
            ),
            (
                "xrf_pixel_spacing_calibrated_states",
                &self.xrf_pixel_spacing_calibrated_states,
            ),
            (
                "xrf_xa_positioner_angles_present_states",
                &self.xrf_xa_positioner_angles_present_states,
            ),
        ] {
            grouped_object.insert(
                field.to_string(),
                serde_json::to_value(map).expect("XRF coverage count map must serialize"),
            );
        }
        grouped_object.insert(
            "pixel_spacings".to_string(),
            serde_json::to_value(&self.pixel_spacings)
                .expect("pixel spacing count map must serialize"),
        );
        grouped_object.insert(
            "imager_pixel_spacings".to_string(),
            serde_json::to_value(&self.imager_pixel_spacings)
                .expect("imager pixel spacing count map must serialize"),
        );
        grouped_object.insert(
            "image_orientations_patient".to_string(),
            serde_json::to_value(&self.image_orientations_patient)
                .expect("image orientation patient count map must serialize"),
        );
        grouped_object.insert(
            "image_positions_patient".to_string(),
            serde_json::to_value(&self.image_positions_patient)
                .expect("image position patient count map must serialize"),
        );
        grouped_object.insert(
            "slice_thicknesses".to_string(),
            serde_json::to_value(&self.slice_thicknesses)
                .expect("slice thickness count map must serialize"),
        );
        grouped_object.insert(
            "spacing_between_slices".to_string(),
            serde_json::to_value(&self.spacing_between_slices)
                .expect("spacing between slices count map must serialize"),
        );
        grouped_object.insert(
            "slice_locations".to_string(),
            serde_json::to_value(&self.slice_locations)
                .expect("slice location count map must serialize"),
        );
        grouped_object.insert(
            "mr_scanning_sequences".to_string(),
            serde_json::to_value(&self.mr_scanning_sequences)
                .expect("MR scanning sequence count map must serialize"),
        );
        grouped_object.insert(
            "mr_sequence_variants".to_string(),
            serde_json::to_value(&self.mr_sequence_variants)
                .expect("MR sequence variant count map must serialize"),
        );
        grouped_object.insert(
            "mr_acquisition_types".to_string(),
            serde_json::to_value(&self.mr_acquisition_types)
                .expect("MR acquisition type count map must serialize"),
        );
        grouped_object.insert(
            "mr_repetition_times".to_string(),
            serde_json::to_value(&self.mr_repetition_times)
                .expect("MR repetition time count map must serialize"),
        );
        grouped_object.insert(
            "mr_echo_times".to_string(),
            serde_json::to_value(&self.mr_echo_times)
                .expect("MR echo time count map must serialize"),
        );
        grouped_object.insert(
            "mr_echo_train_lengths".to_string(),
            serde_json::to_value(&self.mr_echo_train_lengths)
                .expect("MR echo train length count map must serialize"),
        );
        grouped_object.insert(
            "mr_magnetic_field_strengths".to_string(),
            serde_json::to_value(&self.mr_magnetic_field_strengths)
                .expect("MR magnetic field strength count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_effective_echo_times".to_string(),
            serde_json::to_value(&self.enhanced_mr_effective_echo_times)
                .expect("Enhanced MR effective echo time count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_temporal_position_time_offsets".to_string(),
            serde_json::to_value(&self.enhanced_mr_temporal_position_time_offsets)
                .expect("Enhanced MR temporal position time offset count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_temporal_position_indices".to_string(),
            serde_json::to_value(&self.enhanced_mr_temporal_position_indices)
                .expect("Enhanced MR temporal position index count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_dimension_index_values".to_string(),
            serde_json::to_value(&self.enhanced_mr_dimension_index_values)
                .expect("Enhanced MR dimension index value count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_frame_acquisition_numbers".to_string(),
            serde_json::to_value(&self.enhanced_mr_frame_acquisition_numbers)
                .expect("Enhanced MR frame acquisition number count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_dimension_index_pointers".to_string(),
            serde_json::to_value(&self.enhanced_mr_dimension_index_pointers)
                .expect("Enhanced MR dimension index pointer count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_functional_group_pointers".to_string(),
            serde_json::to_value(&self.enhanced_mr_functional_group_pointers)
                .expect("Enhanced MR functional group pointer count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_temporal_position_time_offset_units".to_string(),
            serde_json::to_value(&self.enhanced_mr_temporal_position_time_offset_units)
                .expect("Enhanced MR temporal position time offset unit count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_velocity_encoding_minimum_values".to_string(),
            serde_json::to_value(&self.enhanced_mr_velocity_encoding_minimum_values)
                .expect("Enhanced MR velocity encoding minimum value count map must serialize"),
        );
        grouped_object.insert(
            "enhanced_mr_velocity_encoding_maximum_values".to_string(),
            serde_json::to_value(&self.enhanced_mr_velocity_encoding_maximum_values)
                .expect("Enhanced MR velocity encoding maximum value count map must serialize"),
        );
        grouped_object.insert(
            "segmentation_types".to_string(),
            serde_json::to_value(&self.segmentation_types)
                .expect("segmentation type count map must serialize"),
        );
        grouped_object.insert(
            "segmentation_fractional_types".to_string(),
            serde_json::to_value(&self.segmentation_fractional_types)
                .expect("segmentation fractional type count map must serialize"),
        );
        grouped_object.insert(
            "segmentation_maximum_fractional_values".to_string(),
            serde_json::to_value(&self.segmentation_maximum_fractional_values)
                .expect("segmentation maximum fractional value count map must serialize"),
        );
        grouped_object.insert(
            "gsps_content_labels".to_string(),
            serde_json::to_value(&self.gsps_content_labels)
                .expect("GSPS Content Label count map must serialize"),
        );
        grouped_object.insert(
            "gsps_content_descriptions".to_string(),
            serde_json::to_value(&self.gsps_content_descriptions)
                .expect("GSPS Content Description count map must serialize"),
        );
        grouped_object.insert(
            "gsps_presentation_size_modes".to_string(),
            serde_json::to_value(&self.gsps_presentation_size_modes)
                .expect("GSPS Presentation Size Mode count map must serialize"),
        );
        grouped_object.insert(
            "gsps_presentation_pixel_aspect_ratios".to_string(),
            serde_json::to_value(&self.gsps_presentation_pixel_aspect_ratios)
                .expect("GSPS Presentation Pixel Aspect Ratio count map must serialize"),
        );
        grouped_object.insert(
            "gsps_window_centers".to_string(),
            serde_json::to_value(&self.gsps_window_centers)
                .expect("GSPS Window Center count map must serialize"),
        );
        grouped_object.insert(
            "gsps_window_widths".to_string(),
            serde_json::to_value(&self.gsps_window_widths)
                .expect("GSPS Window Width count map must serialize"),
        );
        grouped_object.insert(
            "gsps_presentation_lut_shapes".to_string(),
            serde_json::to_value(&self.gsps_presentation_lut_shapes)
                .expect("GSPS Presentation LUT Shape count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_content_labels".to_string(),
            serde_json::to_value(&self.rwvm_content_labels)
                .expect("RWVM Content Label count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_lut_labels".to_string(),
            serde_json::to_value(&self.rwvm_lut_labels)
                .expect("RWVM LUT Label count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_first_values_mapped".to_string(),
            serde_json::to_value(&self.rwvm_first_values_mapped)
                .expect("RWVM first value mapped count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_last_values_mapped".to_string(),
            serde_json::to_value(&self.rwvm_last_values_mapped)
                .expect("RWVM last value mapped count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_intercepts".to_string(),
            serde_json::to_value(&self.rwvm_intercepts)
                .expect("RWVM intercept count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_slopes".to_string(),
            serde_json::to_value(&self.rwvm_slopes).expect("RWVM slope count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_units_code_values".to_string(),
            serde_json::to_value(&self.rwvm_units_code_values)
                .expect("RWVM units Code Value count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_units_coding_scheme_designators".to_string(),
            serde_json::to_value(&self.rwvm_units_coding_scheme_designators)
                .expect("RWVM units Coding Scheme Designator count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_units_code_meanings".to_string(),
            serde_json::to_value(&self.rwvm_units_code_meanings)
                .expect("RWVM units Code Meaning count map must serialize"),
        );
        grouped_object.insert(
            "rwvm_referenced_frame_numbers".to_string(),
            serde_json::to_value(&self.rwvm_referenced_frame_numbers)
                .expect("RWVM referenced frame number count map must serialize"),
        );
        grouped_object.insert(
            "rt_dose_units".to_string(),
            serde_json::to_value(&self.rt_dose_units)
                .expect("RT Dose Units count map must serialize"),
        );
        grouped_object.insert(
            "rt_dose_types".to_string(),
            serde_json::to_value(&self.rt_dose_types)
                .expect("RT Dose Type count map must serialize"),
        );
        grouped_object.insert(
            "rt_dose_summation_types".to_string(),
            serde_json::to_value(&self.rt_dose_summation_types)
                .expect("RT Dose Summation Type count map must serialize"),
        );
        grouped_object.insert(
            "rt_dose_grid_scalings".to_string(),
            serde_json::to_value(&self.rt_dose_grid_scalings)
                .expect("RT Dose Grid Scaling count map must serialize"),
        );
        grouped_object.insert(
            "rt_structure_set_labels".to_string(),
            serde_json::to_value(&self.rt_structure_set_labels)
                .expect("RT Structure Set Label count map must serialize"),
        );
        grouped_object.insert(
            "rt_structure_set_roi_names".to_string(),
            serde_json::to_value(&self.rt_structure_set_roi_names)
                .expect("RT Structure Set ROI Name count map must serialize"),
        );
        grouped_object.insert(
            "rt_roi_generation_algorithms".to_string(),
            serde_json::to_value(&self.rt_roi_generation_algorithms)
                .expect("RT ROI Generation Algorithm count map must serialize"),
        );
        grouped_object.insert(
            "rt_contour_geometric_types".to_string(),
            serde_json::to_value(&self.rt_contour_geometric_types)
                .expect("RT Contour Geometric Type count map must serialize"),
        );
        grouped_object.insert(
            "rt_contour_points".to_string(),
            serde_json::to_value(&self.rt_contour_points)
                .expect("RT Contour Points count map must serialize"),
        );
        grouped_object.insert(
            "rt_roi_interpreted_types".to_string(),
            serde_json::to_value(&self.rt_roi_interpreted_types)
                .expect("RT ROI Interpreted Type count map must serialize"),
        );
        grouped_object.insert(
            "encapsulated_document_burned_in_annotations".to_string(),
            serde_json::to_value(&self.encapsulated_document_burned_in_annotations)
                .expect("encapsulated document Burned In Annotation count map must serialize"),
        );
        grouped_object.insert(
            "encapsulated_document_recognizable_visual_features".to_string(),
            serde_json::to_value(&self.encapsulated_document_recognizable_visual_features).expect(
                "encapsulated document Recognizable Visual Features count map must serialize",
            ),
        );
        grouped_object.insert(
            "encapsulated_document_titles".to_string(),
            serde_json::to_value(&self.encapsulated_document_titles)
                .expect("encapsulated document title count map must serialize"),
        );
        grouped_object.insert(
            "encapsulated_document_mime_types".to_string(),
            serde_json::to_value(&self.encapsulated_document_mime_types)
                .expect("encapsulated document MIME type count map must serialize"),
        );
        grouped_object.insert(
            "encapsulated_document_lengths".to_string(),
            serde_json::to_value(&self.encapsulated_document_lengths)
                .expect("encapsulated document length count map must serialize"),
        );
        grouped_object.insert(
            "sr_completion_flags".to_string(),
            serde_json::to_value(&self.sr_completion_flags)
                .expect("SR Completion Flag count map must serialize"),
        );
        grouped_object.insert(
            "sr_verification_flags".to_string(),
            serde_json::to_value(&self.sr_verification_flags)
                .expect("SR Verification Flag count map must serialize"),
        );
        grouped_object.insert(
            "sr_root_value_types".to_string(),
            serde_json::to_value(&self.sr_root_value_types)
                .expect("SR root Value Type count map must serialize"),
        );
        grouped_object.insert(
            "sr_root_continuity_of_content".to_string(),
            serde_json::to_value(&self.sr_root_continuity_of_content)
                .expect("SR root Continuity of Content count map must serialize"),
        );
        grouped_object.insert(
            "sr_content_sequence_item_counts".to_string(),
            serde_json::to_value(&self.sr_content_sequence_item_counts)
                .expect("SR Content Sequence item count map must serialize"),
        );
        grouped_object.insert(
            "sr_observation_texts".to_string(),
            serde_json::to_value(&self.sr_observation_texts)
                .expect("SR observation text count map must serialize"),
        );
        grouped_object.insert(
            "sr_measurement_numeric_values".to_string(),
            serde_json::to_value(&self.sr_measurement_numeric_values)
                .expect("SR measurement numeric value count map must serialize"),
        );
        grouped_object.insert(
            "kos_document_titles".to_string(),
            serde_json::to_value(&self.kos_document_titles)
                .expect("KOS document title count map must serialize"),
        );
        grouped_object.insert(
            "kos_key_object_counts".to_string(),
            serde_json::to_value(&self.kos_key_object_counts)
                .expect("KOS key object count map must serialize"),
        );
        grouped_object.insert(
            "kos_key_object_relationship_types".to_string(),
            serde_json::to_value(&self.kos_key_object_relationship_types)
                .expect("KOS key object relationship type count map must serialize"),
        );
        grouped_object.insert(
            "kos_key_object_value_types".to_string(),
            serde_json::to_value(&self.kos_key_object_value_types)
                .expect("KOS key object value type count map must serialize"),
        );
        grouped_object.insert(
            "kos_referenced_frame_numbers".to_string(),
            serde_json::to_value(&self.kos_referenced_frame_numbers)
                .expect("KOS referenced frame number count map must serialize"),
        );
        grouped_object.insert(
            "modality_lut_descriptors".to_string(),
            serde_json::to_value(&self.modality_lut_descriptors)
                .expect("modality LUT descriptor count map must serialize"),
        );
        grouped_object.insert(
            "modality_lut_types".to_string(),
            serde_json::to_value(&self.modality_lut_types)
                .expect("modality LUT type count map must serialize"),
        );
        grouped_object.insert(
            "modality_lut_data_value_lengths".to_string(),
            serde_json::to_value(&self.modality_lut_data_value_lengths)
                .expect("modality LUT data value length count map must serialize"),
        );
        grouped_object.insert(
            "voi_lut_descriptors".to_string(),
            serde_json::to_value(&self.voi_lut_descriptors)
                .expect("VOI LUT descriptor count map must serialize"),
        );
        grouped_object.insert(
            "voi_lut_data_value_lengths".to_string(),
            serde_json::to_value(&self.voi_lut_data_value_lengths)
                .expect("VOI LUT data value length count map must serialize"),
        );
        grouped_object.insert(
            "overlay_geometries".to_string(),
            serde_json::to_value(&self.overlay_geometries)
                .expect("overlay geometry count map must serialize"),
        );
        grouped_object.insert(
            "overlay_types".to_string(),
            serde_json::to_value(&self.overlay_types)
                .expect("overlay type count map must serialize"),
        );
        grouped_object.insert(
            "overlay_origins".to_string(),
            serde_json::to_value(&self.overlay_origins)
                .expect("overlay origin count map must serialize"),
        );
        grouped_object.insert(
            "overlay_bits_allocated".to_string(),
            serde_json::to_value(&self.overlay_bits_allocated)
                .expect("overlay bits allocated count map must serialize"),
        );
        grouped_object.insert(
            "overlay_bit_positions".to_string(),
            serde_json::to_value(&self.overlay_bit_positions)
                .expect("overlay bit position count map must serialize"),
        );
        grouped_object.insert(
            "overlay_data_value_lengths".to_string(),
            serde_json::to_value(&self.overlay_data_value_lengths)
                .expect("overlay data value length count map must serialize"),
        );
        grouped_object.insert(
            "display_shutter_shapes".to_string(),
            serde_json::to_value(&self.display_shutter_shapes)
                .expect("display shutter shape count map must serialize"),
        );
        grouped_object.insert(
            "display_shutter_presentation_values".to_string(),
            serde_json::to_value(&self.display_shutter_presentation_values)
                .expect("display shutter presentation value count map must serialize"),
        );
        grouped_object.insert(
            "body_parts_examined".to_string(),
            serde_json::to_value(&self.body_parts_examined)
                .expect("body part examined count map must serialize"),
        );
        grouped_object.insert(
            "view_positions".to_string(),
            serde_json::to_value(&self.view_positions)
                .expect("view position count map must serialize"),
        );
        grouped_object.insert(
            "study_instance_uid_roots".to_string(),
            serde_json::to_value(&self.study_instance_uid_roots)
                .expect("study instance UID root count map must serialize"),
        );
        grouped_object.insert(
            "series_instance_uid_roots".to_string(),
            serde_json::to_value(&self.series_instance_uid_roots)
                .expect("series instance UID root count map must serialize"),
        );
        grouped_object.insert(
            "sop_instance_uid_roots".to_string(),
            serde_json::to_value(&self.sop_instance_uid_roots)
                .expect("SOP instance UID root count map must serialize"),
        );
        grouped
    }
}

fn basic_offset_table_state(file: &Value) -> Option<&'static str> {
    file.pointer("/pixel_data/encapsulated_pixel_data/basic_offset_table/populated")
        .and_then(Value::as_bool)
        .map(|populated| if populated { "populated" } else { "empty" })
}

fn encapsulated_fragment_layout(file: &Value) -> Option<&'static str> {
    let fragments_per_frame = file
        .pointer("/pixel_data/encapsulated_pixel_data/fragments_per_frame")
        .and_then(Value::as_array)?;
    if fragments_per_frame
        .iter()
        .all(|fragment_count| fragment_count.as_u64() == Some(1))
    {
        Some("single_fragment_per_frame")
    } else {
        Some("multi_fragment_per_frame")
    }
}

fn extended_offset_table_state(file: &Value) -> Option<&'static str> {
    file.pointer("/pixel_data/encapsulated_pixel_data/extended_offset_table/present")
        .and_then(Value::as_bool)
        .map(|present| if present { "present" } else { "absent" })
}

fn geometry_bucket(row: &Value) -> Option<String> {
    let geometry = row.get("geometry")?;
    let rows = geometry.get("rows").and_then(Value::as_u64)?;
    let columns = geometry.get("columns").and_then(Value::as_u64)?;
    Some(format!("{rows}x{columns}"))
}

fn overlay_geometry_bucket(row: &Value) -> Option<String> {
    let rows = row.get("overlay_rows").and_then(Value::as_u64)?;
    let columns = row.get("overlay_columns").and_then(Value::as_u64)?;
    Some(format!("{rows}x{columns}"))
}

fn uid_root_bucket(uid: &str) -> Option<&'static str> {
    uid.strip_prefix("2.25.")
        .filter(|suffix| !suffix.is_empty())
        .map(|_| "2.25")
}

fn increment_map(map: &mut BTreeMap<String, usize>, key: Option<&str>) {
    if let Some(key) = key {
        *map.entry(key.to_string()).or_default() += 1;
    }
}

fn increment_scalar_map(map: &mut BTreeMap<String, usize>, value: Option<&Value>) {
    if let Some(key) = value.and_then(report_scalar_label) {
        *map.entry(key).or_default() += 1;
    }
}

fn increment_string_array_map(map: &mut BTreeMap<String, usize>, value: Option<&Value>) {
    for key in value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        *map.entry(key.to_string()).or_default() += 1;
    }
}

fn increment_u64_array_map(map: &mut BTreeMap<String, usize>, value: Option<&Value>) {
    for key in value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
    {
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
        .and_then(Value::as_number)
        .map_or_else(String::new, |number| number.to_string())
}

fn markdown_number_list(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_number)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn markdown_value_array(value: Option<&Value>) -> String {
    let joined = value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(report_scalar_label)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    markdown_cell(Some(&joined))
}

fn markdown_bool(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string())
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
    unavailable_cases: &[Value],
) -> Result<Value, GenerateError> {
    let skipped_cases =
        skipped_cases_for_run(registry, run, generated_case_ids, unavailable_cases)?;
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
    unavailable_cases: &[Value],
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
        if let Some(unavailable) = unavailable_cases
            .iter()
            .find(|unavailable| unavailable.get("case_id").and_then(Value::as_str) == Some(case_id))
        {
            skipped.push(unavailable.clone());
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
    if let Some(profile_filter) = profile_filter {
        if !SUPPORTED_PROFILES.contains(&profile_filter) {
            return Err(CaseRegistryError::InvalidProfile(
                profile_filter.to_string(),
            ));
        }
    }
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
        "case_id\tstatus\tprofiles\tsop_class_uid\ttransfer_syntax_uid\tstandards_evidence\tartifact_kind\tprovider\tobject_family\troadmap_priority\tblocker_codes\n",
    );

    for case in cases {
        let profiles = string_array(case.get("profiles"))?;
        if let Some(profile_filter) = profile_filter {
            if !case_matches_profile(&profiles, profile_filter, false) {
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
        let sop_class_uid = case
            .get("sop_class_uid")
            .and_then(Value::as_str)
            .unwrap_or("-");
        let transfer_syntax_uid = case
            .get("transfer_syntax_uid")
            .and_then(Value::as_str)
            .unwrap_or("-");
        let artifact_kind = required_str(case, "artifact_kind")?;
        let provider = case
            .pointer("/provider/id")
            .and_then(Value::as_str)
            .ok_or(CaseRegistryError::Shape("missing provider id"))?;
        let object_family = required_str(case, "object_family")?;
        let roadmap_priority = case
            .pointer("/roadmap/priority")
            .and_then(Value::as_str)
            .unwrap_or("-");
        let blocker_codes = case
            .get("blockers")
            .and_then(Value::as_array)
            .ok_or(CaseRegistryError::Shape("missing blockers array"))?
            .iter()
            .map(|blocker| {
                blocker
                    .get("code")
                    .and_then(Value::as_str)
                    .ok_or(CaseRegistryError::Shape("missing blocker code"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let evidence = case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .ok_or(CaseRegistryError::Shape("missing standards_evidence array"))?;
        let covered = evidence
            .iter()
            .filter(|entry| entry.get("covered").and_then(Value::as_bool) == Some(true))
            .count();

        output.push_str(&format!(
            "{case_id}\t{status}\t{}\t{sop_class_uid}\t{transfer_syntax_uid}\t{covered}/{} covered\t{artifact_kind}\t{provider}\t{object_family}\t{roadmap_priority}\t{}\n",
            profiles.join(","),
            evidence.len(),
            blocker_codes.join(",")
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

    if status == "planned" || status == "blocked" {
        if let Some(blockers) = case.get("blockers").and_then(Value::as_array) {
            for blocker in blockers {
                let code = blocker
                    .get("code")
                    .and_then(Value::as_str)
                    .ok_or(CaseRegistryError::Shape("missing blocker code"))?;
                gaps.push(StandardsGap {
                    kind: "roadmap_blocker".to_string(),
                    reason: code.to_string(),
                });
            }
        }
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
    use dicom_core::{DataElement, PrimitiveValue, VR};
    use dicom_dictionary_std::uids;
    use dicom_object::{InMemDicomObject, meta::FileMetaTableBuilder};
    use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

    #[test]
    fn nuclear_medicine_report_fields_are_exact_grouped_and_rendered() {
        let file = serde_json::json!({
            "expected_nm_multiframe": {
                "frame_increment_pointers": ["0054,0010", "0054,0020"],
                "energy_window_vector": [1, 1, 2, 2],
                "detector_vector": [1, 2, 1, 2],
                "energy_windows": [
                    { "name": "Tc99m Photopeak" },
                    { "name": "Tc99m Scatter" }
                ],
                "detectors": [
                    { "start_angle_degrees": 0.0 },
                    { "start_angle_degrees": 180.0 }
                ],
                "frame_dimensions": [
                    { "frame_number": 1, "energy_window_index": 1, "detector_index": 1 },
                    { "frame_number": 2, "energy_window_index": 1, "detector_index": 2 },
                    { "frame_number": 3, "energy_window_index": 2, "detector_index": 1 },
                    { "frame_number": 4, "energy_window_index": 2, "detector_index": 2 }
                ]
            }
        });
        let fields = nm_multiframe_report_fields(&file);
        assert_eq!(
            fields.frame_increment_pointers.as_deref(),
            Some("0054,0010; 0054,0020")
        );
        assert_eq!(fields.energy_window_vector.as_deref(), Some("1; 1; 2; 2"));
        assert_eq!(fields.detector_vector.as_deref(), Some("1; 2; 1; 2"));
        assert_eq!(
            fields.energy_window_names.as_deref(),
            Some("Tc99m Photopeak; Tc99m Scatter")
        );
        assert_eq!(
            fields.detector_start_angles_degrees.as_deref(),
            Some("0.0; 180.0")
        );
        assert_eq!(
            fields.frame_dimension_tuples.as_deref(),
            Some("1:1:1; 2:1:2; 3:2:1; 4:2:2")
        );

        let row = serde_json::json!({
            "case_id": "classic/nm/multiframe_explicit_le",
            "nm_frame_increment_pointers": fields.frame_increment_pointers,
            "nm_energy_window_vector": fields.energy_window_vector,
            "nm_detector_vector": fields.detector_vector,
            "nm_energy_window_names": fields.energy_window_names,
            "nm_detector_start_angles_degrees": fields.detector_start_angles_degrees,
            "nm_frame_dimension_tuples": fields.frame_dimension_tuples
        });
        let mut grouped = GroupedCoverage::default();
        grouped.record(&row);
        let grouped_json = grouped.to_json();
        assert_eq!(
            grouped_json.pointer("/nm_energy_window_vectors/1; 1; 2; 2"),
            Some(&Value::from(1))
        );
        assert_eq!(
            grouped_json.pointer("/nm_frame_dimension_tuples/1:1:1; 2:1:2; 3:2:1; 4:2:2"),
            Some(&Value::from(1))
        );

        let markdown = render_coverage_report_markdown(&serde_json::json!({
            "coverage_matrix": [row],
            "grouped_coverage": grouped_json,
            "gaps": []
        }));
        assert!(markdown.contains("## Nuclear Medicine Multi-frame Expectations"));
        assert!(markdown.contains("0054,0010; 0054,0020"));
        assert!(markdown.contains("1:1:1; 2:1:2; 3:2:1; 4:2:2"));
        assert!(markdown.contains("## NM Energy Window Vectors"));
    }

    #[test]
    fn pet_activity_report_fields_are_exact_grouped_and_rendered() {
        let file = serde_json::json!({
            "expected_pet_activity": {
                "units": "BQML",
                "counts_source": "EMISSION",
                "series_type": ["STATIC", "IMAGE"],
                "corrected_image": ["DCAL"],
                "decay_correction": "NONE",
                "dose_calibration_factor": 1.0,
                "rescale_intercept": 0.0,
                "rescale_slope": 2.5,
                "stored_values": [0, 100, 200, 400],
                "activity_values_bqml": [0.0, 250.0, 500.0, 1000.0],
                "frame_reference_time_ms": 30000.0,
                "actual_frame_duration_ms": 60000,
                "image_index": 1,
                "radiopharmaceutical_information_item_count": 0
            }
        });
        let fields = pet_activity_report_fields(&file);
        assert_eq!(fields.units.as_deref(), Some("BQML"));
        assert_eq!(fields.counts_source.as_deref(), Some("EMISSION"));
        assert_eq!(fields.series_type.as_deref(), Some("STATIC; IMAGE"));
        assert_eq!(fields.corrected_image.as_deref(), Some("DCAL"));
        assert_eq!(fields.decay_correction.as_deref(), Some("NONE"));
        assert_eq!(fields.dose_calibration_factor, Some(1.0));
        assert_eq!(fields.rescale_intercept, Some(0.0));
        assert_eq!(fields.rescale_slope, Some(2.5));
        assert_eq!(fields.stored_values.as_deref(), Some("0; 100; 200; 400"));
        assert_eq!(
            fields.activity_values_bqml.as_deref(),
            Some("0.0; 250.0; 500.0; 1000.0")
        );
        assert_eq!(fields.frame_reference_time_ms, Some(30000.0));
        assert_eq!(fields.actual_frame_duration_ms, Some(60000));
        assert_eq!(fields.image_index, Some(1));
        assert_eq!(fields.radiopharmaceutical_information_item_count, Some(0));

        let row = serde_json::json!({
            "case_id": "classic/pet/rescaled_activity_explicit_le",
            "pet_units": fields.units,
            "pet_counts_source": fields.counts_source,
            "pet_series_type": fields.series_type,
            "pet_corrected_image": fields.corrected_image,
            "pet_decay_correction": fields.decay_correction,
            "pet_dose_calibration_factor": fields.dose_calibration_factor,
            "pet_rescale_intercept": fields.rescale_intercept,
            "pet_rescale_slope": fields.rescale_slope,
            "pet_stored_values": fields.stored_values,
            "pet_activity_values_bqml": fields.activity_values_bqml,
            "pet_frame_reference_time_ms": fields.frame_reference_time_ms,
            "pet_actual_frame_duration_ms": fields.actual_frame_duration_ms,
            "pet_image_index": fields.image_index,
            "pet_radiopharmaceutical_information_item_count":
                fields.radiopharmaceutical_information_item_count
        });
        let mut grouped = GroupedCoverage::default();
        grouped.record(&row);
        let grouped_json = grouped.to_json();
        for (pointer, expected) in [
            ("/pet_units/BQML", 1),
            ("/pet_counts_sources/EMISSION", 1),
            ("/pet_series_types/STATIC; IMAGE", 1),
            ("/pet_corrected_images/DCAL", 1),
            ("/pet_decay_corrections/NONE", 1),
            ("/pet_dose_calibration_factors/1.0", 1),
            ("/pet_rescale_intercepts/0.0", 1),
            ("/pet_rescale_slopes/2.5", 1),
            ("/pet_stored_values/0; 100; 200; 400", 1),
            ("/pet_activity_values_bqml/0.0; 250.0; 500.0; 1000.0", 1),
            ("/pet_frame_reference_times_ms/30000.0", 1),
            ("/pet_actual_frame_durations_ms/60000", 1),
            ("/pet_image_indices/1", 1),
            ("/pet_radiopharmaceutical_information_item_counts/0", 1),
        ] {
            assert_eq!(grouped_json.pointer(pointer), Some(&Value::from(expected)));
        }

        let markdown = render_coverage_report_markdown(&serde_json::json!({
            "coverage_matrix": [row],
            "grouped_coverage": grouped_json,
            "gaps": []
        }));
        assert!(markdown.contains("## PET Activity Expectations"));
        assert!(markdown.contains("classic/pet/rescaled_activity_explicit_le"));
        assert!(markdown.contains("0.0; 250.0; 500.0; 1000.0"));
        assert!(markdown.contains("## PET Units"));
        assert!(markdown.contains("## PET Activity Values (BQML)"));
    }

    #[test]
    fn enhanced_pet_report_fields_are_exact_grouped_and_rendered() {
        let file = serde_json::json!({
            "expected_enhanced_pet": {
                "image_type": ["DERIVED", "PRIMARY", "STATIC", "MULTIPLICATION"],
                "frame_type": ["DERIVED", "PRIMARY", "STATIC", "MULTIPLICATION"],
                "view_code": {"code_value": "24422004", "coding_scheme_designator": "SCT", "code_meaning": "Axial"},
                "view_modifier_item_count": 0,
                "slice_progression_direction_present": false,
                "stack_ids": ["1", "1"],
                "in_stack_position_numbers": [1, 2],
                "dimension_index_values": [1, 2],
                "temporal_position_indices": [1, 1],
                "image_positions_patient_mm": [[0.0, 0.0, 0.0], [0.0, 0.0, 5.0]],
                "stored_values_by_frame": [[0, 100, 200, 400], [0, 100, 200, 400]],
                "activity_values_bqml_by_frame": [[0.0, 250.0, 500.0, 1000.0], [0.0, 250.0, 500.0, 1000.0]],
                "real_world_value_mapping": {
                    "intercept": 0.0,
                    "slope": 2.5,
                    "measurement_units": {"code_value": "Bq/ml", "coding_scheme_designator": "UCUM", "code_meaning": "Becquerels/milliliter"}
                },
                "corrections": {
                    "decay": "NO", "attenuation": "NO", "scatter": "NO", "dead_time": "NO",
                    "gantry_motion": "NO", "patient_motion": "NO", "count_loss_normalization": "NO",
                    "randoms": "NO", "non_uniform_radial_sampling": "NO",
                    "sensitivity_calibration": "NO", "detector_normalization": "NO"
                }
            }
        });
        let fields = enhanced_pet_report_fields(Path::new("manifest.json"), &file)
            .expect("complete Enhanced PET expectations must be reportable");
        assert_eq!(
            fields.image_type.as_deref(),
            Some("DERIVED; PRIMARY; STATIC; MULTIPLICATION")
        );
        assert_eq!(fields.view_code.as_deref(), Some("24422004|SCT|Axial"));
        assert_eq!(fields.in_stack_position_numbers.as_deref(), Some("1; 2"));
        assert_eq!(fields.rwvm_slope, Some(2.5));
        assert!(
            fields
                .corrections
                .as_deref()
                .is_some_and(|value| value.contains("detector_normalization=NO"))
        );

        let row = serde_json::json!({
            "case_id": "enhanced/pet/multiframe_explicit_le",
            "enhanced_pet_image_type": fields.image_type,
            "enhanced_pet_frame_type": fields.frame_type,
            "enhanced_pet_view_code": fields.view_code,
            "enhanced_pet_view_modifier_item_count": fields.view_modifier_item_count,
            "enhanced_pet_slice_progression_direction_present": fields.slice_progression_direction_present,
            "enhanced_pet_stack_ids": fields.stack_ids,
            "enhanced_pet_in_stack_position_numbers": fields.in_stack_position_numbers,
            "enhanced_pet_dimension_index_values": fields.dimension_index_values,
            "enhanced_pet_temporal_position_indices": fields.temporal_position_indices,
            "enhanced_pet_image_positions_patient_mm": fields.image_positions_patient_mm,
            "enhanced_pet_stored_values_by_frame": fields.stored_values_by_frame,
            "enhanced_pet_activity_values_bqml_by_frame": fields.activity_values_bqml_by_frame,
            "enhanced_pet_rwvm_intercept": fields.rwvm_intercept,
            "enhanced_pet_rwvm_slope": fields.rwvm_slope,
            "enhanced_pet_rwvm_measurement_units": fields.rwvm_measurement_units,
            "enhanced_pet_corrections": fields.corrections
        });
        let mut grouped = GroupedCoverage::default();
        grouped.record(&row);
        let grouped_json = grouped.to_json();
        assert_eq!(
            grouped_json.pointer("/enhanced_pet_view_codes/24422004|SCT|Axial"),
            Some(&Value::from(1))
        );
        assert_eq!(
            grouped_json.pointer("/enhanced_pet_rwvm_slopes/2.5"),
            Some(&Value::from(1))
        );
        let markdown = render_coverage_report_markdown(&serde_json::json!({
            "coverage_matrix": [row], "grouped_coverage": grouped_json, "gaps": []
        }));
        assert!(markdown.contains("## Enhanced PET Multi-frame Expectations"));
        assert!(markdown.contains("24422004\\|SCT\\|Axial"));
    }

    #[test]
    fn ultrasound_multiframe_report_fields_are_exact_grouped_and_rendered() {
        let file = serde_json::json!({
            "expected_us_multiframe": {
                "image_type": ["ORIGINAL", "PRIMARY", "ABDOMINAL", "0001"],
                "frame_increment_pointer": "0018,1063",
                "frame_time_ms": 100.0,
                "frame_relative_times_ms": [0.0, 100.0, 200.0, 300.0],
                "frame_count": 4,
                "frames": [
                    { "frame_sha256": "be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7" },
                    { "frame_sha256": "303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee" },
                    { "frame_sha256": "7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb" },
                    { "frame_sha256": "8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650" }
                ],
                "spatially_related_frames": false,
                "color_data_present": false,
                "region_calibrated": false,
                "lossy_image_compression": "00"
            }
        });
        let fields = us_multiframe_report_fields(Path::new("manifest.json"), &file)
            .expect("complete US report contract must extract");
        assert_eq!(
            fields.image_type.as_deref(),
            Some("ORIGINAL; PRIMARY; ABDOMINAL; 0001")
        );
        assert_eq!(fields.frame_increment_pointer.as_deref(), Some("0018,1063"));
        assert_eq!(fields.frame_time_ms, Some(100.0));
        assert_eq!(
            fields.frame_relative_times_ms.as_deref(),
            Some("0.0; 100.0; 200.0; 300.0")
        );
        assert_eq!(fields.frame_count, Some(4));
        assert!(
            fields
                .ordered_frame_hashes
                .as_deref()
                .is_some_and(|hashes| hashes.starts_with("be422fa58b70"))
        );
        assert_eq!(fields.spatially_related_frames, Some(false));
        assert_eq!(fields.color_data_present, Some(false));
        assert_eq!(fields.region_calibrated, Some(false));
        assert_eq!(fields.lossy_image_compression.as_deref(), Some("00"));

        let row = serde_json::json!({
            "case_id": "classic/us/multiframe_explicit_le",
            "us_image_type": fields.image_type,
            "us_frame_increment_pointer": fields.frame_increment_pointer,
            "us_frame_time_ms": fields.frame_time_ms,
            "us_frame_relative_times_ms": fields.frame_relative_times_ms,
            "us_frame_count": fields.frame_count,
            "us_ordered_frame_hashes": fields.ordered_frame_hashes,
            "us_spatially_related_frames": fields.spatially_related_frames,
            "us_color_data_present": fields.color_data_present,
            "us_region_calibrated": fields.region_calibrated,
            "us_lossy_image_compression": fields.lossy_image_compression
        });
        let mut grouped = GroupedCoverage::default();
        grouped.record(&row);
        let grouped_json = grouped.to_json();
        for (pointer, expected) in [
            ("/us_image_types/ORIGINAL; PRIMARY; ABDOMINAL; 0001", 1),
            ("/us_frame_increment_pointers/0018,1063", 1),
            ("/us_frame_times_ms/100.0", 1),
            ("/us_frame_counts/4", 1),
            ("/us_spatially_related_frames/false", 1),
            ("/us_color_data_present/false", 1),
            ("/us_region_calibrated/false", 1),
            ("/us_lossy_image_compressions/00", 1),
        ] {
            assert_eq!(grouped_json.pointer(pointer), Some(&Value::from(expected)));
        }

        let markdown = render_coverage_report_markdown(&serde_json::json!({
            "coverage_matrix": [row],
            "grouped_coverage": grouped_json,
            "gaps": []
        }));
        assert!(markdown.contains("## Ultrasound Multi-frame Expectations"));
        assert!(markdown.contains("classic/us/multiframe_explicit_le"));
        assert!(markdown.contains("0.0; 100.0; 200.0; 300.0"));
        assert!(markdown.contains("## US Frame Increment Pointers"));
        assert!(markdown.contains("## US Lossy Image Compression History"));
    }

    #[test]
    fn ultrasound_multiframe_report_rejects_partial_contract() {
        let file = serde_json::json!({
            "expected_us_multiframe": {
                "image_type": ["ORIGINAL", "PRIMARY", "ABDOMINAL", "0001"]
            }
        });
        let error = us_multiframe_report_fields(Path::new("manifest.json"), &file)
            .expect_err("partial US report contract must not silently disappear");
        assert!(matches!(
            error,
            ReportError::MetadataShape {
                message: "expected_us_multiframe must define the complete report contract",
                ..
            }
        ));
    }

    #[test]
    fn xa_projection_report_fields_are_exact_grouped_and_rendered() {
        let file = serde_json::json!({
            "expected_xa_projection": {
                "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"],
                "frame_count": 1,
                "body_part_examined": "HEART",
                "patient_orientation_empty": true,
                "laterality_present": false,
                "pixel_intensity_relationship": "LIN",
                "radiation_setting": "GR",
                "kvp": 80.0,
                "exposure_mas": 4,
                "imager_pixel_spacing_mm": [0.2, 0.2],
                "positioner_primary_angle_degrees": 15.0,
                "positioner_secondary_angle_degrees": -10.0,
                "distance_source_to_detector_mm": 1200.0,
                "distance_source_to_patient_mm": 800.0,
                "estimated_radiographic_magnification_factor": 1.5,
                "lossy_image_compression": "00",
                "multiframe_cine": false,
                "biplane_data_present": false,
                "contrast_used": false,
                "subtraction_applied": false,
                "table_motion_present": false,
                "patient_space_geometry_present": false,
                "pixel_spacing_calibrated": false
            }
        });
        let fields = xa_projection_report_fields(Path::new("manifest.json"), &file)
            .expect("complete XA report contract must extract");
        assert_eq!(
            fields.image_type.as_deref(),
            Some("ORIGINAL; PRIMARY; SINGLE PLANE")
        );
        assert_eq!(fields.frame_count, Some(1));
        assert_eq!(fields.body_part_examined.as_deref(), Some("HEART"));
        assert_eq!(fields.patient_orientation_empty, Some(true));
        assert_eq!(fields.laterality_present, Some(false));
        assert_eq!(fields.pixel_intensity_relationship.as_deref(), Some("LIN"));
        assert_eq!(fields.radiation_setting.as_deref(), Some("GR"));
        assert_eq!(fields.kvp, Some(80.0));
        assert_eq!(fields.exposure_mas, Some(4));
        assert_eq!(fields.imager_pixel_spacing_mm.as_deref(), Some("0.2; 0.2"));
        assert_eq!(fields.positioner_primary_angle_degrees, Some(15.0));
        assert_eq!(fields.positioner_secondary_angle_degrees, Some(-10.0));
        assert_eq!(fields.distance_source_to_detector_mm, Some(1200.0));
        assert_eq!(fields.distance_source_to_patient_mm, Some(800.0));
        assert_eq!(
            fields.estimated_radiographic_magnification_factor,
            Some(1.5)
        );
        assert_eq!(fields.lossy_image_compression.as_deref(), Some("00"));
        assert_eq!(fields.multiframe_cine, Some(false));
        assert_eq!(fields.biplane_data_present, Some(false));
        assert_eq!(fields.contrast_used, Some(false));
        assert_eq!(fields.subtraction_applied, Some(false));
        assert_eq!(fields.table_motion_present, Some(false));
        assert_eq!(fields.patient_space_geometry_present, Some(false));
        assert_eq!(fields.pixel_spacing_calibrated, Some(false));

        let row = serde_json::json!({
            "case_id": "classic/xa/monoplane_explicit_le",
            "xa_image_type": fields.image_type,
            "xa_frame_count": fields.frame_count,
            "xa_body_part_examined": fields.body_part_examined,
            "xa_patient_orientation_empty": fields.patient_orientation_empty,
            "xa_laterality_present": fields.laterality_present,
            "xa_pixel_intensity_relationship": fields.pixel_intensity_relationship,
            "xa_radiation_setting": fields.radiation_setting,
            "xa_kvp": fields.kvp,
            "xa_exposure_mas": fields.exposure_mas,
            "xa_imager_pixel_spacing_mm": fields.imager_pixel_spacing_mm,
            "xa_positioner_primary_angle_degrees": fields.positioner_primary_angle_degrees,
            "xa_positioner_secondary_angle_degrees": fields.positioner_secondary_angle_degrees,
            "xa_distance_source_to_detector_mm": fields.distance_source_to_detector_mm,
            "xa_distance_source_to_patient_mm": fields.distance_source_to_patient_mm,
            "xa_estimated_radiographic_magnification_factor":
                fields.estimated_radiographic_magnification_factor,
            "xa_lossy_image_compression": fields.lossy_image_compression,
            "xa_multiframe_cine": fields.multiframe_cine,
            "xa_biplane_data_present": fields.biplane_data_present,
            "xa_contrast_used": fields.contrast_used,
            "xa_subtraction_applied": fields.subtraction_applied,
            "xa_table_motion_present": fields.table_motion_present,
            "xa_patient_space_geometry_present": fields.patient_space_geometry_present,
            "xa_pixel_spacing_calibrated": fields.pixel_spacing_calibrated
        });
        let mut grouped = GroupedCoverage::default();
        grouped.record(&row);
        let grouped_json = grouped.to_json();
        for pointer in [
            "/xa_image_types/ORIGINAL; PRIMARY; SINGLE PLANE",
            "/xa_frame_counts/1",
            "/xa_body_parts_examined/HEART",
            "/xa_patient_orientation_empty_states/true",
            "/xa_laterality_present_states/false",
            "/xa_pixel_intensity_relationships/LIN",
            "/xa_radiation_settings/GR",
            "/xa_kvps/80.0",
            "/xa_exposures_mas/4",
            "/xa_imager_pixel_spacings_mm/0.2; 0.2",
            "/xa_positioner_primary_angles_degrees/15.0",
            "/xa_positioner_secondary_angles_degrees/-10.0",
            "/xa_distances_source_to_detector_mm/1200.0",
            "/xa_distances_source_to_patient_mm/800.0",
            "/xa_estimated_radiographic_magnification_factors/1.5",
            "/xa_lossy_image_compressions/00",
            "/xa_multiframe_cine_states/false",
            "/xa_biplane_data_present_states/false",
            "/xa_contrast_used_states/false",
            "/xa_subtraction_applied_states/false",
            "/xa_table_motion_present_states/false",
            "/xa_patient_space_geometry_present_states/false",
            "/xa_pixel_spacing_calibrated_states/false",
        ] {
            assert_eq!(
                grouped_json.pointer(pointer),
                Some(&Value::from(1)),
                "{pointer}"
            );
        }

        let markdown = render_coverage_report_markdown(&serde_json::json!({
            "coverage_matrix": [row],
            "grouped_coverage": grouped_json,
            "gaps": []
        }));
        assert!(markdown.contains("## X-Ray Angiographic Projection Expectations"));
        assert!(markdown.contains("classic/xa/monoplane_explicit_le"));
        assert!(markdown.contains("ORIGINAL; PRIMARY; SINGLE PLANE"));
        assert!(markdown.contains("## XA Positioner Primary Angles (degrees)"));
        assert!(markdown.contains("## XA Patient-space Geometry Present States"));
    }

    #[test]
    fn xa_projection_report_rejects_partial_contract() {
        let file = serde_json::json!({
            "expected_xa_projection": {
                "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"]
            }
        });
        let error = xa_projection_report_fields(Path::new("manifest.json"), &file)
            .expect_err("partial XA report contract must not silently disappear");
        assert!(matches!(
            error,
            ReportError::MetadataShape {
                message: "expected_xa_projection must define the complete report contract",
                ..
            }
        ));
    }

    #[test]
    fn xa_projection_report_rejects_missing_contract_for_xa_file() {
        let file = serde_json::json!({"dicom": {"modality": "XA"}});
        let error = xa_projection_report_fields(Path::new("manifest.json"), &file)
            .expect_err("XA report contract must not silently disappear");
        assert!(matches!(
            error,
            ReportError::MetadataShape {
                message: "XA file must define expected_xa_projection",
                ..
            }
        ));

        let non_xa = serde_json::json!({"dicom": {"modality": "CT"}});
        assert_eq!(
            xa_projection_report_fields(Path::new("manifest.json"), &non_xa)
                .expect("non-XA files may omit the XA contract"),
            XaProjectionReportFields::default()
        );
    }

    #[test]
    fn xrf_projection_report_fields_are_complete_grouped_and_rendered() {
        let file = serde_json::json!({
            "expected_xrf_projection": {
                "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"],
                "frame_count": 1,
                "body_part_examined": "ABDOMEN",
                "patient_orientation_empty": true,
                "laterality_present": false,
                "pixel_intensity_relationship": "LIN",
                "radiation_setting": "SC",
                "kvp": 70.0,
                "exposure_mas": 1,
                "imager_pixel_spacing_mm": [0.2, 0.2],
                "distance_source_to_detector_mm": 1200.0,
                "distance_source_to_patient_mm": 800.0,
                "estimated_radiographic_magnification_factor": 1.5,
                "column_angulation_degrees": 10.0,
                "lossy_image_compression": "00",
                "multiframe_cine": false,
                "biplane_data_present": false,
                "contrast_used": false,
                "subtraction_applied": false,
                "table_position_present": false,
                "table_motion_present": false,
                "table_tilt_present": false,
                "tomography_present": false,
                "patient_space_geometry_present": false,
                "pixel_spacing_calibrated": false,
                "xa_positioner_angles_present": false
            }
        });
        let fields = xrf_projection_report_fields(Path::new("manifest.json"), &file)
            .expect("complete XRF report contract must extract");
        assert_eq!(
            fields.image_type.as_deref(),
            Some("ORIGINAL; PRIMARY; SINGLE PLANE")
        );
        assert_eq!(fields.body_part_examined.as_deref(), Some("ABDOMEN"));
        assert_eq!(fields.column_angulation_degrees, Some(10.0));
        assert_eq!(fields.table_position_present, Some(false));
        assert_eq!(fields.tomography_present, Some(false));
        assert_eq!(fields.xa_positioner_angles_present, Some(false));

        let row = serde_json::json!({
            "case_id": "classic/xrf/monoplane_explicit_le",
            "xrf_image_type": fields.image_type,
            "xrf_frame_count": fields.frame_count,
            "xrf_body_part_examined": fields.body_part_examined,
            "xrf_patient_orientation_empty": fields.patient_orientation_empty,
            "xrf_laterality_present": fields.laterality_present,
            "xrf_pixel_intensity_relationship": fields.pixel_intensity_relationship,
            "xrf_radiation_setting": fields.radiation_setting,
            "xrf_kvp": fields.kvp,
            "xrf_exposure_mas": fields.exposure_mas,
            "xrf_imager_pixel_spacing_mm": fields.imager_pixel_spacing_mm,
            "xrf_distance_source_to_detector_mm": fields.distance_source_to_detector_mm,
            "xrf_distance_source_to_patient_mm": fields.distance_source_to_patient_mm,
            "xrf_estimated_radiographic_magnification_factor": fields.estimated_radiographic_magnification_factor,
            "xrf_column_angulation_degrees": fields.column_angulation_degrees,
            "xrf_lossy_image_compression": fields.lossy_image_compression,
            "xrf_multiframe_cine": fields.multiframe_cine,
            "xrf_biplane_data_present": fields.biplane_data_present,
            "xrf_contrast_used": fields.contrast_used,
            "xrf_subtraction_applied": fields.subtraction_applied,
            "xrf_table_position_present": fields.table_position_present,
            "xrf_table_motion_present": fields.table_motion_present,
            "xrf_table_tilt_present": fields.table_tilt_present,
            "xrf_tomography_present": fields.tomography_present,
            "xrf_patient_space_geometry_present": fields.patient_space_geometry_present,
            "xrf_pixel_spacing_calibrated": fields.pixel_spacing_calibrated,
            "xrf_xa_positioner_angles_present": fields.xa_positioner_angles_present
        });
        let mut grouped = GroupedCoverage::default();
        grouped.record(&row);
        let grouped_json = grouped.to_json();
        assert_eq!(
            grouped_json.pointer("/xrf_column_angulations_degrees/10.0"),
            Some(&Value::from(1))
        );
        assert_eq!(
            grouped_json.pointer("/xrf_table_position_present_states/false"),
            Some(&Value::from(1))
        );

        let markdown = render_coverage_report_markdown(&serde_json::json!({
            "coverage_matrix": [row],
            "grouped_coverage": grouped_json,
            "gaps": []
        }));
        assert!(markdown.contains("## X-Ray Radiofluoroscopic Projection Expectations"));
        assert!(markdown.contains("classic/xrf/monoplane_explicit_le"));
        assert!(markdown.contains("## XRF Column Angulations (degrees)"));
    }

    #[test]
    fn xrf_projection_report_rejects_partial_or_missing_rf_contract() {
        let partial = serde_json::json!({
            "expected_xrf_projection": {
                "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"]
            }
        });
        assert!(matches!(
            xrf_projection_report_fields(Path::new("manifest.json"), &partial),
            Err(ReportError::MetadataShape {
                message: "expected_xrf_projection must define the complete report contract",
                ..
            })
        ));

        let missing = serde_json::json!({"dicom": {"modality": "RF"}});
        assert!(matches!(
            xrf_projection_report_fields(Path::new("manifest.json"), &missing),
            Err(ReportError::MetadataShape {
                message: "RF file must define expected_xrf_projection",
                ..
            })
        ));
        let non_xrf = serde_json::json!({"dicom": {"modality": "CT"}});
        assert_eq!(
            xrf_projection_report_fields(Path::new("manifest.json"), &non_xrf)
                .expect("non-XRF files may omit the XRF contract"),
            XrfProjectionReportFields::default()
        );
    }

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
    fn float32_manifest_pixel_validation_accepts_exact_native_frames() {
        let values = [0.25_f32, 0.5, 1.0, 1.5];
        let obj = float32_test_object(&values, false);
        let manifest = float32_test_manifest(&values);
        let mut failures = Vec::new();

        validate_manifest_image_pixel_data(
            &mut failures,
            "parametric-map.dcm",
            Path::new("manifest.json"),
            &manifest,
            &obj,
        )
        .expect("float32 manifest validation should complete");

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn float32_manifest_pixel_validation_rejects_integer_fields_and_bad_hash() {
        let values = [0.25_f32, 0.5, 1.0, 1.5];
        let obj = float32_test_object(&values, true);
        let mut manifest = float32_test_manifest(&values);
        manifest["image"]["bits_stored"] = serde_json::json!(32);
        manifest["pixel_data"]["frame_hashes"][0] =
            serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
        let mut failures = Vec::new();

        validate_manifest_image_pixel_data(
            &mut failures,
            "parametric-map.dcm",
            Path::new("manifest.json"),
            &manifest,
            &obj,
        )
        .expect("invalid float32 metadata should produce validation failures");

        let joined = failures.join("\n");
        assert!(joined.contains("bits_stored_absent"));
        assert!(joined.contains("integer_pixel_data_absent"));
        assert!(joined.contains("double_float_pixel_data_absent"));
        assert!(joined.contains("float_pixel_data_frame_hash"));
    }

    #[test]
    fn u32_sc_manifest_pixel_contract_accepts_exact_full_range_words() {
        let manifest = u32_sc_test_manifest();
        let bytes = u32_sc_test_bytes();
        let mut failures = Vec::new();

        validate_u32_sc_manifest_pixel_contract(
            &mut failures,
            "classic/sc/mono2_u32_explicit_le/instance.dcm",
            Path::new("manifest.json"),
            &manifest,
            &bytes,
        )
        .expect("well-formed unsigned 32-bit contract should validate");

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn u32_sc_manifest_pixel_contract_rejects_tampered_words_and_metadata() {
        let mut manifest = u32_sc_test_manifest();
        manifest["expected_u32_pixels"]["stored_values"][3] = Value::from(0_u64);
        manifest["expected_u32_pixels"]["pixel_data_sha256"] = Value::from("0".repeat(64));
        manifest["expected_u32_pixels"]["word_byte_order"] = Value::from("big_endian");
        manifest["expected_u32_pixels"]["full_unsigned_range"] = Value::from(false);
        manifest["image"]["bits_stored"] = Value::from(31);
        let mut bytes = u32_sc_test_bytes();
        bytes[12] = 0;
        let mut failures = Vec::new();

        validate_u32_sc_manifest_pixel_contract(
            &mut failures,
            "classic/sc/mono2_u32_explicit_le/instance.dcm",
            Path::new("manifest.json"),
            &manifest,
            &bytes,
        )
        .expect("semantic mismatches should be validation failures");

        let joined = failures.join("\n");
        for check in [
            "u32_bits_stored",
            "u32_expected_stored_values",
            "u32_word_byte_order",
            "u32_full_unsigned_range",
            "u32_declared_pixel_sha256",
            "u32_pixel_data_sha256",
            "u32_decoded_stored_values",
        ] {
            assert!(joined.contains(check), "missing {check} failure:\n{joined}");
        }
    }

    #[test]
    fn u1_sc_manifest_pixel_contract_accepts_continuous_frames_and_padding() {
        let manifest = u1_sc_test_manifest();
        let obj = u1_sc_test_object();
        let bytes = vec![0x55, 0x55, 0x01, 0x00];
        let mut failures = Vec::new();

        validate_u1_sc_manifest_pixel_contract(
            &mut failures,
            "classic/sc/mono2_u1_native/instance.dcm",
            Path::new("manifest.json"),
            &manifest,
            &obj,
            &bytes,
        )
        .expect("well-formed one-bit contract should validate");

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn u1_sc_manifest_pixel_contract_rejects_per_frame_padding_and_tampering() {
        let mut manifest = u1_sc_test_manifest();
        manifest["expected_u1_pixels"]["packing_order"] = Value::from("most_significant_bit_first");
        manifest["expected_u1_pixels"]["stored_values"][9] = Value::from(1);
        manifest["expected_u1_pixels"]["pixel_data_sha256"] = Value::from("0".repeat(64));
        manifest["expected_u1_pixels"]["value_field_padding_bytes"] = Value::from(0);
        let obj = u1_sc_test_object();
        let bytes = vec![0x55, 0x01, 0xaa, 0x00];
        let mut failures = Vec::new();

        validate_u1_sc_manifest_pixel_contract(
            &mut failures,
            "classic/sc/mono2_u1_native/instance.dcm",
            Path::new("manifest.json"),
            &manifest,
            &obj,
            &bytes,
        )
        .expect("one-bit semantic mismatches should be validation failures");

        let joined = failures.join("\n");
        for check in [
            "u1_expected_stored_values",
            "u1_packing_order",
            "u1_declared_pixel_sha256",
            "u1_value_field_padding_bytes",
            "u1_pixel_data_sha256",
            "u1_decoded_stored_values",
        ] {
            assert!(joined.contains(check), "missing {check} failure:\n{joined}");
        }
    }

    #[test]
    fn icc_profile_manifest_contract_accepts_locked_input_profile() {
        let manifest = icc_profile_test_manifest();
        let obj = icc_profile_test_object(icc_profile_test_bytes(), "SRGB");
        let mut failures = Vec::new();

        validate_icc_profile_manifest_contract(
            &mut failures,
            "vl/photo/rgb_icc_profile_explicit_le/instance.dcm",
            Path::new("manifest.json"),
            &manifest,
            &obj,
        )
        .expect("well-formed ICC contract should validate");

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn icc_profile_manifest_contract_rejects_tampered_profile_and_metadata() {
        let mut manifest = icc_profile_test_manifest();
        manifest["expected_icc_profile"]["color_space"] = Value::from("ADOBERGB");
        let mut profile = icc_profile_test_bytes();
        profile[36..40].copy_from_slice(b"zzzz");
        let obj = icc_profile_test_object(profile, "ADOBERGB");
        let mut failures = Vec::new();

        validate_icc_profile_manifest_contract(
            &mut failures,
            "vl/photo/rgb_icc_profile_explicit_le/instance.dcm",
            Path::new("manifest.json"),
            &manifest,
            &obj,
        )
        .expect("ICC semantic mismatches should be validation failures");

        let joined = failures.join("\n");
        for check in [
            "icc_manifest_color_space",
            "icc_color_space",
            "icc_profile_sha256",
            "icc_profile_signature",
        ] {
            assert!(joined.contains(check), "missing {check} failure:\n{joined}");
        }
    }

    fn icc_profile_test_object(profile: Vec<u8>, color_space: &str) -> OpenedObject {
        let mut obj = InMemDicomObject::new_empty();
        obj.put(DataElement::new(
            tags::COLOR_SPACE,
            VR::CS,
            PrimitiveValue::from(color_space),
        ));
        obj.put(DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(profile.into()),
        ));
        obj.with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .media_storage_sop_class_uid(uids::VL_PHOTOGRAPHIC_IMAGE_STORAGE)
                .media_storage_sop_instance_uid("2.25.1")
                .implementation_class_uid("2.25.2"),
        )
        .expect("ICC test object should have valid file metadata")
    }

    fn icc_profile_test_bytes() -> Vec<u8> {
        include_str!("generator/native/dcmtk_srgb_input_profile.hex")
            .split_ascii_whitespace()
            .flat_map(|word| {
                word.as_bytes().chunks_exact(2).map(|pair| {
                    let text = std::str::from_utf8(pair).expect("profile source must be ASCII");
                    u8::from_str_radix(text, 16).expect("profile source must be hexadecimal")
                })
            })
            .collect()
    }

    fn icc_profile_test_manifest() -> Value {
        serde_json::json!({
            "case_id": "vl/photo/rgb_icc_profile_explicit_le",
            "image": {
                "rows": 2,
                "columns": 2,
                "frames": 1,
                "samples_per_pixel": 3,
                "photometric_interpretation": "RGB",
                "bits_allocated": 8,
                "bits_stored": 8,
                "high_bit": 7,
                "pixel_representation": 0,
                "planar_configuration": 0
            },
            "pixel_data": {"value_length": 12},
            "expected_icc_profile": {
                "tag": "(0028,2000)",
                "vr": "OB",
                "profile_sha256": "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
                "profile_size_bytes": 736,
                "declared_profile_size_bytes": 736,
                "profile_version": "2.1.0",
                "device_class": "scnr",
                "data_color_space": "RGB",
                "profile_connection_space": "XYZ",
                "profile_signature": "acsp",
                "rendering_intent": "perceptual",
                "rendering_intent_code": 0,
                "tag_count": 9,
                "color_space": "SRGB",
                "profile_description": "sRGB",
                "copyright": "CC0",
                "source_identity": "DCMTK 3.7.0 DCMTK_SRGB_ICC_SAMPLE"
            }
        })
    }

    fn u1_sc_test_object() -> OpenedObject {
        let mut obj = InMemDicomObject::new_empty();
        obj.put(DataElement::new(
            tags::FRAME_INCREMENT_POINTER,
            VR::AT,
            PrimitiveValue::Tags(vec![tags::PAGE_NUMBER_VECTOR].into()),
        ));
        obj.put(DataElement::new(
            tags::PAGE_NUMBER_VECTOR,
            VR::IS,
            PrimitiveValue::from("1\\2"),
        ));
        obj.with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .media_storage_sop_class_uid(
                    uids::MULTI_FRAME_SINGLE_BIT_SECONDARY_CAPTURE_IMAGE_STORAGE,
                )
                .media_storage_sop_instance_uid("2.25.1")
                .implementation_class_uid("2.25.2"),
        )
        .expect("u1 test object should have valid file metadata")
    }

    fn u1_sc_test_manifest() -> Value {
        serde_json::json!({
            "case_id": "classic/sc/mono2_u1_native",
            "recipe": {"recipe_parameters": {"pixel_values": [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]}},
            "image": {
                "rows": 3,
                "columns": 3,
                "frames": 2,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 1,
                "bits_stored": 1,
                "high_bit": 0,
                "pixel_representation": 0,
                "planar_configuration": Value::Null
            },
            "pixel_data": {
                "vr": "OB",
                "native_or_encapsulated": "native",
                "value_length": 4,
                "frame_count": 2
            },
            "expected_semantics": {"pixel_min": 0, "pixel_max": 1},
            "expected_u1_pixels": {
                "packing_order": "least_significant_bit_first",
                "frame_boundary_policy": "continuous_without_per_frame_padding",
                "stored_values": [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
                "decoded_frame_sha256": [
                    "a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3",
                    "c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae"
                ],
                "pixel_data_sha256": "9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b",
                "significant_bits": 18,
                "significant_packed_bytes": 3,
                "unused_high_bits": 6,
                "value_field_padding_bytes": 1,
                "frame_two_bit_offset": 9
            }
        })
    }

    fn u32_sc_test_bytes() -> Vec<u8> {
        [0_u32, 65_535, 2_147_483_648, 4_294_967_295]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    }

    fn u32_sc_test_manifest() -> Value {
        serde_json::json!({
            "case_id": "classic/sc/mono2_u32_explicit_le",
            "recipe": {"recipe_parameters": {"pixel_values": [0_u64, 65_535_u64, 2_147_483_648_u64, 4_294_967_295_u64]}},
            "image": {
                "rows": 2,
                "columns": 2,
                "frames": 1,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 32,
                "bits_stored": 32,
                "high_bit": 31,
                "pixel_representation": 0,
                "planar_configuration": Value::Null
            },
            "pixel_data": {
                "vr": "OW",
                "native_or_encapsulated": "native",
                "value_length": 16,
                "frame_count": 1
            },
            "expected_semantics": {"pixel_min": 0_u64, "pixel_max": 4_294_967_295_u64},
            "expected_u32_pixels": {
                "stored_values": [0_u64, 65_535_u64, 2_147_483_648_u64, 4_294_967_295_u64],
                "pixel_data_sha256": "56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41",
                "word_byte_order": "little_endian",
                "full_unsigned_range": true
            }
        })
    }

    fn float32_test_object(values: &[f32], include_forbidden_fields: bool) -> OpenedObject {
        let mut obj = InMemDicomObject::new_empty();
        for (tag, vr, value) in [
            (tags::SOP_CLASS_UID, VR::UI, uids::PARAMETRIC_MAP_STORAGE),
            (tags::SOP_INSTANCE_UID, VR::UI, "2.25.1"),
            (tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "MONOCHROME2"),
        ] {
            obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
        }
        for (tag, value) in [
            (tags::ROWS, 1_u16),
            (tags::COLUMNS, 2),
            (tags::SAMPLES_PER_PIXEL, 1),
            (tags::BITS_ALLOCATED, 32),
        ] {
            obj.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
        }
        obj.put(DataElement::new(
            tags::NUMBER_OF_FRAMES,
            VR::IS,
            PrimitiveValue::from("2"),
        ));
        obj.put(DataElement::new(
            tags::FLOAT_PIXEL_DATA,
            VR::OF,
            PrimitiveValue::F32(values.to_vec().into()),
        ));
        if include_forbidden_fields {
            obj.put(DataElement::new(
                tags::BITS_STORED,
                VR::US,
                PrimitiveValue::from(32_u16),
            ));
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OW,
                PrimitiveValue::from(vec![0_u8; 16]),
            ));
            obj.put(DataElement::new(
                tags::DOUBLE_FLOAT_PIXEL_DATA,
                VR::OD,
                PrimitiveValue::F64(vec![0.0_f64; 4].into()),
            ));
        }
        obj.with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .media_storage_sop_class_uid(uids::PARAMETRIC_MAP_STORAGE)
                .media_storage_sop_instance_uid("2.25.1")
                .implementation_class_uid("2.25.2"),
        )
        .expect("float32 test object should have valid file metadata")
    }

    fn float32_test_manifest(values: &[f32]) -> Value {
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        serde_json::json!({
            "image": {
                "sample_type": "float32",
                "rows": 1,
                "columns": 2,
                "frames": 2,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 32,
                "planar_configuration": Value::Null
            },
            "pixel_data": {
                "vr": "OF",
                "native_or_encapsulated": "native",
                "value_length": bytes.len(),
                "frame_count": 2,
                "frame_hashes": [sha256_hex(&bytes[..8]), sha256_hex(&bytes[8..])]
            }
        })
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
                "enhanced/mr/multiframe_temporal_position_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t24/24 covered"
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
        assert!(
            output.contains(
                "classic/sc/rgb_planar0_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented RGB RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_i16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented signed RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_i16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented signed MONOCHROME1 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_i16_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t7/7 covered"
            ),
            "list-cases output must show implemented signed MONOCHROME1 multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_odd_3x3_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented odd 3x3 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_i16_odd_3x3_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented MONOCHROME1 signed odd 3x3 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_rect_2x3_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented rectangular 2x3 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u8_padding_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t8/8 covered"
            ),
            "list-cases output must show implemented 8-bit Pixel Padding RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_i16_rect_2x3_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t7/7 covered"
            ),
            "list-cases output must show implemented MONOCHROME1 signed rectangular 2x3 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented 16-bit multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_u8_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented MONOCHROME1 multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_tiny_1x1_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented tiny 1x1 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_u16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented 16-bit MONOCHROME1 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_u16_padding_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t8/8 covered"
            ),
            "list-cases output must show implemented Pixel Padding RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_u16_padding_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t10/10 covered"
            ),
            "list-cases output must show implemented MONOCHROME1 unsigned multi-frame Pixel Padding RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono2_i16_padding_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t10/10 covered"
            ),
            "list-cases output must show implemented signed multi-frame Pixel Padding RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/mono1_i16_padding_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t10/10 covered"
            ),
            "list-cases output must show implemented MONOCHROME1 signed Pixel Padding RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/rgb_planar0_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented RGB multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/rgb_planar1_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented RGB planar-1 RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/rgb_planar1_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented RGB planar-1 multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/ybr_full_planar0_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
            ),
            "list-cases output must show implemented YBR_FULL RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/ybr_full_planar0_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented YBR_FULL multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/ybr_full_planar1_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
            ),
            "list-cases output must show implemented YBR_FULL planar-1 multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/palette_color_u8_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t11/11 covered"
            ),
            "list-cases output must show implemented PALETTE COLOR RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/sc/palette_color_u8_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t12/12 covered"
            ),
            "list-cases output must show implemented PALETTE COLOR multi-frame RLE Lossless status"
        );
        assert!(
            output.contains(
                "vl/photo/palette_color_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.77.1.4\t1.2.840.10008.1.2.5\t13/13 covered"
            ),
            "list-cases output must show implemented VL Photographic PALETTE COLOR RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/mr/mono2_u16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4\t1.2.840.10008.1.2.5\t13/13 covered"
            ),
            "list-cases output must show implemented MR RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/cr/overlay_modality_voi_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1\t1.2.840.10008.1.2.5\t16/16 covered"
            ),
            "list-cases output must show implemented CR RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/dx/display_shutter_mono2_u16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1.1\t1.2.840.10008.1.2.5\t11/11 covered"
            ),
            "list-cases output must show implemented DX RLE Lossless status"
        );
        assert!(
            output.contains(
                "classic/mg/for_processing_mono2_u16_12bit_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1.2.1\t1.2.840.10008.1.2.5\t15/15 covered"
            ),
            "list-cases output must show implemented MG For Processing RLE Lossless status"
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
    fn list_cases_all_profile_uses_generation_union() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("all"), None)
            .expect("all cases should list");

        assert!(output.contains("classic/sc/mono2_u8_explicit_le"));
        assert!(output.contains("classic/ct/mono2_i16_rescale_12bit_explicit_le"));
        assert!(output.contains("enhanced/ct/multiframe_shared_perframe_explicit_le"));
        assert!(!output.contains("classic/sc/mono2_u8_explicit_be"));
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
    fn coverage_report_exposes_key_object_selection_labels() {
        let out_dir = unique_temp_dir("kos_report_labels");
        let prepared = prepare_generation_run(GenerateOptions {
            profile: "extended".to_string(),
            out_dir: out_dir.clone(),
            seed: 1,
            include_stress: false,
        })
        .expect("extended generation run should prepare");

        write_generation_run(&prepared).expect("extended manifest should write");
        let report = build_coverage_report(&out_dir).expect("coverage report should build");
        let kos_row = coverage_row(&report, "derived/sr/key_object_selection_explicit_le");

        assert_eq!(
            kos_row.get("kos_document_title").and_then(Value::as_str),
            Some("Of Interest")
        );
        assert_eq!(
            kos_row.get("kos_key_object_count").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            kos_row
                .get("kos_key_object_relationship_types")
                .and_then(Value::as_str),
            Some("CONTAINS; CONTAINS")
        );
        assert_eq!(
            kos_row
                .get("kos_key_object_value_types")
                .and_then(Value::as_str),
            Some("IMAGE; IMAGE")
        );
        assert_eq!(
            kos_row
                .get("kos_referenced_frame_numbers")
                .and_then(Value::as_str),
            Some("1; 2")
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/kos_document_titles/Of Interest")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/kos_key_object_counts/2")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/kos_key_object_relationship_types/CONTAINS; CONTAINS")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/kos_key_object_value_types/IMAGE; IMAGE")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/grouped_coverage/kos_referenced_frame_numbers/1; 2")
                .and_then(Value::as_u64),
            Some(1)
        );

        let markdown = render_coverage_report_markdown(&report);
        assert!(markdown.contains("### KOS Document Titles"));
        assert!(markdown.contains("| Of Interest | 1 |"));
        assert!(markdown.contains("### KOS Key Object Relationship Types"));
        assert!(markdown.contains("| CONTAINS; CONTAINS | 1 |"));

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

        let skipped = skipped_cases_for_run(&registry, &run, &generated_case_ids, &[])
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
            generated.files.is_empty(),
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
    fn backend_unavailability_overrides_generic_missing_generator_status() {
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: unique_temp_dir("backend_unavailable"),
            manifest_path: unique_temp_dir("backend_unavailable").join("manifest.json"),
            seed: 1,
            include_stress: false,
        };
        let case_id = "derived/parametric-map/float32_ct_derived_explicit_le";
        let registry = serde_json::json!({
            "cases": [{
                "case_id": case_id,
                "status": "implemented",
                "profiles": ["extended"],
                "requirements": {
                    "features": [],
                    "external_codecs": [],
                    "external_validators": []
                },
                "skip": null,
                "standards_evidence": []
            }]
        });
        let unavailable = serde_json::json!({
            "case_id": case_id,
            "status": "unavailable",
            "reason_code": "external_backend_unavailable",
            "message": "The prepared uv runtime is absent.",
            "recheck_phase": "phase-1",
            "standards_evidence": []
        });

        let skipped = skipped_cases_for_run(&registry, &run, &[], &[unavailable])
            .expect("backend availability should build a skipped row");
        assert_eq!(skipped.len(), 1);
        assert_eq!(
            skipped[0].get("reason_code").and_then(Value::as_str),
            Some("external_backend_unavailable")
        );
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

    fn coverage_row<'a>(report: &'a Value, case_id: &str) -> &'a Value {
        report
            .get("coverage_matrix")
            .and_then(Value::as_array)
            .and_then(|rows| {
                rows.iter()
                    .find(|row| row.get("case_id").and_then(Value::as_str) == Some(case_id))
            })
            .expect("coverage row should be present")
    }
}
