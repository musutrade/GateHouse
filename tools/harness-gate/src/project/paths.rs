use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_repo_path(
    root: &Path,
    path: &Path,
    label: &str,
    must_exist: bool,
) -> Result<PathBuf> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        bail!("{label} must be a non-empty repository-relative path");
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        bail!("{label} may not escape the repository: {}", path.display());
    }

    let candidate = root.join(path);
    if fs::symlink_metadata(&candidate).is_ok() {
        let resolved = candidate
            .canonicalize()
            .with_context(|| format!("resolve {label} {}", candidate.display()))?;
        if !resolved.starts_with(root) {
            bail!("{label} escapes the repository: {}", candidate.display());
        }
        return Ok(resolved);
    }
    if must_exist {
        bail!("{label} is missing: {}", candidate.display());
    }

    let mut ancestor = candidate.as_path();
    while fs::symlink_metadata(ancestor).is_err() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot resolve {label}: {}", candidate.display()))?;
    }
    let resolved_ancestor = ancestor
        .canonicalize()
        .with_context(|| format!("resolve {label} parent {}", ancestor.display()))?;
    if !resolved_ancestor.starts_with(root) {
        bail!("{label} escapes the repository: {}", candidate.display());
    }
    Ok(candidate)
}
