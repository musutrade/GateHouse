use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use super::commands::{
    healthcheck_args, inspect_container_args, mapped_port_args, runtime_info_args,
    start_container_args, stop_container_args,
};
use super::inspection::{parse_mapped_port, parse_runtime_inspection, RuntimeInspection};

/// Small runtime boundary shared by Docker-compatible container engines.
/// Keeping command execution behind this trait makes service orchestration
/// independent of the selected CLI while preserving the existing adapter.
pub(crate) trait ContainerRuntime {
    fn executable(&self) -> &'static str;

    fn start_container(
        &self,
        project: &Project,
        options: ContainerStartOptions<'_>,
        timeout: Duration,
    ) -> Result<()> {
        let name = options.name;
        let image = options.image;
        let args = start_container_args(
            name,
            image,
            options.environment,
            options.labels,
            options.container_port,
        );
        let output = crate::process::capture(self.executable(), &args, &project.root, timeout)
            .with_context(|| format!("start {} container {name:?}", self.executable()))?;
        if !output.status.success() {
            bail!(
                "failed to start {} container {name:?} with image {image}: {}",
                self.executable(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn mapped_port(
        &self,
        project: &Project,
        name: &str,
        container_port: u16,
        timeout: Duration,
    ) -> Result<Option<String>> {
        let args = mapped_port_args(name, container_port);
        let output = crate::process::capture(self.executable(), &args, &project.root, timeout)?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(parse_mapped_port(&output.stdout))
    }

    fn run_healthcheck(
        &self,
        project: &Project,
        name: &str,
        command: &[String],
        timeout: Duration,
    ) -> Result<bool> {
        let args = healthcheck_args(name, command);
        Ok(
            crate::process::capture(self.executable(), &args, &project.root, timeout)?
                .status
                .success(),
        )
    }

    fn stop_container(&self, cwd: &Path, name: &str, timeout: Duration) -> Result<()> {
        let args = stop_container_args(name);
        let output = crate::process::capture_cleanup(self.executable(), &args, cwd, timeout)?;
        if !output.status.success() {
            bail!(
                "failed to stop {} container {name:?}: {}",
                self.executable(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    /// Inspect the current object immediately before a destructive action.
    /// The caller must compare this identity and label set with its lease.
    fn inspect_container(
        &self,
        project: &Project,
        name: &str,
        timeout: Duration,
    ) -> Result<RuntimeInspection> {
        let output = crate::process::capture(
            self.executable(),
            &inspect_container_args(name),
            &project.root,
            timeout,
        )
        .with_context(|| format!("inspect {} container {name:?}", self.executable()))?;
        if !output.status.success() {
            bail!(
                "failed to inspect {} container {name:?}: {}",
                self.executable(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        parse_runtime_inspection(&output.stdout)
            .with_context(|| format!("parse {} inspection for {name:?}", self.executable()))
    }

    fn check_available(&self, project: &Project, timeout: Duration) -> Result<()> {
        let output = crate::process::capture(
            self.executable(),
            &runtime_info_args(),
            &project.root,
            timeout,
        )
        .with_context(|| format!("{} is required for managed services", self.executable()))?;
        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("{} daemon is unavailable", self.executable())
        }
    }
}

pub(crate) struct ContainerStartOptions<'a> {
    pub(crate) name: &'a str,
    pub(crate) image: &'a str,
    pub(crate) environment: &'a BTreeMap<String, String>,
    pub(crate) labels: &'a BTreeMap<String, String>,
    pub(crate) container_port: u16,
}

pub(crate) fn stop_owned_container(
    runtime: ContainerRuntimeKind,
    cwd: &Path,
    name: &str,
    timeout: Duration,
) -> Result<()> {
    runtime.stop_container(cwd, name, timeout)
}

pub(crate) fn inspect_owned_container(
    runtime: ContainerRuntimeKind,
    project: &Project,
    name: &str,
    timeout: Duration,
) -> Result<RuntimeInspection> {
    runtime.inspect_container(project, name, timeout)
}

impl ContainerRuntime for ContainerRuntimeKind {
    fn executable(&self) -> &'static str {
        match self {
            ContainerRuntimeKind::Docker => "docker",
            ContainerRuntimeKind::Podman => "podman",
        }
    }
}
