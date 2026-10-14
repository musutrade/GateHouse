#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const FLOW: &str = r#"
version = 2
[project]
name = "non-rust-metadata"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "fail"
rules = [{ patterns = ["**"], components = ["project"] }]
[[steps]]
id = "project.check"
label = "check"
component = "project"
profiles = ["full"]
program = "check"
args = []
cwd = "{root}"
log = "check.log"
timeout_secs = 1
"#;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
    command
        .args(["--color", "never", "--project-root"])
        .arg(root);
    command
}

fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}

fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    success(
        command(root.path())
            .args(["init", "--preset", "generic"])
            .output()
            .unwrap(),
    );
    fs::write(root.path().join(".harness-gate/flow.toml"), FLOW).unwrap();
    fs::create_dir(root.path().join("bin")).unwrap();
    script(root.path(), "check", "printf executed > executed");
    for args in [
        vec!["init", "--quiet"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Metadata Test",
            "-c",
            "user.email=metadata@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    ] {
        success(
            Command::new(real_git())
                .args(args)
                .current_dir(root.path())
                .output()
                .unwrap(),
        );
    }
    root
}

fn real_git() -> std::path::PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .unwrap()
}

fn git_script(root: &Path, body: &str) {
    let git = real_git();
    let quoted = git.to_str().unwrap().replace('\'', "'\\''");
    script(root, "git", &format!("if [ \"$1\" = rev-parse ] && [ \"$2\" = --verify ] && [ \"$3\" = 'HEAD^{{commit}}' ]; then\n{body}\nelse\nexec '{quoted}' \"$@\"\nfi"));
}

fn script(root: &Path, name: &str, body: &str) {
    let path = root.join("bin").join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

// Emergency cleanup targets the still-owned direct child group before reap.
struct Watchdog(Option<Child>);

impl Watchdog {
    fn start(root: &Path, args: &[&str]) -> Self {
        Self(Some(
            command(root)
                .args(args)
                .env("PATH", root.join("bin"))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0)
                .spawn()
                .unwrap(),
        ))
    }

    fn finish(mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(8);
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "verify exceeded external watchdog"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
            }
        }
    }
}

