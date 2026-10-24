//! Fail-closed adapter for retained legacy engine records. No engine execution.
use super::container::{RuntimeInspection, RuntimeOperations};
use crate::{config::ContainerRuntimeKind, project::Project};
use anyhow::{bail, Result};
use std::{path::Path, time::Duration};

pub(super) struct RetiredRuntimeOperations;

impl RuntimeOperations for RetiredRuntimeOperations {
    fn inspect(
        &self,
        runtime: ContainerRuntimeKind,
        _project: &Project,
        _name: &str,
        _timeout: Duration,
    ) -> Result<RuntimeInspection> {
        retired_lease(runtime)
    }

    fn stop(
        &self,
        runtime: ContainerRuntimeKind,
        _cwd: &Path,
        _name: &str,
        _timeout: Duration,
    ) -> Result<()> {
        retired_lease(runtime)
    }
}

// Legacy records remain readable, but an engine name is not an installed host
// binding. Never infer mutation authority from current service configuration.
fn retired_lease<T>(runtime: ContainerRuntimeKind) -> Result<T> {
    bail!(
        "legacy {} CLI service operations are retired; lease retained without migration; quiesce old Core processes and resolve the recorded resource with the previously approved version or an explicit operator procedure before configuring lifecycle_host; see docs/runtime-lifecycle-v1.md#legacy-cli-retirement",
        runtime.executable()
    )
}
