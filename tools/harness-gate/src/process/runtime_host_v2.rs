//! Staged Linux read-only host. All trusted fields originate from its caller,
//! not a public document/CLI. Single-file pins do not bind dynamic libraries,
//! interpreter closures or prove daemon identity. Path execution retains its
//! existing race boundary; there is no sandbox or installed-artifact authority.
use super::{
    adapter::VerifiedExecutable, fd_inputs::SealedInputs, raw_adapter, runtime_config, HostPolicy,
};
use gatehouse_runtime_protocol::{v2, Capability, Operation, Outcome};
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, &'static str>;

#[allow(dead_code)]
pub(super) struct ReadonlyHostInput {
    pub expected: v2::Request,
    pub config: Vec<u8>,
    pub actual: Vec<u8>,
    pub executable: PathBuf,
    pub executable_sha256: String,
    /// Independently trusted metadata, not a version queried from the binary.
    pub executable_version: String,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
}

#[allow(dead_code)]
pub(super) fn run_readonly(
    input: ReadonlyHostInput,
    cancelled: impl Fn() -> bool,
) -> Result<v2::Response> {
    input
        .expected
        .validate()
        .map_err(|_| "invalid runtime request")?;
    if matches!(input.expected.operation, Operation::Remove { .. }) {
        return Err("runtime mutation refused");
    }
    if !(1..=30_000).contains(&input.timeout_ms)
        || input.actual.len() > 65_536
        || input.config.len() > 65_536
    {
        return Err("invalid runtime bounds");
    }
    let actual = v2::parse_request(&input.actual).map_err(|_| "invalid runtime actual request")?;
    if actual != input.expected {
        return Err("runtime request mismatch");
    }
    runtime_config::validate_config_binding(&input.config, &input.expected)?;
    if input.executable_sha256 != input.expected.binding.adapter_sha256
        || input.executable_version != input.expected.binding.adapter_version
        || !input.executable.is_absolute()
    {
        return Err("runtime executable pin mismatch");
    }
    let executable = VerifiedExecutable::open(&input.executable, &input.executable_sha256)
        .map_err(|_| "runtime executable verification failed")?;
    executable
        .require_elf()
        .map_err(|_| "runtime ELF verification failed")?;
    if !input.cwd.is_absolute()
        || !input.cwd.is_dir()
        || std::fs::canonicalize(&input.cwd).map_err(|_| "invalid runtime cwd")? != input.cwd
    {
        return Err("invalid runtime cwd");
    }
    let expected_bytes =
        serde_json::to_vec(&input.expected).map_err(|_| "runtime request encoding failed")?;
    if expected_bytes.len() > 65_536 {
        return Err("invalid runtime bounds");
    }
    if cancelled() {
        return Err("runtime cancelled");
    }
    let inputs =
        SealedInputs::new(&input.config, &expected_bytes).map_err(|_| "runtime inputs failed")?;
    let mut command = Command::new(&input.executable);
    command.current_dir(&input.cwd).env_clear();
    let policy = HostPolicy {
        max_stdout_bytes: 65_536,
        max_stderr_bytes: 65_536,
        max_output_bytes: 65_536,
        reader_deadline: Duration::from_secs(2),
        ..HostPolicy::default()
    };
    executable
        .revalidate()
        .map_err(|_| "runtime executable changed")?;
    let output = raw_adapter::run_with_inputs(
        command,
        inputs,
        &policy,
        input.timeout_ms,
        input.actual,
        Instant::now(),
        &cancelled,
    )
    .map_err(|_| "runtime transport failed")?;
    raw_adapter::validate_process_output(&output, &policy)
        .map_err(|_| "runtime output rejected")?;
    if !output.status.success()
        || output.timed_out
        || output.was_cancelled
        || output.request_write_error.is_some()
    {
        return Err("runtime transport incomplete");
    }
    let response = v2::parse_response(&output.stdout.bytes, &input.expected)
        .map_err(|_| "invalid runtime response")?;
    finish(response, &executable, &cancelled)
}

