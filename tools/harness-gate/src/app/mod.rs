mod commands;
mod output;
mod plan;
pub(crate) mod quality;

use crate::cli::{
    AdapterAction, Cli, Commands, CompatAction, ConfigAction, ConfigFormat, SchemaAction,
};
use crate::error::CliError;
use crate::project::Project;
use anyhow::Context;
use clap::Parser;
use std::collections::BTreeSet;
use std::fs;

pub(crate) fn run() -> Result<bool, CliError> {
    run_cli(Cli::parse())
}

pub(crate) fn run_cli(cli: Cli) -> Result<bool, CliError> {
    crate::ui::configure(cli.color);
    match &cli.command {
        Commands::Quality { action } => {
            quality::run(action, cli.replay_state_dir.as_deref()).map_err(Into::into)
        }
        Commands::Init { preset, force } => {
            let directory = cli.presets_dir.clone().or_else(|| {
                std::env::var_os("HARNESS_GATE_PRESETS_DIR").map(std::path::PathBuf::from)
            });
            if let Some(directory) = directory {
                crate::preset::init_from(
                    &standalone_root(&cli)?,
                    preset,
                    *force,
                    Some(&directory),
                )?;
            } else {
                crate::preset::init(&standalone_root(&cli)?, preset, *force)?;
            }
            Ok(true)
        }
        Commands::Presets => {
            let directory = cli.presets_dir.clone().or_else(|| {
                std::env::var_os("HARNESS_GATE_PRESETS_DIR").map(std::path::PathBuf::from)
            });
            crate::preset::print_presets(directory.as_deref())?;
            Ok(true)
        }
        Commands::Adapter { action } => run_adapter(action, cli.replay_state_dir.as_deref()),
        Commands::Compat {
            action: CompatAction::Compare { old, new, output },
        } => compare_files(old, new, output),
        Commands::Compat {
            action: CompatAction::Canary { state, slice },
        } => {
            let state = crate::compat::set_canary(state, slice)?;
            println!("Canary enabled for {}: {}", state.slice, state.updated_at);
            Ok(true)
        }
        Commands::Compat {
            action: CompatAction::Rollback { state },
        } => {
            let state = crate::compat::rollback(state)?;
            println!("Canary rolled back: {}", state.updated_at);
            Ok(true)
        }
        Commands::Schema {
            action: SchemaAction::Export { output, quality },
        } => export_schema(standalone_root(&cli)?, output.as_deref(), *quality),
        Commands::Config {
            action:
                ConfigAction::Import {
                    input,
                    output,
                    execution_only,
                },
        } => {
            crate::preset::import_arc_flow(
                &standalone_root(&cli)?,
                input,
                output,
                *execution_only,
            )?;
            Ok(true)
        }
        Commands::Config {
            action:
                ConfigAction::Migrate {
                    input,
                    output,
                    force,
                },
        } => {
            crate::preset::migrate(
                &standalone_root(&cli)?,
                input.clone().or_else(|| cli.config.clone()),
                output.clone(),
                *force,
            )?;
            Ok(true)
        }
        Commands::Config {
            action: ConfigAction::Check {
                format: ConfigFormat::Json,
            },
        } => check_config_json(cli.project_root.clone(), cli.config.clone()),
        Commands::Verify {
            plan: true,
            scope,
            components,
            profile,
            json,
        } => plan::run(
            cli.project_root.clone(),
            cli.config.clone(),
            scope.mode(),
            components,
            profile.as_deref(),
            *json,
        ),
        _ => run_project(
            cli.project_root,
            cli.config,
            cli.command,
            cli.replay_state_dir,
        ),
    }
}

fn standalone_root(cli: &Cli) -> Result<std::path::PathBuf, CliError> {
    Ok(cli
        .project_root
        .clone()
        .unwrap_or(std::env::current_dir().context("read current directory")?))
}

