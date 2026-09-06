//! Pure reporting over validated caller-owned manifest evidence.

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(crate) fn project(manifest: &Value) -> Result<Value, String> {
    crate::manifest_contract::validate_external_corpus_manifest(manifest)
        .map_err(|e| e.to_string())?;
    let ledger = manifest["selection_ledger"].as_array().unwrap();
    let files = manifest["files"].as_array().unwrap();
    let mut metadata_failures = Vec::new();
    crate::metadata::validate_manifest_metadata_corpus(
        crate::manifest_contract::ManifestContractKind::ExternalCorpus,
        files,
        &mut metadata_failures,
    );
    if !metadata_failures.is_empty() {
        return Err(metadata_failures.join("; "));
    }

    let mut outcomes = BTreeMap::<String, usize>::new();
    for row in ledger {
        *outcomes
            .entry(row["outcome"].as_str().unwrap().into())
            .or_default() += 1;
    }
    let direct = ledger.iter().filter(|r| r["selection"] == "direct").count();
    let profile = manifest["run"]["profile"].as_str().unwrap();
    let mut coverage_matrix = Vec::with_capacity(files.len());
    let mut grouped_coverage = crate::GroupedCoverage::default();
    for file in files {
        crate::vl::validate_manifest(file)?;
        crate::encapsulated::validate_manifest(file)?;
        crate::waveform::validate_manifest(file)?;
        // Report2 is declaration-driven. Suppress legacy report1's curated
        // case-name inference, then restore the caller's identity fields.
        let case_id = file["case_id"].as_str().unwrap();
        let mut projection_file = file.clone();
        projection_file["case_id"] = "external/report2/declaration-driven".into();
        if projection_file["recipe"]["recipe_parameters"]
            .get("encapsulated_contract")
            .is_some()
        {
            projection_file["recipe"]["recipe_parameters"]["encapsulated_contract"]["case_id"] =
                projection_file["case_id"].clone();
        }
        if projection_file["recipe"]["recipe_parameters"]
            .get("waveform_contract")
            .is_some()
        {
            projection_file["recipe"]["recipe_parameters"]["waveform_contract"]["case_id"] =
                projection_file["case_id"].clone();
        }
        let mut row = crate::generated_external_coverage_row(
            Path::new("manifest.json"),
            &projection_file,
            profile,
        )
        .map_err(|error| error.to_string())?;
        row["case_id"] = case_id.into();
        row["object_type"] = case_id.split('/').next().unwrap_or(case_id).into();
        grouped_coverage.record(&row);
        coverage_matrix.push(row);
    }
    Ok(json!({
        "coverage_report_schema_version":"2.0.0", "report_kind":"external_corpus",
        "evidence":{"class":"manifest_projection","validation":"not_assessed","independent_conformance":"not_assessed","payloads_reopened":false},
        "identity_projection":manifest["identity_projection"], "source_manifest":manifest,
        "summary":{"logical_cases":ledger.len(),"direct_cases":direct,"dependency_cases":ledger.len()-direct,"emitted_files":files.len(),"qualifications":manifest["qualifications"].as_array().unwrap().len(),"outcomes":outcomes},
        "case_dimensions":dimensions(ledger.iter().map(|row| (row["case_id"].as_str().unwrap(), &row["case_definition"])), false),
        "artifact_dimensions":dimensions(files.iter().map(|file| (file["path"].as_str().unwrap(), file)), true),
        "coverage_matrix":coverage_matrix,
        "grouped_coverage":grouped_coverage.to_json()
    }))
}

fn dimensions<'a>(rows: impl Iterator<Item = (&'a str, &'a Value)>, artifact: bool) -> Value {
    let mut groups = BTreeMap::<&str, BTreeMap<String, BTreeSet<String>>>::new();
    for key in [
        "profiles",
        "modalities",
        "sop_classes",
        "transfer_syntaxes",
        "determinism",
        "providers",
    ] {
        groups.insert(key, BTreeMap::new());
    }
    for (member, row) in rows {
        let mut add = |dimension, value: String| {
            groups
                .get_mut(dimension)
                .unwrap()
                .entry(value)
                .or_default()
                .insert(member.to_owned());
        };
        let profiles = if artifact {
            &row["profile_membership"]
        } else {
            &row["profiles"]
        };
        for profile in profiles.as_array().into_iter().flatten() {
            add("profiles", profile.as_str().unwrap().into());
        }
        for (dimension, value) in [
            (
                "modalities",
                if artifact {
                    &row["dicom"]["modality"]
                } else {
                    &row["modality"]
                },
            ),
            (
                "sop_classes",
                if artifact {
                    &row["dicom"]["sop_class_uid"]
                } else {
                    &row["sop_class_uid"]
                },
            ),
            (
                "transfer_syntaxes",
                if artifact {
                    &row["dicom"]["transfer_syntax_uid"]
                } else {
                    &row["transfer_syntax_uid"]
                },
            ),
            ("determinism", &row["determinism"]),
            ("providers", &row["provider"]),
        ] {
            add(dimension, value.to_string());
        }
    }
    serde_json::to_value(groups.into_iter().map(|(dimension, values)| (dimension,values.into_iter().map(|(value,members)| json!({"value":value,"count":members.len(),"members":members})).collect::<Vec<_>>())).collect::<BTreeMap<_,_>>()).unwrap()
}

