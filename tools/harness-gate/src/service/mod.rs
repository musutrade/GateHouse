mod commands;
mod docker;
mod inspection;
mod lease;
mod report_directory;
mod runtime;

#[cfg(test)]
mod tests;

use crate::config::ServiceConfig;
use crate::project::Project;
use crate::recipes::validate_external_value;
use anyhow::{bail, Context, Result};
use docker::{start_docker, DockerStartOptions};
use runtime::ContainerRuntime;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub(crate) use lease::{cleanup as cleanup_resources, ResourceLease};
pub(crate) use report_directory::ReportDirectoryGuard;

const RESOURCE_LOCK_POLL: Duration = Duration::from_millis(25);
const RESOURCE_LOCK_WAIT: Duration = Duration::from_secs(30);

pub struct ServiceManager<'a> {
    project: &'a Project,
    resources: BTreeMap<String, Arc<ServiceResource>>,
}

impl<'a> ServiceManager<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self {
            project,
            resources: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub fn environment(&mut self, id: &str) -> Result<(String, String)> {
        let lease = self.handle(id)?.acquire()?;
        Ok(lease.environment())
    }

    pub(super) fn cleanup(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for resource in self.resources.values() {
            if let Err(error) = resource.cleanup() {
                errors.push(format!("{error:#}"));
            }
            match resource.take_cleanup_errors() {
                Ok(resource_errors) => errors.extend(resource_errors),
                Err(error) => errors.push(format!("{error:#}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            bail!("service cleanup failed: {}", errors.join("; "))
        }
    }

    pub(super) fn handle(&mut self, id: &str) -> Result<ServiceHandle<'a>> {
        let config = self
            .project
            .config
            .service(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
        let resource = self
            .resources
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(ServiceResource::new(is_shareable(&config))))
            .clone();
        Ok(ServiceHandle {
            project: self.project,
            id: id.to_string(),
            config,
            resource,
        })
    }

    /// Build owned startup jobs for services referenced by the selected plan.
    /// Jobs deliberately do not hold a lease: managed services are exclusive,
    /// so a normal lease would block the first real step until the gates finish.
    pub(super) fn warmup_jobs<I>(&mut self, ids: I) -> Result<Vec<ServiceWarmup>>
    where
        I: IntoIterator<Item = String>,
    {
        let mut jobs = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for id in ids {
            if !seen.insert(id.clone()) {
                continue;
            }
            let config = self
                .project
                .config
                .service(&id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
            let resource = self
                .resources
                .entry(id.clone())
                .or_insert_with(|| Arc::new(ServiceResource::new(is_shareable(&config))))
                .clone();
            jobs.push(ServiceWarmup {
                project: self.project.clone(),
                id,
                config,
                resource,
            });
        }
        Ok(jobs)
    }
}

/// A service resource is shareable only when its adapter can safely serve
/// multiple child processes without mutable teardown state. Environment values
/// are immutable; managed containers remain exclusive until a future adapter
/// explicitly provides a shared lifecycle contract.
fn is_shareable(config: &ServiceConfig) -> bool {
    matches!(config, ServiceConfig::Environment { .. })
}

struct ServiceResource {
    state: Mutex<ResourceState>,
    changed: Condvar,
    shareable: bool,
    cleanup_errors: Arc<Mutex<Vec<String>>>,
}

enum ResourceState {
    Empty,
    Starting,
    Ready {
        service: Box<RunningService>,
        users: usize,
    },
    /// A non-shareable service is being torn down after its final lease.
    /// Keeping this state visible prevents a new startup from racing the
    /// adapter's cleanup transition.
    Stopping,
    Failed(String),
}

impl ServiceResource {
    fn new(shareable: bool) -> Self {
        Self {
            state: Mutex::new(ResourceState::Empty),
            changed: Condvar::new(),
            shareable,
            cleanup_errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn cleanup(&self) -> Result<()> {
        let service = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
            let previous = std::mem::replace(&mut *state, ResourceState::Stopping);
            self.changed.notify_all();
            match previous {
                ResourceState::Ready { service, .. } => Some(service),
                ResourceState::Empty => {
                    *state = ResourceState::Empty;
                    self.changed.notify_all();
                    None
                }
                ResourceState::Failed(error) => {
                    *state = ResourceState::Failed(error);
                    self.changed.notify_all();
                    None
                }
                ResourceState::Starting | ResourceState::Stopping => {
                    *state = ResourceState::Empty;
                    self.changed.notify_all();
                    bail!("service resource is still transitioning during cleanup")
                }
            }
        };
        drop(service);
        if let Ok(mut state) = self.state.lock() {
            if matches!(*state, ResourceState::Stopping) {
                *state = ResourceState::Empty;
                self.changed.notify_all();
            }
        }
        Ok(())
    }

    fn take_cleanup_errors(&self) -> Result<Vec<String>> {
        let mut errors = self
            .cleanup_errors
            .lock()
            .map_err(|_| anyhow::anyhow!("service cleanup error lock was poisoned"))?;
        Ok(std::mem::take(&mut *errors))
    }

    fn acquire(
        self: &Arc<Self>,
        project: &Project,
        id: &str,
        config: ServiceConfig,
    ) -> Result<ServiceLease> {
        let started = Instant::now();
        loop {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
            match &mut *state {
                ResourceState::Ready { service, users } if self.shareable || *users == 0 => {
                    *users += 1;
                    return Ok(ServiceLease {
                        resource: Arc::clone(self),
                        inject_env: service.inject_env.clone(),
                        value: service.value.clone(),
                    });
                }
                ResourceState::Ready { .. } | ResourceState::Starting | ResourceState::Stopping => {
                    if crate::process::cancelled() {
                        bail!("verification cancelled while waiting for service {id:?} resource");
                    }
                    let remaining = RESOURCE_LOCK_WAIT.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        bail!("timed out waiting for service {id:?} resource");
                    }
                    let wait = remaining.min(RESOURCE_LOCK_POLL);
                    let (next, _) = self
                        .changed
                        .wait_timeout(state, wait)
                        .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                    drop(next);
                }
                ResourceState::Failed(error) => bail!("service {id:?} previously failed: {error}"),
                ResourceState::Empty => {
                    if crate::process::cancelled() {
                        bail!("verification cancelled while acquiring service {id:?} resource");
                    }
                    *state = ResourceState::Starting;
                    drop(state);
                    let result = RunningService::start(
                        project,
                        id,
                        config,
                        Arc::clone(&self.cleanup_errors),
                    );
                    // A cancellation can arrive while an adapter is starting.
                    // Drop any partial service and return the resource to Empty
                    // instead of publishing it as ready after cancellation.
                    if crate::process::cancelled() {
                        drop(result);
                        let mut state = self
                            .state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                        *state = ResourceState::Empty;
                        self.changed.notify_all();
                        bail!("verification cancelled while starting service {id:?}");
                    }
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                    match result {
                        Ok(service) => {
                            let inject_env = service.inject_env.clone();
                            let value = service.value.clone();
                            *state = ResourceState::Ready {
                                service: Box::new(service),
                                users: 1,
                            };
                            self.changed.notify_all();
                            return Ok(ServiceLease {
                                resource: Arc::clone(self),
                                inject_env,
                                value,
                            });
                        }
                        Err(error) => {
                            let detail = format!("{error:#}");
                            *state = ResourceState::Failed(detail.clone());
                            self.changed.notify_all();
                            bail!("{detail}");
                        }
                    }
                }
            }
        }
    }

    fn release(&self) {
        let service = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let ResourceState::Ready { users, .. } = &mut *state else {
                return;
            };
            *users = users.saturating_sub(1);
            if self.shareable || *users != 0 {
                self.changed.notify_all();
                return;
            }

            // Move the managed instance out before dropping the lock. The
            // Stopping marker makes waiters observe the teardown transition.
            let previous = std::mem::replace(&mut *state, ResourceState::Stopping);
            self.changed.notify_all();
            match previous {
                ResourceState::Ready { service, .. } => Some(service),
                _ => None,
            }
        };

        if let Some(service) = service {
            drop(service);
            if let Ok(mut state) = self.state.lock() {
                if matches!(*state, ResourceState::Stopping) {
                    *state = ResourceState::Empty;
                    self.changed.notify_all();
                }
            }
        }
    }

    fn prewarm(
        self: &Arc<Self>,
        project: &Project,
        id: &str,
        config: ServiceConfig,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        let started = Instant::now();
        loop {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) || crate::process::cancelled() {
                bail!("verification cancelled while warming service {id:?}");
            }
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
            match &*state {
                ResourceState::Ready { .. } => return Ok(()),
                ResourceState::Failed(error) => bail!("service {id:?} previously failed: {error}"),
                ResourceState::Starting | ResourceState::Stopping => {
                    let remaining = RESOURCE_LOCK_WAIT.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        bail!("timed out waiting for service {id:?} resource");
                    }
                    let wait = remaining.min(RESOURCE_LOCK_POLL);
                    let (next, _) = self
                        .changed
                        .wait_timeout(state, wait)
                        .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                    drop(next);
                }
                ResourceState::Empty => {
                    *state = ResourceState::Starting;
                    drop(state);
                    let result = RunningService::start(
                        project,
                        id,
                        config,
                        Arc::clone(&self.cleanup_errors),
                    );
                    if cancelled.load(std::sync::atomic::Ordering::Acquire)
                        || crate::process::cancelled()
                    {
                        drop(result);
                        let mut state = self
                            .state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                        *state = ResourceState::Empty;
                        self.changed.notify_all();
                        bail!("verification cancelled while warming service {id:?}");
                    }
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("service resource lock was poisoned"))?;
                    match result {
                        Ok(service) => {
                            *state = ResourceState::Ready {
                                service: Box::new(service),
                                users: 0,
                            };
                            self.changed.notify_all();
                            return Ok(());
                        }
                        Err(error) => {
                            let detail = format!("{error:#}");
                            *state = ResourceState::Failed(detail.clone());
                            self.changed.notify_all();
                            bail!("{detail}");
                        }
                    }
                }
            }
        }
    }

    fn discard_if_idle(&self) {
        let service = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let ResourceState::Ready { users, .. } = &mut *state else {
                return;
            };
            if *users != 0 {
                return;
            }
            let previous = std::mem::replace(&mut *state, ResourceState::Stopping);
            self.changed.notify_all();
            match previous {
                ResourceState::Ready { service, .. } => Some(service),
                _ => None,
            }
        };
        if let Some(service) = service {
            drop(service);
            if let Ok(mut state) = self.state.lock() {
                if matches!(*state, ResourceState::Stopping) {
                    *state = ResourceState::Empty;
                    self.changed.notify_all();
                }
            }
        }
    }
}

