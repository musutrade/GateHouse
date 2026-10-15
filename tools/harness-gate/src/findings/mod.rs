//! Presentation-only interfaces. This module cannot evaluate or update policy.
mod history;
mod model;
mod sarif;
#[cfg(test)]
mod tests;

use anyhow::{ensure, Context as _, Result};
use clap::Subcommand;
use model::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FINDINGS: usize = 10_000;

#[derive(Debug, Subcommand)]
pub(crate) enum Action {
    /// Export the versioned input bundle or normalized report JSON schema.
    Schema {
        #[arg(long)]
        bundle: bool,
        #[arg(long)]
        output: PathBuf,
    },
    /// Normalize native JSON or SARIF sources bound to one completed Core report.
    Normalize {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        report: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Append diagnostic history using an independently pinned previous digest.
    Compare {
        #[arg(long)]
        current: PathBuf,
        #[arg(long)]
        previous: PathBuf,
        #[arg(long)]
        previous_sha256: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Export normalized findings as SARIF 2.1.0; policy is unchanged.
    Export {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Bundle {
    schema_version: String,
    context: model::Context,
    report_sha256: String,
    sources: Vec<Source>,
}

fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "finding input exceeds 16 MiB"
    );
    Ok(bytes)
}

fn parse(bytes: &[u8]) -> Result<Value> {
    // The shared parser rejects duplicate keys rather than silently choosing one.
    Ok(harness_gate::quality::parse(std::str::from_utf8(bytes)?)?)
}

fn write(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "finding output exceeds 16 MiB"
    );
    // Never overwrite an input, history checkpoint or existing evidence.
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&bytes)?;
    Ok(())
}

pub(crate) fn run(action: &Action) -> Result<bool> {
    let (output, value) = match action {
        Action::Schema { bundle, output } => (
            output,
            if *bundle {
                serde_json::to_value(schemars::schema_for!(Bundle))?
            } else {
                serde_json::to_value(schemars::schema_for!(Report))?
            },
        ),
        Action::Normalize {
            input,
            report,
            output,
        } => {
            let bundle: Bundle = serde_json::from_value(parse(&read(input)?)?)?;
            (
                output,
                serde_json::to_value(normalize(bundle, &read(report)?)?)?,
            )
        }
        Action::Compare {
            current,
            previous,
            previous_sha256,
            output,
        } => {
            let current = load(&read(current)?)?;
            let previous_bytes = read(previous)?;
            ensure!(
                hex(previous_sha256, 64) && digest(&previous_bytes) == *previous_sha256,
                "previous history digest mismatch; obtain pin from trusted storage"
            );
            let previous = load(&previous_bytes)?;
            (
                output,
                serde_json::to_value(history::compare(current, previous, previous_sha256)?)?,
            )
        }
        Action::Export { input, output } => (output, sarif::export(&load(&read(input)?)?)),
    };
    write(output, &value)?;
    Ok(true)
}

fn normalize(bundle: Bundle, core_bytes: &[u8]) -> Result<Report> {
    ensure!(
        bundle.schema_version == VERSION,
        "unsupported findings schema version"
    );
    validate_context(&bundle.context)?;
    ensure!(
        hex(&bundle.report_sha256, 64) && digest(core_bytes) == bundle.report_sha256,
        "Core report digest mismatch"
    );
    let core = parse(core_bytes)?;
    ensure!(
        core["schema_version"] == "1" && core["evidence_complete"] == true,
        "missing/incomplete Core report"
    );
    let status = nonempty(&core, "status")?;
    ensure!(
        matches!(status.as_str(), "PASS" | "FAIL") && core["passed"] == (status == "PASS"),
        "inconsistent Core decision"
    );
    let source_identity = nonempty(&core, "source_identity")?;
    ensure!(
        source_identity == format!("git-tree:{}", bundle.context.tree),
        "findings require an immutable staged Core report matching context.tree"
    );
    let invocation_id = nonempty(&core, "invocation_id")?;
    ensure!(
        !bundle.sources.is_empty() && bundle.sources.len() <= 100,
        "expected 1..100 source documents"
    );
    let mut report = Report {
        schema_version: VERSION.into(),
        context: bundle.context,
        report_sha256: bundle.report_sha256,
        invocation_id,
        source_identity,
        core_status: status,
        sources: Vec::new(),
        findings: Vec::new(),
        diagnostics: Vec::new(),
        previous_sha256: None,
        history: Vec::new(),
    };
    let mut measurements = std::collections::BTreeSet::new();
    for (index, mut source) in bundle.sources.into_iter().enumerate() {
        ensure!(
            source.complete
                && !source.measurement_id.trim().is_empty()
                && measurements.insert(source.measurement_id.clone()),
            "incomplete or duplicate source measurement identity"
        );
        // Redact recursively without changing JSON types, keys, or structure.
        redact_value(&mut source.document);
        validate_identifier(&source.measurement_id)?;
        let findings = match source.format.as_str() {
            "native/1" => native(&source.document, index)?,
            "sarif/2.1.0" => sarif::import(&source.document, index)?,
            _ => anyhow::bail!("unsupported finding source format"),
        };
        report.findings.extend(findings);
        ensure!(
            report.findings.len() <= MAX_FINDINGS,
            "finding count exceeds 10000"
        );
        report.sources.push(source);
    }
    for finding in &report.findings {
        if !finding.stable {
            report.diagnostics.push(format!(
                "{}: location-only identity; cannot compare history",
                finding.id
            ));
        }
    }
    report.history = history::initial(&report);
    history::validate(&report, false)?;
    Ok(report)
}

