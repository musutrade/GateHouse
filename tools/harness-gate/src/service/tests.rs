use super::lease::{cleanup, ResourceLease};
use super::{check_available, ServiceManager};
use crate::config::ServiceConfig;
use crate::project::Project;
use crate::test_support::TestWorkspace;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[test]
fn cross_process_resource_lease_reports_a_conflict_and_can_be_reacquired() {
    let (_workspace, project) = project();
    let resource = format!("workspace:lease-conflict:{}", std::process::id());
    let lease = ResourceLease::acquire(
        &project,
        resource.clone(),
        "workspace",
        "invocation-a",
        None,
        None,
    )
    .expect("first owner acquires lease");
    let conflict = ResourceLease::acquire(
        &project,
        resource.clone(),
        "workspace",
        "invocation-b",
        None,
        None,
    )
    .expect_err("second owner must be rejected");
    assert!(conflict.to_string().contains("resource lease conflict"));
    drop(lease);
    ResourceLease::acquire(&project, resource, "workspace", "invocation-b", None, None)
        .expect("released lease can be acquired");
}

#[test]
fn releasing_a_lease_leaves_no_marker_file() {
    let (_workspace, project) = project();
    let resource = format!("workspace:lease-no-leak:{}", std::process::id());
    let lease = ResourceLease::acquire(
        &project,
        resource,
        "workspace",
        "invocation-no-leak",
        None,
        None,
    )
    .expect("lease");
    let lease_path = std::fs::read_dir(&project.resource_leases)
        .expect("lease directory")
        .next()
        .expect("lease entry")
        .expect("lease entry result")
        .path();
    assert!(lease_path.is_file());
    drop(lease);
    assert!(
        !lease_path.exists(),
        "released lease marker must be removed"
    );
    let report = cleanup(&project, false).expect("cleanup after release");
    assert_eq!(report.scanned, 0, "cleanup must find no released lease");
}

#[test]
fn cleanup_dry_run_lists_active_marked_resources_without_removing_them() {
    let (_workspace, project) = project();
    let resource = format!("workspace:lease-dry-run:{}", std::process::id());
    let lease = ResourceLease::acquire(
        &project,
        resource,
        "workspace",
        "invocation-dry-run",
        None,
        None,
    )
    .expect("lease");
    let report = cleanup(&project, true).expect("cleanup dry run");
    assert_eq!(report.scanned, 1);
    assert_eq!(report.active, 1);
    assert_eq!(report.reclaimed, 0);
    assert!(report.failures.is_empty());
    drop(lease);
}

#[test]
fn cleanup_reclaims_a_stale_marked_lease() {
    let (_workspace, project) = project();
    let lease = ResourceLease::acquire(
        &project,
        format!("workspace:lease-stale:{}", std::process::id()),
        "workspace",
        "invocation-stale",
        None,
        None,
    )
    .expect("lease");
    // Freeze the owner before forging an expired marker. Otherwise the
    // background heartbeat can race the fixture and make the result depend
    // on the CI scheduler. `retain` consumes the lease and leaves its marker
    // for cleanup to inspect.
    lease.retain();
    let path = std::fs::read_dir(&project.resource_leases)
        .expect("lease directory")
        .next()
        .expect("lease entry")
        .expect("lease entry result")
        .path();
    let mut record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("lease contents")).expect("json");
    record["pid"] = serde_json::json!(0);
    record["process_start_identity"] = serde_json::json!(proven_identity_fixture());
    record["expires_at"] = serde_json::json!(0);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&record).expect("serialize"),
    )
    .expect("mark stale");
    let report = cleanup(&project, false).expect("cleanup stale lease");
    assert_eq!(report.stale, 1);
    assert_eq!(report.reclaimed, 1);
    assert!(report.failures.is_empty());
    assert!(!path.exists());
}

fn proven_identity_fixture() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux:1"
    }
    #[cfg(target_os = "macos")]
    {
        "macos:1:0"
    }
    #[cfg(target_os = "windows")]
    {
        "windows:1"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "unavailable:test"
    }
}

fn project() -> (TestWorkspace, Project) {
    let workspace = TestWorkspace::new("service-manager");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    (workspace, project)
}

