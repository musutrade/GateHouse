#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use tempfile::TempDir;

fn fixture(source_repository: &str) -> TempDir {
    let root = TempDir::new().unwrap();
    let initialized = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args(["init", "--preset", "generic"])
        .output()
        .unwrap();
    assert!(initialized.status.success(), "{initialized:?}");
    fs::write(root.path().join("input"), "candidate").unwrap();
    let tool = root.path().join("check-fixture");
    fs::write(&tool, "#!/bin/sh\ntest -f input\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(root.path().join("templates")).unwrap();
    fs::write(
        root.path().join("templates/verification.html"),
        "<!doctype html><html><body><p>{{ summary }}</p><p>{{ report.status }}</p></body></html>\n",
    )
    .unwrap();
    let flow = format!(
        r#"
version = 2
[project]
name = "retired-source-navigation"
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
program = "check-fixture"
args = []
cwd = "{{root}}"
log = "check.log"
timeout_secs = 5
[report_templates]
root = "templates"
template = "templates/verification.html"
source_repository = "{source_repository}"
"#
    );
    fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
    root
}

fn verify(root: &TempDir, bin: &TempDir) -> Output {
    fs::copy(
        root.path().join("check-fixture"),
        bin.path().join("check-fixture"),
    )
    .unwrap();
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
fn retired_non_github_value_renders_locally_without_git_or_echo() {
    let retired = "https://gitlab.example/owner/project-retired-navigation-sentinel";
    let root = fixture(retired);
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
    assert!(
        !marker.exists(),
        "rendering invoked a platform or host tool"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("report_templates.source_repository is deprecated"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("retired-navigation-sentinel") && !stderr.contains("gitlab.example"),
        "{stderr}"
    );
    let html =
        fs::read_to_string(root.path().join(".harness-gate/reports/test_result.html")).unwrap();
    assert!(html.contains("PASS"), "{html}");
    assert!(
        !html.contains("retired-navigation-sentinel") && !html.contains("gitlab.example"),
        "{html}"
    );
    assert!(!html.contains("class=\"source-link\""), "{html}");
    let result: Value = serde_json::from_slice(
        &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["status"], "PASS");
}
