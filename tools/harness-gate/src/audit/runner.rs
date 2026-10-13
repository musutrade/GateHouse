use super::config::{parse_audit_config, validate_audit_config, Config};
use super::report::{generate_markdown, generate_report};
use super::scanner::{resolve_rule_roots, scan_arch_rules, scan_files};
use super::{log_parser, AuditError, AuditOutcome};
use crate::utils::fs as output_fs;
use anyhow::{Context, Result};
use std::path::Path;

pub fn run(
    project_root: &Path,
    config_path: &Path,
    report_dir: &Path,
    emit_json: bool,
) -> std::result::Result<AuditOutcome, AuditError> {
    let config_str = crate::config::read_config_source(config_path)
        .with_context(|| format!("read audit config {}", config_path.display()))
        .map_err(AuditError::configuration)?;
    let config = parse_audit_config(&config_str)
        .with_context(|| format!("parse audit config {}", config_path.display()))
        .map_err(AuditError::configuration)?;
    run_with_config(project_root, report_dir, emit_json, config).map_err(AuditError::execution)
}

fn run_with_config(
    project_root: &Path,
    report_dir: &Path,
    emit_json: bool,
    config: Config,
) -> Result<AuditOutcome> {
    let exclude_dirs = validate_audit_config(project_root, &config)?;
    let mut all_hard_violations = Vec::new();
    for rule in &config.hard_rules {
        let root_paths =
            resolve_rule_roots(project_root, &rule.paths, &config.paths.aliases, &rule.name)?;
        all_hard_violations.extend(scan_files(
            project_root,
            &root_paths,
            &exclude_dirs,
            rule,
            &config.engine,
        )?);
    }
    let arch_violations = scan_arch_rules(project_root, &config, &exclude_dirs)?;
    let report = generate_report(&config, &all_hard_violations, &arch_violations);
    let full_json = serde_json::to_string_pretty(&report)?;
    let outcome = AuditOutcome {
        total_violations: report.summary.total_violations,
        blocker_count: report.summary.blocker_count,
        error_count: report.summary.error_count,
        warning_count: report.summary.warning_count,
        report_file: report_dir.join(&config.engine.json_report_filename),
    };
    output_fs::confined_atomic_write(
        report_dir,
        Path::new(&config.engine.json_report_filename),
        &full_json,
        true,
    )?;
    let markdown = generate_markdown(&config, &all_hard_violations, &arch_violations);
    let truncated = if markdown.len() > config.engine.markdown_max_bytes {
        let mut value = markdown;
        let mut boundary = config.engine.markdown_max_bytes;
        while !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        value.truncate(boundary);
        value.push_str(&format!(
            "\n\n... (report truncated to {} bytes; see {})",
            config.engine.markdown_max_bytes, config.engine.json_report_filename
        ));
        value
    } else {
        markdown
    };
    output_fs::confined_atomic_write(
        report_dir,
        Path::new(&config.engine.markdown_report_filename),
        truncated,
        true,
    )?;
    if emit_json {
        println!("{full_json}");
    }
    Ok(outcome)
}

pub fn parse_logs(input: &Path, output: &Path) -> std::result::Result<(), AuditError> {
    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
        .with_context(|| format!("parse log file {}", input.display()))
        .map_err(AuditError::log_parsing)
}
