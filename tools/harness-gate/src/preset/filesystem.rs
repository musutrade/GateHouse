use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn ensure_writable(path: &Path, force: bool) -> Result<()> {
    if fs::symlink_metadata(path).is_ok() && !force {
        bail!(
            "{} already exists; pass --force to replace it",
            path.display()
        );
    }
    Ok(())
}

/// Publish one preset/migration output through the shared filesystem boundary.
pub(super) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    crate::utils::fs::atomic_write(path, content, true)
        .with_context(|| format!("publish preset output {}", path.display()))
}

/// Stage a group of preset outputs logically before changing any destination.
/// All paths are preflighted for symlink/non-directory components. If a later
/// publication fails, regular files that were already replaced are restored
/// through the same shared publisher; symlink targets are never followed.
pub(super) fn atomic_write_batch(entries: &[(&Path, &[u8])]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut seen = BTreeSet::new();
    let mut originals = Vec::with_capacity(entries.len());
    for (path, _) in entries {
        if !seen.insert(path.to_path_buf()) {
            bail!(
                "preset batch contains duplicate destination: {}",
                path.display()
            );
        }
        validate_destination(path)?;
        let original = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => Some(
                fs::read(path)
                    .with_context(|| format!("read existing preset output {}", path.display()))?,
            ),
            Ok(_) => unreachable!("validate_destination rejects non-files"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect preset output {}", path.display()));
            }
        };
        originals.push(original);
    }

    for (committed, (index, (path, content))) in entries.iter().enumerate().enumerate() {
        if let Err(error) = atomic_write(path, content) {
            for ((restore_path, _), original) in entries
                .iter()
                .take(committed)
                .zip(originals.iter().take(committed))
                .rev()
            {
                match original {
                    Some(bytes) => {
                        let _ = atomic_write(restore_path, bytes);
                    }
                    None => remove_regular_destination(restore_path),
                }
            }
            return Err(error).with_context(|| {
                format!("publish preset batch entry {index} ({})", path.display())
            });
        }
    }
    Ok(())
}

fn validate_destination(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    crate::utils::fs::ensure_parent_components(parent)
        .with_context(|| format!("inspect preset output parent {}", parent.display()))?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "preset output target is a symbolic link: {}",
                path.display()
            )
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!(
                "preset output target is not a regular file: {}",
                path.display()
            )
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("inspect preset output {}", path.display()))
        }
    }
}

fn remove_regular_destination(path: &Path) {
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn resolve_inside(root: &Path, path: PathBuf) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    if fs::symlink_metadata(&path).is_ok() {
        let resolved = path.canonicalize()?;
        if !resolved.starts_with(root) {
            bail!("path must remain inside the project: {}", path.display());
        }
        return Ok(path);
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    let resolved_parent = if parent.exists() {
        parent.canonicalize()?
    } else {
        let mut existing = parent;
        while !existing.exists() {
            existing = existing
                .parent()
                .ok_or_else(|| anyhow::anyhow!("cannot resolve {}", path.display()))?;
        }
        existing.canonicalize()?
    };
    if !resolved_parent.starts_with(root) {
        bail!("path must remain inside the project: {}", path.display());
    }
    Ok(path)
}
