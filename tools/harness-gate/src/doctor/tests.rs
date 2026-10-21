use super::checks::{check_env_or_file, check_glob, check_path, check_remotes};
use crate::config::{PathScope, PathType};
use crate::test_support::TestWorkspace;
use std::fs;
use std::time::Duration;

#[test]
fn git_remote_check_rejects_non_git_directory() {
    let root = TestWorkspace::new("doctor");

    let error =
        check_remotes(&root, Duration::from_secs(2)).expect_err("non-Git directory must fail");

    assert!(error.to_string().contains("not a Git worktree"));
}

#[test]
fn relative_glob_matches_paths_from_the_project_root() {
    let root = TestWorkspace::new("doctor-glob");
    crate::preset::init(&root.root, "generic", false).expect("initialize fixture");
    root.init_git();
    fs::write(
        root.root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\n",
    )
    .expect("write glob fixture");
    let project =
        crate::project::Project::discover(Some(root.root.clone()), None).expect("discover fixture");

    let result = check_glob(&project, "Cargo.toml");

    assert!(result.is_ok(), "relative glob should match: {result:?}");
}

fn doctor_project(name: &str) -> (TestWorkspace, crate::project::Project) {
    let root = TestWorkspace::new(name);
    crate::preset::init(&root.root, "generic", false).expect("initialize fixture");
    root.init_git();
    let project =
        crate::project::Project::discover(Some(root.root.clone()), None).expect("discover fixture");
    (root, project)
}

#[test]
#[cfg(unix)]
fn doctor_service_check_keeps_recipe_validation_on_both_outcomes() {
    let (_root, mut project) = doctor_project("doctor-service-recipe");
    let name = "HARNESS_GATE_DOCTOR_SERVICE_RECIPE_FIXTURE";
    project.config.services.insert(
        "database".into(),
        crate::config::ServiceConfig::Docker {
            readiness_host: None,
            runtime: crate::config::ContainerRuntimeKind::Docker,
            image: "postgres:16-alpine".into(),
            image_env: None,
            external_env: Some(name.into()),
            inject_env: "TEST_DATABASE_URL".into(),
            startup_timeout_secs: 1,
            timeout_env: None,
            container_port: 5432,
            environment: Default::default(),
            healthcheck: vec!["true".into()],
            connection: "postgres://unused".into(),
        },
    );
    project.config.doctor.checks = vec![crate::config::DoctorCheck {
        id: "service.database".into(),
        label: "test database".into(),
        required: true,
        help: None,
        timeout_secs: 5,
        kind: crate::config::DoctorCheckKind::Service {
            service: "database".into(),
        },
    }];

    crate::recipes::tests::install_fixture(&mut project, "database", false);
    std::env::set_var(name, "opaque-invalid");
    let report = super::run(&project).expect("doctor run");
    assert_eq!(
        report.failures, 1,
        "an invalid recipe value must fail the doctor service check"
    );

    crate::recipes::tests::install_fixture(&mut project, "database", true);
    std::env::set_var(name, "opaque-valid");
    let report = super::run(&project).expect("doctor run");
    assert_eq!(
        report.failures, 0,
        "a valid recipe value must pass the doctor service check"
    );
    std::env::remove_var(name);
}

#[test]
fn repository_scoped_paths_cannot_probe_outside_the_repository() {
    let (root, project) = doctor_project("doctor-scope");
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("credentials");
    fs::write(&secret, "TOKEN=outside\n").unwrap();
    fs::write(root.root.join("inside.txt"), "TOKEN=inside\n").unwrap();
    let absolute = secret.to_string_lossy().to_string();
    let link = root.root.join("escape");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();

    for value in [absolute.as_str(), "../outside.txt", "{root}/../outside.txt"]
        .into_iter()
        .chain(cfg!(unix).then_some("escape/credentials"))
    {
        let error = check_path(&project, value, &PathType::Any, PathScope::Repository)
            .expect_err("repository scope must reject an outside path");
        assert!(
            format!("{error:#}").contains("path_scope = \"host\""),
            "{value}: {error:#}"
        );
        let error = check_env_or_file(
            &project,
            "GH315_UNSET_ENV",
            value,
            "TOKEN=",
            PathScope::Repository,
        )
        .expect_err("repository scope must not read an outside file");
        assert!(
            !format!("{error:#}").contains("outside\n"),
            "content must not be echoed"
        );
    }

    check_path(
        &project,
        "inside.txt",
        &PathType::File,
        PathScope::Repository,
    )
    .expect("relative repository path");
    check_path(
        &project,
        "{root}/inside.txt",
        &PathType::File,
        PathScope::Repository,
    )
    .expect("placeholder repository path");
    check_path(&project, &absolute, &PathType::File, PathScope::Host).expect("explicit host path");
    let found = check_env_or_file(
        &project,
        "GH315_UNSET_ENV",
        &absolute,
        "TOKEN=",
        PathScope::Host,
    )
    .expect("explicit host file");
    assert!(
        !found.contains("outside"),
        "detail must not echo file content: {found}"
    );
}
