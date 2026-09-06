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
    let schema_bytes = if report_kind.is_none() && version == "1.1.0" {
        include_bytes!("../schemas/coverage-report-v1.1.schema.json").as_slice()
    } else if version == supported_current {
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
    if version == supported_current || (report_kind.is_none() && version == "1.1.0") {
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

    const NONSQUARE_FIELDS: [&str; 7] = [
        "nonsquare_variant_id",
        "nonsquare_pixel_spacing",
        "nonsquare_nominal_scanned_pixel_spacing",
        "nonsquare_pixel_aspect_ratio",
        "nonsquare_uncalibrated",
        "nonsquare_patient_space_geometry_present",
        "nonsquare_pixel_data_sha256",
    ];

    fn nonsquare_report(status: &str, variant: &str) -> Value {
        let mut report = current_report(
            include_bytes!("../tests/fixtures/cli/coverage-report-v0.1.json"),
            "coverage_report_schema_version",
            "1.1.0",
        );
        let row = &mut report["coverage_matrix"][0];
        row["case_id"] = "classic/sc/nonsquare_pixel_spacing".into();
        row["status"] = status.into();
        row["nonsquare_variant_id"] = variant.into();
        row["nonsquare_uncalibrated"] = true.into();
        row["nonsquare_patient_space_geometry_present"] = false.into();
        row["nonsquare_pixel_data_sha256"] =
            "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6".into();
        if variant == "pixel_spacing" {
            row["nonsquare_pixel_spacing"] = "0.6\\0.3".into();
            row["nonsquare_nominal_scanned_pixel_spacing"] = "0.6\\0.3".into();
            row["nonsquare_pixel_aspect_ratio"] = Value::Null;
        } else {
            row["nonsquare_pixel_spacing"] = Value::Null;
            row["nonsquare_nominal_scanned_pixel_spacing"] = Value::Null;
            row["nonsquare_pixel_aspect_ratio"] = "2\\1".into();
        }
        report
    }

    #[test]
    fn coverage_1_1_preserves_original_rows_and_other_case_guards_exactly() {
        let legacy: Value =
            serde_json::from_slice(include_bytes!("../schemas/coverage-report.schema.json"))
                .unwrap();
        let current: Value =
            serde_json::from_slice(include_bytes!("../schemas/coverage-report-v1.schema.json"))
                .unwrap();
        let new: Value = serde_json::from_slice(include_bytes!(
            "../schemas/coverage-report-v1.1.schema.json"
        ))
        .unwrap();
        fn explicit_references(value: &mut Value) {
            match value {
                Value::Object(object) => {
                    for (key, value) in object {
                        if key == "$ref" && value.as_str().is_some_and(|s| s.starts_with('#')) {
                            *value =
                                format!("{LEGACY_COVERAGE_ID}{}", value.as_str().unwrap()).into();
                        } else {
                            explicit_references(value);
                        }
                    }
                }
                Value::Array(values) => values.iter_mut().for_each(explicit_references),
                _ => {}
            }
        }
        let mut expected = legacy["$defs"]["coverage_row"].clone();
        explicit_references(&mut expected);
        let mut actual = new["$defs"]["coverage_row"].clone();
        let generated_only_cases = [
            "classic/sc/mono2_u1_native",
            "classic/sc/mono2_u32_explicit_le",
            "derived/mesh/encapsulated_stl",
            "derived/registration/spatial_ct_pair",
            "non-image/waveform/twelve_lead_ecg",
            "vl/photo/rgb_icc_profile_explicit_le",
        ];
        let mut normalized_status_guards = Vec::new();
        for rule in actual["allOf"].as_array_mut().unwrap() {
            let Some(case_id) = rule
                .pointer("/if/properties/case_id/const")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if !generated_only_cases.contains(&case_id.as_str()) {
                continue;
            }
            assert_eq!(
                rule.pointer("/if/properties/status/const"),
                Some(&Value::String("generated".into()))
            );
            assert_eq!(
                rule.pointer("/if/required"),
                Some(&serde_json::json!(["case_id", "status"]))
            );
            rule["if"]["properties"]
                .as_object_mut()
                .unwrap()
                .remove("status");
            if case_id == "derived/mesh/encapsulated_stl" {
                rule["if"]["required"] = serde_json::json!(["case_id"]);
            } else {
                rule["if"].as_object_mut().unwrap().remove("required");
            }
            normalized_status_guards.push(case_id);
        }
        normalized_status_guards.sort();
        assert_eq!(normalized_status_guards, generated_only_cases);
        actual["allOf"][15] = actual["allOf"][15]["anyOf"][0].clone();
        assert_eq!(
            actual, expected,
            "only the reviewed nonsquare alternative and generated-status guards may change"
        );
        let mut normalized = new.clone();
        normalized.as_object_mut().unwrap().remove("$defs");
        for field in ["$id", "title"] {
            normalized[field] = current[field].clone();
        }
        for field in ["coverage_report_schema_version", "coverage_matrix"] {
            normalized["properties"][field] = current["properties"][field].clone();
        }
        assert_eq!(normalized, current);
        for status in [
            "generated",
            "planned",
            "skipped",
            "blocked",
            "deprecated",
            "unavailable",
        ] {
            for variant in ["pixel_spacing", "pixel_aspect_ratio"] {
                let mut report = nonsquare_report(status, variant);
                validate_report_contract(&report).unwrap();
                report["coverage_report_schema_version"] = "1.0.0".into();
                validate_report_contract(&report).unwrap();
            }
        }
    }

    #[test]
    fn coverage_1_1_non_generated_null_alternative_is_closed() {
        for status in ["planned", "skipped", "blocked", "deprecated", "unavailable"] {
            let mut report = nonsquare_report(status, "pixel_spacing");
            for field in NONSQUARE_FIELDS {
                report["coverage_matrix"][0][field] = Value::Null;
            }
            validate_report_contract(&report).unwrap();
            let mut old = report.clone();
            old["coverage_report_schema_version"] = "1.0.0".into();
            assert!(
                validate_report_contract(&old).is_err(),
                "frozen reader must stay frozen"
            );
            for invalid in ["generated", "unknown", "not_run"] {
                let mut mutated = report.clone();
                mutated["coverage_matrix"][0]["status"] = invalid.into();
                assert!(validate_report_contract(&mutated).is_err());
            }
            for field in NONSQUARE_FIELDS {
                let mut missing = report.clone();
                missing["coverage_matrix"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                assert!(validate_report_contract(&missing).is_err());
                let mut partial = report.clone();
                partial["coverage_matrix"][0][field] = "invented-observation".into();
                assert!(validate_report_contract(&partial).is_err());
            }
            report["coverage_matrix"][0]
                .as_object_mut()
                .unwrap()
                .remove("status");
            assert!(validate_report_contract(&report).is_err());
        }
    }

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
    fn coverage_1_1_artifact_specific_rows_are_status_aware() {
        const GENERATED_ONLY_CASES: [&str; 6] = [
            "classic/sc/mono2_u1_native",
            "classic/sc/mono2_u32_explicit_le",
            "derived/mesh/encapsulated_stl",
            "derived/registration/spatial_ct_pair",
            "non-image/waveform/twelve_lead_ecg",
            "vl/photo/rgb_icc_profile_explicit_le",
        ];

        for case_id in GENERATED_ONLY_CASES {
            let mut unavailable = current_report(
                include_bytes!("../tests/fixtures/cli/coverage-report-v0.1.json"),
                "coverage_report_schema_version",
                "1.1.0",
            );
            let row = &mut unavailable["coverage_matrix"][0];
            row["case_id"] = case_id.into();
            row["status"] = "unavailable".into();
            row["reason_code"] = "generator_not_implemented".into();
            row["validation_status"] = "unavailable".into();
            validate_report_contract(&unavailable).unwrap_or_else(|error| {
                panic!("1.1 unavailable projection for {case_id} must validate: {error}")
            });

            let mut missing_generated_observations = unavailable;
            missing_generated_observations["coverage_matrix"][0]["status"] = "generated".into();
            missing_generated_observations["coverage_matrix"][0]["reason_code"] = Value::Null;
            missing_generated_observations["coverage_matrix"][0]["validation_status"] =
                "passed".into();
            assert!(
                validate_report_contract(&missing_generated_observations).is_err(),
                "generated {case_id} must retain its exact artifact observations"
            );
        }
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
                "1.1.0",
            ),
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
                "1.1.0",
            ),
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
