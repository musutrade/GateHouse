use super::command::{isolate_process_tree, terminate};
use super::reader::{
    collect_limited_reader, spawn_limited_reader, DEFAULT_CAPTURE_BYTES, DEFAULT_READER_DEADLINE,
};
use super::signal::cancelled;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CapturedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CaptureLimits {
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) total_bytes: usize,
    pub(crate) reader_deadline: Duration,
}

impl Default for CaptureLimits {
    fn default() -> Self {
        Self {
            stdout_bytes: DEFAULT_CAPTURE_BYTES,
            stderr_bytes: DEFAULT_CAPTURE_BYTES,
            total_bytes: DEFAULT_CAPTURE_BYTES.saturating_mul(2),
            reader_deadline: DEFAULT_READER_DEADLINE,
        }
    }
}

pub fn capture(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<CapturedOutput> {
    capture_command(program, args, cwd, timeout, true, CaptureLimits::default())
}

pub fn capture_cleanup(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<CapturedOutput> {
    capture_command(program, args, cwd, timeout, false, CaptureLimits::default())
}

pub(crate) fn capture_with_limits(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    limits: CaptureLimits,
) -> Result<CapturedOutput> {
    capture_command(program, args, cwd, timeout, true, limits)
}

pub(crate) fn capture_environment(
    program: &str,
    cwd: &Path,
    timeout: Duration,
    environment: &std::collections::BTreeMap<String, String>,
) -> Result<CapturedOutput> {
    capture_command_environment(
        program,
        &[],
        cwd,
        timeout,
        true,
        CaptureLimits::default(),
        Some(environment),
    )
}

fn capture_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    observe_cancel: bool,
    limits: CaptureLimits,
) -> Result<CapturedOutput> {
    capture_command_environment(program, args, cwd, timeout, observe_cancel, limits, None)
}

fn capture_command_environment(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    observe_cancel: bool,
    limits: CaptureLimits,
    environment: Option<&std::collections::BTreeMap<String, String>>,
) -> Result<CapturedOutput> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(environment) = environment {
        command.env_clear().envs(environment).stdin(Stdio::null());
    }
    isolate_process_tree(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("start internal command {program}"))?;
    let stdout = child
        .stdout
        .take()
        .context("internal command stdout was not captured")?;
    let stderr = child
        .stderr
        .take()
        .context("internal command stderr was not captured")?;
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let (stdout_handle, stdout_receiver) =
        spawn_limited_reader(stdout, limits.stdout_bytes, Arc::clone(&stdout_overflow));
    let (stderr_handle, stderr_receiver) =
        spawn_limited_reader(stderr, limits.stderr_bytes, Arc::clone(&stderr_overflow));
    let started = Instant::now();
    let mut wait_round = 0_u32;

    let (status, timed_out, was_cancelled, output_limited) = loop {
        if let Some(status) = child.try_wait()? {
            break (
                status,
                false,
                false,
                stdout_overflow.load(Ordering::Acquire) || stderr_overflow.load(Ordering::Acquire),
            );
        }
        if stdout_overflow.load(Ordering::Acquire) {
            break (terminate(&mut child)?, false, false, true);
        }
        if stderr_overflow.load(Ordering::Acquire) {
            break (terminate(&mut child)?, false, false, true);
        }
        if observe_cancel && cancelled() {
            break (terminate(&mut child)?, false, true, false);
        }
        if started.elapsed() >= timeout {
            break (terminate(&mut child)?, true, false, false);
        }
        // Fast internal commands should not pay the full polling interval.
        // Retain the previous 50 ms cap for long-running commands and never
        // sleep beyond the remaining command deadline.
        std::thread::sleep(capture_poll_delay(
            wait_round,
            timeout.saturating_sub(started.elapsed()),
        ));
        wait_round = wait_round.saturating_add(1);
    };
    let reader_started = Instant::now();
    let stdout = collect_limited_reader(
        stdout_handle,
        stdout_receiver,
        limits.reader_deadline,
        "internal command stdout",
    )
    .map_err(|error| anyhow::anyhow!(error))?;
    let remaining_reader_deadline = limits
        .reader_deadline
        .saturating_sub(reader_started.elapsed());
    let stderr = collect_limited_reader(
        stderr_handle,
        stderr_receiver,
        remaining_reader_deadline,
        "internal command stderr",
    )
    .map_err(|error| anyhow::anyhow!(error))?;

    let combined_bytes = stdout.bytes.len().saturating_add(stderr.bytes.len());
    if output_limited || stdout.truncated || stderr.truncated || combined_bytes > limits.total_bytes
    {
        let (stream, captured, limit) = if stdout.truncated {
            ("stdout", stdout.bytes.len(), limits.stdout_bytes)
        } else if stderr.truncated {
            ("stderr", stderr.bytes.len(), limits.stderr_bytes)
        } else {
            ("combined", combined_bytes, limits.total_bytes)
        };
        bail!(
            "internal command {program} {stream} output exceeded {limit} bytes (captured {captured} bytes; truncated=true)"
        );
    }

    if was_cancelled {
        bail!("internal command {program} was cancelled");
    }
    if timed_out {
        bail!(
            "internal command {program} timed out after {} ms",
            timeout.as_millis()
        );
    }
    Ok(CapturedOutput {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    })
}

fn capture_poll_delay(round: u32, remaining: Duration) -> Duration {
    Duration::from_millis((5_u64 << round.min(4)).min(50)).min(remaining)
}

#[cfg(test)]
mod polling_tests {
    use super::*;

    #[test]
    fn short_initial_wait_backs_off_without_exceeding_previous_cap_or_deadline() {
        let remaining = Duration::from_secs(1);
        let delays: Vec<_> = (0..7)
            .map(|round| capture_poll_delay(round, remaining).as_millis())
            .collect();
        assert_eq!(delays, [5, 10, 20, 40, 50, 50, 50]);
        assert_eq!(
            capture_poll_delay(u32::MAX, remaining),
            Duration::from_millis(50)
        );
        for round in [0, 1, 3, 4, u32::MAX] {
            assert_eq!(
                capture_poll_delay(round, Duration::from_millis(2)),
                Duration::from_millis(2)
            );
            assert_eq!(capture_poll_delay(round, Duration::ZERO), Duration::ZERO);
        }
    }
}
