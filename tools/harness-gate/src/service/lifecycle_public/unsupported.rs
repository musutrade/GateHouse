use super::Config;
use crate::project::Project;
use anyhow::{bail, Result};
use std::time::Instant;
pub(crate) struct ManagedLifecycle;
#[cfg(not(any(target_os = "linux")))]
impl ManagedLifecycle {
    pub(crate) fn start(_: &Project, _: &str, _: Config, _: Instant) -> Result<Self> {
        bail!("lifecycle services require Linux; no CLI fallback")
    }
    pub(crate) fn port(&self) -> u16 {
        0
    }
    pub(crate) fn cleanup(self, _: Instant) -> Result<()> {
        bail!("lifecycle services require Linux")
    }
}

#[cfg(not(any(target_os = "linux")))]
pub(crate) fn cleanup_known(_: &Project, _: &str, _: u64, _: &str) -> Result<String> {
    bail!("lifecycle recovery requires Linux; no CLI fallback")
}
