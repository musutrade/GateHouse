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
    let (directory, installed, mut request, _) = fixture_inputs("created", &binary, Some(&socket));
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
        let operations: Vec<Vec<Step>> = vec![
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
                    json!({"ID":"b".repeat(64),"ContainerID":"a".repeat(64),"Running":false,"ExitCode":7}),
                ),
                ("GET", inspect, 200, running),
            ],
        ];
        for operation in operations {
            for (method, path, status, value) in
                identity().into_iter().chain(operation).chain(identity())
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
                assert!(header.starts_with(&format!("{method} {path} HTTP/1.1\r\n")));
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
                    assert_eq!(body["HostConfig"]["AutoRemove"], false);
                    assert_eq!(body["Image"], format!("sha256:{}", "e".repeat(64)));
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
    let root = directory.path().join("journal");
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
