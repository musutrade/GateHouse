use crate::config::quality::compiler::{self, TrustedState};
use crate::config::quality::{baseline, collectors, QualityConfig};
use crate::process::adapter::{HostPolicy, TrustedKey};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use harness_gate::quality::{evidence::ValidationContext, parse, policy, project_report};
use serde_json::Value;
use std::{fs, path::PathBuf};

#[derive(Debug, Subcommand)]
pub(crate) enum QualityAction {
    /// Advanced: evaluate explicit trusted inputs in Rust; ordinary projects use verify.
    Evaluate(Box<EvaluateArgs>),
    /// Compile validated configuration and host-owned state to generic contracts.
    Compile(CompileArgs),
    /// Collect validated project measurements through configured signed adapters.
    Collect(CollectArgs),
    /// Resolve a trusted baseline into a new directory outside the working tree.
    Baseline(BaselineArgs),
}

#[derive(Debug, Args)]
pub(crate) struct BaselineArgs {
    #[arg(long)]
    repository_root: PathBuf,
    /// Trusted head state, as used by quality compile.
    #[arg(long)]
    state: PathBuf,
    /// Host-pinned base state and authenticated retained manifest digest.
    #[arg(long)]
    request: Option<PathBuf>,
    /// New directory; contains resolution.json and the evaluator base inputs.
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct CollectArgs {
    #[command(flatten)]
    inputs: CompileArgs,
    /// Host-owned JSON array of trusted Ed25519 keys; never read from a collector.
    #[arg(long)]
    trusted_keys: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct CompileArgs {
    #[arg(long)]
    repository_root: PathBuf,
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct EvaluateArgs {
    /// Host-owned state: compile project configuration before using the same evaluator.
    #[arg(long, requires = "repository_root", conflicts_with_all = ["selection", "mappings", "exceptions"])]
    state: Option<PathBuf>,
    #[arg(long, requires = "state")]
    repository_root: Option<PathBuf>,
    /// Trusted harness-project/v1 model.
    #[arg(long, required_unless_present = "state", conflicts_with = "state")]
    project: Option<PathBuf>,
    /// Trusted harness-policy/v1 rules.
    #[arg(long, required_unless_present = "state", conflicts_with = "state")]
    policy: Option<PathBuf>,
    /// Collected harness-evidence/v1 records (JSON array).
    #[arg(long)]
    evidence: PathBuf,
    /// Trusted expected source, collector and measurement-series context.
    #[arg(long, required_unless_present = "state", conflicts_with = "state")]
    expected: Option<PathBuf>,
    #[arg(long, required_unless_present = "state", conflicts_with = "state")]
    source_root: Option<PathBuf>,
    #[arg(long, required_unless_present = "state", conflicts_with = "state")]
    artifact_root: Option<PathBuf>,
    /// Optional trusted selection, subject mappings and exception metadata.
    #[arg(long)]
    selection: Option<PathBuf>,
    #[arg(long)]
    mappings: Option<PathBuf>,
    #[arg(long)]
    exceptions: Option<PathBuf>,
    /// All five base arguments must be supplied together for baseline comparison.
    #[arg(long, requires_all = ["base_project", "base_expected", "base_source_root", "base_artifact_root"])]
    base_evidence: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_project: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_expected: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_source_root: Option<PathBuf>,
    #[arg(long, requires = "base_evidence")]
    base_artifact_root: Option<PathBuf>,
    /// Trusted RFC3339 evaluation clock; defaults to current UTC time.
    #[arg(long)]
    now: Option<String>,
    /// Destination for the existing harness-project-report/v1 contract.
    #[arg(long)]
    output: PathBuf,
}

fn read(path: &PathBuf) -> Result<Value> {
    parse(&fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?)
        .with_context(|| format!("parse {}", path.display()))
}

pub(crate) fn run(
    action: &QualityAction,
    replay_state_dir: Option<&std::path::Path>,
) -> Result<bool> {
    match action {
        QualityAction::Compile(args) => compile_inputs(args),
        QualityAction::Evaluate(args) => evaluate(args),
        QualityAction::Collect(args) => collect(args, replay_state_dir),
        QualityAction::Baseline(args) => {
            let state = read_state(&args.state)?;
            let request = args
                .request
                .as_ref()
                .map(|p| -> Result<baseline::Request> { Ok(serde_json::from_value(read(p)?)?) })
                .transpose()?;
            baseline::resolve(
                &args.repository_root,
                &state,
                request.as_ref(),
                &args.output,
            )?;
            Ok(true)
        }
    }
}

fn collect(args: &CollectArgs, replay_state_dir: Option<&std::path::Path>) -> Result<bool> {
    let state = read_state(&args.inputs.state);
    if let Ok(state) = &state {
        protect_output(&args.inputs.output, &args.inputs.repository_root, state)?;
        let root = args.inputs.repository_root.canonicalize()?;
        let artifact_root = root.join(&state.artifact_root).canonicalize()?;
        let parent = args
            .inputs
            .output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."))
            .canonicalize()?;
        anyhow::ensure!(
            !parent.starts_with(artifact_root),
            "collection output must be outside artifact root"
        );
    }
    clear_output(
        &args.inputs.output,
        [Some(&args.inputs.state), Some(&args.trusted_keys)],
    )?;
    let trusted_keys: Vec<TrustedKey> = serde_json::from_value(read(&args.trusted_keys)?)?;
    let root = args.inputs.repository_root.canonicalize()?;
    let flow = crate::config::FlowConfig::load_with_diagnostics(
        &root.join(crate::config::DEFAULT_CONFIG_PATH),
        Some(&root),
    )?;
    let config = QualityConfig::load_optional(&root, &flow)?.context("missing quality config")?;
    let policy = HostPolicy {
        max_artifact_bytes: config
            .limits
            .as_ref()
            .map(|limits| limits.max_artifact_bytes)
            .or(HostPolicy::default().max_artifact_bytes),
        trusted_keys,
        ..HostPolicy::default()
    };
    let state = state?;
    let policy = crate::process::replay::collector_policy(
        policy,
        &root,
        replay_state_dir,
        &root.join(&state.artifact_root),
    )?;
    let collection = collectors::collect(&args.inputs.repository_root, &state, &policy)?;
    crate::utils::fs::atomic_write(
        &args.inputs.output,
        format!("{}\n", serde_json::to_string_pretty(&collection)?),
        true,
    )?;
    Ok(true)
}

fn compile_inputs(args: &CompileArgs) -> Result<bool> {
    let state = read_state(&args.state);
    if let Ok(state) = &state {
        protect_output(&args.output, &args.repository_root, state)?;
    }
    clear_output(&args.output, [Some(&args.state)])?;
    let compiled = compiler::compile(&args.repository_root, &state?)?;
    crate::utils::fs::atomic_write(
        &args.output,
        format!("{}\n", serde_json::to_string_pretty(&compiled)?),
        true,
    )?;
    Ok(true)
}

fn evaluate(args: &EvaluateArgs) -> Result<bool> {
    // Clear an earlier report before reading inputs: errors must not leave a stale pass.
    // Refuse aliases of input documents before touching the destination.
    let paths = [
        args.project.as_ref(),
        args.policy.as_ref(),
        Some(&args.evidence),
        args.state.as_ref(),
        args.expected.as_ref(),
        args.selection.as_ref(),
        args.mappings.as_ref(),
        args.exceptions.as_ref(),
        args.base_evidence.as_ref(),
        args.base_project.as_ref(),
        args.base_expected.as_ref(),
    ];
    let state = args.state.as_ref().map(read_state).transpose();
    if let (Ok(Some(state)), Some(root)) = (&state, &args.repository_root) {
        protect_output(&args.output, root, state)?;
    }
    clear_output(&args.output, paths)?;
    let compiled = state?
        .map(|state| -> Result<_> {
            compiler::compile(
                args.repository_root.as_ref().expect("clap requires root"),
                &state,
            )
        })
        .transpose()?;
    let records = read(&args.evidence)?;
    if let Some(compiled) = &compiled {
        compiled.validate_bindings(&records)?;
    }
    let (project, rules, expected, source_root, artifact_root, selection, mappings, exceptions) =
        if let Some(compiled) = compiled {
            (
                compiled.project,
                compiled.policy,
                compiled.expected,
                compiled.source_root,
                compiled.artifact_root,
                compiled.selection,
                compiled.mappings,
                compiled.exceptions,
            )
        } else {
            (
                read(args.project.as_ref().expect("clap requires project"))?,
                read(args.policy.as_ref().expect("clap requires policy"))?,
                read(args.expected.as_ref().expect("clap requires expected"))?,
                args.source_root.clone().expect("clap requires source root"),
                args.artifact_root
                    .clone()
                    .expect("clap requires artifact root"),
                args.selection.as_ref().map(read).transpose()?,
                args.mappings.as_ref().map(read).transpose()?,
                args.exceptions.as_ref().map(read).transpose()?,
            )
        };
    let base_records = args.base_evidence.as_ref().map(read).transpose()?;
    let base_project = args.base_project.as_ref().map(read).transpose()?;
    let base_expected = args.base_expected.as_ref().map(read).transpose()?;
    let head = ValidationContext {
        project: &project,
        expected: &expected,
        source_root: &source_root,
        artifact_root: &artifact_root,
    };
    let base = base_project.as_ref().map(|project| ValidationContext {
        project,
        expected: base_expected
            .as_ref()
            .expect("clap requires complete base context"),
        source_root: args
            .base_source_root
            .as_ref()
            .expect("clap requires base source root"),
        artifact_root: args
            .base_artifact_root
            .as_ref()
            .expect("clap requires base artifact root"),
    });
    let now = args
        .now
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    chrono::DateTime::parse_from_rfc3339(&now).context("invalid evaluation clock")?;
    let result = policy::evaluate(
        &rules,
        &records,
        &head,
        &policy::EvaluationOptions {
            selection: selection.as_ref(),
            base_records: base_records.as_ref(),
            base_context: base.as_ref(),
            mappings: mappings.as_ref(),
            exceptions: exceptions.as_ref(),
            now: Some(&now),
        },
    )?;
    let report = project_report::report(&result, &project, &rules)?;
    crate::utils::fs::atomic_write(
        &args.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
        true,
    )?;
    Ok(report["aggregate"]["state"] == "pass")
}

fn read_state(path: &PathBuf) -> Result<TrustedState> {
    Ok(serde_json::from_value(read(path)?)?)
}

fn clear_output<'a>(
    output: &std::path::Path,
    inputs: impl IntoIterator<Item = Option<&'a PathBuf>>,
) -> Result<()> {
    if output.exists() {
        let destination = output.canonicalize()?;
        for input in inputs.into_iter().flatten() {
            anyhow::ensure!(
                !input.canonicalize().is_ok_and(|path| path == destination),
                "output aliases an input"
            );
        }
        fs::remove_file(output).context("remove stale generic output")?;
    }
    Ok(())
}

// Compilation outputs must not overwrite the inputs pinned by the host.
fn protect_output(
    output: &std::path::Path,
    root: &std::path::Path,
    state: &TrustedState,
) -> Result<()> {
    if !output.exists() {
        return Ok(());
    }
    let output = output.canonicalize()?;
    let inputs = state
        .config_files
        .keys()
        .map(|p| root.join(p))
        .chain(
            state
                .subjects
                .values()
                .flatten()
                .map(|s| root.join(&s.path)),
        )
        .chain(
            state
                .artifacts
                .keys()
                .map(|p| root.join(&state.artifact_root).join(p)),
        );
    for input in inputs {
        anyhow::ensure!(
            !input.canonicalize().is_ok_and(|p| p == output),
            "output aliases a pinned input"
        );
    }
    Ok(())
}
