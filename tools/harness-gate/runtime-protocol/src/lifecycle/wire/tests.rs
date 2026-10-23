use super::*;
use serde_json::{json, Value};
fn labels() -> Vec<Entry> {
    [
        (
            "gatehouse.lifecycle.authority",
            "container-lifecycle/mutate",
        ),
        (
            "gatehouse.lifecycle.invocation",
            "11111111111111111111111111111111",
        ),
        ("gatehouse.lifecycle.project", "project"),
        ("gatehouse.lifecycle.resource", "resource"),
    ]
    .into_iter()
    .map(|(k, v)| Entry {
        key: k.into(),
        value: v.into(),
    })
    .collect()
}
fn target() -> Target {
    Target {
        container_port: 1234,
        object_id: "a".repeat(64),
        name: "test-container".into(),
        labels: labels(),
    }
}
fn request(operation: Operation) -> Request {
    let mut r = Request {
        protocol: PROTOCOL.into(),
        binding: Binding {
            installation_sha256: "b".repeat(64),
            executable_sha256: "c".repeat(64),
            config_sha256: "d".repeat(64),
            engine_binding: EngineBinding {
                engine: "docker".into(),
                transport: "unix-http".into(),
                socket_path: "/run/test.sock".into(),
                socket_uid: 1000,
                principal_uid: 1000,
                rootless: true,
                api_version: "1.47".into(),
                server_version: "27".into(),
                daemon_id: "test-daemon".into(),
                scope: "test".into(),
            },
            project_id: "project".into(),
            resource_id: "resource".into(),
            lease_invocation: "1".repeat(32),
            invocation: "2".repeat(32),
            generation: 1,
            operation_key: "0".repeat(64),
        },
        operation,
    };
    r.binding.operation_key = r.operation_key().unwrap();
    r
}
fn object(state: ContainerState) -> Object {
    let t = target();
    Object {
        object_id: t.object_id,
        name: t.name,
        labels: t.labels,
        state,
        ports: if state == ContainerState::Running {
            vec![Port {
                container_port: 1234,
                protocol: "tcp".into(),
                host_ip: "127.0.0.1".into(),
                host_port: 49152,
            }]
        } else {
            vec![]
        },
        image_id: format!("sha256:{}", "e".repeat(64)),
    }
}
fn observations(r: &Request) -> Observations {
    let e = &r.binding.engine_binding;
    let o = Observation {
        api_version: e.api_version.clone(),
        server_version: e.server_version.clone(),
        daemon_id: e.daemon_id.clone(),
    };
    Observations {
        observed_before: o.clone(),
        observed_after: o,
    }
}
fn response(r: &Request, outcome: Outcome) -> Response {
    Response {
        protocol: r.protocol.clone(),
        binding: r.binding.clone(),
        operation: r.operation.clone(),
        outcome,
        observations: Some(observations(r)),
    }
}
fn podman_request(operation: Operation) -> Request {
    let mut r = request(operation);
    r.binding.engine_binding = EngineBinding {
        engine: "podman".into(),
        transport: "unix-http".into(),
        socket_path: "/run/user/1000/podman/podman.sock".into(),
        socket_uid: 1000,
        principal_uid: 1000,
        rootless: true,
        api_version: "5.7.0".into(),
        server_version: "5.7.0".into(),
        daemon_id: "podman-host".into(),
        scope: "/home/test/.local/share/containers/storage".into(),
    };
    r.binding.operation_key = r.operation_key().unwrap();
    r
}
fn bytes<T: Serialize>(t: &T) -> Vec<u8> {
    canonical::encode(&serde_json::to_value(t).unwrap()).unwrap()
}
fn absent() -> Outcome {
    let t = target();
    Outcome::Absent {
        evidence: Absence {
            object_id: t.object_id.clone(),
            route: format!("/v1.47/containers/{}/json", t.object_id),
            http_status: 404,
            error_message_sha256: format!(
                "{:x}",
                Sha256::digest(format!("No such container: {}", t.object_id).as_bytes())
            ),
        },
    }
}

