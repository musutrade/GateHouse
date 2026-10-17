//! Presentation-only grouping; eligibility never changes supplied gate decisions.
pub(super) mod debt;
mod distribution;
mod ranking;
use crate::report_evidence;
use crate::verify::quality::QualityResult;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub(super) struct View<'a> {
    pub groups: Vec<Group<'a>>,
    pub panels: Vec<Panel>,
    rankings: Vec<ranking::Panel>,
    distributions: Vec<distribution::Panel>,
    debts: Vec<debt::Panel>,
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
    source_url: Option<String>,
    policy: &'a Value,
    subject: &'a Value,
    state: &'a Value,
    reason: &'a Value,
    metric: &'a Value,
    base: &'a Value,
    head: &'a Value,
    limit: &'a Value,
    visual_note: &'static str,
    range: Option<ranking::Range>,
    distribution: Option<distribution::Selection>,
    ratchet: &'a Value,
    debt: Option<debt::Selection>,
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
        source_url: None,
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
        range: None,
        distribution: None,
        ratchet: &record["ratchet"],
        debt: None,
    }
}
#[cfg(test)]
pub(super) fn project(quality: &QualityResult) -> View<'_> {
    project_with_sources(quality, None)
}
pub(super) fn project_with_sources<'a>(
    quality: &'a QualityResult,
    mut sources: Option<crate::source_links::Links<'_>>,
) -> View<'a> {
    let mut groups: BTreeMap<(String, String), Vec<Row<'_>>> = BTreeMap::new();
    let mut panels: BTreeMap<(String, String, String), Vec<Cell>> = BTreeMap::new();
    let mut rankings = ranking::Builder::default();
    let mut distributions = distribution::Builder::default();
    let mut debts = debt::Builder::default();
    let index = report_evidence::Index::new(&quality.inputs["project"], &quality.evidence);
    if let Some(gates) = quality.project_report["gates"].as_object() {
        for (position, (gate_id, gate)) in gates.iter().enumerate() {
            let component = text(&gate["record"]["component"]);
            let path = text(&gate["record"]["subject"]["path"]);
            let mut row = row(gate, position);
            if !matches!(quality.status, "measurement_error" | "not_collected") {
                row.source_url = sources.as_mut().and_then(|links| links.link(gate, &index));
            }
            row.debt = Some(debts.push(
                gate,
                !matches!(quality.status, "measurement_error" | "not_collected"),
                &index,
            ));
            row.distribution = distributions.push(
                gate,
                !matches!(quality.status, "measurement_error" | "not_collected"),
                &index,
            );
            if !matches!(quality.status, "measurement_error" | "not_collected")
                && display_identity(gate)
            {
                row.range = rankings.push(gate_id, gate, &row.anchor, &index);
            }
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
        rankings: rankings.finish(),
        distributions: distributions.finish(),
        debts: debts.finish(),
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
    pub(crate) fn combined_fixture(count: usize, commit: &str) -> QualityResult {
        let mut quality = fixture(count);
        for i in 0..count {
            let (metric, value, base, limit) = match i % 3 {
                0 => (
                    "coverage.line",
                    json!({"type":"ratio","covered":i%101,"total":100}),
                    json!({"type":"ratio","covered":80,"total":100}),
                    json!({"type":"ratio","covered":80,"total":100}),
                ),
                1 => (
                    "complexity.cyclomatic",
                    json!({"type":"count","value":i%41}),
                    json!({"type":"count","value":15}),
                    json!({"type":"count","value":10}),
                ),
                _ => (
                    "risk.crap",
                    json!({"type":"decimal","value":(i%100).to_string()}),
                    json!({"type":"decimal","value":"40"}),
                    json!({"type":"decimal","value":"30"}),
                ),
            };
            let operator = if i % 3 == 0 { "ge" } else { "le" };
            let rule = json!({"operator":operator,"limit":limit,"ratchet":{"allow_legacy_debt":true,"deny_regression":true},"on_violation":"fail"});
            let (state, annotation) =
                harness_gate::quality::ratchet::decision(&rule, &value, Some(&base)).unwrap();
            let evidence = &mut quality.evidence[i];
            evidence["series"]["metrics"] = json!([{"name":metric,"type":value["type"]}]);
            evidence["series"]["id"] =
                json!(harness_gate::quality::evidence::series_id(&evidence["series"]).unwrap());
            evidence["metrics"] = json!([{"name":metric,"value":value}]);
            evidence["context"]["commit"] = json!(commit);
            let gate = &mut quality.project_report["gates"][format!("g{i:04}")];
            gate["state"] = json!(state);
            gate["record"]["metric"] = json!(metric);
            gate["record"]["head"] = value;
            gate["record"]["base"] = base;
            gate["record"]["policy"]["limit"] = limit;
            gate["record"]["ratchet"] = annotation;
            gate["record"]["context"]["commit"] = json!(commit);
            gate["record"]["measurement_series"]["head"] = evidence["series"]["id"].clone();
        }
        quality.status = "fail";
        quality.full_quality_status = "fail";
        quality.project_report["aggregate"] = json!({"state":"fail","blockers":[{"policy":"required","subject":quality.project_report["gates"]["g0000"]["subject"],"state":"fail","reason":"Synthetic report acceptance fixture"}]});
        quality
    }
    pub(crate) fn distribution_fixture() -> QualityResult {
        let mut quality = ranking_fixture();
        let rule = json!({"operator":"le","limit":{"type":"decimal","value":"30"},"ratchet":{"allow_legacy_debt":true,"deny_regression":true},"on_violation":"fail"});
        let pairs = [
            ("10", "10"),
            ("40", "10"),
            ("40", "50"),
            ("50", "40"),
            ("40", "40"),
            ("10", "40"),
        ];
        for i in 0..24 {
            let (head, base) = pairs[i % pairs.len()];
            let decimal = json!({"type":"decimal","value":head});
            let prior = json!({"type":"decimal","value":base});
            let (_, ratchet) =
                harness_gate::quality::ratchet::decision(&rule, &decimal, Some(&prior)).unwrap();
            let value = match i {
                20 => json!({"type":"rational","numerator":1,"denominator":0}),
                21 => json!({"type":"decimal","value":"9".repeat(4097)}),
                22 => json!({"type":"count","value":50}),
                _ if i >= 12 => {
                    json!({"type":"rational","numerator":head.parse::<u32>().unwrap(),"denominator":1})
                }
                _ => decimal,
            };
            let evidence = &mut quality.evidence[i];
            evidence["series"]["metrics"] = json!([{"name":"risk.crap","type":value["type"]}]);
            evidence["series"]["id"] =
                json!(harness_gate::quality::evidence::series_id(&evidence["series"]).unwrap());
            evidence["metrics"] = json!([{"name":"risk.crap","value":value}]);
            let gate = &mut quality.project_report["gates"][format!("g{i:04}")];
            gate["record"]["metric"] = json!("risk.crap");
            gate["record"]["head"] = value;
            gate["record"]["measurement_series"]["head"] = evidence["series"]["id"].clone();
            gate["record"]["ratchet"] = ratchet;
        }
        quality.project_report["gates"]["g0023"]["record"]["evidence_links"]["head"]
            ["evidence_id"] = json!("missing");
        quality
    }
    #[test]
    fn distribution_keeps_unplotted_and_supplied_ratchets() {
        let mut quality = distribution_fixture();
        let view = project(&quality);
        assert_eq!(
            view.distributions
                .iter()
                .map(|p| p.total_gates)
                .sum::<usize>(),
            24
        );
        assert_eq!(
            view.distributions
                .iter()
                .map(|p| p.bins[5].count)
                .sum::<usize>(),
            4
        );
        for state in [
            "none",
            "new",
            "improved",
            "regressed",
            "unchanged",
            "resolved",
        ] {
            assert!(view
                .groups
                .iter()
                .flat_map(|g| &g.rows)
                .any(|r| r.ratchet["debt"] == state));
        }
        for status in ["measurement_error", "not_collected"] {
            quality.status = status;
            let view = project(&quality);
            assert_eq!(
                view.distributions
                    .iter()
                    .map(|p| p.bins[5].count)
                    .sum::<usize>(),
                24
            );
        }
    }
    pub(crate) fn ranking_fixture() -> QualityResult {
        let mut quality = fixture(120);
        let values = [
            "0",
            "10",
            "9007199254740992",
            "9007199254740993",
            "18446744073709551615",
            "18446744073709551616",
            "99999999999999999999999999999999999999999999999999",
        ];
        for i in 0..120 {
            let evidence = &mut quality.evidence[i];
            let raw = match i {
                1 => "1.5",
                2 => "1e20",
                3 => "-1",
                _ => values[i % values.len()],
            };
            let value = serde_json::json!({"type":"count","value":serde_json::from_str::<Value>(raw).unwrap()});
            evidence["series"]["metrics"] =
                serde_json::json!([{"name":"complexity.cyclomatic","type":"count"}]);
            evidence["series"]["id"] = serde_json::json!(
                harness_gate::quality::evidence::series_id(&evidence["series"]).unwrap()
            );
            evidence["metrics"] =
                serde_json::json!([{"name":"complexity.cyclomatic","value":value}]);
            let gate = &mut quality.project_report["gates"][format!("g{i:04}")];
            gate["record"]["metric"] = serde_json::json!("complexity.cyclomatic");
            gate["record"]["head"] = value;
            gate["record"]["measurement_series"]["head"] = evidence["series"]["id"].clone();
        }
        let mut duplicate = quality.project_report["gates"]["g0000"].clone();
        duplicate["policy"] = json!("duplicate-policy");
        duplicate["record"]["policy"]["id"] = json!("duplicate-policy");
        quality.project_report["gates"]["duplicate-policy-gate"] = duplicate;
        quality
    }
    #[test]
    fn ranking_keeps_exact_values_and_separate_policy_observations() {
        let quality = ranking_fixture();
        let view = project(&quality);
        assert_eq!(view.groups.iter().map(|g| g.rows.len()).sum::<usize>(), 121);
        assert_eq!(
            view.groups
                .iter()
                .flat_map(|g| &g.rows)
                .filter(|r| r.range.is_some())
                .count(),
            118
        );
        assert_eq!(
            view.rankings.iter().map(|p| p.total_gates).sum::<usize>(),
            118
        );
        assert!(view
            .rankings
            .iter()
            .any(|p| p.total_gates > 10 && p.rows.len() == 10));
        assert!(view.rankings.iter().any(|p| p.policy == "duplicate-policy"));
        assert!(view
            .rankings
            .iter()
            .flat_map(|p| &p.rows)
            .any(|r| r.value == "99999999999999999999999999999999999999999999999999"));
        assert_eq!(quality.project_report["aggregate"]["blockers"], json!([]));
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
