use crate::audit::AuditError;
use crate::scope::ScopeError;
use crate::secrets::SecretsError;
use crate::verify::VerifyError;
use thiserror::Error;

/// A user-visible error code that remains stable across command output changes.
/// This trait ensures consistent error identification for tooling and diagnostics.
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
    #[error("command failed: {message}")]
    Configuration {
        message: String,
        #[source]
        diagnostics: Box<crate::config::ConfigDiagnostics>,
    },
}

impl From<anyhow::Error> for CliError {
    fn from(error: anyhow::Error) -> Self {
        if let Some(diagnostics) = error.downcast_ref::<crate::config::ConfigDiagnostics>() {
            return Self::Configuration {
                message: format!("{error:#}"),
                diagnostics: Box::new(diagnostics.clone()),
            };
        }
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
            Self::Command { .. } | Self::Configuration { .. } => "E1000",
        }
    }
}

impl CliError {
    pub fn render_terminal(&self) -> String {
        match self {
            Self::Configuration { diagnostics, .. } => format!(
                "{}\n{}",
                crate::ui::error("ERROR [E1000]: configuration is invalid"),
                diagnostics.render_terminal(),
            ),
            _ => crate::ui::error(format!("ERROR [{}]: {self}", self.code())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CliError, CodedError};

    #[test]
    fn contextual_configuration_keeps_display_and_typed_source() {
        let error = anyhow::Error::new(crate::config::ConfigDiagnostics::single(
            "HGCFG-CHECK",
            "project",
            "invalid configuration",
            "repair project configuration",
        ))
        .context("loading project");
        let previous = format!("command failed: {error:#}");
        let cli = CliError::from(error);
        assert_eq!(cli.to_string(), previous);
        assert_eq!(cli.code(), "E1000");
        assert!(matches!(&cli, CliError::Configuration { .. }));
        assert!(std::error::Error::source(&cli).is_some());
        let terminal = cli.render_terminal();
        assert!(terminal.contains("error[HGCFG-CHECK]"));
        assert!(terminal.contains("   = field: project"));
    }

    #[test]
    fn untyped_command_error_keeps_existing_output() {
        let cli = CliError::from(anyhow::anyhow!("ordinary failure"));
        assert!(matches!(&cli, CliError::Command { .. }));
        assert_eq!(cli.to_string(), "command failed: ordinary failure");
        assert_eq!(cli.code(), "E1000");
        assert!(cli
            .render_terminal()
            .contains("ERROR [E1000]: command failed: ordinary failure"));
    }
}
