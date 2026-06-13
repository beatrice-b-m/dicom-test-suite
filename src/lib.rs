use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::VR;
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::{FileDicomObject, InMemDicomObject, open_file};
use serde_json::Value;

mod generator;
pub mod uid;
mod validation;
pub use uid::{DeterministicUidInput, UidRole, deterministic_uid};

type OpenedObject = FileDicomObject<InMemDicomObject<StandardDataDictionary>>;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RUSTC_VERSION: &str = env!("DICOM_TEST_SUITE_RUSTC_VERSION");
pub const TARGET_TRIPLE: &str = env!("DICOM_TEST_SUITE_TARGET");

pub fn version_banner() -> String {
    format!("{PACKAGE_NAME} {PACKAGE_VERSION}")
}

pub const SUPPORTED_PROFILES: &[&str] = &[
    "smoke", "core", "extended", "legacy", "stress", "all", "negative", "fuzz",
];
pub const SUPPORTED_CASE_STATUSES: &[&str] =
    &["planned", "implemented", "skipped", "blocked", "deprecated"];

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

    let mut failures = Vec::new();
    for file in files {
        validate_manifest_file(root_dir, &manifest_path, file, &mut failures)?;
    }

    Ok(ValidationSummary {
        manifest_path,
        files_checked: files.len(),
        failures,
    })
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

    let obj = match open_file(&path) {
        Ok(obj) => obj,
        Err(err) => {
            failures.push(format!("{relative_path}: open_file: {err}"));
            return Ok(());
        }
    };

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
    validate_str_element(
        failures,
        relative_path,
        &obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        "photometric_interpretation",
        manifest_str(
            manifest_path,
            file,
            "/image/photometric_interpretation",
            "photometric_interpretation must be a string",
        )?,
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
    let pixel_bytes = match pixel_element.value().to_bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            failures.push(format!("{relative_path}: pixel_data_bytes: {err}"));
            return Ok(());
        }
    };
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
    let expected_native_length = if photometric == "YBR_FULL_422" {
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
        Ok(actual) => validate_equal(failures, relative_path, name, actual, expected),
        Err(err) => failures.push(format!("{relative_path}: {name}: {err}")),
    }
    Ok(expected)
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
            Ok(actual) => validate_equal(failures, relative_path, "frames", actual, expected),
            Err(err) => failures.push(format!("{relative_path}: frames: {err}")),
        },
        Ok(None) if expected == 1 => {}
        Ok(None) => failures.push(format!(
            "{relative_path}: frames: Number of Frames is missing for {expected} frames"
        )),
        Err(err) => failures.push(format!("{relative_path}: frames: {err}")),
    }
    Ok(expected)
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
        "derived_refs": [],
        "validation_status": file.pointer("/validation/status").and_then(Value::as_str).unwrap_or("not_run"),
        "determinism": report_str(manifest_path, file, "/determinism", "determinism must be a string")?,
        "object_type": file.get("case_id").and_then(Value::as_str).and_then(|case_id| case_id.split('/').next()),
        "known_stressors": file.get("known_stressors").cloned().unwrap_or_else(|| serde_json::json!([]))
    }))
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
            if skipped.get("reason_code").and_then(Value::as_str) == Some("case_planned") =>
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
        pin_status,
        "dicom_standard_kb.commit",
        &mut warnings,
    )?;
    require_documented_nullable_pin(
        path,
        &kb_value,
        "/db_sha256",
        pin_status,
        "dicom_standard_kb.db_sha256",
        &mut warnings,
    )?;

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
    pin_status: &str,
    field: &str,
    warnings: &mut Vec<String>,
) -> Result<(), StandardsError> {
    match value.pointer(pointer) {
        Some(Value::Null) => {
            if pin_status.is_empty() {
                return Err(standards_shape(
                    path,
                    format!("{field} is null and dicom_standard_kb.pin_status is empty"),
                ));
            }
            warnings.push(format!("{field} unavailable: {pin_status}"));
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
        Some(Value::Null) => warnings.push(format!(
            "source_artifact.{part}.{format} sha256 unavailable: {status}"
        )),
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
        .map(|file| file.manifest_entry)
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
            "feature_flags": []
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
            "implemented" => skipped.push(serde_json::json!({
                "case_id": case_id,
                "status": "unavailable",
                "reason_code": "generator_not_implemented",
                "message": "This implemented registry case does not have a generator recipe.",
                "recheck_phase": "remediation-r1",
                "standards_evidence": case.get("standards_evidence").cloned().unwrap_or_else(|| serde_json::json!([]))
            })),
            "planned" => skipped.push(serde_json::json!({
                "case_id": case_id,
                "status": "unavailable",
                "reason_code": "case_planned",
                "message": "This planned registry case does not have an implemented generator recipe yet.",
                "recheck_phase": planned_recheck_phase(case_id),
                "standards_evidence": case.get("standards_evidence").cloned().unwrap_or_else(|| serde_json::json!([]))
            })),
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
    if case_id.starts_with("derived/") {
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
            matches!(profile.as_str(), "smoke" | "core" | "extended" | "legacy")
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
                "derived/seg/binary_multiframe_explicit_le\tplanned\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.1\t8/8 covered"
            ),
            "list-cases output must show planned SEG extended status"
        );
    }

    #[test]
    fn list_cases_filters_by_profile_and_status() {
        let output =
            list_cases_from_registry_path("cases/registry.json", Some("extended"), Some("planned"))
                .expect("extended planned cases should list");

        assert!(
            output.contains(
                "derived/seg/binary_multiframe_explicit_le\tplanned\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.1\t8/8 covered"
            ),
            "status filter should include planned SEG in extended"
        );
        assert!(
            !output.contains("enhanced/ct/multiframe_shared_perframe_explicit_le"),
            "status filter should exclude implemented extended cases"
        );
        assert!(
            !output.contains("vl/photo/rgb_planar0_explicit_le"),
            "profile filter should still exclude planned core VL cases"
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
                    "skip": null,
                    "standards_evidence": []
                },
                {
                    "case_id": "classic/sc/missing_recipe_explicit_le",
                    "status": "implemented",
                    "profiles": ["core"],
                    "skip": null,
                    "standards_evidence": [{"source": "dicom-standard-kb", "covered": true}]
                },
                {
                    "case_id": "vl/photo/rgb_planar0_explicit_le",
                    "status": "planned",
                    "profiles": ["core"],
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
            4,
            "implemented missing recipe, planned, skipped, and blocked cases should be reported"
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
