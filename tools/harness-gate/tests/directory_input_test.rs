#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use tempfile::TempDir;

fn fixture(script: &str) -> TempDir {
    let root = TempDir::new().unwrap();
    let initialized = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args(["init", "--preset", "generic"])
        .output()
        .unwrap();
    assert!(initialized.status.success(), "{initialized:?}");
    fs::write(root.path().join("input"), "candidate").unwrap();
    let flow = format!(
        r#"
version = 2
[project]
name = "directory-input"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "fail"
rules = [{{ patterns = ["**"], components = ["project"] }}]
[[steps]]
id = "project.check"
label = "check"
component = "project"
profiles = ["full"]
program = "/bin/sh"
args = ["-c", {script:?}]
cwd = "{{root}}"
log = "check.log"
timeout_secs = 5
"#
    );
    fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
    root
}

fn verify(root: &TempDir, bin: &TempDir) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args(["verify", "--all", "--snapshot"])
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env("PATH", bin.path())
        .output()
        .unwrap()
}

#[test]
fn real_core_binds_directory_content_without_git_or_a_publisher() {
    let root = fixture("test -f input");
    let bin = TempDir::new().unwrap();
    let marker = bin.path().join("called");
    for name in ["git", "gh", "herdr", "systemctl", "sudo"] {
        let path = bin.path().join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf forbidden > '{}'\nexit 99\n",
                marker.display()
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let output = verify(&root, &bin);
    assert!(output.status.success(), "{output:?}");
    assert!(!marker.exists());
    let result_path = root.path().join(".harness-gate/reports/test_result.json");
    let result: Value = serde_json::from_slice(&fs::read(&result_path).unwrap()).unwrap();
    assert_eq!(result["status"], "PASS");
    assert_eq!(result["input_mode"], "directory");
    assert!(result["source_identity"]
        .as_str()
        .unwrap()
        .starts_with("content-v1:sha256:"));
    let identity = result["source_identity"].clone();
    let git = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(git.status.success(), "{git:?}");
    let output = verify(&root, &bin);
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(&fs::read(result_path).unwrap()).unwrap();
    assert_eq!(result["source_identity"], identity);
    assert!(!marker.exists());
}

#[test]
fn mutated_materialized_input_cannot_publish_success() {
    let root = fixture("printf changed > input");
    let bin = TempDir::new().unwrap();
    let output = verify(&root, &bin);
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("materialized candidate changed"));
    assert_eq!(
        fs::read_to_string(root.path().join("input")).unwrap(),
        "candidate"
    );
    assert!(!root
        .path()
        .join(".harness-gate/reports/test_result.json")
        .exists());
}
