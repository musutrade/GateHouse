//! Read policy-owned participation requirements; never evaluate thresholds here.
use super::{model::*, validation::path};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Deserialize)]
struct PolicyDocument {
    schema: String,
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct Rule {
    id: String,
    metric: String,
    required: bool,
    scope: serde_json::Value,
    ratchet: Option<Ratchet>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ratchet {
    deny_regression: bool,
    allow_legacy_debt: bool,
}

pub(super) fn validate_binding(
    config: &QualityConfig,
    binding: &PolicyBinding,
    root: &Path,
) -> Result<bool> {
    let file = path(root, &binding.policy_file, "policy_file", true)?;
    let source = crate::config::read_config_source(&file).context("read policy_file")?;
    let value = harness_gate::quality::parse(&source)?;
    harness_gate::quality::policy::validate_policy_document(&value)?;
    let document: PolicyDocument = serde_json::from_value(value)
        .context("read harness-policy/v1 participation requirements")?;
    ensure!(
        document.schema == "harness-policy/v1",
        "policy_file must use harness-policy/v1"
    );
    let mut ids = BTreeSet::new();
    ensure!(
        document
            .rules
            .iter()
            .all(|rule| !rule.id.is_empty() && ids.insert(&rule.id)),
        "policy_file has empty or duplicate rule IDs"
    );
    let rule = document
        .rules
        .iter()
        .find(|rule| rule.id == binding.rule)
        .with_context(|| format!("unknown policy rule {}", binding.rule))?;
    ensure!(
        rule.metric == binding.expectation.capability,
        "policy rule capability does not match binding"
    );
    let (kind, key, id) = match &binding.expectation.target {
        Target::Component { id } => ("component", "component", id),
        Target::Subject { id } => ("subject", "subject", id),
        Target::Relationship { id } => ("relationship", "relationship", id),
    };
    ensure!(
        rule.scope["kind"] == kind && rule.scope[key].as_str() == Some(id),
        "policy rule scope does not match binding target"
    );
    if let Some(ratchet) = &rule.ratchet {
        ensure!(
            kind != "relationship",
            "relationship policies cannot use debt ratchets"
        );
        if ratchet.deny_regression || ratchet.allow_legacy_debt {
            ensure!(
                config.baseline.required
                    && !matches!(config.baseline.provider, BaselineProvider::None),
                "ratchet policy requires a required baseline provider"
            );
        }
    }
    Ok(rule.required)
}
