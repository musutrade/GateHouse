use crate::audit::AuditError;
use crate::scope::ScopeError;
use crate::secrets::SecretsError;
use crate::verify::VerifyError;
use thiserror::Error;

/// A user-visible error code that remains stable across command output changes.
pub trait CodedError {
    fn code(&self) -> &'static str;
}

/// Error boundary for the CLI. Module-specific errors retain their own codes so
/// callers can distinguish a failed gate from a failed command invocation.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Audit(#[from] AuditError),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Scope(#[from] ScopeError),
    #[error(transparent)]
    Verify(#[from] VerifyError),
    #[error("command failed: {message}")]
    Command { message: String },
}

impl From<anyhow::Error> for CliError {
    fn from(error: anyhow::Error) -> Self {
        Self::Command {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for CliError {
    fn code(&self) -> &'static str {
        match self {
            Self::Audit(error) => error.code(),
            Self::Secrets(error) => error.code(),
            Self::Scope(error) => error.code(),
            Self::Verify(error) => error.code(),
            Self::Command { .. } => "E1000",
        }
    }
}
