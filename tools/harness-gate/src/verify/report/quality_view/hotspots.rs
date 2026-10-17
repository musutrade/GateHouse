//! Small exact rankings of supplied observations, never new severity decisions.
use crate::report_evidence;
use harness_gate::quality::policy::compare;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub(super) struct Panel {
    pub component: String,
    pub policy: String,
    pub series: String,
    pub metric: String,
    pub value_type: String,
    pub eligible: usize,
    pub unavailable: usize,
    pub rows: Vec<Item>,
}
#[derive(Serialize)]
pub(super) struct Item {
    pub anchor: String,
    pub path: String,
    pub name: String,
    pub subject: String,
    pub gate: String,
    pub value: Value,
}
type Identity = (String, String, String, String, String);
#[derive(Default)]
pub(super) struct Builder {
    panels: BTreeMap<Identity, Panel>,
}

fn bounded(value: &Value) -> bool {
    // Pairwise rational comparisons multiply both fields. Keep that work bounded;
    // larger supplied values remain exact in the detail table and distribution.
    match value["type"].as_str() {
        Some("decimal") => value["value"].as_str().is_some_and(|s| s.len() <= 128),
        Some("rational") => ["numerator", "denominator"].iter().all(|key| {
            value[*key]
                .as_number()
                .is_some_and(|n| n.to_string().len() <= 128)
        }),
        _ => false,
    }
}
fn eligible(
    gate: &Value,
    available: bool,
    index: &report_evidence::Index<'_>,
) -> Result<(), &'static str> {
    if !available || !super::display_identity(gate) {
        return Err("Hot spot unavailable: quality or source identity unavailable");
    }
    let record = &gate["record"];
    match record["metric"].as_str() {
        Some("coverage.line") if super::ratio(&record["head"]).is_some() => (),
        Some("risk.crap") if bounded(&record["head"]) => (),
        _ => return Err(
            "Hot spot unavailable: unsupported value, zero total, or field exceeds display limit",
        ),
    }
    index
        .resolve(gate, record["metric"].as_str().unwrap_or(""))
        .map_err(|_| "Hot spot unavailable: evidence or identity mismatch")?;
    Ok(())
}
fn before(left: &Item, right: &Item, metric: &str) -> anyhow::Result<bool> {
    let operator = if metric == "risk.crap" { "gt" } else { "lt" };
    if compare(&left.value, operator, &right.value)? {
        return Ok(true);
    }
    Ok(compare(&left.value, "eq", &right.value)?
        && (&left.path, &left.name, &left.subject, &left.gate)
            < (&right.path, &right.name, &right.subject, &right.gate))
}
fn insert(panel: &mut Panel, item: Item) -> anyhow::Result<()> {
    let mut position = panel.rows.len();
    for (i, existing) in panel.rows.iter().enumerate() {
        if before(&item, existing, &panel.metric)? {
            position = i;
            break;
        }
    }
    if position < 10 {
        panel.rows.insert(position, item);
        panel.rows.truncate(10);
    }
    panel.eligible += 1;
    Ok(())
}
impl Builder {
    pub(super) fn push(
        &mut self,
        id: &str,
        gate: &Value,
        anchor: &str,
        available: bool,
        index: &report_evidence::Index<'_>,
    ) -> Option<&'static str> {
        let record = &gate["record"];
        if !matches!(
            record["metric"].as_str(),
            Some("risk.crap" | "coverage.line")
        ) {
            return None;
        }
        let key = (
            super::text(&record["component"]),
            super::text(&gate["policy"]),
            super::text(&record["measurement_series"]["head"]),
            super::text(&record["metric"]),
            super::text(&record["head"]["type"]),
        );
        let panel = self.panels.entry(key.clone()).or_insert_with(|| Panel {
            component: key.0,
            policy: key.1,
            series: key.2,
            metric: key.3,
            value_type: key.4,
            eligible: 0,
            unavailable: 0,
            rows: Vec::new(),
        });
        let result = eligible(gate, available, index).and_then(|()| {
            insert(
                panel,
                Item {
                    anchor: anchor.into(),
                    path: super::text(&record["subject"]["path"]),
                    name: super::text(&record["subject"]["discriminator"]),
                    subject: super::text(&gate["subject"]),
                    gate: id.into(),
                    value: record["head"].clone(),
                },
            )
            .map_err(|_| "Hot spot unavailable: exact comparison failed")
        });
        match result {
            Ok(()) => Some("Eligible for exact hot spot ranking"),
            Err(reason) => {
                panel.unavailable += 1;
                Some(reason)
            }
        }
    }
    pub(super) fn finish(self) -> Vec<Panel> {
        self.panels.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn item(id: &str, value: Value) -> Item {
        Item {
            anchor: id.into(),
            path: "src/a.rs".into(),
            name: "f".into(),
            subject: id.into(),
            gate: id.into(),
            value,
        }
    }
    #[test]
    fn exact_order_keeps_fractional_precision_and_source_ties() {
        let a = item(
            "a",
            json!({"type":"decimal","value":"30.00000000000000001"}),
        );
        let b = item(
            "b",
            json!({"type":"decimal","value":"30.00000000000000002"}),
        );
        assert!(before(&b, &a, "risk.crap").unwrap());
        let a = item(
            "a",
            json!({"type":"rational","numerator":9007199254740993_u64,"denominator":9007199254740992_u64}),
        );
        let b = item(
            "b",
            json!({"type":"rational","numerator":1,"denominator":1}),
        );
        assert!(before(&a, &b, "risk.crap").unwrap());
        let a = item("a", json!({"type":"ratio","covered":1,"total":3}));
        let b = item("b", json!({"type":"ratio","covered":2,"total":6}));
        assert!(before(&a, &b, "coverage.line").unwrap());
        assert!(!before(&b, &a, "coverage.line").unwrap());
        assert!(before(&a, &b, "risk.crap").unwrap());
        assert!(!bounded(&json!({"type":"decimal","value":"9".repeat(129)})));
    }
    #[test]
    fn rankings_are_bounded_and_unavailable_evidence_remains_visible() {
        let mut quality = super::super::tests::fixture(120);
        let index = report_evidence::Index::new(&quality.inputs["project"], &quality.evidence);
        let mut builder = Builder::default();
        for (id, gate) in quality.project_report["gates"].as_object().unwrap() {
            builder.push(id, gate, id, true, &index);
        }
        let panels = builder.finish();
        assert_eq!(panels.iter().map(|p| p.eligible).sum::<usize>(), 120);
        assert!(panels
            .iter()
            .all(|p| p.rows.len() <= 10 && p.unavailable == 0));
        for panel in &panels {
            for pair in panel.rows.windows(2) {
                assert!(before(&pair[0], &pair[1], "coverage.line").unwrap());
            }
        }
        quality.project_report["gates"]["g0000"]["record"]["head"]["covered"] = json!(50);
        let index = report_evidence::Index::new(&quality.inputs["project"], &quality.evidence);
        let gate = &quality.project_report["gates"]["g0000"];
        let mut builder = Builder::default();
        assert!(builder
            .push("g", gate, "row", true, &index)
            .unwrap()
            .contains("mismatch"));
        let panels = builder.finish();
        assert_eq!(
            (
                panels[0].eligible,
                panels[0].unavailable,
                panels[0].rows.len()
            ),
            (0, 1, 0)
        );
        assert!(eligible(gate, false, &index).is_err());
    }
    #[test]
    fn crap_types_are_separate_and_invalid_observations_are_not_zero() {
        let quality = super::super::tests::distribution_fixture();
        let index = report_evidence::Index::new(&quality.inputs["project"], &quality.evidence);
        let mut builder = Builder::default();
        for (id, gate) in quality.project_report["gates"].as_object().unwrap() {
            builder.push(id, gate, id, true, &index);
        }
        let panels = builder.finish();
        assert_eq!(panels.iter().map(|p| p.eligible).sum::<usize>(), 20);
        assert_eq!(panels.iter().map(|p| p.unavailable).sum::<usize>(), 4);
        for panel in &panels {
            assert!(panel
                .rows
                .iter()
                .all(|row| row.value["type"] == panel.value_type));
            for pair in panel.rows.windows(2) {
                assert!(before(&pair[0], &pair[1], "risk.crap").unwrap());
            }
        }
        assert!(before(
            &item(
                "a",
                json!({"type":"rational","numerator":1,"denominator":0})
            ),
            &item(
                "b",
                json!({"type":"rational","numerator":1,"denominator":1})
            ),
            "risk.crap"
        )
        .is_err());
    }
}
