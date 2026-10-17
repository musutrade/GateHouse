//! Deterministic queries over verified report times, not inferred Git ancestry.
use super::input::Completed;
use anyhow::{ensure, Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::Args;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Args, Debug, Default)]
pub(crate) struct Query {
    /// Select the latest N distinct supplied commits (latest invocation per commit).
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=30))]
    pub last_commits: Option<u8>,
    /// Inclusive report timestamp lower bound, RFC3339 (not Git commit time).
    #[arg(long, conflicts_with = "last_days")]
    pub since: Option<String>,
    /// Inclusive report timestamp upper bound, RFC3339.
    #[arg(long)]
    pub until: Option<String>,
    /// Days before --until or the latest verified report timestamp, inclusive.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=30))]
    pub last_days: Option<u8>,
}
impl Query {
    pub(super) fn active(&self) -> bool {
        self.last_commits.is_some()
            || self.since.is_some()
            || self.until.is_some()
            || self.last_days.is_some()
    }
}
fn timestamp(raw: &str) -> Result<DateTime<Utc>> {
    ensure!(raw.len() <= 64, "history timestamp exceeds 64 characters");
    Ok(DateTime::parse_from_rfc3339(raw)
        .context("invalid RFC3339 history timestamp")?
        .with_timezone(&Utc))
}
fn identity(report: &Completed) -> Result<(DateTime<Utc>, String)> {
    ensure!(
        matches!(report.commit.len(), 40 | 64)
            && report
                .commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "query requires a full lowercase hexadecimal invocation commit"
    );
    let time = timestamp(
        report.machine["timestamp"]
            .as_str()
            .context("missing report timestamp")?,
    )?;
    let invocation = report.machine["invocation_id"]
        .as_str()
        .context("missing invocation id")?
        .to_owned();
    Ok((time, invocation))
}
fn bounds(query: &Query, latest: DateTime<Utc>) -> Result<(Option<DateTime<Utc>>, DateTime<Utc>)> {
    ensure!(
        !(query.since.is_some() && query.last_days.is_some()),
        "since conflicts with last-days"
    );
    for value in [query.last_commits, query.last_days].into_iter().flatten() {
        ensure!(
            (1..=30).contains(&value),
            "history query count must be 1 to 30"
        );
    }
    let until = query
        .until
        .as_deref()
        .map(timestamp)
        .transpose()?
        .unwrap_or(latest);
    let since = if let Some(days) = query.last_days {
        Some(
            until
                .checked_sub_signed(Duration::days(i64::from(days)))
                .context("history time window underflow")?,
        )
    } else {
        query.since.as_deref().map(timestamp).transpose()?
    };
    ensure!(
        since.is_none_or(|start| start <= until),
        "history since is after until"
    );
    Ok((since, until))
}
pub(super) fn select(reports: Vec<Completed>, query: &Query) -> Result<(Vec<Completed>, Value)> {
    let indexed = reports.len();
    let project =
        reports.first().context("empty history archive")?.machine["project_identity"].clone();
    ensure!(
        project.as_str().is_some_and(|s| !s.is_empty()),
        "query requires project identity"
    );
    let mut ordered = Vec::with_capacity(indexed);
    for report in reports {
        ensure!(
            report.machine["project_identity"] == project,
            "query archive mixes projects"
        );
        let (time, invocation) = identity(&report)?;
        ordered.push((time, invocation, report));
    }
    ordered.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    let latest = ordered.last().context("empty history archive")?.0;
    let (since, until) = bounds(query, latest)?;
    let mut commits = BTreeMap::new();
    let mut eligible = 0;
    for (time, invocation, report) in ordered {
        if time <= until && since.is_none_or(|start| time >= start) {
            eligible += 1;
            commits.insert(report.commit.clone(), (time, invocation, report));
        }
    }
    let distinct = commits.len();
    let mut selected: Vec<_> = commits.into_values().collect();
    selected.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    if let Some(count) = query.last_commits {
        let skip = selected.len().saturating_sub(usize::from(count));
        selected.drain(..skip);
    }
    ensure!(!selected.is_empty(), "history query matched no reports");
    ensure!(
        selected.len() <= 30,
        "history query exceeds 30 commits; narrow bounds or use --last-commits"
    );
    let summary = json!({"since":since.map(|t|t.to_rfc3339()),"until":until.to_rfc3339(),"last_days":query.last_days,"last_commits":query.last_commits,"indexed_invocations":indexed,"eligible_invocations":eligible,"eligible_distinct_commits":distinct,"selected_commits":selected.len(),"ordering":"report timestamp UTC, invocation ID tie; latest invocation per commit"});
    Ok((
        selected.into_iter().map(|(_, _, report)| report).collect(),
        summary,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_accepts_query_and_rejects_conflicts_and_ranges() {
        use clap::Parser;
        let base = [
            "harness-gate",
            "report-history",
            "--input",
            "index.json",
            "--output",
            "view",
            "--policy",
            "p",
            "--subject",
            "s",
            "--metric",
            "coverage.line",
        ];
        let mut args = base.to_vec();
        args.extend(["--last-commits", "30", "--last-days", "30"]);
        let cli = crate::cli::Cli::try_parse_from(&args).unwrap();
        assert!(matches!(
            cli.command,
            crate::cli::Commands::ReportHistory {
                query: Query {
                    last_commits: Some(30),
                    last_days: Some(30),
                    ..
                },
                ..
            }
        ));
        for extra in [
            vec!["--last-commits", "31"],
            vec!["--last-days", "0"],
            vec!["--since", "2026-01-01T00:00:00Z", "--last-days", "1"],
        ] {
            let mut args = base.to_vec();
            args.extend(extra);
            assert!(crate::cli::Cli::try_parse_from(args).is_err());
        }
    }
    fn report(id: &str, commit: u8, time: &str) -> Completed {
        Completed {
            machine: json!({"invocation_id":id,"timestamp":time,"project_identity":"demo","status":"FAIL"}),
            commit: format!("{commit:040x}"),
            machine_bytes: 1,
            directory: id.into(),
            machine_sha256: "a".repeat(64),
            manifest_sha256: "b".repeat(64),
        }
    }
    #[test]
    fn selects_latest_commit_including_failure_and_utc_ties() {
        let reports = vec![
            report("z", 1, "2026-01-02T01:00:00+01:00"),
            report("old", 1, "2026-01-01T00:00:00Z"),
            report("a", 2, "2026-01-02T00:00:00Z"),
            report("a-tie", 1, "2026-01-02T00:00:00Z"),
        ];
        let (selected, summary) = select(
            reports,
            &Query {
                last_commits: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            selected
                .iter()
                .map(|r| r.machine["invocation_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        assert_eq!(selected[1].machine["status"], "FAIL");
        assert_eq!(summary["indexed_invocations"], 4);
    }
    #[test]
    fn inclusive_days_are_anchored_to_archive_and_reject_invalid_windows() {
        let reports = vec![
            report("edge", 1, "2024-02-29T00:00:00Z"),
            report("end", 2, "2024-03-01T00:00:00Z"),
        ];
        let (selected, summary) = select(
            reports,
            &Query {
                last_days: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(selected.len(), 2);
        assert_eq!(summary["since"], "2024-02-29T00:00:00+00:00");
        for query in [
            Query {
                since: Some("bad".into()),
                ..Default::default()
            },
            Query {
                since: Some("2026-01-02T00:00:00Z".into()),
                until: Some("2026-01-01T00:00:00Z".into()),
                ..Default::default()
            },
            Query {
                last_days: Some(0),
                ..Default::default()
            },
            Query {
                since: Some("2026-01-01T00:00:00Z".into()),
                last_days: Some(1),
                ..Default::default()
            },
        ] {
            assert!(select(vec![report("one", 1, "2026-01-01T00:00:00Z")], &query).is_err());
        }
    }
    #[test]
    fn rejects_empty_mixed_project_bad_identity_and_unbounded_output() {
        let q = Query {
            until: Some("2026-01-02T00:00:00Z".into()),
            ..Default::default()
        };
        assert!(select(
            (0..31)
                .map(|i| report(&format!("sample{i}"), i, "2026-01-01T00:00:00Z"))
                .collect(),
            &q
        )
        .is_err());
        let none = Query {
            since: Some("2027-01-01T00:00:00Z".into()),
            until: Some("2027-02-01T00:00:00Z".into()),
            ..Default::default()
        };
        assert!(select(vec![report("one", 1, "2026-01-01T00:00:00Z")], &none).is_err());
        let mut mixed = report("two", 2, "2026-01-01T00:00:00Z");
        mixed.machine["project_identity"] = json!("other");
        assert!(select(vec![report("one", 1, "2026-01-01T00:00:00Z"), mixed], &q).is_err());
        let mut invalid = report("one", 1, "2026-01-01T00:00:00Z");
        invalid.commit = "short".into();
        assert!(select(vec![invalid], &q).is_err());
    }
}
