//! Versioned explanations over the execution planner. This module never executes work.
use super::plan::{PlanNodeKind, VerificationPlan};
use crate::{
    config::{quality::QualityConfig, UnmatchedScope},
    project::Project,
    scope::ScopeResult,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn reason(code: &str, message: &str) -> Value {
    json!({"code": code, "message": message})
}

pub(crate) fn build(
    project: &Project,
    scope: &ScopeResult,
    profile: &str,
    explicit_profile: bool,
) -> Result<Value> {
    ensure!(
        project
            .config
            .steps
            .iter()
            .any(|s| s.profiles.contains(profile)),
        "unknown or empty verification profile {profile:?}"
    );
    let plan = VerificationPlan::build(project, scope, profile, None)?;
    super::waiver::validate_plan_waivers(project, &plan, scope)?;
    let nodes = node_explanations(project, scope, profile, &plan);
    Ok(json!({
        "project": project.config.project.name,
        "configuration_digest": project.input().configuration_digest,
        "scope": scope,
        "paths": path_explanations(project, scope),
        "rules": rule_explanations(project, scope),
        "components": component_explanations(project, scope),
        "profiles": project.config.steps.iter().flat_map(|s| &s.profiles)
            .collect::<BTreeSet<_>>().into_iter().map(|id| json!({
                "id": id, "selected": id == profile,
                "reasons": [if id == profile && explicit_profile { reason("PROFILE_REQUESTED", "Selected by the explicit --profile argument.") }
                    else if id == profile { reason("PROFILE_DEFAULT", "Selected by project.default_profile.") }
                    else { reason("PROFILE_NOT_SELECTED", "A different profile was selected.") }]
            })).collect::<Vec<_>>(),
        "profile": profile,
        "nodes": nodes,
        "execution_order": plan.nodes.iter().map(|n| &n.id).collect::<Vec<_>>(),
        "services": service_explanations(project, &plan),
        "quality": quality_explanation(project, profile)?,
        "runtime_inputs": "not_validated",
        "notice": "Selection plan only. Commands, services, signed requests and quality evidence were not executed or authenticated; no verification result is available."
    }))
}

fn path_explanations(project: &Project, scope: &ScopeResult) -> Vec<Value> {
    scope
        .changed_files
        .iter()
        .map(|path| {
            let rules = project.scope_rules.matching_rules(path);
            let explanation = if !rules.is_empty() {
                reason("PATH_RULE_MATCH", "Path matches the listed scope rules.")
            } else if matches!(project.config.scope.unmatched, UnmatchedScope::All) {
                reason(
                    "UNMATCHED_ALL",
                    "Unmatched path triggers selection of every component.",
                )
            } else {
                reason(
                    "UNMATCHED_IGNORED",
                    "Unmatched path is ignored by the configured policy.",
                )
            };
            json!({"path": path, "rules": rules, "reasons": [explanation]})
        })
        .collect()
}

fn rule_explanations(project: &Project, scope: &ScopeResult) -> Vec<Value> {
    project.config.scope.rules.iter().enumerate().map(|(index, rule)| {
        let paths = scope.changed_files.iter()
            .filter(|path| project.scope_rules.matching_rules(path).contains(&index))
            .collect::<Vec<_>>();
        json!({"index": index, "patterns": rule.patterns, "components": rule.components,
            "selected": !paths.is_empty(), "paths": paths,
            "reasons": [if paths.is_empty() { reason("RULE_NOT_MATCHED", "No changed path matched this rule; explicit/all selection does not require a rule match.") }
                else { reason("RULE_MATCHED", "The listed changed paths matched this rule.") }]})
    }).collect()
}

fn component_explanations(project: &Project, scope: &ScopeResult) -> Vec<Value> {
    project.config.components().into_iter().map(|id| {
        let selected = scope.components.contains(&id);
        let rules = project.config.scope.rules.iter().enumerate()
            .filter(|(_, rule)| rule.components.contains(&id))
            .map(|(index, _)| index).collect::<Vec<_>>();
        let explanation = if !selected {
            reason("COMPONENT_NOT_AFFECTED", "No scope selection selected this component; individual steps may still be dependencies.")
        } else if scope.mode == "components" {
            reason("EXPLICIT_COMPONENT", "Explicit component selection overrides path detection.")
        } else if scope.mode == "all" {
            reason("ALL_COMPONENTS", "Explicit all mode selects every component.")
        } else if !scope.unmatched_files.is_empty()
            && matches!(project.config.scope.unmatched, UnmatchedScope::All) {
            reason("UNMATCHED_FALLBACK_ALL", "At least one unmatched path requires all components.")
        } else {
            reason("SCOPE_RULE_COMPONENT", "A changed path matched a rule for this component.")
        };
        json!({"id": id, "selected": selected, "rules": rules, "reasons": [explanation]})
    }).collect()
}

fn node_explanations(
    project: &Project,
    scope: &ScopeResult,
    profile: &str,
    plan: &VerificationPlan<'_>,
) -> Vec<Value> {
    let mut nodes = plan.nodes.iter().map(|node| {
        let required_by = plan.nodes.iter().filter(|n| n.depends_on.contains(&node.id))
            .map(|n| &n.id).collect::<BTreeSet<_>>();
        let explanation = if node.kind != PlanNodeKind::External {
            reason("MANDATORY_BUILTIN_GATE", "Core requires this built-in gate regardless of component or profile filtering.")
        } else if node.step.is_some_and(|s| scope.components.contains(&s.component) && s.profiles.contains(profile)) {
            reason("COMPONENT_AND_PROFILE", "Component and profile both select this step.")
        } else {
            reason("DEPENDENCY_REQUIRED", "Dependency closure includes this step even outside its component or profile.")
        };
        json!({"id": node.id, "label": node.label, "selected": true,
            "kind": if node.kind == PlanNodeKind::External { "external-step" } else { "builtin-gate" },
            "component": node.step.map(|s| &s.component),
            "profiles": node.step.map(|s| &s.profiles),
            "input": node.step.map(|s| s.input),
            "depends_on": node.depends_on, "required_by": required_by,
            "services": node.step.map(|s| &s.services),
            "reasons": [explanation]})
    }).collect::<Vec<_>>();
    for step in &project.config.steps {
        if plan.nodes.iter().any(|n| n.id == step.id) {
            continue;
        }
        let mut reasons = Vec::new();
        if !scope.components.contains(&step.component) {
            reasons.push(reason(
                "COMPONENT_NOT_SELECTED",
                "The step's component was not selected.",
            ));
        }
        if !step.profiles.contains(profile) {
            reasons.push(reason(
                "PROFILE_NOT_INCLUDED",
                "The step does not participate in the selected profile.",
            ));
        }
        nodes.push(
            json!({"id": step.id, "label": step.label, "selected": false,
            "kind": "external-step", "component": step.component, "profiles": step.profiles,
            "input": step.input, "depends_on": step.depends_on, "required_by": [],
            "services": step.services, "reasons": reasons}),
        );
    }
    nodes
}

fn service_explanations(project: &Project, plan: &VerificationPlan<'_>) -> Vec<Value> {
    project.config.services.keys().map(|id| {
        let required_by = plan.nodes.iter().filter_map(|n| n.step)
            .filter(|s| s.services.contains(id)).map(|s| &s.id).collect::<BTreeSet<_>>();
        json!({"id": id, "selected": !required_by.is_empty(), "required_by": required_by,
            "checks": project.config.service_checks.get(id),
            "reasons": [if required_by.is_empty() { reason("SERVICE_NOT_REQUIRED", "No selected step requires this service.") }
                else { reason("SERVICE_REQUIRED", "The listed selected steps require this service; availability was not probed.") }]})
    }).collect()
}

fn quality_explanation(project: &Project, profile: &str) -> Result<Value> {
    let Some(config) = QualityConfig::load_optional(&project.execution_root, &project.config)?
    else {
        return Ok(
            json!({"enabled": false, "reasons": [reason("QUALITY_NOT_CONFIGURED", "No quality.toml is configured.")]}),
        );
    };
    let participation = config
        .profiles
        .get(profile)
        .context("quality profile is not declared")?;
    let workflow = participation
        .workflow
        .as_ref()
        .context("quality profile requires workflow state and trusted_keys")?;
    Ok(json!({
        "enabled": true, "participation": config.participation(profile),
        "workflow": workflow, "baseline": config.baseline,
        "components": config.components, "subjects": config.subjects,
        "relationships": config.relationships,
        "collectors": config.collectors.iter().map(|(id, collector)| json!({
            "id": id, "selected": participation.collectors.contains(id),
            "request": collector.request, "protocol": collector.protocol,
            "produces": collector.produces,
            "reasons": [if participation.collectors.contains(id) { reason("COLLECTOR_SELECTED", "Profile requires this collector and its signed request.") }
                else { reason("COLLECTOR_NOT_SELECTED", "Profile omits this collector.") }]
        })).collect::<Vec<_>>(),
        "policies": config.policies.iter().map(|(id, policy)| json!({
            "id": id, "selected": participation.policies.contains(id),
            "policy_file": policy.policy_file, "rule": policy.rule, "expectation": policy.expectation,
            "reasons": [if participation.policies.contains(id) { reason("POLICY_SELECTED", "Profile requires evaluation of this policy.") }
                else { reason("POLICY_NOT_SELECTED", "Profile omits this policy; no complete-quality success is implied.") }]
        })).collect::<Vec<_>>()
    }))
}

pub(crate) fn render(envelope: &Value) -> String {
    if envelope["status"] != "planned" {
        return format!(
            "Invalid verification plan (not a verification result):\n{}\n",
            serde_json::to_string_pretty(&envelope["diagnostics"]).unwrap_or_default()
        );
    }
    let plan = &envelope["plan"];
    let mut output = format!(
        "Verification plan (not executed): profile={} scope={}\n",
        plan["profile"].as_str().unwrap_or_default(),
        plan["scope"]["mode"].as_str().unwrap_or_default()
    );
    // Human and JSON interfaces carry the same explanations and required inputs.
    output.push_str(&serde_json::to_string_pretty(plan).unwrap_or_default());
    output.push('\n');
    output
}