#[test]
fn environment_service_injects_configured_value() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_SERVICE_FIXTURE";
    std::env::set_var(name, "http://127.0.0.1:1234");
    project.config.services.insert(
        "fixture".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "FIXTURE_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    let value = manager.environment("fixture").expect("environment service");
    assert_eq!(
        value,
        ("FIXTURE_URL".into(), "http://127.0.0.1:1234".into())
    );
    manager.cleanup().expect("service cleanup");
    std::env::remove_var(name);
}

#[test]
fn cleanup_surfaces_errors_recorded_by_a_service_resource() {
    let (_workspace, mut project) = project();
    project.config.services.insert(
        "fixture".into(),
        ServiceConfig::Environment {
            source_env: "HARNESS_GATE_CLEANUP_FIXTURE".into(),
            inject_env: "FIXTURE_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    manager.handle("fixture").expect("register service");
    let resource = manager.resources.get("fixture").expect("resource");
    resource
        .cleanup_errors
        .lock()
        .expect("cleanup error lock")
        .push("simulated cleanup failure".into());

    let error = manager
        .cleanup()
        .expect_err("cleanup failure must propagate");
    assert!(error.to_string().contains("simulated cleanup failure"));
}

#[test]
fn warmup_populates_resource_before_the_first_lease() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_WARMUP_SERVICE_FIXTURE";
    std::env::set_var(name, "http://127.0.0.1:4322");
    project.config.services.insert(
        "warmup".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "WARMUP_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    let jobs = manager
        .warmup_jobs(vec!["warmup".into()])
        .expect("warmup job");
    assert_eq!(jobs.len(), 1);
    let cancelled = AtomicBool::new(false);
    jobs.into_iter().next().expect("warmup job").run(&cancelled);
    assert_eq!(
        manager.environment("warmup").expect("warmed service"),
        ("WARMUP_URL".into(), "http://127.0.0.1:4322".into())
    );
    std::env::remove_var(name);
}

#[test]
fn failed_warmup_is_cached_for_the_first_real_lease() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_FAILED_WARMUP_SERVICE_FIXTURE";
    std::env::remove_var(name);
    project.config.services.insert(
        "warmup-failed".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "WARMUP_FAILED_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    let jobs = manager
        .warmup_jobs(vec!["warmup-failed".into()])
        .expect("warmup job");
    jobs.into_iter()
        .next()
        .expect("warmup job")
        .run(&AtomicBool::new(false));
    let error = manager
        .environment("warmup-failed")
        .expect_err("failed warmup must fail the real lease");
    assert!(error.to_string().contains(name));
}

#[test]
fn missing_service_is_cached_as_a_failure() {
    let (_workspace, project) = project();
    let mut manager = ServiceManager::new(&project);
    let first = manager.environment("unknown").expect_err("unknown service");
    let second = manager.environment("unknown").expect_err("cached failure");
    assert!(first.to_string().contains("unknown service"));
    assert!(second.to_string().contains("unknown service"));
}

#[test]
fn check_available_reports_unknown_service_and_environment_service() {
    let (_workspace, mut project) = project();
    let error =
        check_available(&project, "unknown", Duration::from_secs(1)).expect_err("unknown service");
    assert!(error.to_string().contains("unknown service"));
    let name = "HARNESS_GATE_CHECK_SERVICE_FIXTURE";
    std::env::set_var(name, "configured");
    project.config.services.insert(
        "fixture".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "FIXTURE".into(),
        },
    );
    assert_eq!(
        check_available(&project, "fixture", Duration::from_secs(1)).expect("available"),
        format!("{name} is configured")
    );
    std::env::remove_var(name);
}

#[test]
fn environment_service_rejects_missing_and_empty_values() {
    let (_workspace, mut project) = project();
    let missing = "HARNESS_GATE_MISSING_SERVICE_FIXTURE";
    std::env::remove_var(missing);
    project.config.services.insert(
        "missing".into(),
        ServiceConfig::Environment {
            source_env: missing.into(),
            inject_env: "MISSING".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    assert!(manager.environment("missing").is_err());

    let empty = "HARNESS_GATE_EMPTY_SERVICE_FIXTURE";
    std::env::set_var(empty, "   ");
    project.config.services.insert(
        "empty".into(),
        ServiceConfig::Environment {
            source_env: empty.into(),
            inject_env: "EMPTY".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    assert!(manager.environment("empty").is_err());
    std::env::remove_var(empty);
}

#[test]
fn shareable_environment_service_supports_concurrent_leases() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_SHARED_SERVICE_FIXTURE";
    std::env::set_var(name, "http://127.0.0.1:4321");
    project.config.services.insert(
        "shared".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "SHARED_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    let first = manager.handle("shared").expect("first handle");
    let second = manager.handle("shared").expect("second handle");
    std::thread::scope(|scope| {
        let first = scope.spawn(move || first.acquire().expect("first lease"));
        let second = scope.spawn(move || second.acquire().expect("second lease"));
        assert_eq!(
            first.join().expect("first worker").environment().1,
            "http://127.0.0.1:4321"
        );
        assert_eq!(
            second.join().expect("second worker").environment().1,
            "http://127.0.0.1:4321"
        );
    });
    std::env::remove_var(name);
}

#[test]
fn failed_service_startup_wakes_waiters_without_deadlock() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_FAILED_SHARED_SERVICE_FIXTURE";
    std::env::remove_var(name);
    project.config.services.insert(
        "missing".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "MISSING_URL".into(),
        },
    );
    let mut manager = ServiceManager::new(&project);
    let first = manager.handle("missing").expect("first handle");
    let second = manager.handle("missing").expect("second handle");
    std::thread::scope(|scope| {
        let first = scope.spawn(move || first.acquire());
        let second = scope.spawn(move || second.acquire());
        assert!(first.join().expect("first worker").is_err());
        assert!(second.join().expect("second worker").is_err());
    });
}

#[test]
fn check_available_rejects_empty_environment_service() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_EMPTY_CHECK_SERVICE_FIXTURE";
    std::env::set_var(name, "");
    project.config.services.insert(
        "empty".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "EMPTY".into(),
        },
    );
    assert!(check_available(&project, "empty", Duration::from_secs(1)).is_err());
    std::env::remove_var(name);
}

#[test]
fn docker_service_uses_a_valid_external_value_without_docker() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_DOCKER_EXTERNAL_SERVICE_FIXTURE";
    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/quality_test");
    project.config.services.insert(
        "database".into(),
        ServiceConfig::Docker {
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
    let mut manager = ServiceManager::new(&project);
    assert_eq!(
        manager.environment("database").expect("external value"),
        (
            "TEST_DATABASE_URL".into(),
            "postgres://test:test@127.0.0.1:5432/quality_test".into()
        )
    );
    std::env::remove_var(name);
}

#[test]
#[cfg(unix)]
fn docker_service_rejects_an_invalid_external_value_before_any_runtime() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_DOCKER_INVALID_EXTERNAL_FIXTURE";
    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/quality");
    project.config.services.insert(
        "database".into(),
        ServiceConfig::Docker {
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
    crate::recipes::tests::install_fixture(&mut project, "database", false);
    let mut manager = ServiceManager::new(&project);
    let error = manager
        .environment("database")
        .expect_err("a production-like database name must fail closed");
    assert!(error.to_string().contains("reported failure"), "{error:?}");
    std::env::remove_var(name);
}

#[test]
fn exclusive_external_service_is_not_reused_after_its_last_lease() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_EXCLUSIVE_EXTERNAL_REUSE_FIXTURE";
    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/first_test");
    project.config.services.insert(
        "database".into(),
        ServiceConfig::Docker {
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

    let mut manager = ServiceManager::new(&project);
    let first = manager
        .handle("database")
        .expect("first service handle")
        .acquire()
        .expect("first external service lease");
    assert!(first.environment().1.ends_with("/first_test"));
    drop(first);

    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/second_test");
    let second = manager
        .handle("database")
        .expect("second service handle")
        .acquire()
        .expect("second external service lease");
    assert!(second.environment().1.ends_with("/second_test"));
    std::env::remove_var(name);
}

#[test]
fn podman_runtime_uses_external_service_without_starting_a_container() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_PODMAN_EXTERNAL_SERVICE_FIXTURE";
    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/quality_test");
    project.config.services.insert(
        "database".into(),
        ServiceConfig::Docker {
            runtime: crate::config::ContainerRuntimeKind::Podman,
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
    let mut manager = ServiceManager::new(&project);
    assert_eq!(
        manager.environment("database").expect("external value").0,
        "TEST_DATABASE_URL"
    );
    std::env::remove_var(name);
}

#[cfg(unix)]
#[test]
fn managed_service_lease_excludes_concurrent_users() {
    let (_workspace, mut project) = project();
    let name = "HARNESS_GATE_EXCLUSIVE_SERVICE_FIXTURE";
    std::env::set_var(name, "postgres://test:test@127.0.0.1:5432/quality_test");
    project.config.services.insert(
        "exclusive".into(),
        ServiceConfig::Docker {
            runtime: crate::config::ContainerRuntimeKind::Docker,
            image: "postgres:16-alpine".into(),
            image_env: None,
            external_env: Some(name.into()),
            inject_env: "EXCLUSIVE_URL".into(),
            startup_timeout_secs: 1,
            timeout_env: None,
            container_port: 5432,
            environment: Default::default(),
            healthcheck: vec!["true".into()],
            connection: "postgres://unused".into(),
        },
    );

    let mut manager = ServiceManager::new(&project);
    let first = manager
        .handle("exclusive")
        .expect("first service handle")
        .acquire()
        .expect("first service lease");
    let second_handle = manager.handle("exclusive").expect("second service handle");

    std::thread::scope(|scope| {
        let second = scope.spawn(move || second_handle.acquire());
        std::thread::sleep(Duration::from_millis(100));
        assert!(!second.is_finished(), "exclusive resource must be held");
        drop(first);
        second
            .join()
            .expect("second lease worker")
            .expect("second lease after release");
    });
    std::env::remove_var(name);
}

#[cfg(unix)]
#[test]
fn managed_availability_keeps_check_daemon_and_image_failures_blocking() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    if std::env::var_os("HG_MANAGED_AVAILABILITY_CHILD").is_none() {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("docker");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$HG_FAKE_RUNTIME_LOG"
case "$1" in
 info) exit "${HG_FAKE_INFO_EXIT:-0}";;
 image) exit "${HG_FAKE_IMAGE_EXIT:-0}";;
 *) exit 99;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut paths = vec![root.path().to_path_buf()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "service::tests::managed_availability_keeps_check_daemon_and_image_failures_blocking", "--nocapture"])
            .env("HG_MANAGED_AVAILABILITY_CHILD", "1")
            .env("HG_FAKE_RUNTIME_LOG", root.path().join("runtime.log"))
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let (_workspace, mut project) = project();
    std::env::remove_var("HG_MANAGED_EXTERNAL_VALUE");
    project.config.services.insert(
        "database".into(),
        ServiceConfig::Docker {
            runtime: crate::config::ContainerRuntimeKind::Docker,
            image: "fixture-image".into(),
            image_env: None,
            external_env: Some("HG_MANAGED_EXTERNAL_VALUE".into()),
            inject_env: "TEST_DATABASE_URL".into(),
            startup_timeout_secs: 5,
            timeout_env: None,
            container_port: 1234,
            environment: Default::default(),
            healthcheck: vec!["true".into()],
            connection: "opaque:{host_port}".into(),
        },
    );
    let bind_check = |project: &mut Project, success| {
        crate::recipes::tests::install_fixture(project, "database", success);
        let path =
            std::path::PathBuf::from(&project.config.service_checks["database"][0].executable);
        let source = std::fs::read_to_string(&path).unwrap();
        let managed = source.replace("\"mode\":\"external\"", "\"mode\":\"managed\"");
        assert_ne!(managed, source);
        std::fs::write(&path, managed).unwrap();
        project.config.service_checks.get_mut("database").unwrap()[0].sha256 =
            format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()));
    };
    let log = std::path::PathBuf::from(std::env::var_os("HG_FAKE_RUNTIME_LOG").unwrap());
    bind_check(&mut project, false);
    std::fs::write(&log, "").unwrap();
    assert!(check_available(&project, "database", Duration::from_secs(5)).is_err());
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "",
        "required check must precede runtime commands"
    );
    bind_check(&mut project, true);
    for (info, image, success, calls) in [
        ("1", "0", false, "info\n"),
        ("0", "1", false, "info\nimage inspect fixture-image\n"),
        ("0", "0", true, "info\nimage inspect fixture-image\n"),
    ] {
        std::env::set_var("HG_FAKE_INFO_EXIT", info);
        std::env::set_var("HG_FAKE_IMAGE_EXIT", image);
        std::fs::write(&log, "").unwrap();
        assert_eq!(
            check_available(&project, "database", Duration::from_secs(5)).is_ok(),
            success
        );
        assert_eq!(std::fs::read_to_string(&log).unwrap(), calls);
    }
    std::env::set_var("HG_MANAGED_EXTERNAL_VALUE", "");
    assert!(check_available(&project, "database", Duration::from_secs(5)).is_ok());
    if let ServiceConfig::Docker { external_env, .. } =
        project.config.services.get_mut("database").unwrap()
    {
        *external_env = None;
    }
    assert!(check_available(&project, "database", Duration::from_secs(5)).is_ok());
    assert!(check_available(&project, "unknown", Duration::from_secs(5)).is_err());
    project.config.services.insert(
        "environment".into(),
        ServiceConfig::Environment {
            source_env: "HG_MANAGED_ENVIRONMENT_VALUE".into(),
            inject_env: "FIXTURE_VALUE".into(),
        },
    );
    std::env::remove_var("HG_MANAGED_ENVIRONMENT_VALUE");
    assert!(check_available(&project, "environment", Duration::from_secs(5)).is_err());
    std::env::set_var("HG_MANAGED_ENVIRONMENT_VALUE", "");
    assert!(check_available(&project, "environment", Duration::from_secs(5)).is_err());
    crate::recipes::tests::install_fixture(&mut project, "environment", true);
    std::env::set_var("HG_MANAGED_ENVIRONMENT_VALUE", "opaque");
    assert!(check_available(&project, "environment", Duration::from_secs(5)).is_ok());
    crate::recipes::tests::install_fixture(&mut project, "environment", false);
    assert!(check_available(&project, "environment", Duration::from_secs(5)).is_err());
}

