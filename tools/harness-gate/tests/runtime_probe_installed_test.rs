#![cfg(target_os = "linux")]

use gatehouse_runtime_protocol::v2::{Binding, Engine, Protocol, Request, Response, Transport};
use gatehouse_runtime_protocol::{Capability, Operation, Outcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixListener;
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
    char input[65536];
    while (read(0, input, sizeof(input)) > 0) {}
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
    PathBuf::from(std::env::var_os("HOME").expect("HOME"))
}

fn protected_root() -> (TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("gatehouse-runtime-installed-")
        .tempdir_in(home())
        .unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

fn write_mode(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn config_bytes(socket: &Path) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema": "gate-runtime-engine-config/v2",
        "engine": "docker",
        "transport": "unix-http",
        "socket_path": socket,
        "socket_uid": unsafe { libc::geteuid() },
        "principal_uid": unsafe { libc::geteuid() },
        "rootless": false,
        "api_version": "1.47",
        "server_version": "27.0.1",
        "daemon_id": "daemon-1",
        "scope": "scope-1"
    }))
    .unwrap()
}

fn request(config: &[u8], executable: &str, version: &str, socket: &Path) -> Request {
    Request {
        binding: Binding {
            protocol: Protocol::V2,
            adapter_sha256: executable.into(),
            adapter_version: version.into(),
            invocation: "inv-1".into(),
            operation_key: "op-1".into(),
            generation: "gen-1".into(),
            adapter_config_sha256: digest(config),
            engine: Engine::Docker,
            transport: Transport::UnixHttp,
            socket_path: socket.to_string_lossy().into_owned(),
            socket_uid: unsafe { libc::geteuid() },
            principal_uid: unsafe { libc::geteuid() },
            rootless: false,
            api_version: "1.47".into(),
            server_version: "27.0.1".into(),
            daemon_id: "daemon-1".into(),
            scope: "scope-1".into(),
        },
        operation: Operation::Capabilities,
    }
}

fn response(request: &Request) -> Vec<u8> {
    serde_json::to_vec(&Response {
        binding: request.binding.clone(),
        operation: Operation::Capabilities,
        outcome: Outcome::Capabilities {
            supported: vec![Capability::Inspect],
        },
        observations: None,
    })
    .unwrap()
}

struct Fixture {
    _guard: TempDir,
    root: PathBuf,
    adapter: PathBuf,
    adapter_digest: String,
    version: String,
    config: Vec<u8>,
    expected: Vec<u8>,
    actual: PathBuf,
    descriptor: PathBuf,
    descriptor_bytes: Vec<u8>,
    host: PathBuf,
    config_path: PathBuf,
    expected_path: PathBuf,
    _socket: Option<UnixListener>,
}

impl Fixture {
    fn new() -> Self {
        let (guard, root) = protected_root();
        let source = root.join("helper.c");
        let adapter = root.join("adapter");
        fs::write(&source, HELPER).unwrap();
        let output = Command::new("cc")
            .args(["-O0", "-std=c99", "-o"])
            .arg(&adapter)
            .arg(&source)
            .output()
            .expect("cc");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).unwrap();
        Self::with_adapter(guard, root, adapter, VERSION.into(), None)
    }

    fn with_adapter(
        guard: TempDir,
        root: PathBuf,
        adapter: PathBuf,
        version: String,
        listener: Option<UnixListener>,
    ) -> Self {
        let socket_path = root.join("docker.sock");
        let _socket = listener;
        let digest = digest(&fs::read(&adapter).unwrap());
        let config = config_bytes(&socket_path);
        let expected =
            serde_json::to_vec(&request(&config, &digest, &version, &socket_path)).unwrap();
        let actual = root.join("actual.json");
        let descriptor = root.join("installation.json");
        let config_path = root.join("config.json");
        let expected_path = root.join("expected.json");
        let host = root.join("host.json");
        write_mode(&config_path, &config, 0o600);
        write_mode(&expected_path, &expected, 0o600);
        write_mode(&actual, &expected, 0o600);
        write_mode(
            &root.join("response"),
            &response(&request(&config, &digest, &version, &socket_path)),
            0o600,
        );
        let descriptor_bytes = serde_json::to_vec(&json!({
            "schema": "gate-runtime-installation/v1",
            "repository": "musutrade/GateProbes",
            "tag": "docker-runtime-v0.1.1",
            "source_commit": "29c619b603ef1656dc5901466385a879806cf104",
            "archive_sha256": "9be496218abde930e3682170aeac2b6d450db3adb7bacf588bf5cfdb3f9fe213",
            "executable": {"path": adapter, "sha256": digest, "version": version}
        }))
        .unwrap();
        write_mode(&descriptor, &descriptor_bytes, 0o600);
        let fixture = Self {
            _guard: guard,
            root: root.clone(),
            adapter,
            adapter_digest: digest,
            version,
            config,
            expected,
            actual,
            descriptor,
            descriptor_bytes,
            host,
            config_path,
            expected_path,
            _socket,
        };
        fixture.write_host(fixture.host_value());
        fixture
    }

    fn host_value(&self) -> Value {
        json!({
            "schema": "gate-runtime-host/installed-v1",
            "installation": {
                "path": self.descriptor,
                "sha256": digest(&fs::read(&self.descriptor).unwrap()),
                "schema": "gate-runtime-installation/v1"
            },
            "executable": {"path": self.adapter, "sha256": self.adapter_digest, "version": self.version},
            "config": {"path": self.config_path, "sha256": digest(&self.config), "schema": "gate-runtime-engine-config/v2"},
            "expected": {"path": self.expected_path, "sha256": digest(&self.expected), "schema": "gate-runtime/v2"},
            "cwd": self.root,
            "timeout_ms": 5000
        })
    }

    fn write_host(&self, value: Value) {
        write_mode(&self.host, &serde_json::to_vec(&value).unwrap(), 0o600);
    }

    fn host_without_installation(&self) -> Value {
        let mut value = self.host_value();
        value.as_object_mut().unwrap().remove("installation");
        value
    }

    fn command(&self, installed: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        command
            .current_dir(&self.root)
            .arg("adapter")
            .arg(if installed {
                "runtime-probe-installed"
            } else {
                "runtime-probe-v2"
            })
            .arg("--request")
            .arg(&self.actual)
            .arg("--trusted-host")
            .arg(&self.host);
        command
    }

    fn run(&self, installed: bool) -> Output {
        self.command(installed).output().unwrap()
    }

    fn dispatched(&self) -> bool {
        self.root.join("dispatched").exists()
    }

    fn assert_rejected(&self, output: &Output) {
        assert!(!output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(!self.dispatched(), "adapter was spawned");
    }

    fn assert_positive(&self, output: &Output) {
        assert!(output.status.success(), "{output:?}");
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["operation"], json!("capabilities"));
        assert_eq!(
            value["outcome"],
            json!({"capabilities": {"supported": ["inspect"]}})
        );
        assert!(self.dispatched());
    }
    fn write_descriptor(&self, bytes: &[u8]) {
        write_mode(&self.descriptor, bytes, 0o600);
        let mut host = self.host_value();
        host["installation"]["sha256"] = json!(digest(bytes));
        self.write_host(host);
    }
}

