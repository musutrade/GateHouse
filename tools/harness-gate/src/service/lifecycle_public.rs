//! Public service routing shares one immutable configuration with the coordinator.
use std::collections::BTreeMap;
pub(crate) struct Config {
    pub host: String,
    pub image: String,
    pub container_port: u16,
    pub environment: BTreeMap<String, String>,
    pub healthcheck: Vec<String>,
    pub command: Option<Vec<String>>,
}
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{cleanup_known, ManagedLifecycle};
#[cfg(not(any(target_os = "linux")))]
mod unsupported;
#[cfg(not(any(target_os = "linux")))]
pub(crate) use unsupported::{cleanup_known, ManagedLifecycle};
