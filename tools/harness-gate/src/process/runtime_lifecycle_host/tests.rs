use super::*;
use gatehouse_runtime_protocol::lifecycle::{
    journal::Journal,
    state::Phase,
    wire::{Entry, Operation, Outcome},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::OnceLock,
};
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn save(path: &Path, value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap();
    fs::write(path, &bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    hash(&bytes)
}
fn executable() -> &'static [u8] {
    static EXE: OnceLock<Vec<u8>> = OnceLock::new();
    EXE.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("main.rs");
        fs::write(&src, include_bytes!("fixture.txt")).unwrap();
        let output = Command::new("rustc")
            .arg("--edition=2021")
            .arg(&src)
            .arg("-o")
            .arg(d.path().join("fixture"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(d.path().join("fixture")).unwrap()
    })
}
struct Fixture {
    _dir: tempfile::TempDir,
    installed: VerifiedInstallation,
    journal: Journal,
    host: PathBuf,
}
fn fixture(mode: &str) -> Fixture {
    let root = std::env::var_os("GATEHOUSE_JOURNAL_TEST_ROOT")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap();
    let d = tempfile::Builder::new()
        .prefix("lifecycle-host-")
        .tempdir_in(root)
        .unwrap();
    fs::set_permissions(d.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let exe = d.path().join("adapter");
    fs::write(&exe, executable()).unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o500)).unwrap();
    let mut request = wire::parse_request(include_bytes!(
        "../../../runtime-protocol/src/lifecycle/wire/status-request.json"
    ))
    .unwrap();
    let Operation::Status { target } = request.operation else {
        panic!()
    };
    request.operation = Operation::Create {
        name: target.name,
        image_id: format!("sha256:{}", "e".repeat(64)),
        labels: target.labels,
        container_port: target.container_port,
        environment: vec![Entry {
            key: "HOST_TEST_MODE".into(),
            value: mode.into(),
        }],
    };
    request.binding.engine_binding.principal_uid = unsafe { libc::geteuid() };
    let config = d.path().join("config.json");
    let install = d.path().join("installation.json");
    let host = d.path().join("host.json");
    request.binding.executable_sha256 = hash(executable());
    request.binding.config_sha256 = save(
        &config,
        &json!({"schema":"gate-runtime/lifecycle-config-v1","engine_binding":request.binding.engine_binding}),
    );
    request.binding.installation_sha256 = save(
        &install,
        &json!({"schema":"gate-runtime/lifecycle-installation-v1","product":"docker-lifecycle","version":"0.1.0","protocol":wire::PROTOCOL,"executable_path":exe,"executable_sha256":request.binding.executable_sha256,"config_schema":"gate-runtime/lifecycle-config-v1"}),
    );
    save(
        &host,
        &json!({"schema":"gate-runtime/lifecycle-host-v1","installation_path":install,"installation_sha256":request.binding.installation_sha256,"config_path":config,"config_sha256":request.binding.config_sha256}),
    );
    request.binding.operation_key = request.operation_key().unwrap();
    let installed = VerifiedInstallation::load(&host).unwrap();
    let journal_dir = d.path().join("journal");
    fs::create_dir(&journal_dir).unwrap();
    fs::set_permissions(&journal_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let journal = Journal::create(&journal_dir, &request).unwrap();
    Fixture {
        _dir: d,
        installed,
        journal,
        host,
    }
}
#[test]
fn descriptor_execution_receives_sealed_exact_inputs_and_empty_environment() {
    let mut f = fixture("created");
    let result = dispatch(f.journal.dispatch().unwrap(), &f.installed, 3000, || false).unwrap();
    assert!(matches!(result.outcome, Outcome::Created { .. }));
    assert_eq!(f.journal.phase(), Phase::KnownCreated);
    assert_eq!(f.journal.target().unwrap().object_id, "a".repeat(64));
    assert!(f.journal.dispatch().is_err());
}
#[test]
fn failures_and_cancellation_consume_dispatch_without_retry() {
    for (mode, timeout, cancel) in [
        ("stderr", 3000, false),
        ("exit", 3000, false),
        ("malformed", 3000, false),
        ("timeout", 100, false),
        ("ok", 3000, true),
        ("ok", 0, false),
    ] {
        let mut f = fixture(mode);
        assert!(
            dispatch(f.journal.dispatch().unwrap(), &f.installed, timeout, || {
                cancel
            })
            .is_err(),
            "{mode}"
        );
        assert_eq!(f.journal.phase(), Phase::UnresolvedNoId);
        assert!(f.journal.dispatch().is_err());
    }
}
#[test]
fn replaced_host_is_rejected_before_spawn() {
    let mut f = fixture("ok");
    let bytes = fs::read(&f.host).unwrap();
    let replacement = f.host.with_extension("replacement");
    fs::write(&replacement, bytes).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(replacement, &f.host).unwrap();
    assert!(dispatch(f.journal.dispatch().unwrap(), &f.installed, 3000, || false).is_err());
    assert_eq!(f.journal.phase(), Phase::UnresolvedNoId);
}

