#![cfg(unix)]

use serde_json::Value;
use std::{fs, process::Command};

#[test]
fn json_false_positive_cli_fails_with_incomplete_parser_report() {
    let root = tempfile::tempdir().unwrap();
    assert!(Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args(["init", "--preset", "generic"])
        .output()
        .unwrap()
        .status
        .success());
    fs::write(
        root.path().join(".harness-gate/flow.toml"),
        r#"
version = 2
[project]
name = "json-boundary"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
rules = [{ patterns = ["**"], components = ["project"] }]
[parsers.results]
kind = "json"
minimum = 1
[[steps]]
id = "project.json"
label = "JSON boundary"
component = "project"
profiles = ["full"]
program = "cat"
args = ["{root}/fixture.json"]
cwd = "{root}"
log = "json.log"
timeout_secs = 20
parser = "results"
"#,
    )
    .unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    for payload in [
        r#"{"error":"failed","duration":42}"#,
        r#"{"metadata":{"unrelated":[1,2]},"exit":0}"#,
        r#"{"results":[{}],"testcases":[{}]}"#,
        r#"{"count":7}"#,
    ] {
        fs::write(root.path().join("fixture.json"), payload).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .args(["--project-root"])
            .arg(root.path())
            .args(["verify", "--all"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{payload}");
        let report: Value = serde_json::from_slice(
            &fs::read(root.path().join(".harness-gate/reports/test_result.json")).unwrap_or_else(
                |error| {
                    panic!(
                        "{error}; stderr={}",
                        String::from_utf8_lossy(&output.stderr)
                    )
                },
            ),
        )
        .unwrap();
        assert_eq!(report["passed"], false, "{payload}");
        let step = report["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step["label"] == "JSON boundary")
            .unwrap();
        assert_eq!(step["passed"], false);
        assert_eq!(step["failure_code"], "RESULT_PARSE_FAILURE");
        assert_eq!(step["parser"]["complete"], false);
        assert_eq!(step["parser"]["observed"], 0);
    }
}
