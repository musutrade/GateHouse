use super::model::*;
use anyhow::{ensure, Result};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn initial(report: &Report) -> Vec<Record> {
    let mut seen = BTreeSet::new();
    report
        .findings
        .iter()
        .filter(|f| seen.insert(f.id.clone()))
        .map(|finding| Record {
            finding: finding.clone(),
            state: "new".into(),
            first_seen: report.context.commit.clone(),
            last_seen: report.context.commit.clone(),
            evidence: snapshot(&report.sources[finding.source], finding),
        })
        .collect()
}

pub(super) fn validate(report: &Report, require_stable: bool) -> Result<()> {
    let mut ids = BTreeSet::new();
    let facts: BTreeSet<_> = report
        .findings
        .iter()
        .map(serde_json::to_vec)
        .collect::<std::result::Result<_, _>>()?;

    for record in &report.history {
        let mut evidence = super::source_findings(&record.evidence, record.finding.source)?;
        ensure!(
            evidence.len() == 1,
            "historical evidence must contain one observation"
        );
        evidence[0].pointer = record.finding.pointer.clone();
        ensure!(
            serde_json::to_vec(&evidence[0])? == serde_json::to_vec(&record.finding)?,
            "historical provenance mismatch"
        );

        ensure!(
            ids.insert(record.finding.id.clone()),
            "duplicate history identity"
        );
        ensure!(
            (!require_stable || record.finding.stable)
                && hex(&record.first_seen, 40)
                && hex(&record.last_seen, 40),
            "history requires stable fingerprints and exact commits"
        );
        ensure!(
            matches!(
                record.state.as_str(),
                "new" | "persistent" | "resolved" | "reopened"
            ),
            "unknown history state"
        );
    }
    ensure!(
        report
            .previous_sha256
            .as_ref()
            .is_none_or(|pin| hex(pin, 64)),
        "invalid history parent digest"
    );
    if report.previous_sha256.is_none() {
        ensure!(
            report.history.iter().all(|r| r.state == "new"
                && r.first_seen == report.context.commit
                && r.last_seen == report.context.commit),
            "fresh report must contain initial history"
        );
    }
    let active: BTreeSet<_> = report
        .history
        .iter()
        .filter(|r| r.state != "resolved")
        .map(|r| &r.finding.id)
        .collect();
    for record in report.history.iter().filter(|r| r.state != "resolved") {
        ensure!(
            record.last_seen == report.context.commit
                && facts.contains(&serde_json::to_vec(&record.finding)?),
            "active history/provenance mismatch"
        );
    }
    let current: BTreeSet<_> = report.findings.iter().map(|f| &f.id).collect();
    ensure!(
        active == current && (!require_stable || report.findings.iter().all(|f| f.stable)),
        "history/current findings mismatch or unstable identity"
    );
    Ok(())
}

pub(super) fn compare(mut current: Report, previous: Report, pin: &str) -> Result<Report> {
    validate(&current, true)?;
    validate(&previous, true)?;
    ensure!(
        current.previous_sha256.is_none(),
        "current report is already compared"
    );
    ensure!(
        current.context.repository == previous.context.repository
            && current.context.series == previous.context.series,
        "incompatible repository/measurement series; history cannot be reset"
    );
    // Measurement IDs identify individual runs. Tool/version/format describe
    // comparable coverage; do not resolve findings when a source disappears.
    ensure!(
        current.sources.len() == previous.sources.len()
            && current
                .sources
                .iter()
                .zip(&previous.sources)
                .all(|(a, b)| a.complete
                    && b.complete
                    && a.format == b.format
                    && source_tools(a) == source_tools(b)),
        "incompatible source coverage; cannot infer resolution"
    );
    let mut records: BTreeMap<_, _> = previous
        .history
        .into_iter()
        .map(|r| (r.finding.id.clone(), r))
        .collect();
    let active: BTreeSet<_> = current.findings.iter().map(|f| f.id.clone()).collect();
    for record in records.values_mut() {
        if !active.contains(&record.finding.id) {
            record.state = "resolved".into();
        }
    }
    let mut seen = BTreeSet::new();
    for finding in current
        .findings
        .iter()
        .filter(|f| seen.insert(f.id.clone()))
    {
        let existed = records.contains_key(&finding.id);
        let record = records.entry(finding.id.clone()).or_insert_with(|| Record {
            finding: finding.clone(),
            state: "new".into(),
            first_seen: current.context.commit.clone(),
            last_seen: current.context.commit.clone(),
            evidence: snapshot(&current.sources[finding.source], finding),
        });
        record.state = if !existed {
            "new"
        } else if record.state == "resolved" {
            "reopened"
        } else {
            "persistent"
        }
        .into();
        record.finding = finding.clone();
        record.evidence = snapshot(&current.sources[finding.source], finding);
        record.last_seen = current.context.commit.clone();
    }
    ensure!(
        records.len() <= super::MAX_FINDINGS,
        "history limit exceeded; archive explicitly without resetting lineage"
    );
    current.history = records.into_values().collect();
    current.previous_sha256 = Some(pin.to_string());
    Ok(current)
}

fn source_tools(source: &Source) -> serde_json::Value {
    match source.format.as_str() {
        "native/1" => serde_json::json!([source.document["tool"], source.document["tool_version"]]),
        _ => serde_json::json!(source.document["runs"]
            .as_array()
            .map(|runs| runs.iter().map(|r| r["tool"].clone()).collect::<Vec<_>>())),
    }
}

// Store only this observation and its source/run metadata. Copying a complete
// N-result document into every history record would use quadratic space.
fn snapshot(source: &Source, finding: &Finding) -> Source {
    fn without(value: &serde_json::Value, key: &str) -> serde_json::Value {
        serde_json::Value::Object(
            value
                .as_object()
                .expect("validated source object")
                .iter()
                .filter(|(name, _)| name.as_str() != key)
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
        )
    }
    let result = source
        .document
        .pointer(&finding.pointer)
        .expect("validated result pointer")
        .clone();
    let document = if source.format == "native/1" {
        let mut document = without(&source.document, "findings");
        document["findings"] = serde_json::json!([result]);
        document
    } else {
        let run_index: usize = finding
            .pointer
            .split('/')
            .nth(2)
            .expect("SARIF pointer")
            .parse()
            .expect("SARIF run index");
        let mut run = without(&source.document["runs"][run_index], "results");
        run["results"] = serde_json::json!([result]);
        let mut document = without(&source.document, "runs");
        document["runs"] = serde_json::json!([run]);
        document
    };
    Source {
        document,
        format: source.format.clone(),
        measurement_id: source.measurement_id.clone(),
        complete: source.complete,
    }
}
