use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::{
    CanonicalContent, GenericPlanValidator, IdentityPlan, MaterializedReference, ResolvedAttribute,
    ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use crate::sha256_hex;

pub fn validate_composition_root(root: &Path, manifest: &Value) -> (usize, Vec<String>) {
    let mut failures = Vec::new();
    if let Err(error) = super::manifest::validate_manifest_schema(manifest) {
        failures.push(error.to_string());
        return (0, failures);
    }
    let entries = manifest
        .pointer("/composition/entries")
        .and_then(Value::as_array)
        .expect("composition manifest schema requires entries");
    let mut paths = BTreeSet::new();
    for entry in entries {
        let instance_id = entry["instance_id"].as_str().unwrap_or("unknown");
        let relative_path = entry["path"].as_str().expect("schema requires path");
        if !paths.insert(relative_path) {
            failures.push(format!(
                "{instance_id}: duplicate output path {relative_path}"
            ));
            continue;
        }
        let path = root.join(relative_path);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                failures.push(format!("{instance_id}: read {relative_path}: {error}"));
                continue;
            }
        };
        if entry["size_bytes"].as_u64() != Some(bytes.len() as u64) {
            failures.push(format!("{instance_id}: output size differs from manifest"));
        }
        let actual_sha = sha256_hex(&bytes);
        if entry["sha256"].as_str() != Some(actual_sha.as_str()) {
            failures.push(format!(
                "{instance_id}: output SHA-256 differs from manifest"
            ));
        }
        let external_import = entry["construction_origin"] == "external_provider_import";
        let plan = match plan_from_entry(entry, external_import) {
            Ok(plan) => plan,
            Err(error) => {
                failures.push(format!("{instance_id}: reconstruct resolved plan: {error}"));
                continue;
            }
        };
        let plan_sha = plan.canonical_sha256();
        if !external_import && entry["resolved_plan_sha256"].as_str() != Some(plan_sha.as_str()) {
            failures.push(format!(
                "{instance_id}: resolved plan SHA-256 differs from manifest"
            ));
        }
        failures.extend(
            GenericPlanValidator
                .validate_file(&plan, &path)
                .into_iter()
                .filter(|check| check.status == "failed")
                .map(|check| format!("{instance_id}: {}: {}", check.rule_id, check.message)),
        );
    }
    let actual_paths = fs::read_dir(root.join("instances"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| format!("instances/{}", entry.file_name().to_string_lossy()))
        .collect::<BTreeSet<_>>();
    let declared_paths = paths
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for extra in actual_paths.difference(&declared_paths) {
        failures.push(format!("undeclared composition output {extra}"));
    }
    (entries.len(), failures)
}

fn plan_from_entry(
    entry: &Value,
    external_import: bool,
) -> Result<ResolvedInstancePlan, serde_json::Error> {
    Ok(ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: entry["instance_id"].as_str().unwrap().into(),
        template_id: serde_json::from_value::<TemplateId>(entry["template_id"].clone())?,
        template_version: serde_json::from_value::<TemplateVersion>(
            entry["template_version"].clone(),
        )?,
        sop_class_uid: entry["dicom"]["sop_class_uid"].as_str().unwrap().into(),
        transfer_syntax_uid: entry["dicom"]["transfer_syntax_uid"]
            .as_str()
            .unwrap()
            .into(),
        identities: IdentityPlan {
            logical_instance_id: entry["instance_id"].as_str().unwrap().into(),
            identities: serde_json::from_value::<BTreeMap<String, String>>(entry["uids"].clone())?,
        },
        attributes: serde_json::from_value::<Vec<ResolvedAttribute>>(
            entry["resolved_attributes"].clone(),
        )?,
        content: if external_import {
            Vec::new()
        } else {
            serde_json::from_value::<Vec<CanonicalContent>>(entry["content"].clone())?
        },
        references: serde_json::from_value::<Vec<MaterializedReference>>(
            entry["references"].clone(),
        )?,
    })
}

