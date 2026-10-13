mod common;

use common::*;
use std::process::Command;

#[test]
fn test_help_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);
    assert!(stdout.contains("harness-gate"));
    assert!(stdout.contains("Usage:"));
}

#[test]
fn test_version_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);
    assert!(stdout.contains("harness-gate"));
    assert_eq!(
        stdout.trim(),
        concat!("harness-gate ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_invalid_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("invalid-subcommand")
        .output()
        .expect("Failed to execute command");

    assert_failure(&output);
    let stderr = stderr_str(&output);
    assert!(stderr.contains("unrecognized subcommand") || stderr.contains("unexpected argument"));
}

#[test]
fn test_presets_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("presets")
        .output()
        .expect("Failed to execute command");

    assert_success(&output);
    let stdout = stdout_str(&output);

    // Should list available presets
    assert!(stdout.contains("generic"));
}

#[test]
fn test_schema_export_writes_the_flow_schema_without_a_project_config() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["schema", "export"]);

    assert_success(&output);
    let schema = ctx.read_file("schema/flow.schema.json");
    assert!(schema.contains("\"title\": \"FlowConfig\""));
}

#[test]
fn test_init_with_invalid_preset() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["init", "--preset", "nonexistent-preset"]);

    assert_failure(&output);
    let stderr = stderr_str(&output);
    assert!(
        stderr.contains("preset") || stderr.contains("not found") || stderr.contains("unknown")
    );
}

#[test]
fn test_verify_without_project() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["verify", "--all"]);

    assert_failure(&output);
    let stderr = stderr_str(&output);
    // Should error about missing config or not being in a project
    assert!(
        stderr.contains("not found")
            || stderr.contains("No such file")
            || stderr.contains("config")
    );
}

#[test]
fn test_config_check_without_config() {
    let ctx = TestContext::new();
    let output = ctx.run_harness_gate(&["config", "check"]);

    // Should fail when no config exists
    assert_failure(&output);
    let stderr = stderr_str(&output);
    assert!(stderr.contains("harness-gate init --preset generic"));
    assert!(stderr.contains("version = 2"));
}

#[test]
fn test_legacy_report_dir_alias_warns_and_new_namespace_wins() {
    let ctx = TestContext::new();
    ctx.init_preset("generic");

    let legacy = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["--color", "never", "config", "print", "--resolved"])
        .arg("--project-root")
        .arg(&ctx.project_root)
        .env("REPORT_DIR", ".harness-gate/legacy-reports")
        .env_remove("HARNESS_GATE_REPORTS")
        .output()
        .expect("run config print with legacy report alias");
    assert_success(&legacy);
    assert!(stdout_str(&legacy).contains(".harness-gate/legacy-reports"));
    assert!(stderr_str(&legacy).contains("REPORT_DIR is deprecated"));

    let current = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["--color", "never", "config", "print", "--resolved"])
        .arg("--project-root")
        .arg(&ctx.project_root)
        .env("REPORT_DIR", ".harness-gate/legacy-reports")
        .env("HARNESS_GATE_REPORTS", ".harness-gate/current-reports")
        .output()
        .expect("run config print with both report aliases");
    assert_success(&current);
    assert!(stdout_str(&current).contains(".harness-gate/current-reports"));
}

#[test]
fn test_scope_without_git() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));
    let output = ctx.run_harness_gate(&["scope"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1301]"));
}

#[test]
fn test_audit_reports_a_typed_configuration_error() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));
    ctx.write_file(
        ".harness-gate/audit.toml",
        "version = 999\n[engine]\nignore_filename = \".gitignore\"\n",
    );

    let output = ctx.run_harness_gate(&["audit"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1101]"));
}

