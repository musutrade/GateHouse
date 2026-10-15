use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tempfile::TempDir;

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

fn suggest(source: &str) -> (TempDir, std::process::Output) {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("flow.toml"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args([
            "--config",
            "flow.toml",
            "--color",
            "never",
            "config",
            "suggest",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("flow.toml")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_dir(root.path()).unwrap().count(),
        1,
        "suggestion must not write reports or run commands"
    );
    (root, output)
}

#[test]
fn valid_workflow_plan_is_read_only_and_not_full_project_validation() {
    let (_root, output) = suggest(FLOW);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["schema"], "configuration-suggestions/v1");
    assert_eq!(plan["scope"], "workflow-only");
    assert_eq!(plan["configuration"]["valid"], true);
    assert_eq!(plan["suggestions"], serde_json::json!([]));
    assert_eq!(
        plan["source_sha256"],
        format!("{:x}", Sha256::digest(FLOW.as_bytes()))
    );
}

#[test]
fn invalid_log_plan_binds_an_edit_to_the_original_token() {
    let source = FLOW.replace("probe.log", "bad/path.log");
    let (_root, output) = suggest(&source);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    let suggestion = plan["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["diagnostic_id"] == "HGCFG-INVALID-LOG")
        .expect("log repair");
    assert_eq!(suggestion["applicability"], "maybe_incorrect");
    let edit = &suggestion["edit"];
    let start = edit["start"].as_u64().unwrap() as usize;
    let end = edit["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], "\"bad/path.log\"");
    let mut repaired = source.clone();
    repaired.replace_range(start..end, edit["replacement"].as_str().unwrap());
    let (_, rechecked) = suggest(&repaired);
    assert!(rechecked.status.success());
}

#[test]
fn missing_required_check_has_no_automatic_removal() {
    let source = format!("{FLOW}\n[policy]\nrequired_steps = [\"missing-check\"]\n");
    let (_, output) = suggest(&source);
    assert_eq!(output.status.code(), Some(1));
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    let suggestion = plan["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["field"] == "policy.required_steps")
        .expect("required reference diagnostic");
    assert_eq!(suggestion["applicability"], "manual");
    assert!(suggestion.get("edit").is_none());
    assert!(suggestion["message"]
        .as_str()
        .unwrap()
        .contains("without dropping required steps"));
}

#[test]
fn malformed_toml_produces_guidance_without_source_values() {
    let (_, output) = suggest("version = [\npassword = 'synthetic-credential'\n");
    assert_eq!(output.status.code(), Some(1));
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["suggestions"][0]["diagnostic_id"], "HGCFG-TOML-PARSE");
    assert!(plan["suggestions"][0].get("edit").is_none());
    assert!(!String::from_utf8(output.stdout)
        .unwrap()
        .contains("synthetic-credential"));
}

#[test]
fn duplicate_log_candidate_resolves_the_conflict_without_dropping_steps() {
    let second = FLOW
        .split_once("[[steps]]")
        .unwrap()
        .1
        .replace("id = \"probe\"", "id = \"second\"");
    let source = format!("{FLOW}\n[[steps]]{second}");
    let (_, output) = suggest(&source);
    assert_eq!(output.status.code(), Some(1));
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    let suggestion = plan["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["diagnostic_id"] == "HGCFG-DUPLICATE-LOG")
        .unwrap();
    let edit = &suggestion["edit"];
    let mut repaired = source.clone();
    repaired.replace_range(
        edit["start"].as_u64().unwrap() as usize..edit["end"].as_u64().unwrap() as usize,
        edit["replacement"].as_str().unwrap(),
    );
    assert_eq!(repaired.matches("[[steps]]").count(), 2);
    assert!(repaired.contains("id = \"probe\"") && repaired.contains("id = \"second\""));
    let (_, output) = suggest(&repaired);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn human_plans_show_scope_and_applicability_without_writing() {
    for source in [FLOW.to_string(), FLOW.replace("probe.log", "bad/path.log")] {
        let (root, _) = suggest(&source);
        let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(root.path())
            .args([
                "--config",
                "flow.toml",
                "--color",
                "never",
                "config",
                "suggest",
            ])
            .output()
            .unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("read-only; not full config check"));
        if source == FLOW {
            assert!(output.status.success());
            assert!(text.contains("No workflow repairs suggested"));
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(text.contains("applicability: maybe incorrect"));
        }
        assert!(output.stderr.is_empty());
        assert_eq!(
            fs::read_to_string(root.path().join("flow.toml")).unwrap(),
            source
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn config_outside_selected_root_is_rejected() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let path = outside.path().join("flow.toml");
    fs::write(&path, FLOW).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .arg("--config")
        .arg(&path)
        .args(["--color", "never", "config", "suggest", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("workflow config must be inside"));
    assert_eq!(fs::read_to_string(path).unwrap(), FLOW);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn environment_dependent_peer_log_withholds_a_collision_prone_edit() {
    let first = FLOW.replace("probe.log", "bad/path.log");
    let peer = FLOW
        .split_once("[[steps]]")
        .unwrap()
        .1
        .replace("id = \"probe\"", "id = \"peer\"")
        .replace("probe.log", "${GH_REPAIR_PEER:-step-0-repaired.log}");
    let source = format!("{first}\n[[steps]]{peer}");
    let (_, output) = suggest(&source);
    assert_eq!(output.status.code(), Some(1));
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    let suggestion = plan["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["diagnostic_id"] == "HGCFG-INVALID-LOG")
        .unwrap();
    assert_eq!(suggestion["applicability"], "manual");
    assert!(suggestion.get("edit").is_none());
}

#[test]
fn interactive_application_rejects_pipes_and_json_without_writes() {
    let directory = TempDir::new().unwrap();
    let source = FLOW.replace("probe.log", "bad/path.log");
    let path = directory.path().join("flow.toml");
    fs::write(&path, &source).unwrap();
    for format in ["human", "json"] {
        let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(directory.path())
            .args([
                "--config",
                "flow.toml",
                "config",
                "suggest",
                "--apply",
                "--format",
                format,
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains(if format == "json" {
                "interactive repairs cannot use JSON output"
            } else {
                "interactive repairs require terminal input and stderr"
            }),
            "{error}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[cfg(unix)]
mod terminal {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::process::{Child, Stdio};
    use std::time::{Duration, Instant};

    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn terminals() -> (fs::File, fs::File) {
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: openpty receives valid writable descriptor pointers; optional
        // name, terminal settings and window size pointers are null defaults.
        let result = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, 0, "{}", std::io::Error::last_os_error());
        // SAFETY: successful openpty returned two owned, distinct descriptors.
        let (master, slave) =
            unsafe { (fs::File::from_raw_fd(master), fs::File::from_raw_fd(slave)) };
        // SAFETY: master is a live owned descriptor and F_SETFL takes an integer
        // flag argument. Nonblocking reads keep the test deadline enforceable.
        assert_ne!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            -1
        );
        (master, slave)
    }

    fn exercise(answer: &[u8], concurrent_edit: bool, expect_success: bool) {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("flow.toml");
        let source = FLOW.replace("probe.log", "bad/path.log");
        fs::write(&path, &source).unwrap();
        let (mut master, slave) = terminals();
        let mut child = ChildGuard(
            Command::new(env!("CARGO_BIN_EXE_harness-gate"))
                .arg("--project-root")
                .arg(directory.path())
                .args(["--config", "flow.toml", "config", "suggest", "--apply"])
                .stdin(slave.try_clone().unwrap())
                .stderr(slave)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut output = Vec::new();
        let mut sent = false;
        let status = loop {
            assert!(
                Instant::now() < deadline,
                "terminal repair timed out: {}",
                String::from_utf8_lossy(&output)
            );
            let mut buffer = [0; 4096];
            match master.read(&mut buffer) {
                Ok(count) => output.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                // Linux PTYs report EIO after the last slave closes.
                Err(error) if error.raw_os_error() == Some(libc::EIO) => {}
                Err(error) => panic!("terminal read: {error}"),
            }
            if !sent
                && String::from_utf8_lossy(&output).contains("Apply these candidate edits? [y/N]")
            {
                assert!(String::from_utf8_lossy(&output).contains("step-0-repaired.log"));
                if concurrent_edit {
                    fs::write(&path, format!("{source}\n# concurrent edit\n")).unwrap();
                }
                master.write_all(answer).unwrap();
                sent = true;
            }
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(sent, "{}", String::from_utf8_lossy(&output));
        assert_eq!(
            status.success(),
            expect_success,
            "{}",
            String::from_utf8_lossy(&output)
        );
        let expected = if concurrent_edit {
            format!("{source}\n# concurrent edit\n")
        } else if expect_success {
            source.replace("bad/path.log", "step-0-repaired.log")
        } else {
            source
        };
        assert_eq!(fs::read_to_string(path).unwrap(), expected);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn confirms_real_terminal_repair() {
        exercise(b"yes\n", false, true);
    }

    #[test]
    fn empty_terminal_confirmation_declines() {
        exercise(b"\n", false, false);
    }

    #[test]
    fn terminal_repair_refuses_concurrent_file_changes() {
        exercise(b"yes\n", true, false);
    }
}
