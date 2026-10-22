use super::*;
use crate::lifecycle::{
    canonical,
    wire::{
        Absence, ContainerState, Entry, Object, Observation, Observations, Outcome, Port, Response,
    },
};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
fn dir() -> tempfile::TempDir {
    let d = tempfile::Builder::new()
        .prefix("journal-test-")
        .tempdir_in(
            std::env::var_os("GATEHOUSE_JOURNAL_TEST_ROOT")
                .or_else(|| std::env::var_os("HOME"))
                .expect("protected test parent required"),
        )
        .unwrap();
    fs::set_permissions(d.path(), fs::Permissions::from_mode(0o700)).unwrap();
    d
}
fn create() -> Request {
    let mut r = wire::parse_request(include_bytes!("../../wire/status-request.json")).unwrap();
    let Operation::Status { target } = r.operation else {
        panic!()
    };
    r.operation = Operation::Create {
        name: target.name,
        image_id: format!("sha256:{}", "e".repeat(64)),
        labels: target.labels,
        environment: Vec::<Entry>::new(),
        container_port: target.container_port,
    };
    r.binding.operation_key = r.operation_key().unwrap();
    r
}
fn req(j: &Journal, op: Operation) -> Request {
    let mut r = j.record.create.clone();
    r.operation = op;
    r.binding.generation = j.revision() + 1;
    r.binding.invocation = format!("{:032x}", r.binding.generation + 100);
    r.binding.operation_key = r.operation_key().unwrap();
    r
}
pub(super) fn reply(r: &Request, outcome: Outcome) -> Vec<u8> {
    let e = &r.binding.engine_binding;
    let o = Observation {
        api_version: e.api_version.clone(),
        server_version: e.server_version.clone(),
        daemon_id: e.daemon_id.clone(),
    };
    let response = Response {
        protocol: r.protocol.clone(),
        binding: r.binding.clone(),
        operation: r.operation.clone(),
        outcome,
        observations: Some(Observations {
            observed_before: o.clone(),
            observed_after: o,
        }),
    };
    canonical::encode(&serde_json::to_value(response).unwrap()).unwrap()
}
pub(super) fn object(r: &Request, state: ContainerState) -> Object {
    let Operation::Create {
        name,
        image_id,
        labels,
        container_port,
        ..
    } = &r.operation
    else {
        panic!()
    };
    Object {
        object_id: "a".repeat(64),
        name: name.clone(),
        image_id: image_id.clone(),
        labels: labels.clone(),
        state,
        ports: if state == ContainerState::Running {
            vec![Port {
                container_port: *container_port,
                protocol: "tcp".into(),
                host_ip: "127.0.0.1".into(),
                host_port: 49152,
            }]
        } else {
            vec![]
        },
    }
}
fn created(d: &tempfile::TempDir) -> Journal {
    let r = create();
    let mut j = Journal::create(d.path(), &r).unwrap();
    let b = reply(
        &r,
        Outcome::Created {
            object: object(&r, ContainerState::Created),
        },
    );
    j.dispatch().unwrap().observe(Some(&b)).unwrap();
    j
}
fn status(j: &mut Journal, state: ContainerState, purpose: Purpose) {
    let r = req(
        j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    let b = reply(
        &r,
        Outcome::Status {
            object: object(&j.record.create, state),
        },
    );
    j.begin(&r, purpose).unwrap().observe(Some(&b)).unwrap();
}
pub(super) fn absent(r: &Request) -> Vec<u8> {
    let Operation::Status { target } = &r.operation else {
        panic!()
    };
    reply(
        r,
        Outcome::Absent {
            evidence: Absence {
                object_id: target.object_id.clone(),
                route: format!(
                    "/v{}/containers/{}/json",
                    r.binding.engine_binding.api_version, target.object_id
                ),
                http_status: 404,
                error_message_sha256: format!(
                    "{:x}",
                    Sha256::digest(format!("No such container: {}", target.object_id).as_bytes())
                ),
            },
        },
    )
}
#[test]
fn dropped_create_token_cannot_replay_and_reopen_blocks_second_create() {
    let d = dir();
    let r = create();
    let mut j = Journal::create(d.path(), &r).unwrap();
    drop(j.dispatch().unwrap());
    assert!(j.dispatch().is_err());
    drop(j);
    let mut j = Journal::open(d.path(), &r).unwrap();
    assert_eq!(j.phase(), Phase::UnresolvedNoId);
    assert!(j.dispatch().is_err());
    assert!(j.target().is_err());
    drop(j);
    assert!(Journal::create(d.path(), &r).is_err());
}
#[test]
fn generation_target_authority_and_consumed_response_fail_closed() {
    let d = dir();
    let mut j = created(&d);
    let op = Operation::Status {
        target: j.target().unwrap(),
    };
    let r = req(&j, op);
    let mut wrong = r.clone();
    wrong.binding.engine_binding.daemon_id = "other".into();
    wrong.binding.operation_key = wrong.operation_key().unwrap();
    assert!(j.begin(&wrong, Purpose::Service).is_err());
    wrong = r.clone();
    if let Operation::Status { target } = &mut wrong.operation {
        target.object_id = "f".repeat(64);
    }
    wrong.binding.operation_key = wrong.operation_key().unwrap();
    assert!(j.begin(&wrong, Purpose::Service).is_err());
    let b = reply(
        &r,
        Outcome::Status {
            object: object(&j.record.create, ContainerState::Created),
        },
    );
    j.begin(&r, Purpose::Service)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
    assert!(j.begin(&r, Purpose::Service).is_err());
    let next = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    assert!(j
        .begin(&next, Purpose::Service)
        .unwrap()
        .observe(Some(&b))
        .is_err());
    assert!(j.dispatch().is_err());
    drop(j);
    assert_eq!(
        Journal::open(d.path(), &create()).unwrap().phase(),
        Phase::Retained
    );
}
#[test]
fn start_requires_fresh_exact_status_and_durable_intent() {
    let d = dir();
    let mut j = created(&d);
    let r = req(
        &j,
        Operation::Start {
            target: j.target().unwrap(),
        },
    );
    assert!(j.begin(&r, Purpose::Service).is_err());
    status(&mut j, ContainerState::Created, Purpose::Service);
    let r = req(
        &j,
        Operation::Start {
            target: j.target().unwrap(),
        },
    );
    let b = reply(
        &r,
        Outcome::Started {
            object: object(&j.record.create, ContainerState::Running),
        },
    );
    let token = j.begin(&r, Purpose::Service).unwrap();
    let on_disk: Record =
        serde_json::from_slice(&fs::read(d.path().join("lease.json")).unwrap()).unwrap();
    assert_eq!(on_disk.state.phase(), Phase::Starting);
    assert_eq!(token.request(), &r);
    token.observe(Some(&b)).unwrap();
    assert_eq!(j.phase(), Phase::Running);
}
#[test]
fn remove_crash_recovers_absence_without_replay_and_retains_tombstone() {
    let d = dir();
    let mut j = created(&d);
    status(&mut j, ContainerState::Created, Purpose::OperatorCleanup);
    let r = req(
        &j,
        Operation::Remove {
            target: j.target().unwrap(),
        },
    );
    drop(j.begin(&r, Purpose::OperatorCleanup).unwrap());
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    assert_eq!(j.phase(), Phase::ReconcilingRemove);
    assert!(j.dispatch().is_err());
    let r = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    let b = absent(&r);
    j.begin(&r, Purpose::Reconcile)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
    assert_eq!(j.phase(), Phase::VerifiedAbsent);
    assert!(d.path().join("active").exists());
    j.finish_release().unwrap();
    assert!(!d.path().join("active").exists());
    assert_eq!(
        fs::read(d.path().join("lease.json")).unwrap(),
        fs::read(d.path().join("tombstone.json")).unwrap()
    );
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    j.finish_release().unwrap();
    assert!(j.dispatch().is_err());
}
#[test]
fn recovered_present_requires_new_operator_generation_not_old_remove() {
    let d = dir();
    let mut j = created(&d);
    status(&mut j, ContainerState::Created, Purpose::OperatorCleanup);
    let remove = req(
        &j,
        Operation::Remove {
            target: j.target().unwrap(),
        },
    );
    drop(j.begin(&remove, Purpose::OperatorCleanup).unwrap());
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    assert!(j.begin(&remove, Purpose::OperatorCleanup).is_err());
    status(&mut j, ContainerState::Running, Purpose::Reconcile);
    let stop = req(
        &j,
        Operation::Stop {
            target: j.target().unwrap(),
            grace_seconds: 10,
        },
    );
    assert!(j.begin(&stop, Purpose::Service).is_err());
    let b = reply(
        &stop,
        Outcome::Stopped {
            object: object(&j.record.create, ContainerState::Exited),
        },
    );
    j.begin(&stop, Purpose::OperatorCleanup)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
    assert_eq!(j.phase(), Phase::Stopped);
    assert!(j.record.attempts.iter().any(|a| a.request == remove));
}
#[test]
fn retained_absence_requires_explicit_operator_and_no_remove_claim() {
    let d = dir();
    let j = created(&d);
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    let r = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    let b = absent(&r);
    assert!(j.begin(&r, Purpose::Service).is_err());
    j.begin(&r, Purpose::OperatorCleanup)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
    assert_eq!(j.phase(), Phase::VerifiedAbsent);
    assert!(!j
        .record
        .attempts
        .iter()
        .any(|a| matches!(a.request.operation, Operation::Remove { .. })));
    j.finish_release().unwrap();
}
#[test]
fn missing_marker_wrong_tombstone_symlink_and_lock_fail_closed() {
    let d = dir();
    let j = created(&d);
    assert!(Journal::open(d.path(), &create()).is_err());
    drop(j);
    fs::remove_file(d.path().join("active")).unwrap();
    assert!(Journal::open(d.path(), &create()).is_err());
    let d = dir();
    std::os::unix::fs::symlink("missing", d.path().join("lease.json")).unwrap();
    assert!(Journal::create(d.path(), &create()).is_err());
    let d = dir();
    let j = created(&d);
    drop(j);
    let path = d.path().join("tombstone.json");
    fs::write(&path, b"{}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Journal::open(d.path(), &create()).is_err());
}
#[test]
fn atomic_failure_windows_never_remove_active_or_issue_dispatch() {
    for fail in 0..3 {
        let d = dir();
        let mut j = created(&d);
        status(&mut j, ContainerState::Created, Purpose::Service);
        let r = req(
            &j,
            Operation::Start {
                target: j.target().unwrap(),
            },
        );
        let next = j.record.begin(&r, Purpose::Service, true).unwrap();
        let outcome = j.commit_with(next, |stage| {
            if stage == fail {
                Err(Invalid("injected commit failure"))
            } else {
                Ok(())
            }
        });
        assert!(j.dispatch().is_err());
        assert!(outcome.is_err());
        assert!(d.path().join("active").exists());
        drop(j);
        let mut j = Journal::open(d.path(), &create()).unwrap();
        assert!(j.dispatch().is_err());
        assert!(j.finish_release().is_err());
    }
}
#[test]
fn absence_commit_then_tombstone_then_marker_recovery() {
    let d = dir();
    let j = created(&d);
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    let r = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    j.begin(&r, Purpose::OperatorCleanup)
        .unwrap()
        .observe(Some(&absent(&r)))
        .unwrap();
    // Crash after absence record, before tombstone.
    drop(j);
    let j = Journal::open(d.path(), &create()).unwrap();
    Journal::atomic(&j.directory, "tombstone.json", &j.bytes).unwrap();
    // Crash after tombstone, before marker removal.
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    assert!(d.path().join("active").exists());
    j.finish_release().unwrap();
    drop(j);
    assert_eq!(
        Journal::open(d.path(), &create()).unwrap().phase(),
        Phase::VerifiedAbsent
    );
}

#[test]
fn restore_rejects_changed_target_history_and_marker_removal_before_dispatch() {
    let d = dir();
    let mut j = created(&d);
    status(&mut j, ContainerState::Created, Purpose::Service);
    let mut changed = j.record.clone();
    if let Operation::Status { target } = &mut changed.attempts[1].request.operation {
        target.container_port = 9999;
    }
    changed.attempts[1].request.binding.operation_key =
        changed.attempts[1].request.operation_key().unwrap();
    assert!(changed.validate().is_err());
    fs::remove_file(d.path().join("active")).unwrap();
    let r = req(
        &j,
        Operation::Start {
            target: j.target().unwrap(),
        },
    );
    assert!(j.begin(&r, Purpose::Service).is_err());
}

fn running(j: &mut Journal) {
    status(j, ContainerState::Created, Purpose::Service);
    let r = req(
        j,
        Operation::Start {
            target: j.target().unwrap(),
        },
    );
    let b = reply(
        &r,
        Outcome::Started {
            object: object(&j.record.create, ContainerState::Running),
        },
    );
    j.begin(&r, Purpose::Service)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
}
#[test]
fn normal_service_teardown_does_not_impersonate_operator() {
    let d = dir();
    let mut j = created(&d);
    running(&mut j);
    status(&mut j, ContainerState::Running, Purpose::ServiceCleanup);
    let stop = req(
        &j,
        Operation::Stop {
            target: j.target().unwrap(),
            grace_seconds: 10,
        },
    );
    let b = reply(
        &stop,
        Outcome::Stopped {
            object: object(&j.record.create, ContainerState::Exited),
        },
    );
    j.begin(&stop, Purpose::ServiceCleanup)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
    status(&mut j, ContainerState::Exited, Purpose::ServiceCleanup);
    let remove = req(
        &j,
        Operation::Remove {
            target: j.target().unwrap(),
        },
    );
    let b = reply(&remove, Outcome::Removed);
    j.begin(&remove, Purpose::ServiceCleanup)
        .unwrap()
        .observe(Some(&b))
        .unwrap();
    let inspect = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    j.begin(&inspect, Purpose::Reconcile)
        .unwrap()
        .observe(Some(&absent(&inspect)))
        .unwrap();
    let proof = j.record.absence.as_ref().unwrap();
    assert_eq!(proof.reason, super::super::AbsenceReason::PostRemoveAbsence);
    assert_eq!(
        proof.remove_request_sha256.as_deref(),
        Some(remove.binding.operation_key.as_str())
    );
    assert_eq!(proof.status_request_sha256, inspect.binding.operation_key);
    assert!(!j
        .record
        .attempts
        .iter()
        .any(|a| a.purpose == Purpose::OperatorCleanup));
    j.finish_release().unwrap();
}
#[test]
fn live_present_after_remove_claim_or_lost_response_needs_new_operator_intent() {
    for lost in [false, true] {
        let d = dir();
        let mut j = created(&d);
        status(&mut j, ContainerState::Created, Purpose::OperatorCleanup);
        let remove = req(
            &j,
            Operation::Remove {
                target: j.target().unwrap(),
            },
        );
        let b = reply(&remove, Outcome::Removed);
        let token = j.begin(&remove, Purpose::OperatorCleanup).unwrap();
        if lost {
            drop(token);
            j.retain_unknown().unwrap();
        } else {
            token.observe(Some(&b)).unwrap();
        }
        status(&mut j, ContainerState::Running, Purpose::Reconcile);
        assert_eq!(j.phase(), Phase::ReconcilingRemove);
        let stop = req(
            &j,
            Operation::Stop {
                target: j.target().unwrap(),
                grace_seconds: 10,
            },
        );
        assert!(j.begin(&stop, Purpose::ServiceCleanup).is_err());
        let b = reply(
            &stop,
            Outcome::Stopped {
                object: object(&j.record.create, ContainerState::Exited),
            },
        );
        j.begin(&stop, Purpose::OperatorCleanup)
            .unwrap()
            .observe(Some(&b))
            .unwrap();
        assert!(j.begin(&remove, Purpose::OperatorCleanup).is_err());
    }
}
#[test]
fn cancelled_service_cleanup_retains_and_requires_operator_recovery() {
    let d = dir();
    let mut j = created(&d);
    running(&mut j);
    status(&mut j, ContainerState::Running, Purpose::ServiceCleanup);
    let r = req(
        &j,
        Operation::Stop {
            target: j.target().unwrap(),
            grace_seconds: 10,
        },
    );
    j.begin(&r, Purpose::ServiceCleanup)
        .unwrap()
        .observe(None)
        .unwrap();
    assert_eq!(j.phase(), Phase::Retained);
    let inspect = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    assert!(j.begin(&inspect, Purpose::ServiceCleanup).is_err());
    status(&mut j, ContainerState::Running, Purpose::OperatorCleanup);
}
#[test]
fn restore_replays_history_rejecting_impossible_phase_and_purpose() {
    let d = dir();
    let j = created(&d);
    let original = j.bytes.clone();
    drop(j);
    for phase in ["running", "starting", "awaiting-absence", "verified-absent"] {
        let mut v: serde_json::Value = serde_json::from_slice(&original).unwrap();
        v["state"]["phase"] = phase.into();
        fs::write(d.path().join("lease.json"), canonical::encode(&v).unwrap()).unwrap();
        assert!(Journal::open(d.path(), &create()).is_err());
    }
    fs::write(d.path().join("lease.json"), &original).unwrap();
    let j = Journal::open(d.path(), &create()).unwrap();
    let mut fake = j.record.clone();
    fake.attempts[0].purpose = Purpose::OperatorCleanup;
    assert!(fake.validate().is_err());
    // A validly bound start response is insufficient without a preceding status.
    let start = req(
        &j,
        Operation::Start {
            target: j.target().unwrap(),
        },
    );
    let bytes = reply(
        &start,
        Outcome::Started {
            object: object(&j.record.create, ContainerState::Running),
        },
    );
    let mut fake = j.record.clone();
    fake.recoveries.clear();
    fake.state =
        serde_json::from_value(serde_json::json!({"phase":"running","object_id":"a".repeat(64)}))
            .unwrap();
    fake.revision = start.binding.generation;
    fake.attempts.push(super::super::Attempt {
        request: start.clone(),
        purpose: Purpose::Service,
        response: Some(wire::parse_response(&bytes, &start).unwrap()),
    });
    assert!(fake.validate().is_err());
}
#[test]
fn operator_absence_reason_and_no_remove_digest_are_checked() {
    let d = dir();
    let j = created(&d);
    drop(j);
    let mut j = Journal::open(d.path(), &create()).unwrap();
    let r = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    j.begin(&r, Purpose::OperatorCleanup)
        .unwrap()
        .observe(Some(&absent(&r)))
        .unwrap();
    let proof = j.record.absence.as_ref().unwrap();
    assert_eq!(
        proof.reason,
        super::super::AbsenceReason::OperatorObservedAbsence
    );
    assert_eq!(proof.remove_request_sha256, None);
    let mut forged = j.record.clone();
    forged.absence.as_mut().unwrap().remove_request_sha256 = Some("a".repeat(64));
    assert!(forged.validate().is_err());
    forged = j.record.clone();
    forged.absence.as_mut().unwrap().status_request_sha256 = "f".repeat(64);
    assert!(forged.validate().is_err());
}
#[test]
fn marker_only_creation_crash_blocks_reacquisition() {
    let d = dir();
    let active = d.path().join("active");
    fs::write(&active, create().binding.operation_key).unwrap();
    fs::set_permissions(active, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Journal::create(d.path(), &create()).is_err());
    assert!(Journal::open(d.path(), &create()).is_err());
}
#[test]
fn attempt_cap_never_discards_history() {
    let d = dir();
    let j = created(&d);
    let mut r = j.record.clone();
    drop(j);
    for generation in 2..=128 {
        let mut request = r.create.clone();
        request.operation = Operation::Status {
            target: r.target().unwrap(),
        };
        request.binding.generation = generation;
        request.binding.invocation = format!("{generation:032x}");
        request.binding.operation_key = request.operation_key().unwrap();
        let b = reply(
            &request,
            Outcome::Status {
                object: object(&r.create, ContainerState::Created),
            },
        );
        r = r
            .begin(&request, Purpose::Service, false)
            .unwrap()
            .observe(Some(&b))
            .unwrap();
    }
    assert_eq!(r.attempts.len(), 128);
    let mut request = r.create.clone();
    request.operation = Operation::Status {
        target: r.target().unwrap(),
    };
    request.binding.generation = 129;
    request.binding.invocation = format!("{:032x}", 129);
    request.binding.operation_key = request.operation_key().unwrap();
    assert!(r.begin(&request, Purpose::Service, false).is_err());
    assert_eq!(r.attempts.len(), 128);
}

#[test]
fn release_unlink_failure_preserves_tombstone_and_poisoned_journal() {
    let d = dir();
    drop(created(&d));
    let mut j = Journal::open(d.path(), &create()).unwrap();
    let request = req(
        &j,
        Operation::Status {
            target: j.target().unwrap(),
        },
    );
    j.begin(&request, Purpose::OperatorCleanup)
        .unwrap()
        .observe(Some(&absent(&request)))
        .unwrap();
    let committed = fs::read(d.path().join("lease.json")).unwrap();
    fs::remove_file(d.path().join("active")).unwrap();
    fs::create_dir(d.path().join("active")).unwrap();
    let error = j.finish_release().unwrap_err();
    assert!(error.to_string().contains("cannot remove lifecycle marker"));
    assert!(d.path().join("active").is_dir());
    assert_eq!(
        fs::read(d.path().join("tombstone.json")).unwrap(),
        committed
    );
    assert_eq!(fs::read(d.path().join("lease.json")).unwrap(), committed);
    assert!(j.finish_release().is_err());
    assert!(j.dispatch().is_err());
}
