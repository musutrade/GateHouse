#[allow(dead_code)]
mod common;

use common::*;
use std::process::Command;

const BASELINE: &str =
    include_str!("../../../docs/dogfood/arc-admin/sources/.arc-flow/flow.toml.txt");

fn generic_source() -> String {
    let source = BASELINE.replace("external_value_policy = \"isolated-postgres\"", "");
    format!("{source}\n[[service_checks.test-postgres]]\nexecutable = \"/uninstalled/required-check\"\nsha256 = \"{}\"\nprotocol = \"gate-service-check/v1\"\nplugin = \"fixture\"\nversion = \"1\"\ntimeout_secs = 15\npass_env = [\"PATH\", \"DATABASE_URL\"]\n", "0".repeat(64))
}

fn fixture() -> TestContext {
    let ctx = TestContext::new();
    ctx.write_file(".arc-flow/flow.toml", &generic_source());
    ctx
}

fn import(ctx: &TestContext) -> std::process::Output {
    ctx.run_harness_gate(&["config", "import", "--execution-only"])
}

#[test]
fn historical_postgres_import_requires_plugin_migration_without_publication() {
    let ctx = TestContext::new();
    ctx.write_file(".arc-flow/flow.toml", BASELINE);
    let output = import(&ctx);
    assert_failure(&output);
    assert!(stderr_str(&output).contains("gateprobes-postgres migrate"));
    assert_eq!(ctx.read_file(".arc-flow/flow.toml"), BASELINE);
    assert!(!ctx.file_exists(".harness-gate/flow.toml"));
    assert!(!ctx.file_exists(".harness-gate/flow.import.json"));
}

#[test]
fn generic_import_preserves_checks_and_frozen_step_inventory_without_host_environment() {
    let ctx = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["config", "import", "--execution-only", "--project-root"])
        .arg(&ctx.project_root)
        .env("ARC_FLOW_BACKEND", "wrong-backend")
        .env("RUST_TEST_TIMEOUT", "not-an-integer")
        .env("ARC_FLOW_POSTGRES_IMAGE", "wrong-image")
        .env("HARNESS_GATE_SECRETS_CONFIG", "wrong-policy")
        .output()
        .unwrap();
    assert_success(&output);
    assert!(stdout_str(&output).contains("Authority transfer BLOCKED"));
    assert_eq!(ctx.read_file(".arc-flow/flow.toml"), generic_source());
    let actual: toml::Value = toml::from_str(&ctx.read_file(".harness-gate/flow.toml")).unwrap();
    let mut expected: toml::Value = toml::from_str(include_str!(
        "../../../docs/dogfood/arc-admin/import/flow.toml"
    ))
    .unwrap();
    expected["services"]["test-postgres"]
        .as_table_mut()
        .unwrap()
        .remove("external_value_policy");
    let source: toml::Value = toml::from_str(&generic_source()).unwrap();
    expected
        .as_table_mut()
        .unwrap()
        .insert("service_checks".into(), source["service_checks"].clone());
    assert_eq!(actual, expected);
    let report: serde_json::Value =
        serde_json::from_str(&ctx.read_file(".harness-gate/flow.import.json")).unwrap();
    let frozen: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/dogfood/arc-admin/import/flow.import.json"
    ))
    .unwrap();
    assert_eq!(report["steps"], frozen["steps"]);
    assert_eq!(report["runtime_blockers"], frozen["runtime_blockers"]);
    assert_eq!(report["authority_transfer"], "blocked");
    let second = fixture();
    assert_success(&import(&second));
    assert_eq!(
        ctx.read_file(".harness-gate/flow.toml"),
        second.read_file(".harness-gate/flow.toml")
    );
    assert_eq!(
        ctx.read_file(".harness-gate/flow.import.json"),
        second.read_file(".harness-gate/flow.import.json")
    );
    assert!(!ctx.file_exists(".harness-gate/secrets.toml"));
}

#[test]
fn arc_import_refuses_runtime_migration_and_existing_outputs() {
    let ctx = fixture();
    let output = ctx.run_harness_gate(&["config", "import"]);
    assert_failure(&output);
    assert!(stderr_str(&output).contains("full Arc-Flow migration is blocked"));
    assert!(!ctx.file_exists(".harness-gate/flow.toml"));
    ctx.write_file(".harness-gate/flow.import.json", "prior evidence");
    assert_failure(&import(&ctx));
    assert!(!ctx.file_exists(".harness-gate/flow.toml"));
    assert_eq!(
        ctx.read_file(".harness-gate/flow.import.json"),
        "prior evidence"
    );
    let ctx = fixture();
    ctx.write_file(".harness-gate/flow.toml", "existing policy");
    assert_failure(&import(&ctx));
    assert_eq!(ctx.read_file(".harness-gate/flow.toml"), "existing policy");
}

