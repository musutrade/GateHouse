//! Operator-pinned runtime transport. The public probe rejects Remove.
//! Private mutation entry requires a store-bound, durably committed token.
use super::adapter::{HostPolicy, VerifiedExecutable};
use super::{raw_adapter, signal};
use anyhow::{bail, Result};
use gatehouse_runtime_protocol::{Operation, Request, Response, MAX_MESSAGE_BYTES};
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustedHost {
    schema: String,
    executable: PathBuf,
    cwd: PathBuf,
    expected: Request,
    timeout_ms: u64,
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    // Limit the actual read, not merely a racy metadata length observation.
    let mut bytes = Vec::new();
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|_| anyhow::anyhow!("cannot open runtime input"))?;
    if !file
        .metadata()
        .map_err(|_| anyhow::anyhow!("cannot inspect runtime input"))?
        .is_file()
    {
        bail!("runtime input must be a regular file");
    }
    file.take(MAX_MESSAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("cannot read runtime input"))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        bail!("runtime input exceeds size limit");
    }
    Ok(bytes)
}

impl TrustedHost {
    fn validate(&self, request: &Request) -> Result<()> {
        if self.schema != "gate-runtime-host/v1" {
            bail!("unsupported runtime host schema");
        }
        request.require_same_intent(&self.expected)?;
        if !self.executable.is_absolute() || !self.cwd.is_absolute() {
            bail!("runtime host paths must be absolute");
        }
        if self.timeout_ms == 0 || self.timeout_ms > 30_000 {
            bail!("runtime probe timeout must be between 1 and 30000ms");
        }
        let metadata = std::fs::symlink_metadata(&self.cwd)
            .map_err(|_| anyhow::anyhow!("runtime working directory unavailable"))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            bail!("runtime working directory must be a directory, not a symlink");
        }
        let canonical = std::fs::canonicalize(&self.cwd)
            .map_err(|_| anyhow::anyhow!("runtime working directory unavailable"))?;
        if canonical != self.cwd {
            bail!("runtime working directory must be a canonical absolute path");
        }
        Ok(())
    }
}

pub(crate) fn run_files(request: &Path, trusted_host: &Path) -> Result<Response> {
    if !cfg!(unix) {
        bail!("runtime probe is supported only on Unix in this version");
    }
    let request_bytes = read_bounded(request)?;
    let trusted_bytes = read_bounded(trusted_host)?;
    let host: TrustedHost = serde_json::from_slice(&trusted_bytes)
        .map_err(|_| anyhow::anyhow!("invalid trusted runtime host document"))?;
    let request = gatehouse_runtime_protocol::parse_request(&request_bytes)?;
    run(&host, &request, signal::cancelled)
}

fn run(host: &TrustedHost, request: &Request, cancelled: impl Fn() -> bool) -> Result<Response> {
    if matches!(request.operation, Operation::Remove { .. }) {
        bail!("runtime probe does not authorize removal");
    }
    execute(host, request, cancelled)
}

// Reuse exactly the probe's pinned executable, bounds, environment and strict
// response path. Only the consuming-token entry below can pass Remove here.
fn execute(
    host: &TrustedHost,
    request: &Request,
    cancelled: impl Fn() -> bool,
) -> Result<Response> {
    host.validate(request)?;
    if cancelled() {
        bail!("runtime probe cancelled before dispatch");
    }
    let executable = VerifiedExecutable::open(&host.executable, &request.binding.adapter_sha256)
        .map_err(|_| anyhow::anyhow!("runtime executable verification failed"))?;
    let bytes = serde_json::to_vec(request)
        .map_err(|_| anyhow::anyhow!("runtime request serialization failed"))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        bail!("runtime request exceeds protocol limit");
    }
    let policy = HostPolicy {
        max_stdout_bytes: MAX_MESSAGE_BYTES,
        max_stderr_bytes: MAX_MESSAGE_BYTES,
        max_output_bytes: MAX_MESSAGE_BYTES,
        reader_deadline: Duration::from_secs(2),
        ..HostPolicy::default()
    };
    let mut command = Command::new(&host.executable);
    command.current_dir(&host.cwd).env_clear();
    executable
        .revalidate()
        .map_err(|_| anyhow::anyhow!("runtime executable changed before dispatch"))?;
    if cancelled() {
        bail!("runtime probe cancelled before dispatch");
    }
    let output = raw_adapter::run(
        &mut command,
        &policy,
        host.timeout_ms,
        bytes,
        Instant::now(),
        &cancelled,
    )
    .map_err(|_| anyhow::anyhow!("runtime probe transport failed; no absence established"))?;
    raw_adapter::validate_process_output(&output, &policy)
        .map_err(|_| anyhow::anyhow!("runtime probe output limit exceeded"))?;
    validate_exit(&output)?;
    executable
        .revalidate()
        .map_err(|_| anyhow::anyhow!("runtime executable changed during probe"))?;
    let response = gatehouse_runtime_protocol::parse_response(&output.stdout.bytes, request)?;
    if cancelled() {
        bail!("runtime probe cancelled after dispatch");
    }
    Ok(response)
}