fn redact_value(value: &mut Value) {
    match value {
        Value::String(text) => *text = crate::utils::redaction::redact_text(text),
        Value::Array(values) => values.iter_mut().for_each(redact_value),
        Value::Object(map) => {
            for (key, value) in map {
                let probe = json!({key: "SYNTHETIC_CREDENTIAL"}).to_string();
                if crate::utils::redaction::redact_text(&probe) != probe {
                    *value = json!("[REDACTED]");
                } else {
                    redact_value(value);
                }
            }
        }
        _ => {}
    }
}

fn native(document: &Value, source: usize) -> Result<Vec<Finding>> {
    ensure!(
        document["schema_version"] == "native/1",
        "unsupported native source version"
    );
    let tool = nonempty(document, "tool")?;
    let version = nonempty(document, "tool_version")?;
    validate_identifier(&tool)?;
    validate_identifier(&version)?;
    let values = document["findings"]
        .as_array()
        .context("missing native findings array")?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let rule = nonempty(value, "rule")?;
            let namespace = value["namespace"].as_str().unwrap_or(&tool).to_string();
            ensure!(!namespace.trim().is_empty(), "empty namespace");
            let locations: Vec<Location> = serde_json::from_value(value["locations"].clone())?;
            ensure!(!locations.is_empty(), "finding has no locations");
            for location in &locations {
                validate_location(location)?;
            }
            let anchor = value["anchor"].as_str().unwrap_or_default().to_string();
            let stable = !anchor.trim().is_empty();
            let anchor = if stable {
                anchor
            } else {
                format!("location:{}", digest(&serde_json::to_vec(&locations)?))
            };
            let severity = nonempty(value, "severity")?;
            ensure!(
                matches!(severity.as_str(), "error" | "warning" | "note" | "none"),
                "unknown finding severity"
            );
            for field in [&namespace, &rule, &anchor] {
                validate_identifier(field)?;
            }
            Ok(Finding {
                id: identity(&namespace, &rule, &anchor),
                stable,
                namespace,
                rule,
                anchor,
                tool: tool.clone(),
                tool_version: version.clone(),
                severity,
                message: nonempty(value, "message")?,
                locations,
                source,
                pointer: format!("/findings/{index}"),
                fixes: value["fixes"].clone(),
            })
        })
        .collect()
}

fn load(bytes: &[u8]) -> Result<Report> {
    let report: Report = serde_json::from_value(parse(bytes)?)?;
    ensure!(
        report.schema_version == VERSION,
        "unsupported findings schema version"
    );
    validate_context(&report.context)?;
    ensure!(
        report.findings.len() <= MAX_FINDINGS && report.history.len() <= MAX_FINDINGS,
        "finding/history limit exceeded"
    );
    ensure!(
        matches!(report.core_status.as_str(), "PASS" | "FAIL")
            && hex(&report.report_sha256, 64)
            && !report.invocation_id.is_empty()
            && report.source_identity == format!("git-tree:{}", report.context.tree),
        "invalid report binding"
    );
    ensure!(
        !report.sources.is_empty() && report.sources.len() <= 100,
        "invalid source count"
    );
    let mut rebuilt = Vec::new();
    let mut measurements = std::collections::BTreeSet::new();
    for (index, source) in report.sources.iter().enumerate() {
        ensure!(
            source.complete
                && !source.measurement_id.trim().is_empty()
                && measurements.insert(&source.measurement_id),
            "incomplete or duplicate measurement"
        );
        rebuilt.extend(source_findings(source, index)?);
    }
    ensure!(
        serde_json::to_value(&rebuilt)? == serde_json::to_value(&report.findings)?,
        "findings/source provenance mismatch"
    );
    for finding in report
        .findings
        .iter()
        .chain(report.history.iter().map(|r| &r.finding))
    {
        ensure!(
            finding.id == identity(&finding.namespace, &finding.rule, &finding.anchor),
            "finding identity mismatch"
        );
        ensure!(!finding.locations.is_empty(), "missing locations");
        for location in &finding.locations {
            validate_location(location)?;
        }
    }
    history::validate(&report, false)?;
    Ok(report)
}

fn source_findings(source: &Source, index: usize) -> Result<Vec<Finding>> {
    ensure!(
        source.complete && !source.measurement_id.trim().is_empty(),
        "incomplete historical source"
    );
    validate_identifier(&source.measurement_id)?;
    let mut redacted = source.document.clone();
    redact_value(&mut redacted);
    ensure!(
        redacted == source.document,
        "normalized report contains unredacted data"
    );
    match source.format.as_str() {
        "native/1" => native(&source.document, index),
        "sarif/2.1.0" => sarif::import(&source.document, index),
        _ => anyhow::bail!("unsupported source format"),
    }
}