#[test]
fn arc_import_rejects_loss_unknown_fields_and_broken_blockers_before_writing() {
    let baseline = generic_source();
    let cases = [
        (
            baseline.replacen(
                "plugin = \"fixture\"",
                "plugin = \"fixture\"\noptional = true",
                1,
            ),
            "optional",
        ),
        (
            baseline.replacen(
                "pass_env = [\"PATH\", \"DATABASE_URL\"]",
                "pass_env = [\"PATH\", \"PATH\", \"DATABASE_URL\"]",
                1,
            ),
            "lossy",
        ),
        (
            baseline.replacen("version = 2", "version = 3", 1),
            "version",
        ),
        (
            baseline.replacen("version = 2", "version = 2\nfuture_blocker = true", 1),
            "future_blocker",
        ),
        (
            baseline.replacen(
                "label = \"git\"",
                "label = \"git\"\nfuture_blocker = true",
                1,
            ),
            "future_blocker",
        ),
        (
            baseline.replacen(
                "kind = \"regex\"",
                "kind = \"regex\"\nfuture_blocker = true",
                1,
            ),
            "future_blocker",
        ),
        (
            baseline.replacen(
                "kind = \"docker\"",
                "kind = \"docker\"\nfuture_blocker = true",
                1,
            ),
            "future_blocker",
        ),
        (
            baseline.replacen(
                "kind = \"docker\"",
                "kind = \"docker\"\nruntime = \"podman\"",
                1,
            ),
            "runtime",
        ),
        (
            baseline.replacen("kind = \"regex\"", "kind = \"junit\"", 1),
            "parser kind",
        ),
        (
            baseline.replacen(
                "required_steps = [",
                "required_steps = [\"missing.blocker\",",
                1,
            ),
            "requires",
        ),
        (
            baseline.replacen(
                "profiles = [\"full\", \"hook\"]",
                "profiles = [\"full\", \"full\", \"hook\"]",
                1,
            ),
            "lossy",
        ),
        (
            baseline.replacen("program = \"git\"", "program = \"${IMPORT_SECRET}\"", 1),
            "interpolation",
        ),
        (
            baseline.replacen(
                "[[steps]]",
                "[[steps]]\ndepends_on = [\"missing.blocker\"]",
                1,
            ),
            "depend",
        ),
        (
            baseline.replacen("[[steps]]", "[[steps]]\noptional = true", 1),
            "optional",
        ),
    ];
    for (source, diagnostic) in cases {
        assert_ne!(source, baseline, "mutation must change the baseline");
        let ctx = fixture();
        ctx.write_file(".arc-flow/flow.toml", &source);
        let output = import(&ctx);
        assert_failure(&output);
        assert!(
            stderr_str(&output).contains(diagnostic),
            "{}",
            stderr_str(&output)
        );
        assert!(!ctx.file_exists(".harness-gate/flow.toml"));
        assert!(!ctx.file_exists(".harness-gate/flow.import.json"));
        assert_eq!(ctx.read_file(".arc-flow/flow.toml"), source);
    }
}

#[test]
fn compatible_dependencies_and_arc_defaults_are_preserved() {
    let ctx = fixture();
    let mut source: toml::Value = toml::from_str(&generic_source()).unwrap();
    source["paths"]
        .as_table_mut()
        .unwrap()
        .remove("secrets_config");
    let steps = source["steps"].as_array_mut().unwrap();
    let prerequisite = steps[0]["id"].clone();
    steps[1].as_table_mut().unwrap().insert(
        "depends_on".into(),
        toml::Value::Array(vec![prerequisite.clone()]),
    );
    ctx.write_file(".arc-flow/flow.toml", &toml::to_string(&source).unwrap());
    assert_success(&import(&ctx));
    let imported: toml::Value = toml::from_str(&ctx.read_file(".harness-gate/flow.toml")).unwrap();
    assert_eq!(
        imported["paths"]["secrets_config"].as_str(),
        Some(".arc-flow/secrets.toml")
    );
    assert_eq!(imported["steps"][1]["depends_on"][0], prerequisite);
    assert_eq!(imported["steps"][1]["input"].as_str(), Some("repository"));
}

#[test]
fn arc_import_rejects_destination_escape() {
    let ctx = fixture();
    let output = ctx.run_harness_gate(&[
        "config",
        "import",
        "--execution-only",
        "--output",
        "missing/../../escape.toml",
    ]);
    assert_failure(&output);
    assert!(stderr_str(&output).contains("parent traversal"));
}
