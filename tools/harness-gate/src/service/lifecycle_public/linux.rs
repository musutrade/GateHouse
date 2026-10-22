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
        deadline: Instant,
    ) -> Result<Self> {
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
        };
        let prepared = LifecycleService::prepare(
            &base,
            VerifiedInstallation::load(Path::new(&config.host))?,
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
