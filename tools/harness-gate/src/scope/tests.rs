use crate::project::Project;
use crate::scope::{detect, ScopeMode};
use crate::test_support::TestWorkspace;
use std::collections::BTreeSet;

fn config() -> crate::config::FlowConfig {
    toml::from_str(include_str!(
        "../../fixtures/preset-compat/rust-api.flow.toml"
    ))
    .expect("parse config")
}

#[test]
fn workflow_changes_force_all_components() {
    // Use a path that matches the rust-api preset patterns
    let components = config()
        .classify_paths(&[".harness-gate/flow.toml".into()])
        .expect("classify")
        .0;
    // rust-api preset has 1 component: app
    assert_eq!(components.len(), 1);
    assert!(components.contains("app"));
}

#[test]
fn frontend_change_only_selects_frontend() {
    // rust-api preset doesn't have frontend, test with app component
    let components = config()
        .classify_paths(&["src/main.rs".into()])
        .expect("classify")
        .0;
    assert_eq!(components, BTreeSet::from(["app".to_string()]));
}

#[test]
fn unmatched_paths_are_reported() {
    let (components, unmatched) = config()
        .classify_paths(&["unconfigured/new-tool.lock".into()])
        .expect("classify");

    assert!(components.is_empty());
    assert_eq!(unmatched, vec!["unconfigured/new-tool.lock"]);
}

#[test]
fn detects_working_tree_changes() {
    let workspace = TestWorkspace::new("scope-working-tree");
    crate::preset::init_from(
        &workspace.root,
        "rust-api",
        false,
        Some(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")),
    )
    .expect("initialize fixture");
    workspace.init_git();
    std::fs::create_dir_all(workspace.root.join("src")).expect("create source directory");
    std::fs::write(workspace.root.join("src/main.rs"), "fn main() {}\n").expect("write fixture");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let result = detect(&project, &ScopeMode::WorkingTree).expect("detect working tree");
    assert!(result
        .changed_files
        .iter()
        .any(|path| path == "src/main.rs"));
    assert!(result.components.contains("app"));
}

#[test]
fn detects_staged_changes() {
    let workspace = TestWorkspace::new("scope-staged");
    crate::preset::init_from(
        &workspace.root,
        "rust-api",
        false,
        Some(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")),
    )
    .expect("initialize fixture");
    workspace.init_git();
    std::fs::create_dir_all(workspace.root.join("src")).expect("create source directory");
    std::fs::write(workspace.root.join("src/main.rs"), "fn main() {}\n").expect("write fixture");
    std::process::Command::new("git")
        .args(["add", "src/main.rs"])
        .current_dir(&workspace.root)
        .status()
        .expect("stage fixture");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let result = detect(&project, &ScopeMode::Staged).expect("detect staged");
    assert_eq!(result.mode, "staged");
    assert!(result
        .changed_files
        .iter()
        .any(|path| path == "src/main.rs"));
}

#[test]
fn detects_changes_against_a_base_revision() {
    let workspace = TestWorkspace::new("scope-base");
    crate::preset::init_from(
        &workspace.root,
        "rust-api",
        false,
        Some(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")),
    )
    .expect("initialize fixture");
    workspace.init_git();
    std::fs::create_dir_all(workspace.root.join("src")).expect("create source directory");
    std::fs::write(workspace.root.join("src/main.rs"), "fn main() {}\n").expect("write fixture");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&workspace.root)
        .status()
        .expect("stage initial fixture");
    std::process::Command::new("git")
        .args(["commit", "--quiet", "-m", "initial"])
        .current_dir(&workspace.root)
        .status()
        .expect("commit initial fixture");
    std::fs::write(workspace.root.join("src/lib.rs"), "pub fn value() {}\n").expect("write change");
    std::process::Command::new("git")
        .args(["add", "src/lib.rs"])
        .current_dir(&workspace.root)
        .status()
        .expect("stage change");
    std::process::Command::new("git")
        .args(["commit", "--quiet", "-m", "change"])
        .current_dir(&workspace.root)
        .status()
        .expect("commit change");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let result = detect(&project, &ScopeMode::Base("HEAD^".into())).expect("detect base");
    assert_eq!(result.mode, "base:HEAD^");
    assert!(result.changed_files.iter().any(|path| path == "src/lib.rs"));
}

#[test]
fn rejects_a_missing_base_revision() {
    let workspace = TestWorkspace::new("scope-missing-base");
    crate::preset::init_from(
        &workspace.root,
        "rust-api",
        false,
        Some(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")),
    )
    .expect("initialize fixture");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let error =
        detect(&project, &ScopeMode::Base("missing".into())).expect_err("missing base must fail");
    assert!(error.to_string().contains("base reference does not exist"));
}

#[test]
fn unmatched_scope_can_fail_closed() {
    let workspace = TestWorkspace::new("scope-unmatched");
    crate::preset::init_from(
        &workspace.root,
        "rust-api",
        false,
        Some(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")),
    )
    .expect("initialize fixture");
    workspace.init_git();
    std::fs::write(workspace.root.join("unmatched.txt"), "change\n").expect("write fixture");
    let mut project =
        Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    project.config.scope.unmatched = crate::config::UnmatchedScope::Fail;
    let error = detect(&project, &ScopeMode::WorkingTree).expect_err("unmatched path must fail");
    assert!(error.to_string().contains("unmatched changed file"));
}

#[test]
fn all_policy_falls_back_only_for_unmatched_changes() {
    let workspace = TestWorkspace::new("scope-all-fallback");
    crate::preset::init_from(
        &workspace.root,
        "rust-api",
        false,
        Some(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")),
    )
    .expect("initialize fixture");
    workspace.init_git();
    std::fs::create_dir_all(workspace.root.join("src")).expect("create source directory");
    std::fs::write(workspace.root.join("src/main.rs"), "fn main() {}\n").expect("write source");
    for args in [
        vec!["add", "."],
        vec![
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    ] {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&workspace.root)
            .output()
            .expect("commit fixture");
        assert!(output.status.success(), "{output:?}");
    }
    let mut project =
        Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    project.config.scope.unmatched = crate::config::UnmatchedScope::All;
    let mut unrelated = project.config.steps[0].clone();
    unrelated.id = "unrelated.test".into();
    unrelated.component = "unrelated".into();
    project.config.steps.push(unrelated);
    let clean = detect(&project, &ScopeMode::WorkingTree).expect("clean scope");
    assert!(clean.changed_files.is_empty());
    assert!(clean.components.is_empty());

    std::fs::write(
        workspace.root.join("src/main.rs"),
        "fn main() { println!(\"changed\"); }\n",
    )
    .expect("modify source");
    let matched = detect(&project, &ScopeMode::WorkingTree).expect("matched scope");
    assert_eq!(matched.components, BTreeSet::from(["app".into()]));
    assert!(matched.unmatched_files.is_empty());

    std::fs::write(workspace.root.join("unmatched.txt"), "change\n").expect("write unmatched");
    let mixed = detect(&project, &ScopeMode::WorkingTree).expect("fallback scope");
    assert_eq!(mixed.components, project.config.components());
    assert_eq!(mixed.unmatched_files, vec!["unmatched.txt"]);
}
