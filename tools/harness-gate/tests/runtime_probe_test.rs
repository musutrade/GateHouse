#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NOISE: &str = "ADAPTER-STDERR-SECRET";

fn sha256(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn python() -> String {
    let output = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .expect("python3 is required for Unix subprocess fixtures");
    assert!(output.status.success(), "python3 probe failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

struct Fixture {
    dir: TempDir,
    dispatch: PathBuf,
    ready: PathBuf,
    env_out: PathBuf,
    argv_out: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let dispatch = dir.path().join("dispatched");
        let ready = dir.path().join("ready");
        let env_out = dir.path().join("env.json");
        let argv_out = dir.path().join("argv.json");
        Self {
            dir,
            dispatch,
            ready,
            env_out,
            argv_out,
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn adapter(&self, mode: &str) -> (PathBuf, String) {
        let path = self.dir.path().join(format!("adapter-{mode}.py"));
        let script = format!(
            "#!{}\nimport json,os,sys,time\nopen({dispatch},'a').write('x')\nreq=json.load(sys.stdin)\nmode={mode:?}\nif mode=='env':\n    open({env},'w').write(json.dumps(dict(os.environ)))\n    open({argv},'w').write(json.dumps(sys.argv))\nelif mode=='timeout':\n    time.sleep(30)\nelif mode=='cancel':\n    open({ready},'w').write('ready')\n    time.sleep(30)\nelif mode=='nonzero':\n    print({noise},file=sys.stderr)\n    sys.exit(3)\nelif mode=='oversize':\n    sys.stdout.write('x'*70000)\n    sys.exit(0)\nelif mode=='selfmodify':\n    original=open(sys.argv[0],'rb').read()\n    open(sys.argv[0],'wb').write(original+b'\\n# changed\\n')\nop=req['operation']\nif isinstance(op,dict) and 'inspect' in op:\n    outcome={{'present':{{'object':{{'object_id':op['inspect']['object_id'],'labels':[{{'key':'k','value':'v'}}]}}}}}}\nelse:\n    outcome={{'capabilities':{{'supported':['inspect']}}}}\nif mode=='unavailable':\n    outcome='unavailable'\nif mode=='duplicate':\n    sys.stdout.write('{{\"binding\":'+json.dumps(req['binding'])+',\"operation\":'+json.dumps(op)+',\"outcome\":\"absent\",\"outcome\":\"absent\"}}')\n    sys.exit(0)\nbinding=req['binding']\nif mode=='mismatch':\n    binding['generation']='gen-2'\npayload=json.dumps({{'binding':binding,'operation':op,'outcome':outcome}},separators=(',',':'))\nsys.stdout.write(payload)\nif mode=='latecancel':\n    sys.stdout.flush()\n    open({ready},'w').write('ready')\n    time.sleep(30)\n",
            python(),
            dispatch = serde_json::to_string(&self.dispatch).unwrap(),
            env = serde_json::to_string(&self.env_out).unwrap(),
            argv = serde_json::to_string(&self.argv_out).unwrap(),
            ready = serde_json::to_string(&self.ready).unwrap(),
            noise = serde_json::to_string(NOISE).unwrap(),
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let digest = sha256(&path);
        (path, digest)
    }

    fn request_value(&self, operation: Value, digest: &str) -> Value {
        json!({
            "binding": {
                "protocol": "gate-runtime/v1",
                "adapter_sha256": digest,
                "adapter_version": "1.0.0",
                "invocation": "inv-1",
                "operation_key": "op-1",
                "generation": "gen-1",
                "namespace": {
                    "engine": "docker",
                    "endpoint": "unix:///run/docker.sock",
                    "context": "ctx-1",
                    "principal": "uid:1000",
                    "rootless": true,
                    "scope": "scope-1"
                }
            },
            "operation": operation
        })
    }

    fn write_request(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }

    fn host_value(&self, executable: &Path, expected: Value, timeout_ms: u64) -> Value {
        json!({
            "schema": "gate-runtime-host/v1",
            "executable": executable,
            "cwd": self.dir.path(),
            "timeout_ms": timeout_ms,
            "expected": expected
        })
    }

    fn write_host(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }

    fn command(&self, request: &Path, host: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        command
            .current_dir(self.dir.path())
            .arg("adapter")
            .arg("runtime-probe")
            .arg("--request")
            .arg(request)
            .arg("--trusted-host")
            .arg(host);
        command
    }

    fn run(&self, request: &Path, host: &Path) -> Output {
        self.command(request, host).output().unwrap()
    }

    fn run_with_deadline(&self, request: &Path, host: &Path) -> Output {
        wait_deadline(
            self.command(request, host).spawn().unwrap(),
            Duration::from_secs(5),
        )
    }

    fn dispatched(&self) -> bool {
        self.dispatch.exists()
    }

    fn assert_no_state(&self) {
        assert!(!self.dir.path().join(".harness-gate").exists());
    }
}

fn wait_deadline(mut child: Child, deadline: Duration) -> Output {
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if started.elapsed() > deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!("probe exceeded {deadline:?}: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn assert_rejected(output: &Output, fixture: &Fixture) {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(NOISE), "{stderr}");
    fixture.assert_no_state();
}

fn fact(output: &Output) -> Value {
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(NOISE), "{stderr}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("outcome").is_some(), "{value}");
    assert_ne!(value.get("status"), Some(&json!("PASS")));
    value
}

#[test]
fn pin_and_input_rejections_do_not_dispatch() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("success");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 5_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);

    let raw = serde_json::to_string(&request).unwrap();
    let duplicate = fixture.dir.path().join("duplicate.json");
    fs::write(
        &duplicate,
        raw.replacen(
            r#""operation""#,
            r#""operation":"capabilities","operation""#,
            1,
        ),
    )
    .unwrap();
    assert_rejected(&fixture.run(&duplicate, &host_path), &fixture);
    assert!(!fixture.dispatched());

    let trailing = fixture.dir.path().join("trailing.json");
    fs::write(&trailing, format!("{raw}{{}}")).unwrap();
    assert_rejected(&fixture.run(&trailing, &host_path), &fixture);
    assert!(!fixture.dispatched());

    let mut bad_host = host.clone();
    bad_host["extra"] = json!(1);
    let bad_host = fixture.write_host("bad-host.json", &bad_host);
    assert_rejected(&fixture.run(&req_path, &bad_host), &fixture);
    assert!(!fixture.dispatched());

    let mut moved = host.clone();
    moved["expected"]["binding"]["generation"] = json!("gen-2");
    let moved = fixture.write_host("moved-host.json", &moved);
    assert_rejected(&fixture.run(&req_path, &moved), &fixture);
    assert!(!fixture.dispatched());
}

#[test]
fn remove_is_refused_even_when_trusted() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("success");
    let request = fixture.request_value(json!({"remove": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 5_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);
    assert_rejected(&fixture.run(&req_path, &host_path), &fixture);
    assert!(!fixture.dispatched());
}

#[test]
fn pins_reject_before_dispatch() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("success");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let req_path = fixture.write_request("request.json", &request);

    let mut wrong_digest = request.clone();
    wrong_digest["binding"]["adapter_sha256"] =
        json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let wrong_host = fixture.host_value(&adapter, wrong_digest.clone(), 5_000);
    let wrong_req = fixture.write_request("wrong-digest.json", &wrong_digest);
    let wrong_host = fixture.write_host("wrong-digest-host.json", &wrong_host);
    assert_rejected(&fixture.run(&wrong_req, &wrong_host), &fixture);
    assert!(!fixture.dispatched());

    let changed = fixture.dir.path().join("changed-adapter.py");
    fs::copy(&adapter, &changed).unwrap();
    fs::set_permissions(&changed, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        &changed,
        format!("{}\n# changed\n", fs::read_to_string(&changed).unwrap()),
    )
    .unwrap();
    let changed_host = fixture.host_value(&changed, request.clone(), 5_000);
    let changed_host = fixture.write_host("changed-host.json", &changed_host);
    assert_rejected(&fixture.run(&req_path, &changed_host), &fixture);
    assert!(!fixture.dispatched());

    for (name, executable, cwd, timeout) in [
        (
            "relative-exec",
            PathBuf::from("adapter.py"),
            fixture.path().to_path_buf(),
            5_000,
        ),
        ("relative-cwd", adapter.clone(), PathBuf::from("."), 5_000),
        (
            "zero-timeout",
            adapter.clone(),
            fixture.path().to_path_buf(),
            0,
        ),
        (
            "large-timeout",
            adapter.clone(),
            fixture.path().to_path_buf(),
            30_001,
        ),
    ] {
        let mut host = fixture.host_value(&executable, request.clone(), timeout);
        host["cwd"] = json!(cwd);
        let host = fixture.write_host(&format!("{name}.json"), &host);
        assert_rejected(&fixture.run(&req_path, &host), &fixture);
        assert!(!fixture.dispatched());
    }
}

#[test]
fn successful_probe_returns_facts_without_state() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("success");
    for (name, operation, expected_outcome) in [
        ("inspect", json!({"inspect": {"object_id": ID}}), "present"),
        ("capabilities", json!("capabilities"), "capabilities"),
    ] {
        let request = fixture.request_value(operation, &digest);
        let host = fixture.host_value(&adapter, request.clone(), 5_000);
        let req_path = fixture.write_request(&format!("{name}-request.json"), &request);
        let host_path = fixture.write_host(&format!("{name}-host.json"), &host);
        let value = fact(&fixture.run(&req_path, &host_path));
        assert!(value["outcome"].get(expected_outcome).is_some(), "{value}");
        assert!(fixture.dispatched());
        fixture.assert_no_state();
    }
    let (adapter, digest) = fixture.adapter("unavailable");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 5_000);
    let req_path = fixture.write_request("unavailable-request.json", &request);
    let host_path = fixture.write_host("unavailable-host.json", &host);
    let value = fact(&fixture.run(&req_path, &host_path));
    assert_eq!(value["outcome"], json!("unavailable"));
    assert_ne!(value["outcome"], json!("absent"));
    fixture.assert_no_state();
}

#[test]
fn adapter_environment_and_arguments_are_cleared() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("env");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 5_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);
    let output = fixture
        .command(&req_path, &host_path)
        .env("HOST_SENTINEL", "leak")
        .env("PATH", "/host/path")
        .env("DOCKER_HOST", "unix:///host.sock")
        .output()
        .unwrap();
    fact(&output);
    let environment: Value = serde_json::from_slice(&fs::read(&fixture.env_out).unwrap()).unwrap();
    assert!(environment.get("HOST_SENTINEL").is_none(), "{environment}");
    assert!(environment.get("PATH").is_none(), "{environment}");
    assert!(environment.get("DOCKER_HOST").is_none(), "{environment}");
    let argv: Vec<String> = serde_json::from_slice(&fs::read(&fixture.argv_out).unwrap()).unwrap();
    assert_eq!(argv.len(), 1, "{argv:?}");
}

