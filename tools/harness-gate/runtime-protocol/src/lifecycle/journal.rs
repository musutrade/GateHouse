//! Lifecycle evidence journal. No process execution or old-lease conversion.
//! Only the private durable store may publish a record or issue dispatch tokens.
use super::{
    state::{Event, Phase, State},
    wire::{self, Binding, ContainerState, Operation, Outcome, Request, Response, Target},
};
use crate::{Invalid, Result};
use serde::{Deserialize, Serialize};

#[cfg(unix)]
mod storage;
#[cfg(unix)]
pub use storage::{Dispatch, Journal};

const SCHEMA: &str = "gatehouse-lifecycle-lease/v1";
const MAX_ATTEMPTS: usize = 128;

/// Supplied by the trusted Core command path, never parsed from adapter output.
/// This expresses policy; it does not authenticate the human/operator itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    Service,
    OperatorCleanup,
    Reconcile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    request: Request,
    purpose: Purpose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    request: Request,
    purpose: Purpose,
    // None includes abandoned/unknown outcomes. Never infer no side effect.
    response: Option<Response>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    create: Request,
    state: State,
    revision: u64,
    pending: Option<Pending>,
    attempts: Vec<Attempt>,
    absence: Option<Response>,
}
fn same_authority(a: &Binding, b: &Binding) -> bool {
    let mut a = a.clone();
    a.invocation = b.invocation.clone();
    a.operation_key = b.operation_key.clone();
    a.generation = b.generation;
    a == *b
}
impl Record {
    fn new(create: &Request) -> Result<Self> {
        create.validate()?;
        if !matches!(create.operation, Operation::Create { .. }) || create.binding.generation != 1 {
            return Err(Invalid("journal requires first create generation"));
        }
        Ok(Self {
            schema: SCHEMA.into(),
            create: create.clone(),
            state: State::pending_create(),
            revision: 1,
            pending: Some(Pending {
                request: create.clone(),
                purpose: Purpose::Service,
            }),
            attempts: vec![],
            absence: None,
        })
    }
    fn target(&self) -> Result<Target> {
        let Operation::Create {
            name,
            labels,
            container_port,
            ..
        } = &self.create.operation
        else {
            return Err(Invalid("invalid journal create"));
        };
        Ok(Target {
            object_id: self
                .state
                .object_id()
                .ok_or(Invalid("journal object unresolved"))?
                .into(),
            name: name.clone(),
            labels: labels.clone(),
            container_port: *container_port,
        })
    }
    fn check_request(&self, r: &Request) -> Result<()> {
        r.validate()?;
        if !same_authority(&r.binding, &self.create.binding) {
            return Err(Invalid("journal authority mismatch"));
        }
        let target = match &r.operation {
            Operation::Status { target }
            | Operation::Start { target }
            | Operation::Stop { target, .. }
            | Operation::Remove { target }
            | Operation::HealthExec { target, .. } => target,
            _ => return Err(Invalid("journal operation unsupported after creation")),
        };
        if *target != self.target()? {
            return Err(Invalid("journal target mismatch"));
        }
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA || self.attempts.len() > MAX_ATTEMPTS {
            return Err(Invalid("invalid lifecycle journal"));
        }
        self.create.validate()?;
        self.state.validate()?;
        if !matches!(self.create.operation, Operation::Create { .. })
            || self.create.binding.generation != 1
        {
            return Err(Invalid("invalid journal origin"));
        }
        let mut revision = 0;
        let mut nonces = std::collections::BTreeSet::new();
        for a in &self.attempts {
            if a.request.binding.generation != revision + 1
                || !nonces.insert(&a.request.binding.invocation)
            {
                return Err(Invalid("journal generation or nonce replay"));
            }
            a.request.validate()?;
            if revision == 0 {
                if a.request != self.create || a.purpose != Purpose::Service {
                    return Err(Invalid("journal creation history mismatch"));
                }
                match a.response.as_ref().map(|r| &r.outcome) {
                    Some(Outcome::Created { object })
                        if self.state.object_id() == Some(object.object_id.as_str()) => {}
                    None | Some(Outcome::Unknown { .. } | Outcome::RejectedBeforeEffect { .. })
                        if self.state.object_id().is_none() => {}
                    _ => return Err(Invalid("journal object has no creation evidence")),
                }
            } else {
                self.check_request(&a.request)?;
            }
            if !same_authority(&a.request.binding, &self.create.binding) {
                return Err(Invalid("journal historical authority mismatch"));
            }
            if let Some(r) = &a.response {
                r.validate(&a.request)?;
            }
            revision += 1;
        }
        if let Some(p) = &self.pending {
            if p.request.binding.generation != revision + 1
                || !nonces.insert(&p.request.binding.invocation)
            {
                return Err(Invalid("journal pending replay"));
            }
            p.request.validate()?;
            if revision == 0 {
                if p.request != self.create
                    || p.purpose != Purpose::Service
                    || self.state.phase() != Phase::PendingCreate
                {
                    return Err(Invalid("journal initial intent mismatch"));
                }
            } else {
                self.check_request(&p.request)?;
            }
            if !same_authority(&p.request.binding, &self.create.binding) {
                return Err(Invalid("journal pending authority mismatch"));
            }
            revision += 1;
        }
        if revision != self.revision {
            return Err(Invalid("journal revision mismatch"));
        }
        if self.state.phase() == Phase::VerifiedAbsent {
            let a = self
                .attempts
                .last()
                .ok_or(Invalid("missing absence attempt"))?;
            if a.purpose == Purpose::Service
                || (a.purpose == Purpose::Reconcile
                    && !self
                        .attempts
                        .iter()
                        .any(|old| matches!(old.request.operation, Operation::Remove { .. })))
                || self.pending.is_some()
                || self.absence.as_ref() != a.response.as_ref()
                || !matches!(
                    self.absence.as_ref().map(|r| &r.outcome),
                    Some(Outcome::Absent { .. })
                )
            {
                return Err(Invalid("missing journal absence evidence"));
            }
        } else if self.absence.is_some() {
            return Err(Invalid("premature absence evidence"));
        }
        Ok(())
    }
    fn begin(&self, r: &Request, purpose: Purpose, fresh_status: bool) -> Result<Self> {
        self.validate()?;
        self.check_request(r)?;
        if self.pending.is_some()
            || self.attempts.len() >= MAX_ATTEMPTS
            || self.state.phase() == Phase::VerifiedAbsent
            || r.binding.generation
                != self
                    .revision
                    .checked_add(1)
                    .ok_or(Invalid("generation overflow"))?
            || self
                .attempts
                .iter()
                .any(|a| a.request.binding.invocation == r.binding.invocation)
        {
            return Err(Invalid("journal request replay or unfinished attempt"));
        }
        let mut next = self.clone();
        let status = self
            .attempts
            .last()
            .filter(|_| fresh_status)
            .and_then(|a| a.response.as_ref())
            .filter(|r| matches!(r.operation, Operation::Status { .. }));
        let observed = match status.map(|r| &r.outcome) {
            Some(Outcome::Status { object }) => Some(object.state),
            _ => None,
        };
        next.state = match &r.operation {
            Operation::Status { .. } => {
                if matches!(
                    self.state.phase(),
                    Phase::Retained | Phase::ReconcilingRemove
                ) && purpose == Purpose::Service
                {
                    return Err(Invalid("recovered lease requires explicit reconciliation"));
                }
                self.state.clone()
            }
            Operation::Start { .. }
                if purpose == Purpose::Service && observed == Some(ContainerState::Created) =>
            {
                self.state.transition(Event::BeginStart)?
            }
            Operation::HealthExec { .. }
                if purpose == Purpose::Service && observed == Some(ContainerState::Running) =>
            {
                self.state.transition(Event::BeginHealth)?
            }
            Operation::Stop { .. }
                if purpose == Purpose::OperatorCleanup
                    && observed == Some(ContainerState::Running) =>
            {
                let ev = if self.state.phase() == Phase::ReconcilingRemove {
                    Event::OperatorCleanupRunningVerified
                } else {
                    Event::CleanupRunningVerified
                };
                self.state.transition(ev)?
            }
            Operation::Remove { .. }
                if purpose == Purpose::OperatorCleanup
                    && matches!(
                        observed,
                        Some(ContainerState::Created | ContainerState::Exited)
                    ) =>
            {
                let stopped = if self.state.phase() == Phase::Stopped {
                    self.state.clone()
                } else {
                    self.state
                        .transition(if self.state.phase() == Phase::ReconcilingRemove {
                            Event::OperatorCleanupTerminalVerified
                        } else {
                            Event::CleanupTerminalVerified
                        })?
                };
                stopped.transition(Event::BeginRemove)?
            }
            _ => {
                return Err(Invalid(
                    "journal mutation lacks fresh evidence or authorization",
                ))
            }
        };
        next.revision = r.binding.generation;
        next.pending = Some(Pending {
            request: r.clone(),
            purpose,
        });
        next.validate()?;
        Ok(next)
    }
    fn observe(&self, bytes: Option<&[u8]>) -> Result<Self> {
        self.validate()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(Invalid("no journal request pending"))?;
        let response = bytes
            .map(|b| wire::parse_response(b, &pending.request))
            .transpose()?;
        let mut next = self.clone();
        let outcome = response.as_ref().map(|r| &r.outcome);
        // The exact pinned image remains immutable across every returned object.
        if let Some(
            Outcome::Created { object }
            | Outcome::Status { object }
            | Outcome::Started { object }
            | Outcome::Stopped { object }
            | Outcome::Health { object, .. },
        ) = outcome
        {
            if let Operation::Create { image_id, .. } = &self.create.operation {
                if &object.image_id != image_id {
                    return Err(Invalid("journal image changed"));
                }
            }
        }
        next.state = match (&pending.request.operation, outcome) {
            (_, None | Some(Outcome::Unknown { .. } | Outcome::RejectedBeforeEffect { .. })) => {
                self.state.recover()?
            }
            (Operation::Create { .. }, Some(Outcome::Created { object })) => self
                .state
                .transition(Event::Created(object.object_id.clone()))?,
            (Operation::Status { .. }, Some(Outcome::Status { .. })) => self.state.clone(),
            (Operation::Status { .. }, Some(Outcome::Absent { .. })) => {
                let ev = match (self.state.phase(), pending.purpose) {
                    (
                        Phase::AwaitingAbsence | Phase::ReconcilingRemove,
                        Purpose::Reconcile | Purpose::OperatorCleanup,
                    ) => Event::AbsenceVerified,
                    (Phase::Retained, Purpose::OperatorCleanup) => Event::OperatorAbsenceVerified,
                    _ => return Err(Invalid("absence lacks reconciliation authorization")),
                };
                next.absence = response.clone();
                self.state.transition(ev)?
            }
            (Operation::Start { .. }, Some(Outcome::Started { .. })) => {
                self.state.transition(Event::RunningVerified)?
            }
            (Operation::Stop { .. }, Some(Outcome::Stopped { .. })) => {
                self.state.transition(Event::StoppedVerified)?
            }
            (Operation::Remove { .. }, Some(Outcome::Removed)) => {
                self.state.transition(Event::RemoveClaimed)?
            }
            (Operation::HealthExec { .. }, Some(Outcome::Health { exit_code: 0, .. })) => {
                self.state.transition(Event::HealthVerified)?
            }
            (Operation::HealthExec { .. }, Some(Outcome::Health { .. })) => {
                self.state.transition(Event::Failed)?
            }
            _ => return Err(Invalid("journal outcome cannot advance state")),
        };
        next.attempts.push(Attempt {
            request: pending.request.clone(),
            purpose: pending.purpose,
            response,
        });
        next.pending = None;
        next.validate()?;
        Ok(next)
    }
    fn recover(&self) -> Result<Self> {
        let mut next = if self.pending.is_some() {
            self.observe(None)?
        } else {
            self.clone()
        };
        next.state = next.state.recover()?;
        next.validate()?;
        Ok(next)
    }
}
