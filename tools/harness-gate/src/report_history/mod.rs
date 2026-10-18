//! Read-only projection of explicitly pinned completed invocation reports.
mod archive;
pub(crate) use archive::{archive_path, record};
mod crap;
mod dashboard;
pub(crate) use dashboard::{run as dashboard, SelectionFilter};
mod export;
pub(crate) use export::run as export_html;
mod input;
mod projection;
mod publication;
mod query;
mod render;
pub(crate) use query::Query;

use anyhow::{ensure, Context, Result};
use std::path::Path;

/// Read a historical result using the same pinned evidence resolver as reports.
/// Success means integrity verification, not a new quality decision or current applicability.
pub(crate) fn verify_saved(path: &Path) -> Result<serde_json::Value> {
    let (root, index, _) = input::index(path)?;
    ensure!(
        index.samples.len() == 1,
        "report-verify requires exactly one invocation"
    );
    let report = input::completed(&root, index.samples.into_iter().next().unwrap())?;
    let invocation = input::invocation_id(&report.machine)?;
    Ok(serde_json::json!({
        "historical_integrity": "verified",
        "current_applicability": "not_evaluated",
        "invocation_id": invocation,
        "machine_sha256": report.machine_sha256,
        "manifest_sha256": report.manifest_sha256,
        "result": report.machine,
    }))
}

#[cfg(test)]
pub(crate) fn run(
    index: &Path,
    output: &Path,
    policy: &str,
    subject: &str,
    metric: &str,
) -> Result<bool> {
    run_query(index, output, policy, subject, metric, &Query::default())
}

pub(crate) fn run_query(
    index: &Path,
    output: &Path,
    policy: &str,
    subject: &str,
    metric: &str,
    query: &Query,
) -> Result<bool> {
    ensure!(
        [policy, subject, metric]
            .iter()
            .all(|s| !s.is_empty() && s.len() <= 1024),
        "history selection must contain bounded nonempty identifiers"
    );
    let archive = input::load_archive(index)?;
    let active = query.active();
    ensure!(
        active || archive.schema == "harness-report-history/v1",
        "v2 archive requires explicit query options"
    );
    let (reports, query_summary) = if active {
        let (reports, summary) = query::select(archive.reports, query)?;
        (reports, Some(summary))
    } else {
        (archive.reports, None)
    };
    let selection = projection::Selection {
        policy,
        subject,
        metric,
    };
    let html = render_reports(&reports, &selection, query_summary.as_ref())?;
    let destination = destination(index, output, &reports)?;
    let mut manifest = serde_json::json!({"schema":"harness-report-history-output/v1","selection":{"policy":crate::utils::redaction::redact_text(policy),"subject":crate::utils::redaction::redact_text(subject),"metric":crate::utils::redaction::redact_text(metric)},"html_sha256":input::digest(html.as_bytes()),"samples":reports.iter().map(|r|serde_json::json!({"invocation_id":r.machine["invocation_id"],"machine_sha256":r.machine_sha256,"manifest_sha256":r.manifest_sha256})).collect::<Vec<_>>()});
    if let Some(summary) = query_summary {
        manifest["schema"] = serde_json::json!("harness-report-history-output/v2");
        manifest["query"] = summary;
        manifest["index_sha256"] = serde_json::json!(archive.digest);
        manifest["samples"] = serde_json::json!(reports.iter().map(|r|serde_json::json!({"invocation_id":r.machine["invocation_id"],"commit":r.commit,"timestamp":r.machine["timestamp"],"machine_sha256":r.machine_sha256,"manifest_sha256":r.manifest_sha256})).collect::<Vec<_>>());
    }
    publication::publish(
        &destination,
        html.as_bytes(),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "History display written to {} (not a verification decision)",
        destination.display().to_string().escape_debug()
    );
    Ok(true)
}

fn render_reports(
    reports: &[input::Completed],
    selection: &projection::Selection<'_>,
    summary: Option<&serde_json::Value>,
) -> Result<String> {
    let points = projection::project(reports, selection, summary.is_some())?;
    Ok(match summary {
        Some(summary) => render::queried_html(&points, selection, summary),
        None => render::html(&points, selection),
    })
}
fn destination(
    index: &Path,
    output: &Path,
    reports: &[input::Completed],
) -> Result<std::path::PathBuf> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let destination = parent.join(
        output
            .file_name()
            .context("history output directory name")?,
    );
    let input_root = std::path::absolute(index)?
        .parent()
        .context("history index parent")?
        .canonicalize()?;
    ensure!(
        !destination.starts_with(&input_root),
        "history output must be outside the index input tree"
    );
    ensure!(
        !reports
            .iter()
            .any(|r| destination.starts_with(&r.directory)),
        "history output cannot be inside an input invocation"
    );
    Ok(destination)
}
