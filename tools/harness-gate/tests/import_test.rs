#[allow(dead_code)]
mod common;

use common::*;
use std::process::Command;

const BASELINE: &str =
    include_str!("../../../docs/dogfood/arc-admin/sources/.arc-flow/flow.toml.txt");

fn fixture() -> TestContext {
    let ctx = TestContext::new();
    ctx.write_file(".arc-flow/flow.toml", BASELINE);
    ctx
}

fn import(ctx: &TestContext) -> std::process::Output {
    ctx.run_harness_gate(&["config", "import", "--execution-only"])
}

#[test]
fn arc_admin_import_matches_frozen_config_and_inventory_without_host_environment() {
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
    assert_eq!(ctx.read_file(".arc-flow/flow.toml"), BASELINE);
    assert_eq!(
        ctx.read_file(".harness-gate/flow.toml"),
        include_str!("../../../docs/dogfood/arc-admin/import/flow.toml")
    );
    assert_eq!(
        ctx.read_file(".harness-gate/flow.import.json"),
        include_str!("../../../docs/dogfood/arc-admin/import/flow.import.json")
    );
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
    let cases = [
        (
            BASELINE.replacen("version = 2", "version = 3", 1),
            "version",
        ),
        (
            BASELINE.replacen("version = 2", "version = 2\nfuture_blocker = true", 1),
            "future_blocker",
        ),
        (
            BASELINE.replacen(
                "label = \"git\"",
                "label = \"git\"\nfuture_blocker = true",
                1,
            ),
            "future_blocker",
        ),
        (
            BASELINE.replacen(
                "kind = \"regex\"",
                "kind = \"regex\"\nfuture_blocker = true",
                1,
            ),
            "future_blocker",
        ),
        (
            BASELINE.replacen(
                "kind = \"docker\"",
                "kind = \"docker\"\nfuture_blocker = true",
                1,
            ),
            "future_blocker",
        ),
        (
            BASELINE.replacen(
                "kind = \"docker\"",
                "kind = \"docker\"\nruntime = \"podman\"",
                1,
            ),
            "runtime",
        ),
        (
            BASELINE.replacen("kind = \"regex\"", "kind = \"junit\"", 1),
            "parser kind",
        ),
        (
            BASELINE.replacen(
                "required_steps = [",
                "required_steps = [\"missing.blocker\",",
                1,
            ),
            "requires",
        ),
        (
            BASELINE.replacen(
                "profiles = [\"full\", \"hook\"]",
                "profiles = [\"full\", \"full\", \"hook\"]",
                1,
            ),
            "lossy",
        ),
        (
            BASELINE.replacen("program = \"git\"", "program = \"${IMPORT_SECRET}\"", 1),
            "interpolation",
        ),
        (
            BASELINE.replacen(
                "[[steps]]",
                "[[steps]]\ndepends_on = [\"missing.blocker\"]",
                1,
            ),
            "depend",
        ),
        (
            BASELINE.replacen("[[steps]]", "[[steps]]\noptional = true", 1),
            "optional",
        ),
    ];
    for (source, diagnostic) in cases {
        assert_ne!(source, BASELINE, "mutation must change the baseline");
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
    let mut source: toml::Value = toml::from_str(BASELINE).unwrap();
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
