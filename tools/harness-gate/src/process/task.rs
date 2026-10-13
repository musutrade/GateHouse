use super::command::{isolate_process_tree, terminate};
use super::isolation;
use super::reader::{
    collect_limited_reader, spawn_limited_reader, LimitedOutput, DEFAULT_CAPTURE_BYTES,
    DEFAULT_READER_DEADLINE,
};
use super::signal::cancelled;
use crate::config::{RunnerResultFormat, TestIsolation};
use crate::failure::{FailureCode, RetryClass};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Task {
    pub label: String,
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    env: Vec<(OsString, OsString)>,
    env_remove: Vec<OsString>,
    timeout: Duration,
    log: PathBuf,
    runner: Option<RunnerExecution>,
    isolation_state: Option<PathBuf>,
}

/// Records the declared runner contract and the effective inputs used for a task.
/// Environment values are limited to runner-owned declarations; service values
/// are intentionally not copied into this report field.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerExecution {
    pub version: u32,
    pub kind: String,
    pub program: String,
    pub effective_args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    pub result_format: RunnerResultFormat,
    pub isolation: TestIsolation,
    pub threads: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_id: Option<String>,
    #[serde(default)]
    pub worker_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_root: Option<String>,
    pub migration_decision: String,
    pub lock_decision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub label: String,
    pub passed: bool,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u128,
    pub log: String,
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<FailureCode>,
    #[serde(default)]
    pub attempts: Vec<TaskAttempt>,
    #[serde(default)]
    pub flaky: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_class: Option<RetryClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserEvidence>,
    #[serde(default)]
    pub waived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiver: Option<WaiverEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<RunnerExecution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParserEvidence {
    pub mode: String,
    pub version: u32,
    pub observed: usize,
    pub minimum: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskAttempt {
    pub attempt: u32,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: u128,
    pub timed_out: bool,
    pub cancelled: bool,
    pub log: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaiverEvidence {
    pub id: String,
    pub risk: String,
    pub owner: String,
    pub approved_by: String,
    pub created_at: String,
    pub expires_at: String,
    pub compensating_control: String,
}

impl Task {
    pub fn new(
        label: impl Into<String>,
        program: impl AsRef<OsStr>,
        cwd: &Path,
        log: PathBuf,
    ) -> Self {
        Self {
            label: label.into(),
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: cwd.to_path_buf(),
            env: Vec::new(),
            env_remove: Vec::new(),
            timeout: Duration::from_secs(180),
            log,
            runner: None,
            isolation_state: None,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env_remove.push(key.as_ref().to_os_string());
        self
    }

    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout = Duration::from_secs(seconds);
        self
    }

    pub fn runner(mut self, execution: RunnerExecution) -> Self {
        self.runner = Some(execution);
        self
    }

    pub fn isolation_state(mut self, state_file: PathBuf) -> Self {
        self.isolation_state = Some(state_file);
        self
    }

    pub fn run(self) -> Result<TaskResult> {
        let _state_guard = self
            .isolation_state
            .as_deref()
            .map(IsolationStateGuard::new);
        let mut log_file = crate::utils::fs::create_atomic_output(&self.log, true)
            .with_context(|| format!("create log {}", self.log.display()))?;
        let started = Instant::now();
        let started_at = chrono::Utc::now().to_rfc3339();
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(self.env);
        for name in self.env_remove {
            command.env_remove(name);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        isolate_process_tree(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", self.program.to_string_lossy()))?;

        let stdout = child
            .stdout
            .take()
            .context("task stdout was not captured")?;
        let stderr = child
            .stderr
            .take()
            .context("task stderr was not captured")?;
        let stdout_overflow = Arc::new(AtomicBool::new(false));
        let stderr_overflow = Arc::new(AtomicBool::new(false));
        let (stdout_handle, stdout_receiver) =
            spawn_limited_reader(stdout, DEFAULT_CAPTURE_BYTES, Arc::clone(&stdout_overflow));
        let (stderr_handle, stderr_receiver) =
            spawn_limited_reader(stderr, DEFAULT_CAPTURE_BYTES, Arc::clone(&stderr_overflow));

        let mut output_limited = None;
        let mut wait_round = 0_u32;
        let (status, timed_out, was_cancelled) = loop {
            if let Some(status) = child.try_wait()? {
                if stdout_overflow.load(Ordering::Acquire) {
                    output_limited = Some("stdout");
                } else if stderr_overflow.load(Ordering::Acquire) {
                    output_limited = Some("stderr");
                }
                break (status, false, false);
            }
            if stdout_overflow.load(Ordering::Acquire) {
                output_limited = Some("stdout");
                break (terminate(&mut child)?, false, false);
            }
            if stderr_overflow.load(Ordering::Acquire) {
                output_limited = Some("stderr");
                break (terminate(&mut child)?, false, false);
            }
            if cancelled() {
                break (terminate(&mut child)?, false, true);
            }
            if started.elapsed() >= self.timeout {
                break (terminate(&mut child)?, true, false);
            }
            // `Child::try_wait` is the portable API available on all supported
            // platforms. Keep the initial delay short for fast commands, then
            // back off to cap wakeups while preserving timeout/cancellation checks.
            std::thread::sleep(wait_backoff(wait_round));
            wait_round = wait_round.saturating_add(1);
        };

        let reader_started = Instant::now();
        let mut reader_error = None;
        let stdout = match collect_limited_reader(
            stdout_handle,
            stdout_receiver,
            DEFAULT_READER_DEADLINE,
            "task stdout",
        ) {
            Ok(output) => output,
            Err(error) => {
                reader_error = Some(format!("task stdout reader deadline/error: {error}"));
                LimitedOutput {
                    bytes: Vec::new(),
                    truncated: false,
                }
            }
        };
        let remaining_reader_deadline =
            DEFAULT_READER_DEADLINE.saturating_sub(reader_started.elapsed());
        let stderr = match collect_limited_reader(
            stderr_handle,
            stderr_receiver,
            remaining_reader_deadline,
            "task stderr",
        ) {
            Ok(output) => output,
            Err(error) => {
                reader_error
                    .get_or_insert_with(|| format!("task stderr reader deadline/error: {error}"));
                LimitedOutput {
                    bytes: Vec::new(),
                    truncated: false,
                }
            }
        };

        let mut failure_code = None;
        let mut limit_detail = None;
        let combined_bytes = stdout.bytes.len().saturating_add(stderr.bytes.len());
        if let Some(stream) = output_limited
            .or_else(|| {
                stdout
                    .truncated
                    .then_some("stdout")
                    .or_else(|| stderr.truncated.then_some("stderr"))
            })
            .or_else(|| {
                (combined_bytes > DEFAULT_CAPTURE_BYTES.saturating_mul(2)).then_some("combined")
            })
        {
            failure_code = Some(FailureCode::OutputLimitExceeded);
            let captured = if stream == "stdout" {
                stdout.bytes.len()
            } else if stream == "stderr" {
                stderr.bytes.len()
            } else {
                combined_bytes
            };
            limit_detail = Some(format!(
                "{stream} output exceeded {} bytes (captured {captured} bytes; truncated=true)",
                if stream == "combined" {
                    DEFAULT_CAPTURE_BYTES.saturating_mul(2)
                } else {
                    DEFAULT_CAPTURE_BYTES
                }
            ));
        } else if let Some(error) = reader_error {
            failure_code = Some(FailureCode::ReaderDeadlineExceeded);
            limit_detail = Some(error);
        }

        log_file.write_all(&stdout.bytes)?;
        if !stdout.bytes.is_empty() && !stderr.bytes.is_empty() {
            log_file.write_all(b"\n")?;
        }
        log_file.write_all(&stderr.bytes)?;
        if let Some(detail) = &limit_detail {
            log_file
                .write_all(format!("\n[HARNESS_GATE_EVIDENCE_FAILURE] {detail}\n").as_bytes())?;
        }
        log_file
            .publish()
            .with_context(|| format!("publish log {}", self.log.display()))?;

        let detail = if let Some(detail) = limit_detail {
            Some(detail)
        } else if was_cancelled {
            Some("cancelled".to_string())
        } else if timed_out {
            Some("timed out".to_string())
        } else if status.success() {
            None
        } else {
            status.code().map(|code| format!("exit code {code}"))
        };

        Ok(TaskResult {
            step_id: None,
            invocation_id: None,
            attempt: None,
            started_at: Some(started_at),
            finished_at: Some(chrono::Utc::now().to_rfc3339()),
            label: self.label,
            passed: status.success() && !timed_out && !was_cancelled && failure_code.is_none(),
            timed_out,
            cancelled: was_cancelled,
            duration_ms: started.elapsed().as_millis(),
            log: self.log.to_string_lossy().to_string(),
            detail,
            failure_code,
            attempts: Vec::new(),
            flaky: false,
            retry_class: None,
            parser: None,
            waived: false,
            waiver: None,
            runner: self.runner,
        })
    }
}

fn wait_backoff(round: u32) -> Duration {
    let shift = round.min(4);
    Duration::from_millis((5_u64 << shift).min(80))
}

struct IsolationStateGuard<'a> {
    path: &'a Path,
}

impl<'a> IsolationStateGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }
}

impl Drop for IsolationStateGuard<'_> {
    fn drop(&mut self) {
        let _ = isolation::mark_terminal(self.path, "worker exited");
        let _ = isolation::remove(self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::wait_backoff;
    use std::time::Duration;

    #[test]
    fn wait_backoff_is_bounded_and_starts_short() {
        assert_eq!(wait_backoff(0), Duration::from_millis(5));
        assert_eq!(wait_backoff(1), Duration::from_millis(10));
        assert_eq!(wait_backoff(4), Duration::from_millis(80));
        assert_eq!(wait_backoff(u32::MAX), Duration::from_millis(80));
    }
}
