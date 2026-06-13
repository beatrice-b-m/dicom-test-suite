use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde_json::Value;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version_banner() -> String {
    format!("{PACKAGE_NAME} {PACKAGE_VERSION}")
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
}
