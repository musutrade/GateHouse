use super::*;
use crate::test_support::TestWorkspace;
use std::env;
#[cfg(unix)]
use std::path::Path;

#[test]
fn preparation_does_not_change_the_process_working_directory() {
    let workspace = TestWorkspace::new("project-prepare");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let before = env::current_dir().expect("read working directory");

    project.prepare().expect("prepare project");

    assert_eq!(
        env::current_dir().expect("read working directory"),
        before,
        "preparing one project must not affect another project's process context"
    );
}

#[test]
fn invocation_reports_resolve_their_stable_invocation_id() {
    let workspace = TestWorkspace::new("project-invocation-id");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    let mut invocation = project.clone();
    invocation.reports = project.reports.join("invocations/inv-test");

    assert_eq!(invocation.invocation_id(), "inv-test");
    assert!(project.invocation_id().starts_with("standalone-"));
}

#[cfg(unix)]
#[test]
fn repository_path_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("project-root");
    let outside = TestWorkspace::new("project-outside");
    symlink(&outside.root, root.join("reports")).expect("create symlink");

    let error = resolve_repo_path(&root, Path::new("reports"), "reports", false)
        .expect_err("symlink escape must fail");

    assert!(error.to_string().contains("escapes the repository"));
}
