use std::collections::BTreeMap;

use serde::Serialize;

pub const CLI_API_VERSION: &str = "1.0.0";
pub const GENERATION_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const COMPOSITION_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const TEMPLATES_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const VALIDATION_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const REPORT_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const CASE_LIST_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const STANDARDS_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const CONFORMANCE_RESULT_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuccessEnvelope<T> {
    pub cli_api_version: &'static str,
    pub command: &'static str,
    pub status: &'static str,
    pub result: T,
}

impl<T> SuccessEnvelope<T> {
    pub fn new(command: &'static str, result: T) -> Self {
        Self {
            cli_api_version: CLI_API_VERSION,
            command,
            status: "success",
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnavailableCapabilitySummary {
    pub capability_id: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanonicalPlanPreview {
    pub artifact_count: usize,
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileProducingOutcome {
    pub requested_output_root: String,
    pub manifest_path: Option<String>,
    pub run_kind: &'static str,
    pub seed: u64,
    pub request_schema_version: String,
    pub manifest_schema_version: String,
    pub product_version: &'static str,
    pub emitted_artifact_count: usize,
    pub output_bytes: u64,
    pub unavailable_capability_count: usize,
    pub unavailable_capabilities: Vec<UnavailableCapabilitySummary>,
    pub corpus_plan_sha256: String,
    pub published: bool,
    pub publication_status: &'static str,
    pub validation_status: &'static str,
    pub plan_preview: Option<CanonicalPlanPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenerationResult {
    pub generation_result_schema_version: &'static str,
    #[serde(flatten)]
    pub outcome: FileProducingOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionResult {
    pub composition_result_schema_version: &'static str,
    #[serde(flatten)]
    pub outcome: FileProducingOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplatesResult<T> {
    pub templates_result_schema_version: &'static str,
    pub view: &'static str,
    pub template_count: usize,
    pub templates: Vec<T>,
}

impl<T> TemplatesResult<T> {
    pub fn new(view: &'static str, templates: Vec<T>) -> Self {
        Self {
            templates_result_schema_version: TEMPLATES_RESULT_SCHEMA_VERSION,
            view,
            template_count: templates.len(),
            templates,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationResult {
    pub validation_result_schema_version: &'static str,
    pub generated_root: String,
    pub manifest_path: String,
    pub files_checked: usize,
    pub valid: bool,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReportResult<T> {
    pub report_result_schema_version: &'static str,
    pub report_kind: String,
    pub report_schema_version: String,
    pub report: T,
}

impl<T> ReportResult<T> {
    pub fn new(
        report_kind: impl Into<String>,
        report_schema_version: impl Into<String>,
        report: T,
    ) -> Self {
        Self {
            report_result_schema_version: REPORT_RESULT_SCHEMA_VERSION,
            report_kind: report_kind.into(),
            report_schema_version: report_schema_version.into(),
            report,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseListResult {
    pub case_list_result_schema_version: &'static str,
    pub profile_filter: Option<String>,
    pub status_filter: Option<String>,
    pub case_count: usize,
    pub cases: Vec<crate::CaseListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandardsResult<T> {
    pub standards_result_schema_version: &'static str,
    pub operation: &'static str,
    pub record_count: usize,
    pub records: Vec<T>,
}

impl<T> StandardsResult<T> {
    pub fn new(operation: &'static str, records: Vec<T>) -> Self {
        Self {
            standards_result_schema_version: STANDARDS_RESULT_SCHEMA_VERSION,
            operation,
            record_count: records.len(),
            records,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConformanceResult<T> {
    pub conformance_result_schema_version: &'static str,
    pub operation: &'static str,
    pub outcome: T,
}

impl<T> ConformanceResult<T> {
    pub fn new(operation: &'static str, outcome: T) -> Self {
        Self {
            conformance_result_schema_version: CONFORMANCE_RESULT_SCHEMA_VERSION,
            operation,
            outcome,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ErrorContextValue {
    String(String),
    Integer(i64),
    Number(f64),
    Boolean(bool),
    Null,
    Strings(Vec<String>),
    Integers(Vec<i64>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicError {
    pub code: &'static str,
    pub message: String,
    pub context: BTreeMap<String, ErrorContextValue>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ErrorEnvelope {
    pub cli_api_version: &'static str,
    pub command: String,
    pub status: &'static str,
    pub error: PublicError,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CliFailure {
    pub command: String,
    pub exit: u8,
    pub error: PublicError,
}

impl CliFailure {
    pub fn classify(command: impl Into<String>, message: impl Into<String>) -> Self {
        let command = command.into();
        let message = message.into();
        let normalized = message.to_ascii_lowercase();
        let (code, exit, retryable) = if normalized.contains("product resource integrity failed") {
            ("evidence.integrity.failed", 5, false)
        } else if normalized.contains("destination") && normalized.contains("exist") {
            ("output.destination.exists", 4, false)
        } else if normalized.contains("unsafe")
            && (normalized.contains("path") || normalized.contains("traversal"))
        {
            ("output.path.unsafe", 4, false)
        } else if normalized.contains("unavailable") {
            ("capability.runtime.unavailable", 3, false)
        } else if normalized.starts_with("unknown ")
            || normalized.starts_with("unsupported ")
            || normalized.contains("must be non-zero")
        {
            ("command.syntax.invalid", 2, false)
        } else if normalized.contains(" requires ")
            || normalized.starts_with("requires ")
            || normalized.ends_with(" is required")
        {
            ("command.argument.missing", 2, false)
        } else if normalized.contains("validation failed")
            || normalized.contains("verification failed")
            || normalized.contains("conformance failed")
        {
            ("validation.artifact.failed", 5, false)
        } else if normalized.contains("read ") || normalized.contains("write ") {
            ("io.read.failed", 6, true)
        } else {
            ("internal.invariant.failed", 6, false)
        };
        Self {
            command,
            exit,
            error: PublicError {
                code,
                message,
                context: BTreeMap::new(),
                retryable,
            },
        }
    }

    pub fn envelope(&self) -> ErrorEnvelope {
        ErrorEnvelope {
            cli_api_version: CLI_API_VERSION,
            command: self.command.clone(),
            status: "error",
            error: self.error.clone(),
        }
    }
}