#[test]
fn valid_unknown_and_rejection_are_explicit_errors() {
    for (mode, error) in [
        ("ok", DispatchError::Unknown(Reason::Transport)),
        (
            "rejected",
            DispatchError::RejectedBeforeEffect(Reason::UnsupportedState),
        ),
    ] {
        let mut f = fixture(mode);
        assert_eq!(
            dispatch(f.journal.dispatch().unwrap(), &f.installed, 3000, || false),
            Err(error)
        );
        assert_eq!(f.journal.phase(), Phase::UnresolvedNoId);
        assert!(f.journal.dispatch().is_err());
    }
}
#[test]
fn final_cancellation_occurs_after_validated_created_response() {
    use std::cell::Cell;
    let mut f = fixture("created");
    let crossed = Cell::new(false);
    let final_checks = Cell::new(0);
    let result = dispatch_observed(
        f.journal.dispatch().unwrap(),
        &f.installed,
        3000,
        || {
            if crossed.get() {
                final_checks.set(final_checks.get() + 1);
                true
            } else {
                false
            }
        },
        |boundary| {
            if let Boundary::ResponseValidated(response) = boundary {
                assert!(matches!(response.outcome, Outcome::Created { .. }));
                crossed.set(true);
            }
        },
    );
    assert!(crossed.get());
    assert_eq!(final_checks.get(), 1);
    assert_eq!(
        result,
        Err(DispatchError::Boundary(
            "lifecycle final boundary cancelled or expired"
        ))
    );
    assert_eq!(f.journal.phase(), Phase::UnresolvedNoId);
    assert!(f.journal.dispatch().is_err());
}
#[test]
fn same_byte_replacement_at_both_boundaries_prevents_commit() {
    for filename in ["host.json", "installation.json", "config.json", "adapter"] {
        for after_response in [false, true] {
            let mut f = fixture("created");
            let path = f.host.parent().unwrap().join(filename);
            let mut replaced = false;
            let mut validated = false;
            let result = dispatch_observed(
                f.journal.dispatch().unwrap(),
                &f.installed,
                3000,
                || false,
                |boundary| {
                    let at_response = matches!(boundary, Boundary::ResponseValidated(_));
                    if let Boundary::ResponseValidated(response) = boundary {
                        assert!(matches!(response.outcome, Outcome::Created { .. }));
                        validated = true;
                    }
                    if at_response == after_response {
                        let replacement = path.with_extension("replacement");
                        fs::write(&replacement, fs::read(&path).unwrap()).unwrap();
                        fs::set_permissions(
                            &replacement,
                            fs::metadata(&path).unwrap().permissions(),
                        )
                        .unwrap();
                        fs::rename(replacement, &path).unwrap();
                        replaced = true;
                    }
                },
            );
            assert!(replaced);
            assert_eq!(validated, after_response);
            assert_eq!(
                result,
                Err(DispatchError::Boundary("lifecycle installation changed")),
                "{filename}"
            );
            assert_eq!(f.journal.phase(), Phase::UnresolvedNoId);
            assert!(f.journal.dispatch().is_err());
        }
    }
}
