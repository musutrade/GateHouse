use super::projection::{Point, Selection};
use crate::utils::redaction::redact_text;

pub(super) fn escape(text: &str) -> String {
    redact_text(text)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn chart(points: &[Point]) -> String {
    let max = points
        .iter()
        .filter_map(|p| p.value)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let count = points.len().saturating_sub(1).max(1) as f64;
    let mut svg = String::from("<svg viewBox=\"0 0 640 220\" role=\"img\" aria-label=\"Selected metric in input order; gaps are not connected\"><path d=\"M30 10V190H620\" fill=\"none\" stroke=\"currentColor\"/>");
    svg.push_str(&format!(
        "<text x=\"32\" y=\"16\">{max}</text><text x=\"10\" y=\"195\">0</text>"
    ));
    let mut previous: Option<(f64, f64)> = None;
    for (i, point) in points.iter().enumerate() {
        let x = 30.0 + 580.0 * i as f64 / count;
        svg.push_str(&format!(
            "<text x=\"{x:.3}\" y=\"212\" text-anchor=\"middle\" font-size=\"10\">{}</text>",
            i + 1
        ));
        if let Some(value) = point.value {
            let x = 30.0 + 580.0 * i as f64 / count;
            let y = 190.0 - 170.0 * value / max;
            if let Some((a, b)) = previous {
                svg.push_str(&format!("<path d=\"M{a:.3} {b:.3}L{x:.3} {y:.3}\" fill=\"none\" stroke=\"currentColor\"/>"));
            }
            svg.push_str(&format!(
                "<circle cx=\"{x:.3}\" cy=\"{y:.3}\" r=\"4\"><title>Sample {}: {}</title></circle>",
                i + 1,
                escape(&point.exact)
            ));
            previous = Some((x, y));
        } else {
            previous = None;
        }
    }
    svg.push_str("</svg>");
    svg
}
fn crap_chart(points: &[Point]) -> String {
    let Some(ceiling) = points
        .iter()
        .find(|p| p.crap_grid.is_some())
        .map(|p| p.crap_ceiling)
    else {
        return "<p>No comparable CRAP values within the supported chart range. Exact supplied values and gap reasons remain below.</p>".into();
    };
    let mut svg=format!("<p>CRAP display scale: 0 to {ceiling}, divided into 200 floor-quantized positions. These are display coordinates, not policy thresholds. Coincident pixels do not imply equal values; a tiny positive value may share the zero coordinate. Exact values remain in the table. Sample order is chronological for queries and input order otherwise, never severity order.</p><svg viewBox=\"0 0 640 240\" role=\"img\" aria-label=\"Exact CRAP values quantized to 200 display positions; gaps are not connected\"><path d=\"M30 10V210H620\" fill=\"none\" stroke=\"currentColor\"/><text x=\"32\" y=\"16\">{ceiling}</text><text x=\"10\" y=\"215\">0</text>");
    let count = points.len().saturating_sub(1).max(1);
    let mut previous = None;
    for (i, point) in points.iter().enumerate() {
        let x = 30 + 580 * i / count;
        svg.push_str(&format!(
            "<text x=\"{x}\" y=\"232\" text-anchor=\"middle\" font-size=\"10\">{}</text>",
            i + 1
        ));
        if let Some(grid) = point.crap_grid {
            let y = 210 - grid;
            if let Some((a, b)) = previous {
                svg.push_str(&format!(
                    "<path d=\"M{a} {b}L{x} {y}\" fill=\"none\" stroke=\"currentColor\"/>"
                ));
            }
            svg.push_str(&format!(
                "<circle cx=\"{x}\" cy=\"{y}\" r=\"4\"><title>Sample {}: exact {}</title></circle>",
                i + 1,
                escape(&point.exact)
            ));
            previous = Some((x, y));
        } else {
            previous = None;
        }
    }
    svg.push_str("</svg>");
    svg
}
pub(super) fn ratchet_timeline(points: &[Point]) -> String {
    let mut html=String::from("<h2>Supplied debt states by sample</h2><p>Discrete Core annotations, not calculated debt amounts or inferred transitions. States on unavailable or incompatible samples remain observations only. Resolved is not outstanding debt.</p><ol>");
    for (i, point) in points.iter().enumerate() {
        html.push_str(&format!(
            "<li data-ratchet-sample=\"{}\"><strong>Sample {} — {}</strong>",
            i + 1,
            i + 1,
            escape(&point.timestamp)
        ));
        if point.ratchet.is_empty() {
            html.push_str("<p>Supplied ratchet: not available</p>");
        } else {
            html.push_str("<dl>");
            for (label, value) in &point.ratchet {
                html.push_str(&format!("<dt>{label}</dt><dd>{}</dd>", escape(value)));
            }
            html.push_str("</dl>");
        }
        if !point.reason.is_empty() {
            html.push_str(&format!("<p>{}</p>", escape(&point.reason)));
        }
        html.push_str("</li>");
    }
    html.push_str("</ol>");
    html
}
pub(super) fn html(points: &[Point], selection: &Selection<'_>) -> String {
    render(points, selection, None)
}
fn render(
    points: &[Point],
    selection: &Selection<'_>,
    query: Option<&serde_json::Value>,
) -> String {
    let mut html = String::from("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Report history</title><style>body{font:1rem/1.5 system-ui;margin:1rem auto;padding:0 1rem;max-width:72rem;overflow-wrap:anywhere}svg{width:100%;max-height:20rem;fill:currentColor}table{border-collapse:collapse;width:100%;min-width:50rem}th,td{text-align:left;vertical-align:top;padding:.5rem;border-bottom:1px solid #888;overflow-wrap:anywhere;max-width:22rem}th:first-child{white-space:nowrap;min-width:4rem}.scroll{overflow-x:auto}:focus-visible{outline:3px solid #2684ff}summary{cursor:pointer}code{white-space:pre-wrap;overflow-wrap:anywhere}</style><h1>Completed report history</h1>");
    html.push_str(&fragment(points, selection, query));
    html.push_str("</html>");
    html
}
pub(super) fn fragment(
    points: &[Point],
    selection: &Selection<'_>,
    query: Option<&serde_json::Value>,
) -> String {
    let mut html = String::new();
    if let Some(summary) = query {
        html.push_str(&query_description(summary));
    }
    html.push_str(&format!("<p>Policy: <strong>{}</strong>; subject: <strong>{}</strong>; metric: <strong>{}</strong>.</p>",escape(selection.policy),escape(selection.subject),escape(selection.metric)));
    if query.is_none() {
        html.push_str("<p>Input sequence order, not inferred Git ancestry or elapsed time. ");
    } else {
        html.push_str("<p>Selected chronological sequence order, not elapsed-time spacing. ");
    }
    html.push_str("Only samples compatible with the first plotted sample are drawn. Missing or incompatible samples break the line. Ratios use covered / total as a display coordinate; exact reported values remain below. Supplied states are not recalculated. Input pins prove selected-byte integrity, not trusted signatures.</p>");
    if selection.metric == "risk.crap" {
        html.push_str(&crap_chart(points));
    } else {
        html.push_str(&chart(points));
    }
    html.push_str("<div class=\"scroll\" tabindex=\"0\" role=\"region\" aria-label=\"History table; scroll horizontally\"><table><caption>Exact supplied observations (also available without JavaScript)</caption><thead><tr><th>Sample</th><th>Source</th><th>Reported state</th><th>Exact value</th><th>Comparison</th></tr></thead><tbody>");
    for (index, point) in points.iter().enumerate() {
        html.push_str(&format!("<tr><th scope=\"row\">{}</th><td><details><summary>{}</summary>Invocation: {}<br>Commit: {}</details></td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",index+1,escape(&point.timestamp),escape(&point.invocation),escape(&point.commit),escape(&point.status),escape(&point.exact),escape(if point.reason.is_empty() { "Comparable" } else { &point.reason })));
    }
    html.push_str("</tbody></table></div>");
    if selection.metric == "risk.crap" {
        html.push_str(&ratchet_timeline(points));
    }
    html.push_str(
        "<p>This display never adopts a baseline, changes debt or creates a project PASS.</p>",
    );
    html
}

pub(super) fn query_description(summary: &serde_json::Value) -> String {
    let mut html=String::from("<h2>History query</h2><p>Reports are ordered by UTC report time, with the latest report for each commit. This is not Git ancestry or commit time. Only reports in the supplied archive are included.</p><dl>");
    for (key, label, missing) in [
        ("indexed_invocations", "Reports in archive", "Not available"),
        (
            "eligible_invocations",
            "Reports within time range",
            "Not available",
        ),
        (
            "eligible_distinct_commits",
            "Distinct commits within time range",
            "Not available",
        ),
        ("selected_commits", "Commits selected", "Not available"),
        ("since", "From (UTC, inclusive)", "No lower bound"),
        ("until", "Through (UTC, inclusive)", "Not specified"),
        ("last_days", "Requested days", "Not specified"),
        ("last_commits", "Requested maximum commits", "Not specified"),
    ] {
        let value = match &summary[key] {
            serde_json::Value::String(value) => value.clone(),
            serde_json::Value::Number(value) => value.to_string(),
            _ => missing.into(),
        };
        html.push_str(&format!("<dt>{label}</dt><dd>{}</dd>", escape(&value)));
    }
    html.push_str("</dl>");
    html
}

pub(super) fn queried_html(
    points: &[Point],
    selection: &Selection<'_>,
    summary: &serde_json::Value,
) -> String {
    render(points, selection, Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unqueried_html_matches_pre_query_bytes() {
        // SHA-256 independently derived from base130c9d93 renderer with this fixed empty selection.
        let html = html(
            &[],
            &Selection {
                policy: "p",
                subject: "s",
                metric: "m",
            },
        );
        assert_eq!(
            super::super::input::digest(html.as_bytes()),
            "06e3a4e7f65255a98582bfc7958eebd724dcc7beb45aea973d051c019ae9fb0a"
        );
    }
    #[test]
    fn query_summary_uses_readable_labels_and_safe_scalar_values() {
        let summary = serde_json::json!({"indexed_invocations":31,"eligible_invocations":31,"eligible_distinct_commits":31,"selected_commits":30,"since":null,"until":"2026-01-31T00:00:00+00:00","last_days":null,"last_commits":30});
        let html = queried_html(
            &[],
            &Selection {
                policy: "p",
                subject: "s",
                metric: "m",
            },
            &summary,
        );
        for text in [
            "Reports in archive</dt><dd>31",
            "Distinct commits within time range</dt><dd>31",
            "Commits selected</dt><dd>30",
            "No lower bound",
            "Requested days</dt><dd>Not specified",
            "Requested maximum commits</dt><dd>30",
            "2026-01-31T00:00:00+00:00",
        ] {
            assert!(html.contains(text), "missing {text}");
        }
        assert!(
            !html.contains("eligible_distinct_commits") && !html.contains("indexed_invocations")
        );
        let hostile = serde_json::json!({"since":"<img src=x> token=query-secret", "until":{"unexpected":"must-not-display"}});
        let html = query_description(&hostile);
        assert!(html.contains("&lt;img") && html.contains("[REDACTED]"));
        assert!(!html.contains("query-secret") && !html.contains("must-not-display"));
    }
    #[test]
    fn crap_gap_only_has_no_fake_chart_and_annotations_are_safe() {
        let point = Point {
            invocation: "one".into(),
            timestamp: "time".into(),
            commit: "commit".into(),
            status: "FAIL".into(),
            exact: "9007199254740993".into(),
            value: None,
            crap: None,
            crap_grid: None,
            crap_ceiling: 1,
            reason: "above chart range".into(),
            compatibility: None,
            ratchet: vec![
                ("Debt", "resolved".into()),
                ("Trend", "<img src=x> token=ratchet-secret".into()),
            ],
        };
        let output = html(
            &[point],
            &Selection {
                policy: "p",
                subject: "s",
                metric: "risk.crap",
            },
        );
        assert!(!output.contains("<svg") && !output.contains("display scale:"));
        assert!(output.contains("9007199254740993") && output.contains("<dd>resolved</dd>"));
        assert!(output.contains("&lt;img") && output.contains("[REDACTED]"));
        assert!(!output.contains("ratchet-secret") && !output.contains("<img"));
    }
    #[test]
    fn displayed_selection_is_escaped_and_redacted() {
        let html = html(
            &[],
            &Selection {
                policy: "<img src=x onerror=alert(1)>",
                subject: "token=secret-history-value",
                metric: "&\"'",
            },
        );
        assert!(html.contains("&lt;img") && html.contains("[REDACTED]"));
        assert!(!html.contains("<img") && !html.contains("secret-history-value"));
        assert!(html.contains("&amp;&quot;&#39;"));
    }
}
