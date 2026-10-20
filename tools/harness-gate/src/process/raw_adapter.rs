//! Raw byte subprocess boundary shared by adapter hosts. No JSON parsing or
//! policy decision occurs here. The caller must authenticate the executable and
//! request before entry and reject cancellation after response validation.
//! HostPolicy is reused for its output/drain budgets; signing and artifact
//! verification remain in adapter.rs. No runtime engine is selected here.
use super::adapter::{AdapterError, HostPolicy};
use super::command::{isolate_process_tree, terminate, try_complete};
use super::reader::{collect_limited_reader, spawn_limited_reader};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) struct ProcessExit {
    pub(super) status: std::process::ExitStatus,
    pub(super) timed_out: bool,
    pub(super) was_cancelled: bool,
    pub(super) output_limited: Option<(&'static str, usize)>,
    pub(super) request_write_error: Option<AdapterError>,
    pub(super) request_write_done: bool,
}

pub(super) struct ProcessOutput {
    pub(super) status: std::process::ExitStatus,
    pub(super) timed_out: bool,
    pub(super) was_cancelled: bool,
    pub(super) output_limited: Option<(&'static str, usize)>,
    pub(super) request_write_error: Option<AdapterError>,
    pub(super) stdout: super::reader::LimitedOutput,
    pub(super) stderr: super::reader::LimitedOutput,
}

pub(super) fn run(
    command: &mut Command,
    policy: &HostPolicy,
    timeout_ms: u64,
    request_json: Vec<u8>,
    started: Instant,
    is_cancelled: &impl Fn() -> bool,
) -> Result<ProcessOutput, AdapterError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    isolate_process_tree(command);
    spawn_and_supervise(
        command,
        policy,
        timeout_ms,
        request_json,
        started,
        is_cancelled,
    )
}

/// Additive Linux bootstrap transport. Consumes Command so descriptor guards
/// cannot survive a failed/cancelled call or be accidentally reinstalled/retried.
/// Callers still authenticate inputs/executable and validate response semantics.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub(super) fn run_with_inputs(
    mut command: Command,
    inputs: super::fd_inputs::SealedInputs,
    policy: &HostPolicy,
    timeout_ms: u64,
    request_json: Vec<u8>,
    started: Instant,
    is_cancelled: &impl Fn() -> bool,
) -> Result<ProcessOutput, AdapterError> {
    if is_cancelled() {
        return Err(AdapterError::Protocol("adapter cancelled".into()));
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    inputs
        .install(&mut command)
        .map_err(|_| AdapterError::Protocol("install runtime inputs failed".into()))?;
    // Callback order is descriptor remap, then setsid. No caller callback may
    // change descriptor policy; this entry does not authenticate arbitrary code.
    isolate_process_tree(&mut command);
    if is_cancelled() {
        return Err(AdapterError::Protocol("adapter cancelled".into()));
    }
    spawn_and_supervise(
        &mut command,
        policy,
        timeout_ms,
        request_json,
        started,
        is_cancelled,
    )
}

fn spawn_and_supervise(
    command: &mut Command,
    policy: &HostPolicy,
    timeout_ms: u64,
    request_json: Vec<u8>,
    started: Instant,
    is_cancelled: &impl Fn() -> bool,
) -> Result<ProcessOutput, AdapterError> {
    let mut child = command.spawn().map_err(|error| {
        AdapterError::Protocol(format!(
            "start adapter {}: {error}",
            command.get_program().to_string_lossy()
        ))
    })?;
    let stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            let _ = terminate(&mut child);
            return Err(AdapterError::Protocol("adapter stdin was not piped".into()));
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = terminate(&mut child);
            return Err(AdapterError::Protocol(
                "adapter stdout was not piped".into(),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = terminate(&mut child);
            return Err(AdapterError::Protocol(
                "adapter stderr was not piped".into(),
            ));
        }
    };
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let (stdout_handle, stdout_receiver) = spawn_limited_reader(
        stdout,
        policy.max_stdout_bytes,
        Arc::clone(&stdout_overflow),
    );
    let (stderr_handle, stderr_receiver) = spawn_limited_reader(
        stderr,
        policy.max_stderr_bytes,
        Arc::clone(&stderr_overflow),
    );
    let (stdin_handle, stdin_receiver) = spawn_request_writer(stdin, request_json);
    let ProcessExit {
        status,
        timed_out,
        was_cancelled,
        output_limited,
        mut request_write_error,
        request_write_done,
    } = wait_for_adapter(
        &mut child,
        &stdin_receiver,
        [&stdout_overflow, &stderr_overflow],
        policy,
        timeout_ms,
        started,
        is_cancelled,
    )?;
    let reader_started = Instant::now();
    let stdout = collect_limited_reader(
        stdout_handle,
        stdout_receiver,
        policy.reader_deadline,
        "adapter stdout",
    )
    .map_err(|error| map_reader_error("stdout", policy.reader_deadline, error))?;
    let remaining_reader_deadline = policy
        .reader_deadline
        .saturating_sub(reader_started.elapsed());
    let stderr = collect_limited_reader(
        stderr_handle,
        stderr_receiver,
        remaining_reader_deadline,
        "adapter stderr",
    )
    .map_err(|error| map_reader_error("stderr", remaining_reader_deadline, error))?;
    let writer_deadline = policy
        .reader_deadline
        .saturating_sub(reader_started.elapsed());
    if request_write_done || request_write_error.is_some() {
        if stdin_handle.join().is_err() {
            request_write_error = Some(AdapterError::Protocol(
                "adapter stdin writer thread panicked".into(),
            ));
        }
    } else {
        request_write_error =
            collect_request_writer(stdin_handle, stdin_receiver, writer_deadline).err();
    }
    Ok(ProcessOutput {
        status,
        timed_out,
        was_cancelled,
        output_limited,
        request_write_error,
        stdout,
        stderr,
    })
}

pub(super) fn wait_for_adapter(
    child: &mut std::process::Child,
    stdin_receiver: &Receiver<io::Result<()>>,
    overflow: [&AtomicBool; 2],
    policy: &HostPolicy,
    timeout_ms: u64,
    started: Instant,
    is_cancelled: &impl Fn() -> bool,
) -> Result<ProcessExit, AdapterError> {
    let timeout = Duration::from_millis(timeout_ms);
    let mut timed_out = false;
    let mut was_cancelled = false;
    let mut output_limited = None;
    let mut request_write_error: Option<AdapterError> = None;
    let mut request_write_done = false;
    let status = loop {
        if let Some(status) = try_complete(child)
            .map_err(|error| AdapterError::Protocol(format!("wait for adapter: {error}")))?
        {
            break status;
        }
        if !request_write_done {
            match stdin_receiver.try_recv() {
                Ok(Ok(())) => request_write_done = true,
                Ok(Err(error)) => {
                    request_write_error = Some(AdapterError::Protocol(format!(
                        "write adapter request: {error}"
                    )));
                    break terminate(child).map_err(|error| {
                        AdapterError::Protocol(format!(
                            "terminate adapter after stdin error: {error}"
                        ))
                    })?;
                }
                Err(TryRecvError::Disconnected) => {
                    request_write_error = Some(AdapterError::Protocol(
                        "adapter stdin writer disconnected".into(),
                    ));
                    break terminate(child).map_err(|error| {
                        AdapterError::Protocol(format!(
                            "terminate adapter after stdin error: {error}"
                        ))
                    })?;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        let termination = if overflow[0].load(Ordering::Acquire) {
            output_limited = Some(("stdout", policy.max_stdout_bytes));
            Some("noisy")
        } else if overflow[1].load(Ordering::Acquire) {
            output_limited = Some(("stderr", policy.max_stderr_bytes));
            Some("noisy")
        } else if is_cancelled() {
            was_cancelled = true;
            Some("cancelled")
        } else if started.elapsed() >= timeout {
            timed_out = true;
            Some("timed out")
        } else {
            None
        };
        if let Some(reason) = termination {
            break terminate(child).map_err(|error| {
                AdapterError::Protocol(format!("terminate {reason} adapter: {error}"))
            })?;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ok(ProcessExit {
        status,
        timed_out,
        was_cancelled,
        output_limited,
        request_write_error,
        request_write_done,
    })
}

pub(super) fn validate_process_output(
    output: &ProcessOutput,
    policy: &HostPolicy,
) -> Result<(), AdapterError> {
    let ProcessOutput {
        output_limited,
        stdout,
        stderr,
        ..
    } = output;
    if let Some((stream, limit)) = *output_limited {
        let captured = if stream == "stdout" {
            stdout.bytes.len()
        } else {
            stderr.bytes.len()
        };
        return Err(AdapterError::OutputLimit {
            stream,
            limit,
            captured,
        });
    }
    let combined_bytes = stdout.bytes.len().saturating_add(stderr.bytes.len());
    if stdout.truncated || stderr.truncated || combined_bytes > policy.max_output_bytes {
        let (stream, limit, captured) = if stdout.truncated {
            ("stdout", policy.max_stdout_bytes, stdout.bytes.len())
        } else if stderr.truncated {
            ("stderr", policy.max_stderr_bytes, stderr.bytes.len())
        } else {
            ("combined", policy.max_output_bytes, combined_bytes)
        };
        return Err(AdapterError::OutputLimit {
            stream,
            limit,
            captured,
        });
    }
    Ok(())
}

fn spawn_request_writer(
    mut stdin: std::process::ChildStdin,
    request: Vec<u8>,
) -> (JoinHandle<()>, Receiver<io::Result<()>>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = std::thread::spawn(move || {
        let result = stdin
            .write_all(&request)
            .and_then(|_| stdin.write_all(b"\n"));
        let _ = sender.send(result);
    });
    (handle, receiver)
}

fn collect_request_writer(
    handle: JoinHandle<()>,
    receiver: Receiver<io::Result<()>>,
    deadline: Duration,
) -> Result<(), AdapterError> {
    match receiver.recv_timeout(deadline) {
        Ok(result) => {
            if handle.join().is_err() {
                return Err(AdapterError::Protocol(
                    "adapter stdin writer thread panicked".into(),
                ));
            }
            result
                .map_err(|error| AdapterError::Protocol(format!("write adapter request: {error}")))
        }
        Err(RecvTimeoutError::Timeout) => {
            drop(handle);
            Err(AdapterError::ReaderDeadline {
                stream: "adapter stdin",
                deadline_ms: deadline.as_millis(),
            })
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            Err(AdapterError::Protocol(
                "adapter stdin writer disconnected".into(),
            ))
        }
    }
}

fn map_reader_error(
    stream: &'static str,
    deadline: Duration,
    error: std::io::Error,
) -> AdapterError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        AdapterError::ReaderDeadline {
            stream,
            deadline_ms: deadline.as_millis(),
        }
    } else {
        AdapterError::Protocol(format!("read adapter {stream}: {error}"))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn preserves_raw_request_and_duplicate_json_response_bytes() {
        let directory = tempdir().unwrap();
        let captured = directory.path().join("request.bin");
        let adapter = directory.path().join("adapter.sh");
        fs::write(
            &adapter,
            "#!/bin/sh\ncat > \"$1\"\nprintf '%s' '{\"a\":1,\"a\":2,\"nested\":{\"b\":1,\"b\":2}}'\n",
        )
        .unwrap();
        fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).unwrap();
        let mut command = Command::new(&adapter);
        command.arg(&captured);
        let request = br#"{"request":1,"request":2}"#.to_vec();
        let policy = HostPolicy {
            max_stdout_bytes: 4096,
            max_stderr_bytes: 4096,
            max_output_bytes: 8192,
            reader_deadline: Duration::from_secs(5),
            ..HostPolicy::default()
        };
        let output = run(
            &mut command,
            &policy,
            5_000,
            request.clone(),
            Instant::now(),
            &|| false,
        )
        .unwrap();
        validate_process_output(&output, &policy).unwrap();
        assert!(output.status.success());
        assert_eq!(
            output.stdout.bytes.as_slice(),
            br#"{"a":1,"a":2,"nested":{"b":1,"b":2}}"#
        );
        assert!(output.stderr.bytes.is_empty());
        let mut expected = request;
        expected.push(b'\n');
        assert_eq!(fs::read(&captured).unwrap(), expected);
        // A Value round-trip would drop the duplicate fields; the raw boundary
        // must hand the exact bytes to the strict parser.
        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout.bytes).unwrap();
        assert_eq!(parsed.to_string(), r#"{"a":2,"nested":{"b":2}}"#);
    }
}
