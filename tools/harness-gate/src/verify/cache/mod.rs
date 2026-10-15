//! Local, explicitly trusted command evidence. Never caches Core decisions.
#[cfg(unix)]
mod identity;
#[cfg(unix)]
mod store;
#[cfg(all(test, unix))]
mod tests;

use crate::{
    config::StepConfig,
    process::{CacheEvidence, TaskResult},
    project::Project,
};
use anyhow::Result;
use std::{path::Path, time::Instant};

pub(crate) fn clear(path: &Path) -> Result<serde_json::Value> {
    #[cfg(unix)]
    {
        store::Store::open(path)?.clear()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        anyhow::bail!("step-cache/v1 storage is supported on Unix hosts only")
    }
}

pub(super) fn run(
    project: &Project,
    step: &StepConfig,
    execute: impl FnOnce() -> Result<TaskResult>,
) -> Result<TaskResult> {
    if step.cache.is_none() {
        return execute();
    }
    #[cfg(not(unix))]
    let _ = project;
    let started = Instant::now();
    let mut evidence = CacheEvidence {
        schema_version: "step-cache/v1",
        status: "BYPASS".into(),
        reason: "CACHE_HOST_DISABLED".into(),
        key: None,
        source_invocation: None,
        lookup_ms: 0,
        saved_execution_ms: 0,
        store_ms: 0,
    };
    #[cfg(unix)]
    if let Some(path) = &project.step_cache_dir {
        match prepare(project, step, path) {
            Ok((store, identity)) => {
                return cached_run(project, step, execute, store, identity, started, evidence)
            }
            Err(reason) => evidence.reason = reason.into(),
        }
    }
    #[cfg(not(unix))]
    {
        evidence.reason = "CACHE_PLATFORM_UNSUPPORTED".into();
    }
    evidence.lookup_ms = started.elapsed().as_millis();
    let mut result = execute()?;
    result.cache = Some(evidence);
    Ok(result)
}

#[cfg(unix)]
fn cached_run(
    project: &Project,
    step: &StepConfig,
    execute: impl FnOnce() -> Result<TaskResult>,
    store: store::Store,
    identity: Vec<u8>,
    started: Instant,
    mut evidence: CacheEvidence,
) -> Result<TaskResult> {
    let key = store.key(&identity);
    evidence.key = Some(key.clone());
    evidence.status = "MISS".into();
    evidence.reason = "CACHE_ENTRY_ABSENT".into();
    if let Some(mut result) = try_hit(project, step, &store, &key, &identity, &mut evidence) {
        evidence.lookup_ms = started.elapsed().as_millis();
        result.duration_ms = evidence.lookup_ms;
        result.cache = Some(evidence);
        return Ok(result);
    }
    evidence.lookup_ms = started.elapsed().as_millis();
    let mut result = execute()?;
    let store_started = Instant::now();
    if cacheable_success(&result) {
        let saved = identity::build(project, step).and_then(|after| {
            anyhow::ensure!(after == identity, "inputs changed during execution");
            store.save(project, step, &result, &key)
        });
        if saved.is_err() {
            evidence.reason = "CACHE_STORE_REJECTED".into();
        }
    }
    evidence.store_ms = store_started.elapsed().as_millis();
    result.cache = Some(evidence);
    Ok(result)
}

#[cfg(unix)]
fn cacheable_success(result: &TaskResult) -> bool {
    result.passed
        && !result.timed_out
        && !result.cancelled
        && !result.flaky
        && !result.waived
        && result.failure_code.is_none()
        && !crate::process::cancelled()
}

#[cfg(unix)]
fn try_hit(
    project: &Project,
    step: &StepConfig,
    store: &store::Store,
    key: &str,
    identity: &[u8],
    evidence: &mut CacheEvidence,
) -> Option<TaskResult> {
    let record = match store.load(key) {
        Ok(Some(record)) => record,
        Ok(None) => return None,
        Err(_) => {
            evidence.reason = "CACHE_ENTRY_UNTRUSTED".into();
            return None;
        }
    };
    if !identity::outputs(project, step).is_ok_and(|v| v == record.outputs) {
        evidence.reason = "CACHE_OUTPUT_MISSING_OR_CHANGED".into();
        return None;
    }
    evidence.reason = "CACHE_REVALIDATION_FAILED".into();
    let result = restore(project, step, &record).ok()?;
    if !result.passed || crate::process::cancelled() {
        return None;
    }
    if !identity::build(project, step).is_ok_and(|v| v == identity) {
        return None;
    }
    evidence.status = "HIT".into();
    evidence.reason = "CACHE_INPUTS_AND_OUTPUTS_VERIFIED".into();
    evidence.source_invocation = Some(record.invocation);
    evidence.saved_execution_ms = record.execution_ms;
    Some(result)
}

#[cfg(unix)]
fn prepare(
    project: &Project,
    step: &StepConfig,
    path: &Path,
) -> std::result::Result<(store::Store, Vec<u8>), &'static str> {
    let directory = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !directory.is_absolute()
        || directory.starts_with(&project.root)
        || project.root.starts_with(&directory)
    {
        return Err("CACHE_HOST_UNTRUSTED");
    }
    crate::config::validate_step_cache(&project.config, step)
        .map_err(|_| "CACHE_DECLARATION_UNBOUND")?;
    let identity = identity::build(project, step).map_err(|_| "CACHE_INPUT_UNBOUND")?;
    let store = store::Store::open(path).map_err(|_| "CACHE_HOST_UNTRUSTED")?;
    Ok((store, identity))
}

#[cfg(unix)]
fn restore(project: &Project, step: &StepConfig, record: &store::Record) -> Result<TaskResult> {
    use base64::Engine;
    anyhow::ensure!(
        record.step == step.id && !record.invocation.is_empty(),
        "cache provenance mismatch"
    );
    let log = super::steps::log(project, &step.log)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&record.log)?;
    anyhow::ensure!(
        identity::digest(&bytes) == record.log_digest,
        "cache log digest mismatch"
    );
    let mut file = crate::utils::fs::create_atomic_output(&log, true)?;
    use std::io::Write;
    file.write_all(&bytes)?;
    file.publish()?;
    let now = chrono::Utc::now().to_rfc3339();
    let result = TaskResult {
        cache: None,
        step_id: None,
        invocation_id: None,
        attempt: None,
        started_at: Some(now.clone()),
        finished_at: Some(now),
        label: step.label.clone(),
        passed: true,
        timed_out: false,
        cancelled: false,
        duration_ms: 0,
        log: log.to_string_lossy().into_owned(),
        detail: None,
        failure_code: None,
        attempts: Vec::new(),
        flaky: false,
        retry_class: None,
        parser: None,
        waived: false,
        waiver: None,
        runner: None,
    };
    super::steps::evaluate_result(
        result,
        step.parser
            .as_deref()
            .and_then(|id| project.config.parser(id)),
    )
}
