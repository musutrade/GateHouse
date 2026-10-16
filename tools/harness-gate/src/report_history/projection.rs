use super::input::Completed;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

pub(super) struct Selection<'a> {
    pub policy: &'a str,
    pub subject: &'a str,
    pub metric: &'a str,
}
pub(super) struct Point {
    pub invocation: String,
    pub timestamp: String,
    pub commit: String,
    pub status: String,
    pub exact: String,
    pub value: Option<f64>,
    pub reason: String,
    pub compatibility: Option<Value>,
}
fn text(value: &Value) -> String {
    value.as_str().unwrap_or("not available").to_owned()
}
fn gap(machine: &Value, reason: &str) -> Point {
    Point {
        invocation: text(&machine["invocation_id"]),
        timestamp: text(&machine["timestamp"]),
        commit: "not available".into(),
        status: text(&machine["status"]),
        exact: "not available".into(),
        value: None,
        reason: reason.into(),
        compatibility: None,
    }
}
fn unique<'a>(
    items: impl Iterator<Item = &'a Value>,
    description: &str,
) -> Result<Option<&'a Value>> {
    let mut items = items;
    let first = items.next();
    ensure!(items.next().is_none(), "ambiguous history {description}");
    Ok(first)
}
fn verify_subject(quality: &Value, found: &Value) -> Result<()> {
    let project_id = quality["inputs"]["project"]["id"]
        .as_str()
        .context("missing quality project ID")?;
    ensure!(
        harness_gate::quality::project::subject_id(project_id, &found["subject"])?
            == found["subject"]["id"],
        "noncanonical history subject identity"
    );
    let subjects = quality["inputs"]["project"]["subjects"]
        .as_array()
        .context("missing project subject registry")?;
    ensure!(
        unique(
            subjects
                .iter()
                .filter(|s| s["id"] == found["subject"]["id"]),
            "subject registry"
        )? == Some(&found["subject"]),
        "history subject differs from project registry"
    );
    Ok(())
}
fn evidence<'a>(quality: &'a Value, gate: &Value) -> Result<&'a Value> {
    let record = &gate["record"];
    let id = record
        .pointer("/evidence_links/head/evidence_id")
        .and_then(Value::as_str)
        .context("selected gate has no head evidence identity")?;
    let records = quality["evidence"]
        .as_array()
        .context("quality evidence must be an array")?;
    let found = unique(
        records.iter().filter(|item| item["id"] == id),
        "evidence identity",
    )?
    .context("selected gate head evidence is missing")?;
    harness_gate::quality::evidence::validate_series(&found["series"])?;
    verify_subject(quality, found)?;
    ensure!(
        found["series"]["id"] == record["measurement_series"]["head"],
        "gate and evidence series differ"
    );
    ensure!(
        found["collector"] == found["series"]["collector"],
        "evidence collector differs from series"
    );
    ensure!(
        found["context"] == record["context"],
        "gate and evidence context differ"
    );
    ensure!(
        found["subject"] == record["subject"] && found["component"] == record["component"],
        "gate and evidence subject/component differ"
    );
    ensure!(
        found["project"] == quality["inputs"]["project"]["id"],
        "quality project identity differs"
    );
    Ok(found)
}
fn numeric(value: &Value) -> Option<f64> {
    let number = value.as_u64()?;
    (number <= 9_007_199_254_740_991).then_some(number as f64)
}
fn visual(metric: &Value) -> (String, Option<f64>, &'static str) {
    let exact = metric.to_string();
    match metric["type"].as_str() {
        Some("count") => (
            exact,
            numeric(&metric["value"]),
            "count outside safe display range",
        ),
        Some("ratio") => {
            let value = numeric(&metric["covered"])
                .zip(numeric(&metric["total"]))
                .filter(|(a, b)| *b > 0.0 && a <= b)
                .map(|(a, b)| a / b);
            (
                exact,
                value,
                "ratio has zero total, invalid range or unsupported precision",
            )
        }
        _ => (exact, None, "metric type has no chart projection"),
    }
}
fn verify_configuration(machine: &Value, quality: &Value, gate: &Value) -> Result<()> {
    let record = &gate["record"];
    for field in [
        "project_identity",
        "profile",
        "input_mode",
        "configuration_digest",
    ] {
        ensure!(
            machine[field].as_str().is_some_and(|s| !s.is_empty()),
            "missing history compatibility field: {field}"
        );
    }
    ensure!(
        quality["inputs"]["project"].is_object() && quality["inputs"]["policy"].is_object(),
        "missing quality configuration identity"
    );
    ensure!(
        record["subject"]["id"] == gate["subject"] && record["policy"]["id"] == gate["policy"],
        "gate policy/subject identity differs from record"
    );
    Ok(())
}
fn verified_metric<'a>(
    quality: &'a Value,
    gate: &Value,
    selection: &Selection<'_>,
) -> Result<&'a Value> {
    let record = &gate["record"];
    let evidence = evidence(quality, gate)?;
    let metrics = evidence["metrics"]
        .as_array()
        .context("evidence metrics must be an array")?;
    let metric = unique(
        metrics.iter().filter(|m| m["name"] == selection.metric),
        "metric",
    )?
    .context("claimed metric missing from evidence")?;
    ensure!(
        metric["value"] == record["head"],
        "gate value differs from its evidence"
    );
    let contracts = evidence["series"]["metrics"]
        .as_array()
        .context("missing series metric contracts")?;
    ensure!(
        unique(
            contracts.iter().filter(|m| m["name"] == selection.metric),
            "series metric contract"
        )?
        .is_some_and(|m| m["type"] == metric["value"]["type"]),
        "metric does not match its series contract"
    );
    let _: harness_gate::quality::model::MetricValue =
        serde_json::from_value(metric["value"].clone())?;
    Ok(evidence)
}
fn selected_gate<'a>(
    project_report: &'a Value,
    selection: &Selection<'_>,
) -> Result<Option<&'a Value>> {
    ensure!(
        project_report["schema"] == "harness-project-report/v1",
        "unsupported project report schema"
    );
    let gates = project_report["gates"]
        .as_object()
        .context("quality gates must be an object")?;
    unique(
        gates.values().filter(|g| {
            g["policy"] == selection.policy
                && g["subject"] == selection.subject
                && g["record"]["metric"] == selection.metric
        }),
        "selected gate",
    )
}
fn observation(machine: &Value, record: &Value, gate: &Value, unavailable: bool) -> Point {
    let (exact, visual_value, visual_reason) = visual(&record["head"]);
    let value = if unavailable { None } else { visual_value };
    let reason = if unavailable {
        "quality unavailable or not collected"
    } else if value.is_some() {
        ""
    } else {
        visual_reason
    };
    let mut result = gap(machine, reason);
    result.commit = text(&record["context"]["commit"]);
    result.status = text(&gate["state"]);
    result.exact = exact;
    result.value = value;
    result
}
fn point(report: &Completed, selection: &Selection<'_>) -> Result<Point> {
    let machine = &report.machine;
    let quality = &machine["quality"];
    if quality.is_null() {
        return Ok(gap(machine, "quality was not collected"));
    }
    ensure!(
        quality["schema"] == "quality-verification/v1",
        "unsupported quality report schema"
    );
    let unavailable = matches!(
        quality["status"].as_str(),
        Some("not_collected" | "measurement_error")
    );
    let project_report = &quality["project_report"];
    if unavailable && project_report.is_null() {
        return Ok(gap(machine, "quality unavailable or not collected"));
    }
    let gate = selected_gate(project_report, selection)?;
    let Some(gate) = gate else {
        return Ok(gap(machine, "selected policy/subject/metric is absent"));
    };
    let record = &gate["record"];
    if record["head"].is_null()
        && record
            .pointer("/evidence_links/head")
            .is_none_or(Value::is_null)
    {
        return Ok(gap(machine, "selected head metric is unavailable"));
    }
    verify_configuration(machine, quality, gate)?;
    let evidence = verified_metric(quality, gate, selection)?;
    let mut result = observation(machine, record, gate, unavailable);
    result.compatibility = Some(
        json!({"project":machine["project_identity"],"profile":machine["profile"],"mode":machine["input_mode"],"flow":machine["configuration_digest"],"project_model":quality["inputs"]["project"],"policy_model":quality["inputs"]["policy"],"rule":record["policy"],"series":evidence["series"],"subject":record["subject"],"metric":selection.metric,"type":record["head"]["type"],"unit":record["head"]["unit"]}),
    );
    Ok(result)
}
pub(super) fn project(reports: &[Completed], selection: &Selection<'_>) -> Result<Vec<Point>> {
    let mut points = reports
        .iter()
        .map(|r| point(r, selection))
        .collect::<Result<Vec<_>>>()?;
    let baseline = points
        .iter()
        .find(|p| p.value.is_some())
        .and_then(|p| p.compatibility.clone());
    for point in &mut points {
        if point.value.is_some() && point.compatibility != baseline {
            point.value = None;
            point.reason = "comparison break: differs from first plotted sample's project, configuration, profile or measurement identity".into();
        }
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visual_coordinates_never_invent_missing_or_unsafe_values() {
        for (value, expected) in [
            (json!({"type":"count","value":0}), Some(0.0)),
            (json!({"type":"count","value":7}), Some(7.0)),
            (json!({"type":"count","value":9007199254740992_u64}), None),
            (json!({"type":"ratio","covered":1,"total":4}), Some(0.25)),
            (json!({"type":"ratio","covered":0,"total":0}), None),
            (json!({"type":"ratio","covered":3,"total":2}), None),
            (json!({"type":"ratio","covered":-1,"total":2}), None),
            (json!({"type":"duration","value":3,"unit":"seconds"}), None),
            (Value::Null, None),
        ] {
            let (exact, coordinate, _) = visual(&value);
            assert_eq!(coordinate, expected);
            assert_eq!(exact, value.to_string());
        }
    }
    #[test]
    fn claimed_evidence_requires_unique_reference_and_valid_series() {
        let gate = json!({"record":{"evidence_links":{"head":{"evidence_id":"expected"}}}});
        assert!(evidence(&json!({"evidence":[]}), &gate).is_err());
        assert!(evidence(
            &json!({"evidence":[{"id":"expected"},{"id":"expected"}]}),
            &gate
        )
        .is_err());
        assert!(evidence(&json!({"evidence":[{"id":"expected","series":{}}]}), &gate).is_err());
        assert!(evidence(&json!({"evidence":[]}), &json!({"record":{}})).is_err());
    }
}
