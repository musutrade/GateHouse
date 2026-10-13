use crate::config::TestIsolation;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const ISOLATION_MODE_ENV: &str = "HARNESS_GATE_ISOLATION_MODE";
pub(crate) const ISOLATION_IDS_ENV: &str = "HARNESS_GATE_WORKER_IDS";
pub(crate) const ISOLATION_ROOT_ENV: &str = "HARNESS_GATE_ISOLATION_ROOT";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IsolationAllocation {
    pub(crate) invocation_id: String,
    pub(crate) step_id: String,
    pub(crate) mode: TestIsolation,
    pub(crate) workers: Vec<String>,
    pub(crate) state_file: String,
}

/// Allocate deterministic, invocation-scoped worker identities. The state
/// file is deliberately plain JSON so `cleanup` and external adapters can
/// inspect it without linking to Harness-Gate.
pub(crate) fn allocate(
    report_root: &Path,
    invocation_id: &str,
    step_id: &str,
    mode: TestIsolation,
    worker_count: usize,
) -> Result<(IsolationAllocation, PathBuf)> {
    let directory = report_root.join("isolation");
    fs::create_dir_all(&directory)
        .with_context(|| format!("create isolation directory {}", directory.display()))?;
    let safe_step = step_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let state_file = directory.join(format!("{safe_step}.json"));
    let workers = (0..worker_count.max(1))
        .map(|index| format!("{invocation_id}:{step_id}:worker-{index}"))
        .collect::<Vec<_>>();
    let allocation = IsolationAllocation {
        invocation_id: invocation_id.into(),
        step_id: step_id.into(),
        mode,
        workers,
        state_file: state_file.to_string_lossy().into_owned(),
    };
    let bytes = serde_json::to_vec_pretty(&allocation).context("serialize isolation allocation")?;
    crate::utils::fs::atomic_write(&state_file, bytes, true)
        .with_context(|| format!("publish isolation allocation {}", state_file.display()))?;
    Ok((allocation, state_file))
}

pub(crate) fn remove(state_file: &Path) -> Result<()> {
    match fs::remove_file(state_file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("remove isolation state {}", state_file.display()))
        }
    }
}

/// Preserve a terminal marker outside the active state filename. The active
/// allocation is removed so a later invocation cannot accidentally reuse it,
/// while cleanup/audit tooling can still prove that an abnormal worker ended.
pub(crate) fn mark_terminal(state_file: &Path, reason: &str) -> Result<PathBuf> {
    let terminal = state_file.with_extension("terminal.json");
    let record = serde_json::json!({
        "state": "terminal",
        "reason": reason,
        "at": chrono::Utc::now().to_rfc3339(),
    });
    let bytes = serde_json::to_vec_pretty(&record).context("serialize terminal isolation state")?;
    crate::utils::fs::atomic_write(&terminal, bytes, true)
        .with_context(|| format!("publish terminal isolation state {}", terminal.display()))?;
    Ok(terminal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn allocation_has_unique_worker_ids_and_cleanup_state() {
        let directory = tempdir().unwrap();
        let (allocation, state_file) = allocate(
            directory.path(),
            "inv-1",
            "backend.tests",
            TestIsolation::DatabasePerWorker,
            4,
        )
        .unwrap();
        assert_eq!(allocation.workers.len(), 4);
        assert_eq!(
            allocation
                .workers
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
        assert!(state_file.is_file());
        remove(&state_file).unwrap();
        assert!(!state_file.exists());
    }
}
