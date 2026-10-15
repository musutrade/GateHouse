use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn cli(root: &Path, args: &[&str], success: bool) {
    let result = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        result.status.success(),
        success,
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn completed_core_report_to_findings_history_and_sarif_cli() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    cli(&root, &["init", "--preset", "generic"], true);
    fs::write(
        root.join(".harness-gate/flow.toml"),
        r#"
version = 2
[project]
name = "findings-cli"
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
label = "check"
component = "project"
profiles = ["full"]
program = "git"
args = ["--version"]
cwd = "{root}"
log = "check.log"
timeout_secs = 30
"#,
    )
    .unwrap();
    fs::write(root.join(".gitignore"), ".harness-gate/reports/\n").unwrap();
    fs::write(root.join("source.txt"), "source\n").unwrap();
    for args in [
        &["init", "-q"][..],
        &["config", "user.name", "fixture"],
        &["config", "user.email", "fixture@example.invalid"],
        &["add", "."],
        &["commit", "-qm", "fixture"],
    ] {
        git(&root, args);
    }
    cli(&root, &["verify", "--staged"], true);
    let core_path = root.join(".harness-gate/reports/test_result.json");
    let original = fs::read(&core_path).unwrap();
    let core: Value = serde_json::from_slice(&original).unwrap();
    assert!(core["source_identity"]
        .as_str()
        .unwrap()
        .starts_with("git-tree:"));
    let mut bundle = json!({"schema_version":"harness-findings/1", "context": {
        "repository":"owner/repo", "commit":git(&root, &["rev-parse", "HEAD"]),
        "tree":git(&root, &["rev-parse", "HEAD^{tree}"]), "series":"test/1"},
        "report_sha256":digest(&original), "sources":[{"format":"native/1", "measurement_id":"run-1", "complete":true,
            "document":{"schema_version":"native/1", "tool":"sample", "tool_version":"1", "findings":[{
                "rule":"rule-1", "anchor":"symbol-1", "severity":"error", "message":"review this source",
                "locations":[{"path":"source.txt", "line":1, "end_line":1}]}]}}]});
    fs::write(
        root.join("bundle.json"),
        serde_json::to_vec(&bundle).unwrap(),
    )
    .unwrap();
    let core_name = core_path.to_str().unwrap();
    cli(
        &root,
        &[
            "findings",
            "normalize",
            "--input",
            "bundle.json",
            "--report",
            core_name,
            "--output",
            "first.json",
        ],
        true,
    );
    // Existing evidence is immutable, including the original Core report.
    cli(
        &root,
        &[
            "findings",
            "normalize",
            "--input",
            "bundle.json",
            "--report",
            core_name,
            "--output",
            "first.json",
        ],
        false,
    );
    bundle["context"]["commit"] = json!("c".repeat(40));
    bundle["sources"][0]["document"]["findings"][0]["locations"][0]["line"] = json!(2);
    bundle["sources"][0]["document"]["findings"][0]["locations"][0]["end_line"] = json!(2);
    fs::write(
        root.join("bundle.json"),
        serde_json::to_vec(&bundle).unwrap(),
    )
    .unwrap();
    cli(
        &root,
        &[
            "findings",
            "normalize",
            "--input",
            "bundle.json",
            "--report",
            core_name,
            "--output",
            "next.json",
        ],
        true,
    );
    let pin = digest(&fs::read(root.join("first.json")).unwrap());
    cli(
        &root,
        &[
            "findings",
            "compare",
            "--current",
            "next.json",
            "--previous",
            "first.json",
            "--previous-sha256",
            &"0".repeat(64),
            "--output",
            "bad.json",
        ],
        false,
    );
    assert!(!root.join("bad.json").exists());
    cli(
        &root,
        &[
            "findings",
            "compare",
            "--current",
            "next.json",
            "--previous",
            "first.json",
            "--previous-sha256",
            &pin,
            "--output",
            "compared.json",
        ],
        true,
    );
    let compared: Value =
        serde_json::from_slice(&fs::read(root.join("compared.json")).unwrap()).unwrap();
    assert_eq!(compared["history"][0]["state"], "persistent");
    assert_eq!(compared["previous_sha256"], pin);
    cli(
        &root,
        &[
            "findings",
            "export",
            "--input",
            "compared.json",
            "--output",
            "output.sarif",
        ],
        true,
    );
    let sarif: Value =
        serde_json::from_slice(&fs::read(root.join("output.sarif")).unwrap()).unwrap();
    assert_eq!(sarif["runs"][0]["results"][0]["level"], "error");
    assert_eq!(compared["core_status"], "PASS"); // tool severity cannot replace Core decision.
    assert_eq!(fs::read(&core_path).unwrap(), original);
    cli(
        &root,
        &["findings", "schema", "--output", "report-schema.json"],
        true,
    );
    cli(
        &root,
        &[
            "findings",
            "schema",
            "--bundle",
            "--output",
            "bundle-schema.json",
        ],
        true,
    );
    assert_eq!(
        fs::read(root.join("report-schema.json")).unwrap(),
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema/findings.schema.json"))
            .unwrap()
    );
    assert_eq!(
        fs::read(root.join("bundle-schema.json")).unwrap(),
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema/findings-bundle.schema.json")
        )
        .unwrap()
    );
}
