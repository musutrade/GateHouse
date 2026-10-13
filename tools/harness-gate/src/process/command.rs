use std::process::{Child, Command, ExitStatus};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
pub(super) fn isolate_process_tree(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        // SAFETY: `pre_exec` runs after fork and only invokes `setsid`, which is
        // async-signal-safe and does not touch Rust synchronization primitives.
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

#[cfg(not(unix))]
pub(super) fn isolate_process_tree(_command: &mut Command) {
    // Windows termination uses `taskkill /T` below to cover descendants. The
    // command itself has no portable process-group primitive to configure here.
}

#[cfg(unix)]
pub(super) fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    terminate_with_signal(child, send_signal, Duration::from_secs(2))
}

#[cfg(unix)]
pub(super) fn terminate_with_signal(
    child: &mut Child,
    signal: fn(i32, libc::c_int) -> std::io::Result<()>,
    grace: Duration,
) -> std::io::Result<ExitStatus> {
    // This component must exclusively own wait/reap for this Child throughout
    // termination. WNOWAIT cannot protect against an external concurrent reaper.
    // ECHILD is deliberately propagated: Child has no public cached-status
    // getter, and try_wait here could instead observe a reused PID.
    observe_exit(child)?;
    let started = Instant::now();
    let mut deadline = started + grace + Duration::from_secs(2);
    let process_group = -(child.id() as i32);
    let mut first_error = signal(process_group, libc::SIGTERM).err();
    if first_error.is_none() {
        // Keep even an exited leader waitable until the final group signal, so
        // its PID/PGID cannot be recycled while TERM-resistant descendants live.
        thread::sleep(grace.min(deadline.saturating_duration_since(Instant::now())));
    } else {
        // An error skips TERM's grace and can only shorten the shared budget.
        deadline = deadline.min(started + Duration::from_secs(2));
    }
    if let Err(error) = signal(process_group, libc::SIGKILL) {
        first_error.get_or_insert(error);
    }
    // No more group signals after this point. In particular, a failed KILL is
    // not followed by an unbounded wait for a leader that may still be alive.
    // TERM, grace, KILL and reap share one deadline; KILL never restarts it.
    // Even an expired budget permits one immediate reap attempt after KILL.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return first_error.map_or(Ok(status), Err),
            Ok(None) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(error) => return Err(first_error.unwrap_or(error)),
        }
    }
    Err(first_error.unwrap_or_else(reap_timeout))
}

#[cfg(unix)]
fn reap_timeout() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "process leader not reaped within cleanup deadline",
    )
}

#[cfg(unix)]
pub(super) fn observe_exit(child: &Child) -> std::io::Result<bool> {
    observe_pid(child.id() as i32)
}

#[cfg(unix)]
pub(super) fn observe_pid(pid: i32) -> std::io::Result<bool> {
    // SAFETY: info is initialized storage, and P_PID identifies the exclusively
    // owned direct child. WNOWAIT observes without releasing its PID identity.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: waitid initialized the siginfo_t discriminant and union.
        Ok(unsafe { info.si_pid() } != 0)
    }
}

#[cfg(unix)]
fn send_signal(process_group: i32, signal: libc::c_int) -> std::io::Result<()> {
    // SAFETY: the process group is created by `isolate_process_tree` and the
    // signal values are fixed constants owned by this module.
    let result = unsafe { libc::kill(process_group, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if error.raw_os_error() == Some(libc::EPERM)
        && darwin_group_is_finished(-process_group, observe_pid, list_process_group)
    {
        return Ok(());
    }
    Err(error)
}

#[cfg(target_os = "macos")]
fn list_process_group(leader: i32, pids: &mut [libc::pid_t; 2]) -> libc::c_int {
    // SAFETY: libproc writes at most this initialized two-PID buffer. The
    // official PROC_PGRP_ONLY selector is 2; the return value counts bytes.
    unsafe {
        libc::proc_listpids(
            2,
            leader as u32,
            pids.as_mut_ptr().cast(),
            std::mem::size_of_val(pids) as libc::c_int,
        )
    }
}

#[cfg(target_os = "macos")]
pub(super) fn darwin_group_is_finished(
    leader: i32,
    observe: fn(i32) -> std::io::Result<bool>,
    list: fn(i32, &mut [libc::pid_t; 2]) -> libc::c_int,
) -> bool {
    // Only an exclusively owned, unreaped zombie can anchor this proof. The
    // kernel enumerates both live and zombie members under proc_list_mlock,
    // also held for group membership changes. Exactly one row in a two-slot
    // buffer proves the list was complete at that point. Zero (including query
    // errors), full/truncated buffers and all other members remain uncertain.
    // No per-PID queries or waiting can turn that uncertainty into success.
    if !matches!(observe(leader), Ok(true)) {
        return false;
    }
    let mut pids = [0; 2];
    let bytes = list(leader, &mut pids);
    bytes == std::mem::size_of::<libc::pid_t>() as libc::c_int && pids[0] == leader
}

#[cfg(not(unix))]
pub(super) fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let tree_status = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .status();
        if !tree_status.is_ok_and(|status| status.success()) {
            let _ = child.kill();
        }
    }
    #[cfg(not(windows))]
    child.kill()?;
    child.wait()
}
