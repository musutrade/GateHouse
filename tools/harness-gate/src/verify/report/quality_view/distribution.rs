//! Display-only exact CRAP intervals, never policy thresholds or debt calculation.
use crate::report_evidence;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const FIELD_LIMIT: usize = 4096;
const LABELS: [&str; 6] = [
    "[0, 10)",
    "[10, 20)",
    "[20, 40)",
    "[40, 80)",
    "[80, +infinity)",
    "Not plotted",
];

#[derive(Serialize)]
pub(super) struct Selection {
    pub panel: String,
    pub bin: usize,
    pub note: &'static str,
}
#[derive(Serialize)]
pub(super) struct Bin {
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
    pub value_type: String,
    pub total_gates: usize,
    pub bins: Vec<Bin>,
}
type Identity = (String, String, String, String);
#[derive(Default)]
pub(super) struct Builder {
    panels: BTreeMap<Identity, Panel>,
}
fn bounded(value: &Value) -> bool {
    match value["type"].as_str() {
        Some("decimal") => value["value"]
            .as_str()
            .is_some_and(|s| s.len() <= FIELD_LIMIT),
        Some("rational") => ["numerator", "denominator"].iter().all(|key| {
            value[*key]
                .as_number()
                .is_some_and(|n| n.to_string().len() <= FIELD_LIMIT)
        }),
        _ => false,
    }
}
fn interval(value: &Value) -> Result<usize, &'static str> {
    if !bounded(value) {
        return Err("Not plotted: unsupported type or field exceeds 4096 characters");
    }
    for (bin, boundary) in [10, 20, 40, 80].iter().enumerate() {
        let limit = if value["type"] == "decimal" {
            json!({"type":"decimal","value":boundary.to_string()})
        } else {
            json!({"type":"rational","numerator":boundary,"denominator":1})
        };
        match harness_gate::quality::policy::compare(value, "lt", &limit) {
            Ok(true) => return Ok(bin),
            Ok(false) => (),
            Err(_) => return Err("Not plotted: invalid exact metric"),
        }
    }
    Ok(4)
}
fn eligible(
    gate: &Value,
    available: bool,
    index: &report_evidence::Index<'_>,
) -> Result<usize, &'static str> {
    if !available {
        return Err("Not plotted: quality measurement unavailable");
    }
    // Bound fields before the resolver parses the typed metric or compare validates it.
    let bin = interval(&gate["record"]["head"])?;
    if !super::display_identity(gate) || index.resolve(gate, "risk.crap").is_err() {
        return Err("Not plotted: evidence or identity mismatch");
    }
    Ok(bin)
}
impl Builder {
    pub(super) fn push(
        &mut self,
        gate: &Value,
        available: bool,
        index: &report_evidence::Index<'_>,
    ) -> Option<Selection> {
        let record = &gate["record"];
        if record["metric"] != "risk.crap" {
            return None;
        }
        let key = (
            super::text(&record["component"]),
            super::text(&gate["policy"]),
            super::text(&record["measurement_series"]["head"]),
            super::text(&record["head"]["type"]),
        );
        let next_id = format!("crap-panel-{}", self.panels.len());
        let panel = self.panels.entry(key.clone()).or_insert_with(|| Panel {
            id: next_id,
            component: key.0,
            policy: key.1,
            series: key.2,
            value_type: key.3,
            total_gates: 0,
            bins: LABELS
                .iter()
                .enumerate()
                .map(|(index, label)| Bin {
                    index,
                    label,
                    count: 0,
                    width: 0,
                })
                .collect(),
        });
        let (bin, note) = match eligible(gate, available, index) {
            Ok(bin) => (bin, "Exact CRAP distribution available"),
            Err(note) => (5, note),
        };
        panel.total_gates += 1;
        panel.bins[bin].count += 1;
        Some(Selection {
            panel: panel.id.clone(),
            bin,
            note,
        })
    }
    pub(super) fn finish(self) -> Vec<Panel> {
        self.panels
            .into_values()
            .map(|mut panel| {
                for bin in &mut panel.bins {
                    bin.width = bin.count * 100 / panel.total_gates;
                }
                panel
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_boundaries_and_invalid_values() {
        for (value, expected) in [
            ("0", 0),
            ("9.999999999999999999999999", 0),
            ("10", 1),
            ("20", 2),
            ("40", 3),
            ("80", 4),
        ] {
            assert_eq!(
                interval(&json!({"type":"decimal","value":value})),
                Ok(expected)
            );
        }
        assert_eq!(
            interval(&json!({"type":"rational","numerator":79,"denominator":2})),
            Ok(2)
        );
        assert!(interval(&json!({"type":"rational","numerator":1,"denominator":0})).is_err());
        assert!(interval(&json!({"type":"decimal","value":"1e2"})).is_err());
        assert!(interval(&json!({"type":"count","value":1})).is_err());
        assert!(interval(&json!({"type":"unknown","value":1})).is_err());
        for field in ["numerator", "denominator"] {
            let mut value = json!({"type":"rational","numerator":1,"denominator":1});
            value[field] = serde_json::from_str(&"9".repeat(4097)).unwrap();
            assert!(!bounded(&value));
        }
        let huge: Value = serde_json::from_str("100000000000000000000000000000000000001").unwrap();
        let denominator: Value =
            serde_json::from_str("10000000000000000000000000000000000000").unwrap();
        assert_eq!(
            interval(&json!({"type":"rational","numerator":huge,"denominator":denominator})),
            Ok(1)
        );
        assert!(interval(&json!({"type":"decimal","value":"9".repeat(4097)})).is_err());
    }
    #[test]
    fn maximum_fields_remain_bounded_and_exact() {
        let start = std::time::Instant::now();
        for _ in 0..3 {
            assert_eq!(
                interval(&json!({"type":"decimal","value":"9".repeat(4096)})),
                Ok(4)
            );
            let large: Value = serde_json::from_str(&"9".repeat(4096)).unwrap();
            assert_eq!(
                interval(&json!({"type":"rational","numerator":large,"denominator":large})),
                Ok(0)
            );
        }
        eprintln!("CRAP six maximum-field samples: {:?}", start.elapsed());
    }
}
