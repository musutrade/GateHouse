//! Required external service checks. Core owns execution and the final decision.
use crate::config::ServiceCheck;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) const PROTOCOL: &str = "gate-service-check/v1";

pub(crate) fn validate_check(check: &ServiceCheck) -> Result<()> {
    if !Path::new(&check.executable).is_absolute() {
        bail!("required service check executable must be an absolute installed artifact path");
    }
    if check.sha256.len() != 64 || !check.sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("required service check must pin its artifact SHA-256");
    }
    if check.protocol != PROTOCOL || check.plugin.is_empty() || check.version.is_empty() {
        bail!("required service check protocol or identity is incompatible");
    }
    if check.timeout_secs == 0 || check.timeout_secs > 300 {
        bail!("required service check timeout_secs must be between 1 and 300");
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckResult {
    protocol: String,
    plugin: String,
    version: String,
    mode: String,
    success: bool,
}

fn verify_artifact(check: &ServiceCheck) -> Result<()> {
    let bytes = std::fs::read(&check.executable).context("read required service check artifact")?;
    if format!("{:x}", Sha256::digest(bytes)) != check.sha256.to_ascii_lowercase() {
        bail!("required service check artifact digest mismatch");
    }
    Ok(())
}

fn run_check(
    check: &ServiceCheck,
    root: &Path,
    value: Option<&str>,
    timeout: Duration,
) -> Result<()> {
    validate_check(check)?;
    verify_artifact(check)?;
    let mode = if value.is_some() {
        "external"
    } else {
        "managed"
    };
    let mut environment: BTreeMap<String, String> = check
        .pass_env
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (name.clone(), value)))
        .collect();
    environment.insert("GATE_CHECK_PROTOCOL".into(), PROTOCOL.into());
    environment.insert("GATE_CHECK_MODE".into(), mode.into());
    if let Some(value) = value {
        environment.insert("GATE_CHECK_VALUE".into(), value.into());
    }
    let output = crate::process::capture_environment(
        &check.executable,
        root,
        timeout.min(Duration::from_secs(check.timeout_secs)),
        &environment,
    )?;
    verify_artifact(check)?;
    // Never publish untrusted stdout/stderr or the service value in diagnostics.
    if !output.status.success() {
        bail!(
            "required service check {} exited unsuccessfully",
            check.plugin
        );
    }
    let result: CheckResult = serde_json::from_slice(&output.stdout)
        .map_err(|_| anyhow::anyhow!("required service check result missing or malformed"))?;
    if result.protocol != check.protocol
        || result.plugin != check.plugin
        || result.version != check.version
        || result.mode != mode
    {
        bail!("required service check result identity mismatch");
    }
    if !result.success {
        bail!("required service check {} reported failure", check.plugin);
    }
    Ok(())
}

pub(crate) fn run_checks(
    project: &Project,
    service: &str,
    value: Option<&str>,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    if let Some(checks) = project.config.service_checks.get(service) {
        if checks.is_empty() {
            bail!("declared service checks cannot be empty");
        }
        for check in checks {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .context("required service checks timed out")?;
            run_check(check, &project.root, value, remaining)?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