fn validate_exit(output: &raw_adapter::ProcessOutput) -> Result<()> {
    if output.timed_out || output.was_cancelled || output.request_write_error.is_some() {
        bail!("runtime probe did not complete; no absence established");
    }
    if !output.status.success() {
        bail!("runtime probe exited unsuccessfully; no absence established");
    }
    Ok(())
}

/// Internal staged entry, intentionally not exposed by the CLI/service manager.
/// The token is consumed even on failure; its borrow retains the directory lock.
/// A mutation response can never complete the intent, including `completed`.
#[cfg(unix)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "staged internal host; engine integration pending")
)]
pub(crate) fn dispatch_authorized(
    authorization: gatehouse_runtime_protocol::store::DispatchAuthorization<'_>,
    trusted_host: &Path,
    cancelled: impl Fn() -> bool,
) -> Result<Response> {
    let bytes = read_bounded(trusted_host)?;
    let host: TrustedHost = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid trusted runtime host document"))?;
    let response = execute(&host, authorization.request(), &cancelled)?;
    if cancelled() {
        bail!("runtime mutation cancelled; retain unknown intent");
    }
    authorization.observe(Some(&response))?;
    Ok(response)
}

/// Recovery never calls the mutation entry. Present/unavailable or any error
/// retains Unknown; only a separately pinned exact-ID Absent can complete it.
#[cfg(unix)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "staged internal recovery; engine integration pending"
    )
)]
pub(crate) fn recover_unknown(
    store: &mut gatehouse_runtime_protocol::store::Store,
    trusted_host: &Path,
    cancelled: impl Fn() -> bool,
) -> Result<Response> {
    let request = store.recovery_request()?;
    let bytes = read_bounded(trusted_host)?;
    let host: TrustedHost = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid trusted runtime host document"))?;
    let response = run(&host, &request, &cancelled)?;
    if cancelled() {
        bail!("runtime recovery cancelled; retain unknown intent");
    }
    if matches!(
        response.outcome,
        gatehouse_runtime_protocol::Outcome::Absent
    ) {
        store.reconcile(&response)?;
    }
    Ok(response)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use gatehouse_runtime_protocol::store::Store;
    use gatehouse_runtime_protocol::{
        Binding, Engine, Label, Namespace, Object, Outcome, Protocol, Request, Response,
    };
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tempfile::TempDir;

    // Fake-subprocess tests only. No real engine, no power-loss claim, and no
    // hostile same-user isolation claim is made.

    fn id() -> String {
        "a".repeat(64)
    }

    fn binding(digest: &str) -> Binding {
        Binding {
            protocol: Protocol::V1,
            adapter_sha256: digest.into(),
            adapter_version: "1.0.0".into(),
            invocation: "inv-1".into(),
            operation_key: "op-1".into(),
            generation: "gen-1".into(),
            namespace: Namespace {
                engine: Engine::Docker,
                endpoint: "unix:///run/docker.sock".into(),
                context: "ctx-1".into(),
                principal: "uid:1000".into(),
                rootless: true,
                scope: "scope-1".into(),
            },
        }
    }

    fn removal(digest: &str) -> Request {
        Request {
            binding: binding(digest),
            operation: Operation::Remove { object_id: id() },
        }
    }

    fn labels() -> Vec<Label> {
        vec![Label {
            key: "harness-gate.owner".into(),
            value: "harness-gate".into(),
        }]
    }

    fn present(request: &Request) -> Response {
        Response {
            binding: request.binding.clone(),
            operation: Operation::Inspect { object_id: id() },
            outcome: Outcome::Present {
                object: Object {
                    object_id: id(),
                    labels: labels(),
                },
            },
        }
    }

    struct Fixture {
        _temporary: TempDir,
        root: PathBuf,
        dispatch: PathBuf,
        ready: PathBuf,
        capture: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().join("intent");
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            Self {
                dispatch: temporary.path().join("dispatched"),
                ready: temporary.path().join("ready"),
                capture: temporary.path().join("request.bin"),
                root: fs::canonicalize(root).unwrap(),
                _temporary: temporary,
            }
        }

        fn adapter(&self, mode: &str) -> (PathBuf, String) {
            let path = self.root.join(format!("adapter-{mode}.py"));
            let python = Command::new("python3")
                .args(["-c", "import sys; print(sys.executable)"])
                .output()
                .unwrap();
            assert!(python.status.success());
            let python = String::from_utf8(python.stdout).unwrap().trim().to_owned();
            let body = r#"
import json,sys,time
data=sys.stdin.buffer.read()
open(__CAPTURE__,'wb').write(data)
open(__DISPATCH__,'a').write('x')
req=json.loads(data)
mode=__MODE__
if mode=='cancel':
    open(__READY__,'w').write('ready')
    time.sleep(30)
    sys.exit(0)
if mode=='timeout':
    time.sleep(30)
if mode=='nonzero':
    print('ADAPTER-NOISE',file=sys.stderr)
    sys.exit(2)
if mode=='oversize':
    sys.stdout.write('x'*70000)
    sys.exit(0)
if mode=='invalid':
    sys.stdout.write('not-json')
    sys.exit(0)
op=req['operation']
if mode=='absent':
    outcome='absent'
elif mode=='present':
    outcome={'present':{'object':{'object_id':op['inspect']['object_id'],'labels':[{'key':'k','value':'v'}]}}}
elif mode=='unavailable':
    outcome='unavailable'
elif mode=='rejected':
    outcome='rejected-before-effect'
elif mode=='unknown':
    outcome='unknown'
else:
    outcome='completed'
binding=req['binding']
if mode=='mismatch':
    binding['generation']='gen-2'
if mode=='duplicate':
    sys.stdout.write('{"binding":'+json.dumps(req['binding'])+',"operation":'+json.dumps(op)+',"outcome":"unknown","outcome":"unknown"}')
    sys.exit(0)
if mode=='selfmodify':
    original=open(sys.argv[0],'rb').read()
    open(sys.argv[0],'wb').write(original+b'\n# changed\n')
if mode=='wrong_absent':
    op={'inspect':{'object_id':'b'*64}}
    outcome='absent'
payload=json.dumps({'binding':binding,'operation':op,'outcome':outcome},separators=(',',':'))
sys.stdout.write(payload)
if mode=='after_response':
    sys.stdout.flush()
    open(__READY__,'w').write('ready')
    time.sleep(30)
"#;
            let script = format!(
                "#!{python}\n{}",
                body.replace(
                    "__CAPTURE__",
                    &serde_json::to_string(&self.capture).unwrap()
                )
                .replace(
                    "__DISPATCH__",
                    &serde_json::to_string(&self.dispatch).unwrap()
                )
                .replace("__READY__", &serde_json::to_string(&self.ready).unwrap())
                .replace("__MODE__", &serde_json::to_string(mode).unwrap())
            );
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            let digest = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
            (path, digest)
        }

        fn host(&self, executable: &Path, expected: &Request, timeout_ms: u64) -> PathBuf {
            let path = self.root.join("trusted-host.json");
            let host = json!({
                "schema": "gate-runtime-host/v1",
                "executable": executable,
                "cwd": self.root,
                "expected": expected,
                "timeout_ms": timeout_ms
            });
            fs::write(&path, serde_json::to_vec(&host).unwrap()).unwrap();
            path
        }

        fn request_file(&self, request: &Request) -> PathBuf {
            let path = self.root.join("request.json");
            fs::write(&path, serde_json::to_vec(request).unwrap()).unwrap();
            path
        }

        fn dispatched(&self) -> bool {
            self.dispatch.exists()
        }

        fn captured(&self) -> Value {
            serde_json::from_slice(&fs::read(&self.capture).unwrap()).unwrap()
        }
    }

    fn open_store(root: &Path, request: &Request) -> Store {
        Store::open(root, request, Duration::from_secs(1), || false).unwrap()
    }

    fn prepare(store: &mut Store, request: &Request) {
        store.prepare(&present(request), &labels()).unwrap();
    }

    fn authorization(
        store: &mut Store,
    ) -> gatehouse_runtime_protocol::store::DispatchAuthorization<'_> {
        store.begin_dispatch().unwrap()
    }

    fn unknown_store(fixture: &Fixture, request: &Request) -> Store {
        let mut store = open_store(&fixture.root, request);
        prepare(&mut store, request);
        let token = authorization(&mut store);
        drop(token);
        store
    }

    fn assert_unknown(store: &Store) {
        assert_eq!(
            store.phase(),
            Some(gatehouse_runtime_protocol::intent::Phase::Unknown)
        );
    }

    #[test]
    fn token_and_host_request_mismatch_never_dispatches() {
        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("completed");
        let request = removal(&digest);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        let mut changed = request.clone();
        changed.binding.generation = "gen-2".into();
        let host = fixture.host(&adapter, &changed, 5_000);
        assert!(dispatch_authorized(token, &host, || false).is_err());
        assert_unknown(&store);
        assert!(!fixture.dispatched());
        assert!(store.begin_dispatch().is_err());
    }

    #[test]
    fn successful_mutation_outcomes_remain_unknown_and_stdin_is_exact() {
        for mode in ["completed", "rejected", "unknown"] {
            let fixture = Fixture::new();
            let (adapter, digest) = fixture.adapter(mode);
            let request = removal(&digest);
            let mut store = open_store(&fixture.root, &request);
            prepare(&mut store, &request);
            let host = fixture.host(&adapter, &request, 5_000);
            let token = authorization(&mut store);
            let response = dispatch_authorized(token, &host, || false).unwrap();
            assert_eq!(
                response.outcome,
                match mode {
                    "completed" => Outcome::Completed,
                    "rejected" => Outcome::RejectedBeforeEffect,
                    _ => Outcome::Unknown,
                }
            );
            assert_unknown(&store);
            let captured = fixture.captured();
            assert_eq!(captured, serde_json::to_value(&request).unwrap());
            let mut expected = serde_json::to_vec(&request).unwrap();
            expected.push(b'\n');
            assert_eq!(fs::read(&fixture.capture).unwrap(), expected);
            assert!(fixture.dispatched());
        }
    }

    #[test]
    fn token_drop_and_cancellation_before_spawn_never_retry() {
        let fixture = Fixture::new();
        let (_adapter, digest) = fixture.adapter("completed");
        let request = removal(&digest);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        drop(token);
        assert_unknown(&store);
        assert!(store.begin_dispatch().is_err());
        assert!(!fixture.dispatched());
        drop(store);

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("completed");
        let request = removal(&digest);
        let host = fixture.host(&adapter, &request, 5_000);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        assert!(dispatch_authorized(token, &host, || true).is_err());
        assert_unknown(&store);
        assert!(!fixture.dispatched());
    }

    #[test]
    fn cancellation_during_and_after_response_keeps_unknown() {
        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("cancel");
        let request = removal(&digest);
        let host = fixture.host(&adapter, &request, 30_000);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        let ready = fixture.ready.clone();
        assert!(dispatch_authorized(token, &host, || ready.exists()).is_err());
        assert_unknown(&store);
        assert!(fixture.dispatched());

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("after_response");
        let request = removal(&digest);
        let host = fixture.host(&adapter, &request, 5_000);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        let ready = fixture.ready.clone();
        assert!(dispatch_authorized(token, &host, || ready.exists()).is_err());
        assert_unknown(&store);
        assert!(fixture.dispatched());
    }

    #[test]
    fn transport_and_response_failures_keep_unknown() {
        for mode in [
            "timeout",
            "nonzero",
            "oversize",
            "invalid",
            "duplicate",
            "mismatch",
        ] {
            let fixture = Fixture::new();
            let (adapter, digest) = fixture.adapter(mode);
            let request = removal(&digest);
            let host = fixture.host(&adapter, &request, 1_000);
            let mut store = open_store(&fixture.root, &request);
            prepare(&mut store, &request);
            let token = authorization(&mut store);
            assert!(dispatch_authorized(token, &host, || false).is_err());
            assert_unknown(&store);
            assert!(fixture.dispatched(), "{mode}");
        }
    }

    #[test]
    fn digest_and_executable_changes_fail_closed() {
        let fixture = Fixture::new();
        let (adapter, _digest) = fixture.adapter("completed");
        let request = removal(&"b".repeat(64));
        let host = fixture.host(&adapter, &request, 5_000);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        assert!(dispatch_authorized(token, &host, || false).is_err());
        assert_unknown(&store);
        assert!(!fixture.dispatched());

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("selfmodify");
        let request = removal(&digest);
        let host = fixture.host(&adapter, &request, 5_000);
        let mut store = open_store(&fixture.root, &request);
        prepare(&mut store, &request);
        let token = authorization(&mut store);
        assert!(dispatch_authorized(token, &host, || false).is_err());
        assert_unknown(&store);
        assert!(fixture.dispatched());
    }

    #[test]
    fn recovery_inspects_only_and_completes_only_absent() {
        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("present");
        let request = removal(&digest);
        let mut store = unknown_store(&fixture, &request);
        let inspection = store.recovery_request().unwrap();
        let host = fixture.host(&adapter, &inspection, 5_000);
        let response = recover_unknown(&mut store, &host, || false).unwrap();
        assert!(matches!(response.outcome, Outcome::Present { .. }));
        assert_unknown(&store);
        assert!(fixture.captured().get("operation").is_some());
        assert!(fixture.captured()["operation"].get("inspect").is_some());
        assert!(fixture.captured()["operation"].get("remove").is_none());

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("absent");
        let request = removal(&digest);
        let mut store = unknown_store(&fixture, &request);
        let inspection = store.recovery_request().unwrap();
        let host = fixture.host(&adapter, &inspection, 5_000);
        let response = recover_unknown(&mut store, &host, || false).unwrap();
        assert_eq!(response.outcome, Outcome::Absent);
        assert_eq!(
            store.phase(),
            Some(gatehouse_runtime_protocol::intent::Phase::Completed)
        );
        assert!(fixture.captured()["operation"].get("inspect").is_some());
    }

    #[test]
    fn recovery_retains_unknown_on_unavailable_wrong_binding_and_terminal() {
        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("unavailable");
        let request = removal(&digest);
        let mut store = unknown_store(&fixture, &request);
        let inspection = store.recovery_request().unwrap();
        let host = fixture.host(&adapter, &inspection, 5_000);
        let response = recover_unknown(&mut store, &host, || false).unwrap();
        assert_eq!(response.outcome, Outcome::Unavailable);
        assert_unknown(&store);

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("absent");
        let request = removal(&digest);
        let mut store = unknown_store(&fixture, &request);
        let inspection = store.recovery_request().unwrap();
        let mut wrong = inspection.clone();
        wrong.binding.generation = "gen-2".into();
        let host = fixture.host(&adapter, &wrong, 5_000);
        assert!(recover_unknown(&mut store, &host, || false).is_err());
        assert_unknown(&store);
        assert!(!fixture.dispatched());

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("wrong_absent");
        let request = removal(&digest);
        let mut store = unknown_store(&fixture, &request);
        let inspection = store.recovery_request().unwrap();
        let host = fixture.host(&adapter, &inspection, 5_000);
        assert!(recover_unknown(&mut store, &host, || false).is_err());
        assert_unknown(&store);

        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("absent");
        let request = removal(&digest);
        let mut store = unknown_store(&fixture, &request);
        let inspection = store.recovery_request().unwrap();
        let host = fixture.host(&adapter, &inspection, 5_000);
        recover_unknown(&mut store, &host, || false).unwrap();
        assert_eq!(
            store.phase(),
            Some(gatehouse_runtime_protocol::intent::Phase::Completed)
        );
        let dispatched = fixture.dispatch.clone();
        assert!(recover_unknown(&mut store, &host, || false).is_err());
        assert_eq!(fs::read(&dispatched).unwrap().len(), 1);
    }

    #[test]
    fn public_probe_still_refuses_remove_before_dispatch() {
        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter("completed");
        let request = removal(&digest);
        let request_path = fixture.request_file(&request);
        let host = fixture.host(&adapter, &request, 5_000);
        assert!(run_files(&request_path, &host).is_err());
        assert!(!fixture.dispatched());
    }
}
