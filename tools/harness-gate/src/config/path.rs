use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use super::model::DEFAULT_CONFIG_PATH;

pub fn resolve_config_path(root: &Path, override_path: Option<PathBuf>) -> Result<PathBuf> {
    let path = override_path.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve workflow config {}", path.display()))?;
    if !path.starts_with(root) {
        bail!(
            "workflow config must be inside the repository: {}",
            path.display()
        );
    }
    Ok(path)
}