fn metadata(root: &Path) -> Value {
    let result: Value = serde_json::from_slice(
        &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["passed"], true);
    assert_eq!(result["evidence_complete"], true);
    let id = result["invocation_id"].as_str().unwrap();
    let metadata: Value = serde_json::from_slice(
        &fs::read(
            root.join(".harness-gate/reports/invocations")
                .join(id)
                .join("invocation.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["metadata_version"], 2);
    assert_eq!(metadata["toolchain"], "unknown");
    assert_eq!(metadata["executor_version"], env!("CARGO_PKG_VERSION"));
    assert!(result["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| step["step_id"] == "project.check" && step["passed"] == true));
    metadata
}

#[test]
fn generic_verify_never_probes_rustc_and_samples_commit_once() {
    for rustc in [
        None,
        Some("printf called > rustc-called; exec /bin/sleep 30"),
        Some("printf called > rustc-called; exit 7"),
    ] {
        let root = fixture();
        let root = root.path();
        if let Some(body) = rustc {
            script(root, "rustc", body);
        }
        let sha = "a".repeat(40);
        git_script(
            root,
            &format!("printf call >> git-calls; printf '{sha}\\n'"),
        );
        success(Watchdog::start(root, &["verify", "--all"]).finish());
        assert!(!root.join("rustc-called").exists());
        assert_eq!(fs::read_to_string(root.join("git-calls")).unwrap(), "call");
        let data = metadata(root);
        assert_eq!(data["commit"], sha);
        assert_eq!(data["source_identity"], format!("working-tree:{sha}"));
    }
}

#[test]
fn optional_commit_failures_are_unknown_without_masking_required_steps() {
    for body in [
        "exit 127",
        "exit 7",
        "exit 0",
        "printf 'not-a-commit\\n'",
        "printf '\\377'",
        "printf '%s\\n' aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; printf '%4096s' ''",
        "printf '%s\\n' aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; printf '%4097s' '' >&2",
    ] {
        let root = fixture();
        let root = root.path();
        git_script(root, body);
        success(Watchdog::start(root, &["verify", "--all"]).finish());
        assert_eq!(metadata(root)["commit"], "unknown", "{body:?}");
        assert_eq!(metadata(root)["source_identity"], "working-tree:unknown");
    }
    let root = fixture();
    let root = root.path();
    git_script(root, "exit 7");
    script(root, "check", "exit 9");
    let output = Watchdog::start(root, &["verify", "--all"]).finish();
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(
        &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["passed"], false);
}

fn hanging_git(root: &Path) {
    git_script(root, "echo $$ > probe.pid\n/bin/sleep 30 &\necho $! > descendant.pid\ntrap 'wait; exit 143' TERM\nprintf ready > probe-ready\nwait");
}

fn assert_probe_stopped(root: &Path) {
    for name in ["probe.pid", "descendant.pid"] {
        let pid: i32 = fs::read_to_string(root.join(name))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "{name} remains alive");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}

#[test]
fn optional_git_timeout_reaps_probe_tree_and_continues_verification() {
    let root = fixture();
    let root = root.path();
    hanging_git(root);
    success(Watchdog::start(root, &["verify", "--all"]).finish());
    assert_eq!(metadata(root)["commit"], "unknown");
    assert_probe_stopped(root);
}

#[test]
fn cancellation_during_optional_git_probe_is_blocking_and_reaps_the_tree() {
    let root = fixture();
    let root = root.path();
    hanging_git(root);
    let child = Watchdog::start(root, &["verify", "--all"]);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.join("probe-ready").exists() {
        assert!(Instant::now() < deadline, "probe did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        unsafe { libc::kill(child.0.as_ref().unwrap().id() as i32, libc::SIGTERM) },
        0
    );
    let output = child.finish();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Git metadata probe was cancelled"));
    assert!(!root.join("executed").exists());
    assert!(!root.join(".harness-gate/reports/test_result.json").exists());
    assert_probe_stopped(root);
}

#[test]
fn failed_required_git_snapshot_is_not_optional_metadata() {
    let root = fixture();
    let root = root.path();
    script(root, "git", "exit 7");
    let output = Watchdog::start(root, &["verify", "--staged"]).finish();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot materialize staged Git index"));
    assert!(!root.join("executed").exists());
}

#[test]
fn missing_git_records_unknown_context_but_blocks_required_secret_scan() {
    let root = fixture();
    let root = root.path();
    let output = Watchdog::start(root, &["verify", "--all"]).finish();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("ERROR [E1202]"));
    let result: Value = serde_json::from_slice(
        &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["passed"], false);
    let id = result["invocation_id"].as_str().unwrap();
    let data: Value = serde_json::from_slice(
        &fs::read(
            root.join(".harness-gate/reports/invocations")
                .join(id)
                .join("invocation.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(data["commit"], "unknown");
    assert_eq!(data["toolchain"], "unknown");
}

#[test]
fn staged_metadata_preserves_sampled_head_and_distinct_index_identity() {
    let root = fixture();
    let root = root.path();
    let git = real_git();
    let head = success(
        Command::new(&git)
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap(),
    );
    fs::write(root.join("source.txt"), "indexed change\n").unwrap();
    success(
        Command::new(&git)
            .args(["add", "source.txt"])
            .current_dir(root)
            .output()
            .unwrap(),
    );
    let tree = success(
        Command::new(&git)
            .arg("write-tree")
            .current_dir(root)
            .output()
            .unwrap(),
    );
    let git_quoted = git.to_str().unwrap().replace('\'', "'\\''");
    script(
        root,
        "git",
        &format!("printf '%s\\n' \"$*\" >> git-calls\nexec '{git_quoted}' \"$@\""),
    );
    script(
        root,
        "rustc",
        "printf called > rustc-called; exec /bin/sleep 30",
    );
    success(Watchdog::start(root, &["verify", "--staged"]).finish());
    let data = metadata(root);
    assert_eq!(
        data["commit"],
        String::from_utf8(head.stdout).unwrap().trim()
    );
    assert_eq!(
        data["source_identity"],
        format!(
            "git-tree:{}",
            String::from_utf8(tree.stdout).unwrap().trim()
        )
    );
    let calls = fs::read_to_string(root.join("git-calls")).unwrap();
    assert_eq!(
        calls
            .lines()
            .filter(|line| *line == "rev-parse --verify HEAD^{commit}")
            .count(),
        1
    );
    assert!(!root.join("rustc-called").exists());
}
