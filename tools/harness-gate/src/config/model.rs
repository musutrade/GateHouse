use crate::failure::RetryClass;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CONFIG_VERSION: u32 = 2;
pub const DEFAULT_CONFIG_PATH: &str = ".harness-gate/flow.toml";
pub const RUNNER_CONFIG_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowConfig {
    pub version: u32,
    pub project: ProjectConfig,
    pub paths: PathsConfig,
    #[serde(default)]
    pub policy: PolicyConfig,
    #[serde(default)]
    pub doctor: DoctorConfig,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceConfig>,
    /// Every declared service check is mandatory before exposing the service.
    #[serde(default)]
    pub service_checks: BTreeMap<String, Vec<ServiceCheck>>,
    #[serde(default)]
    pub parsers: BTreeMap<String, ParserConfig>,
    #[serde(default)]
    pub report_templates: ReportTemplatesConfig,
    #[serde(default)]
    pub execution: ExecutionConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    pub scope: ScopeConfig,
    pub steps: Vec<StepConfig>,
}

/// Controls how eligible verification-plan nodes are dispatched.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub max_parallel: Option<usize>,
    /// Optional per-step retry policies. Retries are opt-in and fail closed.
    #[serde(default)]
    pub retries: BTreeMap<String, RetryConfig>,
    /// Optional per-step shard declaration. A runner receives the shard
    /// identity through reserved environment variables.
    #[serde(default)]
    pub shards: BTreeMap<String, ShardConfig>,
}

impl ExecutionConfig {
    pub const DEFAULT_MAX_PARALLEL: usize = 4;
    pub const MAX_ALLOWED_PARALLEL: usize = 64;

