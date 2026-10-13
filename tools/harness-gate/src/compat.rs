use crate::project::Project;
use crate::scope::ScopeMode;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const COMPAT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompatibilityRequest {
    pub schema_version: u32,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub staged: bool,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    /// Optional shard evidence supplied by a migration wrapper. When present,
    /// the launcher validates completeness and duplicate test identities before
    /// running the serial compatibility path.
    #[serde(default)]
    pub shards: Vec<crate::verify::parser::ShardResult>,
    #[serde(default)]
    pub shard_total: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CompatibilityResponse {
    pub schema_version: u32,
    pub mode: &'static str,
    pub status: &'static str,
    pub invocation_id: Option<String>,
    pub request_id: Option<String>,
    pub result_path: Option<String>,
    pub comparison: Option<ComparisonReport>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ComparisonReport {
    pub equivalent: bool,
    pub differences: Vec<String>,
    pub old_sha256: String,
    pub new_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CanaryState {
    pub schema_version: u32,
    pub enabled: bool,
    pub slice: String,
    pub updated_at: String,
    #[serde(default)]
    pub history: Vec<CanaryEvent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CanaryEvent {
    pub action: String,
    pub slice: String,
    pub at: String,
}

pub(crate) fn run(
    project: &Project,
    input: &Path,
    output: &Path,
    old_result: Option<&Path>,
) -> Result<CompatibilityResponse> {
    let request: CompatibilityRequest = serde_json::from_slice(
        &fs::read(input)
            .with_context(|| format!("read compatibility request {}", input.display()))?,
    )
    .context("parse compatibility request")?;
    if request.schema_version != COMPAT_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported compatibility request schema version {}; expected {}",
            request.schema_version,
            COMPAT_SCHEMA_VERSION
        );
    }
    if !request.shards.is_empty() {
        let total = request
            .shard_total
            .or_else(|| request.shards.first().map(|shard| shard.shard_total))
            .ok_or_else(|| anyhow::anyhow!("shard evidence has no declared total"))?;
        crate::verify::parser::merge_shards(&request.shards, total)
            .context("validate compatibility shard evidence")?;
    }

    // The launcher is deliberately serial. Parallel execution remains an
    // explicit, separately reviewed configuration choice in the native CLI.
    let mut serial_project = if request.staged {
        project.staged_snapshot()?
    } else {
        project.clone()
    };
    serial_project.config.execution.parallel = false;
    serial_project.config.execution.max_parallel = Some(1);
    let scope = if !request.components.is_empty() {
        crate::verify::explicit_scope(&request.components)
    } else {
        let mode = if request.staged {
            ScopeMode::Staged
        } else if request.all {
            ScopeMode::All
        } else if let Some(base) = request.base {
            ScopeMode::Base(base)
        } else {
            ScopeMode::WorkingTree
        };
        crate::scope::detect(&serial_project, &mode)?
    };
    let profile = request
        .profile
        .unwrap_or_else(|| serial_project.config.project.default_profile.clone());
    let report = crate::verify::run_with_request_id(
        &serial_project,
        scope,
        &profile,
        request.staged,
        request.request_id.as_deref(),
    )?;
    let result_path = output.to_path_buf();
    let bytes = serde_json::to_vec_pretty(&report).context("serialize compatibility result")?;
    crate::utils::fs::atomic_write(&result_path, bytes, true)
        .with_context(|| format!("write compatibility result {}", result_path.display()))?;
    let comparison = old_result
        .map(|old| compare_files(old, &result_path))
        .transpose()?;
    Ok(CompatibilityResponse {
        schema_version: COMPAT_SCHEMA_VERSION,
        mode: if comparison.is_some() {
            "shadow"
        } else {
            "serial"
        },
        status: if report.passed { "PASS" } else { "FAIL" },
        invocation_id: Some(report.invocation_id),
        request_id: request.request_id,
        result_path: Some(result_path.to_string_lossy().into_owned()),
        comparison,
    })
}

pub(crate) fn compare_files(old: &Path, new: &Path) -> Result<ComparisonReport> {
    let old_bytes = fs::read(old).with_context(|| format!("read old result {}", old.display()))?;
    let new_bytes = fs::read(new).with_context(|| format!("read new result {}", new.display()))?;
    let old_value: Value =
        serde_json::from_slice(&old_bytes).context("parse old machine result")?;
    let new_value: Value =
        serde_json::from_slice(&new_bytes).context("parse new machine result")?;
    let mut old_normalized = old_value;
    let mut new_normalized = new_value;
    normalize(&mut old_normalized);
    normalize(&mut new_normalized);
    let mut differences = Vec::new();
    diff_values(&old_normalized, &new_normalized, "$", &mut differences);
    Ok(ComparisonReport {
        equivalent: differences.is_empty(),
        differences,
        old_sha256: digest(&old_bytes),
        new_sha256: digest(&new_bytes),
    })
}

fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in [
                "timestamp",
                "duration_ms",
                "report_directory",
                "started_at",
                "finished_at",
                "invocation_id",
                "request_id",
            ] {
                map.remove(key);
            }
            for (key, child) in map.iter_mut() {
                if matches!(key.as_str(), "log" | "path") {
                    normalize_artifact_path(child);
                }
                normalize(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                normalize(child);
            }
        }
        _ => {}
    }
}