fn run_adapter(
    action: &AdapterAction,
    replay_state_dir: Option<&std::path::Path>,
) -> Result<bool, CliError> {
    let AdapterAction::Run {
        request,
        trusted_keys,
        allow_network,
        allow_resources,
        allow_environment,
    } = action;
    let request_path = request.clone();
    let request = crate::process::read_adapter_request(request)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let trusted_keys = trusted_keys
        .iter()
        .map(|path| {
            let bytes = fs::read(path)
                .with_context(|| format!("read trusted adapter key {}", path.display()))?;
            serde_json::from_slice(&bytes)
                .with_context(|| format!("parse trusted adapter key {}", path.display()))
        })
        .collect::<anyhow::Result<Vec<crate::process::TrustedKey>>>()?;
    let policy = crate::process::HostPolicy {
        trusted_keys,
        capabilities: crate::process::CapabilityPolicy {
            network: allow_network.iter().cloned().collect::<BTreeSet<_>>(),
            resources: allow_resources.iter().cloned().collect::<BTreeSet<_>>(),
            environment: allow_environment.iter().cloned().collect::<BTreeSet<_>>(),
        },
        replay_state_dir: Some(
            replay_state_dir
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| {
                    request_path
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join(".harness-gate-adapter-replay")
                }),
        ),
        ..crate::process::HostPolicy::default()
    };
    let outcome = crate::process::run_adapter(request, &policy)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&outcome.response).map_err(anyhow::Error::from)?
    );
    Ok(true)
}

fn compare_files(
    old: &std::path::Path,
    new: &std::path::Path,
    output: &std::path::Path,
) -> Result<bool, CliError> {
    let comparison = crate::compat::compare_files(old, new)?;
    crate::utils::fs::atomic_write(
        output,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&comparison).map_err(anyhow::Error::from)?
        ),
        true,
    )
    .with_context(|| format!("write comparison report {}", output.display()))?;
    println!("Comparison written: {}", output.display());
    Ok(comparison.equivalent)
}

fn export_schema(
    root: std::path::PathBuf,
    output: Option<&std::path::Path>,
    quality: bool,
) -> Result<bool, CliError> {
    let output = output.unwrap_or_else(|| {
        std::path::Path::new(if quality {
            "schema/quality.schema.json"
        } else {
            "schema/flow.schema.json"
        })
    });
    let schema = if quality {
        crate::config::quality::schema_json()?
    } else {
        crate::config::schema_json()?
    };
    if output.is_absolute() {
        return Err(anyhow::anyhow!("schema output must be relative to the project root").into());
    }
    let path = crate::utils::fs::confined_atomic_write(&root, output, format!("{schema}\n"), true)
        .with_context(|| format!("write workflow schema {}", root.join(output).display()))?;
    println!("Schema written: {}", path.display());
    Ok(true)
}

fn check_config_json(
    root: Option<std::path::PathBuf>,
    config: Option<std::path::PathBuf>,
) -> Result<bool, CliError> {
    match Project::discover(root, config).and_then(|project| {
        crate::config::quality::QualityConfig::load_optional(
            &project.execution_root,
            &project.config,
        )?;
        Ok(project)
    }) {
        Ok(project) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&project.config.diagnostics_report())
                    .map_err(anyhow::Error::from)?
            );
            Ok(true)
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&crate::config::report_for_error(&error))
                    .map_err(anyhow::Error::from)?
            );
            Ok(false)
        }
    }
}

fn run_project(
    root: Option<std::path::PathBuf>,
    config: Option<std::path::PathBuf>,
    command: Commands,
    replay_state_dir: Option<std::path::PathBuf>,
) -> Result<bool, CliError> {
    let is_config_check = matches!(
        &command,
        Commands::Config {
            action: ConfigAction::Check { .. }
        }
    );
    let mut project = match Project::discover(root, config) {
        Ok(project) => project,
        Err(error) if is_config_check => {
            print_human_config_error(&error);
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    project.replay_state_dir = replay_state_dir;
    if !is_config_check {
        project.prepare()?;
    }
    commands::run(&project, command)
}

fn print_human_config_error(error: &anyhow::Error) {
    if let Some(diagnostics) = error.downcast_ref::<crate::config::ConfigDiagnostics>() {
        eprintln!(
            "{}",
            crate::ui::error("ERROR [E1000]: configuration check failed")
        );
        eprintln!("{}", diagnostics);
    } else {
        eprintln!(
            "{}",
            crate::ui::error(format!(
                "ERROR [E1000]: configuration check failed: {error:#}"
            ))
        );
        let report = crate::config::report_for_error(error);
        for diagnostic in report.diagnostics {
            eprintln!("  help: {}", diagnostic.help);
        }
    }
    eprintln!("Next: harness-gate init --preset generic");
    eprintln!("Minimal flow.toml shape:");
    eprintln!("{}", crate::config::MINIMAL_CONFIG_SNIPPET);
    eprintln!("Then run: harness-gate config check");
}
