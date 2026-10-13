use crate::error::CodedError;

/// Errors emitted while determining a verification scope.
#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("Git scope detection failed: {message}")]
    Git { message: String },
    #[error("scope configuration failed: {message}")]
    Configuration { message: String },
    #[error("scope has {count} unmatched changed file(s): {files}")]
    UnmatchedFiles { count: usize, files: String },
    #[error("scope report failed: {message}")]
    Report { message: String },
}

impl ScopeError {
    pub(super) fn git(error: anyhow::Error) -> Self {
        Self::Git {
            message: format!("{error:#}"),
        }
    }

    pub(super) fn configuration(error: anyhow::Error) -> Self {
        Self::Configuration {
            message: format!("{error:#}"),
        }
    }

    pub(super) fn report(error: anyhow::Error) -> Self {
        Self::Report {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for ScopeError {
    fn code(&self) -> &'static str {
        match self {
            Self::Git { .. } => "E1301",
            Self::Configuration { .. } => "E1302",
            Self::UnmatchedFiles { .. } => "E1303",
            Self::Report { .. } => "E1304",
        }
    }
}
