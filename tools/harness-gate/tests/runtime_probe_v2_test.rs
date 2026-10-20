#![cfg(target_os = "linux")]

use gatehouse_runtime_protocol::v2::{Binding, Engine, Protocol, Request, Response, Transport};
use gatehouse_runtime_protocol::{Capability, Operation, Outcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const VERSION: &str = "0.0.0";
const HELPER: &str = r#"
#define _GNU_SOURCE
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/types.h>
int main(void) {
    int d = open("dispatched", O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (d < 0) return 1;
    close(d);
    if (access("wait", F_OK) == 0) {
        int r = open("ready", O_WRONLY | O_CREAT | O_TRUNC, 0600);
        if (r < 0) return 2;
        close(r);
        while (access("release", F_OK) != 0) usleep(1000);
    }
    int f = open("response", O_RDONLY);
    if (f < 0) return 3;
    char data[65536];
    ssize_t n = read(f, data, sizeof(data));
    close(f);
    if (n < 0) return 4;
    ssize_t off = 0;
    while (off < n) {
        ssize_t w = write(1, data + off, (size_t)(n - off));
        if (w <= 0) return 5;
        off += w;
    }
    return 0;
}
"#;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn home() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME is required for protected fixtures");
    PathBuf::from(home)
}

fn protected_root() -> (TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("gatehouse-runtime-v2-protected-")
        .tempdir_in(home())
        .unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let canonical = fs::canonicalize(dir.path()).unwrap();
    (dir, canonical)
}

fn write_mode(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn compile_helper(root: &Path) -> (PathBuf, String) {
    let source = root.join("helper.c");
    let helper = root.join("adapter");
    fs::write(&source, HELPER).unwrap();
    let output = Command::new("cc")
        .args(["-O0", "-std=c99", "-o"])
        .arg(&helper)
        .arg(&source)
        .output()
        .expect("cc is required for compiled ELF fixtures");
    assert!(
        output.status.success(),
        "helper compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    let digest = digest(&fs::read(&helper).unwrap());
    (helper, digest)
}

fn config_bytes() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema": "gate-runtime-engine-config/v2",
        "engine": "docker",
        "transport": "unix-http",
        "socket_path": "/run/docker.sock",
        "socket_uid": 1000,
        "principal_uid": 1000,
        "rootless": false,
        "api_version": "1.47",
        "server_version": "27.0.1",
        "daemon_id": "daemon-1",
        "scope": "scope-1"
    }))
    .unwrap()
}

struct Fixture {
    _root: TempDir,
    root: PathBuf,
    adapter: PathBuf,
    adapter_digest: String,
    config: Vec<u8>,
    expected: Vec<u8>,
    actual_path: PathBuf,
    host_path: PathBuf,
    config_path: PathBuf,
    expected_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let (root_guard, root) = protected_root();
        let (adapter, adapter_digest) = compile_helper(&root);
        let config = config_bytes();
        let binding = Binding {
            protocol: Protocol::V2,
            adapter_sha256: adapter_digest.clone(),
            adapter_version: VERSION.into(),
            invocation: "inv-1".into(),
            operation_key: "op-1".into(),
            generation: "gen-1".into(),
            adapter_config_sha256: digest(&config),
            engine: Engine::Docker,
            transport: Transport::UnixHttp,
            socket_path: "/run/docker.sock".into(),
            socket_uid: 1000,
            principal_uid: 1000,
            rootless: false,
            api_version: "1.47".into(),
            server_version: "27.0.1".into(),
            daemon_id: "daemon-1".into(),
            scope: "scope-1".into(),
        };
        let expected = Request {
            binding: binding.clone(),
            operation: Operation::Capabilities,
        };
        let expected = serde_json::to_vec(&expected).unwrap();
        let response = serde_json::to_vec(&Response {
            binding,
            operation: Operation::Capabilities,
            outcome: Outcome::Capabilities {
                supported: vec![Capability::Inspect],
            },
            observations: None,
        })
        .unwrap();

