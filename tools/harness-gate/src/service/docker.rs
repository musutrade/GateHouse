use super::{remaining, RunningService};
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use crate::service::lease::ownership_labels;
use crate::service::runtime::{ContainerRuntime, ContainerStartOptions};
use crate::service::ResourceLease;
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) struct DockerStartOptions {
    pub(super) runtime: ContainerRuntimeKind,
    pub(super) image: String,
    pub(super) inject_env: String,
    pub(super) startup_timeout_secs: u64,
    pub(super) container_port: u16,
    pub(super) environment: BTreeMap<String, String>,
    pub(super) healthcheck: Vec<String>,
    pub(super) connection: String,
    pub(super) deadline: Instant,
    pub(super) cleanup_errors: Arc<Mutex<Vec<String>>>,
    pub(super) invocation_id: String,
}
pub(super) fn start_docker(
    project: &Project,
    id: &str,
    options: DockerStartOptions,
) -> Result<RunningService> {
    let DockerStartOptions {
        runtime,
        image,
        inject_env,
        startup_timeout_secs,
        container_port,
        environment,
        healthcheck,
        connection,
        deadline,
        cleanup_errors,
        invocation_id,
    } = options;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!(
        "harness-gate-{}-{id}-{}-{unique}",
        project.config.project.name,
        std::process::id()
    );
    let lease = ResourceLease::acquire(
        project,
        format!("service:{id}"),
        "container",
        invocation_id.clone(),
        Some(name.clone()),
        Some(runtime),
    )?;
    let executable = runtime.executable();
    if let Err(error) = runtime.start_container(
        project,
        ContainerStartOptions {
            name: &name,
            image: &image,
            environment: &environment,
            labels: &ownership_labels(
                &project.input().project_identity,
                &format!("service:{id}"),
                "container",
                &invocation_id,
            ),
            container_port,
        },
        remaining(deadline)?,
    ) {
        // A failed CLI call may have created the object before reporting an
        // error. Retain the lease so cleanup cannot silently lose the marker.
        lease.retain();
        return Err(anyhow::anyhow!(
            "start {executable} service {id:?}: {error:#}"
        ));
    }

    // Creation alone is not authority. Bind the lease only after a fresh
    // inspect proves that the runtime object carries every ownership label.
    let inspection = match runtime.inspect_container(project, &name, remaining(deadline)?) {
        Ok(inspection) => inspection,
        Err(error) => {
            // The object may exist, but ownership could not be proved. Retain
            // the lease for explicit operator investigation.
            lease.retain();
            return Err(error);
        }
    };
    if let Err(error) = lease.bind_runtime_identity(project, &inspection) {
        lease.retain();
        return Err(error);
    }

    let mut running = RunningService {
        runtime,
        inject_env,
        value: String::new(),
        container: Some(name),
        project: project.clone(),
        cleanup_errors,
        lease: Some(lease),
    };
    while Instant::now() < deadline {
        if let Some(lease) = &running.lease {
            lease.renew()?;
        }
        if crate::process::cancelled() {
            bail!("verification cancelled while waiting for service {id:?}");
        }
        if let Some(container) = running.container.as_deref() {
            if let Some(port) =
                runtime.mapped_port(project, container, container_port, remaining(deadline)?)?
            {
                let ready = runtime.run_healthcheck(
                    project,
                    container,
                    &healthcheck,
                    remaining(deadline)?,
                )?;
                if ready {
                    running.value = connection.replace("{host_port}", &port);
                    return Ok(running);
                }
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    bail!("service {id:?} did not become ready within {startup_timeout_secs} seconds")
}