#[test]
fn malformed_descriptor_fields_are_zero_spawn() {
    let fixture = Fixture::new();
    let base: Value = serde_json::from_slice(&fixture.descriptor_bytes).unwrap();
    let mut cases = Vec::new();
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("repository");
    cases.push(missing);
    let mut unknown = base.clone();
    unknown["unknown"] = json!(true);
    cases.push(unknown);
    let mut schema = base.clone();
    schema["schema"] = json!("gate-runtime-installation/v2");
    cases.push(schema);
    let mut source = base.clone();
    source["source_commit"] = json!("A".repeat(40));
    cases.push(source);
    let mut archive = base.clone();
    archive["archive_sha256"] = json!("f".repeat(63));
    cases.push(archive);
    for descriptor in cases {
        fixture.write_descriptor(&serde_json::to_vec(&descriptor).unwrap());
        fixture.assert_rejected(&fixture.run(true));
    }
    let normal = serde_json::to_string(&base).unwrap();
    let duplicate = format!("{}, \"tag\":\"duplicate\"", normal.trim_end_matches('}')) + "}";
    fixture.write_descriptor(duplicate.as_bytes());
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn executable_leaf_modes_refuse_write_special_and_nonexec() {
    for mode in [0o775, 0o777, 0o4755, 0o2755, 0o1755, 0o644, 0o600] {
        let fixture = Fixture::new();
        fs::set_permissions(&fixture.adapter, fs::Permissions::from_mode(mode)).unwrap();
        fixture.assert_rejected(&fixture.run(true));
    }
}

#[test]
fn executable_leaf_hardlink_is_refused() {
    let fixture = Fixture::new();
    let hard = fixture.root.join("adapter-hard");
    fs::hard_link(&fixture.adapter, &hard).unwrap();
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn executable_leaf_owner_mismatch_is_refused_when_root() {
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let fixture = Fixture::new();
    let path = std::ffi::CString::new(fixture.adapter.to_string_lossy().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::chown(path.as_ptr(), 65534, 65534) }, 0);
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn executable_leaf_postexec_mode_and_replacement_refuse() {
    let fixture = Fixture::new();
    write_mode(&fixture.root.join("wait"), b"wait", 0o600);
    let child = fixture.command(true).spawn().unwrap();
    wait_for(&fixture.root.join("ready"));
    fs::set_permissions(&fixture.adapter, fs::Permissions::from_mode(0o777)).unwrap();
    write_mode(&fixture.root.join("release"), b"release", 0o600);
    let output = wait_deadline(child);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let fixture = Fixture::new();
    write_mode(&fixture.root.join("wait"), b"wait", 0o600);
    let child = fixture.command(true).spawn().unwrap();
    wait_for(&fixture.root.join("ready"));
    let replacement = fixture.root.join("adapter-replacement");
    fs::copy(&fixture.adapter, &replacement).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o755)).unwrap();
    fs::rename(&replacement, &fixture.adapter).unwrap();
    write_mode(&fixture.root.join("release"), b"release", 0o600);
    let output = wait_deadline(child);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(start.elapsed() < Duration::from_secs(5), "timeout {path:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_deadline(mut child: Child) -> Output {
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            panic!("probe timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn installed_capabilities_is_positive_without_daemon() {
    let fixture = Fixture::new();
    fixture.assert_positive(&fixture.run(true));
}

#[test]
fn missing_descriptor_and_wrong_schema_are_rejected_without_spawn() {
    let fixture = Fixture::new();
    fixture.write_host(fixture.host_without_installation());
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let mut host = fixture.host_value();
    host["schema"] = json!("gate-runtime-host/v2");
    fixture.write_host(host);
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let mut host = fixture.host_value();
    host["installation"]["schema"] = json!("gate-runtime-installation/v2");
    fixture.write_host(host);
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn unknown_and_duplicate_host_fields_are_rejected() {
    let fixture = Fixture::new();
    let mut host = fixture.host_value();
    host["unknown"] = json!(true);
    fixture.write_host(host);
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let normal = serde_json::to_string(&fixture.host_value()).unwrap();
    let duplicate = format!("{}, \"timeout_ms\":5000", normal.trim_end_matches('}')) + "}";
    write_mode(&fixture.host, duplicate.as_bytes(), 0o600);
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn descriptor_digest_path_version_and_expected_conflicts_are_zero_spawn() {
    let fixture = Fixture::new();
    let mut changed = fixture.descriptor_bytes.clone();
    changed[0] = b' ';
    write_mode(&fixture.descriptor, &changed, 0o600);
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let mut value: Value = serde_json::from_slice(&fixture.descriptor_bytes).unwrap();
    value["executable"]["path"] = json!("/tmp/other-adapter");
    write_mode(
        &fixture.descriptor,
        &serde_json::to_vec(&value).unwrap(),
        0o600,
    );
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let mut value: Value = serde_json::from_slice(&fixture.descriptor_bytes).unwrap();
    value["executable"]["version"] = json!("0.0.1");
    write_mode(
        &fixture.descriptor,
        &serde_json::to_vec(&value).unwrap(),
        0o600,
    );
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let mut expected: Value = serde_json::from_slice(&fixture.expected).unwrap();
    expected["binding"]["adapter_version"] = json!("0.0.1");
    let expected_bytes = serde_json::to_vec(&expected).unwrap();
    write_mode(&fixture.expected_path, &expected_bytes, 0o600);
    let mut host = fixture.host_value();
    host["expected"]["sha256"] = json!(digest(&expected_bytes));
    fixture.write_host(host);
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn descriptor_symlink_hardlink_and_mode_are_rejected_without_spawn() {
    let fixture = Fixture::new();
    let link = fixture.root.join("descriptor-link");
    symlink(&fixture.descriptor, &link).unwrap();
    let mut host = fixture.host_value();
    host["installation"]["path"] = json!(link);
    fixture.write_host(host);
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    let hard = fixture.root.join("descriptor-hard");
    fs::hard_link(&fixture.descriptor, &hard).unwrap();
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    fs::set_permissions(&fixture.descriptor, fs::Permissions::from_mode(0o644)).unwrap();
    fixture.assert_rejected(&fixture.run(true));
}

#[test]
fn descriptor_replacement_after_spawn_fails() {
    let fixture = Fixture::new();
    write_mode(&fixture.root.join("wait"), b"wait", 0o600);
    let child = fixture.command(true).spawn().unwrap();
    wait_for(&fixture.root.join("ready"));
    let mut changed = fixture.descriptor_bytes.clone();
    changed[0] = b' ';
    write_mode(&fixture.descriptor, &changed, 0o600);
    write_mode(&fixture.root.join("release"), b"release", 0o600);
    let output = wait_deadline(child);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn v2_and_installed_entries_refuse_each_other_without_spawn() {
    let fixture = Fixture::new();
    let mut v2 = fixture.host_value();
    v2.as_object_mut().unwrap().remove("installation");
    v2["schema"] = json!("gate-runtime-host/v2");
    fixture.write_host(v2);
    fixture.assert_rejected(&fixture.run(true));

    let fixture = Fixture::new();
    fixture.assert_rejected(&fixture.run(false));
}

#[test]
fn published_0_1_1_capabilities_if_supplied() {
    let Some(path) = std::env::var_os("GATEHOUSE_PUBLISHED_RUNTIME") else {
        return;
    };
    let path = PathBuf::from(path);
    let (guard, root) = protected_root();
    let socket = root.join("docker.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let adapter = root.join("published-adapter");
    fs::copy(&path, &adapter).unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let version = "0.1.1".to_string();
    let fixture = Fixture::with_adapter(guard, root, adapter, version, Some(listener));
    let output = fixture.run(true);
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["operation"], json!("capabilities"));
    assert_eq!(
        value["outcome"],
        json!({"capabilities": {"supported": ["inspect"]}})
    );
}
