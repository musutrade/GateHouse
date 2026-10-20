//! Explicit operator-pinned, read-only runtime protocol probes. This host has
//! no lease store and rejects Remove before spawning any process.
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
        if matches!(request.operation, Operation::Remove { .. }) {
            bail!("runtime probe does not authorize removal");
        }
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
        Ok(())
    }
}

pub(crate) fn run_files(request: &Path, trusted_host: &Path) -> Result<Response> {
    let request_bytes = read_bounded(request)?;
    let trusted_bytes = read_bounded(trusted_host)?;
    let host: TrustedHost = serde_json::from_slice(&trusted_bytes)
        .map_err(|_| anyhow::anyhow!("invalid trusted runtime host document"))?;
    let request = gatehouse_runtime_protocol::parse_request(&request_bytes)?;
    run(&host, &request, signal::cancelled)
}

fn run(host: &TrustedHost, request: &Request, cancelled: impl Fn() -> bool) -> Result<Response> {
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
