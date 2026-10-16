use super::projection::{Point, Selection};
use crate::utils::redaction::redact_text;

fn escape(text: &str) -> String {
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
pub(super) fn html(points: &[Point], selection: &Selection<'_>) -> String {
    let mut html = String::from("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Report history</title><style>body{font:1rem/1.5 system-ui;margin:1rem auto;padding:0 1rem;max-width:72rem;overflow-wrap:anywhere}svg{width:100%;max-height:20rem;fill:currentColor}table{border-collapse:collapse;width:100%;min-width:50rem}th,td{text-align:left;vertical-align:top;padding:.5rem;border-bottom:1px solid #888;overflow-wrap:anywhere;max-width:22rem}th:first-child{white-space:nowrap;min-width:4rem}.scroll{overflow-x:auto}:focus-visible{outline:3px solid #2684ff}summary{cursor:pointer}code{white-space:pre-wrap;overflow-wrap:anywhere}</style><h1>Completed report history</h1>");
    html.push_str(&format!("<p>Policy: <strong>{}</strong>; subject: <strong>{}</strong>; metric: <strong>{}</strong>.</p>",escape(selection.policy),escape(selection.subject),escape(selection.metric)));
    html.push_str("<p>Input sequence order, not inferred Git ancestry or elapsed time. Only samples compatible with the first plotted sample are drawn. Missing or incompatible samples break the line. Ratios use covered / total as a display coordinate; exact reported values remain below. Supplied states are not recalculated. Input pins prove selected-byte integrity, not trusted signatures.</p>");
    html.push_str(&chart(points));
    html.push_str("<div class=\"scroll\" tabindex=\"0\" role=\"region\" aria-label=\"History table; scroll horizontally\"><table><caption>Exact supplied observations (also available without JavaScript)</caption><thead><tr><th>Sample</th><th>Source</th><th>Reported state</th><th>Exact value</th><th>Comparison</th></tr></thead><tbody>");
    for (index, point) in points.iter().enumerate() {
        html.push_str(&format!("<tr><th scope=\"row\">{}</th><td><details><summary>{}</summary>Invocation: {}<br>Commit: {}</details></td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",index+1,escape(&point.timestamp),escape(&point.invocation),escape(&point.commit),escape(&point.status),escape(&point.exact),escape(if point.reason.is_empty() { "Comparable" } else { &point.reason })));
    }
    html.push_str("</tbody></table></div><p>This display never adopts a baseline, changes debt or creates a project PASS.</p></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;
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
