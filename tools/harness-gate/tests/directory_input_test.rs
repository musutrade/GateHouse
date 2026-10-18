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

fn copy_evidence(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_evidence(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn saved_result_verifies_after_source_cleanup_and_rejects_tampering() {
    use sha2::{Digest, Sha256};
    let root = fixture("printf measured");
    let bin = TempDir::new().unwrap();
    let output = verify(&root, &bin);
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(
        &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap();
    let invocation = result["invocation_id"].as_str().unwrap();
    let archive = TempDir::new().unwrap();
    let saved = archive.path().join("saved");
    copy_evidence(
        &root
            .path()
            .join(".harness-gate/reports/invocations")
            .join(invocation),
        &saved,
    );
    let pin = |name| format!("{:x}", Sha256::digest(fs::read(saved.join(name)).unwrap()));
    let index = archive.path().join("index.json");
    fs::write(&index, serde_json::to_vec(&serde_json::json!({
        "schema":"harness-report-history/v1",
        "samples":[{"directory":"saved", "machine_sha256":pin("test_result.json"), "manifest_sha256":pin("manifest.json")}]
    })).unwrap()).unwrap();
    let removed_root = root.path().to_path_buf();
    root.close().unwrap();
    assert!(!removed_root.exists());
    let read = || {
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .args(["report-verify", "--input"])
            .arg(&index)
            .arg("--project-root")
            .arg(&removed_root)
            .env("PATH", bin.path())
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN")
            .output()
            .unwrap()
    };
    let output = read();
    assert!(output.status.success(), "{output:?}");
    let verified: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(verified["historical_integrity"], "verified");
    assert_eq!(verified["current_applicability"], "not_evaluated");
    assert_eq!(verified["result"], result);
    fs::write(saved.join("logs/check.log"), "tampered").unwrap();
    assert!(!read().status.success());
}

#[test]
fn configured_result_consumer_is_not_a_verification_completion_step() {
    use std::net::TcpListener;
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    for (script, expected) in [("exit 0", "PASS"), ("exit 1", "FAIL")] {
        let root = fixture(script);
        let path = root.path().join(".harness-gate/flow.toml");
        let mut flow = fs::read_to_string(&path).unwrap();
        flow.push_str(&format!(
            "\n[[notifications.webhooks]]\nurl = \"http://{address}/unavailable\"\nallowed_hosts = [\"127.0.0.1\"]\non_success = true\non_failure = true\n"
        ));
        fs::write(path, flow).unwrap();
        let bin = TempDir::new().unwrap();
        let output = verify(&root, &bin);
        assert_eq!(output.status.success(), expected == "PASS", "{output:?}");
        let result: Value = serde_json::from_slice(
            &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(result["status"], expected);
        assert!(String::from_utf8_lossy(&output.stderr).contains("external consumer"));
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }
}
