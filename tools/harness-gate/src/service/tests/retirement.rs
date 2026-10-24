//! Exercise public entrypoints with trap engines on PATH in an isolated process.
#[cfg(unix)]
#[test]
fn retired_engines_never_execute_or_allocate_and_legacy_cleanup_retains_evidence() {
    use super::{project, proven_identity_fixture};
    use crate::config::ContainerRuntimeKind;
    use crate::service::{check_available, cleanup_resources, ResourceLease, ServiceManager};
    use std::{os::unix::fs::PermissionsExt, time::Duration};

    const CHILD: &str = "HG_ENGINE_RETIREMENT_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "service::tests::retirement::retired_engines_never_execute_or_allocate_and_legacy_cleanup_retains_evidence", "--nocapture"])
            .env(CHILD, "1")
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let bins = tempfile::tempdir().unwrap();
    let log = bins.path().join("calls");
    std::fs::write(&log, "").unwrap();
    for engine in ["docker", "podman"] {
        let script = bins.path().join(engine);
        std::fs::write(&script, "#!/bin/sh\nprintf 'unexpected engine call\\n' >> \"$HG_ENGINE_RETIREMENT_LOG\"\nexit 91\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::env::set_var("HG_ENGINE_RETIREMENT_LOG", &log);
    let mut paths = vec![bins.path().to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
    for runtime in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman] {
        let (_workspace, mut project) = project();
        let name = "HG_ENGINE_RETIREMENT_EXTERNAL";
        for external in [None, Some(""), Some("   ")] {
            if let Some(value) = external {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
            project.config.services.insert("fixture".into(), serde_json::from_value(serde_json::json!({
                "kind":"docker", "runtime":runtime.executable(), "image":"fixture", "external_env":name,
                "inject_env":"FIXTURE_URL", "startup_timeout_secs":1, "container_port":1234,
                "environment":{}, "healthcheck":["check"], "connection":"{host_port}"
            })).unwrap());
            let mut manager = ServiceManager::new(&project);
            let error = manager.handle("fixture").unwrap().acquire().err().unwrap();
            assert!(
                format!("{error:#}").contains("requires an explicit lifecycle_host"),
                "{error:#}"
            );
            let error = check_available(&project, "fixture", Duration::from_secs(1)).unwrap_err();
            assert!(
                format!("{error:#}").contains("CLI service operations are retired"),
                "{error:#}"
            );
            if project.resource_leases.exists() {
                assert!(!std::fs::read_dir(&project.resource_leases)
                    .unwrap()
                    .any(|entry| entry
                        .unwrap()
                        .path()
                        .extension()
                        .is_some_and(|ext| ext == "json")));
            }
        }
        // A stale legacy record is not permission to call an engine or adopt it
        // into today's installation. Both dry-run and apply preserve its bytes.
        ResourceLease::acquire(
            &project,
            "service:fixture",
            "container",
            "old-owner",
            Some("harness-gate-old-container".into()),
            Some(runtime),
        )
        .unwrap()
        .retain();
        let marker = project.resource_leases.join(format!(
            "{}.json",
            crate::service::lease::resource_key("service:fixture")
        ));
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
        record["runtime_object_id"] = serde_json::json!("retained-immutable-id");
        record["runtime_labels"] = serde_json::json!(crate::service::lease::ownership_labels(
            &project.input().project_identity,
            "service:fixture",
            "container",
            "old-owner"
        ));
        record["pid"] = serde_json::json!(0);
        record["process_start_identity"] = serde_json::json!(proven_identity_fixture());
        record["expires_at"] = serde_json::json!(0);
        let bytes = serde_json::to_vec_pretty(&record).unwrap();
        std::fs::write(&marker, &bytes).unwrap();
        for dry_run in [true, false, false] {
            let report = cleanup_resources(&project, dry_run).unwrap();
            assert_eq!(report.reclaimed, 0);
            if dry_run {
                assert_eq!(report.resources[0].action, "retained");
                assert!(report
                    .failures
                    .iter()
                    .any(|error| error.contains("lease retained without migration")));
            }
            if !dry_run {
                assert!(
                    report
                        .failures
                        .iter()
                        .any(|error| error.contains("lease retained without migration")),
                    "{report:?}"
                );
            }
            assert_eq!(std::fs::read(&marker).unwrap(), bytes);
        }
        let Err(error) = ResourceLease::acquire(
            &project,
            "service:fixture",
            "container",
            "new-owner",
            Some("harness-gate-new-container".into()),
            Some(runtime),
        ) else {
            panic!("a stale legacy reservation must not be stolen");
        };
        assert!(
            format!("{error:#}").contains("lease retained without migration"),
            "{error:#}"
        );
        assert_eq!(std::fs::read(&marker).unwrap(), bytes);
        // A newly configured host must not implicitly adopt the old marker.
        #[cfg(target_os = "linux")]
        {
            if let crate::config::ServiceConfig::Docker {
                lifecycle_host,
                external_env,
                ..
            } = project.config.services.get_mut("fixture").unwrap()
            {
                *lifecycle_host = Some("/missing-protected-host.json".into());
                *external_env = None;
            }
            let mut manager = ServiceManager::new(&project);
            let error = manager.handle("fixture").unwrap().acquire().err().unwrap();
            assert!(
                format!("{error:#}").contains("legacy service lease still exists"),
                "{error:#}"
            );
            assert_eq!(std::fs::read(&marker).unwrap(), bytes);
        }
    }
    assert!(std::fs::read_to_string(log).unwrap().is_empty());
}
