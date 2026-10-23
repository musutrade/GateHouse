//! In-process engine contract: none of these backend operations execute a CLI.
use super::project;
use crate::config::{ContainerRuntimeKind, ServiceConfig};
use crate::project::Project;
use crate::service::check_available_with_runtime;
use crate::service::inspection::RuntimeInspection;
use crate::service::runtime::{ContainerRuntime, ContainerStartOptions};
use anyhow::{bail, Result};
use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};

#[derive(Clone, Copy)]
enum Behavior {
    Ready,
    InfoFailure,
    ImageMissing,
    ImageFailure,
    ExpireAfterInfo,
}
struct Fake {
    behavior: Behavior,
    calls: Rc<RefCell<Vec<(&'static str, Duration)>>>,
}
impl ContainerRuntime for Fake {
    fn executable(&self) -> &'static str {
        "in-process-fixture"
    }
    fn check_available(&self, _: &Project, timeout: Duration) -> Result<()> {
        self.calls.borrow_mut().push(("info", timeout));
        match self.behavior {
            Behavior::InfoFailure => bail!("fixture info unavailable"),
            Behavior::ExpireAfterInfo => std::thread::sleep(timeout + Duration::from_millis(5)),
            _ => {}
        }
        Ok(())
    }
    fn image_available(&self, _: &Project, image: &str, timeout: Duration) -> Result<bool> {
        assert_eq!(image, "fixture-image");
        self.calls.borrow_mut().push(("image", timeout));
        match self.behavior {
            Behavior::ImageFailure => bail!("fixture image transport failed"),
            Behavior::ImageMissing => Ok(false),
            _ => Ok(true),
        }
    }
    fn start_container(
        &self,
        _: &Project,
        _: ContainerStartOptions<'_>,
        _: Duration,
    ) -> Result<()> {
        panic!("read-only availability must not start containers")
    }
    fn mapped_port(&self, _: &Project, _: &str, _: u16, _: Duration) -> Result<Option<String>> {
        panic!("read-only availability must not query ports")
    }
    fn run_healthcheck(&self, _: &Project, _: &str, _: &[String], _: Duration) -> Result<bool> {
        panic!("read-only availability must not exec container commands")
    }
    fn stop_container(&self, _: &Path, _: &str, _: Duration) -> Result<()> {
        panic!("read-only availability must not remove containers")
    }
    fn inspect_container(&self, _: &Project, _: &str, _: Duration) -> Result<RuntimeInspection> {
        panic!("read-only availability must not inspect owned containers")
    }
}
fn service(runtime: ContainerRuntimeKind, external_env: Option<String>) -> ServiceConfig {
    ServiceConfig::Docker {
        readiness_host: None,
        lifecycle_host: None,
        runtime,
        image: "fixture-image".into(),
        image_env: None,
        external_env,
        inject_env: "FIXTURE_URL".into(),
        startup_timeout_secs: 1,
        timeout_env: None,
        container_port: 5432,
        environment: Default::default(),
        healthcheck: vec![],
        command: None,
        connection: "fixture".into(),
    }
}
#[test]
fn injected_backend_preserves_kind_order_image_and_remaining_budget() {
    for kind in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman] {
        let (_workspace, mut project) = project();
        project
            .config
            .services
            .insert("fixture".into(), service(kind, None));
        let calls = Rc::new(RefCell::new(vec![]));
        let result =
            check_available_with_runtime(&project, "fixture", Duration::from_secs(1), |selected| {
                assert_eq!(selected, kind);
                Fake {
                    behavior: Behavior::Ready,
                    calls: calls.clone(),
                }
            })
            .unwrap();
        assert_eq!(result, "in-process-fixture and fixture-image ready");
        let calls = calls.borrow();
        assert_eq!(
            calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            ["info", "image"]
        );
        assert!(calls[0].1 <= Duration::from_secs(1));
        assert!(calls[1].1 <= calls[0].1 && !calls[1].1.is_zero());
    }
}
#[test]
fn injected_backend_failures_short_circuit_without_mutation() {
    for (behavior, expected, count) in [
        (Behavior::InfoFailure, "fixture info unavailable", 1),
        (
            Behavior::ImageMissing,
            "image fixture-image is not available",
            2,
        ),
        (Behavior::ImageFailure, "fixture image transport failed", 2),
        (Behavior::ExpireAfterInfo, "service operation timed out", 1),
    ] {
        let (_workspace, mut project) = project();
        project.config.services.insert(
            "fixture".into(),
            service(ContainerRuntimeKind::Docker, None),
        );
        let calls = Rc::new(RefCell::new(vec![]));
        let error =
            check_available_with_runtime(&project, "fixture", Duration::from_millis(20), |_| {
                Fake {
                    behavior,
                    calls: calls.clone(),
                }
            })
            .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        assert_eq!(calls.borrow().len(), count);
    }
}
#[test]
fn unknown_and_external_services_do_not_resolve_an_engine() {
    let (_workspace, mut project) = project();
    let name = "HG_IN_PROCESS_AVAILABILITY_EXTERNAL_68";
    std::env::set_var(name, "fixture://external");
    project.config.services.insert(
        "fixture".into(),
        service(ContainerRuntimeKind::Docker, Some(name.into())),
    );
    project.config.services.insert(
        "environment".into(),
        ServiceConfig::Environment {
            source_env: name.into(),
            inject_env: "FIXTURE".into(),
        },
    );
    for id in ["fixture", "environment", "unknown"] {
        let result =
            check_available_with_runtime::<Fake>(&project, id, Duration::from_secs(1), |_| {
                panic!("must not resolve backend")
            });
        assert_eq!(result.is_ok(), id != "unknown");
    }
    std::env::remove_var(name);
}
#[cfg(unix)]
#[test]
fn recipe_rejection_precedes_engine_resolution() {
    let (_workspace, mut project) = project();
    project.config.services.insert(
        "fixture".into(),
        service(ContainerRuntimeKind::Docker, None),
    );
    crate::recipes::tests::install_fixture(&mut project, "fixture", false);
    // The no-external-value route requests a managed recipe. Keep the fixture
    // digest bound to the rewritten bytes so this tests an actual rejection,
    // rather than a mode or executable-integrity failure.
    use sha2::{Digest, Sha256};
    let check = &mut project.config.service_checks.get_mut("fixture").unwrap()[0];
    let source = std::fs::read_to_string(&check.executable).unwrap();
    let managed = source.replace("\"mode\":\"external\"", "\"mode\":\"managed\"");
    assert_ne!(managed, source);
    std::fs::write(&check.executable, &managed).unwrap();
    check.sha256 = format!("{:x}", Sha256::digest(managed.as_bytes()));
    let result =
        check_available_with_runtime::<Fake>(&project, "fixture", Duration::from_secs(1), |_| {
            panic!("recipe failure must precede backend selection")
        });
    assert!(format!("{:#}", result.unwrap_err()).contains("reported failure"));
}

