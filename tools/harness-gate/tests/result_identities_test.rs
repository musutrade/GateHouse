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
    let tool = root.path().join("check-fixture");
    fs::write(&tool, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(tool, fs::Permissions::from_mode(0o755)).unwrap();
    let flow = r#"
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
    root
}

fn run_verify(root: &TempDir, bin: &TempDir, snapshot: bool, isolated: bool) -> Output {
    fs::copy(
        root.path().join("check-fixture"),
        bin.path().join("check-fixture"),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
    command
        .arg("--project-root")
        .arg(root.path())
        .args(["verify", "--all"]);
    if snapshot {
        command.arg("--snapshot");
    }
    let path = if isolated {
        bin.path().display().to_string()
    } else {
        format!(
            "{}:{}",
            bin.path().display(),
            std::env::var("PATH").unwrap_or_default()
        )
    };
    command
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env("PATH", path)
        .output()
        .unwrap()
}

fn run_snapshot(root: &TempDir) -> Output {
    let bin = TempDir::new().unwrap();
    run_verify(root, &bin, true, true)
}

fn machine_result(root: &TempDir) -> Value {
    serde_json::from_slice(
        &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn identity_fields_are_explicit_and_isolated_without_quality() {
    let root = fixture("test -f input");
    let output = run_snapshot(&root);
    assert!(output.status.success(), "{output:?}");
    let result = machine_result(&root);
    let identities = &result["identities"];
    assert_eq!(result["input_mode"], "directory");
    assert!(identities["verification_policy_identity"]
        .as_str()
        .unwrap()
        .starts_with("verification-policy/v1:sha256:"));
    assert!(
        identities["quality_policy_identity"].is_null(),
        "no compiled quality identity must be recorded as null: {identities:#}"
    );
    assert_eq!(identities["policy_binding_complete"], Value::Bool(false));
    assert_eq!(identities["current_applicability"], "not_proven");
    assert!(identities["runtime_bundle_digest"]
        .as_str()
        .unwrap()
        .starts_with("runtime-bundle/v1:sha256:"));
    let facts = &identities["runtime"]["facts"];
    assert_eq!(facts["environment_binding"], "not_attested");
    assert_eq!(facts["isolation_attestation"], "not_provided");
    let core = fs::read(env!("CARGO_BIN_EXE_harness-gate")).unwrap();
    use sha2::Digest;
    assert_eq!(
        facts["core_sha256"],
        format!("sha256:{:x}", sha2::Sha256::digest(&core))
    );
}

#[test]
fn execution_and_policy_changes_move_different_identities() {
    let root = fixture("test -f input");
    let flow_path = root.path().join(".harness-gate/flow.toml");
    let baseline = run_snapshot(&root);
    assert!(baseline.status.success(), "{baseline:?}");
    let before = machine_result(&root);

    let flow = fs::read_to_string(&flow_path).unwrap();
    fs::write(
        &flow_path,
        flow.replace(
            "name = \"directory-input\"",
            "name = \"directory-input-renamed\"",
        ),
    )
    .unwrap();
    let renamed = run_snapshot(&root);
    assert!(renamed.status.success(), "{renamed:?}");
    let after_name = machine_result(&root);
    assert_ne!(
        after_name["identities"]["verification_policy_identity"],
        before["identities"]["verification_policy_identity"],
        "workflow policy intent (project name) must move the policy identity"
    );
    assert_eq!(
        after_name["identities"]["runtime_bundle_digest"],
        before["identities"]["runtime_bundle_digest"],
        "a name-only edit must not move the runtime identity"
    );

    let flow = fs::read_to_string(&flow_path).unwrap();
    fs::write(
        &flow_path,
        format!("{flow}\n[execution]\nmax_parallel = 2\n"),
    )
    .unwrap();
    let executed = run_snapshot(&root);
    assert!(executed.status.success(), "{executed:?}");
    let after_execution = machine_result(&root);
    assert_eq!(
        after_execution["identities"]["verification_policy_identity"],
        after_name["identities"]["verification_policy_identity"],
        "execution controls must not move the policy identity"
    );
    assert_ne!(
        after_execution["identities"]["runtime_bundle_digest"],
        after_name["identities"]["runtime_bundle_digest"],
        "execution controls must move the runtime identity"
    );
    assert_ne!(
        after_execution["identities"]["runtime"]["facts"]["configuration_digest"],
        after_name["identities"]["runtime"]["facts"]["configuration_digest"]
    );
}

#[test]
fn runtime_metadata_never_copies_raw_step_arguments() {
    let sentinel = "SENTINEL-CREDENTIAL-6f2a1c";
    let root = fixture("test -f input");
    let flow_path = root.path().join(".harness-gate/flow.toml");
    let flow = fs::read_to_string(&flow_path).unwrap();
    fs::write(
        &flow_path,
        flow.replace(
            "args = []",
            &format!("args = [\"--token\", \"{sentinel}\"]"),
        ),
    )
    .unwrap();
    let output = run_snapshot(&root);
    assert!(output.status.success(), "{output:?}");
    let raw =
        fs::read_to_string(root.path().join(".harness-gate/reports/test_result.json")).unwrap();
    assert!(
        !raw.contains(sentinel),
        "raw step arguments must not be copied into machine results"
    );
    let result = machine_result(&root);
    assert_eq!(result["status"], "PASS");
    assert!(result["identities"]["runtime"]["facts"]["core_sha256"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}

#[test]
fn builtin_policy_change_during_verification_is_rejected() {
    let root = fixture("printf 'x' >> .harness-gate/audit.toml\ntest -f input");
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "add",
            "-A",
        ],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(root.path())
            .status()
            .unwrap();
        assert!(status.success());
    }
    let bin = TempDir::new().unwrap();
    let output = run_verify(&root, &bin, false, false);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("built-in policy inputs changed during verification"),
        "{output:?}"
    );
}
