#[path = "../src/process/command.rs"]
mod command;

#[cfg(unix)]
mod unix {
    use super::command;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    // Only the living fixture member signals its own group during emergency
    // cleanup. The host guard never signals a saved PGID after leader reap.
    const WORKER: &str = r#"
import os, signal, sys, time
from pathlib import Path
root, mode = Path(sys.argv[1]), sys.argv[2]
group = os.getpgrp()
assert group == os.getpid(), 'fixture must be its isolated group leader'
signal.signal(signal.SIGTERM, signal.SIG_IGN if mode == 'ignore-leader' else lambda *_: os._exit(23))
if mode == 'extra-zombie':
    keeper = os.fork()
    if keeper == 0:
        os.setpgid(0, 0)
        zombie = os.fork()
        if zombie == 0:
            os.setpgid(0, group)
            os._exit(9)
        deadline = time.monotonic() + 30
        while os.waitid(os.P_PID, zombie, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
            assert time.monotonic() < deadline
            time.sleep(.01)
        (root / 'ready.tmp').write_text(f'{group} {zombie} {group}')
        os.replace(root / 'ready.tmp', root / 'ready')
        while not (root / 'cleanup').exists() and time.monotonic() < deadline:
            time.sleep(.01)
        os.waitpid(zombie, 0)
        os._exit(0)
    while not (root / 'ready').exists():
        time.sleep(.01)
    os._exit(7)
pid = os.fork()
if pid == 0:
    signal.signal(signal.SIGTERM, signal.SIG_DFL if mode == 'normal-group' else signal.SIG_IGN)
    assert os.getpgrp() == group and os.getpid() != group
    (root / 'ready.tmp').write_text(f'{group} {os.getpid()} {os.getpgrp()}')
    os.replace(root / 'ready.tmp', root / 'ready')
    deadline = time.monotonic() + 30
    while True:
        if (root / 'cleanup').exists() or time.monotonic() > deadline:
            # Still a live member: this group cannot have been recycled.
            assert os.getpgrp() == group
            os.killpg(os.getpgrp(), signal.SIGKILL)
        (root / 'heartbeat').write_text(str(time.monotonic_ns()))
        time.sleep(.02)
if mode == 'exit-leader':
    while not (root / 'ready').exists():
        time.sleep(.01)
    os._exit(7)
while True:
    time.sleep(.02)
"#;

    struct Fixture {
        root: PathBuf,
        leader: Option<Child>,
    }

    impl Fixture {
        fn new() -> Self {
            let parent = std::env::var_os("GH285_PROCESS_EVIDENCE")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gh285-process")
                });
            fs::create_dir_all(&parent).unwrap();
            let root = tempfile::Builder::new()
                .prefix("process-")
                .tempdir_in(parent)
                .unwrap()
                .keep();
            fs::write(root.join("worker.py"), WORKER).unwrap();
            Self { root, leader: None }
        }

        fn start(&mut self, mode: &str) {
            let mut cmd = Command::new("python3");
            cmd.arg(self.root.join("worker.py"))
                .arg(&self.root)
                .arg(mode);
            command::isolate_process_tree(&mut cmd);
            self.leader = Some(cmd.spawn().unwrap());
            assert_eq!(self.ready()[0], self.leader.as_ref().unwrap().id() as i32);
        }

        fn ready(&self) -> Vec<i32> {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Ok(text) = fs::read_to_string(self.root.join("ready")) {
                    let ids: Vec<i32> = text
                        .split_whitespace()
                        .map(|id| id.parse().unwrap())
                        .collect();
                    if ids.len() == 3 {
                        assert_eq!(ids[0], ids[2], "descendant must share leader PGID");
                        return ids;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "ready timeout: {}",
                    self.root.display()
                );
                thread::sleep(Duration::from_millis(10));
            }
        }