#[test]
fn installed_readiness_failure_never_resolves_legacy_backend() {
    let (_workspace, mut project) = project();
    let mut config = service(ContainerRuntimeKind::Docker, None);
    if let ServiceConfig::Docker { readiness_host, .. } = &mut config {
        *readiness_host = Some(
            project
                .root
                .join("missing-protected-host.json")
                .to_string_lossy()
                .into_owned(),
        );
    }
    project.config.services.insert("fixture".into(), config);
    let result =
        check_available_with_runtime::<Fake>(&project, "fixture", Duration::from_secs(1), |_| {
            panic!("installed readiness must not fall back to CLI")
        });
    assert!(result.is_err());
}
#[test]
fn external_value_bypasses_installed_host_but_podman_managed_is_refused() {
    let (_workspace, mut project) = project();
    let name = "HG_V3_EXTERNAL_FIXTURE_69";
    std::env::set_var(name, "fixture://external");
    let mut config = service(ContainerRuntimeKind::Docker, Some(name.into()));
    if let ServiceConfig::Docker { readiness_host, .. } = &mut config {
        *readiness_host = Some("/missing-host".into());
    }
    project
        .config
        .services
        .insert("fixture".into(), config.clone());
    assert!(check_available_with_runtime::<Fake>(
        &project,
        "fixture",
        Duration::from_secs(1),
        |_| panic!("external must bypass backend")
    )
    .is_ok());
    std::env::remove_var(name);
    if let ServiceConfig::Docker { runtime, .. } = &mut config {
        *runtime = ContainerRuntimeKind::Podman;
    }
    project.config.services.insert("fixture".into(), config);
    let e =
        check_available_with_runtime::<Fake>(&project, "fixture", Duration::from_secs(1), |_| {
            panic!("Podman must not resolve backend")
        })
        .unwrap_err();
    assert!(e.to_string().contains("requires Docker"));
}

#[test]
fn lifecycle_authority_is_neither_readiness_nor_external_override() {
    let (_workspace, mut project) = project();
    for external in [None, Some("HG_LIFECYCLE_EXTERNAL_81".into())] {
        let mut config = service(ContainerRuntimeKind::Docker, external.clone());
        if let ServiceConfig::Docker { lifecycle_host, .. } = &mut config {
            *lifecycle_host = Some("/missing-lifecycle-host.json".into());
        }
        project.config.services.insert("fixture".into(), config);
        let error = check_available_with_runtime::<Fake>(
            &project,
            "fixture",
            Duration::from_secs(1),
            |_| panic!("lifecycle authority must never select a CLI backend"),
        )
        .unwrap_err();
        let expected = if external.is_some() {
            "cannot be combined"
        } else {
            "explicit readiness_host"
        };
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}
