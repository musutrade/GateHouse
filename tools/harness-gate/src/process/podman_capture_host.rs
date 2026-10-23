//! Read-only Podman host; no Docker wire, journal, lease or mutation authority.
use super::{fd_inputs::SealedInputs, raw_adapter, HostPolicy};
use gatehouse_runtime_protocol::lifecycle_v2::{
    capture_wire::{decode_response, CaptureRequest},
    installation::VerifiedPodmanInstallation,
    plan_record::{PlanInputs, PlanRecord},
};
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
    process::Command,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, &'static str>;
/// Project-selected, fixed read-only installation mapping. Never discovers a socket
/// or executable from the environment and never persists the returned evidence.
#[allow(dead_code)]
pub(crate) fn capture_installed(
    project: &crate::project::Project,
    image_id: String,
    inputs: PlanInputs,
    timeout_ms: u32,
    cancelled: impl Fn() -> bool,
) -> Result<PlanRecord> {
    capture_mapped(&project.root, image_id, inputs, timeout_ms, cancelled)
}
fn capture_mapped(
    root: &std::path::Path,
    image_id: String,
    inputs: PlanInputs,
    timeout_ms: u32,
    cancelled: impl Fn() -> bool,
) -> Result<PlanRecord> {
    if !root.is_absolute()
        || root
            .canonicalize()
            .map_err(|_| "Podman project root unavailable")?
            != root
    {
        return Err("Podman project root must be canonical and absolute");
    }
    let host = root.join(".harness-gate/runtime/podman-capture-v1/host.json");
    let installation = VerifiedPodmanInstallation::load(&host)?;
    capture(&installation, image_id, inputs, timeout_ms, cancelled)
}
#[allow(dead_code)]
pub(crate) fn capture(
    installation: &VerifiedPodmanInstallation,
    image_id: String,
    inputs: PlanInputs,
    timeout_ms: u32,
    cancelled: impl Fn() -> bool,
) -> Result<PlanRecord> {
    capture_observed(installation, image_id, inputs, timeout_ms, cancelled, || {})
}
fn capture_observed(
    installation: &VerifiedPodmanInstallation,
    image_id: String,
    inputs: PlanInputs,
    timeout_ms: u32,
    cancelled: impl Fn() -> bool,
    after_revalidation: impl FnOnce(),
) -> Result<PlanRecord> {
    let started = Instant::now();
    if !(1..=30000).contains(&timeout_ms) || cancelled() {
        return Err("Podman capture budget rejected");
    }
    let request = CaptureRequest {
        schema: "gate-runtime/podman-capture-v1".into(),
        profile: installation.identity()?.profile.clone(),
        invocation: nonce()?,
        image_id,
        inputs,
        timeout_ms,
    };
    let request_bytes = request.encode()?;
    let expected = installation.capture_expected(&request)?;
    let descriptors =
        SealedInputs::with_held_config(installation.config_file()?, &expected.encode(&request)?)
            .map_err(|_| "Podman capture descriptor setup")?;
    let fd = unsafe {
        libc::fcntl(
            installation.executable_file()?.as_raw_fd(),
            libc::F_DUPFD_CLOEXEC,
            5,
        )
    };
    if fd < 0 {
        return Err("Podman executable descriptor duplicate");
    }
    let executable = unsafe { File::from_raw_fd(fd) };
    let mut command = Command::new(format!("/proc/self/fd/{}", executable.as_raw_fd()));
    command.env_clear().current_dir("/");
    let policy = HostPolicy {
        max_stdout_bytes: 24 * 1024 * 1024,
        max_stderr_bytes: 65536,
        max_output_bytes: 24 * 1024 * 1024 + 65536,
        reader_deadline: Duration::from_secs(2),
        ..HostPolicy::default()
    };
    installation.revalidate()?;
    let output = raw_adapter::run_with_exact_inputs(
        command,
        descriptors,
        &policy,
        timeout_ms as u64,
        request_bytes,
        started,
        &cancelled,
    )
    .map_err(|_| "Podman capture transport failure")?;
    raw_adapter::validate_process_output(&output, &policy)
        .map_err(|_| "Podman capture output rejected")?;
    if !output.status.success()
        || output.timed_out
        || output.was_cancelled
        || output.request_write_error.is_some()
        || !output.stderr.bytes.is_empty()
    {
        return Err("Podman capture unsuccessful; output discarded");
    }
    let record = decode_response(
        &output.stdout.bytes,
        &request,
        &expected,
        installation.context()?,
    )?;
    installation.match_record(&record)?.revalidate()?;
    if cancelled() || started.elapsed() >= Duration::from_millis(timeout_ms as u64) {
        return Err("Podman capture final boundary expired or cancelled");
    }
    installation.match_record(&record)?.revalidate()?;
    after_revalidation();
    installation.match_record(&record)?.revalidate()?;
    if cancelled() || started.elapsed() >= Duration::from_millis(timeout_ms as u64) {
        return Err("Podman capture post-revalidation expired or cancelled");
    }
    Ok(record)
}
fn nonce() -> Result<String> {
    let mut bytes = [0u8; 16];
    let mut offset = 0;
    while offset < bytes.len() {
        let n = unsafe {
            libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
        };
        if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if n <= 0 {
            return Err("Podman invocation entropy unavailable");
        }
        offset += n as usize;
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
#[cfg(feature = "podman-capture-joint-tests")]
mod tests;