#[test]
fn transport_failures_return_no_fact() {
    for mode in ["timeout", "nonzero", "oversize", "duplicate", "mismatch"] {
        let fixture = Fixture::new();
        let (adapter, digest) = fixture.adapter(mode);
        let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
        let host = fixture.host_value(&adapter, request.clone(), 1_000);
        let req_path = fixture.write_request("request.json", &request);
        let host_path = fixture.write_host("host.json", &host);
        let output = fixture.run_with_deadline(&req_path, &host_path);
        assert_rejected(&output, &fixture);
        assert!(fixture.dispatched(), "{mode}");
    }
}

#[test]
fn executable_change_during_probe_fails_closed() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("selfmodify");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 5_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);
    let output = fixture.run_with_deadline(&req_path, &host_path);
    assert_rejected(&output, &fixture);
    assert!(fixture.dispatched());
    assert_ne!(sha256(&adapter), digest);
}

#[test]
fn cancellation_during_dispatch_fails_closed() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("cancel");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 30_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);
    let mut child = fixture.command(&req_path, &host_path).spawn().unwrap();
    let started = Instant::now();
    while !fixture.ready.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "adapter did not start"
        );
        if child.try_wait().unwrap().is_some() {
            panic!("adapter exited before cancellation");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let output = wait_deadline(child, Duration::from_secs(5));
    assert_rejected(&output, &fixture);
    assert!(fixture.dispatched());
}

