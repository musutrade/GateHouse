use super::{resolve_repo_path, Project};
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

impl Project {
    pub fn prepare(&self) -> Result<()> {
        let reports = resolve_repo_path(
            &self.root,
            Path::new(&self.config.paths.reports),
            "report directory",
            false,
        )?;
        if reports != self.reports {
            bail!("report directory changed during project discovery");
        }
        fs::create_dir_all(self.reports.join("logs"))?;
        Ok(())
    }

    pub fn path(&self, alias: &str) -> Option<&Path> {
        match alias {
            "root" => Some(&self.execution_root),
            "reports" => Some(&self.reports),
            "audit_config" => Some(&self.audit_config),
            "secrets_config" => Some(&self.secrets_config),
            _ => self.aliases.get(alias).map(PathBuf::as_path),
        }
    }

    pub fn expand(&self, value: &str) -> String {
        let mut resolved = value.to_string();
        for name in self.config.paths.aliases.keys().map(String::as_str) {
            if let Some(path) = self.path(name) {
                resolved = resolved.replace(&format!("{{{name}}}"), &path.to_string_lossy());
            }
        }
        for name in ["audit_config", "secrets_config", "reports", "root"] {
            if let Some(path) = self.path(name) {
                resolved = resolved.replace(&format!("{{{name}}}"), &path.to_string_lossy());
            }
        }
        resolved
    }
}
