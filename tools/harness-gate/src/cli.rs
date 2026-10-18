use crate::scope::ScopeMode;
use crate::ui::ColorMode;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "harness-gate",
    version,
    about = "Configuration-driven project verification: init -> verify -> one project decision",
    arg_required_else_help = true,
    after_help = "Project workflow:\n  harness-gate init --preset rust-api --presets-dir /path/to/GateProbes/presets\n  harness-gate config check\n  harness-gate verify --profile ci --all\n\nReference presets generate flow + quality configuration. Provision trusted collector\nstate, signed requests and keys before verify; see generated QUALITY.md.\nEcosystem identity is configuration/pack data; collectors measure, Rust decides.\nAdvanced interfaces: quality evaluate and adapter run (explicit trusted inputs)."
)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) plugins: crate::plugins::HostOptions,
    /// Directory containing a reviewed external catalog.json and preset files.
    #[arg(long, global = true, value_name = "PATH")]
    pub presets_dir: Option<PathBuf>,
    /// Control colored terminal output.
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub(crate) color: ColorMode,

    /// Override automatic project root discovery.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) project_root: Option<PathBuf>,

    /// Override the repository workflow configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    /// Host-owned durable nonce ledger shared by verify, quality collect and adapter run.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) replay_state_dir: Option<PathBuf>,

    /// Enable local step reuse in an exclusively host-owned directory outside the project.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) step_cache_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Verify one pinned saved result and its evidence without loading a project.
    ReportVerify {
        /// Existing v1/v2 history index containing exactly one invocation and trusted pins.
        #[arg(long)]
        input: PathBuf,
    },
    /// Combine latest report hot spots, selected trends and observed commit changes, offline.
    ReportDashboard {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[command(flatten)]
        selection: crate::report_history::SelectionFilter,
        #[command(flatten)]
        query: crate::report_history::Query,
    },
    /// Render one metric's history from explicitly pinned completed reports, offline.
    ReportHistory {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        policy: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        metric: String,
        #[command(flatten)]
        query: crate::report_history::Query,
    },

    /// Export a completed, pinned HTML report for optional artifact delivery; never evaluate policy.
    ReportExport {
        /// Existing v1 history index containing exactly one publication.
        #[arg(long)]
        input: PathBuf,
        /// New directory outside the input tree; contains only HTML and provenance.
        #[arg(long)]
        output: PathBuf,
        /// Independently expected full commit identity for this report.
        #[arg(long)]
        commit: String,
    },

    /// Explain a registered machine failure, CLI error or configuration diagnostic code without loading a project.
    Explain {
        /// Stable code, for example QUALITY_BLOCKED, E1401 or HGCFG-TOML-PARSE.
        #[arg(required_unless_present_any = ["list", "search"], conflicts_with_all = ["list", "search"])]
        code: Option<String>,
        /// List every supported code.
        #[arg(long, conflicts_with = "search")]
        list: bool,
        /// Search codes, titles and guidance (case-insensitive literal text).
        #[arg(long)]
        search: Option<String>,
        /// Emit machine-readable explanation JSON.
        #[arg(long)]
        json: bool,
    },
    /// Consume completed reports as diagnostic findings; never evaluate policy.
    Findings {
        #[command(subcommand)]
        action: crate::findings::Action,
    },
    /// Clear authenticated local step-cache entries; retain unknown or modified files.
    CacheClear {
        #[arg(long)]
        json: bool,
    },
    /// Validate an explicit plugin lock and prepared offline artifacts without a project.
    Plugins {
        #[arg(long, value_name = "PATH")]
        lock: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Advanced evidence evaluation and integration; use verify for project decisions.
    Quality {
        #[command(subcommand)]
        action: crate::app::quality::QualityAction,
    },
    /// Run the versioned serial compatibility launcher and migration tools.
    Compat {
        #[command(subcommand)]
        action: CompatAction,
    },
    /// Advanced signed adapter protocol; use verify for project orchestration.
    Adapter {
        #[command(subcommand)]
        action: AdapterAction,
    },
    /// Check local tools, configuration, Git, and test database access.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Treat warnings as failures.
        #[arg(long)]
        strict: bool,
    },
    /// Inspect and reclaim stale Harness-Gate resource leases.
    Cleanup {
        /// Only report stale marked resources without reclaiming them.
        #[arg(long)]
        dry_run: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show changed files and the verification components they select.
    Scope {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Repeat the in-process matcher benchmark (used by quality baselines).
        #[arg(long, hide = true, default_value_t = 0, value_name = "N")]
        benchmark_repeat: usize,
    },
    /// Scan file names for high-confidence credential patterns.
    Secrets {
        /// Scan the staged snapshot instead of the working tree.
        #[arg(long)]
        staged: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run deterministic architecture rules and write review_context reports.
    Audit {
        /// Emit the complete audit report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run configured execution and quality gates for one blocking project decision.
    #[command(visible_alias = "check")]
    Verify {
        /// Copy a regular-file directory into a content-bound input (--all only).
        #[arg(long, requires = "all", conflicts_with_all = ["plan", "components"])]
        snapshot: bool,
        #[command(flatten)]
        scope: ScopeArgs,
        /// Override scope detection with a comma-separated component list.
        #[arg(
            long,
            value_delimiter = ',',
            num_args = 1..,
            conflicts_with_all = ["staged", "all", "base"]
        )]
        components: Vec<String>,
        /// Select any profile declared by configured steps.
        #[arg(long, value_name = "PROFILE")]
        profile: Option<String>,
        /// Explain the execution plan without running gates, commands or collectors.
        #[arg(long)]
        plan: bool,
        /// Emit a versioned plan envelope (requires --plan).
        #[arg(long, requires = "plan")]
        json: bool,
    },
    /// Run the staged, fast verification profile used by pre-commit.
    Hook,
    /// Explicitly manage thin repository hooks without running verification.
    Hooks {
        #[command(subcommand)]
        action: crate::hooks::HookAction,
    },
    /// Extract one trace's error context from JSON Lines logs.
    ParseLogs {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Validate or inspect flow and optional quality configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Generate artifacts for the workflow configuration model.
    Schema {
        #[command(subcommand)]
        action: SchemaAction,
    },
    /// Run one configured full-profile step after secrets and audit gates.
    Step {
        /// Step id from flow.toml, for example api.clippy.
        id: String,
    },
    /// Initialize flow and reference quality packs; provision trusted inputs before verify.
    Init {
        /// Choose a preset and confirm initialization in a terminal.
        #[arg(long, conflicts_with = "preset")]
        interactive: bool,
        /// Override the directory-derived project identifier.
        #[arg(long, value_name = "NAME")]
        project_name: Option<String>,
        /// Select a locked data-only plugin as the preset catalog.
        #[arg(long, conflicts_with = "presets_dir")]
        plugin: Option<String>,
        #[arg(long, default_value = "generic")]
        preset: String,
        /// Replace existing .harness-gate configuration files.
        #[arg(long)]
        force: bool,
    },
    /// List the generic preset and an explicitly selected external catalog.
    Presets,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CompatAction {
    /// Replay an existing DevRail request through the serial verifier.
    Run {
        #[arg(short, long, value_name = "PATH")]
        input: PathBuf,
        #[arg(short, long, value_name = "PATH")]
        output: PathBuf,
        /// Optional frozen/legacy result to compare in shadow mode.
        #[arg(long, value_name = "PATH")]
        old_result: Option<PathBuf>,
    },
    /// Compare two machine results after removing volatile execution fields.
    Compare {
        #[arg(long, value_name = "PATH")]
        old: PathBuf,
        #[arg(long, value_name = "PATH")]
        new: PathBuf,
        #[arg(short, long, value_name = "PATH")]
        output: PathBuf,
    },
    /// Enable a bounded migration canary slice.
    Canary {
        #[arg(long, value_name = "PATH")]
        state: PathBuf,
        #[arg(long)]
        slice: String,
    },
    /// Disable the launcher and record a rollback event.
    Rollback {
        #[arg(long, value_name = "PATH")]
        state: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AdapterAction {
    /// Advanced: execute one signed request; collectors measure without policy authority.
    Run {
        #[arg(short, long, value_name = "PATH")]
        request: PathBuf,
        /// Trusted Ed25519 key JSON files ({"key_id": ..., "public_key": ...}).
        #[arg(long = "trusted-key", value_name = "PATH")]
        trusted_keys: Vec<PathBuf>,
        /// Capability values allowed by the host. Repeat for multiple values.
        #[arg(long = "allow-network", value_name = "CAPABILITY")]
        allow_network: Vec<String>,
        #[arg(long = "allow-resource", value_name = "CAPABILITY")]
        allow_resources: Vec<String>,
        #[arg(long = "allow-environment", value_name = "NAME")]
        allow_environment: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SchemaAction {
    /// Export the JSON Schema for flow.toml or quality.toml.
    Export {
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Select the quality control-plane schema.
        #[arg(long)]
        quality: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
    /// Plan workflow configuration repairs without writing files or running steps.
    Suggest {
        #[arg(long, value_enum, default_value_t = ConfigFormat::Human)]
        format: ConfigFormat,
        /// Preview candidate edits and ask before applying them from a terminal.
        #[arg(long)]
        apply: bool,
    },
    /// Import compatible Arc-Flow v2 execution configuration and a parity report.
    Import {
        #[arg(long, default_value = ".arc-flow/flow.toml", value_name = "PATH")]
        input: PathBuf,
        #[arg(long, default_value = ".harness-gate/flow.toml", value_name = "PATH")]
        output: PathBuf,
        /// Limit import to declared execution config; runtime authority remains blocked.
        #[arg(long)]
        execution_only: bool,
    },
    /// Validate configuration, environment overrides, and protected steps.
    Check {
        /// Render diagnostics as a machine-readable JSON envelope.
        #[arg(long, value_enum, default_value_t = ConfigFormat::Human)]
        format: ConfigFormat,
    },
    /// Print the source or effective configuration.
    Print {
        /// Include environment overrides in the rendered TOML.
        #[arg(long)]
        resolved: bool,
        /// Print quality.toml instead of flow.toml; both planes are validated.
        #[arg(long)]
        quality: bool,
    },
    /// Convert a schema v1 flow.toml to .harness-gate/flow.toml schema v2.
    Migrate {
        /// Schema v1 flow.toml to convert (defaults to the global --config).
        #[arg(long, value_name = "PATH")]
        input: Option<PathBuf>,
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum ConfigFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ScopeArgs {
    /// Inspect only staged files.
    #[arg(long, conflicts_with_all = ["all", "base"])]
    staged: bool,
    /// Select every verification component.
    #[arg(long, conflicts_with_all = ["staged", "base"])]
    all: bool,
    /// Inspect committed changes in REF...HEAD.
    #[arg(long, value_name = "REF", conflicts_with_all = ["staged", "all"])]
    base: Option<String>,
}

impl ScopeArgs {
    pub(crate) fn mode(&self) -> ScopeMode {
        if self.staged {
            ScopeMode::Staged
        } else if self.all {
            ScopeMode::All
        } else if let Some(reference) = &self.base {
            ScopeMode::Base(reference.clone())
        } else {
            ScopeMode::WorkingTree
        }
    }
}
