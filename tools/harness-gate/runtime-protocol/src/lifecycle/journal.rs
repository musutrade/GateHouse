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
pub use storage::{Dispatch, Journal, Reservation, ResolutionDispatch};

mod resolution;
pub use resolution::CreatePlan;
use resolution::ResolutionProof;

const SCHEMA: &str = "gatehouse-lifecycle-lease/v1";
const MAX_ATTEMPTS: usize = 128;

/// Supplied by the trusted Core command path, never parsed from adapter output.
/// This expresses policy; it does not authenticate the human/operator itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    Service,
    ServiceCleanup,
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
#[serde(rename_all = "kebab-case")]
enum AbsenceReason {
    OperatorObservedAbsence,
    PostRemoveAbsence,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AbsenceProof {
    reason: AbsenceReason,
    remove_request_sha256: Option<String>,
    status_request_sha256: String,
    response: Response,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    create: Request,
    resolution: Option<ResolutionProof>,
    state: State,
    revision: u64,
    pending: Option<Pending>,
    attempts: Vec<Attempt>,
    absence: Option<AbsenceProof>,
    recoveries: Vec<u64>,
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
        Self::new_resolved(create, None)
    }
    fn new_resolved(create: &Request, resolution: Option<ResolutionProof>) -> Result<Self> {
        create.validate()?;
        if !matches!(create.operation, Operation::Create { .. })
            || create.binding.generation != if resolution.is_some() { 2 } else { 1 }
        {
            return Err(Invalid("journal requires first create generation"));
        }
        if let Some(proof) = &resolution {
            proof.validate_create(create)?;
        }
        Ok(Self {
            schema: SCHEMA.into(),
            create: create.clone(),
            resolution,
            state: State::pending_create(),
            revision: create.binding.generation,
            pending: Some(Pending {
                request: create.clone(),
                purpose: Purpose::Service,
            }),
            attempts: vec![],
            absence: None,
            recoveries: vec![],
        })
    }
    fn marker(&self) -> &str {
        self.resolution
            .as_ref()
            .map(|p| p.request.binding.operation_key.as_str())
            .unwrap_or(&self.create.binding.operation_key)
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
            || self.create.binding.generation != if self.resolution.is_some() { 2 } else { 1 }
        {
            return Err(Invalid("invalid journal origin"));
        }
        let base = self.create.binding.generation - 1;
        let mut revision = base;
        let mut nonces = std::collections::BTreeSet::new();
        if let Some(proof) = &self.resolution {
            proof.validate_create(&self.create)?;
            nonces.insert(&proof.request.binding.invocation);
        }
        for a in &self.attempts {
            if a.request.binding.generation != revision + 1
                || !nonces.insert(&a.request.binding.invocation)
            {
                return Err(Invalid("journal generation or nonce replay"));
            }
            a.request.validate()?;
            if revision == base {
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
            if revision == base {
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
                || self.absence.as_ref().map(|p| &p.response) != a.response.as_ref()
                || !matches!(
                    self.absence.as_ref().map(|p| &p.response.outcome),
                    Some(Outcome::Absent { .. })
                )
            {
                return Err(Invalid("missing journal absence evidence"));
            }
        } else if self.absence.is_some() {
            return Err(Invalid("premature absence evidence"));
        }
        self.replay()?;
        Ok(())
    }
    fn replay(&self) -> Result<()> {
        if self.recoveries.windows(2).any(|w| w[0] >= w[1])
            || self.recoveries.iter().any(|g| {
                *g < self.create.binding.generation
                    || *g > self.attempts.len() as u64 + self.create.binding.generation - 1
            })
        {
            return Err(Invalid("invalid journal recovery history"));
        }
        let mut replay = Self::new_resolved(&self.create, self.resolution.clone())?;
        let mut fresh = false;
        for (index, a) in self.attempts.iter().enumerate() {
            if index > 0 {
                replay = replay.begin_inner(&a.request, a.purpose, fresh)?;
            }
            let bytes = a
                .response
                .as_ref()
                .map(|r| {
                    super::canonical::encode(
                        &serde_json::to_value(r).map_err(|_| Invalid("invalid stored response"))?,
                    )
                })
                .transpose()?;
            replay = replay.observe_inner(bytes.as_deref())?;
            fresh = matches!(
                a.response.as_ref().map(|r| (&r.operation, &r.outcome)),
                Some((Operation::Status { .. }, Outcome::Status { .. }))
            );
            if self.recoveries.contains(&replay.revision) {
                replay.state = replay.state.recover()?;
                replay.recoveries.push(replay.revision);
                fresh = false;
            }
        }
        if !self.attempts.is_empty() {
            if let Some(p) = &self.pending {
                replay = replay.begin_inner(&p.request, p.purpose, fresh)?;
            }
        }
        if replay != *self {
            return Err(Invalid(
                "journal state does not follow its evidence history",
            ));
        }
        Ok(())
    }
    fn begin(&self, r: &Request, purpose: Purpose, fresh_status: bool) -> Result<Self> {
        self.validate()?;
        let next = self.begin_inner(r, purpose, fresh_status)?;
        next.validate()?;
        Ok(next)
    }
    fn normal_cleanup(&self) -> bool {
        self.state.phase() == Phase::Running
            || (self.state.phase() == Phase::Stopped
                && self
                    .attempts
                    .iter()
                    .rev()
                    .find(|a| !matches!(a.request.operation, Operation::Status { .. }))
                    .is_some_and(|a| {
                        a.purpose == Purpose::ServiceCleanup
                            && matches!(
                                a.response.as_ref().map(|r| &r.outcome),
                                Some(Outcome::Stopped { .. })
                            )
                    }))
    }
    fn begin_inner(&self, r: &Request, purpose: Purpose, fresh_status: bool) -> Result<Self> {
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
                ) && matches!(purpose, Purpose::Service | Purpose::ServiceCleanup)
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
                if (purpose == Purpose::OperatorCleanup
                    || (purpose == Purpose::ServiceCleanup && self.normal_cleanup()))
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
                if (purpose == Purpose::OperatorCleanup
                    || (purpose == Purpose::ServiceCleanup && self.normal_cleanup()))
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
        Ok(next)
    }
    fn observe(&self, bytes: Option<&[u8]>) -> Result<Self> {
        self.validate()?;
        let next = self.observe_inner(bytes)?;
        next.validate()?;
        Ok(next)
    }
    fn observe_inner(&self, bytes: Option<&[u8]>) -> Result<Self> {
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
            (Operation::Status { .. }, Some(Outcome::Status { .. })) => {
                if matches!(self.state.phase(), Phase::AwaitingAbsence | Phase::Removing) {
                    if !matches!(
                        pending.purpose,
                        Purpose::OperatorCleanup | Purpose::Reconcile
                    ) {
                        return Err(Invalid("present removal requires reconciliation purpose"));
                    }
                    self.state.recover()?
                } else {
                    self.state.clone()
                }
            }
            (Operation::Status { .. }, Some(Outcome::Absent { .. })) => {
                let ev = match (self.state.phase(), pending.purpose) {
                    (
                        Phase::AwaitingAbsence | Phase::ReconcilingRemove,
                        Purpose::Reconcile | Purpose::OperatorCleanup,
                    ) => Event::AbsenceVerified,
                    (Phase::Retained, Purpose::OperatorCleanup) => Event::OperatorAbsenceVerified,
                    _ => return Err(Invalid("absence lacks reconciliation authorization")),
                };
                let remove = self
                    .attempts
                    .iter()
                    .rev()
                    .find(|a| matches!(a.request.operation, Operation::Remove { .. }))
                    .map(|a| a.request.binding.operation_key.clone());
                next.absence = Some(AbsenceProof {
                    reason: if remove.is_some() {
                        AbsenceReason::PostRemoveAbsence
                    } else {
                        AbsenceReason::OperatorObservedAbsence
                    },
                    remove_request_sha256: remove,
                    status_request_sha256: pending.request.binding.operation_key.clone(),
                    response: response
                        .clone()
                        .ok_or(Invalid("missing absence response"))?,
                });
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
        Ok(next)
    }
    fn recover(&self) -> Result<Self> {
        let mut next = if self.pending.is_some() {
            self.observe(None)?
        } else {
            self.clone()
        };
        let recovered = next.state.recover()?;
        if recovered != next.state {
            next.recoveries.push(next.revision);
            next.state = recovered;
        }
        next.validate()?;
        Ok(next)
    }
}
