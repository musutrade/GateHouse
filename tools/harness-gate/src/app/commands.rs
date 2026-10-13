use super::output::print_scope;
use crate::cli::{Commands, CompatAction, ConfigAction, ConfigFormat};
use crate::error::CliError;
use crate::project::InputMode;
use crate::project::Project;
use crate::scope::ScopeMode;
use crate::ui;
use anyhow::Context;

pub(super) fn run(project: &Project, command: Commands) -> Result<bool, CliError> {
    match command {
        Commands::Quality { .. } => {
            unreachable!("generic evaluation is handled before project discovery")
        }
        Commands::Adapter { .. } => {
            unreachable!("adapter execution is handled before project discovery")
        }
        Commands::Compat {
            action:
                CompatAction::Run {
                    input,
                    output,
                    old_result,
                },
        } => {
            let response = crate::compat::run(project, &input, &output, old_result.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response).map_err(anyhow::Error::from)?
            );
            Ok(response.status == "PASS"
                && response
                    .comparison
                    .as_ref()
                    .is_none_or(|result| result.equivalent))
        }
        Commands::Compat { .. } => {
            unreachable!("compatibility maintenance actions handled before project discovery")
        }
        Commands::Doctor { json, strict } => run_doctor(project, json, strict),
        Commands::Cleanup { dry_run, json } => run_cleanup(project, dry_run, json),
        Commands::Scope {
            scope: args,
            json,
            benchmark_repeat,
        } => run_scope(project, args.mode(), json, benchmark_repeat),
        Commands::Secrets { staged, json } => run_secrets(project, staged, json),
        Commands::Audit { json } => run_audit(project, json),
        Commands::Verify {
            scope: args,
            components,
            profile,
        } => run_verify(project, args.mode(), components, profile),
        Commands::Hook => run_hook(project),
        Commands::ParseLogs { input, output } => {
            crate::audit::parse_logs(&input, &output)?;
            println!("Error context: {}", output.display());
            Ok(true)
        }
        Commands::Config { action } => run_config(project, action),
        Commands::Step { id } => Ok(crate::verify::run_step(project, &id)?.passed),
        Commands::Init { .. } | Commands::Presets | Commands::Schema { .. } => {
            unreachable!("handled before project discovery")
        }
    }
}

fn run_doctor(project: &Project, json: bool, strict: bool) -> Result<bool, CliError> {
    let report = crate::doctor::run(project)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?
        );
    } else {
        report.print();
    }
    Ok(report.failures == 0 && (!strict || report.warnings == 0))
}

fn run_cleanup(project: &Project, dry_run: bool, json: bool) -> Result<bool, CliError> {
    let report = crate::service::cleanup_resources(project, dry_run)?;
    crate::utils::fs::confined_write_json(
        &project.reports,
        std::path::Path::new("cleanup.json"),
        &report,
        true,
    )
    .context("write cleanup evidence")?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?
        );
    } else {
        println!(
            "Cleanup {}: scanned {}, active {}, stale {}, reclaimed {}",
            if dry_run { "(dry-run)" } else { "complete" },
            report.scanned,
            report.active,
            report.stale,
            report.reclaimed
        );
        for resource in &report.resources {
            println!(
                "  {:<12} {:<24} {}",
                resource.action, resource.resource_id, resource.lease_file
            );
        }
        for failure in &report.failures {
            eprintln!("  cleanup failure: {failure}");
        }
    }
    Ok(report.failures.is_empty())
}

fn run_scope(
    project: &Project,
    mode: ScopeMode,
    json: bool,
    benchmark_repeat: usize,
) -> Result<bool, CliError> {
    if benchmark_repeat > 0 {
        let benchmark = crate::scope::benchmark(project, &mode, benchmark_repeat)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&benchmark).map_err(anyhow::Error::from)?
        );
        return Ok(true);
    }
    let result = crate::scope::detect(project, &mode)?;
    result.write_reports(project)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(anyhow::Error::from)?
        );
    } else {
        print_scope(&result);
    }
    Ok(true)
}

fn run_secrets(project: &Project, staged: bool, json: bool) -> Result<bool, CliError> {
    let mode = if staged {
        crate::secrets::SecretMode::Staged
    } else {
        crate::secrets::SecretMode::WorkingTree
    };
    let findings = crate::secrets::scan(project, mode)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "passed": findings.is_empty(),
                "findings": findings,
            }))
            .map_err(anyhow::Error::from)?
        );
    } else if findings.is_empty() {
        println!("{}", ui::pass("Secret scan passed"));
    } else {
        eprintln!(
            "{}",
            ui::error(format!("Secret scan failed in {} file(s):", findings.len()))
        );
        for file in &findings {
            eprintln!("  {file}");
        }
        eprintln!("Remove and revoke each credential before continuing.");
    }
    Ok(findings.is_empty())
}

