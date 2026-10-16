use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

fn success(command: &mut Command) -> Vec<u8> {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn history_cli_reads_real_completed_reports_without_running_verification() {
    let workspace = tempfile::tempdir().unwrap();
    let project = workspace.path().join("project");
    fs::create_dir(&project).unwrap();
    let binary = env!("CARGO_BIN_EXE_harness-gate");
    success(
        Command::new(binary)
            .arg("--project-root")
            .arg(&project)
            .arg("init"),
    );
    success(
        Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(["init", "-q"]),
    );
    fs::write(
        project.join(".harness-gate/flow.toml"),
        r#"
version = 2
[project]
name = "history-test"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
[[scope.rules]]
patterns = ["**"]
components = ["core"]
[[steps]]
id = "git-version"
label = "git version"
component = "core"
profiles = ["full"]
program = "git"
args = ["--version"]
cwd = "{root}"
log = "git.log"
timeout_secs = 10
"#,
    )
    .unwrap();
    success(
        Command::new(binary)
            .arg("--project-root")
            .arg(&project)
            .args(["verify", "--all"]),
    );
    let reports = project.join(".harness-gate/reports");
    let machine_bytes = fs::read(reports.join("test_result.json")).unwrap();
    let machine: Value = serde_json::from_slice(&machine_bytes).unwrap();
    assert_eq!(machine["evidence_complete"], true);
    let reports = std::path::PathBuf::from(machine["report_directory"].as_str().unwrap());
    let index = project.join("history.json");
    fs::write(&index, serde_json::to_vec(&json!({
        "schema":"harness-report-history/v1", "samples":[{
            "directory":reports.strip_prefix(&project).unwrap().to_string_lossy().replace('\\', "/"),
            "machine_sha256":format!("{:x}", Sha256::digest(&machine_bytes)),
            "manifest_sha256":format!("{:x}", Sha256::digest(fs::read(reports.join("manifest.json")).unwrap()))
        }]
    })).unwrap()).unwrap();
    let display = workspace.path().join("history-display");
    let run = |output: &Path| {
        Command::new(binary)
            .current_dir(workspace.path())
            .args(["report-history", "--input"])
            .arg(&index)
            .arg("--output")
            .arg(output)
            .args([
                "--policy",
                "absent",
                "--subject",
                "absent",
                "--metric",
                "coverage.line",
            ])
            .output()
            .unwrap()
    };
    let output = run(&display);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let html = fs::read_to_string(display.join("history.html")).unwrap();
    assert!(html.contains("quality was not collected"));
    assert!(!html.contains("<circle"));
    assert_eq!(
        fs::read(reports.join("test_result.json")).unwrap(),
        machine_bytes
    );
    assert!(!run(&display).status.success());
    assert!(!run(&project.join("forbidden-output")).status.success());
    assert!(!project.join("forbidden-output").exists());
    let mut index_value: Value = serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
    let duplicate = index_value["samples"][0].clone();
    index_value["samples"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    fs::write(&index, serde_json::to_vec(&index_value).unwrap()).unwrap();
    let rejected = workspace.path().join("rejected");
    assert!(!run(&rejected).status.success());
    assert!(!rejected.exists());
}
