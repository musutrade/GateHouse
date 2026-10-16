//! Exact integer presentation order; never a quality decision.
use crate::report_evidence;
use serde::Serialize;
use serde_json::Value;
use std::{cmp::Ordering, collections::BTreeMap};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Identity {
    component: String,
    policy: String,
    series: String,
    metric: &'static str,
    value_type: &'static str,
    unit: &'static str,
}
#[derive(Serialize)]
pub(super) struct Panel {
    pub id: String,
    pub component: String,
    pub policy: String,
    pub series: String,
    pub metric: &'static str,
    pub value_type: &'static str,
    pub unit: &'static str,
    pub total_gates: usize,
    pub rows: Vec<Item>,
}
#[derive(Serialize)]
pub(super) struct Item {
    pub anchor: String,
    pub value: String,
    pub path: String,
    pub name: String,
    pub subject: String,
    pub gate: String,
}
#[derive(Serialize)]
pub(super) struct Range {
    pub panel: String,
    pub value: String,
}
#[derive(Default)]
pub(super) struct Builder {
    panels: BTreeMap<Identity, (String, Vec<Item>)>,
}
pub(super) fn integer(value: &Value) -> Option<String> {
    let raw = value.as_number()?.to_string();
    if raw.is_empty() || !raw.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let normalized = raw.trim_start_matches('0');
    Some(
        if normalized.is_empty() {
            "0"
        } else {
            normalized
        }
        .into(),
    )
}
fn compare(left: &str, right: &str) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}
fn order(left: &Item, right: &Item) -> Ordering {
    compare(&right.value, &left.value)
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.subject.cmp(&right.subject))
        .then_with(|| left.gate.cmp(&right.gate))
}
impl Builder {
    pub(super) fn push(
        &mut self,
        gate_id: &str,
        gate: &Value,
        anchor: &str,
        index: &report_evidence::Index<'_>,
    ) -> Option<Range> {
        let record = &gate["record"];
        if record["metric"] != "complexity.cyclomatic"
            || record["head"]["type"] != "count"
            || index.resolve(gate, "complexity.cyclomatic").is_err()
        {
            return None;
        }
        let value = integer(&record["head"]["value"])?;
        let key = Identity {
            component: super::text(&record["component"]),
            policy: super::text(&gate["policy"]),
            series: super::text(&record["measurement_series"]["head"]),
            metric: "complexity.cyclomatic",
            value_type: "count",
            unit: "none",
        };
        let next_id = format!("rank-panel-{}", self.panels.len());
        let (id, rows) = self
            .panels
            .entry(key)
            .or_insert_with(|| (next_id, Vec::new()));
        rows.push(Item {
            anchor: anchor.into(),
            value: value.clone(),
            path: super::text(&record["subject"]["path"]),
            name: super::text(&record["subject"]["discriminator"]),
            subject: super::text(&gate["subject"]),
            gate: gate_id.into(),
        });
        Some(Range {
            panel: id.clone(),
            value,
        })
    }
    pub(super) fn finish(self) -> Vec<Panel> {
        self.panels
            .into_iter()
            .map(|(key, (id, mut rows))| {
                let total_gates = rows.len();
                rows.sort_by(order);
                rows.truncate(10);
                Panel {
                    id,
                    component: key.component,
                    policy: key.policy,
                    series: key.series,
                    metric: key.metric,
                    value_type: key.value_type,
                    unit: key.unit,
                    total_gates,
                    rows,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_counts_keep_large_integers_and_reject_decimal_or_exponent() {
        for raw in [
            "0",
            "9007199254740993",
            "18446744073709551616",
            "99999999999999999999999999999999999999999999999999",
        ] {
            let value: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(integer(&value).as_deref(), Some(raw));
        }
        for raw in ["-1", "1.0", "1e20", "null", "\"2\""] {
            assert!(integer(&serde_json::from_str(raw).unwrap()).is_none());
        }
        assert_eq!(
            compare("9007199254740993", "9007199254740992"),
            Ordering::Greater
        );
        assert_eq!(
            compare("18446744073709551616", "18446744073709551615"),
            Ordering::Greater
        );
    }
    #[test]
    fn ties_use_source_identity_and_gate_not_insertion_order() {
        let item = |gate: &str, path: &str| Item {
            anchor: gate.into(),
            value: "18446744073709551616".into(),
            path: path.into(),
            name: "same".into(),
            subject: "subject".into(),
            gate: gate.into(),
        };
        let mut a = vec![item("b", "z.rs"), item("c", "a.rs"), item("a", "a.rs")];
        let mut b = vec![item("a", "a.rs"), item("c", "a.rs"), item("b", "z.rs")];
        a.sort_by(order);
        b.sort_by(order);
        assert_eq!(
            a.iter().map(|x| &x.gate).collect::<Vec<_>>(),
            b.iter().map(|x| &x.gate).collect::<Vec<_>>()
        );
        assert_eq!(a[0].gate, "a");
    }
}
