use crate::error::CodedError;
use crate::utils::redaction::redact_text;

/// Errors emitted by the architecture-audit boundary.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit configuration failed: {message}")]
    Configuration { message: String },
    #[error("audit execution failed: {message}")]
    Execution { message: String },
    #[error("log parsing failed: {message}")]
    LogParsing { message: String },
}

impl AuditError {
    pub(super) fn configuration(error: anyhow::Error) -> Self {
        Self::Configuration {
            message: redact_text(&format!("{error:#}")),
        }
    }
    pub(super) fn execution(error: anyhow::Error) -> Self {
        Self::Execution {
            message: redact_text(&format!("{error:#}")),
        }
    }
    pub(super) fn log_parsing(error: anyhow::Error) -> Self {
        Self::LogParsing {
            message: redact_text(&format!("{error:#}")),
        }
    }
}

impl CodedError for AuditError {
    fn code(&self) -> &'static str {
        match self {
            Self::Configuration { .. } => "E1101",
            Self::Execution { .. } => "E1102",
            Self::LogParsing { .. } => "E1103",
        }
    }
}
