#![cfg(unix)]
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;
const FLOW: &str = r#"
version = 2
[project]
name = "rename-scope"
default_profile = "full"
hook_profile = "hook"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "fail"
[[scope.rules]]
patterns = ["a/**"]
components = ["a"]
[[scope.rules]]
patterns = ["b/**"]
components = ["b"]
[[scope.rules]]
patterns = [".harness-gate/**", "probe.sh"]
components = ["a", "b"]
[[steps]]
id = "setup"
label = "setup"
component = "support"
profiles = ["full", "hook"]
program = "true"
args = []
cwd = "{root}"
log = "setup.log"
timeout_secs = 10
[[steps]]
id = "a.test"
label = "a test"
component = "a"
profiles = ["full", "hook"]
program = "sh"
args = ["probe.sh"]
depends_on = ["setup"]
cwd = "{root}"
log = "a.log"
timeout_secs = 10
[[steps]]
id = "b.test"
label = "b test"
component = "b"
profiles = ["full", "hook"]
program = "true"
args = []
cwd = "{root}"
log = "b.log"
timeout_secs = 10
[[steps]]
id = "a.full-only"
label = "full only"
component = "a"
profiles = ["full"]
program = "true"
args = []
cwd = "{root}"
log = "full.log"
timeout_secs = 10
[[steps]]
id = "unrelated"
label = "unrelated"
component = "unrelated"
profiles = ["full", "hook"]
program = "false"
args = []
cwd = "{root}"
log = "unrelated.log"
timeout_secs = 10
"#;

fn checked(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}
fn git(root: &Path, args: &[&str]) {
    checked(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap(),
    );
}
fn gate(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--project-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn fixture(policy: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    checked(gate(root, &["init", "--preset", "generic"]));
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "Plan Test"]);
    git(root, &["config", "user.email", "plan@example.invalid"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    // Keep fixture commits from launching detached maintenance that can race
    // the subsequent byte-for-byte repository immutability assertion.
    git(root, &["config", "maintenance.auto", "false"]);
    git(root, &["config", "gc.auto", "0"]);
    fs::write(
        root.join(".harness-gate/flow.toml"),
        FLOW.replace("unmatched = \"fail\"", &format!("unmatched = \"{policy}\"")),
    )
    .unwrap();
    fs::create_dir(root.join("a")).unwrap();
    fs::create_dir(root.join("b")).unwrap();
    fs::write(root.join("a/old name\nfile.txt"), "initial\n").unwrap();
    fs::write(root.join("probe.sh"), "echo actual-step\nexit 0\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "initial"]);
    dir
}
fn plan(root: &Path, options: &[&str]) -> Value {
    let mut args = vec!["verify", "--plan", "--json"];
    args.extend_from_slice(options);
    let output = checked(gate(root, &args));
    serde_json::from_slice(&output.stdout).unwrap()
}
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}
fn node<'a>(value: &'a Value, id: &str) -> &'a Value {
    value["plan"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap()
}