/// Owned worker payload used to overlap service startup with mandatory gates.
pub(super) struct ServiceWarmup {
    project: Project,
    id: String,
    config: ServiceConfig,
    resource: Arc<ServiceResource>,
}

impl ServiceWarmup {
    pub(super) fn run(self, cancelled: &std::sync::atomic::AtomicBool) {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) || crate::process::cancelled() {
            return;
        }
        let result = self
            .resource
            .prewarm(&self.project, &self.id, self.config, cancelled);
        if (cancelled.load(std::sync::atomic::Ordering::Acquire) || crate::process::cancelled())
            && result.is_ok()
        {
            self.resource.discard_if_idle();
        }
    }
}

pub(super) struct ServiceLease {
    resource: Arc<ServiceResource>,
    inject_env: String,
    value: String,
}

pub(super) struct ServiceHandle<'a> {
    project: &'a Project,
    id: String,
    config: ServiceConfig,
    resource: Arc<ServiceResource>,
}

impl ServiceHandle<'_> {
    pub(super) fn acquire(self) -> Result<ServiceLease> {
        self.resource.acquire(self.project, &self.id, self.config)
    }
}

impl ServiceLease {
    pub(super) fn environment(&self) -> (String, String) {
        (self.inject_env.clone(), self.value.clone())
    }
}

