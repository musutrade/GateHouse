//! Opt-in exact-binary joint test; missing inputs fail, never silently skip.
use super::*;
use gatehouse_runtime_protocol::lifecycle::journal::{CreatePlan, Purpose, Reservation};
use std::{
    io::{Read, Write},
    os::unix::net::UnixListener,
    thread,
};
#[test]
fn pinned_native_adapter_resolution_creation_start_and_health_through_core_host() {
    joint(0);
}
#[test]
fn configured_image_service_requires_reservation_and_retains_failed_health() {
    joint(1);
}
#[test]
fn configured_image_service_normal_cleanup_requires_exact_absence() {
    joint(2);
}
#[test]
fn configured_image_service_retains_present_object_after_remove_claim() {
    joint(3);
}
#[test]
fn configured_image_service_rejects_nonexact_absence_after_remove() {
    joint(4);
}
#[test]
fn configured_image_service_requires_created_before_start() {
    joint(5);
}
#[test]
fn configured_image_service_requires_stopped_before_remove() {
    joint(6);
}
#[test]
fn configured_image_service_retains_exited_for_operator_recovery() {
    joint(7);
}
#[test]
fn configured_image_service_retains_failed_stop() {
    joint(8);
}
#[test]
fn configured_image_service_failed_health_requires_explicit_operator_cleanup() {
    joint(9);
}
#[test]
fn configured_image_service_initial_absence_requires_operator_recovery() {
    joint(11);
}
#[test]
fn configured_image_service_rejects_multiple_ports() {
    joint(13);
}
#[test]
fn configured_image_service_rejects_wrong_container_port() {
    joint(14);
}
#[test]
fn configured_image_service_retains_remove_failure() {
    joint(15);
}
#[test]
fn public_service_manager_uses_fixed_slot_and_native_lifecycle_cleanup() {
    joint(16);
}
#[test]
fn public_service_manager_failed_start_keeps_fixed_slot_across_config_changes() {
    joint(17);
}
#[test]
fn public_operator_cleanup_requires_exact_fence_and_releases_failed_start() {
    joint(18);
}
#[test]
fn public_operator_executable_releases_failed_service_from_saved_configuration() {
    joint(19);
}
#[test]
fn operator_recovery_initial_exact_absence_releases_without_mutation() {
    joint(20);
}
#[test]
fn operator_recovery_terminal_object_removes_without_stop() {
    joint(21);
}
#[test]
fn operator_recovery_unproved_stop_retains_slot() {
    joint(22);
}
#[test]
fn operator_recovery_unsupported_state_retains_slot() {
    joint(23);
}
#[test]
fn operator_recovery_present_after_remove_retains_slot() {
    joint(24);
}
#[test]
fn operator_recovery_cancellation_before_status_retains_slot() {
    joint(25);
}
#[test]
fn operator_recovery_failed_stop_retains_slot() {
    joint(26);
}
#[test]
fn public_operator_cleanup_dispatches_in_process_cli() {
    joint(27);
}
#[test]
fn public_service_drop_reports_failed_cleanup_and_retains_slot() {
    joint(28);
}
fn joint(service_path: u8) {
    let path = std::env::var_os("GATEHOUSE_LIFECYCLE_ADAPTER")
        .expect("explicit native adapter path required");
    let digest = std::env::var("GATEHOUSE_LIFECYCLE_ADAPTER_SHA256")
        .expect("explicit native adapter digest required");
    let binary = fs::read(&path).unwrap();
    assert_eq!(hash(&binary), digest, "wrong native adapter artifact");
    let root = std::env::var_os("GATEHOUSE_JOURNAL_TEST_ROOT")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap();
    let socket_dir = tempfile::Builder::new()
        .prefix("lifecycle-joint-")
        .tempdir_in(root)
        .unwrap();
    fs::set_permissions(socket_dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let socket = socket_dir.path().join("engine.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let (directory, installed, mut request, host_path) =
        fixture_inputs("created", &binary, Some(&socket));
    let Operation::Create {
        name,
        labels,
        environment,
        container_port,
        ..
    } = request.operation.clone()
    else {
        panic!()
    };
    let plan = CreatePlan {
        name: name.clone(),
        labels: labels.clone(),
        environment,
        container_port,
    };
    request.operation = Operation::ResolveImage {
        image: "example:latest".into(),
    };
    request.binding.operation_key = request.operation_key().unwrap();
    let labels: serde_json::Map<String, Value> = labels
        .iter()
        .map(|e| (e.key.clone(), e.value.clone().into()))
        .collect();
    let object = json!({"Id":"a".repeat(64),"Name":format!("/{name}"),"Image":format!("sha256:{}","e".repeat(64)),"Config":{"Labels":labels},"State":{"Status":"created"},"NetworkSettings":{"Ports":{"1234/tcp":null}}});
    let mut running = object.clone();
    running["State"]["Status"] = "running".into();
    running["NetworkSettings"]["Ports"] =
        json!({"1234/tcp":[{"HostIp":"127.0.0.1","HostPort":"49152"}]});
    // The executable case deliberately runs refusal-only CLI calls between
    // startup and recovery. They must not consume the fake engine's request
    // deadline: arm recovery only when the valid command is about to launch.
    let (operator_ready, wait_operator) = std::sync::mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        type Step = (&'static str, String, u16, Value);
        let version = json!({"Version":"27","MinAPIVersion":"1.40","ApiVersion":"1.47"});
        let info = json!({"ID":"test-daemon","SecurityOptions":["name=rootless"]});
        let identity = || {
            vec![
                ("GET", "/version".into(), 200, version.clone()),
                ("GET", "/v1.47/info".into(), 200, info.clone()),
            ]
        };
        let inspect = format!("/v1.47/containers/{}/json", "a".repeat(64));
        let exec = format!("/v1.47/exec/{}", "b".repeat(64));
        let mut operations: Vec<Vec<Step>> = vec![
            vec![(
                "GET",
                "/v1.47/images/example%3Alatest/json".into(),
                200,
                json!({"Id":format!("sha256:{}","e".repeat(64)),"RepoDigests":[]}),
            )],
            vec![
                (
                    "POST",
                    "/v1.47/containers/create?name=test-container".into(),
                    201,
                    json!({"Id":"a".repeat(64),"Warnings":[]}),
                ),
                ("GET", inspect.clone(), 200, object.clone()),
            ],
            vec![("GET", inspect.clone(), 200, object.clone())],
            vec![
                ("GET", inspect.clone(), 200, object),
                (
                    "POST",
                    format!("/v1.47/containers/{}/start", "a".repeat(64)),
                    204,
                    Value::Null,
                ),
                ("GET", inspect.clone(), 200, running.clone()),
            ],
            vec![("GET", inspect.clone(), 200, running.clone())],
            vec![
                ("GET", inspect.clone(), 200, running.clone()),
                (
                    "POST",
                    format!("/v1.47/containers/{}/exec", "a".repeat(64)),
                    201,
                    json!({"Id":"b".repeat(64)}),
                ),
                ("POST", format!("{exec}/start"), 200, Value::Null),
                (
                    "GET",
                    format!("{exec}/json"),
                    200,
                    json!({"ID":"b".repeat(64),"ContainerID":"a".repeat(64),"Running":false,"ExitCode":if service_path >= 2 && service_path != 9 && service_path != 17 && service_path != 18 && service_path != 19 && service_path != 27 && !(20..=26).contains(&service_path) {0} else {7}}),
                ),
                ("GET", inspect.clone(), 200, running.clone()),
            ],
        ];
        if service_path >= 2 && service_path != 5 {
            let final_status = if matches!(service_path, 3 | 28) {
                (200, running.clone())
            } else if service_path == 4 {
                (404, json!({"message":"wrong container"}))
            } else {
                (
                    404,
                    json!({"message":format!("No such container: {}", "a".repeat(64))}),
                )
            };
            let mut stopped = running.clone();
            stopped["State"]["Status"] = "exited".into();
            operations.extend([
                vec![("GET", inspect.clone(), 200, running.clone())],
                vec![
                    ("GET", inspect.clone(), 200, running.clone()),
                    (
                        "POST",
                        format!("/v1.47/containers/{}/stop?t=5", "a".repeat(64)),
                        204,
                        Value::Null,
                    ),
                    ("GET", inspect.clone(), 200, stopped.clone()),
                ],
                vec![("GET", inspect.clone(), 200, stopped.clone())],
                vec![
                    ("GET", inspect.clone(), 200, stopped.clone()),
                    (
                        "DELETE",
                        format!("/v1.47/containers/{}?force=false&v=false", "a".repeat(64)),
                        204,
                        Value::Null,
                    ),
                ],
                vec![("GET", inspect, final_status.0, final_status.1)],
            ]);
        }
        match service_path {
            20 => {
                operations[6][0].2 = 404;
                operations[6][0].3 =
                    json!({"message":format!("No such container: {}", "a".repeat(64))});
                operations.truncate(7);
            }
            21 => {
                operations[6][0].3["State"]["Status"] = "exited".into();
                operations.drain(7..9);
            }
            22 => {
                operations[8][0].3 = running.clone();
                operations.truncate(9);
            }
            23 => {
                operations[6][0].3["State"]["Status"] = "paused".into();
                operations.truncate(7);
            }
            24 => {
                operations[10][0].2 = 200;
                operations[10][0].3 = running.clone();
            }
            25 => operations.truncate(6),
            26 => {
                operations[7].truncate(2);
                operations[7][1].2 = 500;
                operations[7][1].3 = json!({"message":"operator stop failed"});
                operations.truncate(8);
            }
            17 => operations.truncate(6),
            11 => {
                operations[6][0].2 = 404;
                operations[6][0].3 =
                    json!({"message":format!("No such container: {}", "a".repeat(64))});
                operations.truncate(7);
            }
            13 | 14 => {
                operations[3][2].3["NetworkSettings"]["Ports"] = if service_path == 13 {
                    json!({"1234/tcp":[{"HostIp":"127.0.0.1","HostPort":"49152"},{"HostIp":"127.0.0.1","HostPort":"49153"}]})
                } else {
                    json!({"4321/tcp":[{"HostIp":"127.0.0.1","HostPort":"49152"}]})
                };
                operations.truncate(4);
            }
            15 => {
                operations[9][1].2 = 500;
                operations[9][1].3 = json!({"message":"remove failed"});
                operations.truncate(10);
            }
            5 => {
                operations[2][0].3 = running.clone();
                operations.truncate(3);
            }
            6 => {
                operations[8][0].3 = running.clone();
                operations.truncate(9);
            }
            7 => {
                let mut exited = running.clone();
                exited["State"]["Status"] = "exited".into();
                operations[6][0].3 = exited;
                operations.truncate(7);
            }
            8 => {
                operations[7].truncate(2);
                operations[7][1].2 = 500;
                operations[7][1].3 = json!({"message":"stop failed"});
                operations.truncate(8);
            }
            _ => {}
        }
        let mut public_object: Option<(String, Value)> = None;
        let last = operations.len() - 1;
        for (index, operation) in operations.into_iter().enumerate() {
            if service_path == 19 && index == 6 {
                wait_operator
                    .recv_timeout(Duration::from_secs(30))
                    .expect("operator command not armed");
            }
            // A malformed final absence response aborts before after-observation.
            let after = if matches!(service_path, 4 | 8 | 13 | 14 | 15 | 26) && index == last {
                vec![]
            } else {
                identity()
            };
            for (method, path, status, mut value) in
                identity().into_iter().chain(operation).chain(after)
            {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "missing joint request {method} {path}"
                            );
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                    assert!(header.len() < 16384);
                }
                let header = String::from_utf8(header).unwrap();
                if matches!(service_path, 16..=19 | 27 | 28) && path.contains("/containers/create?")
                {
                    let route = header
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let name = route
                        .strip_prefix("/v1.47/containers/create?name=gatehouse-")
                        .unwrap();
                    assert_eq!(name.len(), 32);
                    assert!(name.bytes().all(|b| b.is_ascii_hexdigit()));
                    public_object = Some((format!("/gatehouse-{name}"), Value::Null));
                } else {
                    assert!(header.starts_with(&format!("{method} {path} HTTP/1.1\r\n")));
                }
                let length: usize = header
                    .lines()
                    .find_map(|s| s.strip_prefix("Content-Length: "))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                if path.contains("/containers/create?") {
                    let body: Value = serde_json::from_slice(&body).unwrap();
                    if let Some((_, labels)) = &mut public_object {
                        *labels = body["Labels"].clone();
                        assert_eq!(labels["gatehouse.lifecycle.resource"], "service:fixture");
                    }
                    assert_eq!(body["HostConfig"]["AutoRemove"], false);
                    assert_eq!(body["Image"], format!("sha256:{}", "e".repeat(64)));
                }
                if value.get("Config").is_some() {
                    if let Some((name, labels)) = &public_object {
                        value["Name"] = name.clone().into();
                        value["Config"]["Labels"] = labels.clone();
                    }
                }
                let bytes = if value.is_null() {
                    vec![]
                } else {
                    serde_json::to_vec(&value).unwrap()
                };
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n\r\n",
                    bytes.len()
                )
                .unwrap();
                stream.write_all(&bytes).unwrap();
            }
        }
    });
    if matches!(service_path, 16..=19 | 27 | 28) {
        use gatehouse_runtime_protocol::lifecycle::journal::ServiceSlot;
        crate::preset::init(directory.path(), "generic", false).unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(directory.path())
            .status()
            .unwrap()
            .success());
        let mut project =
            crate::project::Project::discover(Some(directory.path().to_path_buf()), None).unwrap();
        project.config.services.insert(
            "fixture".into(),
            serde_json::from_value(json!({
                "kind":"docker", "runtime":"docker", "image":"example:latest",
                "lifecycle_host":host_path, "inject_env":"FIXTURE_URL", "startup_timeout_secs":20,
                "container_port":1234, "environment":{}, "healthcheck":["check"],
                "connection":"tcp://127.0.0.1:{host_port}"
            }))
            .unwrap(),
        );
        let mut manager = crate::service::ServiceManager::new(&project);
        if matches!(service_path, 17..=19 | 27) {
            let error = manager.handle("fixture").unwrap().acquire().err().unwrap();
            assert!(format!("{error:#}").contains("health failed"), "{error:#}");
            drop(manager);
            let base = directory.path().join(".harness-gate/lifecycle-services");
            let root = ServiceSlot::mapped_root(
                &base,
                &project.input().project_identity,
                "service:fixture",
            )
            .unwrap();
            let retained = fs::read(root.join("service.json")).unwrap();
            let record: Value = serde_json::from_slice(&retained).unwrap();
            assert_eq!(record["released"], false);
            if matches!(service_path, 18 | 19 | 27) {
                let invocation = record["resolve"]["binding"]["lease_invocation"]
                    .as_str()
                    .unwrap();
                assert!(
                    crate::service::cleanup_lifecycle(&project, "fixture", 2, invocation).is_err()
                );
                assert!(
                    crate::service::cleanup_lifecycle(&project, "fixture", 1, &"0".repeat(32))
                        .is_err()
                );
                let auth = if service_path == 27 {
                    use clap::Parser;
                    fs::write(
                        &project.config_path,
                        toml::to_string_pretty(&project.config).unwrap(),
                    )
                    .unwrap();
                    let run = |fence: &str| {
                        let cli = crate::cli::Cli::try_parse_from([
                            "harness-gate",
                            "--project-root",
                            project.root.to_str().unwrap(),
                            "lifecycle-cleanup",
                            "--service",
                            "fixture",
                            "--fence",
                            fence,
                            "--lease-invocation",
                            invocation,
                        ])
                        .unwrap();
                        crate::app::run_cli(cli)
                    };
                    assert!(run("2").is_err());
                    assert_eq!(fs::read(root.join("service.json")).unwrap(), retained);
                    assert!(run("1").unwrap());
                    let proofs: Vec<_> = fs::read_dir(&root)
                        .unwrap()
                        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                        .filter(|name| {
                            name.starts_with("operator-") && name.ends_with(".proof.json")
                        })
                        .collect();
                    assert_eq!(proofs.len(), 1);
                    proofs[0]
                        .strip_prefix("operator-")
                        .unwrap()
                        .strip_suffix(".proof.json")
                        .unwrap()
                        .to_owned()
                } else if service_path == 19 {
                    let core = std::env::var_os("GATEHOUSE_LIFECYCLE_CORE")
                        .expect("explicit normal Core binary required");
                    let digest = std::env::var("GATEHOUSE_LIFECYCLE_CORE_SHA256")
                        .expect("explicit normal Core digest required");
                    assert_eq!(
                        hash(&fs::read(&core).unwrap()),
                        digest,
                        "wrong normal Core artifact"
                    );
                    fs::write(
                        &project.config_path,
                        toml::to_string_pretty(&project.config).unwrap(),
                    )
                    .unwrap();
                    let run = |fence: &str, lease: &str| {
                        std::process::Command::new(&core)
                            .arg("--project-root")
                            .arg(&project.root)
                            .args([
                                "lifecycle-cleanup",
                                "--service",
                                "fixture",
                                "--fence",
                                fence,
                                "--lease-invocation",
                                lease,
                            ])
                            .env_clear()
                            .current_dir("/")
                            .output()
                            .unwrap()
                    };
                    let refuse = |output: std::process::Output, expected: &str| {
                        assert_eq!(output.status.code(), Some(1));
                        assert!(output.stdout.is_empty());
                        assert!(
                            String::from_utf8_lossy(&output.stderr).contains(expected),
                            "{}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    };
                    refuse(
                        run("2", invocation),
                        "does not match current resource fence",
                    );
                    refuse(
                        run("1", &"0".repeat(32)),
                        "does not match current resource fence",
                    );
                    assert_eq!(fs::read(root.join("service.json")).unwrap(), retained);
                    operator_ready.send(()).unwrap();
                    let output = run("1", invocation);
                    assert!(
                        output.status.success(),
                        "operator CLI failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert!(output.stderr.is_empty(), "unexpected operator stderr");
                    let released = fs::read(root.join("service.json")).unwrap();
                    refuse(run("1", invocation), "released slot");
                    let mut changed = project.config.clone();
                    if let crate::config::ServiceConfig::Docker { lifecycle_host, .. } =
                        changed.services.get_mut("fixture").unwrap()
                    {
                        *lifecycle_host = None;
                    }
                    fs::write(
                        &project.config_path,
                        toml::to_string_pretty(&changed).unwrap(),
                    )
                    .unwrap();
                    refuse(
                        run("1", invocation),
                        "requires explicit Docker lifecycle_host",
                    );
                    if let crate::config::ServiceConfig::Docker { lifecycle_host, .. } =
                        changed.services.get_mut("fixture").unwrap()
                    {
                        *lifecycle_host = Some(
                            directory
                                .path()
                                .join("missing-host.json")
                                .to_string_lossy()
                                .into_owned()
                                .into(),
                        );
                    }
                    fs::write(
                        &project.config_path,
                        toml::to_string_pretty(&changed).unwrap(),
                    )
                    .unwrap();
                    let unsupported = run("1", invocation);
                    assert_eq!(unsupported.status.code(), Some(1));
                    assert!(unsupported.stdout.is_empty());
                    assert!(!unsupported.stderr.is_empty());
                    assert_eq!(fs::read(root.join("service.json")).unwrap(), released);
                    fs::write(
                        &project.config_path,
                        toml::to_string_pretty(&project.config).unwrap(),
                    )
                    .unwrap();
                    let stdout = String::from_utf8(output.stdout).unwrap();
                    stdout
                        .trim()
                        .strip_prefix("Lifecycle service released; operator authorization ")
                        .unwrap()
                        .to_owned()
                } else {
                    crate::service::cleanup_lifecycle(&project, "fixture", 1, invocation).unwrap()
                };
                assert_eq!(auth.len(), 32);
                assert!(root.join(format!("operator-{auth}.proof.json")).exists());
                let done: Value =
                    serde_json::from_slice(&fs::read(root.join("service.json")).unwrap()).unwrap();
                assert_eq!(done["released"], true);
                assert!(
                    crate::service::cleanup_lifecycle(&project, "fixture", 1, invocation).is_err()
                );
                server.join().unwrap();
                return;
            }
            server.join().unwrap();
            if let crate::config::ServiceConfig::Docker { image, .. } =
                project.config.services.get_mut("fixture").unwrap()
            {
                *image = "another:latest".into();
            }
            let mut next = crate::service::ServiceManager::new(&project);
            let error = next.handle("fixture").unwrap().acquire().err().unwrap();
            assert!(format!("{error:#}").contains("slot retained"), "{error:#}");
            assert_eq!(fs::read(root.join("service.json")).unwrap(), retained);
            return;
        }
        let lease = manager.handle("fixture").unwrap().acquire().unwrap();
        assert_eq!(
            lease.environment(),
            ("FIXTURE_URL".into(), "tcp://127.0.0.1:49152".into())
        );
        let base = directory.path().join(".harness-gate/lifecycle-services");
        let root =
            ServiceSlot::mapped_root(&base, &project.input().project_identity, "service:fixture")
                .unwrap();
        let active: Value =
            serde_json::from_slice(&fs::read(root.join("service.json")).unwrap()).unwrap();
        assert_eq!(active["released"], false);
        drop(lease);
        if service_path == 28 {
            let error = manager.cleanup().unwrap_err().to_string();
            assert!(
                error.contains("lifecycle cleanup retained the resource"),
                "{error}"
            );
            let retained: Value =
                serde_json::from_slice(&fs::read(root.join("service.json")).unwrap()).unwrap();
            assert_eq!(retained["released"], false);
            let invocation = active["resolve"]["binding"]["lease_invocation"]
                .as_str()
                .unwrap();
            let journal = root.join(format!("journal-{invocation}"));
            assert!(journal.join("active").exists());
            assert!(!journal.join("tombstone.json").exists());
            server.join().unwrap();
            return;
        }
        manager.cleanup().unwrap();
        let released: Value =
            serde_json::from_slice(&fs::read(root.join("service.json")).unwrap()).unwrap();
        assert_eq!(released["released"], true);
        server.join().unwrap();
        return;
    }
    let base_root = directory.path().join("journal");
    let root = if service_path != 0 {
        gatehouse_runtime_protocol::lifecycle::journal::ServiceSlot::mapped_root(
            &base_root,
            &request.binding.project_id,
            &request.binding.resource_id,
        )
        .unwrap()
    } else {
        base_root.clone()
    };
    if service_path != 0 {
        use crate::service::lifecycle::{LifecycleService, ServiceIdentity};
        let prepared = LifecycleService::prepare(
            &base_root,
            installed,
            ServiceIdentity {
                project_id: request.binding.project_id.clone(),
                resource_id: request.binding.resource_id.clone(),
                lease_invocation: request.binding.lease_invocation.clone(),
            },
            "example:latest".into(),
            plan.clone(),
            &["check".into()],
        )
        .unwrap();
        let result = prepared.start(Instant::now() + Duration::from_secs(20), || false);
        let journal_root = root.join(format!("journal-{}", request.binding.lease_invocation));
        if service_path == 9 || (20..=26).contains(&service_path) {
            use gatehouse_runtime_protocol::lifecycle::journal::OperatorRecovery;
            assert!(result.err().unwrap().to_string().contains("health failed"));
            let mut recovery = OperatorRecovery::open_mapped(
                &base_root,
                &request.binding.project_id,
                &request.binding.resource_id,
                1,
                &request.binding.lease_invocation,
                &"9".repeat(32),
            )
            .unwrap();
            assert!(recovery
                .begin(
                    Operation::ResolveImage {
                        image: "example:latest".into()
                    },
                    &"8".repeat(32)
                )
                .is_err());
            let cleanup = crate::service::lifecycle::recovery::cleanup_known(
                recovery,
                VerifiedInstallation::load(&host_path).unwrap(),
                Instant::now() + Duration::from_secs(20),
                || service_path == 25,
            );
            if (22..=26).contains(&service_path) {
                let expected = match service_path {
                    22 => "operator stop not proved",
                    23 => "operator recovery state unsupported",
                    24 => "operator removal absence not proved",
                    25 => "operator recovery cancelled",
                    26 => "lifecycle dispatch failed",
                    _ => unreachable!(),
                };
                let error = cleanup.unwrap_err().to_string();
                assert!(error.contains(expected), "{error}");
                assert!(journal_root.join("active").exists());
                assert!(!journal_root.join("tombstone.json").exists());
                assert!(!root
                    .join(format!("operator-{}.proof.json", "9".repeat(32)))
                    .exists());
                let record: Value =
                    serde_json::from_slice(&fs::read(root.join("service.json")).unwrap()).unwrap();
                assert_eq!(record["released"], false);
                server.join().unwrap();
                return;
            }
            cleanup.unwrap();
            assert!(journal_root.join("tombstone.json").exists());
            assert!(!journal_root.join("active").exists());
            assert!(root
                .join(format!("operator-{}.proof.json", "9".repeat(32)))
                .exists());
            assert!(OperatorRecovery::open_mapped(
                &base_root,
                &request.binding.project_id,
                &request.binding.resource_id,
                1,
                &request.binding.lease_invocation,
                &"9".repeat(32)
            )
            .is_err());
            server.join().unwrap();
            return;
        }
        if matches!(service_path, 5 | 13 | 14) {
            let error = result.err().unwrap().to_string();
            if service_path == 5 {
                assert!(error.contains("unexpected state"));
            } else {
                assert!(error.contains("dispatch failed"));
            }
            assert!(journal_root.join("active").exists());
            server.join().unwrap();
            return;
        }
        if service_path >= 2 {
            let service = result.unwrap();
            assert_eq!(service.port(), 49152);
            let cleanup = service.cleanup(Instant::now() + Duration::from_secs(20), || false);
            if service_path == 2 {
                cleanup.unwrap();
                assert!(!journal_root.join("active").exists());
                assert!(journal_root.join("tombstone.json").exists());
                // Exact crash state: journal released, slot publication not done.
                let slot_file = root.join("service.json");
                let mut slot_record: Value =
                    serde_json::from_slice(&fs::read(&slot_file).unwrap()).unwrap();
                slot_record["released"] = false.into();
                fs::write(&slot_file, serde_json::to_vec(&slot_record).unwrap()).unwrap();
                use gatehouse_runtime_protocol::lifecycle::journal::{
                    OperatorRecovery, ServiceSlot,
                };
                assert!(
                    ServiceSlot::acquire(&root, request.clone(), plan.clone(), vec![]).is_err()
                );
                assert!(OperatorRecovery::open_mapped(
                    &base_root,
                    &request.binding.project_id,
                    &request.binding.resource_id,
                    2,
                    &request.binding.lease_invocation,
                    &"9".repeat(32)
                )
                .is_err());
                let recovery = OperatorRecovery::open_mapped(
                    &base_root,
                    &request.binding.project_id,
                    &request.binding.resource_id,
                    1,
                    &request.binding.lease_invocation,
                    &"9".repeat(32),
                )
                .unwrap();
                crate::service::lifecycle::recovery::cleanup_known(
                    recovery,
                    VerifiedInstallation::load(&host_path).unwrap(),
                    Instant::now() + Duration::from_secs(5),
                    || false,
                )
                .unwrap();
                assert!(root
                    .join(format!("operator-{}.proof.json", "9".repeat(32)))
                    .exists());
                // A released flag alone is insufficient; reacquisition validates
                // the old resolution/journal/tombstone, retains it and increments.
                assert!(
                    ServiceSlot::acquire(&root, request.clone(), plan.clone(), vec![]).is_err()
                );
                let mut next = request.clone();
                next.binding.lease_invocation = "7".repeat(32);
                next.binding.invocation = "8".repeat(32);
                next.binding.operation_key = next.operation_key().unwrap();
                let mut next_plan = plan.clone();
                next_plan
                    .labels
                    .iter_mut()
                    .find(|e| e.key == "gatehouse.lifecycle.invocation")
                    .unwrap()
                    .value = next.binding.lease_invocation.clone();
                let history = root.join("service-history-1.json");
                let prior_history = fs::read(&history).unwrap();
                fs::write(&history, b"{}").unwrap();
                assert!(ServiceSlot::acquire(
                    &root,
                    next.clone(),
                    next_plan.clone(),
                    vec!["check".into()]
                )
                .is_err());
                fs::write(&history, prior_history).unwrap();
                let slot =
                    ServiceSlot::acquire(&root, next, next_plan, vec!["check".into()]).unwrap();
                assert_eq!(slot.fence(), 2);
                assert!(root.join("service-history-1.json").exists());
                assert!(journal_root.join("tombstone.json").exists());
            } else {
                assert!(cleanup.is_err());
                assert!(journal_root.join("active").exists());
                assert!(!journal_root.join("tombstone.json").exists());
            }
            server.join().unwrap();
            return;
        }
        assert!(result.err().unwrap().to_string().contains("health failed"));
        let record: Value =
            serde_json::from_slice(&fs::read(journal_root.join("lease.json")).unwrap()).unwrap();
        assert_eq!(record["create"]["binding"]["generation"], 2);
        assert!(!record["resolution"].is_null());
        assert!(journal_root.join("active").exists());
        assert!(journal_root.join("reservation.json").exists());
        assert!(
            gatehouse_runtime_protocol::lifecycle::journal::ServiceSlot::acquire(
                &root,
                request.clone(),
                plan,
                vec![]
            )
            .is_err()
        );
        server.join().unwrap();
        return;
    }
    let mut reservation = Reservation::create(&root, &request, plan.clone()).unwrap();
    let response = resolve(reservation.dispatch().unwrap(), &installed, 3000, || false).unwrap();
    assert!(matches!(response.outcome, Outcome::Image { .. }));
    assert!(reservation.dispatch().is_err());
    let mut journal = reservation.into_journal(&"3".repeat(32)).unwrap();
    let token = journal.dispatch().unwrap();
    let create = token.request().clone();
    assert_eq!(create.binding.generation, 2);
    let response = dispatch(token, &installed, 3000, || false).unwrap();
    assert!(matches!(response.outcome, Outcome::Created { .. }));
    assert_eq!(journal.phase(), Phase::KnownCreated);
    let target = journal.target().unwrap();
    assert_eq!(target.object_id, "a".repeat(64));
    for (generation, operation) in [
        (
            3,
            Operation::Status {
                target: target.clone(),
            },
        ),
        (
            4,
            Operation::Start {
                target: target.clone(),
            },
        ),
        (
            5,
            Operation::Status {
                target: target.clone(),
            },
        ),
        (
            6,
            Operation::HealthExec {
                target: target.clone(),
                argv: vec!["check".into()],
                timeout_ms: 2000,
            },
        ),
    ] {
        let mut next = create.clone();
        next.operation = operation;
        next.binding.generation = generation;
        next.binding.invocation = format!("{:032x}", generation + 100);
        next.binding.operation_key = next.operation_key().unwrap();
        let response = dispatch(
            journal.begin(&next, Purpose::Service).unwrap(),
            &installed,
            3000,
            || false,
        )
        .unwrap();
        match generation {
            3 => {
                assert!(matches!(response.outcome, Outcome::Status { .. }));
                assert_eq!(journal.phase(), Phase::KnownCreated);
            }
            4 => {
                assert!(matches!(response.outcome, Outcome::Started { .. }));
                assert_eq!(journal.phase(), Phase::Running);
            }
            5 => assert!(matches!(response.outcome, Outcome::Status { .. })),
            6 => {
                assert!(matches!(
                    response.outcome,
                    Outcome::Health { exit_code: 7, .. }
                ));
                assert_eq!(journal.phase(), Phase::Retained);
            }
            _ => unreachable!(),
        }
    }
    assert!(journal.dispatch().is_err());
    drop(journal);
    assert!(Reservation::open(&root, &request, &plan).is_err());
    server.join().unwrap();
}