#[test]
fn cwd_parent_symlink_is_rejected() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("success");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let req_path = fixture.write_request("request.json", &request);
    let real = fixture.dir.path().join("real-cwd");
    fs::create_dir(&real).unwrap();
    fs::create_dir(real.join("nested")).unwrap();
    let linked = fixture.dir.path().join("linked-cwd");
    std::os::unix::fs::symlink(&real, &linked).unwrap();
    let mut host = fixture.host_value(&adapter, request.clone(), 5_000);
    host["cwd"] = json!(linked.join("nested"));
    let host_path = fixture.write_host("symlink-cwd-host.json", &host);
    assert_rejected(&fixture.run(&req_path, &host_path), &fixture);
    assert!(!fixture.dispatched());
}

#[test]
fn late_cancel_after_adapter_response_is_not_success() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("latecancel");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 30_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);
    let mut child = fixture.command(&req_path, &host_path).spawn().unwrap();
    let started = Instant::now();
    while !fixture.ready.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "adapter did not respond"
        );
        if child.try_wait().unwrap().is_some() {
            panic!("adapter exited before late cancellation");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let output = wait_deadline(child, Duration::from_secs(5));
    assert_rejected(&output, &fixture);
    assert!(fixture.dispatched());
}

#[test]
fn unix_inputs_require_regular_nonsymlink_files() {
    let fixture = Fixture::new();
    let (adapter, digest) = fixture.adapter("success");
    let request = fixture.request_value(json!({"inspect": {"object_id": ID}}), &digest);
    let host = fixture.host_value(&adapter, request.clone(), 5_000);
    let req_path = fixture.write_request("request.json", &request);
    let host_path = fixture.write_host("host.json", &host);

    let link = fixture.dir.path().join("request-link.json");
    std::os::unix::fs::symlink(&req_path, &link).unwrap();
    assert_rejected(&fixture.run(&link, &host_path), &fixture);
    assert!(!fixture.dispatched());

    let host_link = fixture.dir.path().join("host-link.json");
    std::os::unix::fs::symlink(&host_path, &host_link).unwrap();
    assert_rejected(&fixture.run(&req_path, &host_link), &fixture);
    assert!(!fixture.dispatched());

    let fifo = fixture.dir.path().join("request.fifo");
    let status = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success());
    assert_rejected(&fixture.run_with_deadline(&fifo, &host_path), &fixture);
    assert!(!fixture.dispatched());
}
