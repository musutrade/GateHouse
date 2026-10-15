use serde_json::Value;
use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

fn check(root: &TempDir, color: &str, json: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
    command
        .arg("--project-root")
        .arg(root.path())
        .args(["--color", color, "config", "check"])
        .env("NO_COLOR", "1");
    if json {
        command.args(["--format", "json"]);
    }
    command.output().unwrap()
}

fn malformed_config() -> TempDir {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join(".harness-gate")).unwrap();
    fs::write(
        root.path().join(".harness-gate/flow.toml"),
        "version = 2\n[project]\nname = [\npassword = 'not-for-terminal-display'\n",
    )
    .unwrap();
    root
}

#[test]
fn plain_config_error_has_location_help_and_no_source_values() {
    let root = malformed_config();
    let output = check(&root, "never", false);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("error[HGCFG-TOML-PARSE]: configuration is not valid TOML"));
    assert!(text.contains("  --> ") && text.contains("flow.toml:"));
    assert!(text.contains("   = field: $"));
    assert!(text.contains("   = help: fix the TOML syntax"));
    assert!(text.contains("   = explain: harness-gate explain HGCFG-TOML-PARSE"));
    assert!(!text.contains("not-for-terminal-display"));
    assert!(!text.contains('\u{1b}'));
    assert!(!root.path().join(".harness-gate/reports").exists());
}

#[test]
fn explicit_color_overrides_no_color_but_auto_respects_it() {
    let root = malformed_config();
    let always = check(&root, "always", false);
    assert_eq!(always.status.code(), Some(1));
    let text = String::from_utf8(always.stderr).unwrap();
    assert!(text.contains("\u{1b}[31merror[HGCFG-TOML-PARSE]\u{1b}[0m"));
    let auto = check(&root, "auto", false);
    assert_eq!(auto.status.code(), Some(1));
    assert!(!auto.stderr.contains(&0x1b));
}

#[test]
fn json_diagnostics_are_identical_across_terminal_color_modes() {
    let root = malformed_config();
    let plain = check(&root, "never", true);
    let colored = check(&root, "always", true);
    for output in [&plain, &colored] {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.contains(&0x1b));
    }
    assert_eq!(plain.stdout, colored.stdout);
    let report: Value = serde_json::from_slice(&plain.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["valid"], false);
    assert_eq!(report["diagnostics"][0]["id"], "HGCFG-TOML-PARSE");
    assert!(report["diagnostics"][0]["location"]["line"].is_number());
    assert!(report["diagnostics"][0].get("details").is_none());
}

#[test]
fn redirected_auto_color_is_plain_without_no_color() {
    let root = malformed_config();
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root.path())
        .args(["--color", "auto", "config", "check"])
        .env_remove("NO_COLOR")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.contains(&0x1b));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("error[HGCFG-TOML-PARSE]"));
}

#[test]
fn ordinary_commands_preserve_typed_configuration_diagnostics() {
    let root = malformed_config();
    for action in ["verify", "doctor"] {
        let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(root.path())
            .args(["--color", "never", action])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{action}");
        assert!(output.stdout.is_empty(), "{action}");
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(
            text.contains("ERROR [E1000]: configuration is invalid"),
            "{text}"
        );
        assert!(text.contains("error[HGCFG-TOML-PARSE]"), "{text}");
        assert!(text.contains("flow.toml:"), "{text}");
        assert!(text.contains("explain: harness-gate explain HGCFG-TOML-PARSE"));
        assert!(!text.contains("not-for-terminal-display") && !text.contains('\u{1b}'));
    }
    assert!(!root.path().join(".harness-gate/reports").exists());
}