impl Drop for ServiceLease {
    fn drop(&mut self) {
        self.resource.release();
    }
}

struct RunningService {
    runtime: crate::config::ContainerRuntimeKind,
    inject_env: String,
    value: String,
    container: Option<String>,
    project: Project,
    cleanup_errors: Arc<Mutex<Vec<String>>>,
    lease: Option<ResourceLease>,
}

impl Drop for RunningService {
    fn drop(&mut self) {
        let mut cleanup_succeeded = true;
        if let Some(container) = &self.container {
            let ownership = self
                .lease
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("managed container has no ownership lease"))
                .and_then(|lease| lease.verify_runtime_ownership(&self.project));
            if let Err(error) = ownership.and_then(|()| {
                self.runtime
                    .stop_container(&self.project.root, container, Duration::from_secs(5))
            }) {
                cleanup_succeeded = false;
                let detail = format!(
                    "failed to clean up {} container {container:?}: {error:#}",
                    self.runtime.executable()
                );
                if let Ok(mut errors) = self.cleanup_errors.lock() {
                    errors.push(detail.clone());
                }
                eprintln!("warning: {detail}");
            }
        }
        if cleanup_succeeded {
            // Stop the heartbeat before release so an ownership uncertainty
            // is surfaced to the verification report and the marker remains.
            if let Some(lease) = self.lease.take() {
                if let Err(error) = lease.release_checked() {
                    let container = self.container.as_deref().unwrap_or("<unknown>");
                    let detail = format!(
                        "failed to release {} lease for container {container:?}: {error:#}",
                        self.runtime.executable()
                    );
                    if let Ok(mut errors) = self.cleanup_errors.lock() {
                        errors.push(detail.clone());
                    }
                    eprintln!("warning: {detail}");
                }
            }
        } else if let Some(lease) = self.lease.take() {
            // Keep the marker available to `harness-gate cleanup` when the
            // adapter could not stop the resource during normal teardown.
            lease.retain();
        }
    }
}

