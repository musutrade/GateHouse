//! Linux lifecycle dispatch boundary. No CLI fallback, discovery or engine policy.
//! Only a durable journal token plus a held installation can reach process spawn.
use super::{fd_inputs::SealedInputs, raw_adapter, HostPolicy};
use gatehouse_runtime_protocol::lifecycle::{
    installation::VerifiedInstallation,
    journal::Dispatch,
    wire::{self, Request, Response},
};
use std::{
    fs::File,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::FileExt,
    },
    process::Command,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, &'static str>;

/// Consumes the token even on pre-spawn failure: uncertain work is never retried.
/// Service orchestration must reconcile the retained journal explicitly.
#[allow(dead_code)]
pub(crate) fn dispatch(
    token: Dispatch<'_>,
    installation: &VerifiedInstallation,
    timeout_ms: u64,
    cancelled: impl Fn() -> bool,
) -> Result<Response> {
    let result = run(token.request(), installation, timeout_ms, &cancelled);
    match result {
        Ok((response, bytes)) => {
            token
                .observe(Some(&bytes))
                .map_err(|_| "lifecycle journal commit failed")?;
            Ok(response)
        }
        Err(error) => {
            token
                .observe(None)
                .map_err(|_| "lifecycle unknown result commit failed")?;
            Err(error)
        }
    }
}

fn run(
    request: &Request,
    installation: &VerifiedInstallation,
    timeout_ms: u64,
    cancelled: &impl Fn() -> bool,
) -> Result<(Response, Vec<u8>)> {
    let started = Instant::now();
    if !(1..=30_000).contains(&timeout_ms) || cancelled() {
        return Err("lifecycle invalid deadline or cancelled");
    }
    installation
        .validate_request(request)
        .map_err(|_| "lifecycle installation mismatch")?;
    let bytes = request
        .canonical_bytes()
        .map_err(|_| "lifecycle request invalid")?;
    let inputs = SealedInputs::new(
        installation
            .config_bytes()
            .map_err(|_| "lifecycle config changed")?,
        &bytes,
    )
    .map_err(|_| "lifecycle sealed inputs failed")?;
    let executable = held_executable(
        installation
            .executable()
            .map_err(|_| "lifecycle executable changed")?,
    )?;
    // The descriptor stays alive across spawn. CLOEXEC closes it only after the
    // kernel has resolved /proc/self/fd; fd >=5 cannot collide with fd3/fd4.
    // Only ELF is accepted: interpreter script paths cannot preserve this model.
    let mut command = Command::new(format!("/proc/self/fd/{}", executable.as_raw_fd()));
    command.env_clear().current_dir("/");
    let policy = HostPolicy {
        max_stdout_bytes: 65_536,
        max_stderr_bytes: 65_536,
        max_output_bytes: 65_536,
        reader_deadline: Duration::from_secs(2),
        ..HostPolicy::default()
    };
    installation
        .revalidate()
        .map_err(|_| "lifecycle installation changed")?;
    let output = raw_adapter::run_with_exact_inputs(
        command, inputs, &policy, timeout_ms, bytes, started, cancelled,
    )
    .map_err(|_| "lifecycle transport failed")?;
    raw_adapter::validate_process_output(&output, &policy)
        .map_err(|_| "lifecycle output rejected")?;
    if !output.status.success()
        || output.timed_out
        || output.was_cancelled
        || output.request_write_error.is_some()
        || !output.stderr.bytes.is_empty()
    {
        return Err("lifecycle transport incomplete");
    }
    let response = wire::parse_response(&output.stdout.bytes, request)
        .map_err(|_| "lifecycle response invalid")?;
    installation
        .revalidate()
        .map_err(|_| "lifecycle installation changed")?;
    if cancelled() || started.elapsed() >= Duration::from_millis(timeout_ms) {
        return Err("lifecycle final boundary cancelled or expired");
    }
    Ok((response, output.stdout.bytes))
}

fn held_executable(file: &File) -> Result<File> {
    let mut magic = [0u8; 4];
    file.read_exact_at(&mut magic, 0)
        .map_err(|_| "lifecycle executable unreadable")?;
    if magic != *b"\x7fELF" {
        return Err("lifecycle executable must be ELF");
    }
    // SAFETY: duplicate an existing descriptor; take ownership only on success.
    let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 5) };
    if fd < 0 {
        return Err("lifecycle executable duplicate failed");
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(test)]
mod tests;