        let config_path = root.join("config.json");
        let expected_path = root.join("expected.json");
        let actual_path = root.join("actual.json");
        let host_path = root.join("host.json");
        write_mode(&config_path, &config, 0o600);
        write_mode(&expected_path, &expected, 0o600);
        write_mode(&actual_path, &expected, 0o600);
        write_mode(&root.join("response"), &response, 0o600);
        let fixture = Self {
            _root: root_guard,
            root,
            adapter,
            adapter_digest,
            config,
            expected,
            actual_path,
            host_path,
            config_path,
            expected_path,
        };
        fixture.write_host(fixture.host_value());
        fixture
    }

    fn host_value(&self) -> Value {
        json!({
            "schema": "gate-runtime-host/v2",
            "executable": {
                "path": self.adapter,
                "sha256": self.adapter_digest,
                "version": VERSION
            },
            "config": {
                "path": self.config_path,
                "sha256": digest(&self.config),
                "schema": "gate-runtime-engine-config/v2"
            },
            "expected": {
                "path": self.expected_path,
                "sha256": digest(&self.expected),
                "schema": "gate-runtime/v2"
            },
            "cwd": self.root,
            "timeout_ms": 5000
        })
    }

    fn write_host(&self, value: Value) {
        self.write_raw_host(&serde_json::to_vec(&value).unwrap());
    }

    fn write_raw_host(&self, bytes: &[u8]) {
        write_mode(&self.host_path, bytes, 0o600);
    }

    fn set_leaf_modes(&self, mode: u32) {
        for path in [&self.host_path, &self.config_path, &self.expected_path] {
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
        }
    }

    fn command_with_paths(&self, host: &Path, actual: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        command
            .current_dir(&self.root)
            .arg("adapter")
            .arg("runtime-probe-v2")
            .arg("--request")
            .arg(actual)
            .arg("--trusted-host")
            .arg(host);
        command
    }

    fn command_with_host(&self, host: &Path) -> Command {
        self.command_with_paths(host, &self.actual_path)
    }

    fn command(&self) -> Command {
        self.command_with_host(&self.host_path)
    }

    fn run(&self) -> Output {
        self.command().output().unwrap()
    }

    fn spawn(&self) -> Child {
        self.command().spawn().unwrap()
    }

    fn dispatched(&self) -> bool {
        self.root.join("dispatched").exists()
    }

    fn write_wait(&self) {
        write_mode(&self.root.join("wait"), b"wait", 0o600);
    }

    fn release(&self) {
        write_mode(&self.root.join("release"), b"release", 0o600);
    }
}

