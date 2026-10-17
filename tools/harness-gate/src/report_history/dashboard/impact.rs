use super::super::{projection::Point, render::escape};
use harness_gate::quality::policy::compare;
use serde_json::Value;

fn available(point: &Point) -> bool {
    point.reason.is_empty() && (point.value.is_some() || point.crap.is_some())
}
fn value(point: &Point) -> Option<Value> {
    let value: Value = serde_json::from_str(&point.exact).ok()?;
    // Bound pairwise exact multiplication independently from the chart's limit.
    for key in ["value", "numerator", "denominator", "covered", "total"] {
        let field = &value[key];
        let length = field
            .as_str()
            .map(str::len)
            .unwrap_or_else(|| field.to_string().len());
        if !field.is_null() && length > 128 {
            return None;
        }
    }
    Some(value)
}
fn change(before: &Point, after: &Point) -> &'static str {
    if !available(before) || !available(after) || before.compatibility != after.compatibility {
        return "Not comparable: missing, unavailable or incompatible observation";
    }
    let Some((left, right)) = value(before).zip(value(after)) else {
        return "Not compared: exact field exceeds the 128-character pairwise display limit";
    };
    match compare(&right, "eq", &left) {
        Ok(true) => "Unchanged supplied metric",
        Ok(false) => match compare(&right, "gt", &left) {
            Ok(true) => "Supplied metric increased",
            Ok(false) => "Supplied metric decreased",
            Err(_) => "Not comparable: exact comparison failed",
        },
        Err(_) => "Not comparable: exact comparison failed",
    }
}
pub(super) fn html(points: &[Point]) -> String {
    let mut html = String::from("<h3>Observed commit changes</h3><p>Adjacent selected reports only, with identical comparison identity. An increase is not necessarily a regression. No numeric debt delta, new gate decision, Git diff or causal attribution is computed. Supplied states remain independent observations.</p><ul>");
    for pair in points.windows(2) {
        html.push_str(&format!("<li><details><summary>{} → {}: {}</summary><p>Supplied gate state: {} → {}.</p><p>Before: <code>{}</code><br>After: <code>{}</code></p></details></li>",escape(&pair[0].commit),escape(&pair[1].commit),change(&pair[0],&pair[1]),escape(&pair[0].status),escape(&pair[1].status),escape(&pair[0].exact),escape(&pair[1].exact)));
    }
    if points.len() < 2 {
        html.push_str(
            "<li>Two comparable selected reports are needed for an observed change.</li>",
        );
    }
    html.push_str("</ul>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn point(value: Value) -> Point {
        Point {
            invocation: "inv".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            commit: "a".repeat(40),
            status: "fail".into(),
            exact: value.to_string(),
            value: Some(0.0),
            crap: None,
            crap_grid: None,
            crap_ceiling: 1,
            ratchet: Vec::new(),
            reason: String::new(),
            compatibility: Some(json!({"same":true})),
        }
    }
    #[test]
    fn changes_are_exact_and_never_cross_comparison_gaps() {
        let a = point(json!({"type":"decimal","value":"30.00000000000000001"}));
        let mut b = point(json!({"type":"decimal","value":"30.00000000000000002"}));
        assert_eq!(change(&a, &b), "Supplied metric increased");
        assert_eq!(change(&b, &a), "Supplied metric decreased");
        assert_eq!(change(&a, &a), "Unchanged supplied metric");
        b.compatibility = Some(json!({"same":false}));
        assert!(change(&a, &b).starts_with("Not comparable"));
        b.compatibility = a.compatibility.clone();
        b.reason = "unavailable".into();
        assert!(change(&a, &b).starts_with("Not comparable"));
        let huge = point(
            json!({"type":"rational","numerator":1,"denominator":serde_json::from_str::<Value>(&"9".repeat(129)).unwrap()}),
        );
        assert!(change(&huge, &huge).starts_with("Not compared"));
        let invalid = point(json!({"type":"rational","numerator":1,"denominator":0}));
        assert!(change(&invalid, &invalid).contains("failed"));
        assert!(html(&[]).contains("Two comparable"));
        b.status = "<script>bad</script>".into();
        assert!(!html(&[a, b]).contains("<script>"));
    }
}
