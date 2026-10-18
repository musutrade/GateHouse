//! Policy identity is deliberately independent of the compiler/runtime bundle.
//! The full compilation identity continues to bind concrete candidate inputs.
use super::{compiler::TrustedState, model::*, validation::path};
use crate::config::FlowConfig;
use anyhow::{Context, Result};
use harness_gate::quality::{canonical, parse};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

pub(crate) fn digest(domain: &str, value: &Value) -> Result<String> {
    Ok(format!(
        "{domain}:sha256:{:x}",
        Sha256::digest(canonical(value)?)
    ))
}

/// Project only policy/workload intent. Presentation, delivery, operator probes,
/// caches and execution isolation are not quality rules. Keep workload commands,
/// dependency order, requiredness, parsers and waivers in this projection.
pub(crate) fn flow_policy(flow: &FlowConfig) -> Result<Value> {
    let mut value = serde_json::to_value(flow)?;
    let object = value.as_object_mut().context("workflow object")?;
    for field in [
        "doctor",
        "report_templates",
        "report_history",
        "notifications",
        "execution",
        "services",
        "service_checks",
    ] {
        object.remove(field);
    }
    if let Some(paths) = object.get_mut("paths").and_then(Value::as_object_mut) {
        paths.remove("reports");
    }
    if let Some(steps) = object.get_mut("steps").and_then(Value::as_array_mut) {
        for step in steps {
            let step = step.as_object_mut().context("workflow step")?;
            for field in [
                "label",
                "log",
                "cache",
                "timeout_secs",
                "timeout_env",
                "remove_env",
            ] {
                step.remove(field);
            }
            if let Some(runner) = step.get_mut("runner").and_then(Value::as_object_mut) {
                runner.remove("isolation");
                runner.remove("threads");
                runner.remove("threads_env");
            }
        }
    }
    Ok(value)
}

pub(crate) fn builtin_policies(root: &Path, flow: &FlowConfig) -> Result<Value> {
    let mut policies = BTreeMap::new();
    for name in [&flow.paths.audit_config, &flow.paths.secrets_config] {
        let file = path(root, name, "built-in policy", false)?;
        let pin = if file.try_exists()? {
            Some(format!(
                "sha256:{:x}",
                Sha256::digest(crate::config::read_config_source(&file)?.as_bytes())
            ))
        } else {
            None
        };
        policies.insert(name, pin);
    }
    Ok(serde_json::to_value(policies)?)
}

pub(super) fn policy_identity(
    root: &Path,
    flow: &FlowConfig,
    config: &QualityConfig,
    state: &TrustedState,
) -> Result<String> {
    let profile = &config.profiles[&state.profile];
    let mut rules = BTreeMap::new();
    let mut bindings = BTreeMap::new();
    let expectations = profile
        .collectors
        .iter()
        .map(|id| (id, &config.collectors[id].produces))
        .collect::<BTreeMap<_, _>>();
    for id in &profile.policies {
        let binding = &config.policies[id];
        let document = parse(&fs::read_to_string(path(
            root,
            &binding.policy_file,
            "policy",
            true,
        )?)?)?;
        let rule = document["rules"]
            .as_array()
            .context("policy rules")?
            .iter()
            .find(|rule| rule["id"] == binding.rule)
            .context("policy rule")?;
        rules.insert(id, rule.clone());
        bindings.insert(id, binding);
    }
    // Hash the selected original rules, before subject IDs are expanded using
    // candidate fingerprints. Neither a new run ID nor a changed source file is
    // itself a policy revision. Concrete expansion stays in compilation identity.
    digest(
        "quality-policy/v1",
        &json!({
            "flow": flow_policy(flow)?, "profile": state.profile,
            "assurance": profile.assurance, "collectors": profile.collectors,
        "collector_expectations": expectations, "builtin_policies": builtin_policies(root, flow)?,
            "components": config.components,
            "project": config.project, "subjects": config.subjects,
            "relationships": config.relationships,
            "bindings": bindings, "rules": rules, "baseline": config.baseline,
            "mappings": state.mappings, "exceptions": state.exceptions,
        }),
    )
}
