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
