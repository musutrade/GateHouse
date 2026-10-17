use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;

type Unique<'a> = BTreeMap<&'a str, Option<&'a Value>>;
pub(crate) struct Index<'a> {
    evidence: Unique<'a>,
    subjects: Unique<'a>,
    series: Unique<'a>,
    project: &'a Value,
}
fn insert<'a>(map: &mut Unique<'a>, item: &'a Value) {
    if let Some(id) = item["id"].as_str() {
        map.entry(id)
            .and_modify(|v| *v = None)
            .or_insert(Some(item));
    }
}
fn get<'a>(map: &Unique<'a>, id: &Value) -> Option<&'a Value> {
    map.get(id.as_str()?).copied().flatten()
}
impl<'a> Index<'a> {
    pub(crate) fn new(project: &'a Value, records: &'a Value) -> Self {
        let mut result = Self {
            evidence: BTreeMap::new(),
            subjects: BTreeMap::new(),
            series: BTreeMap::new(),
            project,
        };
        if let Some(subjects) = result.project["subjects"].as_array() {
            for subject in subjects {
                insert(&mut result.subjects, subject);
            }
        }
        if let Some(records) = records.as_array() {
            for record in records {
                insert(&mut result.evidence, record);
                result.add_series(&record["series"]);
            }
        }
        result
    }
    fn add_series(&mut self, series: &'a Value) {
        let Some(id) = series["id"].as_str() else {
            return;
        };
        match self.series.entry(id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(
                    harness_gate::quality::evidence::validate_series(series)
                        .ok()
                        .map(|_| series),
                );
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get() != &Some(series) {
                    entry.insert(None);
                }
            }
        }
    }
    pub(crate) fn resolve(&self, gate: &Value, metric: &str) -> Result<&'a Value> {
        let evidence = self.resolve_agreement(gate, metric)?;
        let metric = named(&evidence["metrics"], metric)?;
        let _: harness_gate::quality::model::MetricValue =
            serde_json::from_str(&metric["value"].to_string())?;
        Ok(evidence)
    }
    /// Verify provenance and exact value agreement without parsing the metric's numeric domain.
    pub(crate) fn resolve_agreement(&self, gate: &Value, metric: &str) -> Result<&'a Value> {
        let record = &gate["record"];
        let evidence = get(
            &self.evidence,
            &record["evidence_links"]["head"]["evidence_id"],
        )
        .context("selected gate head evidence is missing or ambiguous")?;
        self.identity_agrees(gate, evidence)?;
        self.metric_agrees(record, evidence, metric)?;
        Ok(evidence)
    }
    fn identity_agrees(&self, gate: &Value, evidence: &Value) -> Result<()> {
        let record = &gate["record"];
        let subject = &evidence["subject"];
        let project = self.project["id"]
            .as_str()
            .context("missing quality project ID")?;
        ensure!(
            harness_gate::quality::project::subject_id(project, subject)? == subject["id"],
            "noncanonical subject identity"
        );
        ensure!(
            get(&self.subjects, &subject["id"]) == Some(subject),
            "subject differs from unique project registry"
        );
        ensure!(
            record["subject"] == *subject && gate["subject"] == subject["id"],
            "gate and evidence subject differ"
        );
        ensure!(
            record["context"] == evidence["context"],
            "gate and evidence context differ"
        );
        ensure!(
            record["component"] == evidence["component"] && evidence["project"] == project,
            "evidence project/component differs"
        );
        ensure!(
            gate["policy"] == record["policy"]["id"],
            "gate policy differs from record"
        );
        Ok(())
    }
    fn metric_agrees(&self, record: &Value, evidence: &Value, name: &str) -> Result<()> {
        let series = &evidence["series"];
        ensure!(
            get(&self.series, &record["measurement_series"]["head"]) == Some(series),
            "gate and evidence series differ or are invalid"
        );
        ensure!(
            evidence["collector"] == series["collector"],
            "evidence collector differs from series"
        );
        let metric = named(&evidence["metrics"], name)?;
        ensure!(
            metric["value"] == record["head"],
            "gate value differs from its evidence"
        );
        let contract = named(&series["metrics"], name)?;
        ensure!(
            contract["type"] == metric["value"]["type"],
            "metric does not match series contract"
        );
        Ok(())
    }
}
fn named<'a>(values: &'a Value, name: &str) -> Result<&'a Value> {
    let values = values.as_array().context("missing metric array")?;
    let mut matching = values.iter().filter(|item| item["name"] == name);
    let value = matching.next().context("claimed metric missing")?;
    ensure!(matching.next().is_none(), "ambiguous metric");
    Ok(value)
}
