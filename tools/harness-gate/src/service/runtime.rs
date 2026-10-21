use super::inspection::RuntimeInspection;
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Service engine operations, without implicit CLI execution.
/// Every backend must implement every operation explicitly. Core retains lease,
/// ownership, deadline and cancellation decisions. This is an internal boundary,
/// not the v2 plugin protocol or authority to migrate existing leases.
pub(crate) trait ContainerRuntime {
    fn executable(&self) -> &'static str;
    fn start_container(
        &self,
        project: &Project,
        options: ContainerStartOptions<'_>,
        timeout: Duration,
    ) -> Result<()>;
    fn mapped_port(
        &self,
        project: &Project,
        name: &str,
        container_port: u16,
        timeout: Duration,
    ) -> Result<Option<String>>;
    fn run_healthcheck(
        &self,
        project: &Project,
        name: &str,
        command: &[String],
        timeout: Duration,
    ) -> Result<bool>;
    fn stop_container(&self, cwd: &Path, name: &str, timeout: Duration) -> Result<()>;
    /// Return identity and labels for Core to compare with the ownership lease.
    fn inspect_container(
        &self,
        project: &Project,
        name: &str,
        timeout: Duration,
    ) -> Result<RuntimeInspection>;
    fn check_available(&self, project: &Project, timeout: Duration) -> Result<()>;
    fn image_available(&self, project: &Project, image: &str, timeout: Duration) -> Result<bool>;
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
