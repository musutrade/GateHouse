use super::*;
use gatehouse_runtime_protocol::lifecycle_v2::observation::ExpectedContext;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::UnixListener,
    },
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn fixture() -> (
    tempfile::TempDir,
    VerifiedPodmanInstallation,
    UnixListener,
    Vec<u8>,
) {
    let native = std::env::var("HG_PODMAN_NATIVE_BINARY").expect("pinned native binary required");
    let expected = std::env::var("HG_PODMAN_NATIVE_SHA256").expect("native SHA required");
    fixture_binary(&native, &expected)
}
fn fixture_binary(
    native: &str,
    expected: &str,
) -> (
    tempfile::TempDir,
    VerifiedPodmanInstallation,
    UnixListener,
    Vec<u8>,
) {
    let executable = fs::read(native).unwrap();
    assert_eq!(hash(&executable), expected);
    let dir = tempfile::tempdir_in(std::env::var_os("HOME").unwrap()).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let socket = dir.path().join("p.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let m = fs::metadata(&socket).unwrap();
    let uid = unsafe { libc::geteuid() };
    let context:ExpectedContext=serde_json::from_value(json!({"operator_epoch":"a".repeat(32),"principal_uid":uid,"observation":{"engine":"podman","api_surface":"libpod","selected_api":"6.1.4","server_version":"6.1.4","hostname":"fixture","rootless":uid!=0,"graph_root":"/var/lib/containers/storage","run_root":"/run/containers/storage","remote_socket":null,"socket":{"path":socket,"device":m.dev(),"inode":m.ino(),"uid":m.uid(),"mode":m.mode()}}})).unwrap();
    let config = format!(
        "{{\"schema\":\"gate-runtime/podman-context-v1\",\"context\":{}}}",
        serde_json::to_string(&context).unwrap()
    );
    let cp = dir.path().join("context.json");
    fs::write(&cp, &config).unwrap();
    fs::set_permissions(&cp, fs::Permissions::from_mode(0o600)).unwrap();
    let ep = dir.path().join("adapter");
    fs::write(&ep, &executable).unwrap();
    fs::set_permissions(
        &ep,
        fs::Permissions::from_mode(if uid == 0 { 0o755 } else { 0o700 }),
    )
    .unwrap();
    let install=format!("{{\"schema\":\"gate-runtime/podman-installation-v1\",\"product\":\"podman-lifecycle\",\"version\":\"0.1.0\",\"protocol\":\"gate-runtime/lifecycle-v2\",\"executable_path\":{},\"executable_sha256\":\"{}\",\"config_schema\":\"gate-runtime/podman-context-v1\"}}",serde_json::to_string(&ep).unwrap(),hash(&executable));
    let ip = dir.path().join("installation.json");
    fs::write(&ip, &install).unwrap();
    fs::set_permissions(&ip, fs::Permissions::from_mode(0o600)).unwrap();
    let host=format!("{{\"schema\":\"gate-runtime/podman-host-v1\",\"installation_path\":{},\"installation_sha256\":\"{}\",\"config_path\":{},\"config_sha256\":\"{}\"}}",serde_json::to_string(&ip).unwrap(),hash(install.as_bytes()),serde_json::to_string(&cp).unwrap(),hash(config.as_bytes()));
    let hp = dir.path().join("host.json");
    fs::write(&hp, &host).unwrap();
    fs::set_permissions(&hp, fs::Permissions::from_mode(0o600)).unwrap();
    let info=serde_json::to_vec(&json!({"host":{"hostname":"fixture","security":{"rootless":uid!=0}},"store":{"graphRoot":"/var/lib/containers/storage","runRoot":"/run/containers/storage"},"version":{"APIVersion":"6.1.4","Version":"6.1.4"}})).unwrap();
    let installed = VerifiedPodmanInstallation::load(&hp).unwrap();
    (dir, installed, listener, info)
}
fn serve(listener: UnixListener, info: Vec<u8>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        for n in 0..3 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut b = [0];
                stream.read_exact(&mut b).unwrap();
                header.push(b[0]);
                assert!(header.len() < 16384);
            }
            let route = if n == 1 {
                format!("/v6.1.4/libpod/images/{}/json", "b".repeat(64))
            } else {
                "/v6.1.4/libpod/info".into()
            };
            assert!(String::from_utf8(header)
                .unwrap()
                .starts_with(&format!("GET {route} HTTP/1.1\r\n")));
            let body = if n == 1 {
                serde_json::to_vec(&json!({"Id":"b".repeat(64),"Config":{}})).unwrap()
            } else {
                info.clone()
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    })
}
fn inputs() -> PlanInputs {
    PlanInputs {
        name: "fixture".into(),
        labels: [("owner".into(), "fixture".into())].into(),
        port: None,
    }
}
#[test]
fn native_held_executable_capture_persists_and_matches() {
    let (dir, installed, listener, info) = fixture();
    let server = serve(listener, info);
    let record = capture(&installed, "b".repeat(64), inputs(), 3000, || false).unwrap();
    server.join().unwrap();
    let path = dir.path().join("record");
    record
        .create_new(&path, installed.context().unwrap())
        .unwrap();
    let reopened = PlanRecord::open(&path, installed.context().unwrap()).unwrap();
    installed
        .match_record(&reopened)
        .unwrap()
        .revalidate()
        .unwrap();
    assert_eq!(reopened.encode().unwrap(), record.encode().unwrap());
    assert!(!record.installation_verified());
}
#[test]
fn post_revalidation_cancel_and_expiry_withhold_valid_response() {
    for expire in [false, true] {
        let (_dir, installed, listener, info) = fixture();
        let server = serve(listener, info);
        let cancelled = AtomicBool::new(false);
        let result = capture_observed(
            &installed,
            "b".repeat(64),
            inputs(),
            1500,
            || cancelled.load(Ordering::SeqCst),
            || {
                if expire {
                    thread::sleep(Duration::from_millis(1500));
                } else {
                    cancelled.store(true, Ordering::SeqCst);
                }
            },
        );
        assert_eq!(
            result.err(),
            Some("Podman capture post-revalidation expired or cancelled")
        );
        server.join().unwrap();
    }
}

