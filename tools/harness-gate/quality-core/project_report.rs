//! Lossless project gate tables and indexes over evaluated generic policy results.
use super::{array, error, policy, string, Result};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Build the stable shadow report from a validated project/policy and its result.
/// Gate order and evidence links are preserved; participant indexes may share gates.
pub fn report(policy_result: &Value, project: &Value, policy: &Value) -> Result<Value> {
    let mut gates = Map::new();
    let mut indexes: BTreeMap<&str, BTreeMap<String, Vec<String>>> =
        ["component", "subject", "policy", "status", "relationship"]
            .into_iter()
            .map(|name| (name, BTreeMap::new()))
            .collect();
    for (position, result) in array(&policy_result["results"]).iter().enumerate() {
        let id = format!("gate-{position:06}");
        gates.insert(id.clone(), result.clone());
        let record = &result["record"];
        let link = record
            .get("relationship")
            .filter(|v| v.is_object() && !v.as_object().unwrap().is_empty());
        let components = if let Some(link) = link {
            vec![&link["producer"], &link["consumer"]]
        } else if record["component"].as_str().is_some_and(|s| !s.is_empty()) {
            vec![&record["component"]]
        } else {
            vec![]
        };
        for (dimension, keys) in [
            ("component", components),
            ("subject", vec![&result["subject"]]),
            ("policy", vec![&result["policy"]]),
            ("status", vec![&result["state"]]),
            (
                "relationship",
                link.map_or_else(Vec::new, |l| vec![&l["id"]]),
            ),
        ] {
            for key in keys.into_iter().filter(|k| !k.is_null()) {
                indexes
                    .get_mut(dimension)
                    .unwrap()
                    .entry(string(key).into())
                    .or_default()
                    .push(id.clone());
            }
        }
    }
    // Decode on first actual component use, preserving the public Value API's
    // lazy errors for gates that are never linked to a project component.
    let mut typed_gates: BTreeMap<String, policy::AggregationGate> = BTreeMap::new();
    let mut subsets: BTreeMap<BTreeSet<String>, policy::AggregationPolicy> = BTreeMap::new();
    let mut aggregate = |ids: &[String]| -> Result<Value> {
        for id in ids {
            if !typed_gates.contains_key(id) {
                let result: policy::GateResult = serde_json::from_value(gates[id].clone())
                    .map_err(|e| error(format!("invalid gate result: {e}")))?;
                typed_gates.insert(id.clone(), result.into());
            }
        }
        let results: Vec<_> = ids.iter().map(|id| &typed_gates[id]).collect();
        let policies: BTreeSet<_> = results.iter().map(|r| r.policy().to_owned()).collect();
        if policies.is_empty() {
            return Ok(json!({"state":"not_applicable", "blockers":[]}));
        }
        if !subsets.contains_key(&policies) {
            let selected = policies.iter().map(String::as_str).collect();
            subsets.insert(
                policies.clone(),
                policy::AggregationPolicy::selected(policy, &selected)?,
            );
        }
        subsets[&policies].aggregate(results)
    };
    let mut components = Map::new();
    for component in array(&project["components"]) {
        let id = string(&component["id"]);
        let ids = indexes["component"].get(id).cloned().unwrap_or_default();
        let (cross, local): (Vec<_>, Vec<_>) = ids
            .iter()
            .cloned()
            .partition(|i| gates[i]["record"].get("relationship").is_some());
        components.insert(
            id.into(),
            json!({"gates":ids, "aggregate":aggregate(&ids)?,
            "local":aggregate(&local)?, "cross_component":aggregate(&cross)?}),
        );
    }
    Ok(
        json!({"schema":"harness-project-report/v1", "mode":"shadow", "project":project["id"],
        "aggregate":policy_result["aggregate"], "components":components, "gates":gates,
        "indexes":indexes, "policy_result":policy_result}),
    )
}

#[cfg(test)]
mod parity_tests {
    use super::super as core;
    use super::*;

    fn fixture() -> (Value, Value, Value) {
        let source: Value =
            serde_json::from_str(include_str!("../fixtures/workflow/compiler/direct.json"))
                .unwrap();
        let mut policy = source["policy"].clone();
        policy["rules"][0]["id"] = json!("z-rule");
        let mut second = policy["rules"][0].clone();
        second["id"] = json!("a-rule");
        policy["rules"].as_array_mut().unwrap().push(second);
        let gate = |id: &str, subject: &str, component: &str, state: &str| {
            json!({
            "policy":id,"subject":subject,"state":state,"reason":"retained reason",
            "record":{"component":component,"evidence_links":{"head":{"evidence_id":subject,
                "artifacts":[{"path":"raw.json","sha256":"retained"}]}}}})
        };
        let mut shared = gate("a-rule", "shared-subject", "z", "measurement_error");
        shared["record"]["relationship"] = json!({"id":"shared", "producer":"z", "consumer":"a"});
        let evaluated = json!({"aggregate":{"state":"measurement_error","blockers":[]},
            "results":[gate("z-rule","z-subject","z","fail"),
                gate("z-rule","a-subject","a","pass"), shared]});
        (
            evaluated,
            json!({"id":"example", "components":[{"id":"z"},{"id":"a"}]}),
            policy,
        )
    }