fn run_audit(project: &Project, json: bool) -> Result<bool, CliError> {
    let outcome = crate::audit::run(
        &project.execution_root,
        &project.audit_config,
        &project.reports,
        json,
    )?;
    if !json {
        let summary = format!(
            "Audit: {} violation(s), {} blocker(s), {} error(s), {} warning(s)",
            outcome.total_violations,
            outcome.blocker_count,
            outcome.error_count,
            outcome.warning_count
        );
        println!(
            "{}",
            if outcome.total_violations == 0 {
                ui::pass(summary)
            } else {
                ui::failure(summary)
            }
        );
        println!("Report: {}", outcome.report_file.display());
    }
    Ok(outcome.total_violations == 0)
}

fn run_verify(
    project: &Project,
    mode: ScopeMode,
    components: Vec<String>,
    profile: Option<String>,
) -> Result<bool, CliError> {
    let execution_project = match &mode {
        ScopeMode::Staged => project.staged_snapshot()?,
        ScopeMode::Base(_) => project.clone().with_input_mode(InputMode::Base),
        ScopeMode::All => project.clone().with_input_mode(InputMode::All),
        ScopeMode::WorkingTree => project.clone(),
    };
    let selected = if components.is_empty() {
        crate::scope::detect(&execution_project, &mode)?
    } else {
        let known = execution_project.config.components();
        for component in &components {
            if !known.contains(component) {
                return Err(CliError::from(anyhow::anyhow!(
                    "unknown component {component:?}"
                )));
            }
        }
        crate::verify::explicit_scope(&components)
    };
    let profile =
        profile.unwrap_or_else(|| execution_project.config.project.default_profile.clone());
    Ok(crate::verify::run(
        &execution_project,
        selected,
        &profile,
        matches!(mode, ScopeMode::Staged),
    )?
    .passed)
}

fn run_hook(project: &Project) -> Result<bool, CliError> {
    let execution_project = project.staged_snapshot()?;
    let selected = crate::scope::detect(&execution_project, &ScopeMode::Staged)?;
    Ok(crate::verify::run(
        &execution_project,
        selected,
        &execution_project.config.project.hook_profile,
        true,
    )?
    .passed)
}

fn run_config(project: &Project, action: ConfigAction) -> Result<bool, CliError> {
    let quality_config = crate::config::quality::QualityConfig::load_optional(
        &project.execution_root,
        &project.config,
    )?;
    match action {
        ConfigAction::Check {
            format: ConfigFormat::Human,
        } => {
            println!("Configuration valid: {}", project.config_path.display());
            println!("Schema version: {}", project.config.version);
            println!(
                "Components: {}",
                project
                    .config
                    .components()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "Profiles: {}",
                project
                    .config
                    .steps
                    .iter()
                    .flat_map(|step| step.profiles.iter().cloned())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("Verification steps: {}", project.config.steps.len());
            if quality_config.is_some() {
                println!("Quality configuration valid: .harness-gate/quality.toml (v1)");
            }
            Ok(true)
        }
        ConfigAction::Check {
            format: ConfigFormat::Json,
        } => {
            let report = project.config.diagnostics_report();
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?
            );
            Ok(report.valid)
        }
        ConfigAction::Print { resolved, quality } => {
            if quality {
                let config =
                    quality_config.context("quality.toml is absent; quality is not enabled")?;
                if resolved {
                    println!(
                        "{}",
                        toml::to_string_pretty(&config).map_err(anyhow::Error::from)?
                    );
                } else {
                    let path = crate::project::resolve_repo_path(
                        &project.execution_root,
                        std::path::Path::new(crate::config::quality::QUALITY_CONFIG_PATH),
                        "quality configuration",
                        true,
                    )?;
                    print!(
                        "{}",
                        std::fs::read_to_string(path).context("read quality.toml")?
                    );
                }
            } else if resolved {
                println!(
                    "{}",
                    toml::to_string_pretty(&project.config).map_err(anyhow::Error::from)?
                );
            } else {
                print!(
                    "{}",
                    std::fs::read_to_string(&project.config_path).with_context(|| format!(
                        "read workflow config {}",
                        project.config_path.display()
                    ))?
                );
            }
            Ok(true)
        }
        ConfigAction::Migrate { .. } | ConfigAction::Import { .. } => {
            unreachable!("handled before project discovery")
        }
    }
}
