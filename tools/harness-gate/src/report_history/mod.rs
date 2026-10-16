//! Read-only projection of explicitly pinned completed invocation reports.
mod input;
mod projection;
mod publication;
mod render;

use anyhow::{ensure, Context, Result};
use std::path::Path;

pub(crate) fn run(
    index: &Path,
    output: &Path,
    policy: &str,
    subject: &str,
    metric: &str,
) -> Result<bool> {
    ensure!(
        [policy, subject, metric]
            .iter()
            .all(|s| !s.is_empty() && s.len() <= 1024),
        "history selection must contain bounded nonempty identifiers"
    );
    let reports = input::load(index)?;
    let selection = projection::Selection {
        policy,
        subject,
        metric,
    };
    let points = projection::project(&reports, &selection)?;
    let html = render::html(&points, &selection);
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
    let manifest = serde_json::json!({"schema":"harness-report-history-output/v1","selection":{"policy":crate::utils::redaction::redact_text(policy),"subject":crate::utils::redaction::redact_text(subject),"metric":crate::utils::redaction::redact_text(metric)},"html_sha256":input::digest(html.as_bytes()),"samples":reports.iter().map(|r|serde_json::json!({"invocation_id":r.machine["invocation_id"],"machine_sha256":r.machine_sha256,"manifest_sha256":r.manifest_sha256})).collect::<Vec<_>>()});
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
