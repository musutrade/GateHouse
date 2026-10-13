use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_signal(_signal: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

#[cfg(windows)]
unsafe extern "system" fn handle_console_signal(signal: u32) -> i32 {
    // CTRL_C_EVENT, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT,
    // and CTRL_SHUTDOWN_EVENT all require the same bounded cleanup path.
    if (0..=2).contains(&signal) || (5..=6).contains(&signal) {
        CANCELLED.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}

pub fn install_signal_handlers() {
    install_platform_handlers();
}

#[cfg(unix)]
fn install_platform_handlers() {
    for (signal, name) in [(libc::SIGINT, "SIGINT"), (libc::SIGTERM, "SIGTERM")] {
        if let Err(error) = install_unix_handler(signal) {
            eprintln!(
                "warning: could not install the {name} handler ({error}); \
                 {name} will terminate without bounded cleanup"
            );
        }
    }
}

#[cfg(windows)]
fn install_platform_handlers() {
    type HandlerRoutine = Option<unsafe extern "system" fn(u32) -> i32>;
    #[link(name = "Kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(handler: HandlerRoutine, add: i32) -> i32;
    }

    // SAFETY: the callback has the ABI and lifetime required by the Windows API,
    // and it only updates the process cancellation flag.
    let _ = unsafe { SetConsoleCtrlHandler(Some(handle_console_signal), 1) };
}

#[cfg(not(any(unix, windows)))]
fn install_platform_handlers() {}

/// Install a persistent handler with explicit `sigaction` semantics: no
/// `SA_RESETHAND`, so a repeated signal still reaches the cancellation path,
/// and `SA_RESTART`, matching the BSD semantics `signal` provided (#317).
#[cfg(unix)]
fn install_unix_handler(signal: libc::c_int) -> std::io::Result<()> {
    // SAFETY: a zeroed sigaction is a valid empty action; the handler only
    // performs an atomic store, which is async-signal-safe.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = handle_signal as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        if libc::sigemptyset(&mut action.sa_mask) != 0
            || libc::sigaction(signal, &action, std::ptr::null_mut()) != 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn repeated_termination_signals_keep_reaching_the_cancellation_path() {
        install_signal_handlers();
        for signal in [libc::SIGTERM, libc::SIGTERM, libc::SIGINT, libc::SIGINT] {
            CANCELLED.store(false, Ordering::Relaxed);
            // SAFETY: raising a signal whose handler only stores an atomic.
            assert_eq!(unsafe { libc::raise(signal) }, 0);
            assert!(cancelled(), "signal {signal} must stay handled");
        }
        CANCELLED.store(false, Ordering::Relaxed);
        let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
        // SAFETY: query-only sigaction with a valid output buffer.
        assert_eq!(
            unsafe { libc::sigaction(libc::SIGTERM, std::ptr::null(), &mut current) },
            0
        );
        assert_eq!(current.sa_flags & libc::SA_RESETHAND, 0);
        assert_ne!(current.sa_flags & libc::SA_RESTART, 0);
    }
}
