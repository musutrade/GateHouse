#![cfg(unix)]
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn successful(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    successful(output);
}
fn read_report(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap())
        .unwrap()
}

#[test]
fn declared_cache_survives_real_config_cli_report_plan_and_clear_boundaries() {
    let temporary = tempfile::tempdir().unwrap();
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let project = temporary.path().join("project");
    fs::create_dir(&project).unwrap();
    let cache = temporary.path().join("host-cache");
    let cache_name = cache.to_str().unwrap();
    successful(cli(&project, &["init", "--preset", "generic"]));
    let program = fs::canonicalize("/bin/cat").unwrap();
    let flow = format!(
        r#"
version = 2
[project]
name = "cache-cli"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[policy]
required_steps = ["project.check"]
[scope]
unmatched = "all"
[[scope.rules]]
patterns = ["**"]
components = ["project"]
[[steps]]
id = "project.check"
label = "cache check"
component = "project"
profiles = ["full"]
program = {name:?}
args = ["source.txt"]
cwd = "{{root}}"
log = "check.log"
timeout_secs = 60
[steps.cache]
version = 1
deterministic = true
side_effect_free = true
inputs = []
outputs = []
toolchain = [{program:?}]
environment = []
"#,
        name = program.file_name().unwrap().to_str().unwrap(),
        program = program.to_str().unwrap()
    );
    fs::write(project.join(".harness-gate/flow.toml"), &flow).unwrap();
    fs::write(project.join(".gitignore"), ".harness-gate/reports/\n").unwrap();
    fs::write(project.join("source.txt"), "fixed-source\n").unwrap();
    for args in [
        &["init", "-q"][..],
        &["config", "user.name", "fixture"],
        &["config", "user.email", "fixture@example.invalid"],
        &["config", "maintenance.auto", "false"],
        &["config", "gc.auto", "0"],
        &["add", "."],
        &["commit", "-qm", "fixture"],
    ] {
        git(&project, args);
    }
    successful(cli(&project, &["config", "check"]));
    let plan: Value = serde_json::from_slice(&successful(cli(
        &project,
        &[
            "--step-cache-dir",
            cache_name,
            "verify",
            "--all",
            "--plan",
            "--json",
        ],
    )))
    .unwrap();
    assert_eq!(plan["verification_result"], Value::Null);
    assert!(
        !cache.exists(),
        "read-only plan must not provision host cache"
    );
    let command = ["--step-cache-dir", cache_name, "verify", "--all"];
    successful(cli(&project, &command));
    let first = read_report(&project);
    assert_eq!(first["steps"][2]["cache"]["status"], "MISS", "{first:#}");
    assert_eq!(first["steps"][2]["cache"]["reason"], "CACHE_ENTRY_ABSENT");
    successful(cli(&project, &command));
    let second = read_report(&project);
    assert_eq!(second["steps"][2]["cache"]["status"], "HIT");
    assert_eq!(second["passed"], true);
    assert_eq!(second["evidence_complete"], true);
    assert_ne!(first["invocation_id"], second["invocation_id"]);
    assert_eq!(second["steps"][0]["status"], "PASS");
    assert_eq!(second["steps"][1]["status"], "PASS");
    let log = second["steps"][2]["log"].as_str().unwrap();
    assert_eq!(fs::read_to_string(log).unwrap(), "fixed-source\n");
    successful(cli(&project, &["verify", "--all"]));
    assert_eq!(
        read_report(&project)["steps"][2]["cache"]["reason"],
        "CACHE_HOST_DISABLED"
    );
    let clear: Value = serde_json::from_slice(&successful(cli(
        &project,
        &["--step-cache-dir", cache_name, "cache-clear", "--json"],
    )))
    .unwrap();
    assert_eq!(clear["removed"], 1);
    assert_eq!(clear["retained"], 0);
}
