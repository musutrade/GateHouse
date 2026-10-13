#![cfg(unix)]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

#[test]
#[cfg(target_os = "linux")]
fn arc_admin_required_commands_fail_through_generic_hooks() {
    let output = TempDir::new().unwrap();
    success(
        Command::new("python3")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../docs/dogfood/arc-admin/negative/run.py"),
            )
            .args([
                "--harness-gate",
                env!("CARGO_BIN_EXE_harness-gate"),
                "--output",
            ])
            .arg(output.path())
            .output()
            .unwrap(),
    );
    let cases: Value =
        serde_json::from_slice(&fs::read(output.path().join("commands.json")).unwrap()).unwrap();
    assert_eq!(cases["cases"].as_array().unwrap().len(), 6);
}

const FLOW: &str = r#"
version = 2
[project]
name = "boundary"
default_profile = "full"
hook_profile = "hook"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
[[scope.rules]]
patterns = ["**"]
components = ["project"]
[[steps]]
id = "probe"
label = "probe"
component = "project"
profiles = ["full", "hook"]
program = "sh"
args = ["probe.sh", "original"]
cwd = "{root}"
log = "probe.log"
timeout_secs = 20
"#;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
    command.arg("--project-root").arg(root);
    command
        .env_remove("GH93_DOCTOR_ENV")
        .env_remove("GH93_SERVICE_ENV");
    command
}

fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn git(root: &Path, args: &[&str]) {
    success(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap(),
    );
}

fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    success(
        command(root.path())
            .args(["init", "--preset", "generic"])
            .output()
            .unwrap(),
    );
    git(root.path(), &["init"]);
    fs::write(root.path().join(".harness-gate/flow.toml"), FLOW).unwrap();
    fs::write(root.path().join("probe.sh"), "echo probe-ran\n").unwrap();
    root
}

fn report(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap())
        .unwrap()
}

fn assert_sealed_evidence(json: &Value) {
    assert_eq!(json["evidence_complete"], true);
    let directory = Path::new(json["report_directory"].as_str().unwrap());
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["invocation_id"], json["invocation_id"]);
    let artifacts = manifest["artifacts"].as_array().unwrap();
    assert!(!artifacts.is_empty());
    for artifact in artifacts {
        let path = artifact["path"].as_str().unwrap();
        let bytes = fs::read(directory.join(path)).unwrap();
        assert_eq!(artifact["size_bytes"], bytes.len(), "{path}");
        assert_eq!(
            artifact["sha256"],
            format!("{:x}", Sha256::digest(&bytes)),
            "{path}"
        );
    }
}

