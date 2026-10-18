#![cfg(unix)]

#[path = "common/schema.rs"]
mod schema;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

fn fixture() -> TempDir {
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
    fs::set_permissions(tool, fs::Permissions::from_mode(0o755)).unwrap();
    let flow = r#"
version = 2
[project]
name = "index-input"
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
program = "check-fixture"
args = []
cwd = "{root}"
log = "check.log"
timeout_secs = 5
"#;
    fs::write(root.path().join(".harness-gate/flow.toml"), flow).unwrap();
    git(root.path(), &["init", "--quiet"]);
    git(root.path(), &["add", "-A"]);
    git(
        root.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    );
    root
}

fn verify(root: &TempDir, bin: &TempDir, extra: &[&str]) -> Output {
    fs::copy(
        root.path().join("check-fixture"),
        bin.path().join("check-fixture"),
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args(["verify", "--all", "--snapshot"])
        .args(extra)
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.path().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .unwrap()
}

fn machine_result(root: &TempDir) -> Value {
    serde_json::from_slice(
        &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn index_content_matches_directory_identity_and_ignores_unstaged_flow_edits() {
    let root = fixture();
    let bin = TempDir::new().unwrap();
    let output = verify(&root, &bin, &["--snapshot-source", "index"]);
    assert!(output.status.success(), "{output:?}");
    let index_result = machine_result(&root);
    schema::validate_machine_result(&index_result).unwrap();
    assert_eq!(index_result["status"], "PASS");
    assert_eq!(index_result["input_mode"], "content-index");
    let identity = index_result["source_identity"].clone();
    assert!(identity.as_str().unwrap().starts_with("content-v1:sha256:"));

    let output = verify(&root, &bin, &[]);
    assert!(output.status.success(), "{output:?}");
    let directory_result = machine_result(&root);
    schema::validate_machine_result(&directory_result).unwrap();
    assert_eq!(directory_result["input_mode"], "directory");
    assert_eq!(directory_result["source_identity"], identity);

    fs::write(root.path().join(".harness-gate/flow.toml"), "not = [valid").unwrap();
    let output = verify(&root, &bin, &["--snapshot-source", "index"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(machine_result(&root)["source_identity"], identity);

    fs::remove_file(root.path().join(".harness-gate/reports/test_result.json")).unwrap();
    let output = verify(&root, &bin, &[]);
    assert!(!output.status.success(), "{output:?}");
    assert!(!root
        .path()
        .join(".harness-gate/reports/test_result.json")
        .exists());
}

#[test]
fn index_snapshot_supports_explicit_destination_and_rejects_existing() {
    let root = fixture();
    let bin = TempDir::new().unwrap();
    let host = TempDir::new().unwrap();
    let destination = host.path().join("known-index-destination");
    let output = verify(
        &root,
        &bin,
        &[
            "--snapshot-source",
            "index",
            "--snapshot-destination",
            destination.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(machine_result(&root)["input_mode"], "content-index");
    assert!(!destination.exists());

    let existing = host.path().join("existing-index-destination");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("sentinel"), "preserve").unwrap();
    let output = verify(
        &root,
        &bin,
        &[
            "--snapshot-source",
            "index",
            "--snapshot-destination",
            existing.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("new directory"));
    assert_eq!(
        fs::read_to_string(existing.join("sentinel")).unwrap(),
        "preserve"
    );
}

#[test]
fn directory_snapshot_destination_requires_new_external_absolute_path() {
    let root = fixture();
    let bin = TempDir::new().unwrap();
    let host = TempDir::new().unwrap();
    let destination = host.path().join("known-directory-destination");
    let output = verify(
        &root,
        &bin,
        &["--snapshot-destination", destination.to_str().unwrap()],
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(machine_result(&root)["input_mode"], "directory");
    assert!(!destination.exists());

    let output = verify(
        &root,
        &bin,
        &["--snapshot-destination", "relative-destination"],
    );
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("absolute"));
    assert!(!root.path().join("relative-destination").exists());

    let inside = root.path().join("inside-destination");
    let output = verify(
        &root,
        &bin,
        &["--snapshot-destination", inside.to_str().unwrap()],
    );
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("separate from candidate input"));
    assert!(!inside.exists());
}