fn normalize_artifact_path(value: &mut Value) {
    let Value::String(path) = value else { return };
    if let Some((_, suffix)) = path.split_once("/invocations/") {
        if let Some((_, relative)) = suffix.split_once('/') {
            *path = relative.to_string();
        }
    }
}

fn diff_values(old: &Value, new: &Value, path: &str, differences: &mut Vec<String>) {
    if old == new {
        return;
    }
    match (old, new) {
        (Value::Object(left), Value::Object(right)) => {
            let keys = left
                .keys()
                .chain(right.keys())
                .cloned()
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                match (left.get(&key), right.get(&key)) {
                    (Some(a), Some(b)) => diff_values(a, b, &format!("{path}.{key}"), differences),
                    _ => differences.push(format!("{path}.{key} differs")),
                }
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                differences.push(format!(
                    "{path} length differs: {} vs {}",
                    left.len(),
                    right.len()
                ));
            }
            for index in 0..left.len().min(right.len()) {
                diff_values(
                    &left[index],
                    &right[index],
                    &format!("{path}[{index}]"),
                    differences,
                );
            }
        }
        _ => differences.push(format!("{path} differs")),
    }
}

pub(crate) fn set_canary(path: &Path, slice: &str) -> Result<CanaryState> {
    let mut state = read_canary(path)?.unwrap_or(CanaryState {
        schema_version: COMPAT_SCHEMA_VERSION,
        enabled: false,
        slice: String::new(),
        updated_at: String::new(),
        history: Vec::new(),
    });
    state.enabled = true;
    state.slice = slice.into();
    state.updated_at = chrono::Utc::now().to_rfc3339();
    state.history.push(CanaryEvent {
        action: "enable".into(),
        slice: slice.into(),
        at: state.updated_at.clone(),
    });
    write_canary(path, &state)?;
    Ok(state)
}

pub(crate) fn rollback(path: &Path) -> Result<CanaryState> {
    let mut state =
        read_canary(path)?.ok_or_else(|| anyhow::anyhow!("canary state does not exist"))?;
    state.enabled = false;
    state.updated_at = chrono::Utc::now().to_rfc3339();
    state.history.push(CanaryEvent {
        action: "rollback".into(),
        slice: state.slice.clone(),
        at: state.updated_at.clone(),
    });
    write_canary(path, &state)?;
    Ok(state)
}

fn read_canary(path: &Path) -> Result<Option<CanaryState>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).context("parse canary state")?,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read canary state {}", path.display())),
    }
}

fn write_canary(path: &Path, state: &CanaryState) -> Result<()> {
    crate::utils::fs::atomic_write(path, serde_json::to_vec_pretty(state)?, true)
        .with_context(|| format!("publish canary state {}", path.display()))
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn comparison_ignores_volatile_fields_but_keeps_status_differences() {
        let directory = tempdir().unwrap();
        let old = directory.path().join("old.json");
        let new = directory.path().join("new.json");
        fs::write(
            &old,
            br#"{"status":"PASS","timestamp":"a","steps":[{"status":"PASS","duration_ms":1}]}"#,
        )
        .unwrap();
        fs::write(
            &new,
            br#"{"status":"PASS","timestamp":"b","steps":[{"status":"PASS","duration_ms":2}]}"#,
        )
        .unwrap();
        assert!(compare_files(&old, &new).unwrap().equivalent);
        fs::write(
            &new,
            br#"{"status":"FAIL","timestamp":"c","steps":[{"status":"FAIL","duration_ms":2}]}"#,
        )
        .unwrap();
        assert!(!compare_files(&old, &new).unwrap().equivalent);
    }

    #[test]
    fn comparison_correlates_distinct_invocations_and_artifact_roots() {
        let directory = tempdir().unwrap();
        let old = directory.path().join("old.json");
        let new = directory.path().join("new.json");
        fs::write(
            &old,
            br#"{"invocation_id":"old","steps":[{"invocation_id":"old","log":"reports/invocations/old/logs/a.log","status":"PASS"}]}"#,
        )
        .unwrap();
        fs::write(
            &new,
            br#"{"invocation_id":"new","steps":[{"invocation_id":"new","log":"reports/invocations/new/logs/a.log","status":"PASS"}]}"#,
        )
        .unwrap();
        assert!(compare_files(&old, &new).unwrap().equivalent);
    }

    #[test]
    fn canary_enable_and_rollback_are_auditable() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("canary.json");
        let enabled = set_canary(&path, "team-a").unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.slice, "team-a");
        let rolled_back = rollback(&path).unwrap();
        assert!(!rolled_back.enabled);
        assert_eq!(rolled_back.history.len(), 2);
    }
}
