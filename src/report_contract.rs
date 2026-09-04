//! Version-aware validation for generated report documents.

use std::fmt;

use serde_json::Value;

const LEGACY_COVERAGE_ID: &str =
    "https://dicom-test-suite.local/schemas/coverage-report.schema.json";
const LEGACY_ASSEMBLY_ID: &str =
    "https://dicom-test-suite.local/schemas/structural-assembly-report.schema.json";
const LEGACY_COMPOSITION_ID: &str =
    "https://synth-dicom-gen.local/schemas/composition-report.schema.json";
const MANIFEST_V1_ID: &str = "https://synth-dicom-gen.local/schemas/manifest-v1.schema.json";
const COMPOSITION_MANIFEST_V1_ID: &str =
    "https://synth-dicom-gen.local/schemas/composition-manifest-v1.schema.json";
const ASSEMBLY_MANIFEST_V2_ID: &str =
    "https://synth-dicom-gen.local/schemas/structural-assembly-manifest-v2.schema.json";
const VERSION_V2_ID: &str = "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json";
const LEGACY_MANIFEST_ID: &str = "https://dicom-test-suite.local/schemas/manifest.schema.json";

#[derive(Debug)]
pub(crate) struct ReportContractError(String);

impl fmt::Display for ReportContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReportContractError {}

pub(crate) fn validate_report_contract(report: &Value) -> Result<(), ReportContractError> {
    let report_kind = report.get("report_kind").and_then(Value::as_str);
    if report_kind == Some("external_corpus") {
        return crate::corpus_report::validate(report).map_err(error);
    }
    let (version_field, supported_legacy, supported_current, schema_bytes) = match report_kind {
        None => (
            "coverage_report_schema_version",
            "0.1.0",
            "1.0.0",
            include_bytes!("../schemas/coverage-report-v1.schema.json").as_slice(),
        ),
        Some("composition") => (
            "composition_report_schema_version",
            "0.1.0",
            "1.0.0",
            include_bytes!("../schemas/composition-report-v1.schema.json").as_slice(),
        ),
        Some("structural_assembly") => (
            "structural_assembly_report_schema_version",
            "1.0.0",
            "2.0.0",
            include_bytes!("../schemas/structural-assembly-report-v2.schema.json").as_slice(),
        ),
        Some(kind) => return Err(error(format!("unsupported report kind {kind}"))),
    };
    let version = report
        .get(version_field)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("{version_field} must be a string")))?;
    let schema_bytes = if version == supported_current {
        schema_bytes
    } else if version == supported_legacy {
        match report_kind {
            None => include_bytes!("../schemas/coverage-report.schema.json").as_slice(),
            Some("composition") => {
                include_bytes!("../schemas/composition-report.schema.json").as_slice()
            }
            Some("structural_assembly") => {
                include_bytes!("../schemas/structural-assembly-report.schema.json").as_slice()
            }
            Some(_) => unreachable!(),
        }
    } else {
        return Err(error(format!("unsupported {version_field} {version}")));
    };
    let schema: Value = serde_json::from_slice(schema_bytes)
        .map_err(|failure| error(format!("report schema JSON invalid: {failure}")))?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_resource(
            LEGACY_COVERAGE_ID,
            resource(include_bytes!("../schemas/coverage-report.schema.json"))?,
        )
        .with_resource(
            LEGACY_ASSEMBLY_ID,
            resource(include_bytes!(
                "../schemas/structural-assembly-report.schema.json"
            ))?,
        )
        .with_resource(
            LEGACY_COMPOSITION_ID,
            resource(include_bytes!("../schemas/composition-report.schema.json"))?,
        )
        .with_resource(
            MANIFEST_V1_ID,
            resource(include_bytes!("../schemas/manifest-v1.schema.json"))?,
        )
        .with_resource(
            COMPOSITION_MANIFEST_V1_ID,
            resource(include_bytes!(
                "../schemas/composition-manifest-v1.schema.json"
            ))?,
        )
        .with_resource(
            ASSEMBLY_MANIFEST_V2_ID,
            resource(include_bytes!(
                "../schemas/structural-assembly-manifest-v2.schema.json"
            ))?,
        )
        .with_resource(
            VERSION_V2_ID,
            resource(include_bytes!("../schemas/version-result-v2.schema.json"))?,
        )
        .with_resource(
            LEGACY_MANIFEST_ID,
            resource(include_bytes!("../schemas/manifest.schema.json"))?,
        )
        .build(&schema)
        .map_err(|failure| error(format!("report schema compilation failed: {failure}")))?;
    if let Err(failure) = validator.validate(report) {
        return Err(error(format!("report schema invalid: {failure}")));
    }
    if version == supported_current {
        crate::manifest_contract::validate_identity_projection_runtime_uniqueness(report)
            .map_err(|failure| error(failure.to_string()))?;
    }
    Ok(())
}

