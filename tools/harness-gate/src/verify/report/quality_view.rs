//! Presentation-only grouping; eligibility never changes supplied gate decisions.
use crate::report_evidence;
use crate::verify::quality::QualityResult;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub(super) struct View<'a> {
    pub groups: Vec<Group<'a>>,
    pub panels: Vec<Panel>,
}
#[derive(Serialize)]
pub(super) struct Group<'a> {
    component: String,
    path: String,
    rows: Vec<Row<'a>>,
}
#[derive(Serialize)]
struct Row<'a> {
    anchor: String,
    name: String,
    kind: String,
    span: String,
    policy: &'a Value,
    subject: &'a Value,
    state: &'a Value,
    reason: &'a Value,
    metric: &'a Value,
    base: &'a Value,
    head: &'a Value,
    limit: &'a Value,
    visual_note: &'static str,
}
#[derive(Serialize)]
pub(super) struct Panel {
    component: String,
    policy: String,
    series: String,
    cells: Vec<Cell>,
}
#[derive(Serialize)]
struct Cell {
    anchor: String,
    label: String,
    shade: u8,
    covered: u64,
    total: u64,
}
fn text(value: &Value) -> String {
    value
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("not available")
        .to_owned()
}
fn ratio(value: &Value) -> Option<(u64, u64)> {
    if value["type"] != "ratio" {
        return None;
    }
    let covered = value["covered"].as_u64()?;
    let total = value["total"].as_u64()?;
    (total > 0 && covered <= total && total <= 9_007_199_254_740_991).then_some((covered, total))
}
fn row(gate: &Value, index: usize) -> Row<'_> {
    let record = &gate["record"];
    let span = &record["subject"]["span"];
    Row {
        anchor: format!("quality-row-{index}"),
        name: text(&record["subject"]["discriminator"]),
        kind: text(&record["subject"]["kind"]),
        span: ["start_line", "start_column", "end_line", "end_column"]
            .iter()
            .map(|key| span[*key].as_u64())
            .collect::<Option<Vec<_>>>()
            .map(|v| format!("{}:{}–{}:{}", v[0], v[1], v[2], v[3]))
            .unwrap_or_else(|| "not available".into()),
        policy: &gate["policy"],
        subject: &gate["subject"],
        state: &gate["state"],
        reason: &gate["reason"],
        metric: &record["metric"],
        base: &record["base"],
        head: &record["head"],
        limit: &record["policy"]["limit"],
        visual_note: "No comparable coverage visual",
    }
}
pub(super) fn project(quality: &QualityResult) -> View<'_> {
    let mut groups: BTreeMap<(String, String), Vec<Row<'_>>> = BTreeMap::new();
    let mut panels: BTreeMap<(String, String, String), Vec<Cell>> = BTreeMap::new();
    let index = report_evidence::Index::new(&quality.inputs["project"], &quality.evidence);
    if let Some(gates) = quality.project_report["gates"].as_object() {
        for (position, gate) in gates.values().enumerate() {
            let component = text(&gate["record"]["component"]);
            let path = text(&gate["record"]["subject"]["path"]);
            let mut row = row(gate, position);
            if let Some((covered, total)) = visual(quality, gate, &index) {
                let cell = Cell {
                    anchor: row.anchor.clone(),
                    label: format!("{}: {}", path, row.name),
                    shade: ((covered as u128 * 100) / total as u128) as u8,
                    covered,
                    total,
                };
                panels
                    .entry((
                        component.clone(),
                        text(&gate["policy"]),
                        text(&gate["record"]["measurement_series"]["head"]),
                    ))
                    .or_default()
                    .push(cell);
                row.visual_note = "Coverage map available";
            }
            groups.entry((component, path)).or_default().push(row);
        }
    }
    View {
        groups: groups
            .into_iter()
            .map(|((component, path), rows)| Group {
                component,
                path,
                rows,
            })
            .collect(),
        panels: panels
            .into_iter()
            .map(|((component, policy, series), cells)| Panel {
                component,
                policy,
                series,
                cells,
            })
            .collect(),
    }
}
fn display_identity(gate: &Value) -> bool {
    let record = &gate["record"];
    gate["policy"].as_str().is_some_and(|s| !s.is_empty())
        && record["component"] == record["subject"]["component"]
        && ["component", "path", "discriminator", "kind"]
            .iter()
            .all(|field| {
                record["subject"][*field]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
            })
}
fn visual(
    quality: &QualityResult,
    gate: &Value,
    index: &report_evidence::Index<'_>,
) -> Option<(u64, u64)> {
    if matches!(
        Some(quality.status),
        Some("measurement_error" | "not_collected")
    ) || gate["record"]["metric"] != "coverage.line"
        || !display_identity(gate)
        || index.resolve(gate, "coverage.line").is_err()
    {
        return None;
    }
    ratio(&gate["record"]["head"])
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;
    pub(crate) fn fixture(count: usize) -> QualityResult {
        let version = json!({"name":"test","version":"1"});
        let mut series = json!({"id":"placeholder","name":"coverage","collector":version,"tool":version,"rule":version,"runtime":version,"target":"linux","source_identity":version,"normalization":version,"metrics":[{"name":"coverage.line","type":"ratio"}]});
        series["id"] = json!(harness_gate::quality::evidence::series_id(&series).unwrap());
        let mut subjects = Vec::new();
        let mut evidence = Vec::new();
        let mut gates = serde_json::Map::new();
        for i in 0..count {
            let component = if i % 2 == 0 { "core" } else { "ui" };
            let mut subject = json!({"id":format!("subject-identity/v1:{}", "0".repeat(64)),"identity_version":"subject-identity/v1","component":component,"target":"linux","boundary":"src","kind":"function/v1","path":format!("src/file{}.rs",i%5),"discriminator":format!("function_{i}"),"span":{"start_line":i+1,"start_column":1,"end_line":i+1,"end_column":10},"source_sha256":"a".repeat(64),"metadata":{}});
            subject["id"] =
                json!(harness_gate::quality::project::subject_id("demo", &subject).unwrap());
            let mut measured_series = series.clone();
            if i % 3 == 0 {
                measured_series["tool"]["version"] = json!("2");
                measured_series["id"] =
                    json!(harness_gate::quality::evidence::series_id(&measured_series).unwrap());
            }
            let policy = if i % 4 == 0 { "required" } else { "optional" };
            let value = json!({"type":"ratio","covered":i%101,"total":100});
            let context = json!({"commit":"a".repeat(40),"base_commit":"b".repeat(40),"target":"linux","run":"fixture"});
            let id = format!("e{i}");
            let record = json!({"component":component,"subject":subject,"metric":"coverage.line","head":value,"base":null,"context":context,"policy":{"id":policy},"measurement_series":{"head":measured_series["id"]},"evidence_links":{"head":{"evidence_id":id}}});
            gates.insert(format!("g{i:04}"),json!({"policy":policy,"subject":subject["id"],"state":if i%11==0 {"fail"} else {"pass"},"reason":"supplied outcome","record":record}));
            evidence.push(json!({"id":id,"project":"demo","component":component,"collector":measured_series["collector"],"series":measured_series,"subject":subject,"context":context,"metrics":[{"name":"coverage.line","value":value}]}));
            subjects.push(subject);
        }
        QualityResult {
            schema: "quality-verification/v1",
            status: "pass",
            phase: "complete",
            error: None,
            selection: json!({}),
            inputs: json!({"project":{"id":"demo","subjects":subjects},"policy":{}}),
            evidence: json!(evidence),
            baseline: json!({}),
            project_report: json!({"schema":"harness-project-report/v1","aggregate":{"state":"pass","blockers":[]},"gates":gates}),
            project_report_path: None,
            output: None,
            formats: Default::default(),
            evaluation_time: None,
            participation: json!({}),
            full_quality_status: "pass",
            producers: json!({}),
        }
    }
    #[test]
    fn eligibility_retains_every_gate_without_mixing_series_or_rejudging_states() {
        let mut quality = fixture(12);
        let view = project(&quality);
        assert_eq!(view.groups.iter().map(|g| g.rows.len()).sum::<usize>(), 12);
        assert_eq!(view.panels.iter().map(|p| p.cells.len()).sum::<usize>(), 12);
        assert!(view.panels.len() > 2);
        assert!(view
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .any(|r| r.state == "fail"));
        quality.project_report["gates"]["g0000"]["record"]["head"] =
            json!({"type":"ratio","covered":0,"total":0});
        quality.project_report["gates"]["g0001"]["record"]["subject"]["path"] = Value::Null;
        quality.project_report["gates"]["g0002"]["record"]["measurement_series"]["head"] =
            json!("unknown");
        quality.project_report["gates"]["g0003"]["record"]["metric"] = json!("unrelated.ratio");
        let view = project(&quality);
        assert_eq!(view.groups.iter().map(|g| g.rows.len()).sum::<usize>(), 12);
        assert_eq!(view.panels.iter().map(|p| p.cells.len()).sum::<usize>(), 8);
        assert!(view.groups.iter().any(|g| g.path == "not available"));
        quality.project_report["gates"]["g0004"]["record"]["component"] = Value::Null;
        quality.evidence[4]["component"] = Value::Null;
        assert_eq!(
            project(&quality)
                .panels
                .iter()
                .map(|p| p.cells.len())
                .sum::<usize>(),
            7
        );
        quality.status = "measurement_error";
        assert!(project(&quality).panels.is_empty());
    }
    #[test]
    fn display_ratios_are_bounded_and_missing_values_are_not_zero() {
        for value in [
            json!({"type":"ratio","covered":1,"total":0}),
            json!({"type":"ratio","covered":2,"total":1}),
            json!({"type":"ratio","covered":1,"total":9007199254740992u64}),
            json!({"type":"count","value":0}),
            Value::Null,
        ] {
            assert!(ratio(&value).is_none());
        }
        assert_eq!(
            ratio(&json!({"type":"ratio","covered":0,"total":10})),
            Some((0, 10))
        );
        assert_eq!(
            ratio(&json!({"type":"ratio","covered":10,"total":10})),
            Some((10, 10))
        );
    }
    #[test]
    fn duplicate_evidence_and_modified_series_cannot_color_a_cell() {
        let mut quality = fixture(1);
        let duplicate = quality.evidence[0].clone();
        quality.evidence.as_array_mut().unwrap().push(duplicate);
        assert!(project(&quality).panels.is_empty());
        quality.evidence.as_array_mut().unwrap().pop();
        quality.evidence[0]["series"]["tool"]["version"] = json!("tampered");
        assert!(project(&quality).panels.is_empty());
    }
}