#[test]
fn test_rust_and_python_projects_use_their_own_audit_configuration() {
    let rust = TestContext::new();
    let python = TestContext::new();
    rust.init_preset("generic");
    python.init_preset("generic");

    rust.write_file("src/lib.rs", "pub fn ready() {}\n");
    python.write_file("app/main.py", "def ready():\n    return True\n");
    let rust_flow = rust
        .read_file(".harness-gate/flow.toml")
        .replace(".harness-gate/audit.toml", "policies/rust-audit.toml");
    rust.write_file(".harness-gate/flow.toml", &rust_flow);
    let python_flow = python
        .read_file(".harness-gate/flow.toml")
        .replace(".harness-gate/audit.toml", "policies/python-audit.toml");
    python.write_file(".harness-gate/flow.toml", &python_flow);

    let rust_audit = rust
        .read_file(".harness-gate/audit.toml")
        .replace("review_context.json", "rust-review-context.json")
        + r#"

[[hard_rules]]
name = "Rust unsafe policy"
severity = "blocker"
paths = ["src"]
extensions = ["rs"]
patterns = ["unsafe\\s*\\{"]
"#;
    rust.write_file("policies/rust-audit.toml", &rust_audit);
    let python_audit = python
        .read_file(".harness-gate/audit.toml")
        .replace("review_context.json", "python-review-context.json")
        + r#"

[engine.comment_syntax.py]
line = ['#']
strings = [
  { start = "'", end = "'", escape = "\\" },
  { start = "\"", end = "\"", escape = "\\" },
]

[[hard_rules]]
name = "Python eval policy"
severity = "blocker"
paths = ["app"]
extensions = ["py"]
patterns = ["eval\\s*\\("]
"#;
    python.write_file("policies/python-audit.toml", &python_audit);

    let rust_output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["--color", "never", "audit"])
        .current_dir(&rust.project_root)
        .env("PROJECT_ROOT", &python.project_root)
        .env(
            "HARNESS_GATE_CONFIG",
            python.project_root.join(".harness-gate/flow.toml"),
        )
        .env(
            "AUDITOR_CONFIG",
            python.project_root.join("policies/python-audit.toml"),
        )
        .env(
            "HARNESS_GATE_AUDIT_CONFIG",
            python.project_root.join("policies/python-audit.toml"),
        )
        .output()
        .expect("run audit for the Rust project");
    assert_success(&rust_output);
    assert!(
        stdout_str(&rust_output).contains("rust-review-context.json"),
        "{}",
        stdout_str(&rust_output)
    );
    assert!(rust.file_exists(".harness-gate/reports/rust-review-context.json"));
    assert!(!python.file_exists(".harness-gate/reports/rust-review-context.json"));

    let python_output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["--color", "never", "audit"])
        .current_dir(&python.project_root)
        .env("PROJECT_ROOT", &rust.project_root)
        .env(
            "HARNESS_GATE_CONFIG",
            rust.project_root.join(".harness-gate/flow.toml"),
        )
        .env(
            "AUDITOR_CONFIG",
            rust.project_root.join("policies/rust-audit.toml"),
        )
        .env(
            "HARNESS_GATE_AUDIT_CONFIG",
            rust.project_root.join("policies/rust-audit.toml"),
        )
        .output()
        .expect("run audit for the Python project");
    assert_success(&python_output);
    assert!(
        stdout_str(&python_output).contains("python-review-context.json"),
        "{}",
        stdout_str(&python_output)
    );
    assert!(python.file_exists(".harness-gate/reports/python-review-context.json"));
    assert!(!rust.file_exists(".harness-gate/reports/python-review-context.json"));
}

#[test]
fn test_secrets_reports_a_typed_configuration_error() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));
    ctx.init_git();
    ctx.write_file(
        ".harness-gate/secrets.toml",
        "version = 2\nrules = []\n[placeholders]\nminimum_unique_characters = 4\nmaximum_nonalphanumeric_characters = 2\nprefixes = []\nmarkers = []\nexact = []\n",
    );

    let output = ctx.run_harness_gate(&["secrets"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1201]"));
}

#[test]
fn test_verify_reports_a_typed_selection_error() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));

    let output = ctx.run_harness_gate(&["verify", "--all", "--profile", "missing"]);

    assert_failure(&output);
    assert!(stderr_str(&output).contains("ERROR [E1401]"));
}

#[test]
fn test_doctor_basic() {
    let ctx = TestContext::new();

    // Create minimal config to avoid error
    ctx.write_file(
        ".harness-gate/flow.toml",
        r#"
version = 2
[scope]
default = []
"#,
    );

    let output = ctx.run_harness_gate(&["doctor"]);

    // Doctor should run (might report issues but shouldn't crash)
    // We don't assert success because it depends on system state
    let stderr = stderr_str(&output);
    assert!(!stderr.contains("panic") && !stderr.contains("thread panicked"));
}

#[test]
fn test_cleanup_dry_run_writes_machine_readable_evidence() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["init", "--preset", "generic"]));

    let output = ctx.run_harness_gate(&["cleanup", "--dry-run", "--json"]);
    assert_success(&output);
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cleanup JSON output");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["owner_marker"], "harness-gate");
    assert_eq!(report["dry_run"], true);
    assert!(ctx.file_exists(".harness-gate/reports/cleanup.json"));
}

#[test]
fn test_color_always_styles_human_readable_output() {
    let ctx = TestContext::new();
    ctx.write_file(
        ".harness-gate/flow.toml",
        "version = 2\n[scope]\ndefault = []\n",
    );

    let output = ctx.run_harness_gate(&["--color", "always", "doctor"]);

    assert!(stdout_str(&output).contains("\x1b[") || stderr_str(&output).contains("\x1b["));
}

#[test]
fn test_color_never_keeps_human_readable_output_plain() {
    let ctx = TestContext::new();
    ctx.write_file(
        ".harness-gate/flow.toml",
        "version = 2\n[scope]\ndefault = []\n",
    );

    let output = ctx.run_harness_gate(&["--color", "never", "doctor"]);

    assert!(!stdout_str(&output).contains("\x1b["));
    assert!(!stderr_str(&output).contains("\x1b["));
}

