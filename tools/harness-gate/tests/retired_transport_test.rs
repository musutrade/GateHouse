#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use tempfile::TempDir;

const RETIRED: &str = r#"
[[notifications.webhooks]]
url = "ftp://retired.internal/hook"
allowed_hosts = ["retired.internal"]

[[notifications.webhooks]]
url = "https://example.test/hook"
allowed_hosts = ["*.example.test"]
on_failure = false
on_success = false

[[notifications.webhooks]]
url = "https://example.test/hook"
allowed_hosts = []
"#;

const CREDENTIAL: &str = r#"
[[notifications.webhooks]]
url = "https://user:secret@example.test/hook"
allowed_hosts = []
"#;

fn fixture(script: &str, retired: &str) -> TempDir {
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
    fs::write(&tool, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(tool, fs::Permissions::from_mode(0o755)).unwrap();
    let flow = format!(
        r#"
version = 2
[project]
name = "retired-transport"
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
{retired}
"#
    );
    fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
    root
}

fn verify(root: &TempDir, bin: &TempDir, marker: &std::path::Path) -> Output {
    fs::copy(
        root.path().join("check-fixture"),
        bin.path().join("check-fixture"),
    )
    .unwrap();
    for name in [
        "git",
        "gh",
        "herdr",
        "systemctl",
        "sudo",
        "curl",
        "wget",
        "nc",
    ] {
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

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn retired_transport_settings_do_not_gate_local_verification() {
    let root = fixture("test -f input", RETIRED);
    let bin = TempDir::new().unwrap();
    let marker = bin.path().join("called");
    let output = verify(&root, &bin, &marker);
    assert!(output.status.success(), "{output:?}");
    let text = combined(&output);
    assert!(text.contains("TEST_SUMMARY: PASS"), "{text}");
    assert!(!marker.exists(), "no external tool may run: {text}");
    assert!(
        text.contains("notifications.webhooks is deprecated"),
        "deprecation notice must still be emitted: {text}"
    );
    for leaked in [
        "ftp://retired.internal",
        "retired.internal",
        "*.example.test",
        "example.test",
        "notifications.webhooks[",
        "HGCFG",
    ] {
        assert!(
            !text.contains(leaked),
            "retired transport detail must not appear in output ({leaked}): {text}"
        );
    }
}

#[test]
fn real_step_failure_still_fails_without_retired_transport_errors() {
    let root = fixture("exit 1", RETIRED);
    let bin = TempDir::new().unwrap();
    let marker = bin.path().join("called");
    let output = verify(&root, &bin, &marker);
    assert!(
        !output.status.success(),
        "a real step failure must fail: {output:?}"
    );
    let text = combined(&output);
    assert!(text.contains("TEST_SUMMARY: FAIL"), "{text}");
    assert!(!marker.exists(), "no external tool may run: {text}");
    assert!(
        !text.contains("notifications.webhooks[") && !text.contains("HGCFG"),
        "retired settings must not produce configuration errors: {text}"
    );
}

#[test]
fn secret_scan_still_blocks_credential_urls_without_echoing_them() {
    let root = fixture("test -f input", CREDENTIAL);
    let bin = TempDir::new().unwrap();
    let marker = bin.path().join("called");
    let output = verify(&root, &bin, &marker);
    assert!(!output.status.success(), "{output:?}");
    let text = combined(&output);
    assert!(text.contains("SECRET_SCAN_FAILURE"), "{text}");
    assert!(
        !text.contains("user:secret"),
        "credential must never be echoed: {text}"
    );
    assert!(!text.contains("notifications.webhooks["), "{text}");
}