pub(crate) fn validate(report: &Value) -> Result<(), String> {
    let schema: Value =
        serde_json::from_slice(include_bytes!("../schemas/coverage-report-v2.schema.json"))
            .map_err(|e| e.to_string())?;
    let mut options = jsonschema::options().with_draft(jsonschema::Draft::Draft202012);
    for bytes in [
        include_bytes!("../schemas/manifest-v2.schema.json").as_slice(),
        include_bytes!("../schemas/manifest-v1.schema.json").as_slice(),
        include_bytes!("../schemas/manifest.schema.json").as_slice(),
        include_bytes!("../schemas/version-result-v2.schema.json").as_slice(),
        include_bytes!("../schemas/case-registry.schema.json").as_slice(),
        include_bytes!("../schemas/coverage-report.schema.json").as_slice(),
    ] {
        let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let id = value["$id"].as_str().unwrap().to_owned();
        options = options.with_resource(
            id,
            jsonschema::Resource::from_contents(value).map_err(|e| e.to_string())?,
        );
    }
    options
        .build(&schema)
        .map_err(|e| e.to_string())?
        .validate(report)
        .map_err(|e| e.to_string())?;
    if project(&report["source_manifest"])? != *report {
        return Err("external report differs from its source evidence projection".into());
    }
    Ok(())
}

pub(crate) fn markdown(report: &Value) -> String {
    if let Err(error) = validate(report) {
        return format!(
            "External corpus report invalid: {}\n",
            crate::markdown_cell(Some(&error))
        );
    }
    let summary = &report["summary"];
    let source = &report["source_manifest"];
    let mut text = format!("# External corpus evidence report\n\nManifest projection only. No new validation or independent conformance assessment is performed by reporting; recorded source results are retained. Report-level assessment: **not_assessed**. Payloads were not reopened.\n\nProfile: {}. Selector: {}. Verified corpus digest: {}.\n\nLogical cases: {}; direct: {}; dependencies: {}. Emitted files: {}; qualifications: {}. Case counts are not artifact counts.\n\n| Case outcome | Count |\n| --- | ---: |\n",crate::markdown_cell(source["run"]["profile"].as_str()),crate::markdown_cell(Some(&source["run"]["selector"].to_string())),crate::markdown_cell(report["identity_projection"]["corpus_definition"]["identity"]["corpus_definition_sha256"].as_str()),summary["logical_cases"],summary["direct_cases"],summary["dependency_cases"],summary["emitted_files"],summary["qualifications"]);
    for (outcome, count) in summary["outcomes"].as_object().unwrap() {
        text.push_str(&format!(
            "| {} | {count} |\n",
            crate::markdown_cell(Some(outcome))
        ));
    }
    text.push_str("\n## Captured cases\n\n| Case | Profiles | Selection | Outcome | Reason |\n| --- | --- | --- | --- | --- |\n");
    for row in source["selection_ledger"].as_array().unwrap() {
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            crate::markdown_cell(row["case_id"].as_str()),
            crate::markdown_cell(Some(&row["case_definition"]["profiles"].to_string())),
            crate::markdown_cell(row["selection"].as_str()),
            crate::markdown_cell(row["outcome"].as_str()),
            crate::markdown_cell(row["reason_code"].as_str())
        ));
    }
    text.push_str("\nThe JSON report retains the complete source manifest, identities, definitions, files, qualifications and validation evidence. This report does not upgrade those claims.\n");
    text
}

#[cfg(test)]
#[path = "../tests/captured_corpus_report.rs"]
mod captured_report_tests;
