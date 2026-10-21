//! Staged Linux v3 readiness host. All trusted fields originate from its caller,
//! not a public document/CLI. Single-file pins do not bind dynamic libraries,
//! interpreter closures or prove daemon identity. Path execution retains its
//! existing race boundary; there is no sandbox or installed-artifact authority.
use super::{adapter::VerifiedExecutable, fd_inputs::SealedInputs, raw_adapter, HostPolicy};
use gatehouse_runtime_protocol::v3;
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, &'static str>;

#[allow(dead_code)]
pub(super) struct ReadonlyHostInput {
    pub expected: v3::Request,
    pub config: Vec<u8>,
    pub actual: Vec<u8>,
    pub executable: PathBuf,
    pub executable_sha256: String,
    /// Independently trusted metadata, not a version queried from the binary.
    pub executable_version: String,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
}

#[allow(dead_code)]
pub(super) fn run_readonly(
    input: ReadonlyHostInput,
    cancelled: impl Fn() -> bool,
) -> Result<v3::Response> {
    input
        .expected
        .validate()
        .map_err(|_| "invalid runtime request")?;
    if !(1..=30_000).contains(&input.timeout_ms)
        || input.actual.len() > 65_536
        || input.config.len() > 65_536
    {
        return Err("invalid runtime bounds");
    }
    let actual = v3::parse_request(&input.actual).map_err(|_| "invalid runtime actual request")?;
    if actual != input.expected {
        return Err("runtime request mismatch");
    }

    super::runtime_request_v3::validate(&input.config, &input.expected)?;
    if input.executable_sha256 != input.expected.binding.adapter_sha256
        || input.executable_version != input.expected.binding.adapter_version
        || !input.executable.is_absolute()
    {
        return Err("runtime executable pin mismatch");
    }
    let executable = VerifiedExecutable::open(&input.executable, &input.executable_sha256)
        .map_err(|_| "runtime executable verification failed")?;
    executable
        .require_elf()
        .map_err(|_| "runtime ELF verification failed")?;
    if !input.cwd.is_absolute()
        || !input.cwd.is_dir()
        || std::fs::canonicalize(&input.cwd).map_err(|_| "invalid runtime cwd")? != input.cwd
    {
        return Err("invalid runtime cwd");
    }
    let expected_bytes =
        serde_json::to_vec(&input.expected).map_err(|_| "runtime request encoding failed")?;
    if cancelled() {
        return Err("runtime cancelled");
    }
    // SealedInputs checks both exact byte lengths before allocating any memfd.
    // Keep the serialized expected-input limit at that shared boundary.
    let inputs =
        SealedInputs::new(&input.config, &expected_bytes).map_err(|_| "runtime inputs failed")?;
    let mut command = Command::new(&input.executable);
    command.current_dir(&input.cwd).env_clear();
    let policy = HostPolicy {
        max_stdout_bytes: 65_536,
        max_stderr_bytes: 65_536,
        max_output_bytes: 65_536,
        reader_deadline: Duration::from_secs(2),
        ..HostPolicy::default()
    };
    executable
        .revalidate()
        .map_err(|_| "runtime executable changed")?;
    let output = raw_adapter::run_with_inputs(
        command,
        inputs,
        &policy,
        input.timeout_ms,
        input.actual,
        Instant::now(),
        &cancelled,
    )
    .map_err(|_| "runtime transport failed")?;
    raw_adapter::validate_process_output(&output, &policy)
        .map_err(|_| "runtime output rejected")?;
    if !output.status.success()
        || output.timed_out
        || output.was_cancelled
        || output.request_write_error.is_some()
    {
        return Err("runtime transport incomplete");
    }
    let response = v3::parse_response(&output.stdout.bytes, &input.expected)
        .map_err(|_| "invalid runtime response")?;
    finish(response, &executable, &cancelled)
}

// Separate continuation permits a deterministic test of the post-parse boundary.
fn finish(
    response: v3::Response,
    executable: &VerifiedExecutable,
    cancelled: &impl Fn() -> bool,
) -> Result<v3::Response> {
    if cancelled() {
        return Err("runtime cancelled");
    }
    executable
        .revalidate()
        .map_err(|_| "runtime executable changed")?;
    if cancelled() {
        return Err("runtime cancelled");
    }
    Ok(response)
}