#[cfg(unix)]
#[test]
fn managed_teardown_removes_verified_id_and_retains_failed_cleanup() {
    use super::inspection::RuntimeInspection;
    use crate::config::ContainerRuntimeKind;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};
    if std::env::var_os("HG_ID_TEARDOWN_CHILD").is_none() {
        let directory = tempfile::tempdir().unwrap();
        for executable in ["docker", "podman"] {
            let binary = directory.path().join(executable);
            std::fs::write(
                &binary,
                r#"#!/bin/sh
case "$1" in
 container) printf '%s' "$HG_ID_INSPECTION";;
 rm) printf '%s\n' "$*" >> "$HG_ID_REMOVE_LOG"; exit "$HG_ID_REMOVE_EXIT";;
 *) exit 99;;
esac
"#,
            )
            .unwrap();
            std::fs::set_permissions(binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut paths = vec![directory.path().to_path_buf()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::tests::managed_teardown_removes_verified_id_and_retains_failed_cleanup",
                "--nocapture",
            ])
            .env("HG_ID_TEARDOWN_CHILD", "1")
            .env("HG_ID_REMOVE_LOG", directory.path().join("removed"))
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for runtime in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman] {
        for remove_fails in [false, true] {
            let (_workspace, project) = project();
            let name = "harness-gate-reused-name";
            let id = "0123456789abcdef".repeat(4);
            let resource = "service:teardown-id";
            let invocation = "teardown-id-fixture";
            let labels = super::lease::ownership_labels(
                &project.input().project_identity,
                resource,
                "container",
                invocation,
            );
            let lease = ResourceLease::acquire(
                &project,
                resource,
                "container",
                invocation,
                Some(name.into()),
                Some(runtime),
            )
            .unwrap();
            lease
                .bind_runtime_identity(
                    &project,
                    &RuntimeInspection {
                        object_id: id.clone(),
                        name: name.into(),
                        labels: labels.clone(),
                    },
                )
                .unwrap();
            let marker = std::fs::read_dir(&project.resource_leases)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            std::env::set_var(
                "HG_ID_INSPECTION",
                serde_json::json!({"Id": id, "Name": name, "Config": {"Labels": labels}})
                    .to_string(),
            );
            std::env::set_var("HG_ID_REMOVE_EXIT", if remove_fails { "1" } else { "0" });
            let errors = Arc::new(Mutex::new(Vec::new()));
            std::fs::write(std::env::var_os("HG_ID_REMOVE_LOG").unwrap(), "").unwrap();
            drop(super::RunningService {
                runtime,
                inject_env: String::new(),
                value: String::new(),
                container: Some(name.into()),
                project,
                cleanup_errors: Arc::clone(&errors),
                lease: Some(lease),
            });
            assert_eq!(std::fs::read_to_string(std::env::var_os("HG_ID_REMOVE_LOG").unwrap()).unwrap(), format!("rm --force {id}\n"),
                "normal teardown must carry the inspected immutable ID to the CLI, not resolve the reused name");
            assert_eq!(marker.exists(), remove_fails);
            assert_eq!(errors.lock().unwrap().is_empty(), !remove_fails);
        }
    }
}
