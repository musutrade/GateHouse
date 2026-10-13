//! Baseline identity, debt and review metadata. Exceptions never waive quality.
use super::{
    array, error, evidence, index, policy::compare, project, require, schema, string, ReasonClass,
    Result,
};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub struct Lineage {
    resolved: BTreeMap<String, String>,
    kinds: BTreeMap<String, String>,
}

pub fn validate_mappings(base: &Value, head: &Value, mappings: &Value) -> Result<Lineage> {
    let validate = || -> Result<Lineage> {
        project::validate_project(base)?;
        project::validate_project(head)?;
        schema::shape(mappings, &schema::MAPPINGS, None)?;
        project::metadata(mappings)?;
        require(
            base["id"] == head["id"] && head["id"] == mappings["project"],
            "mapping project mismatch",
        )?;
        let old = index(&base["subjects"], "id", "base subject")?;
        let new = index(&head["subjects"], "id", "head subject")?;
        let mut result = Lineage::default();
        let mut used = BTreeSet::new();
        for mapping in array(&mappings["mappings"]) {
            let source = string(&mapping["from"]);
            let destinations = array(&mapping["to"]);
            require(old.contains_key(source), "unknown mapping base identity")?;
            require(used.insert(source), "ambiguous mapping source")?;
            require(
                !new.contains_key(source),
                "mapping source still exists at head",
            )?;
            let kind = string(&mapping["kind"]);
            require(
                if kind == "split" {
                    destinations.len() >= 2
                } else {
                    destinations.len() == 1
                },
                "invalid mapping cardinality",
            )?;
            for destination in destinations {
                let destination = string(destination);
                require(
                    new.contains_key(destination),
                    "unknown mapping head identity",
                )?;
                require(
                    !result.resolved.contains_key(destination) && !old.contains_key(destination),
                    "ambiguous mapping destination",
                )?;
                let before = old[source];
                let after = new[destination];
                require(
                    before["kind"] == after["kind"],
                    "mapping changes subject kind",
                )?;
                require(
                    before["target"] == after["target"],
                    "mapping changes target",
                )?;
                let same_location =
                    before["component"] == after["component"] && before["path"] == after["path"];
                match kind {
                    "modify" => require(
                        [
                            "component",
                            "target",
                            "boundary",
                            "kind",
                            "path",
                            "discriminator",
                        ]
                        .iter()
                        .all(|k| before[k] == after[k]),
                        "modification changes subject locator",
                    )?,
                    "rename" => require(
                        same_location && before["discriminator"] != after["discriminator"],
                        "invalid rename",
                    )?,
                    "move" => require(
                        !same_location && before["discriminator"] == after["discriminator"],
                        "invalid move",
                    )?,
                    _ => (),
                }
                result.resolved.insert(destination.into(), source.into());
                result.kinds.insert(destination.into(), kind.into());
            }
        }
        Ok(result)
    };
    validate().map_err(|mut e| {
        e.class = ReasonClass::ModelError;
        e
    })
}

pub fn validate_base(
    records: Option<&Value>,
    context: Option<&evidence::ValidationContext<'_>>,
    head: &evidence::ValidationContext<'_>,
    mappings: Option<&Value>,
) -> Result<Lineage> {
    let (records, context) = records
        .zip(context)
        .ok_or_else(|| error("missing base evidence or validation context"))?;
    require(
        context.project["id"] == head.project["id"],
        "base/head project mismatch",
    )?;
    require(
        context.expected["commit"] == head.expected["base_commit"],
        "base commit does not match head provenance",
    )?;
    require(
        context.expected["target"] == head.expected["target"],
        "base/head target mismatch",
    )?;
    evidence::validate_evidence(records, context)?;
    let default =
        json!({"schema":"subject-mappings/v1", "project":context.project["id"], "mappings":[]});
    validate_mappings(context.project, head.project, mappings.unwrap_or(&default))
}

pub fn metric_record<'a>(
    records: &'a [Value],
    identity: &Value,
    metric: &Value,
) -> Result<&'a Value> {
    let matching: Vec<_> = records
        .iter()
        .filter(|r| {
            r["subject"]["id"] == *identity
                && array(&r["capabilities"])
                    .iter()
                    .any(|c| c["metric"] == *metric)
        })
        .collect();
    require(
        matching.len() == 1,
        "missing or ambiguous subject metric evidence",
    )?;
    Ok(matching[0])
}

