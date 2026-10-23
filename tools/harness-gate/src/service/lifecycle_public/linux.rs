use super::Config;
use crate::{
    project::Project,
    service::lifecycle::{self, LifecycleService, ServiceIdentity},
};
use anyhow::Result;
use gatehouse_runtime_protocol::lifecycle::{
    installation::VerifiedInstallation,
    journal::{CreatePlan, ServiceSlot},
    wire::Entry,
};
use std::{path::Path, time::Instant};
pub(crate) struct ManagedLifecycle(LifecycleService);
impl ManagedLifecycle {
    pub(crate) fn start(
        project: &Project,
        id: &str,
        config: Config,
        runtime: crate::config::ContainerRuntimeKind,
        deadline: Instant,
    ) -> Result<Self> {
        reject_legacy_marker(project, id)?;
        let installation = VerifiedInstallation::load(Path::new(&config.host))?;
        let binding = installation.engine_binding()?;
        if binding.engine != runtime.executable() {
            anyhow::bail!(
                "lifecycle installation engine {:?} does not match configured runtime {:?}",
                binding.engine,
                runtime.executable()
            );
        }
        let base = ServiceSlot::project_state_base(&project.root)?;
        let project_id = project.input().project_identity.clone();
        let resource_id = format!("service:{id}");
        let invocation = lifecycle::nonce()?;
        let labels = [
            (
                "gatehouse.lifecycle.authority",
                "container-lifecycle/mutate".into(),
            ),
            ("gatehouse.lifecycle.invocation", invocation.clone()),
            ("gatehouse.lifecycle.project", project_id.clone()),
            ("gatehouse.lifecycle.resource", resource_id.clone()),
        ]
        .into_iter()
        .map(|(key, value)| Entry {
            key: key.into(),
            value,
        })
        .collect();
        let plan = CreatePlan {
            name: format!("gatehouse-{invocation}"),
            labels,
            environment: config
                .environment
                .into_iter()
                .map(|(key, value)| Entry { key, value })
                .collect(),
            container_port: config.container_port,
            command: config.command,
        };
        let prepared = LifecycleService::prepare(
            &base,
            installation,
            ServiceIdentity {
                project_id,
                resource_id,
                lease_invocation: invocation,
            },
            config.image,
            plan,
            &config.healthcheck,
        )?;
        Ok(Self(prepared.start(deadline, crate::process::cancelled)?))
    }
    pub(crate) fn port(&self) -> u16 {
        self.0.port()
    }
    pub(crate) fn cleanup(self, deadline: Instant) -> Result<()> {
        self.0.cleanup(deadline, || false)
    }
}

/// Called only by the explicit CLI operator route; never by normal acquisition.
pub(crate) fn cleanup_known(
    project: &Project,
    service: &str,
    fence: u64,
    invocation: &str,
) -> Result<String> {
    use crate::config::{ContainerRuntimeKind, ServiceConfig};
    use anyhow::{bail, Context};
    use gatehouse_runtime_protocol::lifecycle::journal::OperatorRecovery;
    let Some(ServiceConfig::Docker {
        runtime,
        lifecycle_host: Some(host),
        external_env: None,
        ..
    }) = project.config.services.get(service)
    else {
        bail!("operator cleanup requires explicit lifecycle_host without external_env");
    };
    if !matches!(
        runtime,
        ContainerRuntimeKind::Docker | ContainerRuntimeKind::Podman
    ) {
        bail!("operator cleanup requires a supported lifecycle runtime");
    }
    if fence == 0 {
        bail!("operator cleanup requires a nonzero exact fence");
    }
    let installation = VerifiedInstallation::load(Path::new(host.as_ref()))?;
    if installation.engine_binding()?.engine != runtime.executable() {
        bail!("operator cleanup installation engine does not match configured runtime");
    }
    let base = ServiceSlot::project_state_base(&project.root)?;
    let authorization = lifecycle::nonce()?;
    let recovery = OperatorRecovery::open_mapped(
        &base,
        &project.input().project_identity,
        &format!("service:{service}"),
        fence,
        invocation,
        &authorization,
    )?;
    lifecycle::recovery::cleanup_known(
        recovery,
        installation,
        Instant::now() + std::time::Duration::from_secs(30),
        crate::process::cancelled,
    )
    .with_context(|| {
        format!(
            "operator authorization {authorization} retained; inspect current slot before retrying"
        )
    })?;
    Ok(authorization)
}

fn reject_legacy_marker(project: &Project, id: &str) -> Result<()> {
    use crate::service::lease::{lease_directory, resource_key};
    let marker =
        lease_directory(project)?.join(format!("{}.json", resource_key(&format!("service:{id}"))));
    match std::fs::symlink_metadata(marker) {
        Ok(_) => anyhow::bail!("legacy service lease still exists; resolve it with the prior approved version before lifecycle startup; no automatic migration"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
