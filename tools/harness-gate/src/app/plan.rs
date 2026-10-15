//! Read-only CLI boundary: deliberately bypasses Project::prepare and execution.
use crate::{error::CliError, project::Project, scope::ScopeMode, verify::inspection};
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;

pub(super) fn run(
    root: Option<PathBuf>,
    config: Option<PathBuf>,
    mode: ScopeMode,
    components: &[String],
    profile: Option<&str>,
    json_output: bool,
    plugins: &crate::plugins::HostOptions,
) -> Result<bool, CliError> {
    let result = inspect(root, config, &mode, components, profile, plugins);
    let valid = result.is_ok();
    let envelope = match result {
        Ok(plan) => json!({
            "schema": "verification-plan/v1", "status": "planned",
            "verification_result": null, "plan": plan, "diagnostics": [],
        }),
        Err(error) => json!({
            "schema": "verification-plan/v1", "status": "invalid",
            "verification_result": null, "plan": null,
            "diagnostics": [{
                "code": "PLAN_INVALID",
                "message": crate::utils::redaction::redact_text(&format!("{error:#}")),
                "configuration": crate::config::report_for_error(&error).diagnostics,
            }],
        }),
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).map_err(anyhow::Error::from)?
        );
    } else {
        print!("{}", inspection::render(&envelope));
    }
    Ok(valid)
}

fn inspect(
    root: Option<PathBuf>,
    config: Option<PathBuf>,
    mode: &ScopeMode,
    components: &[String],
    profile: Option<&str>,
    plugins: &crate::plugins::HostOptions,
) -> Result<serde_json::Value> {
    let project = Project::discover_for_plan(root, config)?;
    let project = if matches!(mode, ScopeMode::Staged) {
        project.staged_plan_snapshot()?
    } else {
        project
    };
    crate::plugins::check(&project.execution_root, plugins)?;
    let scope = crate::verify::select_scope(&project, mode, components, true)?;
    let explicit_profile = profile.is_some();
    let profile = profile.unwrap_or(&project.config.project.default_profile);
    inspection::build(&project, &scope, profile, explicit_profile)
}
