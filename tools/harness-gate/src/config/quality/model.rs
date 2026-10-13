//! Versioned control-plane intent. Collector declarations contain measurements only.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QualityConfig {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    pub project: QualityProject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<CollectionLimits>,
    pub components: BTreeMap<String, Component>,
    #[serde(default)]
    pub subjects: BTreeMap<String, Subject>,
    #[serde(default)]
    pub relationships: BTreeMap<String, Relationship>,
    pub collectors: BTreeMap<String, Collector>,
    pub policies: BTreeMap<String, PolicyBinding>,
    pub profiles: BTreeMap<String, Participation>,
    pub baseline: Baseline,
    pub reporting: Reporting,
}

/// Host-reviewed collection resource limits, bound by the quality configuration digest.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionLimits {
    /// Aggregate on-disk bytes in the shared artifact root, including undeclared files.
    #[schemars(range(min = 1))]
    pub max_artifact_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QualityProject {
    pub id: String,
    /// Must match flow.project.name; id is the stable quality identity.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Nonempty subset of execution-plane component names.
    pub flow_components: BTreeSet<String>,
    pub source_roots: Vec<String>,
    pub artifact_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub component: String,
    pub kind: String,
    pub selection: SubjectSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubjectSelection {
    Explicit { paths: Vec<String> },
    Discovery { collector: String, query: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Component { id: String },
    Subject { id: String },
    Relationship { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub kind: String,
    pub from: Target,
    pub to: Target,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Collector {
    pub protocol: CollectorProtocol,
    /// Repository-relative signed adapter request material; never inline policy.
    pub request: String,
    pub produces: Vec<Expectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum CollectorProtocol {
    #[serde(rename = "harness-collector-request/v1")]
    V1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    pub target: Target,
    pub capability: String,
    /// Exact existing measurement-series identity, not a new naming scheme.
    #[schemars(regex(pattern = "^measurement-series/v1:[0-9a-f]{64}$"))]
    pub series: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyBinding {
    pub expectation: Expectation,
    /// Reference to a harness-policy/v1 document and rule, resolved by compilation.
    pub policy_file: String,
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Participation {
    /// Complete profiles enforce every configured required policy. Partial
    /// profiles may omit work, but cannot certify complete project quality.
    #[serde(default)]
    pub assurance: Assurance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<Workflow>,
    pub collectors: BTreeSet<String>,
    pub policies: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Assurance {
    #[default]
    Complete,
    Partial,
}

/// Host-produced inputs for this profile; collectors cannot supply trust roots.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub state: String,
    pub trusted_keys: String,
    pub baseline_request: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub required: bool,
    pub provider: BaselineProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BaselineProvider {
    None,
    Git { reference: String, merge_base: bool },
    RetainedArtifact { manifest: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reporting {
    pub output: String,
    pub formats: BTreeSet<ReportFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Human,
    Json,
}
