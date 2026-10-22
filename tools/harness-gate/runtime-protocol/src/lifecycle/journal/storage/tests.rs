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
fn reply(r: &Request, outcome: Outcome) -> Vec<u8> {
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
fn object(r: &Request, state: ContainerState) -> Object {
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
fn absent(r: &Request) -> Vec<u8> {
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
