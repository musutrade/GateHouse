//! Tool-independent contract bindings over already validated normalized evidence.
use super::{array, error, require, string, Result};
use serde_json::Value;

pub fn relationship<'a>(project: &'a Value, id: &Value) -> Result<&'a Value> {
    let matches: Vec<_> = array(&project["relationships"])
        .iter()
        .filter(|r| r["id"] == *id)
        .collect();
    require(matches.len() == 1, "unknown contract relationship")?;
    Ok(matches[0])
}

pub fn subjects<'a>(
    project: &'a Value,
    id: &Value,
    target: Option<&Value>,
) -> Result<Vec<&'a Value>> {
    let link = relationship(project, id)?;
    let selected: Vec<_> = array(&project["subjects"])
        .iter()
        .filter(|s| {
            array(&link["subjects"]).contains(&s["id"])
                && s["kind"] == "contract/v1"
                && target.is_none_or(|t| s["target"] == *t)
        })
        .collect();
    require(
        !selected.is_empty(),
        "relationship has no contract subject for target",
    )?;
    require(
        selected.iter().all(|s| s["component"] == link["producer"]),
        "contract must belong to relationship provider",
    )?;
    Ok(selected)
}

/// Validate provenance before a supported contract metric is compared.
/// Evidence validation must first check the artifact bytes and trusted context.
pub fn validate(record: &Value, rule: &Value, project: &Value) -> Result<()> {
    let identity = &rule["scope"]["relationship"];
    let link = relationship(project, identity)?;
    let binding = record
        .get("contract")
        .filter(|v| !v.is_null())
        .ok_or_else(|| error("missing contract provenance"))?;
    require(
        binding["relationship"] == *identity
            && binding["producer"] == link["producer"]
            && binding["consumer"] == link["consumer"],
        "contract participants mismatch",
    )?;
    require(
        subjects(project, identity, None)?.contains(&&record["subject"]),
        "contract subject mismatch",
    )?;
    let metric = array(&record["metrics"])
        .iter()
        .find(|m| m["name"] == rule["metric"])
        .ok_or_else(|| error("missing contract metric"))?;
    let artifact = |id: &Value| -> Result<&Value> {
        array(&record["artifacts"])
            .iter()
            .rev()
            .find(|a| a["id"] == *id && array(&metric["artifacts"]).contains(id))
            .ok_or_else(|| {
                error(format!(
                    "missing required contract artifact: {}",
                    string(id)
                ))
            })
    };
    let contract = artifact(&binding["contract_artifact"])?;
    require(
        contract["sha256"] == record["source"]["sha256"],
        "contract artifact/source digest mismatch",
    )?;
    let name = string(&rule["metric"]);
    if matches!(name, "contract.breaking_changes" | "contract.compatible") {
        let baseline = binding
            .get("baseline")
            .filter(|v| !v.is_null())
            .ok_or_else(|| error("missing contract baseline"))?;
        artifact(&baseline["artifact"])?;
        require(
            baseline["commit"] == record["context"]["base_commit"],
            "stale contract baseline commit",
        )?;
        require(
            baseline["series_id"] == record["series"]["id"],
            "incompatible contract baseline series",
        )?;
    }
    if name == "contract.compatible" {
        let consumer = binding
            .get("consumer_artifact")
            .ok_or_else(|| error("missing consumer expectation"))?;
        artifact(consumer)?;
    }
    if name == "contract.client_drift" {
        let client = binding
            .get("generated_client")
            .filter(|v| !v.is_null())
            .ok_or_else(|| error("missing generated-client evidence"))?;
        artifact(&client["artifact"])?;
        require(
            metric["value"]["value"]
                == Value::Bool(client["contract_sha256"] != contract["sha256"]),
            "generated-client drift contradicts contract digest",
        )?;
    }
    Ok(())
}