    fn parity(evaluated: &Value, project: &Value, policy: &Value) -> Result<Value> {
        let expected = report_g(evaluated, project, policy);
        let actual = report(evaluated, project, policy);
        assert_eq!(actual, expected);
        if let (Ok(actual), Ok(expected)) = (&actual, &expected) {
            assert_eq!(
                serde_json::to_vec_pretty(actual).unwrap(),
                serde_json::to_vec_pretty(expected).unwrap()
            );
        }
        actual
    }

    #[test]
    fn typed_subsets_keep_lossless_gates_sharing_order_and_null_relationship() {
        let (mut evaluated, mut project, policy) = fixture();
        let report = parity(&evaluated, &project, &policy).unwrap();
        assert_eq!(
            report["indexes"]["component"]["z"],
            json!(["gate-000000", "gate-000002"])
        );
        assert_eq!(
            report["indexes"]["component"]["a"],
            json!(["gate-000001", "gate-000002"])
        );
        assert_eq!(
            report["components"]["z"]["aggregate"]["blockers"][0]["policy"],
            "z-rule"
        );
        assert_eq!(
            report["components"]["z"]["aggregate"]["blockers"][1]["policy"],
            "a-rule"
        );
        assert_eq!(report["gates"]["gate-000002"], evaluated["results"][2]);
        evaluated["results"][0]["record"]["relationship"] = Value::Null;
        parity(&evaluated, &project, &policy).unwrap();
        evaluated["results"].as_array_mut().unwrap().reverse();
        project["components"].as_array_mut().unwrap().reverse();
        parity(&evaluated, &project, &policy).unwrap();
    }

    #[test]
    fn typed_subsets_reject_unknown_and_duplicate_without_deduplication() {
        let (evaluated, project, policy) = fixture();
        let mut bad = evaluated.clone();
        bad["results"][0]["policy"] = json!("unknown-rule");
        assert_eq!(
            parity(&bad, &project, &policy).unwrap_err().message,
            "unknown result policy"
        );
        let mut bad = evaluated.clone();
        let repeated = bad["results"][0].clone();
        bad["results"].as_array_mut().unwrap().push(repeated);
        assert_eq!(
            parity(&bad, &project, &policy).unwrap_err().message,
            "duplicate gate result"
        );
        let mut bad_policy = policy.clone();
        let repeated = bad_policy["rules"][0].clone();
        bad_policy["rules"].as_array_mut().unwrap().push(repeated);
        assert_eq!(
            parity(&evaluated, &project, &bad_policy)
                .unwrap_err()
                .message,
            "duplicate policy ID: z-rule"
        );
        let mut bad = evaluated.clone();
        bad["results"][2]["record"]["relationship"]["consumer"] = json!("z");
        assert_eq!(
            parity(&bad, &project, &policy).unwrap_err().message,
            "duplicate gate result"
        );
    }

    #[test]
    fn typed_subsets_keep_lazy_gate_errors_and_omitted_rule_boundary() {
        let (mut evaluated, project, mut policy) = fixture();
        evaluated["results"].as_array_mut().unwrap().push(json!({
            "policy":"unused","state":"invalid-state","record":{},"subject":null}));
        let mut unused = policy["rules"][0].clone();
        unused["id"] = json!("unused");
        unused["operator"] = json!("invalid-operator");
        unused["limit"]["covered"] = json!(1.5);
        policy["rules"].as_array_mut().unwrap().push(unused);
        // The unlinked malformed gate and omitted malformed rule were never validated.
        parity(&evaluated, &project, &policy).unwrap();
        let mut linked = evaluated.clone();
        linked["results"][3]["record"]["component"] = json!("z");
        assert!(parity(&linked, &project, &policy)
            .unwrap_err()
            .message
            .starts_with("invalid gate result:"));
        let empty = json!({"id":"example","components":[]});
        parity(&linked, &empty, &json!({"rules":[],"schema":"invalid"})).unwrap();
        let mut bad_policy = policy.clone();
        bad_policy["schema"] = json!("invalid");
        linked["results"][0]["reason"] = json!(17);
        assert!(parity(&linked, &project, &bad_policy)
            .unwrap_err()
            .message
            .starts_with("invalid gate result:"));
    }