#[test]
fn final_observer_installation_replacement_is_rejected() {
    let (dir, installed, listener, info) = fixture();
    let server = serve(listener, info);
    let result = capture_observed(
        &installed,
        "b".repeat(64),
        inputs(),
        3000,
        || false,
        || {
            let path = dir.path().join("adapter");
            let bytes = fs::read(&path).unwrap();
            let mode = fs::metadata(&path).unwrap().permissions();
            fs::rename(&path, dir.path().join("old-adapter")).unwrap();
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, mode).unwrap();
        },
    );
    assert_eq!(result.err(), Some("installation member changed"));
    server.join().unwrap();
}
#[test]
fn native_fault_output_and_term_resistant_timeout_are_discarded_and_reaped() {
    let binary = std::env::var("HG_PODMAN_FAULT_BINARY").expect("fault fixture binary");
    let sha = std::env::var("HG_PODMAN_FAULT_SHA256").expect("fault fixture sha");
    for mode in [
        "partial-exit",
        "stderr",
        "stderr-near-cap",
        "stderr-at-cap",
        "stderr-over-cap",
        "oversize",
        "term-resistant",
    ] {
        let (dir, installed, listener, _) = fixture_binary(&binary, &sha);
        fs::write(dir.path().join("fault-mode"), mode).unwrap();
        let started = Instant::now();
        assert!(
            capture(&installed, "b".repeat(64), inputs(), 500, || false).is_err(),
            "{mode}"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        let pid: i32 = fs::read_to_string(dir.path().join("fault-pid"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "child remains: {mode}");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}

#[test]
fn descendant_supervision_isolated() {
    const KEY: &str = "HG_PODMAN_ISOLATED_DESCENDANT";
    if std::env::var_os(KEY).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process::podman_capture_host::tests::descendant_supervision_isolated",
                "--nocapture",
            ])
            .env(KEY, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    // Only this dedicated test subprocess changes orphan adoption policy.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    let binary = std::env::var("HG_PODMAN_FAULT_BINARY").unwrap();
    let sha = std::env::var("HG_PODMAN_FAULT_SHA256").unwrap();
    for escaped in [false, true] {
        let (dir, installed, listener, _) = fixture_binary(&binary, &sha);
        fs::write(
            dir.path().join("fault-mode"),
            if escaped {
                "descendant-escaped"
            } else {
                "descendant-group"
            },
        )
        .unwrap();
        let result = capture(&installed, "b".repeat(64), inputs(), 1500, || false);
        let direct: i32 = fs::read_to_string(dir.path().join("fault-pid"))
            .unwrap()
            .parse()
            .unwrap();
        let descendant: i32 = fs::read_to_string(dir.path().join("descendant-pid"))
            .unwrap()
            .parse()
            .unwrap();
        let mut status = 0;
        let observed = unsafe { libc::waitpid(descendant, &mut status, libc::WNOHANG) };
        // Cleanup before assertions: escape is a negative containment result.
        if observed == 0 {
            assert_eq!(unsafe { libc::kill(descendant, libc::SIGKILL) }, 0);
            assert_eq!(
                unsafe { libc::waitpid(descendant, &mut status, 0) },
                descendant
            );
        }
        assert!(result.is_err());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(unsafe { libc::kill(direct, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        assert_eq!(observed, if escaped { 0 } else { descendant });
        assert!(libc::WIFSIGNALED(status));
        assert_eq!(libc::WTERMSIG(status), libc::SIGKILL);
        assert_eq!(unsafe { libc::kill(descendant, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        eprintln!("escaped={escaped}: direct reaped; group observation={observed}; adopted descendant reaped");
    }
}

#[test]
fn actual_native_host_get_signal_and_cancellation_discard_and_reap() {
    for signal in [0, libc::SIGTERM, libc::SIGINT] {
        let (_dir, installed, listener, _) = fixture();
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
            let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
            assert_eq!(
                unsafe {
                    libc::getsockopt(
                        stream.as_raw_fd(),
                        libc::SOL_SOCKET,
                        libc::SO_PEERCRED,
                        (&mut peer as *mut libc::ucred).cast(),
                        &mut len,
                    )
                },
                0
            );
            assert!(peer.pid > 0);
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 16384);
            }
            assert!(header.starts_with(b"GET /v6.1.4/libpod/info HTTP/1.1\r\n"));
            // No response or EOF is supplied to trigger the adapter failure.
            if signal == 0 {
                flag.store(true, Ordering::SeqCst);
            } else {
                assert_eq!(unsafe { libc::kill(peer.pid, signal) }, 0);
            }
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 0);
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            peer.pid
        });
        let started = Instant::now();
        let result = capture(&installed, "b".repeat(64), inputs(), 4000, || {
            cancelled.load(Ordering::SeqCst)
        });
        let pid = server.join().unwrap();
        assert!(result.is_err(), "signal {signal}");
        assert!(started.elapsed() < Duration::from_secs(6));
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}

#[test]
fn actual_project_entry_captures_only_when_explicitly_called() {
    let (dir, _installed, listener, info) = fixture();
    let root = dir.path();
    crate::preset::init(root, "generic", false).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .unwrap()
        .success());
    let mapped = root.join(".harness-gate/runtime/podman-capture-v1");
    fs::create_dir_all(&mapped).unwrap();
    for p in [
        root.join(".harness-gate"),
        root.join(".harness-gate/runtime"),
        mapped.clone(),
    ] {
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::copy(root.join("host.json"), mapped.join("host.json")).unwrap();
    let project = crate::project::Project::discover(Some(root.to_path_buf()), None).unwrap();
    assert_eq!(project.root, root);
    // Discovery with a valid mapped host must not execute or connect.
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let server = serve(listener, info);
    let record = capture_installed(&project, "b".repeat(64), inputs(), 3000, || false).unwrap();
    server.join().unwrap();
    assert_eq!(fs::read_dir(&mapped).unwrap().count(), 1);
    assert!(!record.installation_verified());
    let installed = VerifiedPodmanInstallation::load(&mapped.join("host.json")).unwrap();
    record
        .create_new(&root.join("explicit-record"), installed.context().unwrap())
        .unwrap();
    assert_eq!(
        PlanRecord::open(&root.join("explicit-record"), installed.context().unwrap())
            .unwrap()
            .encode()
            .unwrap(),
        record.encode().unwrap()
    );
}

#[test]
fn fixed_project_mapping_refuses_missing_and_symlink_hosts_without_fallback() {
    let (dir, _installed, listener, _) = fixture();
    let root = dir.path();
    // A valid arbitrary host.json already exists at root; never use it as fallback.
    assert!(capture_mapped(root, "b".repeat(64), inputs(), 3000, || false).is_err());
    let mapped = root.join(".harness-gate/runtime/podman-capture-v1");
    fs::create_dir_all(&mapped).unwrap();
    for p in [
        root.join(".harness-gate"),
        root.join(".harness-gate/runtime"),
        mapped.clone(),
    ] {
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::os::unix::fs::symlink(root.join("host.json"), mapped.join("host.json")).unwrap();
    assert!(capture_mapped(root, "b".repeat(64), inputs(), 3000, || false).is_err());
    assert!(capture_mapped(
        std::path::Path::new("."),
        "b".repeat(64),
        inputs(),
        3000,
        || false
    )
    .is_err());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn mapped_host_is_explicit_trust_root_for_protected_external_members() {
    for mode in ["valid", "digest-mismatch", "writable-executable"] {
        let (external, _installed, listener, info) = fixture();
        let project = tempfile::tempdir_in(std::env::var_os("HOME").unwrap()).unwrap();
        fs::set_permissions(project.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let mapped = project
            .path()
            .join(".harness-gate/runtime/podman-capture-v1");
        fs::create_dir_all(&mapped).unwrap();
        for p in [
            project.path().join(".harness-gate"),
            project.path().join(".harness-gate/runtime"),
            mapped.clone(),
        ] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        // Exact descriptor bytes intentionally reference a separate protected tree.
        fs::copy(external.path().join("host.json"), mapped.join("host.json")).unwrap();
        match mode {
            "digest-mismatch" => {
                let path = external.path().join("adapter");
                let mut bytes = fs::read(&path).unwrap();
                bytes.push(0);
                fs::write(path, bytes).unwrap();
            }
            "writable-executable" => fs::set_permissions(
                external.path().join("adapter"),
                fs::Permissions::from_mode(0o777),
            )
            .unwrap(),
            _ => {}
        }
        if mode == "valid" {
            let server = serve(listener, info);
            let record =
                capture_mapped(project.path(), "b".repeat(64), inputs(), 3000, || false).unwrap();
            server.join().unwrap();
            assert!(!record.installation_verified());
            assert_eq!(fs::read_dir(mapped).unwrap().count(), 1);
        } else {
            assert!(
                capture_mapped(project.path(), "b".repeat(64), inputs(), 3000, || false).is_err(),
                "{mode}"
            );
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }
}

#[test]
fn cli_capture_preflight_and_canonical_success() {
    use clap::Parser;
    let (dir, _installed, listener, info) = fixture();
    let root = dir.path();
    crate::preset::init(root, "generic", false).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .unwrap()
        .success());
    let mapped = root.join(".harness-gate/runtime/podman-capture-v1");
    fs::create_dir_all(&mapped).unwrap();
    for p in [
        root.join(".harness-gate"),
        root.join(".harness-gate/runtime"),
        mapped.clone(),
    ] {
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::copy(root.join("host.json"), mapped.join("host.json")).unwrap();
    let argv = vec![
        "harness-gate".to_string(),
        "--project-root".into(),
        root.to_str().unwrap().into(),
        "podman-capture".into(),
        "--trust-host".into(),
        "--image-id".into(),
        "b".repeat(64),
        "--name".into(),
        "fixture".into(),
        "--label".into(),
        "owner=fixture".into(),
    ];
    for negative in 0..6 {
        let mut cli = crate::cli::Cli::try_parse_from(&argv).unwrap();
        if negative == 0 {
            cli.project_root = None;
        }
        if negative == 1 {
            cli.project_root = Some(".".into());
        }
        let crate::cli::Commands::PodmanCapture(ref mut args) = cli.command else {
            panic!()
        };
        match negative {
            2 => args.trust_host = false,
            3 => args.image_id = "short".into(),
            4 => args.label.push("owner=duplicate".into()),
            5 => args.timeout_ms = 0,
            _ => {}
        }
        let crate::cli::Commands::PodmanCapture(ref args) = cli.command else {
            panic!()
        };
        assert!(crate::app::podman_capture_bytes(&cli, args).is_err());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    let cli = crate::cli::Cli::try_parse_from(&argv).unwrap();
    let crate::cli::Commands::PodmanCapture(ref args) = cli.command else {
        panic!()
    };
    let server = serve(listener, info);
    let bytes = crate::app::podman_capture_bytes(&cli, args).unwrap();
    server.join().unwrap();
    let installed = VerifiedPodmanInstallation::load(&mapped.join("host.json")).unwrap();
    let record = PlanRecord::decode(&bytes, installed.context().unwrap()).unwrap();
    assert_eq!(record.encode().unwrap(), bytes);
    assert_eq!(fs::read_dir(&mapped).unwrap().count(), 1);
    fs::remove_file(mapped.join("host.json")).unwrap();
    assert!(crate::app::podman_capture_bytes(&cli, args).is_err());
}

#[test]
fn normal_core_cli_exact_output_config_and_failure_contract() {
    let core = std::env::var("HG_PODMAN_CORE_BINARY").expect("pinned normal Core binary");
    assert_eq!(
        hash(&fs::read(&core).unwrap()),
        std::env::var("HG_PODMAN_CORE_SHA256").unwrap()
    );
    for mode in [
        "success",
        "config",
        "missing-trust",
        "missing-host",
        "full-output",
    ] {
        let (dir, _installed, listener, info) = fixture();
        let root = dir.path();
        crate::preset::init(root, "generic", false).unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root)
            .status()
            .unwrap()
            .success());
        let mapped = root.join(".harness-gate/runtime/podman-capture-v1");
        fs::create_dir_all(&mapped).unwrap();
        for p in [
            root.join(".harness-gate"),
            root.join(".harness-gate/runtime"),
            mapped.clone(),
        ] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        if mode != "missing-host" {
            fs::copy(root.join("host.json"), mapped.join("host.json")).unwrap();
        }
        let mut command = std::process::Command::new(&core);
        command.env_clear().current_dir("/").args([
            "--project-root",
            root.to_str().unwrap(),
            "--color",
            "never",
        ]);
        if mode == "config" {
            fs::create_dir(root.join("alternate")).unwrap();
            fs::copy(
                root.join(crate::config::DEFAULT_CONFIG_PATH),
                root.join("alternate/workflow.toml"),
            )
            .unwrap();
            command.args(["--config", "alternate/workflow.toml"]);
        }
        command.args([
            "podman-capture",
            "--image-id",
            &"b".repeat(64),
            "--name",
            "fixture",
            "--label",
            "owner=fixture",
        ]);
        if mode != "missing-trust" {
            command.arg("--trust-host");
        }
        let connects = matches!(mode, "success" | "config" | "full-output");
        let check_listener = listener.try_clone().unwrap();
        let server = connects.then(|| serve(listener, info));
        if mode == "full-output" {
            command.stdout(
                fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .unwrap(),
            );
        }
        let output = command.output().unwrap();
        if let Some(server) = server {
            server.join().unwrap();
        }
        if matches!(mode, "success" | "config") {
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let installed = VerifiedPodmanInstallation::load(&mapped.join("host.json")).unwrap();
            let record = PlanRecord::decode(&output.stdout, installed.context().unwrap()).unwrap();
            assert_eq!(record.encode().unwrap(), output.stdout);
            assert!(!output.stdout.ends_with(b"\n"));
        } else {
            assert_eq!(
                output.status.code(),
                Some(if mode == "missing-trust" { 2 } else { 1 })
            );
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
        assert_eq!(
            check_listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(
            fs::read_dir(&mapped).unwrap().count(),
            usize::from(mode != "missing-host")
        );
    }
}
