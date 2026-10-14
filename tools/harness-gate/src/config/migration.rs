//! Legacy ecosystem migration belongs to the external recipe owner.
use super::model::FlowConfig;
use anyhow::{bail, Result};

pub fn migrate_v1(_source: &str, _project_name: &str) -> Result<FlowConfig> {
    bail!("v1 migration requires GateProbes: run gateprobes-postgres migrate --input <PATH> --output <NEW_PATH> --project <NAME>; validate the output with harness-gate config check; keep the source for rollback")
}
