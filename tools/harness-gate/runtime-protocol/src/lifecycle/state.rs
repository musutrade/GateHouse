//! Pure lease transitions, not a journal or evidence verifier.
//! The caller must validate evidence and persist before dispatching effects.
use crate::{digest, Invalid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    PendingCreate,
    UnresolvedNoId,
    KnownCreated,
    Starting,
    Running,
    CheckingHealth,
    Retained,
    Stopping,
    Stopped,
    Removing,
    AwaitingAbsence,
    ReconcilingRemove,
    VerifiedAbsent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    phase: Phase,
    object_id: Option<String>,
}

/// Events describe already verified facts or an intent to persist. They are not
/// deserializable adapter messages and do not themselves authenticate evidence.
#[derive(Debug)]
pub enum Event {
    Created(String),
    BeginStart,
    RunningVerified,
    BeginHealth,
    HealthVerified,
    Failed,
    CleanupRunningVerified,
    CleanupTerminalVerified,
    StoppedVerified,
    BeginRemove,
    RemoveClaimed,
    AbsenceVerified,
    OperatorCleanupRunningVerified,
    OperatorCleanupTerminalVerified,
    OperatorAbsenceVerified,
}

impl State {
    pub fn pending_create() -> Self {
        Self {
            phase: Phase::PendingCreate,
            object_id: None,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn object_id(&self) -> Option<&str> {
        self.object_id.as_deref()
    }

    pub fn validate(&self) -> Result<(), Invalid> {
        match (&self.phase, &self.object_id) {
            (Phase::PendingCreate | Phase::UnresolvedNoId, None) => Ok(()),
            (Phase::PendingCreate | Phase::UnresolvedNoId, Some(_)) | (_, None) => {
                Err(Invalid("invalid lifecycle state identity"))
            }
            (_, Some(id)) => digest(id),
        }
    }

    pub fn blocks_acquisition(&self) -> bool {
        self.validate().is_err() || self.phase != Phase::VerifiedAbsent
    }

    /// Recovery never replays an operation and never infers an unknown ID.
    pub fn recover(&self) -> Result<Self, Invalid> {
        self.validate()?;
        let mut next = self.clone();
        next.phase = match self.phase {
            Phase::PendingCreate | Phase::UnresolvedNoId => Phase::UnresolvedNoId,
            Phase::Removing | Phase::AwaitingAbsence | Phase::ReconcilingRemove => {
                Phase::ReconcilingRemove
            }
            Phase::VerifiedAbsent => Phase::VerifiedAbsent,
            _ => Phase::Retained,
        };
        Ok(next)
    }

    pub fn transition(&self, event: Event) -> Result<Self, Invalid> {
        self.validate()?;
        let mut next = self.clone();
        next.phase = match (self.phase, event) {
            (Phase::PendingCreate, Event::Created(id)) => {
                digest(&id)?;
                next.object_id = Some(id);
                Phase::KnownCreated
            }
            (Phase::PendingCreate, Event::Failed) => Phase::UnresolvedNoId,
            (Phase::KnownCreated, Event::BeginStart) => Phase::Starting,
            (Phase::Starting, Event::RunningVerified) => Phase::Running,
            (Phase::Running, Event::BeginHealth) => Phase::CheckingHealth,
            (Phase::CheckingHealth, Event::HealthVerified) => Phase::Running,
            (
                Phase::KnownCreated | Phase::Running | Phase::Retained,
                Event::CleanupRunningVerified,
            ) => Phase::Stopping,
            (
                Phase::KnownCreated | Phase::Running | Phase::Retained,
                Event::CleanupTerminalVerified,
            ) => Phase::Stopped,
            (Phase::ReconcilingRemove, Event::OperatorCleanupRunningVerified) => Phase::Stopping,
            (Phase::ReconcilingRemove, Event::OperatorCleanupTerminalVerified) => Phase::Stopped,
            (Phase::Retained, Event::OperatorAbsenceVerified) => Phase::VerifiedAbsent,
            (Phase::Stopping, Event::StoppedVerified) => Phase::Stopped,
            (Phase::Stopped, Event::BeginRemove) => Phase::Removing,
            (Phase::Removing, Event::RemoveClaimed) => Phase::AwaitingAbsence,
            (Phase::AwaitingAbsence | Phase::ReconcilingRemove, Event::AbsenceVerified) => {
                Phase::VerifiedAbsent
            }
            (
                Phase::Removing | Phase::AwaitingAbsence | Phase::ReconcilingRemove,
                Event::Failed,
            ) => Phase::ReconcilingRemove,
            (
                Phase::KnownCreated
                | Phase::Starting
                | Phase::Running
                | Phase::CheckingHealth
                | Phase::Retained
                | Phase::Stopping
                | Phase::Stopped,
                Event::Failed,
            ) => Phase::Retained,
            _ => return Err(Invalid("invalid lifecycle transition")),
        };
        next.validate()?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn created() -> State {
        State::pending_create()
            .transition(Event::Created("a".repeat(64)))
            .unwrap()
    }

    #[test]
    fn lost_create_response_blocks_reacquisition_and_replay() {
        let state = State::pending_create().recover().unwrap();
        assert_eq!(state.phase(), Phase::UnresolvedNoId);
        assert!(state.blocks_acquisition());
        assert!(state.transition(Event::Created("b".repeat(64))).is_err());
        assert!(state.transition(Event::BeginStart).is_err());
        assert!(state.transition(Event::AbsenceVerified).is_err());
    }

    #[test]
    fn cleanup_requires_stop_and_independent_absence() {
        let running = created()
            .transition(Event::BeginStart)
            .unwrap()
            .transition(Event::RunningVerified)
            .unwrap();
        assert!(running.transition(Event::BeginRemove).is_err());
        let removed = running
            .transition(Event::CleanupRunningVerified)
            .unwrap()
            .transition(Event::StoppedVerified)
            .unwrap()
            .transition(Event::BeginRemove)
            .unwrap()
            .transition(Event::RemoveClaimed)
            .unwrap();
        assert!(removed.blocks_acquisition());
        let absent = removed.transition(Event::AbsenceVerified).unwrap();
        assert!(!absent.blocks_acquisition());
        assert!(absent.transition(Event::BeginStart).is_err());
    }

    #[test]
    fn crash_with_known_id_retains_identity_and_cannot_restart() {
        for phase in [
            Phase::KnownCreated,
            Phase::Starting,
            Phase::Running,
            Phase::CheckingHealth,
            Phase::Stopping,
            Phase::Stopped,
        ] {
            let state = State {
                phase,
                object_id: Some("c".repeat(64)),
            };
            let recovered = state.recover().unwrap();
            assert_eq!(recovered.phase(), Phase::Retained);
            assert_eq!(recovered.object_id(), state.object_id());
            assert!(recovered.blocks_acquisition());
            assert!(recovered.transition(Event::BeginStart).is_err());
            assert!(recovered.transition(Event::AbsenceVerified).is_err());
        }
    }

    #[test]
    fn corrupt_state_and_partial_ids_never_unlock_or_transition() {
        for state in [
            State {
                phase: Phase::VerifiedAbsent,
                object_id: None,
            },
            State {
                phase: Phase::PendingCreate,
                object_id: Some("d".repeat(64)),
            },
            State {
                phase: Phase::Running,
                object_id: Some("short".into()),
            },
        ] {
            assert!(state.blocks_acquisition());
            assert!(state.recover().is_err());
            assert!(state.transition(Event::BeginRemove).is_err());
        }
        assert!(State::pending_create()
            .transition(Event::Created("abc".into()))
            .is_err());
    }

    #[test]
    fn failed_health_retains_id_without_cleanup_or_retry() {
        let state = created()
            .transition(Event::BeginStart)
            .unwrap()
            .transition(Event::RunningVerified)
            .unwrap()
            .transition(Event::BeginHealth)
            .unwrap()
            .transition(Event::Failed)
            .unwrap();
        assert_eq!(state.phase(), Phase::Retained);
        assert!(state.object_id().is_some());
        assert!(state.transition(Event::BeginHealth).is_err());
        assert!(state.transition(Event::BeginRemove).is_err());
    }
    #[test]
    fn every_post_remove_crash_window_reconciles_without_second_remove() {
        // Removing is durable before dispatch, so it also covers a crash before
        // sending and a lost HTTP response. AwaitingAbsence covers response and
        // inspection windows; neither recovery may replay the mutation.
        for phase in [
            Phase::Removing,
            Phase::AwaitingAbsence,
            Phase::ReconcilingRemove,
        ] {
            let state = State {
                phase,
                object_id: Some("d".repeat(64)),
            };
            let recovered = state.recover().unwrap();
            assert_eq!(recovered.phase(), Phase::ReconcilingRemove);
            assert_eq!(recovered.object_id(), state.object_id());
            assert!(recovered.blocks_acquisition());
            assert!(recovered.transition(Event::BeginRemove).is_err());
            assert!(recovered.transition(Event::RemoveClaimed).is_err());
            assert!(recovered.transition(Event::RunningVerified).is_err());
            assert!(recovered
                .transition(Event::CleanupTerminalVerified)
                .is_err());
            let failed = recovered.transition(Event::Failed).unwrap();
            assert_eq!(failed.phase(), Phase::ReconcilingRemove);
            let absent = failed.transition(Event::AbsenceVerified).unwrap();
            assert!(!absent.blocks_acquisition());
            assert_eq!(absent.recover().unwrap(), absent);
        }
    }
    #[test]
    fn present_reconciliation_requires_new_operator_authorization() {
        for phase in [Phase::Removing, Phase::AwaitingAbsence] {
            let state = State {
                phase,
                object_id: Some("e".repeat(64)),
            }
            .recover()
            .unwrap();
            assert!(state.transition(Event::BeginRemove).is_err());
            assert!(state.transition(Event::CleanupRunningVerified).is_err());
            let stopped = state
                .transition(Event::OperatorCleanupRunningVerified)
                .unwrap()
                .transition(Event::StoppedVerified)
                .unwrap();
            let absent = stopped
                .transition(Event::BeginRemove)
                .unwrap()
                .transition(Event::RemoveClaimed)
                .unwrap()
                .transition(Event::AbsenceVerified)
                .unwrap();
            assert!(!absent.blocks_acquisition());
            assert_eq!(absent.object_id(), state.object_id());
            let terminal = state
                .transition(Event::OperatorCleanupTerminalVerified)
                .unwrap();
            assert_eq!(terminal.phase(), Phase::Stopped);
        }
    }

    #[test]
    fn retained_absence_needs_operator_evidence_and_never_a_remove_claim() {
        let retained = created().recover().unwrap();
        assert!(retained.transition(Event::AbsenceVerified).is_err());
        assert!(retained.transition(Event::RemoveClaimed).is_err());
        let absent = retained.transition(Event::OperatorAbsenceVerified).unwrap();
        assert!(!absent.blocks_acquisition());
        assert!(State::pending_create()
            .transition(Event::OperatorAbsenceVerified)
            .is_err());
        assert!(State::pending_create()
            .recover()
            .unwrap()
            .transition(Event::OperatorAbsenceVerified)
            .is_err());
    }
}
