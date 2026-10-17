//! Counts supplied debt labels per report identity; never evaluates a ratchet.
use crate::report_evidence;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

const LABELS: [&str; 8] = [
    "none",
    "new",
    "improved",
    "regressed",
    "unchanged",
    "resolved",
    "Not available",
    "Unrecognized annotation",
];

#[derive(Serialize)]
pub(super) struct Selection {
    pub panel: String,
    pub category: usize,
    pub note: &'static str,
}
#[derive(Serialize)]
pub(super) struct Category {
    pub index: usize,
    pub label: &'static str,
    pub count: usize,
    pub width: usize,
}
#[derive(Serialize)]
pub(super) struct Panel {
    pub id: String,
    pub component: String,
    pub policy: String,
    pub series: String,
    pub metric: String,
    pub value_type: String,
    pub unit: String,
    pub total_gates: usize,
    pub categories: Vec<Category>,
}
type Identity = [Option<String>; 6];
#[derive(Default)]
pub(super) struct Builder {
    panels: BTreeMap<Identity, Panel>,
}
fn identity(gate: &Value) -> Identity {
    let record = &gate["record"];
    let unit = match record["head"]["type"].as_str() {
        Some("duration" | "size") => &record["head"]["unit"],
        Some("ratio" | "rational" | "count" | "decimal" | "boolean") => {
            &Value::String("none".into())
        }
        _ => &Value::Null,
    };
    [
        &record["component"],
        &gate["policy"],
        &record["measurement_series"]["head"],
        &record["metric"],
        &record["head"]["type"],
        unit,
    ]
    .map(|value| value.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
}
fn category(
    gate: &Value,
    available: bool,
    index: &report_evidence::Index<'_>,
) -> (usize, &'static str) {
    if !available {
        return (
            6,
            "Debt category unavailable: quality measurement unavailable",
        );
    }
    let debt = &gate["record"]["ratchet"]["debt"];
    if debt.is_null() {
        return (6, "Debt category unavailable: no supplied debt annotation");
    }
    let metric = gate["record"]["metric"].as_str().unwrap_or("");
    if !super::display_identity(gate) || index.resolve_agreement(gate, metric).is_err() {
        return (
            6,
            "Debt category unavailable: evidence or identity mismatch",
        );
    }
    match LABELS[..6].iter().position(|label| debt == *label) {
        Some(category) => (category, "Supplied debt category; no ratchet recalculation"),
        None => (7, "Debt category unrecognized; inspect supplied annotation"),
    }
}
impl Builder {
    pub(super) fn push(
        &mut self,
        gate: &Value,
        available: bool,
        index: &report_evidence::Index<'_>,
    ) -> Selection {
        let key = identity(gate);
        let next_id = format!("debt-panel-{}", self.panels.len());
        let panel = self.panels.entry(key.clone()).or_insert_with(|| {
            let display = key.map(|v| v.unwrap_or_else(|| "not available".into()));
            Panel {
                id: next_id,
                component: display[0].clone(),
                policy: display[1].clone(),
                series: display[2].clone(),
                metric: display[3].clone(),
                value_type: display[4].clone(),
                unit: display[5].clone(),
                total_gates: 0,
                categories: LABELS
                    .iter()
                    .enumerate()
                    .map(|(index, label)| Category {
                        index,
                        label,
                        count: 0,
                        width: 0,
                    })
                    .collect(),
            }
        });
        let (category, note) = category(gate, available, index);
        panel.total_gates += 1;
        panel.categories[category].count += 1;
        Selection {
            panel: panel.id.clone(),
            category,
            note,
        }
    }
    pub(super) fn finish(self) -> Vec<Panel> {
        self.panels
            .into_values()
            .map(|mut panel| {
                for category in &mut panel.categories {
                    category.width = category.count * 100 / panel.total_gates;
                }
                panel
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    pub(crate) fn fixture() -> crate::verify::quality::QualityResult {
        let mut quality = super::super::tests::fixture(10);
        let rule = json!({"operator":"le","limit":{"type":"ratio","covered":30,"total":100},"ratchet":{"allow_legacy_debt":true,"deny_regression":true},"on_violation":"fail"});
        for (i, (head, base)) in [(10, 10), (40, 10), (40, 50), (50, 40), (40, 40), (10, 40)]
            .into_iter()
            .enumerate()
        {
            let value = json!({"type":"ratio","covered":head,"total":100});
            let prior = json!({"type":"ratio","covered":base,"total":100});
            let (state, annotation) =
                harness_gate::quality::ratchet::decision(&rule, &value, Some(&prior)).unwrap();
            let gate = &mut quality.project_report["gates"][format!("g{i:04}")];
            gate["state"] = json!(state);
            gate["record"]["head"] = value.clone();
            gate["record"]["base"] = prior;
            gate["record"]["ratchet"] = annotation;
            quality.evidence[i]["metrics"][0]["value"] = value;
        }
        quality.project_report["gates"]["g0007"]["record"]["ratchet"] =
            json!({"debt":"<script>alert(1)</script> token=debt-secret"});
        quality.project_report["gates"]["g0008"]["record"]["ratchet"] = json!({"debt":"new"});
        quality.project_report["gates"]["g0008"]["record"]["evidence_links"]["head"]
            ["evidence_id"] = json!("missing");
        quality.project_report["gates"]["g0009"]["record"]["ratchet"] =
            json!({"debt":{"unexpected":"object"}});
        let mut duplicate = quality.project_report["gates"]["g0001"].clone();
        duplicate["policy"] = json!("another-policy");
        duplicate["record"]["policy"]["id"] = json!("another-policy");
        quality.project_report["gates"]["duplicate"] = duplicate;
        quality.project_report["aggregate"] = json!({"state":"fail","blockers":[{"policy":"required","subject":"visible-blocker","state":"fail"}]});
        quality
    }
    #[test]
    fn supplied_categories_reconcile_and_preserve_every_gate() {
        let mut quality = fixture();
        let original = quality.project_report.clone();
        let view = super::super::project(&quality);
        let mut totals = [0; 8];
        for panel in &view.debts {
            assert_eq!(
                panel.total_gates,
                panel.categories.iter().map(|c| c.count).sum::<usize>()
            );
            for c in &panel.categories {
                totals[c.index] += c.count;
            }
        }
        assert_eq!(totals, [1, 2, 1, 1, 1, 1, 2, 2]);
        assert_eq!(view.groups.iter().map(|g| g.rows.len()).sum::<usize>(), 11);
        assert!(view
            .debts
            .iter()
            .any(|p| p.policy == "another-policy" && p.total_gates == 1));
        assert_eq!(quality.project_report, original);
        for status in ["measurement_error", "not_collected"] {
            quality.status = status;
            assert_eq!(
                super::super::project(&quality)
                    .debts
                    .iter()
                    .map(|p| p.categories[6].count)
                    .sum::<usize>(),
                11
            );
        }
    }
    #[test]
    fn grouping_keeps_metric_type_unit_and_missing_identity_separate() {
        let gate = fixture().project_report["gates"]["g0000"].clone();
        let key = identity(&gate);
        for field in ["component", "metric"] {
            let mut other = gate.clone();
            other["record"][field] = json!("different");
            assert_ne!(identity(&other), key);
        }
        let mut other = gate.clone();
        other["record"]["head"]["type"] = json!("size");
        assert_ne!(identity(&other), key);
        let missing = identity(&other);
        other["record"]["head"]["unit"] = json!("not available");
        assert_ne!(identity(&other), missing);
        let bytes = identity(&other);
        other["record"]["head"]["unit"] = json!("bytes");
        assert_ne!(identity(&other), bytes);
        other["record"]["head"]["type"] = json!("duration");
        assert_ne!(identity(&other), key);
    }
}
