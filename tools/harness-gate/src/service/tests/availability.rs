//! Availability uses installed hosts or checked external values only.
use super::project;
use crate::config::{ContainerRuntimeKind, ServiceConfig};
use crate::service::check_available;
use std::time::Duration;

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
        let result = check_available(&project, id, Duration::from_secs(1));
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
    let result = check_available(&project, "fixture", Duration::from_secs(1));
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
    let result = check_available(&project, "fixture", Duration::from_secs(1));
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
    assert!(check_available(&project, "fixture", Duration::from_secs(1)).is_ok());
    std::env::remove_var(name);
    if let ServiceConfig::Docker { runtime, .. } = &mut config {
        *runtime = ContainerRuntimeKind::Podman;
    }
    project.config.services.insert("fixture".into(), config);
    let e = check_available(&project, "fixture", Duration::from_secs(1)).unwrap_err();
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
        let error = check_available(&project, "fixture", Duration::from_secs(1)).unwrap_err();
        let expected = if external.is_some() {
            "cannot be combined"
        } else {
            "explicit readiness_host"
        };
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}
