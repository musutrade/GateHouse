//! Deterministic command construction, kept in the service-core boundary.
use std::collections::BTreeMap;

pub(super) fn start_container_args(
    name: &str,
    image: &str,
    environment: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
    container_port: u16,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".into(),
        "--detach".into(),
        "--pull=never".into(),
        "--name".into(),
        name.to_string(),
    ];
    for (key, value) in environment {
        args.extend(["--env".into(), format!("{key}={value}")]);
    }
    for (key, value) in labels {
        args.extend(["--label".into(), format!("{key}={value}")]);
    }
    args.extend([
        "--publish".into(),
        format!("127.0.0.1::{container_port}"),
        image.to_string(),
    ]);
    args
}

pub(super) fn mapped_port_args(name: &str, container_port: u16) -> Vec<String> {
    vec!["port".into(), name.into(), format!("{container_port}/tcp")]
}

pub(super) fn healthcheck_args(name: &str, command: &[String]) -> Vec<String> {
    let mut args = vec!["exec".into(), name.into()];
    args.extend(command.iter().cloned());
    args
}

pub(super) fn stop_container_args(name: &str) -> Vec<String> {
    vec!["rm".into(), "--force".into(), name.into()]
}

pub(super) fn inspect_container_args(name: &str) -> Vec<String> {
    vec![
        "container".into(),
        "inspect".into(),
        "--format".into(),
        "{{json .}}".into(),
        name.into(),
    ]
}

pub(super) fn runtime_info_args() -> Vec<String> {
    vec!["info".into()]
}

#[cfg(test)]
mod tests {
    use super::{
        healthcheck_args, inspect_container_args, mapped_port_args, runtime_info_args,
        start_container_args, stop_container_args,
    };
    use crate::config::ContainerRuntimeKind;
    use std::collections::BTreeMap;

    #[test]
    fn selects_the_expected_docker_compatible_executable() {
        assert_eq!(ContainerRuntimeKind::Docker.executable(), "docker");
        assert_eq!(ContainerRuntimeKind::Podman.executable(), "podman");
    }

    #[test]
    fn builds_deterministic_start_arguments() {
        let environment = BTreeMap::from([
            ("ZED".to_string(), "last".to_string()),
            ("ALPHA".to_string(), "first".to_string()),
        ]);
        assert_eq!(
            start_container_args(
                "fixture",
                "postgres:16",
                &environment,
                &BTreeMap::new(),
                5432
            ),
            vec![
                "run",
                "--rm",
                "--detach",
                "--pull=never",
                "--name",
                "fixture",
                "--env",
                "ALPHA=first",
                "--env",
                "ZED=last",
                "--publish",
                "127.0.0.1::5432",
                "postgres:16",
            ]
        );
    }

    #[test]
    fn builds_port_healthcheck_cleanup_and_info_arguments() {
        assert_eq!(
            mapped_port_args("fixture", 5432),
            vec!["port", "fixture", "5432/tcp"]
        );
        assert_eq!(
            healthcheck_args(
                "fixture",
                &["pg_isready".into(), "-U".into(), "test".into()]
            ),
            vec!["exec", "fixture", "pg_isready", "-U", "test"]
        );
        assert_eq!(
            stop_container_args("fixture"),
            vec!["rm", "--force", "fixture"]
        );
        assert_eq!(runtime_info_args(), vec!["info"]);
        assert_eq!(
            inspect_container_args("fixture"),
            vec!["container", "inspect", "--format", "{{json .}}", "fixture"]
        );
    }
}
