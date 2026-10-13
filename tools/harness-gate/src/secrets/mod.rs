mod config;
mod matcher;

#[cfg(test)]
mod tests;

use crate::error::CodedError;
use crate::project::Project;
use crate::utils::{fs as output_fs, git};
use anyhow::{Context, Result};
use config::SecretScanner;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::Path;

const MAX_SECRET_SCAN_FILE_BYTES: u64 = 16 * 1024 * 1024;

fn reject_oversized_file(file: &str, size: u64) -> Result<()> {
    if size > MAX_SECRET_SCAN_FILE_BYTES {
        anyhow::bail!(
            "secret scan file {file:?} is too large ({} bytes; limit {} bytes)",
            size,
            MAX_SECRET_SCAN_FILE_BYTES
        );
    }
    Ok(())
}

fn read_working_tree_file(path: &Path, file: &str, size: u64) -> Result<Option<Vec<u8>>> {
    reject_oversized_file(file, size)?;
    let source = match fs::File::open(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(anyhow::Error::from(error).context(format!("open {file}"))),
    };
    let mut bytes = Vec::with_capacity(size as usize);
    source
        .take(MAX_SECRET_SCAN_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {file}"))?;
    reject_oversized_file(file, bytes.len() as u64)?;
    Ok(Some(bytes))
}

/// Errors emitted by the secret-scan boundary.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("secret scan configuration failed: {message}")]
    Configuration { message: String },
    #[error("secret scan failed: {message}")]
    Scan { message: String },
}

impl SecretsError {
    fn configuration(error: anyhow::Error) -> Self {
        Self::Configuration {
            message: format!("{error:#}"),
        }
    }

    fn scan(error: anyhow::Error) -> Self {
        Self::Scan {
            message: format!("{error:#}"),
        }
    }
}

impl CodedError for SecretsError {
    fn code(&self) -> &'static str {
        match self {
            Self::Configuration { .. } => "E1201",
            Self::Scan { .. } => "E1202",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum SecretMode {
    WorkingTree,
    Staged,
}

#[derive(Debug, Serialize)]
struct SecretReport<'a> {
    timestamp: String,
    mode: &'a str,
    findings: &'a [String],
}

fn scanner_for_mode(project: &Project, mode: SecretMode) -> Result<SecretScanner> {
    match mode {
        SecretMode::WorkingTree => SecretScanner::load(&project.secrets_config),
        SecretMode::Staged if project.input().is_snapshot() => {
            SecretScanner::load(&project.secrets_config)
        }
        SecretMode::Staged => {
            let relative = project
                .secrets_config
                .strip_prefix(&project.root)
                .context("secret scan configuration must stay inside the project")?;
            let relative = git::index_path(relative)?;
            let size = git::staged_file_size(&project.root, &relative)?.ok_or_else(|| {
                anyhow::anyhow!("staged secret scan configuration is missing: {relative}")
            })?;
            reject_oversized_file(&relative, size)?;
            let bytes = git::staged_file(&project.root, &relative)?.ok_or_else(|| {
                anyhow::anyhow!("staged secret scan configuration is missing: {relative}")
            })?;
            reject_oversized_file(&relative, bytes.len() as u64)?;
            let source = std::str::from_utf8(&bytes)
                .context("staged secret scan configuration must be UTF-8")?;
            SecretScanner::from_source(source)
                .with_context(|| format!("parse staged secret scan configuration {relative}"))
        }
    }
}

pub fn scan(project: &Project, mode: SecretMode) -> std::result::Result<Vec<String>, SecretsError> {
    let files = match mode {
        SecretMode::WorkingTree => git::null_terminated_paths(
            &project.root,
            [
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )
        .map_err(SecretsError::scan)?,
        SecretMode::Staged => git::null_terminated_paths(
            &project.root,
            [
                "diff",
                "--cached",
                "--diff-filter=ACMR",
                "--name-only",
                "-z",
            ],
        )
        .map_err(SecretsError::scan)?,
    };
    let patterns = scanner_for_mode(project, mode).map_err(SecretsError::configuration)?;
    let mut findings = Vec::new();

    for file in files {
        if crate::process::cancelled() {
            return Err(SecretsError::scan(anyhow::anyhow!("secret scan cancelled")));
        }
        let bytes = match mode {
            SecretMode::WorkingTree => {
                let path = project.execution_root.join(&file);
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(SecretsError::scan(
                            anyhow::Error::from(error).context(format!("inspect {file}")),
                        ));
                    }
                };
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    continue;
                }
                let size = fs::metadata(&path)
                    .map_err(|error| SecretsError::scan(anyhow::Error::from(error)))?
                    .len();
                match read_working_tree_file(&path, &file, size).map_err(SecretsError::scan)? {
                    Some(bytes) => bytes,
                    None => continue,
                }
            }
            SecretMode::Staged => {
                if project.input().is_snapshot() {
                    let path = project.execution_root.join(&file);
                    let metadata = match fs::symlink_metadata(&path) {
                        Ok(metadata) => metadata,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(error) => {
                            return Err(SecretsError::scan(
                                anyhow::Error::from(error).context(format!("inspect {file}")),
                            ));
                        }
                    };
                    if metadata.file_type().is_symlink() || !metadata.is_file() {
                        continue;
                    }
                    match read_working_tree_file(&path, &file, metadata.len())
                        .map_err(SecretsError::scan)?
                    {
                        Some(bytes) => bytes,
                        None => continue,
                    }
                } else {
                    let Some(size) =
                        git::staged_file_size(&project.root, &file).map_err(SecretsError::scan)?
                    else {
                        continue;
                    };
                    reject_oversized_file(&file, size).map_err(SecretsError::scan)?;
                    match git::staged_file(&project.root, &file).map_err(SecretsError::scan)? {
                        Some(bytes) => bytes,
                        None => continue,
                    }
                }
            }
        };
        reject_oversized_file(&file, bytes.len() as u64).map_err(SecretsError::scan)?;
        if patterns.is_match(&bytes) {
            findings.push(file);
        }
    }

    let mode_label = match mode {
        SecretMode::WorkingTree => "working-tree",
        SecretMode::Staged => "staged",
    };
    output_fs::confined_write_json(
        &project.reports,
        Path::new("secret_scan.json"),
        &SecretReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: mode_label,
            findings: &findings,
        },
        true,
    )
    .map_err(SecretsError::scan)?;
    Ok(findings)
}
