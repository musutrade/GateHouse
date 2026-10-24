//! Container-specific ownership checks and the controlled runtime boundary.
//! Lease persistence, heartbeat, report-directory guards and reclamation decisions
//! remain in the parent module. This module is still part of Core.

use super::{ownership_labels, resource_key, LeaseRecord};
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::{bail, Result};
use std::path::Path;
use std::time::Duration;

/// Engine-neutral identity retained for checking legacy ownership evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeInspection {
    pub(super) object_id: String,
    pub(super) name: String,
    pub(super) labels: std::collections::BTreeMap<String, String>,
}

pub(super) trait RuntimeOperations {
    fn inspect(
        &self,
        runtime: ContainerRuntimeKind,
        project: &Project,
        name: &str,
        timeout: Duration,
    ) -> Result<RuntimeInspection>;

    fn stop(
        &self,
        runtime: ContainerRuntimeKind,
        cwd: &Path,
        name: &str,
        timeout: Duration,
    ) -> Result<()>;
}

pub(super) fn validate_runtime_ownership(
    project: &Project,
    path: &Path,
    record: &LeaseRecord,
    inspection: &RuntimeInspection,
) -> Result<()> {
    let expected_name = record.resource_name.as_deref().unwrap_or_default();
    if expected_name.trim().is_empty() {
        bail!("container lease has no resource name");
    }
    if inspection.name != expected_name.trim_start_matches('/') {
        bail!("runtime object name does not match the lease resource name");
    }
    let expected = ownership_labels(
        &record.project_identity,
        &record.resource_id,
        &record.resource_kind,
        &record.invocation_id,
    );
    for (key, value) in expected {
        if inspection.labels.get(&key) != Some(&value) {
            bail!("runtime object is missing or mismatches ownership label {key:?}");
        }
    }
    if record.project_identity != project.input().project_identity {
        bail!("lease project identity does not match the current project");
    }
    if path.file_name().and_then(|name| name.to_str())
        != Some(format!("{}.json", resource_key(&record.resource_id)).as_str())
    {
        bail!("lease filename does not match the deterministic resource key");
    }
    Ok(())
}

pub(super) fn verify_runtime_record<O: RuntimeOperations + ?Sized>(
    runtime_operations: &O,
    project: &Project,
    path: &Path,
    record: &LeaseRecord,
) -> Result<String> {
    let runtime = match record.runtime.as_deref() {
        Some("docker") => ContainerRuntimeKind::Docker,
        Some("podman") => ContainerRuntimeKind::Podman,
        Some(value) => bail!("unsupported container runtime {value:?}"),
        None => bail!("container lease has no runtime"),
    };
    let name = record
        .resource_name
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("container lease has no resource name"))?;
    let inspection = runtime_operations.inspect(runtime, project, name, Duration::from_secs(5))?;
    validate_runtime_ownership(project, path, record, &inspection)?;
    let object_id = record
        .runtime_object_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("container lease has no immutable runtime object ID"))?;
    if inspection.object_id != object_id {
        bail!("runtime object identity changed since the lease was recorded");
    }
    for (key, value) in &record.runtime_labels {
        if inspection.labels.get(key) != Some(value) {
            bail!("runtime ownership label {key:?} changed since lease creation");
        }
    }
    Ok(object_id.to_string())
}
