pub(crate) mod adapter;
mod capture;
mod command;
mod isolation;
mod reader;
pub(crate) mod replay;
mod signal;
mod task;
#[cfg(test)]
mod tests;

pub use adapter::{
    read_request as read_adapter_request, run as run_adapter, CapabilityPolicy, HostPolicy,
    TrustedKey,
};
pub use capture::{capture, capture_cleanup, CapturedOutput};
pub(crate) use isolation::{
    allocate as allocate_isolation, ISOLATION_IDS_ENV, ISOLATION_MODE_ENV, ISOLATION_ROOT_ENV,
};
pub use signal::{cancelled, install_signal_handlers};
pub use task::{ParserEvidence, RunnerExecution, Task, TaskAttempt, TaskResult, WaiverEvidence};
