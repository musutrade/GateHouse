use std::{
    path::Path,
    process::{Command, Output},
};

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", root.join("no-global-config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "PATH",
            std::env::join_paths(
                std::iter::once(
                    Path::new(env!("CARGO_BIN_EXE_harness-gate"))
                        .parent()
                        .unwrap()
                        .to_path_buf(),
                )
                .chain(std::env::split_paths(
                    &std::env::var_os("PATH").unwrap_or_default(),
                )),
            )
            .unwrap(),
        )
        .output()
        .unwrap()
}
fn hg(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", root.join("no-global-config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "PATH",
            std::env::join_paths(
                std::iter::once(
                    Path::new(env!("CARGO_BIN_EXE_harness-gate"))
                        .parent()
                        .unwrap()
                        .to_path_buf(),
                )
                .chain(std::env::split_paths(
                    &std::env::var_os("PATH").unwrap_or_default(),
                )),
            )
            .unwrap(),
        )
        .output()
        .unwrap()
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    success(git(root.path(), &["init", "-q"]));
    success(hg(root.path(), &["init"]));
    root
}

fn assert_gate_failed(root: &Path, expected_step: &str) {
    let bytes = std::fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["status"], "FAIL");
    assert!(
        report["steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step_id"] == expected_step && step["status"] == "FAIL"),
        "{report}"
    );
}

#[test]
fn lifecycle_is_explicit_idempotent_and_preserves_modified_hooks() {
    let root = project();
    assert!(!hg(root.path(), &["hooks", "status"]).status.success());
    success(hg(root.path(), &["hooks", "install"]));
    success(hg(root.path(), &["hooks", "status"]));
    let hook = root.path().join(".git/hooks/pre-commit");
    let content = std::fs::read(&hook).unwrap();
    success(hg(root.path(), &["hooks", "install"]));
    assert_eq!(std::fs::read(&hook).unwrap(), content);
    std::fs::write(&hook, [content.as_slice(), b"# custom\n"].concat()).unwrap();
    for action in ["status", "install", "uninstall"] {
        assert!(!hg(root.path(), &["hooks", action]).status.success());
    }
    assert!(root.path().join(".git/hooks/pre-push").exists());
    assert!(std::fs::read_to_string(&hook)
        .unwrap()
        .ends_with("# custom\n"));
    std::fs::write(&hook, content).unwrap();
    success(hg(root.path(), &["hooks", "uninstall"]));
    success(hg(root.path(), &["hooks", "uninstall"]));
    assert!(!hg(root.path(), &["hooks", "status"]).status.success());
}

#[test]
fn refuses_foreign_paths_busy_state_and_empty_hook_profile() {
    let root = project();
    let hook = root.path().join(".git/hooks/pre-push");
    std::fs::write(&hook, "foreign").unwrap();
    assert!(!hg(root.path(), &["hooks", "install"]).status.success());
    assert!(!root.path().join(".git/hooks/pre-commit").exists());
    assert_eq!(std::fs::read(&hook).unwrap(), b"foreign");
    std::fs::remove_file(&hook).unwrap();
    std::fs::create_dir(root.path().join("elsewhere")).unwrap();
    std::fs::write(root.path().join("elsewhere/pre-commit"), "foreign manager").unwrap();
    success(git(root.path(), &["config", "core.hooksPath", "elsewhere"]));
    assert!(!hg(root.path(), &["hooks", "install"]).status.success());
    assert_eq!(
        std::fs::read(root.path().join("elsewhere/pre-commit")).unwrap(),
        b"foreign manager"
    );
    success(git(root.path(), &["config", "--unset", "core.hooksPath"]));
    let lock = root.path().join(".git/hooks/.harness-gate-hooks.lock");
    std::fs::write(&lock, "unknown stale owner").unwrap();
    for action in ["status", "install", "uninstall"] {
        assert!(!hg(root.path(), &["hooks", action]).status.success());
    }
    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        "unknown stale owner"
    );
    std::fs::remove_file(lock).unwrap();
    let config = root.path().join(".harness-gate/flow.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("hook_profile = \"hook\"", "hook_profile = \"nonexistent\"");
    std::fs::write(config, text).unwrap();
    assert!(!hg(root.path(), &["hooks", "install"]).status.success());
    assert!(!root.path().join(".git/hooks/pre-commit").exists());
}

#[cfg(unix)]
#[test]
fn symlinks_and_missing_executable_permission_are_not_current() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = project();
    let target = root.path().join("foreign");
    std::fs::write(&target, "foreign").unwrap();
    let hook = root.path().join(".git/hooks/pre-commit");
    symlink(&target, &hook).unwrap();
    assert!(!hg(root.path(), &["hooks", "install"]).status.success());
    assert!(!hg(root.path(), &["hooks", "uninstall"]).status.success());
    assert_eq!(std::fs::read(&target).unwrap(), b"foreign");
    std::fs::remove_file(&hook).unwrap();
    success(hg(root.path(), &["hooks", "install"]));
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!hg(root.path(), &["hooks", "status"]).status.success());
    success(hg(root.path(), &["hooks", "uninstall"]));
    let hooks = root.path().join(".git/hooks");
    let moved = root.path().join("saved-hooks");
    std::fs::rename(&hooks, &moved).unwrap();
    symlink(&moved, &hooks).unwrap();
    assert!(!hg(root.path(), &["hooks", "install"]).status.success());
}

#[test]
fn real_commit_push_and_linked_worktree_use_thin_hooks() {
    let root = project();
    success(git(
        root.path(),
        &["config", "user.email", "test@example.invalid"],
    ));
    success(git(root.path(), &["config", "user.name", "Hook Test"]));
    success(git(root.path(), &["add", "."]));
    success(git(root.path(), &["commit", "-qm", "initial"]));
    let linked_parent = tempfile::tempdir().unwrap();
    let linked = linked_parent.path().join("linked project with spaces");
    success(git(
        root.path(),
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    ));
    success(hg(&linked, &["hooks", "install"]));
    success(hg(root.path(), &["hooks", "status"]));
    success(hg(&linked, &["hooks", "uninstall"]));
    success(hg(root.path(), &["hooks", "install"]));
    std::fs::write(
        root.path().join("sample.txt"),
        "bad trailing whitespace   \n",
    )
    .unwrap();
    success(git(root.path(), &["add", "sample.txt"]));
    assert!(!git(root.path(), &["commit", "-qm", "must fail"])
        .status
        .success());
    assert_gate_failed(root.path(), "project.staged-diff-check");
    std::fs::write(root.path().join("sample.txt"), "clean\n").unwrap();
    success(git(root.path(), &["add", "sample.txt"]));
    success(git(root.path(), &["commit", "-qm", "valid"]));
    let remote = tempfile::tempdir().unwrap();
    success(git(remote.path(), &["init", "--bare", "-q"]));
    std::fs::write(
        root.path().join("sample.txt"),
        "bad trailing whitespace   \n",
    )
    .unwrap();
    assert!(!git(
        root.path(),
        &[
            "push",
            remote.path().to_str().unwrap(),
            "HEAD:refs/heads/test"
        ]
    )
    .status
    .success());
    assert_gate_failed(root.path(), "project.diff-check");
    std::fs::write(root.path().join("sample.txt"), "clean\n").unwrap();
    success(git(
        root.path(),
        &[
            "push",
            remote.path().to_str().unwrap(),
            "HEAD:refs/heads/test",
        ],
    ));
}
