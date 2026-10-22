use super::*;
use crate::lifecycle::{
    state::Phase as LeasePhase,
    wire::{Observation, Observations},
};
use std::os::unix::fs::PermissionsExt;
fn fixture() -> (tempfile::TempDir, Request, CreatePlan) {
    let root = std::env::var_os("GATEHOUSE_JOURNAL_TEST_ROOT")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap();
    let dir = tempfile::Builder::new()
        .prefix("reservation-test-")
        .tempdir_in(root)
        .unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let mut request =
        wire::parse_request(include_bytes!("../../../wire/status-request.json")).unwrap();
    let Operation::Status { target } = request.operation else {
        panic!()
    };
    let plan = CreatePlan {
        name: target.name,
        labels: target.labels,
        environment: vec![],
        container_port: target.container_port,
    };
    request.operation = Operation::ResolveImage {
        image: "example:latest".into(),
    };
    request.binding.operation_key = request.operation_key().unwrap();
    (dir, request, plan)
}
fn response(request: &Request, outcome: Outcome) -> Vec<u8> {
    let e = &request.binding.engine_binding;
    let observation = Observation {
        api_version: e.api_version.clone(),
        server_version: e.server_version.clone(),
        daemon_id: e.daemon_id.clone(),
    };
    let r = Response {
        protocol: request.protocol.clone(),
        binding: request.binding.clone(),
        operation: request.operation.clone(),
        outcome,
        observations: Some(Observations {
            observed_before: observation.clone(),
            observed_after: observation,
        }),
    };
    canonical::encode(&serde_json::to_value(r).unwrap()).unwrap()
}
fn image(request: &Request) -> Vec<u8> {
    response(
        request,
        Outcome::Image {
            image_id: format!("sha256:{}", "e".repeat(64)),
            repo_digests: vec![],
        },
    )
}
#[test]
fn resolved_image_transfers_exact_plan_to_create_generation_two() {
    let (d, r, p) = fixture();
    let mut reservation = Reservation::create(d.path(), &r, p.clone()).unwrap();
    let token = reservation.dispatch().unwrap();
    assert_eq!(token.request(), &r);
    token.observe(Some(&image(&r))).unwrap();
    assert!(reservation.dispatch().is_err());
    let mut journal = reservation.into_journal(&"3".repeat(32)).unwrap();
    let token = journal.dispatch().unwrap();
    let create = token.request().clone();
    assert_eq!(create.binding.generation, 2);
    let Operation::Create {
        image_id,
        name,
        environment,
        container_port,
        ..
    } = &create.operation
    else {
        panic!()
    };
    assert_eq!(image_id, &format!("sha256:{}", "e".repeat(64)));
    assert_eq!(name, &p.name);
    assert_eq!(environment, &p.environment);
    assert_eq!(*container_port, p.container_port);
    token.observe(None).unwrap();
    assert_eq!(journal.phase(), LeasePhase::UnresolvedNoId);
    assert!(journal.dispatch().is_err());
    drop(journal);
    assert!(Reservation::open(d.path(), &r, &p).is_err());
    let mut recovered = Journal::open_reserved(d.path(), &r, &p).unwrap();
    assert!(recovered.dispatch().is_err());
    assert_eq!(recovered.phase(), LeasePhase::UnresolvedNoId);
}
#[test]
fn pending_or_resolved_reopen_never_automatically_dispatches() {
    for resolved in [false, true] {
        let (d, r, p) = fixture();
        let mut value = Reservation::create(d.path(), &r, p.clone()).unwrap();
        if resolved {
            value.dispatch().unwrap().observe(Some(&image(&r))).unwrap();
        } else {
            drop(value.dispatch().unwrap());
        }
        drop(value);
        let mut reopened = Reservation::open(d.path(), &r, &p).unwrap();
        assert!(reopened.dispatch().is_err());
        if resolved {
            assert!(!reopened.ready_create);
            reopened.authorize_operator_resume_create().unwrap();
            let mut journal = reopened.into_journal(&"3".repeat(32)).unwrap();
            assert_eq!(journal.dispatch().unwrap().request().binding.generation, 2);
        } else {
            assert!(reopened.authorize_operator_resume_create().is_err());
            reopened.abandon().unwrap();
            assert!(!d.path().join("active").exists());
            assert!(d.path().join("reservation-tombstone.json").exists());
        }
    }
}
#[test]
fn unknown_wrong_identity_and_reused_nonce_cannot_authorize_create() {
    let (d, r, p) = fixture();
    let mut value = Reservation::create(d.path(), &r, p).unwrap();
    value.dispatch().unwrap().observe(None).unwrap();
    assert!(value.into_journal(&"3".repeat(32)).is_err());
    let (d, r, p) = fixture();
    let mut value = Reservation::create(d.path(), &r, p).unwrap();
    let mut wrong = r.clone();
    wrong.binding.invocation = "4".repeat(32).into();
    wrong.binding.operation_key = wrong.operation_key().unwrap();
    assert!(value
        .dispatch()
        .unwrap()
        .observe(Some(&image(&wrong)))
        .is_err());
    assert!(value.dispatch().is_err());
    let (d, r, p) = fixture();
    let mut value = Reservation::create(d.path(), &r, p).unwrap();
    value.dispatch().unwrap().observe(Some(&image(&r))).unwrap();
    assert!(value.into_journal(&r.binding.invocation).is_err());
}
#[test]
fn plan_change_marker_only_and_duplicate_reservation_are_rejected() {
    let (d, r, p) = fixture();
    let value = Reservation::create(d.path(), &r, p.clone()).unwrap();
    assert!(Reservation::create(d.path(), &r, p.clone()).is_err());
    drop(value);
    let mut changed = p.clone();
    changed.container_port = 80;
    assert!(Reservation::open(d.path(), &r, &changed).is_err());
    fs::remove_file(d.path().join("reservation.json")).unwrap();
    assert!(Reservation::open(d.path(), &r, &p).is_err());
    assert!(Reservation::create(d.path(), &r, p).is_err());
}
#[test]
fn transfer_sync_failures_preserve_reservation_or_unresolved_lease_without_replay() {
    for fail in 0..3 {
        let (d, r, p) = fixture();
        let mut value = Reservation::create(d.path(), &r, p.clone()).unwrap();
        value.dispatch().unwrap().observe(Some(&image(&r))).unwrap();
        assert!(value
            .transfer_with(&"3".repeat(32), |stage| if stage == fail {
                Err(Invalid("injected sync failure"))
            } else {
                Ok(())
            })
            .is_err());
        if d.path().join("lease.json").exists() {
            let mut recovered = Journal::open_reserved(d.path(), &r, &p).unwrap();
            assert_eq!(recovered.phase(), LeasePhase::UnresolvedNoId);
            assert!(recovered.dispatch().is_err());
        } else {
            let mut reopened = Reservation::open(d.path(), &r, &p).unwrap();
            assert!(reopened.dispatch().is_err());
            assert!(!reopened.ready_create);
        }
        assert!(d.path().join("active").exists());
    }
}
#[test]
fn forged_resolution_history_and_abandonment_tombstone_fail_closed() {
    let (d, r, p) = fixture();
    let mut value = Reservation::create(d.path(), &r, p.clone()).unwrap();
    value.dispatch().unwrap().observe(Some(&image(&r))).unwrap();
    let journal = value.into_journal(&"3".repeat(32)).unwrap();
    drop(journal);
    let path = d.path().join("lease.json");
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["resolution"]["plan"]["container_port"] = 80.into();
    fs::write(path, canonical::encode(&v).unwrap()).unwrap();
    assert!(Journal::open_reserved(d.path(), &r, &p).is_err());
    let (d, r, p) = fixture();
    let mut value = Reservation::create(d.path(), &r, p.clone()).unwrap();
    value.retain_unknown().unwrap();
    value.abandon().unwrap();
    drop(value);
    let mut reopened = Reservation::open(d.path(), &r, &p).unwrap();
    reopened.abandon().unwrap();
    assert!(reopened.authorize_operator_resume_create().is_err());
    assert!(reopened.dispatch().is_err());
    drop(reopened);
    fs::write(d.path().join("reservation-tombstone.json"), b"{}").unwrap();
    assert!(Reservation::open(d.path(), &r, &p).is_err());
}