fn resource(bytes: &[u8]) -> Result<jsonschema::Resource, ReportContractError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|failure| error(format!("report dependency schema invalid: {failure}")))?;
    jsonschema::Resource::from_contents(value).map_err(|failure| error(failure.to_string()))
}

fn error(message: impl Into<String>) -> ReportContractError {
    ReportContractError(message.into())
}

#[cfg(test)]
mod report_contract_tests {
    use super::*;

    fn current_report(legacy_bytes: &[u8], version_field: &str, version: &str) -> Value {
        let mut report: Value = serde_json::from_slice(legacy_bytes).unwrap();
        let projection = crate::identity::project_manifest_identities(
            &crate::engine_resources::EngineResources::embedded(),
            None,
            Vec::new(),
        )
        .unwrap();
        report[version_field] = version.into();
        report["identity_projection"] = serde_json::to_value(projection).unwrap();
        report
    }

    #[test]
    fn legacy_report_fixtures_remain_schema_valid() {
        for bytes in [
            include_bytes!("../tests/fixtures/cli/coverage-report-v0.1.json").as_slice(),
            include_bytes!("../tests/fixtures/cli/composition-report-v0.1.json").as_slice(),
            include_bytes!("../tests/fixtures/cli/structural-assembly-report-v1.json").as_slice(),
        ] {
            let report: Value = serde_json::from_slice(bytes).unwrap();
            validate_report_contract(&report).unwrap();
            assert!(report.get("identity_projection").is_none());
        }
    }

    #[test]
    fn current_reports_validate_and_preserve_every_legacy_field() {
        for (bytes, version_field, legacy_version, current_version) in [
            (
                include_bytes!("../tests/fixtures/cli/coverage-report-v0.1.json").as_slice(),
                "coverage_report_schema_version",
                "0.1.0",
                "1.0.0",
            ),
            (
                include_bytes!("../tests/fixtures/cli/composition-report-v0.1.json").as_slice(),
                "composition_report_schema_version",
                "0.1.0",
                "1.0.0",
            ),
            (
                include_bytes!("../tests/fixtures/cli/structural-assembly-report-v1.json")
                    .as_slice(),
                "structural_assembly_report_schema_version",
                "1.0.0",
                "2.0.0",
            ),
        ] {
            let legacy: Value = serde_json::from_slice(bytes).unwrap();
            let current = current_report(bytes, version_field, current_version);
            validate_report_contract(&current).unwrap();

            let mut normalized = current;
            normalized
                .as_object_mut()
                .unwrap()
                .remove("identity_projection");
            normalized[version_field] = legacy_version.into();
            assert_eq!(normalized, legacy);
        }
    }

    #[test]
    fn current_report_contracts_reject_identity_and_version_mutations() {
        for (bytes, version_field, current_version) in [
            (
                include_bytes!("../tests/fixtures/cli/coverage-report-v0.1.json").as_slice(),
                "coverage_report_schema_version",
                "1.0.0",
            ),
            (
                include_bytes!("../tests/fixtures/cli/composition-report-v0.1.json").as_slice(),
                "composition_report_schema_version",
                "1.0.0",
            ),
            (
                include_bytes!("../tests/fixtures/cli/structural-assembly-report-v1.json")
                    .as_slice(),
                "structural_assembly_report_schema_version",
                "2.0.0",
            ),
        ] {
            let report = current_report(bytes, version_field, current_version);

            let mut unknown = report.clone();
            unknown[version_field] = "99.0.0".into();
            assert!(validate_report_contract(&unknown).is_err());

            let mut missing = report.clone();
            missing
                .as_object_mut()
                .unwrap()
                .remove("identity_projection");
            assert!(validate_report_contract(&missing).is_err());

            let mut malformed = report.clone();
            malformed["identity_projection"]["engine"]["engine_sha256"] = "A".repeat(64).into();
            assert!(validate_report_contract(&malformed).is_err());

            let mut duplicate = report;
            let first = serde_json::json!({
                "runtime_id": "duplicate-runtime",
                "runtime_kind": "frame_codec",
                "executable_sha256": "a".repeat(64),
                "version": "1.0.0",
                "invocation_sha256": "b".repeat(64)
            });
            let mut second = first.clone();
            second["invocation_sha256"] = "c".repeat(64).into();
            duplicate["identity_projection"]["external_runtime"] =
                serde_json::json!([first, second]);
            assert!(validate_report_contract(&duplicate).is_err());
        }
    }
}