/// ADR-0049: an unknown application runner participates without a core registry,
/// framework parser, or quality collector. Renaming it must preserve blocking.
#[test]
fn project_owned_runner_replacement_preserves_generic_command_gate() {
    for runner in ["nebula-e2e", "quasar-api"] {
        for exit_code in [0, 7] {
            let root = fixture();
            let flow = FLOW
                .replace("id = \"probe\"", &format!("id = \"{runner}\""))
                .replace("program = \"sh\"", &format!("program = \"{runner}\""))
                .replace(
                    "args = [\"probe.sh\", \"original\"]",
                    "args = [\"project-assertion\"]",
                );
            fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
            let executable = root.path().join(runner);
            fs::write(
                &executable,
                format!("#!/bin/sh\n[ \"$1\" = project-assertion ] || exit 9\necho project-owned-result\nexit {exit_code}\n"),
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            success(
                command(root.path())
                    .args(["config", "check"])
                    .output()
                    .unwrap(),
            );
            let output = command(root.path())
                .env(
                    "PATH",
                    std::env::join_paths(std::iter::once(root.path().to_path_buf()).chain(
                        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
                    ))
                    .unwrap(),
                )
                .args(["verify", "--profile", "full", "--all"])
                .output()
                .unwrap();
            assert_eq!(output.status.success(), exit_code == 0, "{runner}");
            let json = report(root.path());
            assert_eq!(json["passed"], exit_code == 0);
            let step = json["steps"]
                .as_array()
                .unwrap()
                .iter()
                .find(|step| step["step_id"] == runner)
                .expect("unknown project runner must execute and retain its identity");
            assert_eq!(step["passed"], exit_code == 0);
            assert!(fs::read_to_string(step["log"].as_str().unwrap())
                .unwrap()
                .contains("project-owned-result"));
            assert_sealed_evidence(&json);
        }
    }
}

#[test]
fn doctor_kinds_execute_success_required_failure_and_optional_warning() {
    let cases = [
        (
            "command",
            "program = 'sh'\nargs = ['-c', 'echo ready']",
            "program = 'sh'\nargs = ['-c', 'echo broken >&2; exit 7']",
        ),
        (
            "path",
            "path = 'version.txt'\npath_type = 'file'",
            "path = 'version.txt'\npath_type = 'directory'",
        ),
        ("glob", "pattern = '*.txt'", "pattern = '*.absent'"),
        ("env", "name = 'GH93_PRESENT'", "name = 'GH93_DOCTOR_ENV'"),
        (
            "env-or-file",
            "env = 'GH93_DOCTOR_ENV'\npath = 'version.txt'\ncontains = '1.2.3'",
            "env = 'GH93_DOCTOR_ENV'\npath = 'version.txt'\ncontains = 'missing'",
        ),
        (
            "git-config",
            "key = 'boundary.value'\nexpected = 'ready'",
            "key = 'boundary.value'\nexpected = 'wrong'",
        ),
        ("git-remotes", "", ""),
        (
            "version",
            "program = 'sh'\nargs = ['-c', 'echo v1.2.3']\npath = 'version.txt'\ntrim_prefix = 'v'",
            "program = 'sh'\nargs = ['-c', 'echo v9']\npath = 'version.txt'\ntrim_prefix = 'v'",
        ),
        ("service", "service = 'available'", "service = 'missing'"),
    ];
    for (kind, pass, fail) in cases {
        let root = fixture();
        fs::write(root.path().join("version.txt"), "1.2.3\n").unwrap();
        git(root.path(), &["config", "boundary.value", "ready"]);
        git(
            root.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/repo.git",
            ],
        );
        for (fields, required, strict, expected_level, exit) in [
            (pass, true, false, "pass", 0),
            (fail, true, false, "fail", 1),
            (fail, false, false, "warn", 0),
            (fail, false, true, "warn", 1),
        ] {
            if kind == "git-remotes" && expected_level != "pass" {
                git(
                    root.path(),
                    &[
                        "remote",
                        "set-url",
                        "origin",
                        "https://fixture:dummy@example.invalid/repo.git",
                    ],
                );
            }
            let flow = format!("{FLOW}\n[services.available]\nkind = 'environment'\nsource_env = 'GH93_PRESENT'\ninject_env = 'AVAILABLE_URL'\n[services.missing]\nkind = 'environment'\nsource_env = 'GH93_SERVICE_ENV'\ninject_env = 'MISSING_URL'\n[[doctor.checks]]\nid = 'boundary'\nlabel = 'boundary'\nkind = '{kind}'\nrequired = {required}\nhelp = 'fixture remediation'\n{fields}\n");
            fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
            let mut cmd = command(root.path());
            cmd.env("GH93_PRESENT", "ready").args(["doctor", "--json"]);
            if strict {
                cmd.arg("--strict");
            }
            let output = cmd.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(exit),
                "{kind}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty(), "{kind}: unexpected stderr");
            let json: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["checks"][0]["level"], expected_level, "{kind}");
            assert_eq!(json["failures"], usize::from(expected_level == "fail"));
            assert_eq!(json["warnings"], usize::from(expected_level == "warn"));
            if expected_level != "pass" {
                assert!(json["checks"][0]["detail"]
                    .as_str()
                    .unwrap()
                    .contains("fixture remediation"));
            }
        }
    }
}