pub fn baseline<'a>(
    head: &Value,
    metric: &Value,
    records: &'a Value,
    context: &evidence::ValidationContext<'_>,
    lineage: &Lineage,
) -> Result<(Option<&'a Value>, Value)> {
    let identity = string(&head["subject"]["id"]);
    let unchanged = array(&context.project["subjects"])
        .iter()
        .any(|s| s["id"] == identity);
    let prior_id = if unchanged {
        Some(identity)
    } else {
        lineage.resolved.get(identity).map(String::as_str)
    };
    let kind = lineage.kinds.get(identity).map(String::as_str);
    let classification = if unchanged {
        "unchanged"
    } else if prior_id.is_some() && kind != Some("split") {
        "modified"
    } else {
        "new"
    };
    let prior = prior_id
        .map(|id| metric_record(array(records), &json!(id), metric))
        .transpose()?;
    if let Some(prior) = prior {
        evidence::require_compatible_series(Some(&prior["series"]), &head["series"])?;
    } else {
        require(
            array(records)
                .iter()
                .any(|r| r["component"] == head["component"] && r["series"] == head["series"]),
            "missing compatible base series for new subject",
        )?;
    }
    Ok((
        prior.filter(|_| kind != Some("split")),
        json!({"classification":classification, "lineage_kind":kind, "lineage_base_subject":prior_id, "inherits_history":prior.is_some() && kind != Some("split")}),
    ))
}

pub fn decision(rule: &Value, head: &Value, base: Option<&Value>) -> Result<(String, Value)> {
    let op = string(&rule["operator"]);
    let absolute = compare(head, op, &rule["limit"])?;
    let base_pass = base.map(|b| compare(b, op, &rule["limit"])).transpose()?;
    let trend = if let Some(base) = base {
        if compare(head, "eq", base)? {
            "unchanged"
        } else if matches!(op, "lt" | "le" | "gt" | "ge") {
            if compare(
                head,
                if matches!(op, "lt" | "le") {
                    "lt"
                } else {
                    "gt"
                },
                base,
            )? {
                "improved"
            } else {
                "regressed"
            }
        } else if Some(absolute) != base_pass {
            if absolute {
                "improved"
            } else {
                "regressed"
            }
        } else {
            "unchanged"
        }
    } else {
        "new"
    };
    let debt = if absolute {
        if base_pass == Some(false) {
            "resolved"
        } else {
            "none"
        }
    } else if base_pass == Some(false) {
        trend
    } else {
        "new"
    };
    let regression = trend == "regressed";
    let legacy = !absolute
        && base_pass == Some(false)
        && !regression
        && rule["ratchet"]["allow_legacy_debt"] == true;
    let violated =
        (!absolute && !legacy) || (regression && rule["ratchet"]["deny_regression"] == true);
    let state = if violated {
        string(&rule["on_violation"])
    } else if legacy {
        "informational"
    } else {
        "pass"
    };
    Ok((
        state.into(),
        json!({"absolute_compliant":absolute,"base_absolute_compliant":base_pass,"trend":trend,"debt":debt,"remaining_debt":!absolute,"regression":regression,"legacy_debt_allowed":legacy}),
    ))
}

fn expiry(value: &str) -> Result<DateTime<FixedOffset>> {
    // Python accepts date-only/naive ISO values, then rejects their missing timezone.
    let normalized = value.replace('Z', "+00:00");
    if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
        || chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
    {
        return Err(error("exception expiry must have timezone"));
    }
    DateTime::parse_from_rfc3339(&normalized)
        .map_err(|_| error(format!("Invalid isoformat string: '{value}'")))
}

pub fn review_exceptions(
    exceptions: &Value,
    policy: &Value,
    project: &Value,
    now: Option<&str>,
) -> Value {
    let review = || -> Result<()> {
        super::json::domain(exceptions)?;
        schema::shape(exceptions, &schema::EXCEPTIONS, None)?;
        let now = match now {
            Some(now) => expiry(now).map_err(|e| {
                if e.message == "exception expiry must have timezone" {
                    error("exception clock must have timezone")
                } else {
                    e
                }
            })?,
            None => Utc::now().fixed_offset(),
        };
        let mut seen = BTreeSet::new();
        for item in array(exceptions) {
            require(
                item.as_object()
                    .unwrap()
                    .values()
                    .all(|v| !string(v).trim().is_empty()),
                "empty exception metadata",
            )?;
            require(
                seen.insert((string(&item["policy"]), string(&item["subject"]))),
                "duplicate policy/subject exception",
            )?;
            require(
                array(&policy["rules"])
                    .iter()
                    .any(|r| r["id"] == item["policy"])
                    && array(&project["subjects"])
                        .iter()
                        .any(|s| s["id"] == item["subject"]),
                "unknown exception policy or subject",
            )?;
            require(expiry(string(&item["expiry"]))? > now, "expired exception")?;
        }
        Ok(())
    };
    let errors: Vec<_> = review().err().map(|e| e.message).into_iter().collect();
    json!({"state":if !errors.is_empty() {"invalid"} else if array(exceptions).is_empty() {"none"} else {"documented"}, "exceptions":exceptions, "errors":errors})
}
