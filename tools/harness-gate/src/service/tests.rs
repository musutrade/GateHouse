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
            readiness_host: None,
            lifecycle_host: None,
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
            command: None,
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
            readiness_host: None,
            lifecycle_host: None,
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
            command: None,
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
            readiness_host: None,
            lifecycle_host: None,
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
            command: None,
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
            readiness_host: None,
            lifecycle_host: None,
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
            command: None,
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
            readiness_host: None,
            lifecycle_host: None,
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
            command: None,
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

mod availability;
mod retirement;

#[cfg(target_os = "linux")]
#[test]
fn installed_lifecycle_start_refuses_existing_legacy_marker_without_rewrite() {
    let (_workspace, mut project) = project();
    let lease = ResourceLease::acquire(
        &project,
        "service:fixture",
        "container",
        "old-owner",
        Some("old-container".into()),
        Some(crate::config::ContainerRuntimeKind::Docker),
    )
    .unwrap();
    lease.retain();
    let marker = project.resource_leases.join(format!(
        "{}.json",
        super::lease::resource_key("service:fixture")
    ));
    for bytes in [
        std::fs::read(&marker).unwrap(),
        b"invalid-old-marker".to_vec(),
    ] {
        std::fs::write(&marker, &bytes).unwrap();
        project.config.services.insert("fixture".into(), serde_json::from_value(serde_json::json!({
            "kind":"docker", "runtime":"docker", "image":"fixture", "lifecycle_host":"/missing-host.json",
            "inject_env":"FIXTURE", "startup_timeout_secs":1, "container_port":1234,
            "environment":{}, "healthcheck":[], "connection":"{host_port}"
        })).unwrap());
        let mut manager = ServiceManager::new(&project);
        let error = manager.handle("fixture").unwrap().acquire().err().unwrap();
        assert!(
            format!("{error:#}").contains("legacy service lease still exists"),
            "{error:#}"
        );
        assert_eq!(std::fs::read(&marker).unwrap(), bytes);
        assert!(!project
            .root
            .join(".harness-gate/lifecycle-services")
            .exists());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn installed_lifecycle_start_retains_dangling_legacy_symlink() {
    let (_workspace, mut project) = project();
    std::fs::create_dir_all(&project.resource_leases).unwrap();
    let marker = project.resource_leases.join(format!(
        "{}.json",
        super::lease::resource_key("service:fixture")
    ));
    let target = project.root.join("missing-old-lease");
    std::os::unix::fs::symlink(&target, &marker).unwrap();
    project.config.services.insert("fixture".into(), serde_json::from_value(serde_json::json!({
        "kind":"docker", "runtime":"docker", "image":"fixture", "lifecycle_host":"/missing-host.json",
        "inject_env":"FIXTURE", "startup_timeout_secs":1, "container_port":1234,
        "environment":{}, "healthcheck":[], "connection":"{host_port}"
    })).unwrap());
    let mut manager = ServiceManager::new(&project);
    let error = manager.handle("fixture").unwrap().acquire().err().unwrap();
    assert!(
        format!("{error:#}").contains("legacy service lease still exists"),
        "{error:#}"
    );
    assert_eq!(std::fs::read_link(&marker).unwrap(), target);
    assert!(!target.exists());
    assert!(!project
        .root
        .join(".harness-gate/lifecycle-services")
        .exists());
}

#[cfg(target_os = "linux")]
fn podman_lifecycle_fixture(
    product: &str,
    version: &str,
    engine: &str,
) -> (tempfile::TempDir, String) {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::var_os("HOME").expect("HOME required");
    let dir = tempfile::Builder::new()
        .prefix("lifecycle-podman-fixture-")
        .tempdir_in(root)
        .unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let executable = dir.path().join("adapter");
    std::fs::write(&executable, b"fixture never executed").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let uid = unsafe { libc::geteuid() };
    let config = serde_json::json!({
        "schema":"gate-runtime/lifecycle-config-v1",
        "engine_binding":{
            "engine":engine,
            "transport":"unix-http",
            "socket_path":format!("/run/user/{uid}/podman/podman.sock"),
            "socket_uid":uid,
            "principal_uid":uid,
            "rootless":true,
            "api_version":"5.7.0",
            "server_version":"5.7.0",
            "daemon_id":"fixture-host",
            "scope":"/fixture/storage"
        }
    });
    let config_bytes = serde_json::to_vec(&config).unwrap();
    let config_path = dir.path().join("config.json");
    std::fs::write(&config_path, &config_bytes).unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let installation = serde_json::json!({
        "schema":"gate-runtime/lifecycle-installation-v1",
        "product":product,
        "version":version,
        "protocol":"gate-runtime/lifecycle-v1",
        "executable_path":executable,
        "executable_sha256":format!("{:x}",Sha256::digest(b"fixture never executed")),
        "config_schema":"gate-runtime/lifecycle-config-v1"
    });
    let installation_bytes = serde_json::to_vec(&installation).unwrap();
    let installation_path = dir.path().join("installation.json");
    std::fs::write(&installation_path, &installation_bytes).unwrap();
    std::fs::set_permissions(&installation_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let host = serde_json::json!({
        "schema":"gate-runtime/lifecycle-host-v1",
        "installation_path":installation_path,
        "installation_sha256":format!("{:x}",Sha256::digest(&installation_bytes)),
        "config_path":config_path,
        "config_sha256":format!("{:x}",Sha256::digest(&config_bytes))
    });
    let host_bytes = serde_json::to_vec(&host).unwrap();
    let host_path = dir.path().join("host.json");
    std::fs::write(&host_path, &host_bytes).unwrap();
    std::fs::set_permissions(&host_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    (dir, host_path.to_str().unwrap().into())
}

#[cfg(target_os = "linux")]
#[test]
fn podman_lifecycle_configuration_loads_and_requires_matching_explicit_runtime() {
    let (_workspace, mut project) = project();
    let (_fixture, host) = podman_lifecycle_fixture("podman-lifecycle", "0.3.0", "podman");
    project.config.services.insert(
        "fixture".into(),
        serde_json::from_value(serde_json::json!({
            "kind":"docker","runtime":"podman","image":"fixture","lifecycle_host":host,
            "inject_env":"FIXTURE","startup_timeout_secs":1,"container_port":1234,
            "environment":{},"healthcheck":["true"],"command":["sleep","30"],"connection":"{host_port}"
        }))
        .unwrap(),
    );
    let available = super::check_available(&project, "fixture", std::time::Duration::from_secs(1))
        .expect("podman lifecycle installation preflight");
    assert!(
        available.contains("podman lifecycle installation"),
        "{available}"
    );

    let error = super::lifecycle_public::ManagedLifecycle::start(
        &project,
        "fixture",
        super::lifecycle_public::Config {
            host: host.clone(),
            image: "fixture".into(),
            container_port: 1234,
            environment: Default::default(),
            healthcheck: vec!["true".into()],
            command: Some(vec!["sleep".into(), "30".into()]),
        },
        crate::config::ContainerRuntimeKind::Docker,
        std::time::Instant::now() + std::time::Duration::from_secs(1),
    )
    .err()
    .expect("runtime mismatch must fail closed");
    assert!(
        format!("{error:#}").contains("does not match configured runtime"),
        "{error:#}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn capture_only_or_stale_installation_cannot_select_lifecycle_mutation() {
    let (_workspace, mut project) = project();
    for (product, version) in [("podman-lifecycle", "0.2.0"), ("docker-lifecycle", "0.1.0")] {
        let (_fixture, host) = podman_lifecycle_fixture(product, version, "podman");
        project.config.services.insert(
            "fixture".into(),
            serde_json::from_value(serde_json::json!({
                "kind":"docker","runtime":"podman","image":"fixture","lifecycle_host":host,
                "inject_env":"FIXTURE","startup_timeout_secs":1,"container_port":1234,
                "environment":{},"healthcheck":["true"],"connection":"{host_port}"
            }))
            .unwrap(),
        );
        let error = super::check_available(&project, "fixture", std::time::Duration::from_secs(1))
            .unwrap_err();
        let detail = format!("{error:#}");
        assert!(
            detail.contains("unsupported lifecycle installation")
                || detail.contains("product/engine mismatch"),
            "{product}/{version}: {detail}"
        );
    }
}

#[cfg(target_os = "linux")]
fn podman_response(
    request: &gatehouse_runtime_protocol::lifecycle::wire::Request,
    outcome: gatehouse_runtime_protocol::lifecycle::wire::Outcome,
) -> gatehouse_runtime_protocol::lifecycle::wire::Response {
    use gatehouse_runtime_protocol::lifecycle::wire::{Observation, Observations, Response};
    let binding = &request.binding.engine_binding;
    let observation = Observation {
        api_version: binding.api_version.clone(),
        server_version: binding.server_version.clone(),
        daemon_id: binding.daemon_id.clone(),
    };
    Response {
        protocol: request.protocol.clone(),
        binding: request.binding.clone(),
        operation: request.operation.clone(),
        outcome,
        observations: Some(Observations {
            observed_before: observation.clone(),
            observed_after: observation,
        }),
    }
}

#[cfg(target_os = "linux")]
fn podman_response_bytes(
    response: &gatehouse_runtime_protocol::lifecycle::wire::Response,
) -> Vec<u8> {
    serde_json::to_vec(&serde_json::to_value(response).unwrap()).unwrap()
}

#[cfg(target_os = "linux")]
#[test]
fn podman_slot_fences_concurrency_old_fence_and_explicit_absence_recovery() {
    use gatehouse_runtime_protocol::lifecycle::{
        installation::VerifiedInstallation,
        journal::{CreatePlan, OperatorRecovery, ServiceSlot},
        wire::{
            exact_absence, Binding, ContainerState, Entry, Object, Operation, Outcome, Port,
            Request, PROTOCOL,
        },
    };
    use std::os::unix::fs::PermissionsExt;

    let (_installation_dir, host) = podman_lifecycle_fixture("podman-lifecycle", "0.3.0", "podman");
    let installation = VerifiedInstallation::load(std::path::Path::new(&host)).unwrap();
    let snapshot = installation.snapshot().unwrap();
    let project = "podman-project";
    let resource = "service:fixture";
    let invocation = "1".repeat(32);
    let labels = vec![
        Entry {
            key: "gatehouse.lifecycle.authority".into(),
            value: "container-lifecycle/mutate".into(),
        },
        Entry {
            key: "gatehouse.lifecycle.invocation".into(),
            value: invocation.clone(),
        },
        Entry {
            key: "gatehouse.lifecycle.project".into(),
            value: project.into(),
        },
        Entry {
            key: "gatehouse.lifecycle.resource".into(),
            value: resource.into(),
        },
    ];
    let plan = CreatePlan {
        name: "podman-fixture".into(),
        labels: labels.clone(),
        environment: vec![],
        container_port: 8080,
        command: Some(vec!["sleep".into(), "60".into()]),
    };
    let mut resolve = Request {
        protocol: PROTOCOL.into(),
        binding: Binding {
            installation_sha256: snapshot.installation_sha256,
            executable_sha256: snapshot.executable_sha256,
            config_sha256: snapshot.config_sha256,
            engine_binding: snapshot.engine_binding,
            project_id: project.into(),
            resource_id: resource.into(),
            lease_invocation: invocation.clone(),
            invocation: "2".repeat(32),
            generation: 1,
            operation_key: String::new(),
        },
        operation: Operation::ResolveImage {
            image: "fixture:latest".into(),
        },
    };
    resolve.binding.operation_key = resolve.operation_key().unwrap();
    let base = tempfile::Builder::new()
        .prefix("podman-slot-")
        .tempdir_in(std::env::var_os("HOME").unwrap())
        .unwrap();
    std::fs::set_permissions(base.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

    let mut slot = ServiceSlot::acquire_mapped(
        base.path(),
        resolve.clone(),
        plan.clone(),
        vec!["true".into()],
    )
    .unwrap();
    let fence = slot.fence();
    assert!(
        ServiceSlot::acquire_mapped(base.path(), resolve.clone(), plan.clone(), vec![]).is_err()
    );

    let mut reservation = slot.reserve().unwrap();
    let image_id = format!("sha256:{}", "e".repeat(64));
    let image = podman_response(
        &resolve,
        Outcome::Image {
            image_id: image_id.clone(),
            repo_digests: vec![],
        },
    );
    reservation
        .dispatch()
        .unwrap()
        .observe(Some(&podman_response_bytes(&image)))
        .unwrap();
    let mut journal = reservation.into_journal(&"3".repeat(32)).unwrap();
    let token = journal.dispatch().unwrap();
    let create = token.request().clone();
    let object = Object {
        image_id: image_id.clone(),
        object_id: "a".repeat(64),
        name: plan.name.clone(),
        labels: labels.clone(),
        state: ContainerState::Created,
        ports: vec![Port {
            container_port: plan.container_port,
            protocol: "tcp".into(),
            host_ip: "127.0.0.1".into(),
            host_port: 49152,
        }],
    };
    let created = podman_response(&create, Outcome::Created { object });
    token
        .observe(Some(&podman_response_bytes(&created)))
        .unwrap();
    drop(journal);
    drop(slot);

    let wrong = OperatorRecovery::open_mapped(
        base.path(),
        project,
        resource,
        fence + 1,
        &invocation,
        &"9".repeat(32),
    )
    .err()
    .expect("stale fence must not authorize recovery");
    assert!(wrong.to_string().contains("fence"), "{wrong}");

    let mut recovery = OperatorRecovery::open_mapped(
        base.path(),
        project,
        resource,
        fence,
        &invocation,
        &"8".repeat(32),
    )
    .unwrap();
    let target = recovery.target().unwrap();
    let token = recovery
        .begin(
            Operation::Status {
                target: target.clone(),
            },
            &"4".repeat(32),
        )
        .unwrap();
    let status = token.request().clone();
    let absence = exact_absence(&status.binding.engine_binding, &target).unwrap();
    let absent = podman_response(&status, Outcome::Absent { evidence: absence });
    token
        .observe(Some(&podman_response_bytes(&absent)))
        .unwrap();
    recovery.finish().unwrap();

    let mut fresh = resolve;
    fresh.binding.lease_invocation = "5".repeat(32);
    fresh.binding.invocation = "6".repeat(32);
    fresh.binding.operation_key = fresh.operation_key().unwrap();
    let mut fresh_plan = plan;
    for entry in &mut fresh_plan.labels {
        if entry.key == "gatehouse.lifecycle.invocation" {
            entry.value = "5".repeat(32);
        }
    }
    let released = ServiceSlot::acquire_mapped(base.path(), fresh, fresh_plan, vec![]);
    assert!(
        released.is_ok(),
        "verified absence must release the Podman fence: {:?}",
        released.err()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn podman_public_operator_cleanup_exercises_protected_guard_and_retains_failed_dispatch() {
    use gatehouse_runtime_protocol::lifecycle::{
        installation::VerifiedInstallation,
        journal::{CreatePlan, ServiceSlot},
        wire::{
            Binding, ContainerState, Entry, Object, Operation, Outcome, Port, Request, PROTOCOL,
        },
    };
    use std::os::unix::fs::PermissionsExt;

    let (_installation_dir, host) = podman_lifecycle_fixture("podman-lifecycle", "0.3.0", "podman");
    let installation = VerifiedInstallation::load(std::path::Path::new(&host)).unwrap();
    let snapshot = installation.snapshot().unwrap();
    let base = tempfile::Builder::new()
        .prefix("podman-operator-")
        .tempdir_in(std::env::var_os("HOME").unwrap())
        .unwrap();
    std::fs::set_permissions(base.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    crate::preset::init(base.path(), "generic", false).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(base.path())
        .status()
        .unwrap()
        .success());
    let mut project =
        crate::project::Project::discover(Some(base.path().to_path_buf()), None).unwrap();
    project.config.services.insert(
        "fixture".into(),
        serde_json::from_value(serde_json::json!({
            "kind":"docker", "runtime":"podman", "image":"fixture:latest",
            "lifecycle_host":host, "inject_env":"FIXTURE_URL",
            "startup_timeout_secs":1, "container_port":8080,
            "environment":{}, "healthcheck":["true"], "command":["sleep","60"],
            "connection":"tcp://127.0.0.1:{host_port}"
        }))
        .unwrap(),
    );
    let project_id = project.input().project_identity.clone();
    let resource = "service:fixture";
    let invocation = "1".repeat(32);
    let labels = vec![
        Entry {
            key: "gatehouse.lifecycle.authority".into(),
            value: "container-lifecycle/mutate".into(),
        },
        Entry {
            key: "gatehouse.lifecycle.invocation".into(),
            value: invocation.clone(),
        },
        Entry {
            key: "gatehouse.lifecycle.project".into(),
            value: project_id.clone(),
        },
        Entry {
            key: "gatehouse.lifecycle.resource".into(),
            value: resource.into(),
        },
    ];
    let plan = CreatePlan {
        name: "podman-fixture".into(),
        labels: labels.clone(),
        environment: vec![],
        container_port: 8080,
        command: Some(vec!["sleep".into(), "60".into()]),
    };
    let mut resolve = Request {
        protocol: PROTOCOL.into(),
        binding: Binding {
            installation_sha256: snapshot.installation_sha256,
            executable_sha256: snapshot.executable_sha256,
            config_sha256: snapshot.config_sha256,
            engine_binding: snapshot.engine_binding,
            project_id: project_id.clone(),
            resource_id: resource.into(),
            lease_invocation: invocation.clone(),
            invocation: "2".repeat(32),
            generation: 1,
            operation_key: String::new(),
        },
        operation: Operation::ResolveImage {
            image: "fixture:latest".into(),
        },
    };
    resolve.binding.operation_key = resolve.operation_key().unwrap();
    let slot_base = ServiceSlot::project_state_base(&project.root).unwrap();
    let mut slot = ServiceSlot::acquire_mapped(
        &slot_base,
        resolve.clone(),
        plan.clone(),
        vec!["true".into()],
    )
    .unwrap();
    let fence = slot.fence();
    let mut reservation = slot.reserve().unwrap();
    let image_id = format!("sha256:{}", "e".repeat(64));
    let image = podman_response(
        &resolve,
        Outcome::Image {
            image_id: image_id.clone(),
            repo_digests: vec![],
        },
    );
    reservation
        .dispatch()
        .unwrap()
        .observe(Some(&podman_response_bytes(&image)))
        .unwrap();
    let mut journal = reservation.into_journal(&"3".repeat(32)).unwrap();
    let token = journal.dispatch().unwrap();
    let create = token.request().clone();
    let object = Object {
        image_id,
        object_id: "a".repeat(64),
        name: plan.name.clone(),
        labels,
        state: ContainerState::Created,
        ports: vec![Port {
            container_port: plan.container_port,
            protocol: "tcp".into(),
            host_ip: "127.0.0.1".into(),
            host_port: 49152,
        }],
    };
    let created = podman_response(&create, Outcome::Created { object });
    token
        .observe(Some(&podman_response_bytes(&created)))
        .unwrap();
    drop(journal);
    drop(slot);

    let error = crate::service::cleanup_lifecycle(&project, "fixture", fence, &invocation)
        .expect_err("the dummy executable cannot dispatch");
    assert!(format!("{error:#}").contains("retained"), "{error:#}");
    assert!(base
        .path()
        .join(".harness-gate/lifecycle-services")
        .exists());
}
