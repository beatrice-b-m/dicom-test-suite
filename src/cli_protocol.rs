use std::collections::BTreeMap;

use serde::Serialize;

pub const CLI_API_VERSION: &str = "1.0.0";
pub const GENERATION_RESULT_SCHEMA_VERSION: &str = "2.0.0";
pub const COMPOSITION_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const ASSEMBLY_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const TEMPLATES_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const VALIDATION_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const REPORT_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const CASE_LIST_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const STANDARDS_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const CONFORMANCE_RESULT_SCHEMA_VERSION: &str = "1.0.0";
pub const INTEROPERABILITY_RESULT_SCHEMA_VERSION: &str = "1.0.0";

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
pub struct AssemblyResult {
    pub assembly_result_schema_version: &'static str,
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
pub struct InteroperabilityResult<T> {
    pub interoperability_result_schema_version: &'static str,
    pub operation: &'static str,
    pub evidence: T,
}

impl<T> InteroperabilityResult<T> {
    pub fn new(operation: &'static str, evidence: T) -> Self {
        Self {
            interoperability_result_schema_version: INTEROPERABILITY_RESULT_SCHEMA_VERSION,
            operation,
            evidence,
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
    pub human_message: String,
}

impl CliFailure {
    pub fn classify(command: impl Into<String>, message: impl Into<String>) -> Self {
        let command = command.into();
        let message = message.into();
        let normalized = message.to_ascii_lowercase();
        let (code, exit, retryable) = if normalized.contains("product resource integrity failed") {
            ("evidence.integrity.failed", 5, false)
        } else if command == "templates describe"
            && (normalized.contains("unknown template")
                || normalized.contains("not qualified")
                || normalized.contains("template unavailable"))
        {
            ("capability.template.unavailable", 3, false)
        } else if normalized.contains("output path") && normalized.contains("already exists")
            || normalized.contains("destination") && normalized.contains("exist")
        {
            ("output.destination.exists", 4, false)
        } else if normalized.contains("unsafe")
            && (normalized.contains("path") || normalized.contains("traversal"))
        {
            ("output.path.unsafe", 4, false)
        } else if normalized.contains("resource limit")
            || normalized.contains("output limit")
            || normalized.contains("limit exceeded")
            || command == "assemble"
                && normalized.contains("resource")
                && (normalized.contains("exceed") || normalized.contains("budget"))
        {
            ("resource.limit.exceeded", 4, false)
        } else if normalized.contains("transfer syntax unavailable") {
            ("capability.transfer_syntax.unavailable", 3, false)
        } else if normalized.contains("template unavailable") {
            ("capability.template.unavailable", 3, false)
        } else if normalized.contains("unavailable") {
            ("capability.runtime.unavailable", 3, false)
        } else if normalized.contains("unsupported")
            && (normalized.contains("schema") || normalized.contains("version"))
        {
            ("request.version.unsupported", 2, false)
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
        } else if matches!(command.as_str(), "compose" | "assemble")
            && normalized.contains("input read failed")
        {
            ("request.read.failed", 2, false)
        } else if matches!(command.as_str(), "compose" | "assemble")
            && normalized.contains("request json invalid")
        {
            ("request.json.invalid", 2, false)
        } else if matches!(command.as_str(), "compose" | "assemble")
            && normalized.contains("request schema invalid")
        {
            ("request.schema.invalid", 2, false)
        } else if command == "assemble"
            && (normalized.contains("assembly request protected element")
                || normalized.contains("assembly element")
                || normalized.contains("duplicate assembly")
                || normalized.contains("assembly reference target missing")
                || normalized.contains("caller asset sha-256 mismatch")
                || normalized.contains("referenced frame exceeds"))
        {
            ("request.schema.invalid", 2, false)
        } else if command == "compose" && normalized.contains("request invalid") {
            if normalized.contains("json") || normalized.contains("expected value") {
                ("request.json.invalid", 2, false)
            } else {
                ("request.schema.invalid", 2, false)
            }
        } else if command == "compose" && normalized.contains("provider failed") {
            ("generation.provider.failed", 5, false)
        } else if command == "compose" && normalized.contains("materialization failed") {
            ("generation.materialization.failed", 5, false)
        } else if command == "compose" && normalized.contains("cancelled") {
            ("generation.execution.cancelled", 5, true)
        } else if command == "assemble" && normalized.contains("cancel") {
            ("generation.execution.cancelled", 5, true)
        } else if command == "assemble" && normalized.contains("material") {
            ("generation.materialization.failed", 5, false)
        } else if command == "assemble" {
            ("generation.planning.failed", 5, false)
        } else if command == "compose" {
            ("generation.planning.failed", 5, false)
        } else if command == "generate" && normalized.contains("plan-first generation failed") {
            if normalized.contains("provider") {
                ("generation.provider.failed", 5, false)
            } else if normalized.contains("materialization") {
                ("generation.materialization.failed", 5, false)
            } else {
                ("generation.planning.failed", 5, false)
            }
        } else if normalized.contains("invalid standards lock")
            || normalized.contains("invalid case registry")
            || normalized.contains("invalid metadata shape")
            || normalized.contains("invalid validator")
        {
            ("resource.document.invalid", 2, false)
        } else if normalized.contains("validation failed") {
            ("validation.artifact.failed", 5, false)
        } else if normalized.contains("conformance verification failed") {
            ("conformance.verification.failed", 5, false)
        } else if normalized.contains("conformance") && normalized.contains("failed") {
            ("conformance.run.failed", 5, false)
        } else if normalized.contains("interoperability") && normalized.contains("failed") {
            ("interoperability.qualification.failed", 5, false)
        } else if normalized.contains("failed to read") || normalized.contains(" read failed") {
            if matches!(
                command.as_str(),
                "validate"
                    | "report"
                    | "conformance run"
                    | "conformance verify"
                    | "interoperate media-dicomdir"
                    | "interoperate protocol-baseline"
            ) {
                ("request.read.failed", 2, false)
            } else {
                ("io.read.failed", 6, true)
            }
        } else if normalized.contains("failed to write") || normalized.contains("write failed") {
            ("io.write.failed", 6, true)
        } else if normalized.contains("read ") || normalized.contains("write ") {
            ("io.read.failed", 6, true)
        } else {
            ("internal.invariant.failed", 6, false)
        };
        let public_message = public_error_message(code).to_string();
        let mut context = BTreeMap::new();
        if code == "request.version.unsupported" {
            context.insert(
                "migration_action".into(),
                ErrorContextValue::String(
                    "select a version advertised by capabilities.result.supported_versions".into(),
                ),
            );
        }
        Self {
            command,
            exit,
            error: PublicError {
                code,
                message: public_message,
                context,
                retryable,
            },
            human_message: message,
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

fn public_error_message(code: &str) -> &'static str {
    match code {
        "command.syntax.invalid" => "command syntax is invalid",
        "command.argument.missing" => "a required command argument is missing",
        "request.read.failed" => "the caller request could not be read",
        "request.json.invalid" => "the caller request is not valid JSON",
        "request.schema.invalid" => "the caller request does not satisfy its schema",
        "request.version.unsupported" => "the requested schema or API version is unsupported",
        "resource.document.invalid" => "a product resource document is invalid",
        "capability.runtime.unavailable" => "the required runtime capability is unavailable",
        "capability.template.unavailable" => "the requested qualified template is unavailable",
        "capability.transfer_syntax.unavailable" => "the requested transfer syntax is unavailable",
        "output.destination.exists" => "the requested output destination already exists",
        "output.path.unsafe" => "the requested output path is unsafe",
        "resource.limit.exceeded" => "a caller-controlled resource limit was exceeded",
        "generation.planning.failed" => "generation planning failed",
        "generation.materialization.failed" => "generation materialization failed",
        "generation.provider.failed" => "a generation provider failed",
        "generation.execution.cancelled" => "generation was cancelled",
        "validation.artifact.failed" => "artifact validation failed",
        "conformance.run.failed" => "conformance execution failed",
        "conformance.verification.failed" => "conformance evidence verification failed",
        "interoperability.qualification.failed" => "interoperability qualification failed",
        "evidence.integrity.failed" => "locked evidence integrity verification failed",
        "io.read.failed" => "an unexpected product read failed",
        "io.write.failed" => "an unexpected product write failed",
        _ => "an internal product invariant failed",
    }
}
