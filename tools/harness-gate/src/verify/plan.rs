use crate::config::{BuiltinGateType, StepConfig, StepKind};
use crate::project::Project;
use crate::scope::ScopeResult;
use anyhow::{bail, Result};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BuiltinGate {
    SecretScan,
    ArchitectureAudit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlanNodeKind {
    Builtin(BuiltinGate),
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NodeStatus {
    Passed,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone)]
pub(super) struct NodeResult {
    pub id: String,
    pub label: String,
    pub kind: PlanNodeKind,
    pub status: NodeStatus,
    pub reason: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PlanNode<'a> {
    pub id: String,
    pub label: String,
    pub kind: PlanNodeKind,
    pub depends_on: Vec<String>,
    pub step: Option<&'a StepConfig>,
}

#[derive(Debug)]
pub(super) struct VerificationPlan<'a> {
    pub nodes: Vec<PlanNode<'a>>,
}

impl<'a> VerificationPlan<'a> {
    pub fn build(
        project: &'a Project,
        scope: &ScopeResult,
        profile: &str,
        only_step: Option<&str>,
    ) -> Result<Self> {
        validate_plan_configuration(project)?;
        let selected = selected_step_ids(project, scope, profile, only_step);
        if let Some(id) = only_step {
            if !project.config.steps.iter().any(|step| step.id == id) {
                bail!("unknown verification step {id:?}");
            }
        }

        let mut nodes = BTreeMap::<String, PlanNode<'a>>::new();
        let mut node_order = Vec::new();
        for step in &project.config.steps {
            if selected.contains(&step.id) && step.kind != Some(StepKind::BuiltinGate) {
                node_order.push(step.id.clone());
                nodes.insert(
                    step.id.clone(),
                    PlanNode {
                        id: step.id.clone(),
                        label: step.label.clone(),
                        kind: PlanNodeKind::External,
                        depends_on: step.depends_on.clone(),
                        step: Some(step),
                    },
                );
            }
        }

        let explicit_secret = project.config.steps.iter().find(|step| {
            step.kind == Some(StepKind::BuiltinGate)
                && step.gate_type == Some(BuiltinGateType::SecretScan)
        });
        let explicit_audit = project.config.steps.iter().find(|step| {
            step.kind == Some(StepKind::BuiltinGate)
                && step.gate_type == Some(BuiltinGateType::ArchitectureAudit)
        });

        let secret_id = "builtin.secret-scan".to_string();
        let audit_id = "builtin.architecture-audit".to_string();
        nodes.insert(
            secret_id.clone(),
            PlanNode {
                id: secret_id.clone(),
                label: explicit_secret
                    .map_or_else(|| "secret scan".into(), |step| step.label.clone()),
                kind: PlanNodeKind::Builtin(BuiltinGate::SecretScan),
                depends_on: explicit_secret.map_or_else(Vec::new, |step| step.depends_on.clone()),
                step: explicit_secret,
            },
        );
        node_order.insert(0, secret_id.clone());
        node_order.insert(1, audit_id.clone());
        nodes.insert(
            audit_id.clone(),
            PlanNode {
                id: audit_id.clone(),
                label: explicit_audit
                    .map_or_else(|| "architecture audit".into(), |step| step.label.clone()),
                kind: PlanNodeKind::Builtin(BuiltinGate::ArchitectureAudit),
                depends_on: vec![secret_id.clone()],
                step: explicit_audit,
            },
        );

        for node in nodes.values_mut() {
            if node.kind == PlanNodeKind::External && !node.depends_on.contains(&audit_id) {
                node.depends_on.insert(0, audit_id.clone());
            }
        }

        let mut ordered = Vec::with_capacity(nodes.len());
        let mut visiting = HashSet::new();
        let mut emitted = HashSet::new();
        fn visit<'a>(
            id: &str,
            nodes: &BTreeMap<String, PlanNode<'a>>,
            visiting: &mut HashSet<String>,
            emitted: &mut HashSet<String>,
            ordered: &mut Vec<PlanNode<'a>>,
        ) -> Result<()> {
            if emitted.contains(id) {
                return Ok(());
            }
            if !visiting.insert(id.to_string()) {
                bail!("verification plan dependency cycle includes {id:?}");
            }
            let node = nodes.get(id).ok_or_else(|| {
                anyhow::anyhow!("verification plan references missing node {id:?}")
            })?;
            for dependency in &node.depends_on {
                visit(dependency, nodes, visiting, emitted, ordered)?;
            }
            visiting.remove(id);
            emitted.insert(id.to_string());
            ordered.push(PlanNode {
                id: node.id.clone(),
                label: node.label.clone(),
                kind: node.kind,
                depends_on: node.depends_on.clone(),
                step: node.step,
            });
            Ok(())
        }
        for id in &node_order {
            visit(id, &nodes, &mut visiting, &mut emitted, &mut ordered)?;
        }
        Ok(Self { nodes: ordered })
    }
}