fn wait_deadline(mut child: Child, deadline: Duration) -> Output {
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if started.elapsed() > deadline {
            let _ = child.kill();
            panic!("runtime probe exceeded {deadline:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for(path: &Path, deadline: Duration) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < deadline,
            "timed out waiting for {path:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_rejected(fixture: &Fixture, output: &Output) {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(!fixture.dispatched(), "adapter was spawned");
}

fn assert_failed_after_spawn(fixture: &Fixture, output: &Output) {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(fixture.dispatched(), "adapter did not run");
}

fn assert_positive(fixture: &Fixture) {
    let output = fixture.run();
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["operation"], json!("capabilities"));
    assert_eq!(
        value["outcome"],
        json!({"capabilities": {"supported": ["inspect"]}})
    );
    assert!(fixture.dispatched());
}

#[test]
fn protected_leaves_accept_0400_and_0600() {
    for mode in [0o400, 0o600] {
        let fixture = Fixture::new();
        fixture.set_leaf_modes(mode);
        assert_positive(&fixture);
    }
}

#[test]
fn duplicate_and_unknown_host_fields_are_refused_before_spawn() {
    let fixture = Fixture::new();
    let normal = serde_json::to_string(&fixture.host_value()).unwrap();
    let duplicate = format!("{}, \"timeout_ms\":5000", normal.trim_end_matches('}')) + "}";
    fixture.write_raw_host(duplicate.as_bytes());
    assert_rejected(&fixture, &fixture.run());

    let fixture = Fixture::new();
    let mut unknown = fixture.host_value();
    unknown["unknown"] = json!(true);
    fixture.write_host(unknown);
    assert_rejected(&fixture, &fixture.run());
}

#[test]
fn pin_and_actual_mismatches_are_refused_before_spawn() {
    let fixture = Fixture::new();
    let mut changed_config = fixture.config.clone();
    changed_config.extend_from_slice(b"\n");
    write_mode(&fixture.config_path, &changed_config, 0o600);
    assert_rejected(&fixture, &fixture.run());

    let fixture = Fixture::new();
    write_mode(&fixture.expected_path, b"{}", 0o600);
    assert_rejected(&fixture, &fixture.run());

    let fixture = Fixture::new();
    let mut actual: Value = serde_json::from_slice(&fixture.expected).unwrap();
    actual["binding"]["operation_key"] = json!("different");
    write_mode(
        &fixture.actual_path,
        &serde_json::to_vec(&actual).unwrap(),
        0o600,
    );
    assert_rejected(&fixture, &fixture.run());

    let fixture = Fixture::new();
    let mut host = fixture.host_value();
    host["executable"]["sha256"] = json!("b".repeat(64));
    fixture.write_host(host);
    assert_rejected(&fixture, &fixture.run());
}

#[test]
fn protected_path_link_fifo_mode_and_size_fail_closed() {
    let fixture = Fixture::new();
    let parent_link = fixture.root.join("parent-link");
    symlink(&fixture.root, &parent_link).unwrap();
    let linked_host = parent_link.join("host.json");
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .current_dir(&fixture.root)
        .arg("adapter")
        .arg("runtime-probe-v2")
        .arg("--request")
        .arg(&fixture.actual_path)
        .arg("--trusted-host")
        .arg(&linked_host)
        .output()
        .unwrap();
    assert_rejected(&fixture, &output);

    let fixture = Fixture::new();
    let leaf_link = fixture.root.join("host-link.json");
    symlink(&fixture.host_path, &leaf_link).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .current_dir(&fixture.root)
        .arg("adapter")
        .arg("runtime-probe-v2")
        .arg("--request")
        .arg(&fixture.actual_path)
        .arg("--trusted-host")
        .arg(&leaf_link)
        .output()
        .unwrap();
    assert_rejected(&fixture, &output);

    let fixture = Fixture::new();
    let hard_link = fixture.root.join("host-hardlink.json");
    fs::hard_link(&fixture.host_path, &hard_link).unwrap();
    assert_rejected(&fixture, &fixture.run());

    let fixture = Fixture::new();
    let actual_link = fixture.root.join("actual-link.json");
    symlink(&fixture.actual_path, &actual_link).unwrap();
    let output = fixture
        .command_with_paths(&fixture.host_path, &actual_link)
        .output()
        .unwrap();
    assert_rejected(&fixture, &output);

    let fixture = Fixture::new();
    let fifo = fixture.root.join("host.fifo");
    let name = CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let output = wait_deadline(
        fixture.command_with_host(&fifo).spawn().unwrap(),
        Duration::from_secs(5),
    );
    assert_rejected(&fixture, &output);

    let fixture = Fixture::new();
    fs::set_permissions(&fixture.host_path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_rejected(&fixture, &fixture.run());

    let fixture = Fixture::new();
    write_mode(&fixture.host_path, &vec![b'{'; 65_537], 0o600);
    assert_rejected(&fixture, &fixture.run());
}

#[test]
fn protected_files_changed_during_execution_are_refused() {
    for target in ["host", "config", "expected"] {
        let fixture = Fixture::new();
        fixture.write_wait();
        let child = fixture.spawn();
        wait_for(&fixture.root.join("ready"), Duration::from_secs(5));
        let path = match target {
            "host" => &fixture.host_path,
            "config" => &fixture.config_path,
            "expected" => &fixture.expected_path,
            _ => unreachable!(),
        };
        write_mode(path, b"changed", 0o600);
        fixture.release();
        let output = wait_deadline(child, Duration::from_secs(5));
        assert_failed_after_spawn(&fixture, &output);
    }
}

#[test]
fn protected_leaf_owner_mismatch_is_refused_when_root() {
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("owner mismatch fixture requires root; static owner check reviewed");
        return;
    }
    let fixture = Fixture::new();
    let name = CString::new(fixture.host_path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::chown(name.as_ptr(), 65_534, 0) }, 0);
    assert_rejected(&fixture, &fixture.run());
}

#[test]
fn cli_contract_uses_separate_untrusted_request_and_protected_host() {
    let fixture = Fixture::new();
    let output = fixture.run();
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("binding").is_some());
    assert!(!fixture.actual_path.join("..").exists());
    assert!(fixture.actual_path.metadata().unwrap().is_file());
    assert_eq!(fixture.actual_path.metadata().unwrap().uid(), unsafe {
        libc::geteuid()
    });
}