#[test]
fn cli_selection_profile_alias_and_discovery_boundaries() {
    let root = fixture();
    for args in [
        vec!["verify", "--components", "project"],
        vec!["check", "--all", "--profile", "hook"],
        vec!["step", "probe"],
    ] {
        success(command(root.path()).args(args).output().unwrap());
        assert_eq!(report(root.path())["passed"], true);
    }
    for (args, code) in [
        (vec!["verify", "--all", "--profile", "absent"], "E1401"),
        (vec!["step", "absent"], "E1401"),
        (vec!["verify", "--components", "absent"], "E1000"),
    ] {
        let output = command(root.path()).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
    let output = command(root.path())
        .args(["verify", "--all", "--staged"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used"));
    let nested = root.path().join("nested/deep");
    fs::create_dir_all(&nested).unwrap();
    success(
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .args(["config", "check"])
            .current_dir(nested)
            .output()
            .unwrap(),
    );
    let empty = TempDir::new().unwrap();
    let output = command(empty.path())
        .args(["config", "check"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("E1000"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn staged_content_controls_result_and_snapshots_are_cleaned_after_failure() {
    for (staged_exit, working_exit) in [(7, 0), (0, 7)] {
        let root = fixture();
        let temp = TempDir::new().unwrap();
        fs::write(
            root.path().join("probe.sh"),
            format!("echo staged-probe\nexit {staged_exit}\n"),
        )
        .unwrap();
        git(root.path(), &["add", "."]);
        fs::write(
            root.path().join("probe.sh"),
            format!("echo working-probe\nexit {working_exit}\n"),
        )
        .unwrap();
        let output = command(root.path())
            .env("TMPDIR", temp.path())
            .args(["verify", "--staged"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(i32::from(staged_exit != 0)),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json = report(root.path());
        assert_eq!(json["passed"], staged_exit == 0);
        assert_sealed_evidence(&json);
        assert!(!Path::new(json["execution_root"].as_str().unwrap()).exists());
        assert!(json["source_identity"]
            .as_str()
            .unwrap()
            .starts_with("git-tree:"));
        let step = json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["step_id"] == "probe")
            .unwrap();
        let log = fs::read_to_string(step["log"].as_str().unwrap()).unwrap();
        assert!(log.contains("staged-probe"));
        assert!(!log.contains("working-probe"));
        assert_eq!(
            fs::read_dir(temp.path()).unwrap().count(),
            0,
            "snapshot leaked"
        );
        let output = command(root.path())
            .args(["verify", "--all"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(i32::from(working_exit != 0)));
    }
    let root = fixture();
    let temp = TempDir::new().unwrap();
    fs::write(
        root.path().join(".harness-gate/flow.toml"),
        "invalid TOML = [",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    fs::write(root.path().join(".harness-gate/flow.toml"), FLOW).unwrap();
    let output = command(root.path())
        .env("TMPDIR", temp.path())
        .args(["verify", "--staged"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("E1000"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    assert!(!root
        .path()
        .join(".harness-gate/reports/test_result.json")
        .exists());
}

#[test]
fn configured_runner_service_isolation_and_shards_match_the_executed_process() {
    for isolation in ["shared", "schema-per-worker", "database-per-worker"] {
        for sharded in [false, true] {
            let root = fixture();
            let threads = if isolation == "shared" { 1 } else { 2 };
            let runner = format!("\n[steps.runner]\nversion = 1\nkind = 'custom'\nthreads = {threads}\nthreads_env = 'PROBE_THREADS'\nargs = ['inserted']\nargs_position = 1\nresult_format = 'regex'\nisolation = '{isolation}'\n");
            let mut flow = FLOW.replace(
                "timeout_secs = 20",
                "timeout_secs = 20\nservices = ['fixture']\nremove_env = ['GH93_REMOVED']",
            );
            flow.push_str(&runner);
            flow.push_str("\n[services.fixture]\nkind = 'environment'\nsource_env = 'GH93_SERVICE_ENV'\ninject_env = 'PROBE_SERVICE'\n");
            if sharded {
                flow.push_str("\n[execution.shards.probe]\nindex = 1\ntotal = 3\n");
            }
            fs::write(root.path().join(".harness-gate/flow.toml"), &flow).unwrap();
            fs::write(
                root.path().join("probe.sh"),
                r#"python3 - "$@" <<'PY'
import json, os, sys
from pathlib import Path
Path('observed.json').write_text(json.dumps({'args': sys.argv[1:], 'env': dict(os.environ)}))
print('probe-ran')
PY
"#,
            )
            .unwrap();
            success(
                command(root.path())
                    .env("GH93_SERVICE_ENV", "fixture-value")
                    .env("GH93_REMOVED", "must-disappear")
                    .args(["verify", "--all"])
                    .output()
                    .unwrap(),
            );
            let observed: Value =
                serde_json::from_slice(&fs::read(root.path().join("observed.json")).unwrap())
                    .unwrap();
            let json = report(root.path());
            assert_eq!(json["passed"], true);
            assert_sealed_evidence(&json);
            let step = json["steps"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["step_id"] == "probe")
                .unwrap();
            let metadata = &step["runner"];
            assert_eq!(
                metadata["effective_args"],
                serde_json::json!(["probe.sh", "inserted", "original"])
            );
            assert_eq!(
                observed["args"],
                serde_json::json!(["inserted", "original"])
            );
            assert_eq!(observed["env"]["PROBE_SERVICE"], "fixture-value");
            assert_eq!(observed["env"]["PROBE_THREADS"], threads.to_string());
            assert!(observed["env"].get("GH93_REMOVED").is_none());
            for (name, value) in metadata["environment"].as_object().unwrap() {
                assert_eq!(&observed["env"][name], value, "{name}");
            }
            assert_eq!(metadata["isolation"], isolation);
            if isolation == "shared" {
                assert_eq!(metadata["lock_decision"], "not-required");
                assert!(metadata["worker_ids"].as_array().unwrap().is_empty());
            } else {
                assert_eq!(metadata["lock_decision"], "invocation-scoped");
                assert_eq!(metadata["worker_ids"].as_array().unwrap().len(), 2);
                let state_root = Path::new(metadata["isolation_root"].as_str().unwrap());
                assert!(
                    !state_root.join("probe.json").exists(),
                    "isolation allocation must be released"
                );
            }
            if sharded {
                assert_eq!(metadata["shard_index"], 1);
                assert_eq!(metadata["shard_total"], 3);
                assert_eq!(metadata["merge_identity"], "probe:shard-1/3");
            } else {
                assert!(metadata["shard_index"].is_null());
            }

            // Validation must reject collisions and malformed runner/shard inputs before spawn.
            let mut invalid_flows = vec![
                flow.replace(
                    "threads_env = 'PROBE_THREADS'",
                    "threads_env = 'PROBE_SERVICE'",
                ),
                flow.replace("args_position = 1", "args_position = 99"),
                flow.replace(&format!("threads = {threads}"), "threads = 0"),
            ];
            if sharded {
                invalid_flows.push(flow.replace("index = 1", "index = 4"));
            }
            for invalid in invalid_flows {
                fs::remove_file(root.path().join("observed.json")).unwrap_or(());
                fs::write(root.path().join(".harness-gate/flow.toml"), &invalid).unwrap();
                let output = command(root.path())
                    .env("GH93_SERVICE_ENV", "fixture-value")
                    .args(["verify", "--all"])
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.code(),
                    Some(1),
                    "invalid flow: {invalid}\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("E1000"),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(!root.path().join("observed.json").exists());
                assert_eq!(
                    report(root.path()),
                    json,
                    "invalid config must not overwrite prior evidence"
                );
            }
        }
    }
}

fn failing_cleanup_runtime(root: &Path) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt;
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    // Only the external runtime is replaced. Service startup, fresh ownership
    // inspection, task execution, teardown and report publication are real.
    let script = r#"#!/usr/bin/env python3
import json, sys
from pathlib import Path
args = sys.argv[1:]
state = Path('runtime-object.json')
if args[0] == 'run':
    labels = dict(args[i+1].split('=', 1) for i, arg in enumerate(args) if arg == '--label')
    state.write_text(json.dumps({'Id': 'fixture-immutable-id', 'Name': '/' + args[args.index('--name')+1], 'Config': {'Labels': labels}}))
elif args[:2] == ['container', 'inspect']:
    print(state.read_text())
elif args[0] == 'port':
    print('127.0.0.1:15432')
elif args[0] == 'rm':
    with Path('remove-calls').open('a') as calls:
        calls.write('remove\n')
    print('injected cleanup failure', file=sys.stderr)
    sys.exit(8)
elif args[0] not in ('info', 'exec', 'image'):
    sys.exit(99)
"#;
    fs::write(bin.join("docker"), script).unwrap();
    fs::set_permissions(bin.join("docker"), fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    std::env::join_paths(paths).unwrap()
}

#[test]
fn verification_cleanup_cancel_and_report_precedence_retains_resource_evidence() {
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    for (cancel, block_report, expected_code) in [
        (false, false, "E1403"),
        (true, false, "E1402"),
        (true, true, "E1404"),
    ] {
        let root = fixture();
        let path = failing_cleanup_runtime(root.path());
        let flow = FLOW.replace(
            "timeout_secs = 20",
            "timeout_secs = 20\nservices = ['fixture']",
        ) + r#"
[services.fixture]
kind = 'docker'
image = 'fixture:local'
inject_env = 'PROBE_SERVICE'
startup_timeout_secs = 10
container_port = 5432
healthcheck = ['ready']
connection = 'fixture:{host_port}'
"#;
        fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
        fs::write(
            root.path().join("probe.sh"),
            if cancel {
                // Publish readiness only after the PID is fully written. Shell
                // redirection creates an empty file before echo writes to it.
                "echo $$ > started.tmp\nmv started.tmp started\nexec sleep 30\n"
            } else {
                "echo completed\n"
            },
        )
        .unwrap();
        if block_report {
            fs::create_dir_all(root.path().join(".harness-gate/reports/test_result.json")).unwrap();
        }
        let mut child = command(root.path())
            .env("PATH", &path)
            .args(["verify", "--all"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut task_pid = None;
        if cancel {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !root.path().join("started").exists() {
                if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                    let _ = child.kill();
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "task did not start: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            task_pid = Some(
                fs::read_to_string(root.path().join("started"))
                    .unwrap()
                    .trim()
                    .parse::<libc::pid_t>()
                    .unwrap(),
            );
            assert_eq!(
                unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) },
                0
            );
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("ERROR [{expected_code}]")),
            "{stderr}"
        );
        let cleanup_cause = if cancel {
            "internal command docker was cancelled"
        } else {
            "injected cleanup failure"
        };
        assert!(
            stderr.contains(cleanup_cause),
            "cancel={cancel} blocked={block_report}: {stderr}"
        );
        if let Some(pid) = task_pid {
            assert_eq!(
                unsafe { libc::kill(pid, 0) },
                -1,
                "cancelled child still alive"
            );
        }
        let invocations = root.path().join(".harness-gate/reports/invocations");
        let paths = fs::read_dir(invocations)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        let json: Value =
            serde_json::from_slice(&fs::read(paths[0].join("test_result.json")).unwrap()).unwrap();
        assert_eq!(json["passed"], false);
        assert_eq!(json["services"][0]["status"], "LEAKED");
        let steps = json["steps"].as_array().unwrap();
        let task = steps
            .iter()
            .find(|step| step["step_id"] == "probe")
            .unwrap();
        assert_eq!(task["cancelled"], cancel);
        assert_eq!(task["passed"], !cancel);
        let cleanup = steps.last().unwrap();
        assert_eq!(cleanup["label"], "service cleanup");
        assert_eq!(cleanup["passed"], false);
        assert_eq!(cleanup["cancelled"], false);
        assert_sealed_evidence(&json);
        if !block_report {
            assert_eq!(report(root.path()), json);
        }
        if cancel {
            assert!(
                !root.path().join("remove-calls").exists(),
                "uncertain ownership must prevent removal"
            );
        } else {
            assert_eq!(
                fs::read_to_string(root.path().join("remove-calls")).unwrap(),
                "remove\n"
            );
        }
        assert!(root.path().join("runtime-object.json").is_file());
        let leases = root.path().join(".harness-gate/reports/leases");
        let entries = fs::read_dir(&leases)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        let lease_files = entries
            .iter()
            .filter(|entry| {
                entry.file_type().unwrap().is_file()
                    && entry.path().extension() == Some(std::ffi::OsStr::new("json"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            lease_files.len(),
            1,
            "failed cleanup must retain exactly one container lease"
        );
        let lease: Value =
            serde_json::from_slice(&fs::read(lease_files[0].path()).unwrap()).unwrap();
        let project_identity = root.path().canonicalize().unwrap();
        let project_identity = project_identity.to_str().unwrap();
        let invocation_id = json["invocation_id"].as_str().unwrap();
        assert_eq!(lease["owner_marker"], "harness-gate");
        assert_eq!(lease["schema_version"], 2);
        assert_eq!(lease["resource_kind"], "container");
        assert_eq!(lease["resource_id"], "service:fixture");
        assert_eq!(lease["invocation_id"], invocation_id);
        assert_eq!(lease["project_identity"], project_identity);
        assert_eq!(lease["runtime"], "docker");
        assert_eq!(lease["runtime_object_id"], "fixture-immutable-id");
        let runtime: Value =
            serde_json::from_slice(&fs::read(root.path().join("runtime-object.json")).unwrap())
                .unwrap();
        assert_eq!(lease["runtime_object_id"], runtime["Id"]);
        assert_eq!(
            runtime["Name"],
            format!("/{}", lease["resource_name"].as_str().unwrap())
        );
        let labels = serde_json::json!({
            "harness-gate.owner": "harness-gate",
            "harness-gate.schema": "2",
            "harness-gate.project": project_identity,
            "harness-gate.resource": "service:fixture",
            "harness-gate.kind": "container",
            "harness-gate.invocation": invocation_id,
        });
        assert_eq!(lease["runtime_labels"], labels);
        assert_eq!(runtime["Config"]["Labels"], labels);
        let container_key = format!("{:x}", Sha256::digest(b"service:fixture"));
        assert_eq!(
            lease_files[0].file_name(),
            std::ffi::OsStr::new(&format!("{}.json", &container_key[..16]))
        );
        let locks = leases.join("report-directory-locks");
        assert!(fs::symlink_metadata(&locks).unwrap().file_type().is_dir());
        for entry in &entries {
            assert!(
                entry.path() == lease_files[0].path() || entry.path() == locks,
                "unexpected lease-directory entry: {:?}",
                entry.path()
            );
        }
        let sidecars = fs::read_dir(&locks)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(sidecars.len(), 1, "one invocation has one stable lock");
        assert!(sidecars[0].file_type().unwrap().is_file());
        assert_eq!(
            sidecars[0].path().extension(),
            Some(std::ffi::OsStr::new("lock"))
        );
        let invocation_key = format!(
            "{:x}",
            Sha256::digest(format!("invocation:{invocation_id}").as_bytes())
        );
        assert_eq!(
            sidecars[0].file_name(),
            std::ffi::OsStr::new(&format!("{}.lock", &invocation_key[..16]))
        );
        // Invocation Drop releases its report lease even when service cleanup
        // or current-report publication fails. Its certificate is a sidecar,
        // while the failed container's ownership marker remains a JSON lease.
        let released: Value =
            serde_json::from_slice(&fs::read(sidecars[0].path()).unwrap()).unwrap();
        assert_eq!(
            released,
            serde_json::json!({
                "schema_version": 1,
                "invocation_id": invocation_id,
                "project_identity": project_identity,
                "root": paths[0].canonicalize().unwrap(),
            })
        );
        let cleanup = command(root.path())
            .args(["cleanup", "--dry-run"])
            .output()
            .unwrap();
        let cleanup = success(cleanup);
        assert!(String::from_utf8_lossy(&cleanup.stdout).contains("Cleanup (dry-run): scanned 1"));
        fs::write(leases.join("invalid.json"), "not JSON").unwrap();
        let cleanup = command(root.path()).args(["cleanup"]).output().unwrap();
        assert!(!cleanup.status.success());
        assert!(String::from_utf8_lossy(&cleanup.stderr).contains("cleanup failure:"));
        assert!(leases.join("invalid.json").is_file());
    }
}

#[test]
fn extracted_cli_handlers_preserve_text_json_and_hook_snapshot_contracts() {
    let root = fixture();
    let doctor = success(command(root.path()).args(["doctor"]).output().unwrap());
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("harness-gate doctor"));
    let scope = success(
        command(root.path())
            .args(["scope", "--all", "--benchmark-repeat", "2"])
            .output()
            .unwrap(),
    );
    let benchmark: Value = serde_json::from_slice(&scope.stdout).unwrap();
    assert!(benchmark.is_object());
    let scope = success(
        command(root.path())
            .args(["scope", "--all", "--json"])
            .output()
            .unwrap(),
    );
    let scope: Value = serde_json::from_slice(&scope.stdout).unwrap();
    assert_eq!(scope["mode"], "all");
    assert!(scope["components"]
        .as_array()
        .unwrap()
        .contains(&Value::String("project".into())));
    let secrets = success(
        command(root.path())
            .args(["secrets", "--json"])
            .output()
            .unwrap(),
    );
    let clean: Value = serde_json::from_slice(&secrets.stdout).unwrap();
    assert_eq!(clean["passed"], true);
    // A deterministic rule avoids placing real credentials in fixtures.
    let config_path = root.path().join(".harness-gate/secrets.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    fs::write(
        config_path,
        format!(
            "{config}\n[[rules]]\nid = 'fixture'\nkind = 'direct'\npattern = 'GH94_SENTINEL'\n"
        ),
    )
    .unwrap();
    fs::write(root.path().join("credential.txt"), "GH94_SENTINEL").unwrap();
    let rejected = command(root.path()).args(["secrets"]).output().unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("credential.txt"));
    fs::remove_file(root.path().join("credential.txt")).unwrap();
    // The rule itself contains the sentinel, so restore it before snapshotting.
    fs::write(root.path().join(".harness-gate/secrets.toml"), config).unwrap();
    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    fs::write(root.path().join("probe.sh"), "echo staged-hook\n").unwrap();
    git(root.path(), &["add", "probe.sh"]);
    fs::write(
        root.path().join("probe.sh"),
        "echo unstaged-hook >&2\nexit 9\n",
    )
    .unwrap();
    // Model macOS's symlinked temporary directory on every Unix runner.
    let temporary = TempDir::new().unwrap();
    let actual = temporary.path().join("actual");
    let alias = temporary.path().join("alias");
    fs::create_dir(&actual).unwrap();
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    success(
        command(root.path())
            .env("TMPDIR", &alias)
            .args(["hook"])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_dir(&actual).unwrap().count(), 0);
    let evidence = report(root.path());
    assert_eq!(evidence["passed"], true);
    assert!(serde_json::to_string(&evidence).unwrap().contains("hook"));
    assert_sealed_evidence(&evidence);
    // Reject an unusable temporary parent without leaving snapshot debris.
    let not_directory = temporary.path().join("not-directory");
    fs::write(&not_directory, "not a directory").unwrap();
    let rejected = command(root.path())
        .env("TMPDIR", &not_directory)
        .args(["hook"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("create staged snapshot"));
    let rejected = command(root.path())
        .env("TMPDIR", temporary.path().join("missing"))
        .args(["hook"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr)
        .contains("resolve staged snapshot temporary directory"));
}

#[test]
fn compatibility_dispatch_retains_request_identity_and_shadow_failure() {
    let root = fixture();
    let input = root.path().join("request.json");
    let output = root.path().join("result.json");
    fs::write(
        &input,
        r#"{"schema_version":1,"all":true,"request_id":"GH94-compat"}"#,
    )
    .unwrap();
    let mut args = vec![
        "compat",
        "run",
        "--input",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ];
    let first = success(command(root.path()).args(&args).output().unwrap());
    assert!(String::from_utf8_lossy(&first.stdout).contains("GH94-compat"));
    let old = root.path().join("old.json");
    fs::write(&old, "{}").unwrap();
    args.extend(["--old-result", old.to_str().unwrap()]);
    let shadow = command(root.path()).args(&args).output().unwrap();
    assert!(!shadow.status.success());
    assert!(String::from_utf8_lossy(&shadow.stdout).contains("\"equivalent\": false"));
    let input = root.path().join("events.jsonl");
    let output = root.path().join("errors.md");
    fs::write(
        &input,
        "{\"level\":\"ERROR\",\"trace_id\":\"fixture\",\"fields\":{\"error\":\"root cause\"}}\n",
    )
    .unwrap();
    success(
        command(root.path())
            .args([
                "parse-logs",
                "--input",
                input.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ])
            .output()
            .unwrap(),
    );
    assert!(fs::read_to_string(output).unwrap().contains("root cause"));
}