// Separate continuation permits a deterministic test of the post-parse boundary.
fn finish(
    response: v2::Response,
    executable: &VerifiedExecutable,
    cancelled: &impl Fn() -> bool,
) -> Result<v2::Response> {
    if cancelled() {
        return Err("runtime cancelled");
    }
    executable
        .revalidate()
        .map_err(|_| "runtime executable changed")?;
    match (&response.operation, &response.outcome) {
        (Operation::Capabilities, Outcome::Capabilities { supported })
            if supported == &[Capability::Inspect] => {}
        (Operation::Inspect { .. }, Outcome::Present { .. } | Outcome::Unavailable) => {}
        _ => return Err("runtime outcome refused"),
    }
    if cancelled() {
        return Err("runtime cancelled");
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gatehouse_runtime_protocol::v2::{
        Binding, Engine, Observations, ObservedIdentity, Protocol, Response, Transport,
    };
    use gatehouse_runtime_protocol::{Capability, Label, Object};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    const HELPER: &str = r##"
#define _GNU_SOURCE
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <sys/types.h>
static int read_all(int fd, char *buf, size_t cap, size_t *len) {
    size_t off = 0; ssize_t n;
    while (off < cap && (n = pread(fd, buf + off, cap - off, off)) > 0) off += (size_t)n;
    if (n < 0) return -1;
    *len = off;
    return 0;
}
static int read_stream(int fd, char *buf, size_t cap, size_t *len) {
    size_t off = 0; ssize_t n;
    while (off < cap && (n = read(fd, buf + off, cap - off)) > 0) off += (size_t)n;
    if (n < 0) return -1;
    *len = off;
    return 0;
}
static int read_file(const char *name, char *buf, size_t cap, size_t *len) {
    int fd = open(name, O_RDONLY); if (fd < 0) return -1;
    size_t off = 0; ssize_t n;
    while (off < cap && (n = read(fd, buf + off, cap - off)) > 0) off += (size_t)n;
    close(fd); if (n < 0) return -1; *len = off; return 0;
}
static void write_file(const char *name, const char *data, size_t len) {
    int fd = open(name, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (fd < 0) _exit(10);
    if (write(fd, data, len) != (ssize_t)len) _exit(11);
    close(fd);
}
int main(int argc, char **argv) {
    (void)argv;
    char mode[32] = {0}; size_t mlen = 0;
    if (read_file("mode", mode, sizeof(mode) - 1, &mlen) < 0) return 2;
    char cwd[4096]; if (!getcwd(cwd, sizeof(cwd))) return 3;
    extern char **environ; int env = 0;
    while (environ && environ[env]) env++;
    char meta[8192]; int n = snprintf(meta, sizeof(meta), "%d %d %s\n", argc, env, cwd);
    write_file("meta", meta, (size_t)n);
    if (strcmp(mode, "v1") == 0) {
        char response[131072]; size_t rlen = 0;
        if (read_file("response", response, sizeof(response), &rlen) < 0) return 4;
        if (write(1, response, rlen) != (ssize_t)rlen) return 5;
        return 0;
    }
    char observed[131072]; size_t len = 0;
    if (read_all(3, observed, sizeof(observed), &len) < 0) return 6;
    write_file("fd3.obs", observed, len);
    if (read_all(4, observed, sizeof(observed), &len) < 0) return 7;
    write_file("fd4.obs", observed, len);
    if (read_stream(0, observed, sizeof(observed), &len) < 0) return 8;
    write_file("stdin.obs", observed, len);
    if (strcmp(mode, "revalidate") == 0) {
        write_file("ready", "1", 1);
        for (;;) {
            if (access("release", F_OK) == 0) break;
            usleep(10000);
        }
    }
    if (strcmp(mode, "tree") == 0) {
        pid_t child = fork(); if (child < 0) return 9;
        if (child == 0) {
            for (int i = 0; i < 3000; i++) {
                if (access("probe", F_OK) == 0) { write_file("survived", "1", 1); _exit(0); }
                usleep(10000);
            }
            _exit(0);
        }
        write_file("ready", "1", 1);
        sleep(30);
        return 0;
    }
    if (strcmp(mode, "sleep") == 0) { sleep(30); return 0; }
    char response[131072]; size_t rlen = 0;
    if (read_file("response", response, sizeof(response), &rlen) < 0) return 4;
    if (write(1, response, rlen) != (ssize_t)rlen) return 5;
    return 0;
}
"##;

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
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

    fn compile(dir: &Path) -> (PathBuf, String) {
        let source = dir.join("helper.c");
        let helper = dir.join("helper");
        fs::write(&source, HELPER).unwrap();
        let output = Command::new("cc")
            .args(["-O0", "-std=c99", "-o"])
            .arg(&helper)
            .arg(&source)
            .output()
            .expect("cc available");
        assert!(
            output.status.success(),
            "helper compile failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let digest = digest(&fs::read(&helper).unwrap());
        (helper, digest)
    }

    struct Fixture {
        dir: TempDir,
        helper: PathBuf,
        helper_digest: String,
        expected: v2::Request,
        config: Vec<u8>,
    }

    impl Fixture {
        fn new(operation: Operation) -> Self {
            let dir = TempDir::new().unwrap();
            let (helper, helper_digest) = compile(dir.path());
            let config = config_bytes();
            let binding = Binding {
                protocol: Protocol::V2,
                adapter_sha256: helper_digest.clone(),
                adapter_version: "helper-1".into(),
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
            Self {
                dir,
                helper,
                helper_digest,
                expected: v2::Request { binding, operation },
                config,
            }
        }

        fn cwd(&self) -> PathBuf {
            fs::canonicalize(self.dir.path()).unwrap()
        }

        fn write(&self, name: &str, bytes: &[u8]) {
            fs::write(self.dir.path().join(name), bytes).unwrap();
        }

        fn input(&self, actual: Vec<u8>, timeout_ms: u64) -> ReadonlyHostInput {
            ReadonlyHostInput {
                expected: self.expected.clone(),
                config: self.config.clone(),
                actual,
                executable: self.helper.clone(),
                executable_sha256: self.helper_digest.clone(),
                executable_version: "helper-1".into(),
                cwd: self.cwd(),
                timeout_ms,
            }
        }

        fn actual(&self) -> Vec<u8> {
            serde_json::to_vec(&self.expected).unwrap()
        }

        fn response(&self, outcome: Outcome, observations: Option<Observations>) -> Vec<u8> {
            serde_json::to_vec(&Response {
                binding: self.expected.binding.clone(),
                operation: self.expected.operation.clone(),
                outcome,
                observations,
            })
            .unwrap()
        }

        fn observations(&self) -> Observations {
            let identity = ObservedIdentity {
                api_version: self.expected.binding.api_version.clone(),
                server_version: self.expected.binding.server_version.clone(),
                daemon_id: self.expected.binding.daemon_id.clone(),
            };
            Observations {
                observed_before: identity.clone(),
                observed_after: identity,
            }
        }

        fn object_id(&self) -> String {
            match &self.expected.operation {
                Operation::Inspect { object_id } => object_id.clone(),
                _ => "a".repeat(64),
            }
        }

        fn present(&self) -> Outcome {
            Outcome::Present {
                object: Object {
                    object_id: self.object_id(),
                    labels: vec![Label {
                        key: "harness-gate.owner".into(),
                        value: "harness-gate".into(),
                    }],
                },
            }
        }

        fn run(
            &self,
            actual: Vec<u8>,
            timeout_ms: u64,
        ) -> std::result::Result<v2::Response, &'static str> {
            self.write("mode", b"respond");
            run_readonly(self.input(actual, timeout_ms), || false)
        }
    }

    fn capabilities() -> Operation {
        Operation::Capabilities
    }

    fn inspect() -> Operation {
        Operation::Inspect {
            object_id: "b".repeat(64),
        }
    }

    fn meta(fixture: &Fixture) -> (i32, i32, String) {
        let text = fs::read_to_string(fixture.dir.path().join("meta")).unwrap();
        let mut parts = text.splitn(3, ' ');
        let argc = parts.next().unwrap().parse().unwrap();
        let env = parts.next().unwrap().parse().unwrap();
        let cwd = parts.next().unwrap().trim_end().to_string();
        (argc, env, cwd)
    }

    #[test]
    fn h01_capabilities_compiled_elf_exact_fd_stdin_env_cwd() {
        let fixture = Fixture::new(capabilities());
        fixture.write(
            "response",
            &fixture.response(
                Outcome::Capabilities {
                    supported: vec![Capability::Inspect],
                },
                None,
            ),
        );
        let actual = fixture.actual();
        let expected_bytes = serde_json::to_vec(&fixture.expected).unwrap();
        let response = fixture.run(actual.clone(), 2_000).unwrap();
        assert_eq!(
            response.outcome,
            Outcome::Capabilities {
                supported: vec![Capability::Inspect]
            }
        );
        assert_eq!(
            fs::read(fixture.dir.path().join("fd3.obs")).unwrap(),
            fixture.config
        );
        assert_eq!(
            fs::read(fixture.dir.path().join("fd4.obs")).unwrap(),
            expected_bytes
        );
        let mut stdin_expected = actual;
        stdin_expected.push(b'\n');
        assert_eq!(
            fs::read(fixture.dir.path().join("stdin.obs")).unwrap(),
            stdin_expected
        );
        let (argc, env, cwd) = meta(&fixture);
        assert_eq!(argc, 1);
        assert_eq!(env, 0);
        assert_eq!(cwd, fixture.cwd().to_string_lossy());
    }

    #[test]
    fn h02_inspect_present_is_accepted() {
        let fixture = Fixture::new(inspect());
        let response = fixture.response(fixture.present(), Some(fixture.observations()));
        fixture.write("response", &response);
        let output = fixture.run(fixture.actual(), 2_000).unwrap();
        assert!(matches!(output.outcome, Outcome::Present { .. }));
    }

    #[test]
    fn h03_inspect_unavailable_is_accepted() {
        let fixture = Fixture::new(inspect());
        fixture.write("response", &fixture.response(Outcome::Unavailable, None));
        let output = fixture.run(fixture.actual(), 2_000).unwrap();
        assert_eq!(output.outcome, Outcome::Unavailable);
    }

    #[test]
    fn h04_absent_is_refused() {
        let fixture = Fixture::new(inspect());
        let response = fixture.response(Outcome::Absent, Some(fixture.observations()));
        fixture.write("response", &response);
        assert_eq!(
            fixture.run(fixture.actual(), 2_000),
            Err("runtime outcome refused")
        );
    }

    #[test]
    fn h05_capabilities_remove_is_refused() {
        let fixture = Fixture::new(capabilities());
        fixture.write(
            "response",
            &fixture.response(
                Outcome::Capabilities {
                    supported: vec![Capability::Inspect, Capability::Remove],
                },
                None,
            ),
        );
        assert_eq!(
            fixture.run(fixture.actual(), 2_000),
            Err("runtime outcome refused")
        );
    }

    #[test]
    fn h06_remove_is_refused_before_spawn() {
        let fixture = Fixture::new(Operation::Remove {
            object_id: "c".repeat(64),
        });
        assert_eq!(
            fixture.run(fixture.actual(), 2_000),
            Err("runtime mutation refused")
        );
        assert!(!fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h07_actual_typed_mismatch_is_refused_before_spawn() {
        let fixture = Fixture::new(capabilities());
        let mut actual = fixture.expected.clone();
        actual.binding.operation_key = "different".into();
        assert_eq!(
            fixture.run(serde_json::to_vec(&actual).unwrap(), 2_000),
            Err("runtime request mismatch")
        );
        assert!(!fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h08_config_pin_mismatch_is_refused_before_spawn() {
        let fixture = Fixture::new(capabilities());
        let mut config: Value = serde_json::from_slice(&fixture.config).unwrap();
        config["scope"] = json!("scope-2");
        fixture.write(
            "response",
            &fixture.response(
                Outcome::Capabilities {
                    supported: vec![Capability::Inspect],
                },
                None,
            ),
        );
        let mut input = fixture.input(fixture.actual(), 2_000);
        input.config = serde_json::to_vec(&config).unwrap();
        assert_eq!(
            run_readonly(input, || false),
            Err("runtime config binding mismatch")
        );
        assert!(!fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h09_executable_pin_mismatch_is_refused_before_spawn() {
        let fixture = Fixture::new(capabilities());
        let mut input = fixture.input(fixture.actual(), 2_000);
        input.executable_sha256 = "b".repeat(64);
        assert_eq!(
            run_readonly(input, || false),
            Err("runtime executable pin mismatch")
        );
        assert!(!fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h10_script_is_not_an_elf() {
        let fixture = Fixture::new(capabilities());
        let script = fixture.dir.path().join("helper.sh");
        fs::write(&script, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let script_digest = digest(&fs::read(&script).unwrap());
        let mut input = fixture.input(fixture.actual(), 2_000);
        input.executable = script;
        input.executable_sha256 = script_digest.clone();
        input.expected.binding.adapter_sha256 = script_digest;
        input.actual = serde_json::to_vec(&input.expected).unwrap();
        assert_eq!(
            run_readonly(input, || false),
            Err("runtime ELF verification failed")
        );
        assert!(!fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h11_noncanonical_cwd_is_refused_before_spawn() {
        let fixture = Fixture::new(capabilities());
        let link = fixture.dir.path().join("cwd-link");
        symlink(fixture.cwd(), &link).unwrap();
        let mut input = fixture.input(fixture.actual(), 2_000);
        input.cwd = link;
        assert_eq!(run_readonly(input, || false), Err("invalid runtime cwd"));
        assert!(!fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h12_timeout_bounds_and_fast_max_timeout() {
        let fixture = Fixture::new(capabilities());
        fixture.write(
            "response",
            &fixture.response(
                Outcome::Capabilities {
                    supported: vec![Capability::Inspect],
                },
                None,
            ),
        );
        for timeout in [0, 30_001] {
            let input = fixture.input(fixture.actual(), timeout);
            assert_eq!(run_readonly(input, || false), Err("invalid runtime bounds"));
            assert!(!fixture.dir.path().join("meta").exists());
        }
        assert!(fixture.run(fixture.actual(), 30_000).is_ok());
    }

    #[test]
    fn h13_output_over_limit_is_refused() {
        let fixture = Fixture::new(capabilities());
        fixture.write("response", &vec![b'x'; 70_000]);
        assert_eq!(
            fixture.run(fixture.actual(), 2_000),
            Err("runtime output rejected")
        );
        assert!(fixture.dir.path().join("meta").exists());
    }

    #[test]
    fn h14_cancellation_before_parse_and_after_parse_continuation() {
        let fixture = Fixture::new(capabilities());
        let input = fixture.input(fixture.actual(), 2_000);
        assert_eq!(run_readonly(input, || true), Err("runtime cancelled"));
        assert!(!fixture.dir.path().join("meta").exists());

        let executable = VerifiedExecutable::open(&fixture.helper, &fixture.helper_digest).unwrap();
        let response = fixture.response(
            Outcome::Capabilities {
                supported: vec![Capability::Inspect],
            },
            None,
        );
        let parsed = v2::parse_response(&response, &fixture.expected).unwrap();
        assert_eq!(
            finish(parsed, &executable, &|| true),
            Err("runtime cancelled")
        );
    }

    fn assert_tree_stopped(fixture: &Fixture) {
        assert!(fixture.dir.path().join("ready").exists());
        fixture.write("probe", b"probe");
        thread::sleep(Duration::from_secs(2));
        assert!(!fixture.dir.path().join("survived").exists());
    }

    #[test]
    fn h15_timeout_and_cancel_terminate_descendant() {
        let fixture = Fixture::new(capabilities());
        fixture.write("mode", b"tree");
        assert!(run_readonly(fixture.input(fixture.actual(), 500), || false).is_err());
        assert_tree_stopped(&fixture);

        let fixture = Fixture::new(capabilities());
        fixture.write("mode", b"tree");
        let ready = fixture.dir.path().join("ready");
        let flag = Arc::new(AtomicBool::new(false));
        let thread_flag = Arc::clone(&flag);
        let watcher = thread::spawn(move || {
            for _ in 0..400 {
                if ready.exists() {
                    thread_flag.store(true, Ordering::SeqCst);
                    return;
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        assert!(
            run_readonly(fixture.input(fixture.actual(), 10_000), || flag
                .load(Ordering::SeqCst))
            .is_err()
        );
        watcher.join().unwrap();
        assert_tree_stopped(&fixture);
    }

    #[test]
    fn h16_errors_do_not_echo_sentinel() {
        let fixture = Fixture::new(capabilities());
        let mut config: Value = serde_json::from_slice(&fixture.config).unwrap();
        config["sentinel"] = json!("SENTINEL-HOST-DO-NOT-ECHO");
        let mut input = fixture.input(fixture.actual(), 2_000);
        input.config = serde_json::to_vec(&config).unwrap();
        let error = run_readonly(input, || false).unwrap_err();
        assert_eq!(error, "invalid runtime config");
        assert!(!error.contains("SENTINEL"));
        assert!(!error.contains("/run/docker.sock"));
    }

    fn valid_parsed_response(fixture: &Fixture) -> v2::Response {
        let response = fixture.response(
            Outcome::Capabilities {
                supported: vec![Capability::Inspect],
            },
            None,
        );
        v2::parse_response(&response, &fixture.expected).unwrap()
    }

    #[test]
    fn h18_continuation_detects_replaced_executable_path() {
        let fixture = Fixture::new(capabilities());
        let executable = VerifiedExecutable::open(&fixture.helper, &fixture.helper_digest).unwrap();
        let replacement = fixture.dir.path().join("replacement");
        fs::write(&replacement, b"replacement bytes").unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o755)).unwrap();
        fs::rename(&fixture.helper, fixture.dir.path().join("original")).unwrap();
        fs::rename(&replacement, &fixture.helper).unwrap();
        assert_eq!(
            finish(valid_parsed_response(&fixture), &executable, &|| false),
            Err("runtime executable changed")
        );
    }

    #[test]
    fn h19_in_flight_executable_replacement_fails_before_response_acceptance() {
        let fixture = Fixture::new(capabilities());
        fixture.write("mode", b"revalidate");
        fixture.write(
            "response",
            &fixture.response(
                Outcome::Capabilities {
                    supported: vec![Capability::Inspect],
                },
                None,
            ),
        );
        let helper = fixture.helper.clone();
        let original = fixture.dir.path().join("original");
        let release = fixture.dir.path().join("release");
        let ready = fixture.dir.path().join("ready");
        let watcher = thread::spawn(move || {
            for _ in 0..1000 {
                if ready.exists() {
                    fs::rename(&helper, &original).unwrap();
                    fs::write(&helper, b"replacement bytes").unwrap();
                    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
                    fs::write(&release, b"release").unwrap();
                    return;
                }
                thread::sleep(Duration::from_millis(5));
            }
            panic!("helper did not reach ready");
        });
        let result = run_readonly(fixture.input(fixture.actual(), 10_000), || false);
        watcher.join().unwrap();
        assert_eq!(result, Err("runtime executable changed"));
    }

    #[test]
    fn h20_final_cancellation_checkpoint_is_counted_and_discards_response() {
        let fixture = Fixture::new(capabilities());
        let executable = VerifiedExecutable::open(&fixture.helper, &fixture.helper_digest).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let result = finish(valid_parsed_response(&fixture), &executable, &move || {
            counter.fetch_add(1, Ordering::SeqCst) >= 1
        });
        assert_eq!(result, Err("runtime cancelled"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn h17_v1_raw_adapter_path_is_unchanged() {
        let fixture = Fixture::new(capabilities());
        fixture.write("mode", b"v1");
        fixture.write("response", b"v1-ok");
        let policy = HostPolicy {
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            max_output_bytes: 2048,
            reader_deadline: Duration::from_secs(2),
            ..HostPolicy::default()
        };
        let mut command = Command::new(&fixture.helper);
        command.current_dir(fixture.cwd());
        let output = raw_adapter::run(
            &mut command,
            &policy,
            2_000,
            b"ignored".to_vec(),
            Instant::now(),
            &|| false,
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.bytes, b"v1-ok");
    }
}