#[test]
fn quality_config_commands_cross_validate_both_planes() {
    let ctx = TestContext::new();
    // Start flow-only; the reference ecosystem presets now explicitly enable quality.
    ctx.init_preset("generic");
    ctx.write_file(
        ".harness-gate/flow.toml",
        include_str!("../fixtures/preset-compat/rust-api.flow.toml"),
    );
    let legacy = ctx.run_harness_gate(&["config", "print", "--resolved"]);
    assert_success(&legacy);
    let raw_flow = ctx.run_harness_gate(&["config", "print"]);
    assert_success(&raw_flow);
    assert_eq!(
        stdout_str(&raw_flow),
        include_str!("../fixtures/preset-compat/rust-api.flow.toml")
    );
    assert_failure(&ctx.run_harness_gate(&["config", "print", "--quality"]));
    let quality = include_str!("../fixtures/workflow/quality.toml");
    ctx.write_file(".harness-gate/quality.toml", quality);
    ctx.write_file(
        ".harness-gate/policy.json",
        include_str!("../fixtures/workflow/policy.json"),
    );
    assert_success(&ctx.run_harness_gate(&["config", "check"]));
    assert_success(&ctx.run_harness_gate(&["config", "check", "--format", "json"]));
    let printed = ctx.run_harness_gate(&["config", "print", "--quality"]);
    assert_success(&printed);
    assert_eq!(stdout_str(&printed).trim(), quality.trim());
    assert_success(&ctx.run_harness_gate(&["config", "print", "--quality", "--resolved"]));
    assert_eq!(
        stdout_str(&legacy),
        stdout_str(&ctx.run_harness_gate(&["config", "print", "--resolved"]))
    );
    let raw_flow_with_quality = ctx.run_harness_gate(&["config", "print"]);
    assert_success(&raw_flow_with_quality);
    assert_eq!(stdout_str(&raw_flow), stdout_str(&raw_flow_with_quality));
    for invalid in [
        quality.replace(
            "flow_components = [\"app\"]",
            "flow_components = [\"missing\"]",
        ),
        quality.replace(
            "[collectors.coverage]",
            "[collectors.coverage]\nrequired = true",
        ),
    ] {
        ctx.write_file(".harness-gate/quality.toml", &invalid);
        assert_failure(&ctx.run_harness_gate(&["config", "check"]));
        assert_failure(&ctx.run_harness_gate(&["config", "print"]));
        let output = ctx.run_harness_gate(&["config", "check", "--format", "json"]);
        assert_failure(&output);
        let report: serde_json::Value = serde_json::from_str(&stdout_str(&output)).unwrap();
        assert!(report.to_string().contains("HGCFG-QUALITY"));
    }
}

#[test]
fn quality_schema_exports_without_project_or_enabling_quality() {
    let ctx = TestContext::new();
    assert_success(&ctx.run_harness_gate(&["schema", "export", "--quality"]));
    let schema: serde_json::Value =
        serde_json::from_str(&ctx.read_file("schema/quality.schema.json")).unwrap();
    assert_eq!(schema["title"], "QualityConfig");
    assert_success(&ctx.run_harness_gate(&[
        "schema",
        "export",
        "--quality",
        "--output",
        "custom.json",
    ]));
    assert_eq!(
        ctx.read_file("custom.json"),
        ctx.read_file("schema/quality.schema.json")
    );
    assert_failure(&ctx.run_harness_gate(&[
        "schema",
        "export",
        "--quality",
        "--output",
        "../escape.json",
    ]));
    ctx.init_preset("generic");
    assert_success(&ctx.run_harness_gate(&["config", "check"]));
    assert_failure(&ctx.run_harness_gate(&["config", "print", "--quality"]));
}

#[test]
fn external_catalog_cli_precedence_and_missing_directory_fail_closed() {
    let catalog = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat");
    let binary = env!("CARGO_BIN_EXE_harness-gate");
    let builtin = Command::new(binary)
        .arg("presets")
        .env_remove("HARNESS_GATE_PRESETS_DIR")
        .output()
        .unwrap();
    assert_success(&builtin);
    assert!(!stdout_str(&builtin).contains("rust-api"));
    let external = Command::new(binary)
        .arg("presets")
        .env("HARNESS_GATE_PRESETS_DIR", &catalog)
        .output()
        .unwrap();
    assert_success(&external);
    assert!(stdout_str(&external).contains("rust-api"));
    let missing = catalog.join("missing");
    let explicit = Command::new(binary)
        .args(["presets", "--presets-dir"])
        .arg(&catalog)
        .env("HARNESS_GATE_PRESETS_DIR", &missing)
        .output()
        .unwrap();
    assert_success(&explicit);
    let invalid = Command::new(binary)
        .args(["presets", "--presets-dir"])
        .arg(&missing)
        .env("HARNESS_GATE_PRESETS_DIR", &catalog)
        .output()
        .unwrap();
    assert_failure(&invalid);
    let ctx = TestContext::new();
    let no_catalog = Command::new(binary)
        .arg("--project-root")
        .arg(&ctx.project_root)
        .args(["init", "--preset", "rust-api"])
        .env_remove("HARNESS_GATE_PRESETS_DIR")
        .output()
        .unwrap();
    assert_failure(&no_catalog);
    assert!(!ctx.project_root.join(".harness-gate/flow.toml").exists());
}