#[test]
fn plans_match_actual_verify_for_scope_modes_profiles_and_rename_regressions() {
    for policy in ["fail", "all", "ignore"] {
        for mode in ["working", "staged", "base"] {
            for change in ["clean", "matched", "rename", "unmatched"] {
                let dir = fixture(policy);
                let root = dir.path();
                match change {
                    "matched" => fs::write(root.join("a/old name\nfile.txt"), "changed\n").unwrap(),
                    "rename" => git(
                        root,
                        &["mv", "a/old name\nfile.txt", "b/new name\nfile.txt"],
                    ),
                    "unmatched" => fs::write(root.join("outside.txt"), "changed\n").unwrap(),
                    _ => {}
                }
                let mut options = vec!["--profile", "hook"];
                match mode {
                    "staged" => {
                        git(root, &["add", "."]);
                        options.push("--staged");
                    }
                    "base" => {
                        git(root, &["add", "."]);
                        git(
                            root,
                            &["commit", "--quiet", "--allow-empty", "-m", "candidate"],
                        );
                        options.extend(["--base", "HEAD^"]);
                    }
                    _ => {}
                }
                if policy == "fail" && change == "unmatched" {
                    let mut args = vec!["verify", "--plan", "--json"];
                    args.extend(&options);
                    let output = gate(root, &args);
                    assert!(!output.status.success());
                    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(value["status"], "invalid");
                    assert!(value["plan"].is_null());
                    continue;
                }
                let before = snapshot(root);
                let value = plan(root, &options);
                assert_eq!(
                    value,
                    plan(root, &options),
                    "determinism {policy}/{mode}/{change}"
                );
                assert_eq!(snapshot(root), before, "read-only {policy}/{mode}/{change}");
                assert_eq!(value["schema"], "verification-plan/v1");
                assert!(value["verification_result"].is_null());
                let expected = value["plan"]["execution_order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut args = vec!["verify"];
                args.extend(&options);
                let _execution = gate(root, &args);
                let report: Value = serde_json::from_slice(
                    &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
                )
                .unwrap();
                let mut actual = report["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["step_id"].as_str().unwrap().to_string())
                    .collect::<std::collections::BTreeSet<_>>();
                for skipped in report["skipped_steps"].as_array().unwrap() {
                    actual.insert(skipped["id"].as_str().unwrap().to_string());
                }
                assert_eq!(actual, expected, "{policy}/{mode}/{change}");
                if change == "rename" {
                    assert_eq!(value["plan"]["scope"]["components"], json!(["a", "b"]));
                    assert!(expected.contains("setup"));
                    assert!(!expected.contains("unrelated"));
                    assert_eq!(value["plan"]["paths"].as_array().unwrap().len(), 2);
                }
                if change == "clean" {
                    assert_eq!(
                        expected.len(),
                        2,
                        "unmatched=all must not select everything"
                    );
                }
                if change == "matched" {
                    assert_eq!(value["plan"]["scope"]["components"], json!(["a"]));
                    assert!(!expected.contains("b.test"));
                    assert_eq!(
                        node(&value, "setup")["reasons"][0]["code"],
                        "DEPENDENCY_REQUIRED"
                    );
                    assert_eq!(
                        node(&value, "a.full-only")["reasons"][0]["code"],
                        "PROFILE_NOT_INCLUDED"
                    );
                }
                if change == "unmatched" && policy == "all" {
                    assert_eq!(
                        value["plan"]["paths"][0]["reasons"][0]["code"],
                        "UNMATCHED_ALL"
                    );
                }
            }
        }
    }
}

#[test]
fn explicit_and_all_modes_preserve_forced_gates_dependency_closure_and_exclusions() {
    let dir = fixture("all");
    let root = dir.path();
    let source = FLOW
        .replacen(
            "profiles = [\"full\", \"hook\"]",
            "profiles = [\"full\"]",
            1,
        )
        .replacen(
            "label = \"a test\"",
            "label = \"a test\"\nservices = [\"db\"]",
            1,
        );
    let source = format!("{source}\n[services.db]\nkind = \"environment\"\nsource_env = \"UNPROVISIONED_PLAN_DB\"\ninject_env = \"DATABASE_URL\"\n[services.unused]\nkind = \"environment\"\nsource_env = \"UNPROVISIONED_PLAN_OTHER\"\ninject_env = \"OTHER_URL\"\n");
    fs::write(root.join(".harness-gate/flow.toml"), source).unwrap();
    let before = snapshot(root);
    let value = plan(root, &["--components", "a", "--profile", "hook"]);
    assert_eq!(snapshot(root), before);
    assert_eq!(node(&value, "setup")["selected"], true);
    assert_eq!(
        node(&value, "setup")["reasons"][0]["code"],
        "DEPENDENCY_REQUIRED"
    );
    assert_eq!(
        node(&value, "builtin.secret-scan")["reasons"][0]["code"],
        "MANDATORY_BUILTIN_GATE"
    );
    assert_eq!(value["plan"]["services"][0]["id"], "db");
    assert_eq!(value["plan"]["services"][0]["selected"], true);
    assert_eq!(value["plan"]["services"][1]["selected"], false);
    assert_eq!(
        value["plan"]["components"][0]["reasons"][0]["code"],
        "EXPLICIT_COMPONENT"
    );
    let all = plan(root, &["--all"]);
    assert_eq!(node(&all, "unrelated")["selected"], true);
    let human = checked(gate(root, &["verify", "--plan", "--all"]));
    assert!(String::from_utf8(human.stdout)
        .unwrap()
        .contains("not executed"));
    assert!(!gate(root, &["verify", "--json"]).status.success());
    assert!(
        !gate(root, &["verify", "--plan", "--staged", "--components", "a"])
            .status
            .success()
    );
    // Unprovisioned services may fail execution, but selected/dispatched or
    // skipped nodes must still match the explicit-component and all plans.
    for (options, preview) in [
        (vec!["--components", "a", "--profile", "hook"], value),
        (vec!["--all"], all),
    ] {
        let mut args = vec!["verify"];
        args.extend(options);
        let _ = gate(root, &args);
        let report: Value = serde_json::from_slice(
            &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
        )
        .unwrap();
        let mut actual = report["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["step_id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        actual.extend(
            report["skipped_steps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["id"].as_str().unwrap()),
        );
        let expected = preview["plan"]["execution_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual, expected);
    }
}

#[test]
fn invalid_configuration_scope_and_profile_are_not_verification_passes() {
    for problem in [
        "profile",
        "component",
        "cycle",
        "dependency",
        "missing",
        "syntax",
        "audit",
    ] {
        let dir = fixture("fail");
        let root = dir.path();
        let options = match problem {
            "profile" => vec!["--profile", "missing", "--all"],
            "component" => vec!["--components", "missing"],
            "cycle" => {
                fs::write(
                    root.join(".harness-gate/flow.toml"),
                    FLOW.replacen(
                        "id = \"setup\"",
                        "id = \"setup\"\ndepends_on = [\"a.test\"]",
                        1,
                    ),
                )
                .unwrap();
                vec!["--all"]
            }
            "dependency" => {
                fs::write(
                    root.join(".harness-gate/flow.toml"),
                    FLOW.replace("depends_on = [\"setup\"]", "depends_on = [\"missing\"]"),
                )
                .unwrap();
                vec!["--all"]
            }
            "missing" => {
                fs::remove_file(root.join(".harness-gate/flow.toml")).unwrap();
                vec!["--all"]
            }
            "syntax" => {
                fs::write(root.join(".harness-gate/flow.toml"), "broken = [").unwrap();
                vec!["--all"]
            }
            "audit" => {
                fs::remove_file(root.join(".harness-gate/audit.toml")).unwrap();
                vec!["--all"]
            }
            _ => unreachable!(),
        };
        let before = snapshot(root);
        let mut args = vec!["verify", "--plan", "--json"];
        args.extend(options);
        let output = gate(root, &args);
        assert!(!output.status.success(), "{problem}: {output:?}");
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["status"], "invalid", "{problem}");
        assert!(value["verification_result"].is_null() && value["plan"].is_null());
        assert!(!value["diagnostics"].as_array().unwrap().is_empty());
        if problem == "profile" {
            assert!(value["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("unknown or empty verification profile"));
        }
        if problem == "component" {
            assert!(value["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("unknown component"));
        }
        assert_eq!(snapshot(root), before, "{problem}");
    }
}

#[test]
fn staged_plan_uses_indexed_configuration_without_filters_objects_reports_or_tokens() {
    let dir = fixture("all");
    let root = dir.path();
    // A filter and a filesystem monitor would write a sentinel if invoked.
    fs::write(
        root.join("forbidden.sh"),
        "#!/bin/sh\necho called >> forbidden-called\ncat\n",
    )
    .unwrap();
    git(
        root,
        &["config", "filter.forbidden.smudge", "sh forbidden.sh"],
    );
    git(root, &["config", "core.fsmonitor", "sh forbidden.sh"]);
    fs::write(root.join(".gitattributes"), "*.rs filter=forbidden\n").unwrap();
    // Disable the monitor only while preparing the fixture's index.
    git(root, &["-c", "core.fsmonitor=false", "add", "."]);
    let source = fs::read_to_string(root.join(".harness-gate/flow.toml"))
        .unwrap()
        .replacen("component = \"a\"", "component = \"b\"", 1);
    fs::write(root.join(".harness-gate/flow.toml"), source).unwrap();
    fs::create_dir(root.join("ledger")).unwrap();
    fs::write(root.join("ledger/nonces"), "untouched\n").unwrap();
    let before = snapshot(root);
    let value = plan(root, &["--staged"]);
    assert_eq!(snapshot(root), before);
    assert!(!root.join("forbidden-called").exists());
    assert_eq!(node(&value, "a.test")["component"], "a");
    assert!(!root.join(".harness-gate/reports").exists());

    // A configuration filter can change selection. Refuse it without running
    // the filter instead of advertising a plan for different configuration.
    for attribute in ["filter=forbidden", "working-tree-encoding=UTF-16", "ident"] {
        fs::write(
            root.join(".gitattributes"),
            format!(".harness-gate/flow.toml {attribute}\n"),
        )
        .unwrap();
        git(
            root,
            &["-c", "core.fsmonitor=false", "add", ".gitattributes"],
        );
        let before = snapshot(root);
        let output = gate(root, &["verify", "--plan", "--staged", "--json"]);
        assert!(!output.status.success());
        let invalid: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(invalid["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("checkout filter"));
        assert_eq!(snapshot(root), before);
    }
    // A partial clone's missing config must fail without running a remote
    // helper or hydrating Git objects during preview.
    fs::write(root.join(".gitattributes"), "").unwrap();
    git(
        root,
        &["-c", "core.fsmonitor=false", "add", ".gitattributes"],
    );
    fs::write(
        root.join("fetch-forbidden.sh"),
        "echo fetch >> forbidden-called\nexit 1\n",
    )
    .unwrap();
    git(
        root,
        &["config", "remote.origin.url", "ext::sh fetch-forbidden.sh"],
    );
    git(root, &["config", "remote.origin.promisor", "true"]);
    git(root, &["config", "protocol.ext.allow", "always"]);
    let missing = format!("100644,{},.harness-gate/flow.toml", "1".repeat(40));
    git(
        root,
        &[
            "-c",
            "core.fsmonitor=false",
            "update-index",
            "--info-only",
            "--cacheinfo",
            &missing,
        ],
    );
    let before = snapshot(root);
    let output = gate(root, &["verify", "--plan", "--staged", "--json"]);
    assert!(!output.status.success());
    let invalid: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(invalid["status"], "invalid");
    assert_eq!(snapshot(root), before);
    assert!(!root.join("forbidden-called").exists());
}

#[test]
fn quality_inputs_are_explained_without_consuming_signed_requests_or_baselines() {
    let dir = fixture("all");
    let root = dir.path();
    let quality = include_str!("../fixtures/workflow/quality.toml")
        .replace("name = \"rust-api\"", "name = \"rename-scope\"")
        .replace("flow_components = [\"app\"]", "flow_components = [\"a\"]")
        .replace("source_roots = [\"src\"]", "source_roots = [\"a\"]")
        .replace("src/lib.rs", "a/old name\nfile.txt");
    // Use a portable ordinary subject path; signed request bodies are deliberately invalid.
    let quality = quality.replace("a/old name\nfile.txt", "a/subject.rs");
    let quality = quality.replace(".harness-gate/policy.json", "./.harness-gate/policy.json");
    fs::write(root.join("a/subject.rs"), "// subject\n").unwrap();
    fs::write(root.join(".harness-gate/quality.toml"), format!(
        "{quality}\n[profiles.full.workflow]\nstate = \".harness-gate/state.json\"\ntrusted_keys = \".harness-gate/keys.json\"\nbaseline_request = \".harness-gate/baseline.json\"\n[profiles.hook.workflow]\nstate = \".harness-gate/hook-state.json\"\ntrusted_keys = \".harness-gate/keys.json\"\n")).unwrap();
    fs::write(
        root.join(".harness-gate/policy.json"),
        include_str!("../fixtures/workflow/policy.json"),
    )
    .unwrap();
    for name in [
        "coverage-request.json",
        "state.json",
        "keys.json",
        "baseline.json",
    ] {
        fs::write(
            root.join(".harness-gate").join(name),
            "NOT AUTHENTICATED OR CONSUMED\n",
        )
        .unwrap();
    }
    git(root, &["add", "."]);
    let before = snapshot(root);
    for mode in [vec!["--all"], vec!["--staged"]] {
        let full = plan(root, &mode);
        assert_eq!(full["plan"]["quality"]["collectors"][0]["selected"], true);
        assert_eq!(
            full["plan"]["quality"]["collectors"][0]["reasons"][0]["code"],
            "COLLECTOR_SELECTED"
        );
        assert_eq!(
            full["plan"]["quality"]["workflow"]["state"],
            ".harness-gate/state.json"
        );
        let mut options = mode;
        options.extend(["--profile", "hook"]);
        let hook = plan(root, &options);
        assert_eq!(hook["plan"]["quality"]["collectors"][0]["selected"], false);
        assert_eq!(
            hook["plan"]["quality"]["policies"][0]["reasons"][0]["code"],
            "POLICY_NOT_SELECTED"
        );
        assert_eq!(
            hook["plan"]["quality"]["participation"]["assurance"],
            "partial"
        );
    }
    assert_eq!(snapshot(root), before);
}

const IMPACT: &str = r#"
[scope.impact]
version = 1
[scope.impact.dependencies]
a = []
b = ["a"]
support = ["b"]
unrelated = []
"#;

fn component<'a>(value: &'a Value, id: &str) -> &'a Value {
    value["plan"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == id)
        .unwrap()
}

#[test]
fn component_impact_explains_transitive_consumers_and_matches_execution() {
    for mode in ["working", "staged", "base"] {
        for change in ["edit", "rename", "delete", "clean"] {
            let dir = fixture("fail");
            let root = dir.path();
            fs::write(
                root.join(".harness-gate/flow.toml"),
                format!("{FLOW}{IMPACT}"),
            )
            .unwrap();
            git(root, &["add", "."]);
            git(root, &["commit", "--quiet", "-m", "review impact graph"]);
            match change {
                "edit" => fs::write(root.join("a/old name\nfile.txt"), "changed\n").unwrap(),
                "rename" => git(root, &["mv", "a/old name\nfile.txt", "b/new.txt"]),
                "delete" => fs::remove_file(root.join("a/old name\nfile.txt")).unwrap(),
                _ => {}
            }
            let mut options = vec!["--profile", "hook"];
            match mode {
                "staged" => {
                    git(root, &["add", "."]);
                    options.push("--staged");
                }
                "base" => {
                    git(root, &["add", "."]);
                    git(
                        root,
                        &["commit", "--quiet", "--allow-empty", "-m", "candidate"],
                    );
                    options.extend(["--base", "HEAD^"]);
                }
                _ => {}
            }
            let before = snapshot(root);
            let value = plan(root, &options);
            assert_eq!(snapshot(root), before);
            assert_eq!(value, plan(root, &options));
            let affected = change != "clean";
            assert_eq!(
                node(&value, "b.test")["selected"],
                affected,
                "{mode}/{change}"
            );
            assert_eq!(node(&value, "unrelated")["selected"], false);
            assert_eq!(node(&value, "a.full-only")["selected"], false);
            if affected {
                assert_eq!(
                    value["plan"]["scope"]["components"],
                    json!(["a", "b", "support"])
                );
                assert_eq!(
                    component(&value, "support")["reasons"][0]["code"],
                    "COMPONENT_IMPACT"
                );
                let expected_chain = if change == "rename" {
                    json!(["b", "support"])
                } else {
                    json!(["a", "b", "support"])
                };
                assert_eq!(component(&value, "support")["impact_chain"], expected_chain);
                assert!(!component(&value, "support")["impact_sources"]
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
            let mut args = vec!["verify"];
            args.extend(&options);
            checked(gate(root, &args));
            let report: Value = serde_json::from_slice(
                &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
            )
            .unwrap();
            let expected: std::collections::BTreeSet<_> = value["plan"]["execution_order"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            let actual: std::collections::BTreeSet<_> = report["steps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["step_id"].as_str().unwrap())
                .collect();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn impact_preserves_explicit_components_all_and_step_dependency_direction() {
    let dir = fixture("all");
    let root = dir.path();
    fs::write(
        root.join(".harness-gate/flow.toml"),
        format!("{FLOW}{IMPACT}"),
    )
    .unwrap();
    let explicit = plan(root, &["--components", "a", "--profile", "hook"]);
    assert_eq!(explicit["plan"]["scope"]["components"], json!(["a"]));
    assert_eq!(node(&explicit, "setup")["selected"], true);
    assert_eq!(node(&explicit, "b.test")["selected"], false);
    assert_eq!(component(&explicit, "a")["impact_chain"], json!([]));
    assert_eq!(node(&plan(root, &["--all"]), "unrelated")["selected"], true);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "graph"]);
    fs::write(root.join("b/new.txt"), "b changed\n").unwrap();
    let reverse = plan(root, &[]);
    assert_eq!(
        reverse["plan"]["scope"]["components"],
        json!(["b", "support"])
    );
    assert_eq!(node(&reverse, "a.test")["selected"], false);
    fs::write(root.join("outside.txt"), "unmatched\n").unwrap();
    // FLOW still uses unmatched=fail; an impact graph must not hide this failure.
    assert!(!gate(root, &["verify", "--plan", "--json"]).status.success());
}

#[test]
fn invalid_impact_graphs_fail_before_dispatch_with_diagnostics() {
    let cases = [
        IMPACT.replace("version = 1", "version = 2"),
        IMPACT.replace("support = [\"b\"]", ""),
        IMPACT.replace("unrelated = []", "stale = []"),
        IMPACT.replace("a = []", "a = [\"support\"]"),
        IMPACT.replace("a = []", "a = [\"a\"]"),
        IMPACT.replace("b = [\"a\"]", "b = [\"missing\"]"),
        IMPACT.replace("b = [\"a\"]", "b = [\"a\", \"a\"]"),
        IMPACT.replace("version = 1", "version = 1\nprovider = 'untrusted-command'"),
        format!(
            "{IMPACT}\n[scope.impact.inputs]\n'../escape' = '{}'\n",
            "0".repeat(64)
        ),
        format!("{IMPACT}\n[scope.impact.inputs]\n'probe.sh' = 'invalid-digest'\n"),
    ];
    for graph in cases {
        let dir = fixture("fail");
        let root = dir.path();
        fs::write(
            root.join(".harness-gate/flow.toml"),
            format!("{FLOW}{graph}"),
        )
        .unwrap();
        let before = snapshot(root);
        let output = gate(root, &["verify", "--plan", "--all", "--json"]);
        assert!(!output.status.success(), "{graph}");
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["status"], "invalid");
        assert!(value["plan"].is_null() && value["verification_result"].is_null());
        assert!(!value["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(snapshot(root), before);
        assert!(!gate(root, &["verify", "--all"]).status.success());
        assert!(!root.join(".harness-gate/reports/invocations").exists());
    }
}

#[test]
fn pinned_impact_inputs_reject_stale_missing_and_filtered_data_and_use_index() {
    use sha2::{Digest, Sha256};
    let dir = fixture("fail");
    let root = dir.path();
    let digest = format!(
        "{:x}",
        Sha256::digest(fs::read(root.join("probe.sh")).unwrap())
    );
    let source = format!("{FLOW}{IMPACT}\n[scope.impact.inputs]\n'probe.sh' = '{digest}'\n");
    fs::write(root.join(".harness-gate/flow.toml"), &source).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "pinned graph"]);
    fs::write(root.join("a/old name\nfile.txt"), "changed\n").unwrap();
    git(root, &["add", "a"]);
    let expected = plan(root, &["--staged"]);
    fs::write(root.join("probe.sh"), "stale\n").unwrap();
    let before = snapshot(root);
    assert_eq!(expected, plan(root, &["--staged"]));
    let output = gate(root, &["verify", "--plan", "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("is stale"));
    assert_eq!(snapshot(root), before);
    let scoped = checked(gate(root, &["scope", "--staged", "--json"]));
    let scoped: Value = serde_json::from_slice(&scoped.stdout).unwrap();
    assert_eq!(
        scoped["components"],
        expected["plan"]["scope"]["components"]
    );
    assert!(!gate(root, &["config", "check", "--format", "json"])
        .status
        .success());
    git(root, &["add", "probe.sh"]);
    assert!(!gate(root, &["verify", "--plan", "--staged", "--json"])
        .status
        .success());
    fs::remove_file(root.join("probe.sh")).unwrap();
    assert!(!gate(root, &["verify", "--plan", "--all", "--json"])
        .status
        .success());
    fs::write(root.join("probe.sh"), "echo actual-step\nexit 0\n").unwrap();
    fs::write(root.join(".gitattributes"), "probe.sh ident\n").unwrap();
    git(root, &["add", "."]);
    let output = gate(root, &["verify", "--plan", "--staged", "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("checkout filter"));
}
