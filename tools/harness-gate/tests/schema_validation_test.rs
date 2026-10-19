#![cfg(unix)]

#[path = "common/schema.rs"]
mod schema_validator;

use serde_json::{json, Value};
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
name = "schema-fixture"
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

fn verify(root: &TempDir) -> Output {
    let bin = TempDir::new().unwrap();
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

fn fresh_result() -> Value {
    let root = fixture("test -f input");
    let output = verify(&root);
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(
        &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn real_directory_machine_result_satisfies_strict_schema() {
    let result = fresh_result();
    assert_eq!(result["input_mode"], "directory");
    schema_validator::validate_machine_result(&result).unwrap();
}

#[test]
fn strict_schema_rejects_unknown_fields_and_invalid_enums() {
    let result = fresh_result();
    let mut extra = result.clone();
    extra
        .as_object_mut()
        .unwrap()
        .insert("unexpected_extra".into(), json!(true));
    let error = schema_validator::validate_machine_result(&extra).unwrap_err();
    assert!(error.contains("additionalProperties"), "{error}");

    let mut wrong_enum = result.clone();
    wrong_enum["input_mode"] = json!("bogus-mode");
    let error = schema_validator::validate_machine_result(&wrong_enum).unwrap_err();
    assert!(error.contains("enum"), "{error}");

    let mut empty_status = result.clone();
    empty_status["status"] = json!("DONE");
    assert!(schema_validator::validate_machine_result(&empty_status).is_err());
}

#[test]
fn historical_results_without_identity_fields_stay_valid() {
    let result = fresh_result();
    assert!(result.get("identities").is_some());
    let mut legacy = result.clone();
    legacy.as_object_mut().unwrap().remove("identities");
    legacy["schema_version"] = json!("1");
    schema_validator::validate_machine_result(&legacy).unwrap();
}
