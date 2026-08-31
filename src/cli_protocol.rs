use std::collections::BTreeMap;

use serde::Serialize;

pub const CLI_API_VERSION: &str = "1.0.0";

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
