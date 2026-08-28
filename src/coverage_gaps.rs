use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

#[derive(Debug)]
pub enum CoverageGapError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Shape(String),
}

impl fmt::Display for CoverageGapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }
            Self::Shape(message) => write!(f, "invalid coverage registry: {message}"),
        }
    }
}

impl Error for CoverageGapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Shape(_) => None,
        }
    }
}

#[derive(Default)]
struct DimensionCases {
    implemented: BTreeSet<String>,
    planned: BTreeSet<String>,
    blocked: BTreeSet<String>,
    providers: BTreeSet<String>,
    has_active_plan: bool,
}

impl DimensionCases {
    fn record(&mut self, case_id: &str, status: &str, provider_id: &str, priority: Option<&str>) {
        match status {
            "implemented" => {
                self.implemented.insert(case_id.to_string());
            }
            "planned" => {
                self.planned.insert(case_id.to_string());
                self.has_active_plan |= priority != Some("later");
            }
            "blocked" | "skipped" => {
                self.blocked.insert(case_id.to_string());
            }
            _ => {}
        }
        self.providers.insert(provider_id.to_string());
    }

    fn state(&self) -> &'static str {
        if !self.implemented.is_empty() {
            "covered"
        } else if !self.blocked.is_empty() && self.planned.is_empty() {
            "blocked"
        } else if self.has_active_plan {
            "planned"
        } else {
            "deferred"
        }
    }

    fn to_json(&self, value: &str) -> Value {
        json!({
            "value": value,
            "coverage_state": self.state(),
            "logical_case_count": self.implemented.len() + self.planned.len() + self.blocked.len(),
            "implemented_case_ids": self.implemented.iter().collect::<Vec<_>>(),
            "planned_case_ids": self.planned.iter().collect::<Vec<_>>(),
            "blocked_case_ids": self.blocked.iter().collect::<Vec<_>>(),
            "provider_ids": self.providers.iter().collect::<Vec<_>>()
        })
    }
}

