pub(crate) mod adapter;
mod capture;
mod command;
#[cfg(target_os = "linux")]
mod fd_inputs;
mod isolation;
mod raw_adapter;
mod reader;
pub(crate) mod replay;
mod runtime_config;
pub(crate) mod runtime_host;
pub(crate) mod runtime_host_files;
pub(crate) mod runtime_host_files_v3;
#[cfg(target_os = "linux")]
mod runtime_host_v2;
#[cfg(target_os = "linux")]
mod runtime_host_v3;
#[cfg(target_os = "linux")]
pub(crate) mod runtime_lifecycle_host;
mod runtime_request_v3;
mod signal;
mod task;
#[cfg(test)]
mod tests;

pub use adapter::{
    read_request as read_adapter_request, run as run_adapter, CapabilityPolicy, HostPolicy,
    TrustedKey,
};
pub use capture::{capture, capture_cleanup, CapturedOutput};
pub(crate) use capture::{capture_environment, capture_with_limits, CaptureLimits};
pub(crate) use isolation::{
    allocate as allocate_isolation, ISOLATION_IDS_ENV, ISOLATION_MODE_ENV, ISOLATION_ROOT_ENV,
};
pub use signal::{cancelled, install_signal_handlers};
pub use task::{
    CacheEvidence, ParserEvidence, RunnerExecution, Task, TaskAttempt, TaskResult, WaiverEvidence,
};
