//! Export an already published HTML display, without collecting or re-rendering.
use super::{destination, input, publication};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::path::Path;

pub(crate) fn run(index: &Path, output: &Path, commit: &str) -> Result<bool> {
    ensure!(
        matches!(commit.len(), 40 | 64)
            && commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "export requires an exact lowercase commit identity"
    );
    let archive = input::load_archive(index)?;
    ensure!(
        archive.schema == "harness-report-history/v1" && archive.reports.len() == 1,
        "HTML export requires a v1 index with exactly one completed publication"
    );
    let report = &archive.reports[0];
    ensure!(
        report.commit == commit,
        "export commit differs from completed invocation"
    );
    let status = report.machine["status"]
        .as_str()
        .context("missing Core status")?;
    ensure!(matches!(status, "PASS" | "FAIL"), "unsupported Core status");
    let html = input::exported_html(report)?;
    let destination = destination(index, output, &archive.reports)?;
    let provenance = json!({
        "schema":"harness-html-export/v1", "commit":commit,
        "invocation_id":report.machine["invocation_id"], "core_status":status,
        "evidence_complete":true, "machine_sha256":report.machine_sha256,
        "manifest_sha256":report.manifest_sha256, "index_sha256":archive.digest,
        "html":{"path":"test_result.html","size_bytes":html.len(),"sha256":input::digest(&html)}
    });
    publication::publish_export(
        &destination,
        &html,
        &serde_json::to_vec_pretty(&provenance)?,
    )?;
    println!(
        "HTML export written to {} (Core status {status}; not a verification decision)",
        destination.display().to_string().escape_debug()
    );
    Ok(true)
}
