use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version_banner() -> String {
    format!("{PACKAGE_NAME} {PACKAGE_VERSION}")
}

pub const SUPPORTED_PROFILES: &[&str] = &[
    "smoke", "core", "extended", "legacy", "stress", "all", "negative", "fuzz",
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

#[derive(Debug)]
pub enum GenerateError {
    InvalidProfile(String),
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
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
        }
    }
}

impl Error for GenerateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidProfile(_) => None,
            Self::CreateOutputDir { source, .. } => Some(source),
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
            Self::Shape(message) => write!(f, "invalid case registry shape: {message}"),
        }
    }
}

impl Error for CaseRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Shape(_) => None,
        }
    }
}

pub fn list_cases_from_registry_path(
    registry_path: impl AsRef<Path>,
    profile_filter: Option<&str>,
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

    list_cases_from_registry_value(&registry, profile_filter)
}

pub fn list_cases_from_registry_value(
    registry: &Value,
    profile_filter: Option<&str>,
) -> Result<String, CaseRegistryError> {
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
        let output = list_cases_from_registry_path("cases/registry.json", Some("smoke"))
            .expect("smoke case registry should list");

        assert!(
            output.contains(
                "classic/sc/mono2_u8_explicit_le\tplanned\tsmoke\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t2/2 covered"
            ),
            "list-cases output must show smoke status and standards evidence coverage"
        );
    }

    #[test]
    fn list_cases_shows_committed_core_case_status_and_evidence() {
        let output = list_cases_from_registry_path("cases/registry.json", Some("core"))
            .expect("core case registry should list");

        assert!(
            output.contains(
                "classic/ct/mono2_i16_rescale_12bit_explicit_le\tplanned\tcore\t1.2.840.10008.5.1.4.1.1.2\t1.2.840.10008.1.2.1\t2/2 covered"
            ),
            "list-cases output must show core status and standards evidence coverage"
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
            "the first skeleton must not write a manifest before manifest content exists"
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
}