#[test]
fn all_operations_have_schema_valid_roundtrip() {
    for op in [
        Operation::ResolveImage {
            image: "repo:tag".into(),
        },
        Operation::Create {
            name: target().name,
            image_id: object(ContainerState::Created).image_id,
            labels: labels(),
            environment: vec![],
            container_port: 1234,
            command: None,
        },
        Operation::Status { target: target() },
        Operation::Start { target: target() },
        Operation::Stop {
            target: target(),
            grace_seconds: 10,
        },
        Operation::Remove { target: target() },
        Operation::HealthExec {
            target: target(),
            argv: vec!["check".into()],
            timeout_ms: 500,
        },
    ] {
        let r = request(op);
        let b = r.canonical_bytes().unwrap();
        assert_eq!(parse_protected_request(&b, &b).unwrap(), r);
    }
}
#[test]
fn full_request_vector_is_stable() {
    let r = request(Operation::Status { target: target() });
    let expected = include_bytes!("status-request.json");
    assert_eq!(r.canonical_bytes().unwrap(), expected);
    assert_eq!(parse_protected_request(expected, expected).unwrap(), r);
}
#[test]
fn protected_request_rejects_digest_echo_stale_generation_and_different_target() {
    let r = request(Operation::Status { target: target() });
    let trusted = r.canonical_bytes().unwrap();
    let mut stale = r.clone();
    stale.binding.generation += 1;
    assert!(parse_request(&bytes(&stale)).is_err());
    stale.binding.operation_key = stale.operation_key().unwrap();
    assert!(parse_protected_request(&bytes(&stale), &trusted).is_err());
    let mut modified = r.clone();
    modified.binding.invocation = "3".repeat(32);
    modified.binding.operation_key = modified.operation_key().unwrap();
    assert!(parse_protected_request(&bytes(&modified), &trusted).is_err());
    let mut with_newline = trusted.clone();
    with_newline.push(b'\n');
    assert!(parse_protected_request(&with_newline, &trusted).is_err());
}
#[test]
fn nested_duplicate_unknown_fields_and_missing_keys_rejected() {
    let r = request(Operation::Status { target: target() });
    let b = r.canonical_bytes().unwrap();
    let s = String::from_utf8(b).unwrap();
    assert!(parse_request(
        s.replace("\"generation\":1", "\"generation\":1,\"generation\":1")
            .as_bytes()
    )
    .is_err());
    let v = serde_json::to_value(r).unwrap();
    for path in ["binding", "operation"] {
        let mut bad = v.clone();
        bad[path]["extra"] = json!(true);
        assert!(parse_request(&bytes(&bad)).is_err());
    }
    let mut bad = v;
    bad["binding"]
        .as_object_mut()
        .unwrap()
        .remove("operation_key");
    assert!(parse_request(&bytes(&bad)).is_err());
}
#[test]
fn valid_outcomes_and_only_exact_operation_pairs() {
    let pairs = [
        (
            Operation::ResolveImage {
                image: "repo:tag".into(),
            },
            Outcome::Image {
                image_id: object(ContainerState::Created).image_id,
                repo_digests: vec![],
            },
        ),
        (
            Operation::Create {
                name: target().name,
                image_id: object(ContainerState::Created).image_id,
                labels: labels(),
                environment: vec![],
                container_port: 1234,
                command: None,
            },
            Outcome::Created {
                object: object(ContainerState::Created),
            },
        ),
        (
            Operation::Status { target: target() },
            Outcome::Status {
                object: object(ContainerState::Running),
            },
        ),
        (
            Operation::Start { target: target() },
            Outcome::Started {
                object: object(ContainerState::Running),
            },
        ),
        (
            Operation::Stop {
                target: target(),
                grace_seconds: 10,
            },
            Outcome::Stopped {
                object: object(ContainerState::Exited),
            },
        ),
        (Operation::Remove { target: target() }, Outcome::Removed),
        (
            Operation::HealthExec {
                target: target(),
                argv: vec!["check".into()],
                timeout_ms: 100,
            },
            Outcome::Health {
                exec_id: "f".repeat(64),
                exit_code: 7,
                object: object(ContainerState::Running),
            },
        ),
    ];
    for (i, (op, outcome)) in pairs.iter().enumerate() {
        let r = request(op.clone());
        assert!(parse_response(&bytes(&response(&r, outcome.clone())), &r).is_ok());
        for (j, (_, other)) in pairs.iter().enumerate() {
            if i != j {
                assert!(
                    response(&r, other.clone()).validate(&r).is_err(),
                    "pair {i}/{j}"
                );
            }
        }
    }
}
#[test]
fn absence_requires_exact_route_message_identity_observations_and_status_operation() {
    let r = request(Operation::Status { target: target() });
    let ok = response(&r, absent());
    assert!(parse_response(&bytes(&ok), &r).is_ok());
    let value = serde_json::to_value(&ok).unwrap();
    for (field, replacement) in [
        ("route", json!("/v1.47/info")),
        ("object_id", json!("b".repeat(64))),
        ("http_status", json!(500)),
        ("error_message_sha256", json!("a".repeat(64))),
    ] {
        let mut bad = value.clone();
        bad["outcome"]["evidence"][field] = replacement;
        assert!(parse_response(&bytes(&bad), &r).is_err());
    }
    let remove = request(Operation::Remove { target: target() });
    assert!(response(&remove, absent()).validate(&remove).is_err());
    let mut wrong = ok.clone();
    wrong
        .observations
        .as_mut()
        .unwrap()
        .observed_after
        .daemon_id = "other".into();
    assert!(wrong.validate(&r).is_err());
    wrong = ok;
    wrong.observations = None;
    assert!(wrong.validate(&r).is_err());
}
#[test]
fn failures_require_explicit_null_observations_and_do_not_claim_success() {
    let r = request(Operation::Remove { target: target() });
    for outcome in [
        Outcome::Unknown {
            reason: Reason::Transport,
        },
        Outcome::RejectedBeforeEffect {
            reason: Reason::InvalidInput,
        },
    ] {
        let mut out = response(&r, outcome);
        assert!(out.validate(&r).is_err());
        out.observations = None;
        assert!(parse_response(&bytes(&out), &r).is_ok());
        let mut v = serde_json::to_value(out).unwrap();
        v.as_object_mut().unwrap().remove("observations");
        assert!(parse_response(&bytes(&v), &r).is_err());
    }
}
#[test]
fn remove_claim_rejects_extra_fields() {
    let r = request(Operation::Remove { target: target() });
    let mut v = serde_json::to_value(response(&r, Outcome::Removed)).unwrap();
    v["outcome"]["absence"] = json!(true);
    assert!(parse_response(&bytes(&v), &r).is_err());
}
#[test]
fn target_labels_image_pin_and_state_mismatches_rejected() {
    let r = request(Operation::Start { target: target() });
    for obj in [
        object(ContainerState::Exited),
        Object {
            object_id: "b".repeat(64),
            ..object(ContainerState::Running)
        },
        Object {
            labels: vec![],
            ..object(ContainerState::Running)
        },
    ] {
        assert!(response(&r, Outcome::Started { object: obj })
            .validate(&r)
            .is_err());
    }
    let pin = format!("repo@sha256:{}", "f".repeat(64));
    let r = request(Operation::ResolveImage { image: pin.clone() });
    assert!(response(
        &r,
        Outcome::Image {
            image_id: object(ContainerState::Created).image_id.clone(),
            repo_digests: vec![]
        }
    )
    .validate(&r)
    .is_err());
    assert!(response(
        &r,
        Outcome::Image {
            image_id: object(ContainerState::Created).image_id,
            repo_digests: vec![pin]
        }
    )
    .validate(&r)
    .is_ok());
}
#[test]
fn health_invalid_bounds_and_noninteger_exit_rejected() {
    for op in [
        Operation::HealthExec {
            target: target(),
            argv: vec![],
            timeout_ms: 100,
        },
        Operation::HealthExec {
            target: target(),
            argv: vec!["check".into()],
            timeout_ms: 30001,
        },
        Operation::Stop {
            target: target(),
            grace_seconds: 0,
        },
    ] {
        assert!(request(op).validate().is_err());
    }
    let r = request(Operation::HealthExec {
        target: target(),
        argv: vec!["check".into()],
        timeout_ms: 100,
    });
    let mut v = serde_json::to_value(response(
        &r,
        Outcome::Health {
            exec_id: "f".repeat(64),
            exit_code: 0,
            object: object(ContainerState::Running),
        },
    ))
    .unwrap();
    v["outcome"]["exit_code"] = Value::Null;
    assert!(parse_response(&bytes(&v), &r).is_err());
}

