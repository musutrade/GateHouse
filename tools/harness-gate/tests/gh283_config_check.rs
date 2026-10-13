use std::process::Command;

fn scenario(name: &str) {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let output = Command::new(python)
        .args([
            "-B",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/gh283_config_check.py"),
            "--binary",
            env!("CARGO_BIN_EXE_harness-gate"),
            "--case",
            name,
        ])
        .output()
        .expect("start real CLI scenario runner");
    assert!(
        output.status.success(),
        "{name}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn root_auto_human_json_agree() {
    scenario("root_auto_human_json_agree");
}

#[test]
fn one_level_auto_human_json_agree() {
    scenario("one_level_auto_human_json_agree");
}

#[test]
fn multi_level_auto_human_json_agree() {
    scenario("multi_level_auto_human_json_agree");
}

#[test]
fn explicit_project_root_override_human_json_agree() {
    scenario("explicit_project_root_override_human_json_agree");
}

#[test]
fn explicit_config_override_human_json_agree() {
    scenario("explicit_config_override_human_json_agree");
}

#[test]
fn missing_project_human_json_agree() {
    scenario("missing_project_human_json_agree");
}

#[test]
fn malformed_config_human_json_agree() {
    scenario("malformed_config_human_json_agree");
}

#[test]
fn invalid_config_human_json_agree() {
    scenario("invalid_config_human_json_agree");
}
