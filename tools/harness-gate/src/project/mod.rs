mod discovery;
mod input;
mod paths;
mod runtime;
#[cfg(test)]
mod tests;

use crate::config::{CompiledScopeRules, FlowConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) use input::{InputMode, InvocationInput};
pub(crate) use paths::resolve_repo_path;

#[derive(Debug, Clone)]
pub struct Project {
    /// Canonical checkout used for Git metadata and explicitly repository-bound
    /// operations. Repository content must be read from `execution_root`.
    pub root: PathBuf,
    /// Trusted CLI override; never selected by repository configuration.
    pub(crate) replay_state_dir: Option<PathBuf>,
    /// Immutable source root used by gates and ordinary external steps.
    pub execution_root: PathBuf,
    pub(crate) invocation_input: InvocationInput,
    pub config_path: PathBuf,
    pub config: FlowConfig,
    pub reports: PathBuf,
    pub audit_config: PathBuf,
    pub secrets_config: PathBuf,
    /// Cross-process resource leases are kept beside the configured report
    /// root, not inside an invocation directory, so concurrent invocations
    /// can observe the same ownership records.
    pub(crate) resource_leases: PathBuf,
    aliases: BTreeMap<String, PathBuf>,
    pub(crate) scope_rules: CompiledScopeRules,
}

impl Project {
    pub(crate) fn repository_root(&self) -> &std::path::Path {
        &self.root
    }

    pub(crate) fn input(&self) -> &InvocationInput {
        &self.invocation_input
    }

    pub(crate) fn with_input_mode(mut self, mode: InputMode) -> Self {
        self.invocation_input.mode = mode;
        self
    }

    pub(crate) fn expand_for_input(&self, value: &str, input: crate::config::StepInput) -> String {
        if input == crate::config::StepInput::Repository {
            let mut resolved = value.to_string();
            resolved = resolved.replace("{root}", &self.root.to_string_lossy());
            for name in self.config.paths.aliases.keys().map(String::as_str) {
                if let Some(entry) = self.config.paths.aliases.get(name) {
                    let path = self.root.join(&entry.path);
                    resolved = resolved.replace(&format!("{{{name}}}"), &path.to_string_lossy());
                }
            }
            for (name, path) in [
                (
                    "audit_config",
                    self.root.join(&self.config.paths.audit_config),
                ),
                (
                    "secrets_config",
                    self.root.join(&self.config.paths.secrets_config),
                ),
                ("reports", self.reports.clone()),
            ] {
                resolved = resolved.replace(&format!("{{{name}}}"), &path.to_string_lossy());
            }
            resolved
        } else {
            self.expand(value)
        }
    }

    pub(crate) fn invocation_id(&self) -> String {
        let is_invocation = self
            .reports
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|name| name == "invocations");
        if is_invocation {
            if let Some(name) = self.reports.file_name().and_then(|name| name.to_str()) {
                return name.to_string();
            }
        }
        format!("standalone-{}", std::process::id())
    }
}