#[test]
fn ports_are_bound_to_requested_service_not_merely_loopback() {
    let r = request(Operation::Status { target: target() });
    let good = object(ContainerState::Running);
    assert!(response(
        &r,
        Outcome::Status {
            object: good.clone()
        }
    )
    .validate(&r)
    .is_ok());
    for port in [
        Port {
            container_port: 9999,
            ..good.ports[0].clone()
        },
        Port {
            host_ip: "0.0.0.0".into(),
            ..good.ports[0].clone()
        },
        Port {
            host_port: 0,
            ..good.ports[0].clone()
        },
        Port {
            protocol: "udp".into(),
            ..good.ports[0].clone()
        },
    ] {
        let bad = Object {
            ports: vec![port],
            ..good.clone()
        };
        assert!(response(&r, Outcome::Status { object: bad })
            .validate(&r)
            .is_err());
    }
    let duplicate = Object {
        ports: vec![good.ports[0].clone(); 2],
        ..good.clone()
    };
    assert!(response(&r, Outcome::Status { object: duplicate })
        .validate(&r)
        .is_err());
    // Empty bindings are a valid observation for cleanup, never readiness.
    let empty = Object {
        ports: vec![],
        ..good
    };
    assert!(response(
        &r,
        Outcome::Status {
            object: empty.clone()
        }
    )
    .validate(&r)
    .is_ok());
    let start = request(Operation::Start { target: target() });
    assert!(response(
        &start,
        Outcome::Started {
            object: empty.clone()
        }
    )
    .validate(&start)
    .is_err());
    let health = request(Operation::HealthExec {
        target: target(),
        argv: vec!["check".into()],
        timeout_ms: 100,
    });
    assert!(response(
        &health,
        Outcome::Health {
            exec_id: "f".repeat(64),
            exit_code: 0,
            object: empty
        }
    )
    .validate(&health)
    .is_err());
}

