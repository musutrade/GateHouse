#![cfg(unix)]

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const FLOW: &str = r#"
version = 2
[project]
name = "rename-scope"
default_profile = "full"
hook_profile = "hook"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "fail"
[[scope.rules]]
patterns = ["a/**"]
components = ["a"]
[[scope.rules]]
patterns = ["b/**"]
components = ["b"]
[[scope.rules]]
patterns = [".harness-gate/**", "probe.sh"]
components = ["a", "b"]
[[steps]]
id = "setup"
label = "setup"
component = "support"
profiles = ["full", "hook"]
program = "true"
args = []
cwd = "{root}"
log = "setup.log"
timeout_secs = 10
[[steps]]
id = "a.test"
label = "a test"
component = "a"
profiles = ["full", "hook"]
program = "sh"
args = ["probe.sh"]
depends_on = ["setup"]
cwd = "{root}"
log = "a.log"
timeout_secs = 10
[[steps]]
id = "b.test"
label = "b test"
component = "b"
profiles = ["full", "hook"]
program = "true"
args = []
cwd = "{root}"
log = "b.log"
timeout_secs = 10
[[steps]]
id = "a.full-only"
label = "full only"
component = "a"
profiles = ["full"]
program = "true"
args = []
cwd = "{root}"
log = "full.log"
timeout_secs = 10
[[steps]]
id = "unrelated"
label = "unrelated"
component = "unrelated"
profiles = ["full", "hook"]
program = "false"
args = []
cwd = "{root}"
log = "unrelated.log"
timeout_secs = 10
"#;

fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn git(root: &Path, args: &[&str]) -> Output {
    success(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap(),
    )
}

fn gate(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn fixture() -> TempDir {
    let root = TempDir::new().unwrap();
    success(gate(root.path(), &["init", "--preset", "generic"]));
    git(root.path(), &["init", "--quiet"]);
    git(root.path(), &["config", "user.name", "Scope Test"]);
    git(
        root.path(),
        &["config", "user.email", "scope@example.invalid"],
    );
    git(root.path(), &["config", "commit.gpgsign", "false"]);
    fs::write(root.path().join(".harness-gate/flow.toml"), FLOW).unwrap();
    fs::write(
        root.path().join("probe.sh"),
        "echo source-component\nexit 7\n",
    )
    .unwrap();
    fs::create_dir(root.path().join("a")).unwrap();
    fs::create_dir(root.path().join("b")).unwrap();
    fs::write(root.path().join("a/old name\nfile.txt"), "initial\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "--quiet", "-m", "initial"]);
    root
}

#[test]
fn changed_paths_cover_both_rename_ends_in_every_mode() {
    let old = "a/old name\nfile.txt";
    for rename_setting in ["true", "false", "copies"] {
        for mode in ["staged", "working-cached", "working-unstaged", "base"] {
            for change in ["cross-rename", "same-rename", "add", "modify", "delete"] {
                let root = fixture();
                let root = root.path();
                git(root, &["config", "diff.renames", rename_setting]);
                let (paths, components) = match change {
                    "cross-rename" | "same-rename" => {
                        let new = if change == "cross-rename" {
                            "b/new name\nfile.txt"
                        } else {
                            "a/new name\nfile.txt"
                        };
                        fs::rename(root.join(old), root.join(new)).unwrap();
                        let components = if change == "cross-rename" {
                            json!(["a", "b"])
                        } else {
                            json!(["a"])
                        };
                        (json!([old, new]), components)
                    }
                    "add" => {
                        fs::write(root.join("b/added name\nfile.txt"), "added\n").unwrap();
                        (json!(["b/added name\nfile.txt"]), json!(["b"]))
                    }
                    "modify" => {
                        fs::write(root.join(old), "modified\n").unwrap();
                        (json!([old]), json!(["a"]))
                    }
                    "delete" => {
                        fs::remove_file(root.join(old)).unwrap();
                        (json!([old]), json!(["a"]))
                    }
                    _ => unreachable!(),
                };
                let mode_args = match mode {
                    "working-unstaged" => vec![],
                    "staged" | "working-cached" => {
                        git(root, &["add", "-A"]);
                        if mode == "staged" {
                            vec!["--staged"]
                        } else {
                            vec![]
                        }
                    }
                    "base" => {
                        git(root, &["add", "-A"]);
                        git(root, &["commit", "--quiet", "-m", "change"]);
                        vec!["--base", "HEAD^"]
                    }
                    _ => unreachable!(),
                };
                let mut args = vec!["scope", "--json"];
                args.extend(mode_args);
                let output = success(gate(root, &args));
                let scope: Value = serde_json::from_slice(&output.stdout).unwrap();
                // Scope is sorted, including same-component renames (new before old).
                let mut expected = paths.as_array().unwrap().clone();
                expected.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                assert_eq!(
                    scope["changed_files"],
                    json!(expected),
                    "{rename_setting}/{mode}/{change}"
                );
                assert_eq!(
                    scope["components"], components,
                    "{rename_setting}/{mode}/{change}"
                );
                assert_eq!(scope["unmatched_files"], json!([]));
            }
        }
    }
}

#[test]
fn source_component_failure_blocks_verify_and_preserves_selection_semantics() {
    for mode in ["staged", "working", "base"] {
        for profile in ["full", "hook"] {
            let root = fixture();
            let root = root.path();
            git(
                root,
                &["mv", "a/old name\nfile.txt", "b/new name\nfile.txt"],
            );
            let mut args = vec!["verify", "--profile", profile];
            match mode {
                "staged" => {
                    args.push("--staged");
                    // The selected source component must execute the indexed script.
                    fs::write(root.join("probe.sh"), "echo working-only\nexit 0\n").unwrap();
                }
                "base" => {
                    git(root, &["commit", "--quiet", "-m", "rename"]);
                    args.extend(["--base", "HEAD^"]);
                }
                "working" => {}
                _ => unreachable!(),
            }
            let output = gate(root, &args);
            assert_eq!(
                output.status.code(),
                Some(1),
                "{mode}/{profile}: {output:?}"
            );
            let result: Value = serde_json::from_slice(
                &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(result["passed"], false);
            let steps = result["steps"].as_array().unwrap();
            let mut ids: Vec<_> = steps
                .iter()
                .map(|s| s["step_id"].as_str().unwrap())
                .collect();
            ids.sort();
            let expected = if profile == "full" {
                vec![
                    "a.full-only",
                    "a.test",
                    "b.test",
                    "builtin.architecture-audit",
                    "builtin.secret-scan",
                    "setup",
                ]
            } else {
                vec![
                    "a.test",
                    "b.test",
                    "builtin.architecture-audit",
                    "builtin.secret-scan",
                    "setup",
                ]
            };
            assert_eq!(ids, expected, "{mode}/{profile}");
            let source = steps.iter().find(|s| s["step_id"] == "a.test").unwrap();
            assert_eq!(source["passed"], false);
            let log = fs::read_to_string(source["log"].as_str().unwrap()).unwrap();
            assert!(log.contains("source-component"));
            assert!(!log.contains("working-only"));
            if mode == "staged" {
                assert!(result["source_identity"]
                    .as_str()
                    .unwrap()
                    .starts_with("git-tree:"));
                assert!(!Path::new(result["execution_root"].as_str().unwrap()).exists());
            }
        }
    }
}