        fn assert_stopped(&self) {
            let pid = self.ready()[1];
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let output = Command::new("ps")
                    .args(["-o", "pid=,stat=", "-p", &pid.to_string()])
                    .output()
                    .unwrap();
                let state = std::str::from_utf8(&output.stdout).unwrap();
                fs::write(
                    self.root.join("descendant-state.txt"),
                    format!(
                        "status={}\nstdout={state}\nstderr={}\n",
                        output.status,
                        String::from_utf8_lossy(&output.stderr)
                    ),
                )
                .unwrap();
                assert!(output.stderr.is_empty(), "ps query error: {output:?}");
                if !output.status.success() {
                    // ps commonly returns 1 when no selected process exists.
                    // Empty stdout alone cannot distinguish that from failure:
                    // require an independent kernel ESRCH probe as well.
                    assert_eq!(output.status.code(), Some(1), "ps query failed: {output:?}");
                    assert!(
                        state.trim().is_empty(),
                        "unexpected ps failure output: {state}"
                    );
                    let result = unsafe { libc::kill(pid, 0) };
                    let error = io::Error::last_os_error();
                    assert!(
                        result == -1 && error.raw_os_error() == Some(libc::ESRCH),
                        "ps did not establish absence: pid={pid}, probe={result}, error={error}"
                    );
                    return;
                } else {
                    let fields: Vec<_> = state.split_whitespace().collect();
                    assert_eq!(fields.len(), 2, "malformed ps result: {state}");
                    assert_eq!(fields[0].parse::<i32>().unwrap(), pid);
                    // BSD/macOS state strings are a run state surrounded by
                    // flags, so strip both sides before reading the state.
                    let run = fields[1]
                        .trim_end_matches(|c: char| !c.is_ascii_alphabetic())
                        .trim_start_matches(|c: char| !c.is_ascii_alphabetic());
                    assert!(!run.is_empty(), "malformed process state: {state}");
                    // Zombies are stopped, though their new parent owns reap.
                    // A process already trying to exit has been reparented as
                    // well: it will never run its fixture loop again.
                    if run.starts_with(['Z', 'E']) {
                        return;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "live descendant {pid}: {state}; fixture {}",
                    self.root.display()
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::write(self.root.join("cleanup"), b"release");
            if let Some(child) = &mut self.leader {
                // No group signals here; keep the direct Child identity until
                // its final direct kill and bounded reap attempt.
                if matches!(child.try_wait(), Ok(None)) {
                    let _ = child.kill();
                    let deadline = Instant::now() + Duration::from_secs(3);
                    while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
    }

    #[test]
    fn leader_term_exit_still_kills_same_group_term_resistant_descendant() {
        let mut fixture = Fixture::new();
        fixture.start("term-leader");
        let started = Instant::now();
        let status = command::terminate(fixture.leader.as_mut().unwrap()).unwrap();
        assert_eq!(status.code(), Some(23));
        assert!(started.elapsed() < Duration::from_secs(5));
        fixture.assert_stopped(); // Before guard/watchdog cleanup, never after.
        assert!(fixture
            .leader
            .as_mut()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_some());
    }

    #[test]
    fn normal_group_and_term_ignoring_leader_are_reaped() {
        use std::os::unix::process::ExitStatusExt;
        for mode in ["normal-group", "ignore-leader"] {
            let mut fixture = Fixture::new();
            fixture.start(mode);
            let status = command::terminate(fixture.leader.as_mut().unwrap()).unwrap();
            if mode == "ignore-leader" {
                assert_eq!(status.signal(), Some(libc::SIGKILL));
            } else {
                assert_eq!(status.code(), Some(23));
            }
            fixture.assert_stopped();
        }
    }

    fn record_no_signal(_: i32, _: libc::c_int) -> io::Result<()> {
        panic!("ECHILD path must never signal any group")
    }

    #[test]
    fn waitable_exit_is_reaped_but_cached_or_external_reap_is_echild_without_signal() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 7"]);
        command::isolate_process_tree(&mut cmd);
        let mut child = cmd.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !command::observe_exit(&child).unwrap() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(command::terminate(&mut child).unwrap().code(), Some(7));
        assert_eq!(
            command::terminate_with_signal(&mut child, record_no_signal, Duration::ZERO)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ECHILD)
        );

        let mut child = Command::new("sh").args(["-c", "exit 9"]).spawn().unwrap();
        let mut status = 0;
        // External reap is sequential here; concurrent reapers are outside the
        // production ownership contract and are not claimed safe.
        assert_eq!(
            unsafe { libc::waitpid(child.id() as i32, &mut status, 0) },
            child.id() as i32
        );
        assert_eq!(
            command::terminate_with_signal(&mut child, record_no_signal, Duration::ZERO)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ECHILD)
        );
    }

    fn term_error_then_real_kill(group: i32, signal: libc::c_int) -> io::Result<()> {
        if signal == libc::SIGTERM {
            Err(io::Error::from_raw_os_error(libc::EACCES))
        } else if unsafe { libc::kill(group, signal) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn both_signals_fail(_: i32, signal: libc::c_int) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(if signal == libc::SIGTERM {
            libc::EACCES
        } else {
            libc::EPERM
        }))
    }

    fn only_kill_fails(group: i32, signal: libc::c_int) -> io::Result<()> {
        if signal == libc::SIGKILL {
            Err(io::Error::from_raw_os_error(libc::EPERM))
        } else if unsafe { libc::kill(group, signal) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(target_os = "macos")]
    fn list_group(pid: i32, pids: &mut [libc::pid_t; 2]) -> libc::c_int {
        unsafe {
            libc::proc_listpids(
                2,
                pid as u32,
                pids.as_mut_ptr().cast(),
                std::mem::size_of_val(pids) as libc::c_int,
            )
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn darwin_guard_requires_a_complete_unique_waitable_leader() {
        fn exited(_: i32) -> io::Result<bool> {
            Ok(true)
        }
        fn live(_: i32) -> io::Result<bool> {
            Ok(false)
        }
        fn reaped(_: i32) -> io::Result<bool> {
            Err(io::Error::from_raw_os_error(libc::ECHILD))
        }
        fn forbidden(_: i32, _: &mut [libc::pid_t; 2]) -> i32 {
            panic!("a live or reaped leader must not reach group enumeration")
        }
        fn unknown(_: i32, _: &mut [libc::pid_t; 2]) -> i32 {
            0
        }
        fn negative(_: i32, _: &mut [libc::pid_t; 2]) -> i32 {
            -1
        }
        fn short(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 {
            pids[0] = pid;
            2
        }
        fn nonintegral(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 {
            pids[0] = pid;
            7
        }
        fn full(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 {
            *pids = [pid, pid + 1];
            8
        }
        fn wrong(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 {
            pids[0] = pid + 1;
            4
        }
        fn denied(_: i32) -> io::Result<bool> {
            Err(io::Error::from_raw_os_error(libc::EPERM))
        }
        fn injected(_: i32, _: i32) -> io::Result<()> {
            Err(io::Error::from_raw_os_error(libc::EPERM))
        }
        for query in [unknown, negative, short, nonintegral, full, wrong] {
            assert!(!command::darwin_group_is_finished(42, exited, query));
        }
        for observe in [live, reaped, denied] {
            assert!(!command::darwin_group_is_finished(42, observe, forbidden));
        }
        for mode in ["ignore-leader", "exit-leader", "extra-zombie"] {
            let mut fixture = Fixture::new();
            fixture.start(mode);
            let child = fixture.leader.as_ref().unwrap();
            let pid = child.id() as i32;
            if mode != "ignore-leader" {
                let deadline = Instant::now() + Duration::from_secs(5);
                while !command::observe_exit(child).unwrap() {
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(10));
                }
                let mut pids = [0; 2];
                assert_eq!(list_group(pid, &mut pids), 8);
                assert!(pids.contains(&pid) && pids.contains(&fixture.ready()[1]));
                fs::write(
                    fixture.root.join("guard-members.txt"),
                    format!("{pids:?}\n"),
                )
                .unwrap();
            }
            assert!(!command::darwin_group_is_finished(
                pid,
                command::observe_pid,
                list_group
            ));
            if mode == "exit-leader" {
                assert_eq!(
                    command::terminate(fixture.leader.as_mut().unwrap())
                        .unwrap()
                        .code(),
                    Some(7)
                );
                fixture.assert_stopped();
            }
        }
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 7"]);
        command::isolate_process_tree(&mut cmd);
        let mut child = cmd.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !command::observe_exit(&child).unwrap() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        assert!(command::darwin_group_is_finished(
            child.id() as i32,
            command::observe_pid,
            list_group
        ));
        // An injected EPERM still propagates even for this proven zombie.
        assert_eq!(
            command::terminate_with_signal(&mut child, injected, Duration::ZERO)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EPERM)
        );
        assert!(!command::darwin_group_is_finished(
            child.id() as i32,
            command::observe_pid,
            forbidden
        ));
    }

    #[test]
    fn first_signal_error_survives_cleanup_and_failed_kill_is_bounded() {
        for kill_succeeds in [true, false] {
            let signal = if kill_succeeds {
                term_error_then_real_kill
            } else {
                both_signals_fail
            };
            let mut fixture = Fixture::new();
            fixture.start("ignore-leader");
            let started = Instant::now();
            let error = command::terminate_with_signal(
                fixture.leader.as_mut().unwrap(),
                signal,
                Duration::ZERO,
            )
            .unwrap_err();
            assert_eq!(error.raw_os_error(), Some(libc::EACCES));
            assert!(started.elapsed() < Duration::from_secs(4));
            if kill_succeeds {
                fixture.assert_stopped();
                assert!(fixture
                    .leader
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_some());
            } else {
                assert!(
                    fixture
                        .leader
                        .as_mut()
                        .unwrap()
                        .try_wait()
                        .unwrap()
                        .is_none(),
                    "failed KILL must not be reported as reaped"
                );
            }
        }
        let mut fixture = Fixture::new();
        fixture.start("ignore-leader");
        let started = Instant::now();
        assert_eq!(
            command::terminate_with_signal(
                fixture.leader.as_mut().unwrap(),
                only_kill_fails,
                Duration::ZERO
            )
            .unwrap_err()
            .raw_os_error(),
            Some(libc::EPERM)
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(fixture
            .leader
            .as_mut()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_none());
    }

    #[test]
    fn signal_callbacks_do_not_restart_the_shared_cleanup_deadline() {
        fn delayed_failure(_: i32, signal: i32) -> io::Result<()> {
            if signal == libc::SIGKILL {
                thread::sleep(Duration::from_millis(1500));
            }
            Err(io::Error::from_raw_os_error(if signal == libc::SIGTERM {
                libc::EACCES
            } else {
                libc::EPERM
            }))
        }
        let mut fixture = Fixture::new();
        fixture.start("ignore-leader");
        let started = Instant::now();
        let error = command::terminate_with_signal(
            fixture.leader.as_mut().unwrap(),
            delayed_failure,
            Duration::from_secs(2),
        )
        .unwrap_err();
        let elapsed = started.elapsed();
        fs::write(
            fixture.root.join("deadline.txt"),
            format!("elapsed={elapsed:?}\nerror={error}\n"),
        )
        .unwrap();
        assert_eq!(error.raw_os_error(), Some(libc::EACCES));
        assert!(elapsed >= Duration::from_millis(1500));
        assert!(elapsed < Duration::from_secs(3));
        assert!(fixture
            .leader
            .as_mut()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_none());
    }

    #[test]
    fn independent_cli_timeout_and_cancellation_stop_term_resistant_descendant() {
        for cancel in [false, true] {
            let mut fixture = Fixture::new();
            let root = &fixture.root;
            let binary = env!("CARGO_BIN_EXE_harness-gate");
            assert!(Command::new(binary)
                .arg("--project-root")
                .arg(root)
                .args(["init", "--preset", "generic"])
                .output()
                .unwrap()
                .status
                .success());
            assert!(Command::new("git")
                .arg("init")
                .current_dir(root)
                .output()
                .unwrap()
                .status
                .success());
            let timeout = if cancel { 20 } else { 2 };
            let flow = format!(
                r#"
version = 2
[project]
name = "termination"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
rules = [{{ patterns = ["**"], components = ["project"] }}]
[[steps]]
id = "project.process"
label = "process"
component = "project"
profiles = ["full"]
program = "python3"
args = ["worker.py", ".", "term-leader"]
cwd = "{{root}}"
log = "process.log"
timeout_secs = {timeout}
"#
            );
            fs::write(root.join(".harness-gate/flow.toml"), flow).unwrap();
            let stdout = fs::File::create(root.join("cli.stdout")).unwrap();
            let stderr = fs::File::create(root.join("cli.stderr")).unwrap();
            fixture.leader = Some(
                Command::new(binary)
                    .arg("--project-root")
                    .arg(root)
                    .args(["verify", "--all"])
                    .stdout(Stdio::from(stdout))
                    .stderr(Stdio::from(stderr))
                    .spawn()
                    .unwrap(),
            );
            // Here the owned Child is the CLI, not the worker group leader.
            fixture.ready();
            if cancel {
                assert_eq!(
                    unsafe {
                        libc::kill(fixture.leader.as_ref().unwrap().id() as i32, libc::SIGTERM)
                    },
                    0
                );
            }
            let deadline = Instant::now() + Duration::from_secs(12);
            let status = loop {
                if let Some(status) = fixture.leader.as_mut().unwrap().try_wait().unwrap() {
                    break status;
                }
                assert!(
                    Instant::now() < deadline,
                    "CLI termination exceeded deadline: {}",
                    root.display()
                );
                thread::sleep(Duration::from_millis(10));
            };
            assert_eq!(status.code(), Some(1));
            fs::write(root.join("cli.status"), "1\n").unwrap();
            fixture.assert_stopped();
            let report: serde_json::Value = serde_json::from_slice(
                &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(report["passed"], false);
            assert!(report["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|step| step[if cancel { "cancelled" } else { "timed_out" }] == true));
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_taskkill_termination_preserves_direct_child_behavior() {
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/C", "ping -n 30 127.0.0.1 > nul"]);
    command::isolate_process_tree(&mut cmd);
    let mut child = cmd.spawn().unwrap();
    assert!(!command::terminate(&mut child).unwrap().success());
    assert!(child.try_wait().unwrap().is_some());
}
