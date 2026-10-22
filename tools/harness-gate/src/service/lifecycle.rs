//! Core service coordination. Configured images can only enter through Reservation.
//! No CLI, readiness authority, direct Create journal or automatic recovery replay.
use crate::process::runtime_lifecycle_host as host;
use anyhow::{bail, Context, Result};
use gatehouse_runtime_protocol::lifecycle::{
    installation::VerifiedInstallation,
    journal::{CreatePlan, Journal, Purpose, ServiceSlot},
    wire::{Binding, ContainerState, Operation, Outcome, Request, Response, PROTOCOL},
};
use std::{path::Path, time::Instant};

pub(crate) struct ServiceIdentity {
    pub project_id: String,
    pub resource_id: String,
    pub lease_invocation: String,
}

/// The private fields prevent adopting a free Request or a generation-1 journal.
/// Dropping a session retains its durable journal; cleanup is an explicit action.
pub(crate) struct LifecycleService {
    installation: VerifiedInstallation,
    slot: ServiceSlot,
    journal: Journal,
    create: Request,
    port: u16,
}

fn nonce() -> Result<String> {
    let mut bytes = [0u8; 16];
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: the remaining slice is writable for exactly the supplied length.
        let n = unsafe {
            libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
        };
        if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if n <= 0 {
            bail!("lifecycle invocation entropy unavailable");
        }
        offset += n as usize;
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn remaining(deadline: Instant) -> Result<u64> {
    let ms = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    if ms == 0 {
        bail!("lifecycle service deadline exhausted; retain journal");
    }
    Ok(ms.min(u64::MAX as u128) as u64)
}
fn outcome(result: std::result::Result<Response, host::DispatchError>) -> Result<Response> {
    result.map_err(|e| anyhow::anyhow!("lifecycle dispatch failed; retain journal: {e:?}"))
}

/// Immutable, fenced configured-service inputs. Only prepare() constructs it.
pub(crate) struct PreparedService {
    installation: VerifiedInstallation,
    slot: ServiceSlot,
}
impl PreparedService {
    pub(crate) fn start(
        self,
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> Result<LifecycleService> {
        LifecycleService::start(self, deadline, cancelled)
    }
}
impl LifecycleService {
    /// The caller supplies a Core-owned resource identity and protected directory.
    /// Only the held installation supplies engine identity and artifact digests.
    pub(crate) fn prepare(
        root: &Path,
        installation: VerifiedInstallation,
        identity: ServiceIdentity,
        image: String,
        plan: CreatePlan,
        healthcheck: &[String],
    ) -> Result<PreparedService> {
        let snapshot = installation.snapshot()?;
        let mut resolve = Request {
            protocol: PROTOCOL.into(),
            binding: Binding {
                installation_sha256: snapshot.installation_sha256,
                executable_sha256: snapshot.executable_sha256,
                config_sha256: snapshot.config_sha256,
                engine_binding: snapshot.engine_binding,
                project_id: identity.project_id,
                resource_id: identity.resource_id,
                lease_invocation: identity.lease_invocation,
                invocation: nonce()?,
                generation: 1,
                operation_key: String::new(),
            },
            operation: Operation::ResolveImage { image },
        };
        resolve.binding.operation_key = resolve.operation_key()?;
        let slot = ServiceSlot::acquire(root, resolve, plan, healthcheck.to_vec())?;
        Ok(PreparedService { installation, slot })
    }
    fn start(
        prepared: PreparedService,
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self> {
        let PreparedService {
            installation,
            mut slot,
        } = prepared;
        slot.check()?;
        installation.validate_request(slot.resolve_request())?;
        let container_port = slot.plan().container_port;
        let healthcheck = slot.healthcheck().to_vec();
        remaining(deadline)?;
        if cancelled() {
            bail!("lifecycle service cancelled; slot retained");
        }
        let mut reservation = slot.reserve()?;
        // Compute the deadline before borrowing/consuming the token. Any failure
        // leaves durable obstruction; this API never retries a reservation.
        let timeout = remaining(deadline)?;
        outcome(host::resolve(
            reservation.dispatch()?,
            &installation,
            timeout,
            &cancelled,
        ))?;
        slot.check()?;
        let invocation = nonce()?;
        let mut journal = reservation.into_journal(&invocation)?;
        let timeout = remaining(deadline)?;
        let token = journal.dispatch()?;
        let create = token.request().clone();
        if create.binding.generation != 2 {
            bail!("configured-image service requires resolved generation-2 create");
        }
        let created = outcome(host::dispatch(token, &installation, timeout, &cancelled))?;
        if !matches!(created.outcome, Outcome::Created { .. }) {
            bail!("lifecycle service creation not proved; retain journal");
        }
        slot.check()?;
        let mut service = Self {
            installation,
            slot,
            journal,
            create,
            port: 0,
        };
        service.require_state(
            Purpose::Service,
            &[ContainerState::Created],
            deadline,
            &cancelled,
        )?;
        let target = service.journal.target()?;
        let started = service.execute(
            Operation::Start { target },
            Purpose::Service,
            deadline,
            &cancelled,
        )?;
        let Outcome::Started { object } = started.outcome else {
            bail!("lifecycle service start not proved; retain journal");
        };
        let [port] = object.ports.as_slice() else {
            bail!("service requires exactly one bound port; retain journal");
        };
        if port.container_port != container_port
            || port.protocol != "tcp"
            || port.host_ip != "127.0.0.1"
            || port.host_port == 0
        {
            bail!("service port differs from immutable plan; retain journal");
        }
        service.port = port.host_port;
        if !healthcheck.is_empty() {
            service.require_state(
                Purpose::Service,
                &[ContainerState::Running],
                deadline,
                &cancelled,
            )?;
            let target = service.journal.target()?;
            let timeout_ms = remaining(deadline)?.min(30_000) as u32;
            let health = service.execute(
                Operation::HealthExec {
                    target,
                    argv: healthcheck.to_vec(),
                    timeout_ms,
                },
                Purpose::Service,
                deadline,
                &cancelled,
            )?;
            if !matches!(health.outcome, Outcome::Health { exit_code: 0, .. }) {
                bail!("lifecycle service health failed; retain journal");
            }
        }
        Ok(service)
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    fn execute(
        &mut self,
        operation: Operation,
        purpose: Purpose,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Response> {
        self.slot.check()?;
        let timeout = remaining(deadline)?;
        let mut request = self.create.clone();
        request.operation = operation;
        request.binding.generation = self
            .journal
            .revision()
            .checked_add(1)
            .context("lifecycle generation exhausted")?;
        request.binding.invocation = nonce()?;
        request.binding.operation_key = request.operation_key()?;
        let response = outcome(host::dispatch(
            self.journal.begin(&request, purpose)?,
            &self.installation,
            timeout,
            cancelled,
        ))?;
        self.slot.check()?;
        Ok(response)
    }
    fn status(
        &mut self,
        purpose: Purpose,
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Response> {
        let target = self.journal.target()?;
        self.execute(Operation::Status { target }, purpose, deadline, cancelled)
    }

    fn require_state(
        &mut self,
        purpose: Purpose,
        expected: &[ContainerState],
        deadline: Instant,
        cancelled: &impl Fn() -> bool,
    ) -> Result<()> {
        match self.status(purpose, deadline, cancelled)?.outcome {
            Outcome::Status { object } if expected.contains(&object.state) => Ok(()),
            _ => bail!("service observed unexpected state; retain slot/journal"),
        }
    }

    /// Only normal live-service teardown; recovery needs separate explicit operator
    /// policy. Never force-remove or treat a successful DELETE as proof of absence.
    pub(crate) fn cleanup(mut self, deadline: Instant, cancelled: impl Fn() -> bool) -> Result<()> {
        let status = self.status(Purpose::ServiceCleanup, deadline, &cancelled)?;
        match status.outcome {
            Outcome::Absent { .. } => return self.slot.finish(self.journal).map_err(Into::into),
            Outcome::Status { object } if object.state == ContainerState::Running => {
                let target = self.journal.target()?;
                self.execute(
                    Operation::Stop {
                        target,
                        grace_seconds: 5,
                    },
                    Purpose::ServiceCleanup,
                    deadline,
                    &cancelled,
                )?;
                self.require_state(
                    Purpose::ServiceCleanup,
                    &[ContainerState::Created, ContainerState::Exited],
                    deadline,
                    &cancelled,
                )?;
            }
            _ => bail!("service cleanup requires live running ownership; retain journal"),
        }
        let target = self.journal.target()?;
        self.execute(
            Operation::Remove { target },
            Purpose::ServiceCleanup,
            deadline,
            &cancelled,
        )?;
        // Post-remove absence is reconciliation evidence, not mutation authority.
        let absent = self.status(Purpose::Reconcile, deadline, &cancelled)?;
        if !matches!(absent.outcome, Outcome::Absent { .. }) {
            bail!("service removal absence not proved; retain journal");
        }
        self.slot.finish(self.journal)?;
        Ok(())
    }
}
