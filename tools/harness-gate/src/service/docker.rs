use super::{remaining, RunningService};
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use crate::service::lease::ownership_labels;
use crate::service::runtime::{ContainerRuntime, ContainerStartOptions};
use crate::service::ResourceLease;
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) struct DockerStartOptions {
    pub(super) runtime: ContainerRuntimeKind,
    pub(super) image: String,
    pub(super) inject_env: String,
    pub(super) startup_timeout_secs: u64,
    pub(super) container_port: u16,
    pub(super) environment: BTreeMap<String, String>,
    pub(super) healthcheck: Vec<String>,
    pub(super) connection: String,
    pub(super) deadline: Instant,
    pub(super) cleanup_errors: Arc<Mutex<Vec<String>>>,
    pub(super) invocation_id: String,
}
pub(super) fn start_docker(
    project: &Project,
    id: &str,
    options: DockerStartOptions,
) -> Result<RunningService> {
    let DockerStartOptions {
        runtime,
        image,
        inject_env,
        startup_timeout_secs,
        container_port,
        environment,
        healthcheck,
        connection,
        deadline,
        cleanup_errors,
        invocation_id,
    } = options;
    super::legacy_cli::legacy_executable(runtime)?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!(
        "harness-gate-{}-{id}-{}-{unique}",
        project.config.project.name,
        std::process::id()
    );
    let lease = ResourceLease::acquire(
        project,
        format!("service:{id}"),
        "container",
        invocation_id.clone(),
        Some(name.clone()),
        Some(runtime),
    )?;
    let executable = runtime.executable();
    if let Err(error) = runtime.start_container(
        project,
        ContainerStartOptions {
            name: &name,
            image: &image,
            environment: &environment,
            labels: &ownership_labels(
                &project.input().project_identity,
                &format!("service:{id}"),
                "container",
                &invocation_id,
            ),
            container_port,
        },
        remaining(deadline)?,
    ) {
        // A failed CLI call may have created the object before reporting an
        // error. Retain the lease so cleanup cannot silently lose the marker.
        lease.retain();
        return Err(anyhow::anyhow!(
            "start {executable} service {id:?}: {error:#}"
        ));
    }

    // Creation alone is not authority. Bind the lease only after a fresh
    // inspect proves that the runtime object carries every ownership label.
    let inspection = match runtime.inspect_container(project, &name, remaining(deadline)?) {
        Ok(inspection) => inspection,
        Err(error) => {
            // The object may exist, but ownership could not be proved. Retain
            // the lease for explicit operator investigation.
            lease.retain();
            return Err(error);
        }
    };
    if let Err(error) = lease.bind_runtime_identity(project, &inspection) {
        lease.retain();
        return Err(error);
    }

    let running = RunningService {
        lifecycle: None,
        runtime,
        inject_env,
        value: String::new(),
        container: Some(name),
        project: project.clone(),
        cleanup_errors,
        lease: Some(lease),
    };
    wait_ready(
        running,
        id,
        startup_timeout_secs,
        container_port,
        &healthcheck,
        &connection,
        deadline,
    )
}

