use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
    command.arg("--project-root").arg(root);
    command
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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
    success(
        Command::new("git")
            .arg("init")
            .current_dir(root.path())
            .output()
            .unwrap(),
    );
    fs::write(
        root.path().join("probe.sh"),
        "fixture root must be a directory",
    )
    .unwrap();
    root
}

#[test]
fn compatibility_cli_preserves_comparison_and_retires_lifecycle_state() {
    let root = fixture();
    let old = root.path().join("old.json");
    let new = root.path().join("new.json");
    let comparison = root.path().join("comparison.json");
    let state = root.path().join("canary.json");
    fs::write(&old, r#"{"status":"pass"}"#).unwrap();
    for (value, equivalent) in [("pass", true), ("fail", false)] {
        fs::write(&new, format!(r#"{{"status":"{value}"}}"#)).unwrap();
        let output = command(root.path())
            .args(["compat", "compare", "--old"])
            .arg(&old)
            .arg("--new")
            .arg(&new)
            .arg("--output")
            .arg(&comparison)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), equivalent);
        let report: Value = serde_json::from_slice(&fs::read(&comparison).unwrap()).unwrap();
        assert_eq!(report["equivalent"], equivalent);
        assert_eq!(
            report["differences"].as_array().unwrap().is_empty(),
            equivalent
        );
    }
    // Retired lifecycle commands are rejected and change no state: no canary
    // file, installed hook or lock may appear.
    for args in [
        vec!["compat", "canary", "--slice", "ci", "--state"],
        vec!["compat", "rollback", "--state"],
        vec!["hooks", "install"],
        vec!["hooks", "status"],
        vec!["hooks", "uninstall"],
    ] {
        let mut invocation = command(root.path());
        invocation.args(&args);
        if args[0] == "compat" {
            invocation.arg(&state);
        }
        let output = invocation.output().unwrap();
        assert!(!output.status.success(), "{args:?}");
        assert!(!state.exists(), "{args:?}");
        assert!(
            !root.path().join(".git/hooks/pre-commit").exists(),
            "{args:?}"
        );
        assert!(
            !root
                .path()
                .join(".git/hooks/.harness-gate-hooks.lock")
                .exists(),
            "{args:?}"
        );
    }
}

fn assert_gate_failed(root: &Path, expected_step: &str) {
    let report: Value = serde_json::from_slice(
        &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["status"], "FAIL", "{report}");
    assert!(
        report["steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step_id"] == expected_step && step["status"] == "FAIL"),
        "{report}"
    );
}

#[cfg(unix)]
#[test]
fn existing_managed_hooks_still_invoke_core_hook_and_verify() {
    use std::os::unix::fs::PermissionsExt;
    let root = fixture();
    let binary_dir = Path::new(env!("CARGO_BIN_EXE_harness-gate"))
        .parent()
        .unwrap()
        .to_path_buf();
    let git = |args: &[&str]| {
        let path = std::env::join_paths(std::iter::once(binary_dir.clone()).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(args)
            .env("PATH", path)
            .env("GIT_CONFIG_GLOBAL", root.path().join("no-global-config"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap()
    };
    success(git(&["config", "user.email", "hooks@example.invalid"]));
    success(git(&["config", "user.name", "Retired Lifecycle"]));
    success(git(&["config", "commit.gpgsign", "false"]));
    // The hook command reads the staged workflow configuration, so the
    // initialized project must be committed before hooks are exercised.
    success(git(&["add", ".harness-gate"]));
    success(git(&["commit", "-qm", "initial config"]));
    let hooks = root.path().join(".git/hooks");
    for (name, body) in [
        (
            "pre-commit",
            "#!/bin/sh\n# harness-gate managed hook v1\nexec harness-gate hook\n",
        ),
        (
            "pre-push",
            "#!/bin/sh\n# harness-gate managed hook v1\nexec harness-gate verify --all\n",
        ),
    ] {
        let path = hooks.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Existing exact managed hook bytes still execute Core hook/verify even
    // though Core no longer owns lifecycle commands.
    fs::write(
        root.path().join("sample.txt"),
        "bad trailing whitespace   \n",
    )
    .unwrap();
    success(git(&["add", "sample.txt"]));
    assert!(!git(&["commit", "-qm", "must fail"]).status.success());
    assert_gate_failed(root.path(), "project.staged-diff-check");
    fs::write(root.path().join("sample.txt"), "clean\n").unwrap();
    success(git(&["add", "sample.txt"]));
    success(git(&["commit", "-qm", "valid"]));
    let remote = root.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    success(git(&["init", "--bare", "-q", remote.to_str().unwrap()]));
    fs::write(
        root.path().join("sample.txt"),
        "bad trailing whitespace   \n",
    )
    .unwrap();
    assert!(
        !git(&["push", remote.to_str().unwrap(), "HEAD:refs/heads/test"])
            .status
            .success()
    );
    assert_gate_failed(root.path(), "project.diff-check");
    fs::write(root.path().join("sample.txt"), "clean\n").unwrap();
    success(git(&[
        "push",
        remote.to_str().unwrap(),
        "HEAD:refs/heads/test",
    ]));
}

#[test]
fn migration_cli_preserves_source_and_refuses_overwrite() {
    let root = TempDir::new().unwrap();
    let legacy = include_str!("../fixtures/v1-flow.toml");
    fs::write(root.path().join("legacy.toml"), legacy).unwrap();
    fs::create_dir(root.path().join(".harness-gate")).unwrap();
    fs::write(
        root.path().join(".harness-gate/audit.toml"),
        include_str!("../presets/empty.audit.toml"),
    )
    .unwrap();
    let missing = command(root.path())
        .args(["config", "migrate"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("requires --input"));
    assert!(!root.path().join(".harness-gate/flow.toml").exists());
    let refused = command(root.path())
        .args(["--config", "legacy.toml", "config", "migrate"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("gateprobes-postgres migrate"));
    assert!(!root.path().join(".harness-gate/flow.toml").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("legacy.toml")).unwrap(),
        legacy
    );
}

#[test]
fn config_cli_discovers_nested_project_and_rejects_file_root() {
    let root = fixture();
    let nested = root.path().join("nested");
    fs::create_dir(&nested).unwrap();
    for config in [None, Some(root.path().join(".harness-gate/flow.toml"))] {
        let mut cli = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        cli.current_dir(&nested);
        if let Some(path) = config {
            cli.arg("--config").arg(path);
        }
        success(cli.args(["config", "check"]).output().unwrap());
    }
    let output = command(&root.path().join("probe.sh"))
        .args(["config", "check"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a directory"));
}