pub fn composition_report(manifest: &Value) -> Result<Value, String> {
    let entries = manifest
        .pointer("/composition/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut templates = BTreeMap::<String, u64>::new();
    let mut transfer_syntaxes = BTreeMap::<String, u64>::new();
    let mut total_bytes = 0_u64;
    let rows = entries
        .iter()
        .map(|entry| {
            let template = format!(
                "{}@{}",
                entry["template_id"].as_str().unwrap_or("unknown"),
                entry["template_version"].as_str().unwrap_or("unknown")
            );
            *templates.entry(template.clone()).or_default() += 1;
            let syntax = entry["dicom"]["transfer_syntax_uid"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            *transfer_syntaxes.entry(syntax.clone()).or_default() += 1;
            total_bytes += entry["size_bytes"].as_u64().unwrap_or(0);
            json!({
                "instance_id": entry["instance_id"],
                "template": template,
                "sop_class_uid": entry["dicom"]["sop_class_uid"],
                "transfer_syntax_uid": syntax,
                "path": entry["path"],
                "size_bytes": entry["size_bytes"],
                "sha256": entry["sha256"],
                "validation_status": entry["validation"]["status"],
                "content_slots": entry["content"].as_array().map(Vec::len).unwrap_or(0),
                "references": entry["references"].as_array().map(Vec::len).unwrap_or(0)
            })
        })
        .collect::<Vec<_>>();
    let current_identity_projection =
        match manifest["manifest_schema_version"].as_str() {
            Some("0.4.0" | "0.5.0") => None,
            Some("1.0.0") => Some(manifest.get("identity_projection").cloned().ok_or_else(
                || "composition manifest 1.0 identity_projection missing".to_string(),
            )?),
            Some(version) => {
                return Err(format!(
                    "unsupported composition manifest version {version}"
                ));
            }
            None => return Err("composition manifest schema version missing".to_string()),
        };
    let mut report = json!({
        "composition_report_schema_version": "0.1.0",
        "report_kind": "composition",
        "generated_at": manifest["generated_at"],
        "standards_lock_sha256": manifest["standards"]["standards_lock_sha256"],
        "counts": {
            "instances": entries.len(),
            "output_bytes": total_bytes,
            "unavailable_capabilities": manifest["composition"]["unavailable_capabilities"].as_array().map(Vec::len).unwrap_or(0)
        },
        "templates": templates,
        "transfer_syntaxes": transfer_syntaxes,
        "instances": rows
    });
    if let Some(identity_projection) = current_identity_projection {
        report["composition_report_schema_version"] = "1.0.0".into();
        report["identity_projection"] = identity_projection;
    }
    Ok(report)
}

pub fn render_composition_report_markdown(report: &Value) -> String {
    let mut output = String::from("# DICOM Composition Report\n\n");
    output.push_str(&format!(
        "- Instances: {}\n- Output bytes: {}\n\n",
        report["counts"]["instances"].as_u64().unwrap_or(0),
        report["counts"]["output_bytes"].as_u64().unwrap_or(0)
    ));
    output.push_str("| Instance | Template | SOP Class UID | Transfer Syntax UID | Validation |\n");
    output.push_str("|---|---|---|---|---|\n");
    for entry in report["instances"].as_array().into_iter().flatten() {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            entry["instance_id"].as_str().unwrap_or(""),
            entry["template"].as_str().unwrap_or(""),
            entry["sop_class_uid"].as_str().unwrap_or(""),
            entry["transfer_syntax_uid"].as_str().unwrap_or(""),
            entry["validation_status"].as_str().unwrap_or("")
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::composition::{ComposeOptions, compose};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn validates_and_reports_composition_without_registry_axes() {
        let root = std::env::temp_dir().join(format!(
            "dts-composition-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let (_, manifest) = compose(&ComposeOptions {
            spec_path: "tests/fixtures/composition/valid/template-only.json".into(),
            out_dir: root.clone(),
            seed: 1,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        let (count, failures) = validate_composition_root(&root, &manifest);
        assert_eq!(count, 1);
        assert!(failures.is_empty(), "{failures:?}");
        let report = composition_report(&manifest).unwrap();
        assert_eq!(report["report_kind"], "composition");
        assert!(!report.to_string().contains("case_id"));
        assert!(!report.to_string().contains("profile"));
        fs::remove_dir_all(root).unwrap();
    }
}
