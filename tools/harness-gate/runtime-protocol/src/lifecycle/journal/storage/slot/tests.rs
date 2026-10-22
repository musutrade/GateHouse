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

#[test]
fn mapping_is_stable_across_config_changes_and_separates_resources() {
    let (d, mut r, p) = fixture();
    let root =
        ServiceSlot::mapped_root(d.path(), &r.binding.project_id, &r.binding.resource_id).unwrap();
    let slot = ServiceSlot::acquire_mapped(d.path(), r.clone(), p.clone(), vec![]).unwrap();
    assert_eq!(slot.root, root);
    drop(slot);
    r.operation = Operation::ResolveImage {
        image: "different:tag".into(),
    };
    r.binding.operation_key = r.operation_key().unwrap();
    assert!(ServiceSlot::acquire_mapped(d.path(), r.clone(), p, vec![]).is_err());
    assert_ne!(
        root,
        ServiceSlot::mapped_root(d.path(), &r.binding.project_id, "another").unwrap()
    );
    let escaped = ServiceSlot::mapped_root(d.path(), "../outside", "../../resource").unwrap();
    assert_eq!(escaped.parent(), Some(d.path()));
}
#[test]
fn mapping_rejects_symlinks_and_unprotected_existing_directories() {
    for symlink in [true, false] {
        let (d, r, p) = fixture();
        let root =
            ServiceSlot::mapped_root(d.path(), &r.binding.project_id, &r.binding.resource_id)
                .unwrap();
        if symlink {
            std::os::unix::fs::symlink(d.path(), &root).unwrap();
        } else {
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(ServiceSlot::acquire_mapped(d.path(), r, p, vec![]).is_err());
    }
}

#[test]
fn ending_directory_guard_revokes_incidental_fork_inheritance() {
    let (d, _, _) = fixture();
    let held = Journal::directory(d.path()).unwrap();
    let mut fds = [-1; 2];
    // SAFETY: writable two-int array; child below uses only async-signal-safe calls.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0);
    if pid == 0 {
        unsafe {
            libc::close(fds[1]);
            let mut byte = 0u8;
            libc::read(fds[0], (&mut byte as *mut u8).cast(), 1);
            libc::_exit(0);
        }
    }
    unsafe {
        libc::close(fds[0]);
    }
    drop(held);
    // The child is still blocked with an inherited descriptor. The owner's guard
    // lifetime has ended, so that incidental descriptor must not retain its lock.
    let acquired = Journal::directory(d.path());
    unsafe {
        let byte = 1u8;
        libc::write(fds[1], (&byte as *const u8).cast(), 1);
        libc::close(fds[1]);
        libc::waitpid(pid, std::ptr::null_mut(), 0);
    }
    assert!(
        acquired.is_ok(),
        "fork child prolonged the owner's ended guard"
    );
}

#[test]
fn nonowner_guard_drop_cannot_unlock_live_owner() {
    let (d, _, _) = fixture();
    let held = Journal::directory(d.path()).unwrap();
    let inherited = LockedDirectory {
        file: held.file.try_clone().unwrap(),
        owner_pid: -1,
    };
    drop(inherited);
    assert!(Journal::directory(d.path()).is_err());
    drop(held);
    assert!(Journal::directory(d.path()).is_ok());
}

#[test]
fn owner_death_retains_lock_until_surviving_child_closes_it() {
    use std::io::{BufRead, Write};
    use std::process::{Command, Stdio};
    if let Some(root) = std::env::var_os("GATEHOUSE_LOCK_DEATH_FIXTURE") {
        let _guard = Journal::directory(Path::new(&root)).unwrap();
        // SAFETY: after fork the child uses only async-signal-safe libc calls.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            unsafe {
                libc::write(1, b"LOCK_CHILD_READY\n".as_ptr().cast(), 17);
                let mut byte = 0u8;
                libc::read(0, (&mut byte as *mut u8).cast(), 1);
                libc::_exit(0);
            }
        }
        // Deliberately bypass Drop, modelling abrupt owner death.
        unsafe {
            libc::_exit(0);
        }
    }
    let (d, _, _) = fixture();
    let mut owner = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lifecycle::journal::storage::slot::tests::owner_death_retains_lock_until_surviving_child_closes_it", "--nocapture"])
        .env("GATEHOUSE_LOCK_DEATH_FIXTURE", d.path())
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut output = std::io::BufReader::new(owner.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(
            output.read_line(&mut line).unwrap() != 0,
            "child never became ready"
        );
        if line.contains("LOCK_CHILD_READY") {
            break;
        }
    }
    let mut control = owner.stdin.take().unwrap();
    assert!(owner.wait().unwrap().success());
    let blocked = Journal::directory(d.path()).is_err();
    control.write_all(b"x").unwrap();
    let mut remaining = vec![];
    output.read_to_end(&mut remaining).unwrap();
    assert!(
        blocked,
        "owner death must not authorize breaking the child's lock"
    );
    assert!(Journal::directory(d.path()).is_ok());
}

#[test]
fn project_state_base_is_fixed_private_and_rejects_unsafe_existing_paths() {
    let (d, _, _) = fixture();
    let base = ServiceSlot::project_state_base(d.path()).unwrap();
    assert_eq!(base, d.path().join(".harness-gate/lifecycle-services"));
    assert_eq!(fs::metadata(&base).unwrap().mode() & 0o777, 0o700);
    assert_eq!(ServiceSlot::project_state_base(d.path()).unwrap(), base);
    fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(ServiceSlot::project_state_base(d.path()).is_err());
    assert_eq!(fs::metadata(&base).unwrap().mode() & 0o777, 0o755);
    let (other, _, _) = fixture();
    std::os::unix::fs::symlink(d.path().join(".harness-gate"), other.path().join(".harness-gate")).unwrap();
    assert!(ServiceSlot::project_state_base(other.path()).is_err());
    let (writable, _, _) = fixture();
    fs::set_permissions(writable.path(), fs::Permissions::from_mode(0o770)).unwrap();
    assert!(ServiceSlot::project_state_base(writable.path()).is_err());
    assert!(!writable.path().join(".harness-gate").exists());
}

#[test]
fn operator_entry_rejects_precreate_and_released_slots_without_intent() {
    let (d, r, p) = fixture();
    let slot = ServiceSlot::acquire_mapped(d.path(), r.clone(), p, vec![]).unwrap();
    let root = slot.root.clone();
    drop(slot);
    for released in [false, true] {
        let path = root.join("service.json");
        let mut record: Record = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        record.released = released;
        fs::write(path, record.bytes().unwrap()).unwrap();
        let error = OperatorRecovery::open_mapped(d.path(), &r.binding.project_id,
            &r.binding.resource_id, 1, &r.binding.lease_invocation, &"9".repeat(32))
            .err().expect("no recovery authority");
        if released {
            assert!(error.to_string().contains("released slot"));
        }
        assert!(!root.join(format!("operator-{}.json", "9".repeat(32))).exists());
    }
}