/// Validate the plan-facing portion of configuration again at the execution
/// boundary. Normal project discovery already validates this data, but keeping
/// this check here makes direct library/test callers fail closed as well.
fn validate_plan_configuration(project: &Project) -> Result<()> {
    let mut ids = HashSet::new();
    let mut builtin_gates = HashSet::new();
    let mut log_names = HashSet::new();
    for step in &project.config.steps {
        if !ids.insert(step.id.as_str()) {
            bail!("verification plan contains duplicate node id {:?}", step.id);
        }
        match step.kind.unwrap_or(StepKind::ExternalStep) {
            StepKind::ExternalStep => {
                if matches!(
                    step.id.as_str(),
                    "builtin.secret-scan" | "builtin.architecture-audit"
                ) {
                    bail!(
                        "external step {:?} uses a reserved built-in gate id",
                        step.id
                    );
                }
                // Configuration loading normally performs this preflight. Keep
                // the execution boundary fail-closed for direct Project callers.
                if !log_names.insert(step.log.to_ascii_lowercase()) {
                    bail!("verification plan contains duplicate log {:?}", step.log);
                }
            }
            StepKind::BuiltinGate => {
                let gate_type = step.gate_type.ok_or_else(|| {
                    anyhow::anyhow!("built-in gate {:?} requires gate_type", step.id)
                })?;
                let expected = format!("builtin.{}", gate_type.as_str());
                if step.id != expected {
                    bail!(
                        "built-in gate {:?} must use reserved id {expected:?}",
                        step.id
                    );
                }
                if !step.depends_on.is_empty() {
                    bail!("built-in gate {:?} may not declare dependencies", step.id);
                }
                if !builtin_gates.insert(gate_type) {
                    bail!("duplicate built-in gate type {gate_type:?}");
                }
            }
        }
    }
    for step in &project.config.steps {
        for dependency in &step.depends_on {
            if !ids.contains(dependency.as_str()) {
                bail!(
                    "verification plan node {:?} references missing dependency {:?}",
                    step.id,
                    dependency
                );
            }
        }
    }
    Ok(())
}

fn selected_step_ids(
    project: &Project,
    scope: &ScopeResult,
    profile: &str,
    only_step: Option<&str>,
) -> BTreeSet<String> {
    let mut selected = project
        .config
        .steps
        .iter()
        .filter(|step| {
            step.kind != Some(StepKind::BuiltinGate)
                && scope.components.contains(&step.component)
                && only_step
                    .map(|id| step.id == id)
                    .unwrap_or_else(|| step.profiles.contains(profile))
        })
        .map(|step| step.id.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let previous = selected.len();
        for step in &project.config.steps {
            if selected.contains(&step.id) {
                selected.extend(step.depends_on.iter().cloned());
            }
        }
        if selected.len() == previous {
            break;
        }
    }
    selected
}

