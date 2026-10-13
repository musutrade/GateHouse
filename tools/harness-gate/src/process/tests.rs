use super::capture::{capture, capture_with_limits, CaptureLimits};
use super::Task;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The replay ledger deliberately rejects group/other-writable ancestors
/// (#316). Pin the unit-test process umask before any test runs so fixture
/// directories do not inherit a user-private-group default (0002) and make
/// local results diverge from CI. Child CLI processes inherit the same mask.
/// The constructor is linked into the whole test binary, not only this module.
#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
#[used]
#[link_section = ".init_array"]
static PIN_TEST_UMASK: extern "C" fn() = pin_test_umask;

#[cfg(target_vendor = "apple")]
#[used]
#[link_section = "__DATA,__mod_init_func"]
static PIN_TEST_UMASK: extern "C" fn() = pin_test_umask;

#[cfg(unix)]
extern "C" fn pin_test_umask() {
    // SAFETY: umask only replaces the process file-mode creation mask.
    unsafe {
        libc::umask(0o022);
    }
}

#[cfg(unix)]
#[test]
fn timeout_terminates_the_task() {
    let log = std::env::temp_dir().join(format!("harness-gate-timeout-{}.log", std::process::id()));
    let result = Task::new("timeout fixture", "sleep", Path::new("."), log.clone())
        .args(["5"])
        .timeout(0)
        .run()
        .expect("run timeout fixture");

    assert!(!result.passed);
    assert!(result.timed_out);
    assert!(log.is_file());
    let _ = fs::remove_file(log);
}

#[cfg(unix)]
#[test]
fn task_can_remove_an_inherited_environment_variable() {
    let log = std::env::temp_dir().join(format!("harness-gate-env-{}.log", std::process::id()));
    let result = Task::new("environment fixture", "env", Path::new("."), log.clone())
        .env("HARNESS_GATE_REMOVE_FIXTURE", "must-not-leak")
        .env_remove("HARNESS_GATE_REMOVE_FIXTURE")
        .run()
        .expect("run environment fixture");

    assert!(result.passed);
    let output = fs::read_to_string(&log).expect("read environment log");
    assert!(!output.contains("HARNESS_GATE_REMOVE_FIXTURE"));
    let _ = fs::remove_file(log);
}

#[cfg(target_os = "linux")]
#[test]
fn task_runs_in_an_isolated_session() {
    let log = std::env::temp_dir().join(format!("harness-gate-session-{}.log", std::process::id()));
    let result = Task::new("session fixture", "sh", Path::new("."), log.clone())
        .args(["-c", "ps -o sid= -p $$"])
        .run()
        .expect("run session fixture");

    assert!(result.passed);
    let child_session = fs::read_to_string(&log)
        .expect("read session log")
        .trim()
        .parse::<i32>()
        .expect("parse child session id");
    let parent_session = unsafe { libc::getsid(0) };
    assert_ne!(child_session, parent_session);
    let _ = fs::remove_file(log);
}

#[cfg(unix)]
#[test]
fn captured_command_has_a_hard_timeout() {
    let args = vec!["-c".to_string(), "sleep 5".to_string()];
    let error = capture("sh", &args, Path::new("/tmp"), Duration::from_millis(100))
        .expect_err("capture must time out");

    assert!(error.to_string().contains("timed out"));
}

#[cfg(unix)]
#[test]
fn captured_command_enforces_output_budget() {
    let args = vec!["-c".to_string(), "printf 1234567890".to_string()];
    let error = capture_with_limits(
        "sh",
        &args,
        Path::new("/tmp"),
        Duration::from_secs(1),
        CaptureLimits {
            stdout_bytes: 4,
            stderr_bytes: 4,
            total_bytes: 8,
            reader_deadline: Duration::from_secs(1),
        },
    )
    .expect_err("capture must enforce stdout budget");
    assert!(error.to_string().contains("stdout output exceeded 4 bytes"));
    assert!(error.to_string().contains("truncated=true"));
}

