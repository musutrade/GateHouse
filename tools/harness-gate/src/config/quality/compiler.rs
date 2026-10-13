//! Deterministic, ecosystem-opaque translation of trusted workflow state.
//! State is host/pack-owned input, never collector-supplied approval authority.
use super::{model::*, validation::path, QualityConfig, QUALITY_CONFIG_PATH};
use crate::config::{FlowConfig, DEFAULT_CONFIG_PATH};
use anyhow::{ensure, Context, Result};
use harness_gate::quality::{canonical, evidence, model, parse, policy, project};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrustedState {
    pub schema: String,
    pub profile: String,
    pub expected: model::Context,
    /// Exact bytes of flow, quality, referenced policies and selected requests.
    pub config_files: BTreeMap<String, String>,
    /// Generic component metadata resolved from trusted pack data.
    pub components: BTreeMap<String, model::Component>,
    /// Pack-owned configuration kind aliases to generic contract kinds.
    pub subject_kinds: BTreeMap<String, String>,
    pub relationship_kinds: BTreeMap<String, String>,
    /// Configuration subject aliases to resolved, canonical subjects.
    pub subjects: BTreeMap<String, Vec<model::Subject>>,
    pub series: BTreeMap<String, model::Series>,
    pub artifact_root: String,
    /// Paths relative to artifact_root; snapshots must not be silently refreshed.
    pub artifacts: BTreeMap<String, String>,
    /// Host-authenticated producer responses for this exact run/config/series.
    /// These pins come from the CI trust boundary, never the downloaded bundle.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub retained: BTreeMap<String, RetainedEvidence>,
    #[serde(default)]
    pub selection: Option<BTreeMap<String, BTreeSet<String>>>,
    #[serde(default)]
    pub mappings: Option<Value>,
    #[serde(default)]
    pub exceptions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedEvidence {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CompiledInputs {
    pub schema: &'static str,
    pub identity: String,
    pub project: Value,
    pub policy: Value,
    pub expected: Value,
    pub source_root: PathBuf,
    pub artifact_root: PathBuf,
    pub selection: Option<Value>,
    pub mappings: Option<Value>,
    pub exceptions: Option<Value>,
    #[serde(skip)]
    bindings: Bindings,
    #[serde(skip)]
    artifacts: BTreeMap<String, String>,
    #[serde(skip)]
    component_artifact_roots: BTreeMap<String, PathBuf>,
}

type Bindings = Vec<(BTreeSet<String>, String, String)>;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pinned(root: &Path, name: &str, digest: &str) -> Result<()> {
    let file = path(root, name, "pinned input", true)?;
    ensure!(
        sha(&fs::read(file)?) == digest,
        "stale input digest: {name}"
    );
    Ok(())
}

pub(super) fn target_subjects(
    target: &Target,
    config: &QualityConfig,
    state: &TrustedState,
) -> BTreeSet<String> {
    match target {
        Target::Subject { id } => state.subjects[id].iter().map(|s| s.id.clone()).collect(),
        Target::Component { id } => state
            .subjects
            .values()
            .flatten()
            .filter(|s| &s.component == id)
            .map(|s| s.id.clone())
            .collect(),
        Target::Relationship { id } => {
            let edge = &config.relationships[id];
            target_subjects(&edge.from, config, state)
                .union(&target_subjects(&edge.to, config, state))
                .cloned()
                .collect()
        }
    }
}

fn component<'a>(target: &'a Target, config: &'a QualityConfig) -> &'a str {
    match target {
        Target::Component { id } => id,
        Target::Subject { id } => &config.subjects[id].component,
        Target::Relationship { .. } => unreachable!("validated relationship endpoint"),
    }
}

pub(crate) fn compile(root: &Path, state: &TrustedState) -> Result<CompiledInputs> {
    let root = root
        .canonicalize()
        .context("resolve compiler repository root")?;
    ensure!(
        state.schema == "quality-trusted-state/v1",
        "unknown trusted state schema"
    );
    let flow_path = path(&root, DEFAULT_CONFIG_PATH, "flow configuration", true)?;
    let flow = FlowConfig::load_with_diagnostics(&flow_path, Some(&root))?;
    let config = QualityConfig::load_optional(&root, &flow)?
        .context("quality.toml is required for compilation")?;
    let profile = config
        .profiles
        .get(&state.profile)
        .context("unknown quality profile")?;
    validate_inventory(&root, &config, profile, state)?;
    let (artifact_root, component_artifact_roots) = resolve_roots(&root, &config, state)?;
    let project = compile_project(&root, &config, state)?;
    let bindings = compile_bindings(&config, profile, state)?;
    let policy = compile_policy(&root, &config, profile, state, &project)?;
    let selection = compile_selection(&config, state)?;
    let expected = serde_json::to_value(&state.expected)?;
    let mut normalized = state.clone();
    for subjects in normalized.subjects.values_mut() {
        subjects.sort_by(|a, b| a.id.cmp(&b.id));
    }
    // Absolute mount paths are transport, not portable compilation identity.
    let payload = json!({"version":"quality-compilation/v1", "compiler":env!("CARGO_PKG_VERSION"),
        "config":config, "flow":flow, "state":normalized, "project":project, "policy":policy, "selection":selection});
    Ok(CompiledInputs {
        schema: "quality-compiled-inputs/v1",
        identity: format!("quality-compilation/v1:{}", sha(&canonical(&payload)?)),
        project,
        policy,
        expected,
        source_root: root,
        artifact_root,
        selection,
        mappings: state.mappings.clone(),
        exceptions: state.exceptions.clone(),
        bindings,
        artifacts: state.artifacts.clone(),
        component_artifact_roots,
    })
}

fn validate_inventory(
    root: &Path,
    config: &QualityConfig,
    profile: &Participation,
    state: &TrustedState,
) -> Result<()> {
    let mut files = BTreeSet::from([
        DEFAULT_CONFIG_PATH.to_owned(),
        QUALITY_CONFIG_PATH.to_owned(),
    ]);
    files.extend(config.policies.values().map(|p| p.policy_file.clone()));
    files.extend(
        profile
            .collectors
            .iter()
            .map(|id| config.collectors[id].request.clone()),
    );
    ensure!(
        files.iter().eq(state.config_files.keys()),
        "mixed configuration file inventory"
    );
    for (file, digest) in &state.config_files {
        pinned(root, file, digest)?;
    }
    ensure!(
        config.components.keys().eq(state.components.keys()),
        "mixed component inventory"
    );
    ensure!(
        config.subjects.keys().eq(state.subjects.keys()),
        "mixed subject inventory"
    );
    ensure!(
        profile.collectors.iter().eq(state.series.keys()),
        "mixed collector inventory"
    );
    ensure!(
        config
            .subjects
            .values()
            .map(|s| &s.kind)
            .collect::<BTreeSet<_>>()
            == state.subject_kinds.keys().collect(),
        "mixed subject kind inventory"
    );
    ensure!(
        config
            .relationships
            .values()
            .map(|r| &r.kind)
            .collect::<BTreeSet<_>>()
            == state.relationship_kinds.keys().collect(),
        "mixed relationship kind inventory"
    );
    Ok(())
}

fn resolve_roots(
    root: &Path,
    config: &QualityConfig,
    state: &TrustedState,
) -> Result<(PathBuf, BTreeMap<String, PathBuf>)> {
    let artifact_root = path(root, &state.artifact_root, "artifact root", true)?;
    for (file, digest) in &state.artifacts {
        pinned(&artifact_root, file, digest)?;
    }
    let component_artifact_roots = config
        .components
        .iter()
        .map(|(id, c)| {
            Ok((
                id.clone(),
                path(root, &c.artifact_root, "component artifact root", true)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    for file in state.artifacts.keys() {
        let resolved = path(&artifact_root, file, "artifact", true)?;
        ensure!(
            component_artifact_roots
                .values()
                .any(|root| resolved.starts_with(root)),
            "artifact escapes component roots"
        );
    }
    Ok((artifact_root, component_artifact_roots))
}

fn compile_project(root: &Path, config: &QualityConfig, state: &TrustedState) -> Result<Value> {
    let mut subjects = Vec::new();
    for (id, declaration) in &config.subjects {
        let resolved = &state.subjects[id];
        ensure!(!resolved.is_empty(), "unresolved subject {id}");
        if let SubjectSelection::Explicit { paths } = &declaration.selection {
            ensure!(
                paths.iter().collect::<BTreeSet<_>>() == resolved.iter().map(|s| &s.path).collect(),
                "explicit subject paths differ: {id}"
            );
        }
        for subject in resolved {
            ensure!(
                subject.component == declaration.component
                    && subject.kind == state.subject_kinds[&declaration.kind]
                    && subject.target == state.expected.target,
                "mixed subject ownership/kind/target: {id}"
            );
            pinned(root, &subject.path, &subject.source_sha256)?;
            let value = serde_json::to_value(subject)?;
            ensure!(
                subject.id == project::subject_id(&config.project.id, &value)?,
                "stale subject identity: {id}"
            );
            subjects.push(value);
        }
    }
    subjects.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    for (id, declaration) in &config.components {
        let resolved = &state.components[id];
        ensure!(&resolved.id == id, "mixed component identity");
        ensure!(
            declaration.source_roots.iter().collect::<BTreeSet<_>>()
                == resolved.source_boundaries.iter().map(|b| &b.path).collect(),
            "mixed source boundaries"
        );
    }
    let relationships: Vec<_> = config.relationships.iter().map(|(id, edge)| json!({
        "id": id, "kind": state.relationship_kinds[&edge.kind], "producer": component(&edge.from, config),
        "consumer": component(&edge.to, config), "subjects": target_subjects(&Target::Relationship { id: id.clone() }, config, state), "metadata": {}
    })).collect();
    let project = json!({"schema":"harness-project/v1", "id":config.project.id, "metadata":{},
        "components":state.components.values().collect::<Vec<_>>(), "subjects":subjects, "relationships":relationships});
    project::validate_project(&project)?;
    Ok(project)
}

fn compile_bindings(
    config: &QualityConfig,
    profile: &Participation,
    state: &TrustedState,
) -> Result<Bindings> {
    let mut bindings = Vec::new();
    for id in &profile.collectors {
        let series = serde_json::to_value(&state.series[id])?;
        // Series identity is opaque here. The core owns metric support/certification.
        ensure!(
            series["id"] == evidence::series_id(&series)?
                && series["target"] == state.expected.target,
            "stale collector/tool/series identity"
        );
        for expectation in &config.collectors[id].produces {
            ensure!(
                expectation.series == state.series[id].id
                    && state.series[id]
                        .metrics
                        .iter()
                        .any(|m| m.name == expectation.capability),
                "incompatible configured capability/series"
            );
            bindings.push((
                target_subjects(&expectation.target, config, state),
                expectation.capability.clone(),
                expectation.series.clone(),
            ));
        }
    }
    Ok(bindings)
}

fn compile_policy(
    root: &Path,
    config: &QualityConfig,
    profile: &Participation,
    state: &TrustedState,
    project: &Value,
) -> Result<Value> {
    // A profile with no policy has no evaluator input, not an empty policy
    // purporting to certify the project. The released policy schema is unchanged.
    if profile.policies.is_empty() {
        return Ok(Value::Null);
    }
    let mut rules = BTreeMap::new();
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
            .find(|r| r["id"] == binding.rule)
            .context("unresolved policy rule")?;
        let mut resolved = vec![rule.clone()];
        if let Target::Subject { .. } = &binding.expectation.target {
            resolved = target_subjects(&binding.expectation.target, config, state)
                .into_iter()
                .map(|subject| {
                    let mut rule = rule.clone();
                    rule["id"] = json!(format!("{}@{}", binding.rule, subject));
                    rule["scope"]["subject"] = json!(subject);
                    rule
                })
                .collect();
        }
        for rule in resolved {
            let key = rule["id"].as_str().context("policy id")?.to_owned();
            ensure!(
                rules.insert(key, rule).is_none(),
                "conflicting compiled policy IDs"
            );
        }
    }
    let policy =
        json!({"schema":"harness-policy/v1", "rules":rules.into_values().collect::<Vec<_>>()});
    policy::validate_policy(&policy, project)?;
    Ok(policy)
}

fn compile_selection(config: &QualityConfig, state: &TrustedState) -> Result<Option<Value>> {
    let selection = state
        .selection
        .as_ref()
        .map(|selection| -> Result<Value> {
            let mut resolved = BTreeMap::new();
            for (kind, aliases) in selection {
                ensure!(
                    matches!(kind.as_str(), "changed_subject" | "critical_subject"),
                    "unknown selection kind"
                );
                let mut ids = BTreeSet::new();
                for alias in aliases {
                    ensure!(
                        config.subjects.contains_key(alias),
                        "unknown selected subject alias"
                    );
                    ids.extend(target_subjects(
                        &Target::Subject { id: alias.clone() },
                        config,
                        state,
                    ));
                }
                resolved.insert(kind, ids);
            }
            Ok(serde_json::to_value(resolved)?)
        })
        .transpose()?;
    Ok(selection)
}

impl CompiledInputs {
    /// Enforce host-owned producer/manifest bindings before the core evaluates facts.
    pub(crate) fn validate_bindings(&self, records: &Value) -> Result<()> {
        for record in records.as_array().context("evidence must be an array")? {
            for capability in record["capabilities"]
                .as_array()
                .context("evidence capabilities")?
            {
                ensure!(
                    self.bindings
                        .iter()
                        .any(|(subjects, metric, series)| subjects
                            .contains(record["subject"]["id"].as_str().unwrap_or(""))
                            && capability["metric"] == *metric
                            && record["series"]["id"] == *series),
                    "unbound evidence subject/capability/series"
                );
            }
            let component_root = self
                .component_artifact_roots
                .get(record["component"].as_str().unwrap_or(""))
                .context("unbound evidence component")?;
            for artifact in record["artifacts"]
                .as_array()
                .context("evidence artifacts")?
            {
                let file = artifact["path"].as_str().context("artifact path")?;
                ensure!(
                    path(&self.artifact_root, file, "artifact", true)?.starts_with(component_root),
                    "artifact escapes owning component root"
                );
                ensure!(
                    artifact["path"]
                        .as_str()
                        .and_then(|p| self.artifacts.get(p))
                        .is_some_and(|sha| artifact["sha256"] == *sha),
                    "unbound artifact identity"
                );
            }
        }
        Ok(())
    }
}