    pub fn effective_max_parallel(&self) -> usize {
        self.max_parallel
            .unwrap_or(Self::DEFAULT_MAX_PARALLEL)
            .clamp(1, Self::MAX_ALLOWED_PARALLEL)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportTemplatesConfig {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    /// Optional JUnit XML output path, relative to the report directory.
    #[serde(default)]
    pub junit: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub webhooks: Vec<WebhookConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WebhookConfig {
    pub url: String,
    /// Exact host names authorized for this destination. Wildcards are not
    /// accepted; address classification is still enforced at connection time.
    pub allowed_hosts: Vec<String>,
    #[serde(default = "default_true")]
    pub on_failure: bool,
    #[serde(default)]
    pub on_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    pub default_profile: String,
    pub hook_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    pub reports: String,
    pub audit_config: String,
    #[serde(default = "default_secrets_config_path")]
    pub secrets_config: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, PathAlias>,
}

fn default_secrets_config_path() -> String {
    ".harness-gate/secrets.toml".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathAlias {
    pub path: String,
    #[serde(default)]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    #[serde(default)]
    pub required_steps: Vec<String>,
    /// Explicit, expiring exceptions. DevRail owns approval policy; the
    /// executor validates scope and expiry before dispatch.
    #[serde(default)]
    pub waivers: Vec<WaiverConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaiverConfig {
    pub id: String,
    pub step: String,
    #[serde(default)]
    pub scope: Option<String>,
    pub risk: String,
    pub reason: String,
    pub owner: String,
    pub approved_by: String,
    pub created_at: String,
    pub expires_at: String,
    pub compensating_control: String,
    #[serde(default)]
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetryConfig {
    #[serde(default = "default_retry_attempts")]
    #[schemars(range(min = 1, max = 5))]
    pub max_attempts: u32,
    #[serde(default)]
    pub backoff_ms: u64,
    #[serde(default)]
    pub retryable: BTreeSet<RetryClass>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShardConfig {
    pub index: u32,
    pub total: u32,
}

fn default_retry_attempts() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoctorConfig {
    #[serde(default)]
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DoctorCheck {
    pub id: String,
    pub label: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default = "default_doctor_timeout")]
    pub timeout_secs: u64,
    #[serde(flatten)]
    pub kind: DoctorCheckKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DoctorCheckKind {
    Command {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Path {
        path: String,
        #[serde(default)]
        path_type: PathType,
        #[serde(default)]
        path_scope: PathScope,
    },
    Glob {
        pattern: String,
    },
    Env {
        name: String,
    },
    EnvOrFile {
        env: String,
        path: String,
        contains: String,
        #[serde(default)]
        path_scope: PathScope,
    },
    GitConfig {
        key: String,
        expected: String,
    },
    GitRemotes,
    Version {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        path: String,
        #[serde(default)]
        trim_prefix: String,
        #[serde(default)]
        path_scope: PathScope,
    },
    Service {
        service: String,
    },
}

/// Where a doctor check path may point. Paths stay inside the repository by
/// default; `host` explicitly opts into probing a host location such as an
/// installed tool or user-level credential file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PathScope {
    #[default]
    Repository,
    Host,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PathType {
    #[default]
    Any,
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ServiceConfig {
    Docker {
        /// Container CLI used for this service. Podman supports the Docker
        /// command surface and is selected explicitly when requested.
        #[serde(default)]
        runtime: ContainerRuntimeKind,
        image: String,
        #[serde(default)]
        image_env: Option<String>,
        #[serde(default)]
        external_env: Option<String>,
        inject_env: String,
        startup_timeout_secs: u64,
        #[serde(default)]
        timeout_env: Option<String>,
        container_port: u16,
        #[serde(default)]
        environment: BTreeMap<String, String>,
        healthcheck: Vec<String>,
        connection: String,
    },
    Environment {
        source_env: String,
        inject_env: String,
    },
}

/// Supported container runtimes for managed services.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContainerRuntimeKind {
    #[default]
    Docker,
    Podman,
}

impl ContainerRuntimeKind {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServiceCheck {
    pub executable: String,
    pub sha256: String,
    pub protocol: String,
    pub plugin: String,
    pub version: String,
    pub timeout_secs: u64,
    /// Only these host environment names are forwarded. Values never enter argv.
    #[serde(default)]
    pub pass_env: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ParserConfig {
    Regex {
        patterns: Vec<String>,
        #[serde(default = "default_capture")]
        capture: usize,
        #[serde(default = "default_minimum")]
        minimum: usize,
    },
    /// Parse standard JUnit XML testcases instead of counting log text.
    Junit {
        #[serde(default = "default_minimum")]
        minimum: usize,
    },
    /// Parse Visual Studio TRX XML test results.
    Trx {
        #[serde(default = "default_minimum")]
        minimum: usize,
    },
    /// Parse stable JSON results. `count_path` may point at an array or a
    /// numeric field; when omitted, common test-result arrays are discovered.
    Json {
        #[serde(default)]
        count_path: Option<String>,
        #[serde(default = "default_minimum")]
        minimum: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeConfig {
    #[serde(default)]
    pub unmatched: UnmatchedScope,
    /// Reviewed, repository-owned component dependencies; never execution order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<ComponentImpact>,
    pub rules: Vec<ScopeRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentImpact {
    pub version: u32,
    /// Optional text inputs used to produce the reviewed graph, pinned by SHA-256.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, String>,
    /// Consumer -> dependencies. Every workflow component must appear,
    /// including components with no dependencies (an empty array).
    pub dependencies: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum UnmatchedScope {
    #[default]
    Fail,
    All,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeRule {
    pub patterns: Vec<String>,
    pub components: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepConfig {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub profiles: BTreeSet<String>,
    #[serde(default)]
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_step_cwd")]
    pub cwd: String,
    #[serde(default)]
    pub log: String,
    #[serde(default = "default_step_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub timeout_env: Option<String>,
    #[serde(default)]
    pub parser: Option<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub remove_env: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Optional discriminator. When absent, this entry is a legacy external step.
    #[serde(default)]
    pub kind: Option<StepKind>,
    /// Closed vocabulary for built-in gate declarations.
    #[serde(default)]
    pub gate_type: Option<BuiltinGateType>,
    /// Optional versioned test-runner contract. Omitted for legacy steps.
    #[serde(default)]
    pub runner: Option<RunnerConfig>,
    /// Repository input required by this step. Snapshot is the default and is
    /// the only source available to ordinary hook steps; `repository` is an
    /// explicit compatibility capability for commands that must inspect Git.
    #[serde(default)]
    pub input: StepInput,
    /// Explicit assertion that this command is deterministic and has no external effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<StepCacheConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepCacheConfig {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    pub deterministic: bool,
    pub side_effect_free: bool,
    /// Additional repository-relative files, including ignored/generated inputs.
    pub inputs: Vec<String>,
    /// Existing output files are verified on a hit; missing files cause execution.
    pub outputs: Vec<String>,
    /// Absolute files: first pins the bare program, followed by its runtime/toolchain/plugin inputs.
    pub toolchain: Vec<String>,
    /// Only these inherited variables are visible to the command.
    pub environment: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StepKind {
    ExternalStep,
    BuiltinGate,
}

impl From<&str> for StepKind {
    fn from(value: &str) -> Self {
        match value {
            "external-step" => Self::ExternalStep,
            "builtin-gate" => Self::BuiltinGate,
            _ => panic!("unknown step kind {value:?}; deserialize configuration instead"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinGateType {
    SecretScan,
    ArchitectureAudit,
}

impl From<&str> for BuiltinGateType {
    fn from(value: &str) -> Self {
        match value {
            "secret-scan" => Self::SecretScan,
            "architecture-audit" => Self::ArchitectureAudit,
            _ => panic!("unknown built-in gate type {value:?}; deserialize configuration instead"),
        }
    }
}

impl BuiltinGateType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SecretScan => "secret-scan",
            Self::ArchitectureAudit => "architecture-audit",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StepInput {
    /// Read files from the invocation's immutable input root.
    #[default]
    #[serde(alias = "staged")]
    Snapshot,
    /// Read files from the original checkout. This is intentionally explicit.
    Repository,
}

/// Declares how a test runner receives concurrency and result-contract inputs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunnerConfig {
    #[schemars(range(min = 2, max = 2))]
    pub version: u32,
    pub kind: String,
    #[serde(default)]
    #[schemars(range(min = 1, max = 256))]
    pub threads: Option<usize>,
    #[serde(default)]
    pub threads_env: Option<String>,
    /// Optional executable constraint supplied by the runner definition.
    #[serde(default)]
    pub required_program: Option<String>,
    /// Arguments carrying the thread budget; `{threads}` expands to its count.
    #[serde(default)]
    pub threads_args: Vec<String>,
    /// Insert thread arguments after the first matching token, appending it if absent.
    /// Without a token, append thread arguments.
    #[serde(default)]
    pub threads_args_after: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Insert runner arguments at this index in the configured step arguments.
    /// When omitted, arguments are appended.
    #[serde(default)]
    pub args_position: Option<usize>,
    #[serde(default)]
    pub result_format: RunnerResultFormat,
    pub isolation: TestIsolation,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunnerResultFormat {
    #[default]
    Regex,
    Junit,
    Trx,
    Json,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TestIsolation {
    Shared,
    SchemaPerWorker,
    DatabasePerWorker,
}

fn default_step_cwd() -> String {
    String::new()
}

fn default_step_timeout() -> u64 {
    0
}

const fn default_true() -> bool {
    true
}

pub(super) const fn default_doctor_timeout() -> u64 {
    15
}

const fn default_capture() -> usize {
    1
}

const fn default_minimum() -> usize {
    1
}