#[test]
fn zero_target_port_is_not_authority_and_port_changes_change_digest() {
    let original = request(Operation::Status { target: target() });
    let mut changed = target();
    changed.container_port = 9999;
    let changed = request(Operation::Status { target: changed });
    assert_ne!(
        original.binding.operation_key,
        changed.binding.operation_key
    );
    assert!(parse_protected_request(
        &changed.canonical_bytes().unwrap(),
        &original.canonical_bytes().unwrap()
    )
    .is_err());
    let mut zero = target();
    zero.container_port = 0;
    assert!(request(Operation::Status { target: zero })
        .validate()
        .is_err());
}

#[test]
fn podman_5_7_is_the_only_native_mutation_profile_and_keeps_docker_unchanged() {
    for (engine, api, server, rootless, principal) in [
        ("podman", "5.7.0", "5.7.0", true, 1000),
        ("podman", "5.7.0", "5.7.0", false, 1000),
        ("podman", "5.7.0", "5.7.0", true, 0),
        ("podman", "6.1.4", "6.1.4", true, 1000),
        ("podman", "5.7.1", "5.7.1", true, 1000),
        ("docker", "1.47", "27", true, 1000),
        ("unknown", "5.7.0", "5.7.0", true, 1000),
    ] {
        let mut r = podman_request(Operation::Status { target: target() });
        r.binding.engine_binding.engine = engine.into();
        r.binding.engine_binding.api_version = api.into();
        r.binding.engine_binding.server_version = server.into();
        r.binding.engine_binding.rootless = rootless;
        r.binding.engine_binding.principal_uid = principal;
        r.binding.operation_key = r.operation_key().unwrap();
        let expected = if engine == "podman" {
            api == "5.7.0" && server == "5.7.0" && rootless && principal != 0
        } else {
            engine == "docker"
        };
        assert_eq!(
            r.validate().is_ok(),
            expected,
            "{engine}/{api}/{server}/{rootless}/{principal}"
        );
    }
}

#[test]
fn podman_absence_uses_exact_libpod_route_and_message_while_docker_does_not() {
    let r = podman_request(Operation::Status { target: target() });
    let evidence = exact_absence(&r.binding.engine_binding, &target()).unwrap();
    assert_eq!(
        evidence.route,
        format!("/v5.7.0/libpod/containers/{}/json", target().object_id)
    );
    assert_eq!(
        evidence.error_message_sha256,
        format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "no container with name or ID \"{}\" found: no such container",
                    target().object_id
                )
                .as_bytes()
            )
        )
    );
    let mut response = response(
        &r,
        Outcome::Absent {
            evidence: evidence.clone(),
        },
    );
    assert!(parse_response(&bytes(&response), &r).is_ok());
    let mut wrong = evidence;
    wrong.route = format!("/v5.7.0/containers/{}/json", target().object_id);
    response.outcome = Outcome::Absent { evidence: wrong };
    assert!(response.validate(&r).is_err());
    let docker = request(Operation::Status { target: target() });
    assert!(response.clone().validate(&docker).is_err());
    let docker_evidence = exact_absence(&docker.binding.engine_binding, &target()).unwrap();
    assert_eq!(
        docker_evidence.route,
        format!("/v1.47/containers/{}/json", target().object_id)
    );
}

#[test]
fn podman_create_command_is_bounded_and_absent_from_old_docker_bytes() {
    let docker = request(Operation::Create {
        name: target().name,
        image_id: format!("sha256:{}", "e".repeat(64)),
        labels: labels(),
        environment: vec![],
        container_port: 1234,
        command: None,
    });
    let value = serde_json::to_value(&docker).unwrap();
    assert!(value["operation"].get("command").is_none());

    let good = vec!["/bin/sh".into(), "-c".into(), "sleep 30".into()];
    let podman = podman_request(Operation::Create {
        name: target().name,
        image_id: format!("sha256:{}", "e".repeat(64)),
        labels: labels(),
        environment: vec![],
        container_port: 1234,
        command: Some(good.clone()),
    });
    assert!(podman.validate().is_ok());
    for bad in [
        vec![],
        vec!["".into()],
        vec!["a\0b".into()],
        vec!["x".into(); 65],
    ] {
        let mut r = podman.clone();
        let Operation::Create { command, .. } = &mut r.operation else {
            panic!()
        };
        *command = Some(bad);
        r.binding.operation_key = r.operation_key().unwrap();
        assert!(r.validate().is_err());
    }
}
