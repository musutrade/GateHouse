mod common;

use common::*;

#[cfg(unix)]
#[test]
fn test_verify_cancellation_stops_the_child_process() {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let ctx = TestContext::new();
    ctx.init_preset("generic");
    ctx.init_git();
    ctx.write_file(
        ".harness-gate/flow.toml",
        r#"
version = 2
[project]
name = "cancellation"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
rules = [{ patterns = ["**"], components = ["project"] }]
[[steps]]
id = "project.sleep"
label = "sleep"
component = "project"
profiles = ["full"]
program = "sleep"
args = ["10"]
cwd = "{root}"
log = "sleep.log"
timeout_secs = 60
"#,
    );
    let child = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["--color", "never", "--project-root"])
        .arg(&ctx.project_root)
        .args(["verify", "--all"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cancellation fixture");
    std::thread::sleep(Duration::from_secs(1));
    let status = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
    assert_eq!(status, 0, "send SIGTERM");
    let output = child.wait_with_output().expect("wait for cancellation");
    assert!(!output.status.success());
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("ERROR [E1402]"), "{combined}");
    assert!(
        ctx.project_root
            .join(".harness-gate/reports/test_result.json")
            .is_file(),
        "cancellation must retain a machine-readable report"
    );
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            ctx.project_root
                .join(".harness-gate/reports/test_result.json"),
        )
        .expect("read cancellation report"),
    )
    .expect("parse cancellation report");
    assert_eq!(report["passed"], false);
    assert!(report["steps"]
        .as_array()
        .is_some_and(|steps| { steps.iter().any(|step| step["cancelled"] == true) }));
}

#[test]
fn test_init_with_rust_api_preset() {
    let ctx = TestContext::new();

    // Run init with rust-api preset
    ctx.init_preset("rust-api");

    // Verify config files were created
    assert!(ctx.file_exists(".harness-gate/flow.toml"));
    assert!(ctx.file_exists(".harness-gate/secrets.toml"));
    assert!(ctx.file_exists(".harness-gate/audit.toml"));

    // Verify flow.toml content
    let flow_content = ctx.read_file(".harness-gate/flow.toml");
    assert!(flow_content.contains("version = 2"));
    assert!(flow_content.contains("[scope]"));
}

#[test]
fn test_config_check_valid() {
    let ctx = TestContext::new();

    // Initialize with preset
    ctx.init_preset("rust-api");

    // Run config check
    let output = ctx.run_harness_gate(&["config", "check"]);
    assert_success(&output);

    let stdout = stdout_str(&output);
    assert!(stdout.contains("valid") || stdout.contains("OK") || stdout.contains("✓"));
}

#[test]
fn test_config_check_invalid() {
    let ctx = TestContext::new();

    // Create invalid config
    ctx.write_file(".harness-gate/flow.toml", "invalid toml content {{");

    let output = ctx.run_harness_gate(&["config", "check"]);
    assert_failure(&output);

    let stderr = stderr_str(&output);
    assert!(stderr.contains("parse") || stderr.contains("invalid") || stderr.contains("error"));
}

#[test]
fn test_config_check_json_is_machine_readable_for_success_and_failure() {
    let ctx = TestContext::new();
    ctx.init_preset("generic");

    let valid = ctx.run_harness_gate(&["config", "check", "--format", "json"]);
    assert_success(&valid);
    let valid_json: serde_json::Value =
        serde_json::from_slice(&valid.stdout).expect("valid config JSON output");
    assert_eq!(valid_json["schema_version"], 1);
    assert_eq!(valid_json["valid"], true);
    assert_eq!(valid_json["diagnostics"], serde_json::json!([]));

    ctx.write_file(
        ".harness-gate/flow.toml",
        "version = 2\n[project]\nname = \"${HG_MISSING_CONFIG_VALUE}\"\n",
    );
    let invalid = ctx.run_harness_gate(&["config", "check", "--format", "json"]);
    assert_failure(&invalid);
    assert!(invalid.stderr.is_empty(), "stderr must remain JSON-free");
    let invalid_json: serde_json::Value =
        serde_json::from_slice(&invalid.stdout).expect("invalid config JSON output");
    assert_eq!(invalid_json["valid"], false);
    assert_eq!(invalid_json["diagnostics"][0]["id"], "HGCFG-INTERPOLATION");
    assert_eq!(invalid_json["diagnostics"][0]["location"]["line"], 3);
    assert!(invalid_json["diagnostics"][0]["help"]
        .as_str()
        .is_some_and(|help| help.contains("HG_MISSING_CONFIG_VALUE")));
}

#[test]
fn test_presets_lists_available() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("presets")
        .arg("--presets-dir")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat"))
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);

    // Should list at least rust-api preset
    assert!(stdout.contains("rust-api"));
}

#[test]
#[cfg(target_os = "linux")]
fn test_secrets_scan_basic() {
    let ctx = TestContext::new();

    // Initialize with preset
    ctx.init_preset("rust-api");
    ctx.init_git();

    // Create a file with no secrets
    ctx.write_file("test.txt", "Hello, world!");

    // Add and commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&ctx.project_root)
        .output()
        .ok();

    // Run secrets scan
    let output = ctx.run_harness_gate(&["secrets", "--staged"]);

    // Should succeed (no secrets found)
    assert_success(&output);
}

#[test]
fn test_scope_detection_all() {
    let ctx = TestContext::new();

    ctx.init_preset("rust-api");
    ctx.init_git();

    // Run scope with --all
    let output = ctx.run_harness_gate(&["scope", "--all"]);

    // Should succeed and show components
    assert_success(&output);
}

#[test]
fn test_verify_fails_without_git() {
    let ctx = TestContext::new();

    // Initialize but don't create git repo
    ctx.run_harness_gate(&["init", "--preset", "rust-api"]);

    // Try to verify
    let output = ctx.run_harness_gate(&["verify"]);

    // Should fail because no git repo
    assert_failure(&output);
}

#[test]
fn test_init_creates_gitignore_entry() {
    let ctx = TestContext::new();

    // Run init
    ctx.init_preset("rust-api");

    // Check if .gitignore was created or updated
    if ctx.file_exists(".gitignore") {
        let gitignore = ctx.read_file(".gitignore");
        assert!(gitignore.contains(".harness-gate/reports") || gitignore.contains("reports"));
    }
}

#[test]
fn test_init_twice_fails() {
    let ctx = TestContext::new();

    // First init should succeed
    ctx.init_preset("rust-api");

    // Second init should fail (already initialized)
    let output = ctx.run_harness_gate(&["init", "--preset", "rust-api"]);
    assert_failure(&output);

    let stderr = stderr_str(&output);
    assert!(stderr.contains("already") || stderr.contains("exists"));
}
