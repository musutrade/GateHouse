pub(crate) mod baseline;
pub(crate) mod collectors;
pub(crate) mod compiler;
mod model;
mod policy;
#[cfg(test)]
mod tests;
mod validation;

use super::{ConfigDiagnostics, FlowConfig};
use anyhow::{Context, Result};
pub(crate) use model::{Assurance, QualityConfig, ReportFormat};
use std::{fs, io::ErrorKind, path::Path};

pub(crate) const QUALITY_CONFIG_PATH: &str = ".harness-gate/quality.toml";

pub fn schema_json() -> Result<String> {
    serde_json::to_string_pretty(&schemars::schema_for!(QualityConfig))
        .context("serialize quality schema")
}

impl QualityConfig {
    /// Describe configured omissions without manufacturing evidence records or
    /// metric values. Capability certification remains with the evidence core.
    pub(crate) fn participation(&self, profile: &str) -> serde_json::Value {
        let selected = &self.profiles[profile];
        serde_json::json!({
            "profile": profile,
            "assurance": selected.assurance,
            "policies": self.policies.iter().map(|(id, binding)| {
                (id, serde_json::json!({
                    "expectation": binding.expectation,
                    "state": if selected.policies.contains(id) { "participating" } else { "not_collected" },
                    "reason": if selected.policies.contains(id) { "selected by profile" } else { "omitted by profile" }
                }))
            }).collect::<std::collections::BTreeMap<_, _>>()
        })
    }

    /// File presence is explicit opt-in. Broken links/unreadable files fail closed.
    pub(crate) fn load_optional(root: &Path, flow: &FlowConfig) -> Result<Option<Self>> {
        let path = root.join(QUALITY_CONFIG_PATH);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            result => result.context("inspect quality configuration")?,
        };
        Self::load(root, flow).map(Some).map_err(|error| {
            ConfigDiagnostics::single(
                "HGCFG-QUALITY",
                "quality",
                format!("{error:#}"),
                "correct quality.toml references, profile participation, and policy/collector authority",
            )
            .with_source(path)
            .into()
        })
    }

    fn load(root: &Path, flow: &FlowConfig) -> Result<Self> {
        let canonical_root = root
            .canonicalize()
            .context("resolve quality repository root")?;
        let root = canonical_root.as_path();
        let path = crate::project::resolve_repo_path(
            root,
            Path::new(QUALITY_CONFIG_PATH),
            "quality configuration",
            true,
        )?;
        let source =
            crate::config::read_config_source(&path).context("read quality configuration")?;
        let config: Self = toml::from_str(&source).context("parse quality.toml (unknown fields, including collector policy authority, are forbidden)")?;
        config.validate(flow, root)?;
        Ok(config)
    }
}
