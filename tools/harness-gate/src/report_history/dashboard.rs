//! Multi-series presentation of verified reports; no policy or baseline writes.
mod impact;
use super::{input, projection, publication, query, render, Query};
use anyhow::{ensure, Context, Result};
use clap::Args;
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

#[derive(Args, Debug, Default)]
pub(crate) struct SelectionFilter {
    /// Exact policy ID; repeat to include several policies.
    #[arg(long)]
    pub policy: Vec<String>,
    /// Exact subject ID; repeat to include several subjects.
    #[arg(long)]
    pub subject: Vec<String>,
    /// Metric name; repeat for coverage.line, coverage.region, complexity.cyclomatic or risk.crap.
    #[arg(long)]
    pub metric: Vec<String>,
}
type Key = (String, String, String);
const METRICS: [&str; 4] = [
    "coverage.line",
    "coverage.region",
    "complexity.cyclomatic",
    "risk.crap",
];
fn validate(filter: &SelectionFilter) -> Result<()> {
    for values in [&filter.policy, &filter.subject, &filter.metric] {
        ensure!(
            values.len() <= 32 && values.iter().all(|s| !s.is_empty() && s.len() <= 1024),
            "dashboard filters require at most 32 nonempty identifiers of at most 1024 bytes"
        );
    }
    ensure!(
        filter.metric.iter().all(|s| METRICS.contains(&s.as_str())),
        "unsupported dashboard metric"
    );
    Ok(())
}
fn matches(values: &[String], selected: &str) -> bool {
    values.is_empty() || values.iter().any(|s| s == selected)
}
fn key(gate: &Value, filter: &SelectionFilter) -> Result<Option<Key>> {
    let fields = [&gate["policy"], &gate["subject"], &gate["record"]["metric"]];
    let Some(parts) = fields
        .iter()
        .map(|v| v.as_str())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    if !METRICS.contains(&parts[2])
        || !matches(&filter.policy, parts[0])
        || !matches(&filter.subject, parts[1])
        || !matches(&filter.metric, parts[2])
    {
        return Ok(None);
    }
    ensure!(
        parts.iter().all(|s| !s.is_empty() && s.len() <= 1024),
        "dashboard series identity is empty or too long"
    );
    Ok(Some((parts[0].into(), parts[1].into(), parts[2].into())))
}
fn catalog(reports: &[input::Completed], filter: &SelectionFilter) -> Result<BTreeSet<Key>> {
    let mut keys = BTreeSet::new();
    for report in reports {
        if let Some(gates) = report
            .machine
            .pointer("/quality/project_report/gates")
            .and_then(Value::as_object)
        {
            for gate in gates.values() {
                if let Some(key) = key(gate, filter)? {
                    keys.insert(key);
                    ensure!(keys.len() <= 12, "dashboard matches more than 12 series; narrow --policy, --subject or --metric (nothing was truncated)");
                }
            }
        }
    }
    ensure!(
        !keys.is_empty(),
        "no dashboard series matched; select a supported supplied metric"
    );
    Ok(keys)
}
fn label(reports: &[input::Completed], key: &Key) -> String {
    for report in reports.iter().rev() {
        if let Some(gates) = report
            .machine
            .pointer("/quality/project_report/gates")
            .and_then(Value::as_object)
        {
            if let Some(gate) = gates.values().find(|g| {
                g["policy"] == key.0 && g["subject"] == key.1 && g["record"]["metric"] == key.2
            }) {
                let subject = &gate["record"]["subject"];
                let path = subject["path"].as_str().unwrap_or("source unavailable");
                let name = subject["discriminator"]
                    .as_str()
                    .unwrap_or("subject unavailable");
                return format!(
                    "{}: {}",
                    path.chars().take(512).collect::<String>(),
                    name.chars().take(512).collect::<String>()
                );
            }
        }
    }
    key.1.clone()
}
fn sections(reports: &[input::Completed], keys: &BTreeSet<Key>, summary: &Value) -> Result<String> {
    let mut html = format!("<section aria-labelledby=\"history-heading\"><h2 id=\"history-heading\">Trends and observed commit changes</h2>{}<p>Each series keeps the existing strict comparison rules. Expand a series for its chart, exact observations and adjacent selected commit changes. Missing observations remain gaps. This view does not infer Git ancestry or attribute causation to a commit.</p>",render::query_description(summary));
    for (policy, subject, metric) in keys {
        let selection = projection::Selection {
            policy,
            subject,
            metric,
        };
        let points = projection::project(reports, &selection, true)?;
        html.push_str(&format!(
            "<details class=\"history-series\"><summary>{} / {} / {}</summary>",
            render::escape(policy),
            render::escape(&label(
                reports,
                &(policy.clone(), subject.clone(), metric.clone())
            )),
            render::escape(metric)
        ));
        html.push_str(&render::fragment(&points, &selection, Some(summary)));
        if metric != "risk.crap" {
            html.push_str(&render::ratchet_timeline(&points));
        }
        html.push_str(&impact::html(&points));
        html.push_str("</details>");
        ensure!(
            html.len() <= 4 * 1024 * 1024,
            "dashboard history exceeds 4 MiB; narrow selection"
        );
    }
    html.push_str("</section>");
    Ok(html)
}
pub(crate) fn run(
    index: &Path,
    output: &Path,
    filter: &SelectionFilter,
    query: &Query,
) -> Result<bool> {
    validate(filter)?;
    let archive = input::load_archive(index)?;
    let default_query = Query {
        last_commits: Some(30),
        ..Default::default()
    };
    let (reports, summary) = query::select(
        archive.reports,
        if query.active() {
            query
        } else {
            &default_query
        },
    )?;
    let keys = catalog(&reports, filter)?;
    let latest = reports.last().context("empty dashboard selection")?;
    let html =
        crate::verify::dashboard_document(&latest.machine, &sections(&reports, &keys, &summary)?)?;
    ensure!(
        html.len() <= 6 * 1024 * 1024,
        "dashboard exceeds 6 MiB; narrow selection"
    );
    let destination = super::destination(index, output, &reports)?;
    let manifest = json!({"schema":"harness-report-dashboard/v1", "index_sha256":archive.digest, "query":summary,
        "selections":keys, "latest_invocation":latest.machine["invocation_id"], "latest_core_status":latest.machine["status"],
        "html":{"path":"dashboard.html","size_bytes":html.len(),"sha256":input::digest(html.as_bytes())},
        "samples":reports.iter().map(|r| json!({"invocation_id":r.machine["invocation_id"],"commit":r.commit,"machine_sha256":r.machine_sha256,"manifest_sha256":r.manifest_sha256})).collect::<Vec<_>>()});
    publication::publish_dashboard(
        &destination,
        html.as_bytes(),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "Dashboard written to {} (not a verification decision)",
        destination.display().to_string().escape_debug()
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selectors_are_explicit_bounded_and_do_not_truncate() {
        use clap::Parser;
        let cli = crate::cli::Cli::try_parse_from([
            "harness-gate",
            "report-dashboard",
            "--input",
            "a.json",
            "--output",
            "out",
            "--metric",
            "risk.crap",
            "--metric",
            "coverage.line",
            "--last-days",
            "30",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            crate::cli::Commands::ReportDashboard { .. }
        ));
        assert!(validate(&SelectionFilter {
            metric: vec!["other".into()],
            ..Default::default()
        })
        .is_err());
        assert!(validate(&SelectionFilter {
            subject: vec!["x".repeat(1025)],
            ..Default::default()
        })
        .is_err());
        let mut gates = serde_json::Map::new();
        for i in 0..13 {
            gates.insert(
                format!("g{i}"),
                json!({"policy":"p","subject":format!("s{i}"),"record":{"metric":"coverage.line"}}),
            );
        }
        let report = input::Completed {
            machine: json!({"quality":{"project_report":{"gates":gates}}}),
            commit: "a".repeat(40),
            machine_bytes: 1,
            directory: "one".into(),
            machine_sha256: String::new(),
            manifest_sha256: String::new(),
        };
        assert!(
            catalog(std::slice::from_ref(&report), &SelectionFilter::default())
                .unwrap_err()
                .to_string()
                .contains("more than 12")
        );
        let filter = SelectionFilter {
            subject: vec!["s0".into()],
            ..Default::default()
        };
        assert_eq!(
            catalog(std::slice::from_ref(&report), &filter)
                .unwrap()
                .len(),
            1
        );
        let filter = SelectionFilter {
            policy: vec!["missing".into()],
            ..Default::default()
        };
        assert!(catalog(&[report], &filter).is_err());
    }
}