impl RunningService {
    fn start(
        project: &Project,
        id: &str,
        config: ServiceConfig,
        cleanup_errors: Arc<Mutex<Vec<String>>>,
    ) -> Result<Self> {
        match config {
            ServiceConfig::Environment {
                source_env,
                inject_env,
            } => {
                let value = std::env::var(&source_env)
                    .with_context(|| format!("service {id:?} requires {source_env}"))?;
                if value.trim().is_empty() {
                    bail!("service {id:?} requires non-empty {source_env}");
                }
                Ok(Self {
                    runtime: crate::config::ContainerRuntimeKind::Docker,
                    inject_env,
                    value,
                    container: None,
                    project: project.clone(),
                    cleanup_errors,
                    lease: None,
                })
            }
            ServiceConfig::Docker {
                runtime,
                image,
                external_env,
                inject_env,
                external_value_policy,
                startup_timeout_secs,
                container_port,
                environment,
                healthcheck,
                connection,
                ..
            } => {
                if let Some((_, value)) = external_env
                    .as_ref()
                    .and_then(|name| std::env::var(name).ok().map(|value| (name, value)))
                    .filter(|(_, value)| !value.trim().is_empty())
                {
                    validate_external_value(external_value_policy, &value)?;
                    return Ok(Self {
                        runtime,
                        inject_env,
                        value,
                        container: None,
                        project: project.clone(),
                        cleanup_errors,
                        lease: None,
                    });
                }
                let deadline = Instant::now() + Duration::from_secs(startup_timeout_secs);
                runtime
                    .check_available(project, remaining(deadline)?)
                    .with_context(|| format!("container runtime unavailable for service {id:?}"))?;
                start_docker(
                    project,
                    id,
                    DockerStartOptions {
                        image,
                        runtime,
                        inject_env,
                        startup_timeout_secs,
                        container_port,
                        environment,
                        healthcheck,
                        connection,
                        deadline,
                        cleanup_errors,
                        invocation_id: project.invocation_id(),
                    },
                )
            }
        }
    }
}
pub fn check_available(project: &Project, id: &str, timeout: Duration) -> Result<String> {
    let service = project
        .config
        .service(id)
        .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
    match service {
        ServiceConfig::Environment { source_env, .. } => {
            let value = std::env::var(source_env)
                .with_context(|| format!("{source_env} is not configured"))?;
            if value.trim().is_empty() {
                bail!("{source_env} is empty");
            }
            Ok(format!("{source_env} is configured"))
        }
        ServiceConfig::Docker {
            runtime,
            image,
            external_env,
            external_value_policy,
            ..
        } => {
            if let Some(name) = external_env {
                if let Ok(value) = std::env::var(name) {
                    if !value.trim().is_empty() {
                        validate_external_value(*external_value_policy, &value)?;
                        return Ok(format!("{name} is configured"));
                    }
                }
            }
            let deadline = Instant::now() + timeout;
            runtime
                .check_available(project, remaining(deadline)?)
                .with_context(|| format!("container runtime unavailable for service {id:?}"))?;
            let args = vec!["image".into(), "inspect".into(), image.clone()];
            let image_ready = crate::process::capture(
                runtime.executable(),
                &args,
                &project.root,
                remaining(deadline)?,
            )?
            .status
            .success();
            if !image_ready {
                bail!(
                    "{} image {image} is not available; run `{} pull {image}`",
                    runtime.executable(),
                    runtime.executable()
                );
            }
            Ok(format!("{} and {image} ready", runtime.executable()))
        }
    }
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| anyhow::anyhow!("service operation timed out"))
}
