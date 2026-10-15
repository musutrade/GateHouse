use anyhow::{ensure, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) const VERSION: &str = "harness-findings/1";

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    pub repository: String,
    pub commit: String,
    pub tree: String,
    pub series: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Source {
    pub format: String,
    pub measurement_id: String,
    pub complete: bool,
    pub document: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Location {
    pub path: String,
    pub line: u32,
    pub end_line: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Finding {
    pub id: String,
    pub stable: bool,
    pub namespace: String,
    pub rule: String,
    pub anchor: String,
    pub tool: String,
    pub tool_version: String,
    pub severity: String,
    pub message: String,
    pub locations: Vec<Location>,
    pub source: usize,
    pub pointer: String,
    pub fixes: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub finding: Finding,
    pub state: String,
    pub first_seen: String,
    pub last_seen: String,
    pub evidence: Source,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    pub schema_version: String,
    pub context: Context,
    pub report_sha256: String,
    pub invocation_id: String,
    pub source_identity: String,
    pub core_status: String,
    pub sources: Vec<Source>,
    pub findings: Vec<Finding>,
    pub diagnostics: Vec<String>,
    pub previous_sha256: Option<String>,
    pub history: Vec<Record>,
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn identity(namespace: &str, rule: &str, anchor: &str) -> String {
    digest(&serde_json::to_vec(&(VERSION, namespace, rule, anchor)).expect("string tuple"))
}

pub(super) fn nonempty(value: &Value, field: &str) -> Result<String> {
    let text = value[field].as_str().unwrap_or_default();
    ensure!(!text.trim().is_empty(), "missing/non-string {field}");
    Ok(text.to_string())
}

pub(super) fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

pub(super) fn validate_context(context: &Context) -> Result<()> {
    ensure!(
        context.repository.len() <= 256,
        "repository identity exceeds 256 bytes"
    );
    validate_identifier(&context.series)?;
    let parts: Vec<_> = context.repository.split('/').collect();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|s| !s.is_empty()
                && *s != "."
                && *s != ".."
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))),
        "invalid repository owner/name"
    );
    ensure!(
        hex(&context.commit, 40) && hex(&context.tree, 40),
        "commit/tree must be exact lowercase Git SHA-1 identities"
    );
    ensure!(
        !context.series.trim().is_empty() && context.series.len() <= 1024,
        "missing measurement series"
    );
    Ok(())
}

pub(super) fn validate_location(location: &Location) -> Result<()> {
    let path = &location.path;
    ensure!(
        !path.is_empty()
            && path.len() <= 4096
            && !path.contains(['\\', ':', '%', '?', '#'])
            && !path.chars().any(char::is_control)
            && path
                .split('/')
                .all(|s| !s.is_empty() && s != "." && s != ".."),
        "location must be a literal repository-relative path"
    );
    ensure!(
        location.line > 0 && location.end_line >= location.line,
        "invalid location line range"
    );
    Ok(())
}

pub(super) fn validate_identifier(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty()
            && value.len() <= 1024
            && !value.chars().any(char::is_control)
            && !value.contains("[REDACTED]")
            && crate::utils::redaction::redact_text(value) == value,
        "identity metadata must be non-secret text of 1..1024 bytes without controls"
    );
    Ok(())
}