    // Frozen G reference excerpts, retained only as a differential test oracle.
    fn aggregate_g(policy: &Value, results: &[policy::GateResult]) -> Result<Value> {
        core::json::domain(policy)?;
        core::schema::shape(policy, &core::schema::POLICY, None)?;
        let rules = core::index(&policy["rules"], "id", "policy ID")?;
        let mut seen = BTreeSet::new();
        let mut blockers = Vec::new();
        for result in results {
            let rule = rules
                .get(result.policy.as_str())
                .ok_or_else(|| error("unknown result policy"))?;
            core::require(
                seen.insert((result.policy.as_str(), result.subject.as_deref())),
                "duplicate gate result",
            )?;
            if rule["required"] == true
                && !matches!(
                    result.state,
                    policy::GateState::Pass | policy::GateState::Informational
                )
            {
                blockers.push(
                    json!({"policy":result.policy,"subject":result.subject,"state":result.state}),
                );
            }
        }
        // Preserve policy order; the lookup index is sorted only for lookup.
        for rule in array(&policy["rules"]) {
            if rule["required"] == true && !seen.iter().any(|(id, _)| rule["id"] == *id) {
                blockers.push(json!({"policy":rule["id"],"subject":null,"state":"blocked"}));
            }
        }
        let status = if blockers.iter().any(|b| b["state"] == "measurement_error") {
            "measurement_error"
        } else if blockers.iter().any(|b| b["state"] == "fail") {
            "fail"
        } else if !blockers.is_empty() {
            "blocked"
        } else {
            "pass"
        };
        Ok(json!({"state":status,"blockers":blockers}))
    }

    fn report_g(policy_result: &Value, project: &Value, policy: &Value) -> Result<Value> {
        let mut gates = Map::new();
        let mut indexes: BTreeMap<&str, BTreeMap<String, Vec<String>>> =
            ["component", "subject", "policy", "status", "relationship"]
                .into_iter()
                .map(|name| (name, BTreeMap::new()))
                .collect();
        for (position, result) in array(&policy_result["results"]).iter().enumerate() {
            let id = format!("gate-{position:06}");
            gates.insert(id.clone(), result.clone());
            let record = &result["record"];
            let link = record
                .get("relationship")
                .filter(|v| v.is_object() && !v.as_object().unwrap().is_empty());
            let components = if let Some(link) = link {
                vec![&link["producer"], &link["consumer"]]
            } else if record["component"].as_str().is_some_and(|s| !s.is_empty()) {
                vec![&record["component"]]
            } else {
                vec![]
            };
            for (dimension, keys) in [
                ("component", components),
                ("subject", vec![&result["subject"]]),
                ("policy", vec![&result["policy"]]),
                ("status", vec![&result["state"]]),
                (
                    "relationship",
                    link.map_or_else(Vec::new, |l| vec![&l["id"]]),
                ),
            ] {
                for key in keys.into_iter().filter(|k| !k.is_null()) {
                    indexes
                        .get_mut(dimension)
                        .unwrap()
                        .entry(string(key).into())
                        .or_default()
                        .push(id.clone());
                }
            }
        }
        let aggregate = |ids: &[String]| -> Result<Value> {
            let results: Vec<policy::GateResult> = ids
                .iter()
                .map(|id| {
                    serde_json::from_value(gates[id].clone())
                        .map_err(|e| error(format!("invalid gate result: {e}")))
                })
                .collect::<Result<_>>()?;
            let policies: BTreeSet<_> = results.iter().map(|r| r.policy.as_str()).collect();
            if policies.is_empty() {
                return Ok(json!({"state":"not_applicable", "blockers":[]}));
            }
            let mut subset = policy.clone();
            subset["rules"] = array(&policy["rules"])
                .iter()
                .filter(|r| policies.contains(string(&r["id"])))
                .cloned()
                .collect();
            aggregate_g(&subset, &results)
        };
        let mut components = Map::new();
        for component in array(&project["components"]) {
            let id = string(&component["id"]);
            let ids = indexes["component"].get(id).cloned().unwrap_or_default();
            let (cross, local): (Vec<_>, Vec<_>) = ids
                .iter()
                .cloned()
                .partition(|i| gates[i]["record"].get("relationship").is_some());
            components.insert(
                id.into(),
                json!({"gates":ids, "aggregate":aggregate(&ids)?,
            "local":aggregate(&local)?, "cross_component":aggregate(&cross)?}),
            );
        }
        Ok(
            json!({"schema":"harness-project-report/v1", "mode":"shadow", "project":project["id"],
        "aggregate":policy_result["aggregate"], "components":components, "gates":gates,
        "indexes":indexes, "policy_result":policy_result}),
        )
    }
}
