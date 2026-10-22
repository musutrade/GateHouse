use super::*;
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

#[test]
fn active_slot_fences_concurrent_and_failed_start_reacquisition() {
    let (d, r, p) = fixture();
    let mut slot =
        ServiceSlot::acquire(d.path(), r.clone(), p.clone(), vec!["check".into()]).unwrap();
    assert_eq!(slot.fence(), 1);
    assert!(ServiceSlot::acquire(d.path(), r.clone(), p.clone(), vec![]).is_err());
    let mut reservation = slot.reserve().unwrap();
    assert!(slot.reserve().is_err());
    reservation.dispatch().unwrap().observe(None).unwrap();
    drop(reservation);
    drop(slot);
    assert!(ServiceSlot::acquire(d.path(), r, p, vec![]).is_err());
    assert!(d.path().join("service.json").exists());
}
#[test]
fn invalid_health_is_rejected_before_fence_and_tamper_invalidates_held_slot() {
    let (d, r, p) = fixture();
    assert!(ServiceSlot::acquire(d.path(), r.clone(), p.clone(), vec!["".into()]).is_err());
    assert!(!d.path().join("service.json").exists());
    let mut slot = ServiceSlot::acquire(d.path(), r, p, vec!["check".into()]).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(d.path().join("service.json")).unwrap()).unwrap();
    value["healthcheck"] = serde_json::json!(["different"]);
    fs::write(
        d.path().join("service.json"),
        canonical::encode(&value).unwrap(),
    )
    .unwrap();
    assert!(slot.check().is_err());
    assert!(slot.reserve().is_err());
}
#[test]
fn forged_released_flag_never_unlocks_unresolved_journal() {
    let (d, r, p) = fixture();
    let slot = ServiceSlot::acquire(d.path(), r.clone(), p.clone(), vec![]).unwrap();
    drop(slot);
    let file = d.path().join("service.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    value["released"] = true.into();
    fs::write(file, canonical::encode(&value).unwrap()).unwrap();
    assert!(ServiceSlot::acquire(d.path(), r, p, vec![]).is_err());
}
#[test]
fn prepared_slot_drop_is_durable_and_root_replacement_is_detected() {
    let (d, r, p) = fixture();
    let root = d.path().join("slot");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let slot = ServiceSlot::acquire(&root, r.clone(), p.clone(), vec![]).unwrap();
    fs::rename(&root, d.path().join("old")).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(slot.check().is_err());
    drop(slot);
    assert!(ServiceSlot::acquire(&d.path().join("old"), r, p, vec![]).is_err());
}

#[test]
fn claim_write_boundaries_never_allow_dispatch_from_failed_acquisition() {
    for fail in 0..=3 {
        let (d, r, p) = fixture();
        assert!(
            ServiceSlot::acquire_with(d.path(), r.clone(), p.clone(), vec![], |stage| {
                if stage == fail {
                    Err(Invalid("injected slot claim failure"))
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        let published = d.path().join("service.json").exists();
        assert_eq!(published, fail >= 2);
        let next = ServiceSlot::acquire(d.path(), r, p, vec![]);
        if published {
            assert!(next.is_err());
        } else {
            assert!(next.is_ok());
        }
    }
}

#[test]
fn standalone_or_legacy_lease_is_never_implicitly_adopted() {
    for member in ["active", "lease.json", "reservation.json", "tombstone.json"] {
        let (d, r, p) = fixture();
        let file = d.path().join(member);
        fs::write(&file, b"legacy or standalone evidence").unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(ServiceSlot::acquire(d.path(), r, p, vec![]).is_err());
        assert!(!d.path().join("service.json").exists());
    }
}