pub fn build_coverage_gap_report(
    registry_path: impl AsRef<Path>,
    standards_lock_path: impl AsRef<Path>,
) -> Result<Value, CoverageGapError> {
    let registry_path = registry_path.as_ref();
    let standards_lock_path = standards_lock_path.as_ref();
    let registry_bytes = read_bytes(registry_path)?;
    let standards_lock_bytes = read_bytes(standards_lock_path)?;
    let registry: Value =
        serde_json::from_slice(&registry_bytes).map_err(|source| CoverageGapError::Parse {
            path: registry_path.to_path_buf(),
            source,
        })?;
    crate::validate_case_registry_semantics(&registry).map_err(CoverageGapError::Shape)?;
    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| CoverageGapError::Shape("missing cases array".to_string()))?;
    let registry_schema_version = registry
        .get("case_registry_schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| CoverageGapError::Shape("missing schema version".to_string()))?;

    let mut status_counts = BTreeMap::new();
    let mut priority_counts = BTreeMap::new();
    let mut provider_counts = BTreeMap::new();
    let mut artifact_kind_counts = BTreeMap::new();
    let mut sop_classes = BTreeMap::new();
    let mut modalities = BTreeMap::new();
    let mut object_families = BTreeMap::new();
    let mut compatibility_axes = BTreeMap::new();
    let mut gaps = Vec::new();

    let mut sorted_cases = cases.iter().collect::<Vec<_>>();
    sorted_cases.sort_by_key(|case| case.get("case_id").and_then(Value::as_str).unwrap_or(""));
    for case in sorted_cases {
        let case_id = required_str(case, "case_id")?;
        let status = required_str(case, "status")?;
        let artifact_kind = required_str(case, "artifact_kind")?;
        let provider_id = pointer_str(case, "/provider/id", "provider id")?;
        let object_family = required_str(case, "object_family")?;
        let priority = case.pointer("/roadmap/priority").and_then(Value::as_str);

        increment(&mut status_counts, status);
        increment(&mut provider_counts, provider_id);
        increment(&mut artifact_kind_counts, artifact_kind);
        if let Some(priority) = priority {
            increment(&mut priority_counts, priority);
        }

        if status_contributes_to_dimensions(status) {
            record_dimension(
                &mut object_families,
                object_family,
                case_id,
                status,
                provider_id,
                priority,
            );
            if let Some(sop_class_uid) = case.get("sop_class_uid").and_then(Value::as_str) {
                record_dimension(
                    &mut sop_classes,
                    sop_class_uid,
                    case_id,
                    status,
                    provider_id,
                    priority,
                );
            }
            if let Some(modality) = case.get("modality").and_then(Value::as_str) {
                record_dimension(
                    &mut modalities,
                    modality,
                    case_id,
                    status,
                    provider_id,
                    priority,
                );
            }
        }
        let axes = string_array(case, "compatibility_axes")?;
        if status_contributes_to_dimensions(status) {
            for axis in &axes {
                record_dimension(
                    &mut compatibility_axes,
                    axis,
                    case_id,
                    status,
                    provider_id,
                    priority,
                );
            }
        }

        if matches!(status, "planned" | "blocked" | "skipped") {
            let blocker_codes = case
                .get("blockers")
                .and_then(Value::as_array)
                .map(|blockers| {
                    blockers
                        .iter()
                        .filter_map(|blocker| blocker.get("code").and_then(Value::as_str))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let fallback_rationale = case
                .pointer("/skip/message")
                .and_then(Value::as_str)
                .unwrap_or("Registry row is unavailable.");
            gaps.push(json!({
                "case_id": case_id,
                "status": status,
                "priority": priority.unwrap_or("later"),
                "delivery_phase": case.pointer("/roadmap/delivery_phase").and_then(Value::as_str).unwrap_or("unassigned"),
                "artifact_kind": artifact_kind,
                "provider_id": provider_id,
                "object_family": object_family,
                "compatibility_axes": axes,
                "blocker_codes": blocker_codes,
                "rationale": case.pointer("/roadmap/rationale").and_then(Value::as_str).unwrap_or(fallback_rationale)
            }));
        }
    }

    Ok(json!({
        "coverage_gap_report_schema_version": "0.1.0",
        "registry_schema_version": registry_schema_version,
        "registry_sha256": crate::sha256_hex(&registry_bytes),
        "standards_lock_sha256": crate::sha256_hex(&standards_lock_bytes),
        "counts": {
            "logical_cases": cases.len(),
            "distinct_sop_classes": sop_classes.len(),
            "distinct_modalities": modalities.len(),
            "distinct_object_families": object_families.len(),
            "distinct_compatibility_axes": compatibility_axes.len(),
            "statuses": status_counts,
            "priorities": priority_counts,
            "providers": provider_counts,
            "artifact_kinds": artifact_kind_counts
        },
        "dimensions": {
            "sop_classes": dimension_rows(&sop_classes),
            "modalities": dimension_rows(&modalities),
            "object_families": dimension_rows(&object_families),
            "compatibility_axes": dimension_rows(&compatibility_axes)
        },
        "gaps": gaps
    }))
}

pub fn render_coverage_gap_report_markdown(report: &Value) -> String {
    let mut output = String::from("# DICOM Registry Coverage Gap Report\n\n");
    output.push_str(&format!(
        "- Registry SHA-256: {}\n- Standards lock SHA-256: {}\n- Logical cases: {}\n",
        report
            .get("registry_sha256")
            .and_then(Value::as_str)
            .unwrap_or(""),
        report
            .get("standards_lock_sha256")
            .and_then(Value::as_str)
            .unwrap_or(""),
        report
            .pointer("/counts/logical_cases")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    ));

    output.push_str("\n## Counts\n\n| Dimension | Value |\n|---|---:|\n");
    for (label, pointer) in [
        ("SOP Classes", "/counts/distinct_sop_classes"),
        ("Modalities", "/counts/distinct_modalities"),
        ("Object Families", "/counts/distinct_object_families"),
        ("Compatibility Axes", "/counts/distinct_compatibility_axes"),
    ] {
        output.push_str(&format!(
            "| {label} | {} |\n",
            report.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
        ));
    }

    for (label, pointer) in [
        ("Statuses", "/counts/statuses"),
        ("Priorities", "/counts/priorities"),
        ("Providers", "/counts/providers"),
        ("Artifact Kinds", "/counts/artifact_kinds"),
    ] {
        output.push_str(&format!("\n### {label}\n\n| Value | Count |\n|---|---:|\n"));
        if let Some(values) = report.pointer(pointer).and_then(Value::as_object) {
            let mut values = values.iter().collect::<Vec<_>>();
            values.sort_by_key(|(key, _)| *key);
            for (key, value) in values {
                output.push_str(&format!(
                    "| {} | {} |\n",
                    markdown_cell(key),
                    value.as_u64().unwrap_or(0)
                ));
            }
        }
    }

    output.push_str("\n## Planned And Blocked Cases\n\n");
    output.push_str("| Case | Priority | Phase | Provider | Blockers |\n");
    output.push_str("|---|---|---|---|---|\n");
    if let Some(gaps) = report.get("gaps").and_then(Value::as_array) {
        for gap in gaps {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                markdown_cell(gap.get("case_id").and_then(Value::as_str).unwrap_or("")),
                markdown_cell(gap.get("priority").and_then(Value::as_str).unwrap_or("")),
                markdown_cell(
                    gap.get("delivery_phase")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                ),
                markdown_cell(gap.get("provider_id").and_then(Value::as_str).unwrap_or("")),
                gap.get("blocker_codes")
                    .and_then(Value::as_array)
                    .map(|values| values
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", "))
                    .map(|value| markdown_cell(&value))
                    .unwrap_or_default()
            ));
        }
    }
    output
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, CoverageGapError> {
    fs::read(path).map_err(|source| CoverageGapError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, CoverageGapError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CoverageGapError::Shape(format!("{field} must be a string")))
}

fn pointer_str<'a>(
    value: &'a Value,
    pointer: &str,
    label: &str,
) -> Result<&'a str, CoverageGapError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| CoverageGapError::Shape(format!("{label} must be a string")))
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, CoverageGapError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| CoverageGapError::Shape(format!("{field} must be an array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| CoverageGapError::Shape(format!("{field} items must be strings")))
        })
        .collect()
}

fn increment(counts: &mut BTreeMap<String, usize>, value: &str) {
    *counts.entry(value.to_string()).or_default() += 1;
}

fn status_contributes_to_dimensions(status: &str) -> bool {
    matches!(status, "implemented" | "planned" | "blocked" | "skipped")
}

fn record_dimension(
    dimensions: &mut BTreeMap<String, DimensionCases>,
    value: &str,
    case_id: &str,
    status: &str,
    provider_id: &str,
    priority: Option<&str>,
) {
    dimensions
        .entry(value.to_string())
        .or_default()
        .record(case_id, status, provider_id, priority);
}

fn dimension_rows(dimensions: &BTreeMap<String, DimensionCases>) -> Vec<Value> {
    dimensions
        .iter()
        .map(|(value, cases)| cases.to_json(value))
        .collect()
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::status_contributes_to_dimensions;

    #[test]
    fn deprecated_cases_do_not_create_empty_dimension_rows() {
        assert!(!status_contributes_to_dimensions("deprecated"));
        for status in ["implemented", "planned", "blocked", "skipped"] {
            assert!(status_contributes_to_dimensions(status));
        }
    }
}