/// The test binary doubles as a child-process fixture. Keeping the fixture in
/// Rust avoids relying on `sleep`, `sh`, or other platform-specific utilities.
#[test]
fn process_tree_child_fixture() {
    if env::var_os("HARNESS_GATE_PROCESS_TREE_MARKER").is_none() {
        return;
    }
    let marker =
        PathBuf::from(env::var_os("HARNESS_GATE_PROCESS_TREE_MARKER").expect("marker path"));
    fs::write(marker.with_extension("ready"), b"ready").expect("write child readiness");
    // Respond only to a probe sent after cleanup returns. A fixed sleep could
    // write before Windows taskkill finishes and falsely report a leaked child.
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if marker.with_extension("probe").exists() {
            fs::write(marker, b"descendant survived").expect("write process-tree marker");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn process_tree_probe_detects_a_live_child() {
    let root = tempfile::tempdir().expect("process probe directory");
    let marker = root.path().join("child.marker");
    fs::write(marker.with_extension("probe"), b"probe").expect("send liveness probe");
    let status = Command::new(env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "process::tests::process_tree_child_fixture",
            "--nocapture",
        ])
        .env("HARNESS_GATE_PROCESS_TREE_MARKER", &marker)
        .status()
        .expect("start live child fixture");
    assert!(status.success());
    assert!(marker.with_extension("ready").exists());
    assert!(marker.exists(), "probe must detect a live child");
}

#[test]
fn process_tree_parent_fixture() {
    if env::var_os("HARNESS_GATE_PROCESS_TREE_MARKER").is_none() {
        return;
    }
    let executable = env::current_exe().expect("test executable");
    let status = Command::new(executable)
        .args([
            "--exact",
            "process::tests::process_tree_child_fixture",
            "--nocapture",
        ])
        .envs(env::vars_os().filter(|(key, _)| key != "HARNESS_GATE_PROCESS_TREE_PARENT"))
        .status()
        .expect("start descendant fixture");
    // The parent remains alive until the task timeout kills the process tree.
    // If the descendant exits early, keep the parent alive long enough for the
    // task runner to exercise its termination path anyway.
    let _ = status;
    std::thread::sleep(Duration::from_secs(10));
}

#[test]
fn timeout_terminates_process_tree_without_a_descendant_leak() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let suffix = format!("{}-{timestamp}", std::process::id());
    let log = env::temp_dir().join(format!("harness-gate-process-tree-{suffix}.log"));
    let marker = env::temp_dir().join(format!("harness-gate-process-tree-{suffix}.marker"));
    let executable = env::current_exe().expect("test executable");
    let result = Task::new(
        "process tree fixture",
        executable,
        Path::new("."),
        log.clone(),
    )
    .args([
        "--exact",
        "process::tests::process_tree_parent_fixture",
        "--nocapture",
    ])
    .env("HARNESS_GATE_PROCESS_TREE_MARKER", &marker)
    .env("HARNESS_GATE_PROCESS_TREE_PARENT", "1")
    .timeout(1)
    .run()
    .expect("run process tree fixture");

    assert!(result.timed_out, "fixture must reach the timeout boundary");
    assert!(!result.passed);
    assert!(
        marker.with_extension("ready").exists(),
        "the descendant must have started before cleanup"
    );
    // A surviving child can acknowledge this probe only after Task::run has
    // returned. Time spent inside platform cleanup cannot produce a false leak.
    fs::write(marker.with_extension("probe"), b"probe").expect("send liveness probe");
    std::thread::sleep(Duration::from_secs(3));
    assert!(!marker.exists(), "timed-out task left a descendant process");
    let _ = fs::remove_file(log);
    let _ = fs::remove_file(marker.with_extension("ready"));
    let _ = fs::remove_file(marker.with_extension("probe"));
    let _ = fs::remove_file(marker);
}

#[test]
fn abnormal_or_cancelled_worker_removes_isolation_state() {
    let root = tempfile::tempdir().expect("temporary isolation root");
    let state = root.path().join("worker.json");
    fs::write(&state, b"{\"worker\":true}").expect("seed isolation state");
    let log = root.path().join("worker.log");
    let marker = root.path().join("worker.marker");
    let executable = env::current_exe().expect("test executable");
    let result = Task::new("abnormal worker", executable, Path::new("."), log)
        .args(["--exact", "process::tests::process_tree_child_fixture"])
        .env("HARNESS_GATE_PROCESS_TREE_MARKER", &marker)
        .timeout(0)
        .isolation_state(state.clone())
        .run()
        .expect("run worker fixture");
    assert!(!result.passed);
    assert!(result.timed_out || result.cancelled);
    assert!(
        !state.exists(),
        "terminal worker state must not be reusable"
    );
    assert!(state.with_extension("terminal.json").is_file());
    fs::write(marker.with_extension("probe"), b"probe").expect("send liveness probe");
    std::thread::sleep(Duration::from_secs(3));
    assert!(
        !marker.exists(),
        "abnormal worker fixture outlived its task"
    );
}
