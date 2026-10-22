//! Opt-in exact-binary joint test; missing inputs fail, never silently skip.
use super::*;
use gatehouse_runtime_protocol::lifecycle::journal::Purpose;
use std::{
    io::{Read, Write},
    os::unix::net::UnixListener,
    thread,
};
#[test]
fn pinned_native_adapter_create_and_status_through_core_host() {
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
    let mut fixture = fixture_binary("created", &binary, Some(&socket));
    let token = fixture.journal.dispatch().unwrap();
    let create = token.request().clone();
    let Operation::Create { name, labels, .. } = &create.operation else {
        panic!()
    };
    let labels: serde_json::Map<String, Value> = labels
        .iter()
        .map(|e| (e.key.clone(), e.value.clone().into()))
        .collect();
    let object = json!({"Id":"a".repeat(64),"Name":format!("/{name}"),"Image":format!("sha256:{}","e".repeat(64)),"Config":{"Labels":labels},"State":{"Status":"created"},"NetworkSettings":{"Ports":{"1234/tcp":null}}});
    let server = thread::spawn(move || {
        let version = json!({"Version":"27","MinAPIVersion":"1.40","ApiVersion":"1.47"});
        let info = json!({"ID":"test-daemon","SecurityOptions":["name=rootless"]});
        let inspect = format!("/v1.47/containers/{}/json", "a".repeat(64));
        let steps = vec![
            ("GET", "/version".into(), 200, version.clone()),
            ("GET", "/v1.47/info".into(), 200, info.clone()),
            (
                "POST",
                "/v1.47/containers/create?name=test-container".into(),
                201,
                json!({"Id":"a".repeat(64),"Warnings":[]}),
            ),
            ("GET", inspect.clone(), 200, object.clone()),
            ("GET", "/version".into(), 200, version.clone()),
            ("GET", "/v1.47/info".into(), 200, info.clone()),
            ("GET", "/version".into(), 200, version.clone()),
            ("GET", "/v1.47/info".into(), 200, info.clone()),
            ("GET", inspect, 200, object),
            ("GET", "/version".into(), 200, version),
            ("GET", "/v1.47/info".into(), 200, info),
        ];
        for (method, path, status, value) in steps {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "missing joint request");
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
            if method == "POST" {
                let body: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(body["HostConfig"]["AutoRemove"], false);
                assert_eq!(body["Image"], format!("sha256:{}", "e".repeat(64)));
            }
            let bytes = serde_json::to_vec(&value).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            stream.write_all(&bytes).unwrap();
        }
    });
    let response = dispatch(token, &fixture.installed, 3000, || false).unwrap();
    assert!(matches!(response.outcome, Outcome::Created { .. }));
    assert_eq!(fixture.journal.phase(), Phase::KnownCreated);
    let target = fixture.journal.target().unwrap();
    assert_eq!(target.object_id, "a".repeat(64));
    let mut status = create;
    status.operation = Operation::Status { target };
    status.binding.generation = 2;
    status.binding.invocation = "3".repeat(32);
    status.binding.operation_key = status.operation_key().unwrap();
    let token = fixture.journal.begin(&status, Purpose::Service).unwrap();
    let response = dispatch(token, &fixture.installed, 3000, || false).unwrap();
    assert!(matches!(response.outcome, Outcome::Status { .. }));
    assert_eq!(fixture.journal.phase(), Phase::KnownCreated);
    assert!(fixture.journal.dispatch().is_err());
    server.join().unwrap();
}