fn wait_ready(
    mut running: RunningService,
    id: &str,
    startup_timeout_secs: u64,
    container_port: u16,
    healthcheck: &[String],
    connection: &str,
    deadline: Instant,
) -> Result<RunningService> {
    let runtime = running.runtime;
    while Instant::now() < deadline {
        if let Some(lease) = &running.lease {
            lease.renew()?;
        }
        if crate::process::cancelled() {
            bail!("verification cancelled while waiting for service {id:?}");
        }
        if let Some(container) = running.container.as_deref() {
            if let Some(port) = runtime.mapped_port(
                &running.project,
                container,
                container_port,
                remaining(deadline)?,
            )? {
                let ready = runtime.run_healthcheck(
                    &running.project,
                    container,
                    healthcheck,
                    remaining(deadline)?,
                )?;
                if ready {
                    running.value = connection.replace("{host_port}", &port);
                    return Ok(running);
                }
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    bail!("service {id:?} did not become ready within {startup_timeout_secs} seconds")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::service::inspection::RuntimeInspection;
    use crate::test_support::TestWorkspace;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn readiness_timeout_and_cancellation_preserve_failed_cleanup() {
        if std::env::var_os("HG_WAIT_CHILD").is_none() {
            let directory = tempfile::tempdir().unwrap();
            let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
                .map(|path| path.join("git"))
                .find(|path| path.is_file())
                .unwrap();
            std::os::unix::fs::symlink(git, directory.path().join("git")).unwrap();
            let script = directory.path().join("podman");
            std::fs::write(
                &script,
                r#"#!/bin/sh
printf '%s\n' "$1" >> "$HG_WAIT_LOG"
case "$1" in
 container) printf '%s' "$HG_WAIT_INSPECTION";;
 port) if [ "$HG_WAIT_CHILD" = health ]; then printf '127.0.0.1:12345\n'; fi;;
 exec|rm) exit 1;;
 *) exit 99;;
esac
"#,
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            for mode in ["timeout", "health", "cancel"] {
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "service::docker::tests::readiness_timeout_and_cancellation_preserve_failed_cleanup", "--nocapture"])
                    .env("HG_WAIT_CHILD", mode)
                    .env("HG_WAIT_LOG", directory.path().join("calls"))
                    .env("PATH", directory.path()).output().unwrap();
                assert!(
                    output.status.success(),
                    "{mode}: {}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
        let workspace = TestWorkspace::new("readiness-boundary");
        crate::preset::init(&workspace.root, "generic", false).unwrap();
        workspace.init_git();
        let project = Project::discover(Some(workspace.root.clone()), None).unwrap();
        let mode = std::env::var("HG_WAIT_CHILD").unwrap();
        let name = "owned-readiness";
        let object_id = "a".repeat(64);
        let labels = ownership_labels(
            &project.input().project_identity,
            "service:wait",
            "container",
            "wait-invocation",
        );
        let lease = ResourceLease::acquire(
            &project,
            "service:wait",
            "container",
            "wait-invocation",
            Some(name.into()),
            Some(ContainerRuntimeKind::Podman),
        )
        .unwrap();
        lease
            .bind_runtime_identity(
                &project,
                &RuntimeInspection {
                    object_id: object_id.clone(),
                    name: name.into(),
                    labels: labels.clone(),
                },
            )
            .unwrap();
        std::env::set_var(
            "HG_WAIT_INSPECTION",
            serde_json::json!({"Id":object_id,"Name":name,"Config":{"Labels":labels}}).to_string(),
        );
        let log = std::env::var_os("HG_WAIT_LOG").unwrap();
        std::fs::write(&log, "").unwrap();
        let errors = Arc::new(Mutex::new(vec![]));
        let running = RunningService {
            lifecycle: None,
            runtime: ContainerRuntimeKind::Podman,
            inject_env: "WAIT".into(),
            value: String::new(),
            container: Some(name.into()),
            project: project.clone(),
            cleanup_errors: Arc::clone(&errors),
            lease: Some(lease),
        };
        if mode == "cancel" {
            crate::process::install_signal_handlers();
            // SAFETY: the installed handler only sets the process cancellation flag;
            // this case runs in its own child process.
            assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
        }
        let error = wait_ready(
            running,
            "wait",
            1,
            1234,
            &["check".into()],
            "{host_port}",
            Instant::now() + Duration::from_millis(500),
        )
        .err()
        .unwrap();
        assert!(
            error.to_string().contains(if mode == "cancel" {
                "verification cancelled"
            } else {
                "did not become ready"
            }),
            "{error:#}"
        );
        assert_eq!(
            std::fs::read_dir(&project.resource_leases).unwrap().count(),
            1
        );
        assert!(
            !errors.lock().unwrap().is_empty(),
            "failed cleanup must be observable"
        );
        let calls = std::fs::read_to_string(log).unwrap();
        if mode == "cancel" {
            assert!(!calls.contains("port"));
        } else {
            assert!(calls.contains("port\n"));
            assert!(calls.contains("rm\n"));
        }
        if mode == "health" {
            assert!(calls.contains("exec\n"));
        }
    }
}