#[cfg(test)]
fn topological_order(candidates: Vec<&StepConfig>) -> Vec<&StepConfig> {
    let ids = candidates
        .iter()
        .map(|step| step.id.as_str())
        .collect::<HashSet<_>>();
    let mut by_id = candidates
        .iter()
        .map(|step| (step.id.as_str(), *step))
        .collect::<HashMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::with_capacity(candidates.len());
    fn emit<'a>(
        id: &str,
        ids: &HashSet<&str>,
        by_id: &mut HashMap<&str, &'a StepConfig>,
        emitted: &mut BTreeSet<String>,
        ordered: &mut Vec<&'a StepConfig>,
    ) {
        if emitted.contains(id) {
            return;
        }
        let Some(step) = by_id.get(id).copied() else {
            return;
        };
        for dependency in &step.depends_on {
            if ids.contains(dependency.as_str()) {
                emit(dependency, ids, by_id, emitted, ordered);
            }
        }
        emitted.insert(id.to_string());
        ordered.push(step);
    }
    for step in &candidates {
        emit(&step.id, &ids, &mut by_id, &mut emitted, &mut ordered);
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::{topological_order, BuiltinGate, PlanNodeKind, VerificationPlan};
    use crate::config::FlowConfig;
    use crate::project::Project;
    use crate::scope::ScopeResult;
    use crate::test_support::TestWorkspace;

    #[test]
    fn dependencies_are_stably_topologically_sorted() {
        let source = include_str!("../../presets/generic.flow.toml");
        let mut config: FlowConfig = toml::from_str(source).expect("preset");
        config.steps[0].depends_on = vec![config.steps[1].id.clone()];
        config.validate().expect("valid DAG");
        let ordered = topological_order(config.steps.iter().collect());
        assert_eq!(ordered[0].id, config.steps[1].id);
        assert_eq!(ordered[1].id, config.steps[0].id);
    }

    #[test]
    fn legacy_plan_synthesizes_mandatory_gate_chain() {
        let workspace = TestWorkspace::new("verify-plan");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project = Project::discover(Some(workspace.root.clone()), None).expect("discover");
        let plan = VerificationPlan::build(&project, &ScopeResult::all(&project), "full", None)
            .expect("plan");
        assert!(matches!(
            plan.nodes[0].kind,
            PlanNodeKind::Builtin(BuiltinGate::SecretScan)
        ));
        assert!(matches!(
            plan.nodes[1].kind,
            PlanNodeKind::Builtin(BuiltinGate::ArchitectureAudit)
        ));
        assert!(plan.nodes[2]
            .depends_on
            .contains(&"builtin.architecture-audit".into()));
    }

    #[test]
    fn plan_rejects_duplicate_ids_before_building_nodes() {
        let workspace = TestWorkspace::new("verify-plan-duplicate");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = Project::discover(Some(workspace.root.clone()), None).expect("discover");
        let duplicate = project.config.steps[0].id.clone();
        project.config.steps[1].id = duplicate.clone();
        let error = VerificationPlan::build(&project, &ScopeResult::all(&project), "full", None)
            .expect_err("duplicate plan ids must fail");
        assert!(error.to_string().contains("duplicate node id"));
        assert!(error.to_string().contains(&duplicate));
    }

    #[test]
    fn plan_rejects_missing_dependency_at_execution_boundary() {
        let workspace = TestWorkspace::new("verify-plan-missing-dependency");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = Project::discover(Some(workspace.root.clone()), None).expect("discover");
        project.config.steps[0].depends_on = vec!["does-not-exist".into()];
        let error = VerificationPlan::build(&project, &ScopeResult::all(&project), "full", None)
            .expect_err("missing dependencies must fail");
        assert!(error.to_string().contains("missing dependency"));
    }

    #[test]
    fn plan_rejects_duplicate_logs_at_execution_boundary() {
        let workspace = TestWorkspace::new("verify-plan-duplicate-log");
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let mut project = Project::discover(Some(workspace.root.clone()), None).expect("discover");
        project.config.steps[1].log = project.config.steps[0].log.clone();
        let error = VerificationPlan::build(&project, &ScopeResult::all(&project), "full", None)
            .expect_err("duplicate logs must fail before execution");
        assert!(error.to_string().contains("duplicate log"));
    }
}
