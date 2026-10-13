pub(crate) mod parser;
mod plan;
mod quality;
mod report;
mod scheduler;
mod steps;
mod waiver;

#[cfg(test)]
mod tests;

use crate::error::CodedError;
use crate::failure::FailureCode;
use crate::process::TaskResult;
use crate::project::Project;
use crate::scope::ScopeResult;
use crate::service::ServiceManager;
use crate::ui::{self, Progress};
use crate::utils::redaction::redact_text;
use plan::VerificationPlan;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;
use steps::{print_external_result, print_result};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("unknown or empty verification profile {profile:?}")]
    UnknownProfile { profile: String },
    #[error("unknown verification step {id:?}")]
    UnknownStep { id: String },
    #[error("verification step {id:?} is not part of profile {profile:?}")]
    StepNotInProfile { id: String, profile: String },
    #[error("verification was cancelled")]
    Cancelled,
    #[error("verification execution failed: {message}")]
    Execution { message: String },
    #[error("verification report failed: {message}")]
    Report { message: String },
    #[error(transparent)]
    Scope(#[from] crate::scope::ScopeError),
    #[error(transparent)]
    Secrets(#[from] crate::secrets::SecretsError),
    #[error(transparent)]
    Audit(#[from] crate::audit::AuditError),
}

impl VerifyError {
    fn execution(error: anyhow::Error) -> Self {
        Self::Execution {
            message: redact_text(&format!("{error:#}")),
        }
    }

    fn report(error: anyhow::Error) -> Self {
        Self::Report {
            message: redact_text(&format!("{error:#}")),
        }
    }
}

impl CodedError for VerifyError {
    fn code(&self) -> &'static str {
        match self {
            Self::UnknownProfile { .. }
            | Self::UnknownStep { .. }
            | Self::StepNotInProfile { .. } => "E1401",
            Self::Cancelled => "E1402",
            Self::Execution { .. } => "E1403",
            Self::Report { .. } => "E1404",
            Self::Scope(error) => error.code(),
            Self::Secrets(error) => error.code(),
            Self::Audit(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub struct VerificationReport {
    pub invocation_id: String,
    pub executor_version: String,
    pub report_directory: String,
    pub timestamp: String,
    pub profile: String,
    pub input_mode: String,
    pub project_identity: String,
    pub source_identity: String,
    pub execution_root: String,
    pub configuration_digest: String,
    pub scope: ScopeResult,
    pub services: Vec<ServiceResult>,
    pub steps: Vec<TaskResult>,
    /// Steps that were not dispatched because a prerequisite failed.
    pub skipped_steps: Vec<SkippedStep>,
    pub passed: bool,
    pub quality: Option<quality::QualityResult>,
}

#[derive(Debug, Clone)]
pub struct ServiceResult {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct SkippedStep {
    pub id: String,
    pub label: String,
    pub reason: String,
    /// Whether dispatch was stopped by cancellation rather than a failed
    /// prerequisite. This remains an internal field so schema version 1 keeps
    /// its existing skipped-step shape.
    #[serde(skip)]
    pub cancelled: bool,
}

pub fn run(
    project: &Project,
    scope: ScopeResult,
    profile: &str,
    staged: bool,
) -> std::result::Result<VerificationReport, VerifyError> {
    if !project
        .config
        .steps
        .iter()
        .any(|step| step.profiles.contains(profile))
    {
        return Err(VerifyError::UnknownProfile {
            profile: profile.to_string(),
        });
    }
    run_selected(project, scope, profile, staged, None, None)
}

pub(crate) fn run_with_request_id(
    project: &Project,
    scope: ScopeResult,
    profile: &str,
    staged: bool,
    request_id: Option<&str>,
) -> std::result::Result<VerificationReport, VerifyError> {
    if !project
        .config
        .steps
        .iter()
        .any(|step| step.profiles.contains(profile))
    {
        return Err(VerifyError::UnknownProfile {
            profile: profile.to_string(),
        });
    }
    run_selected(project, scope, profile, staged, None, request_id)
}

pub fn run_step(
    project: &Project,
    id: &str,
) -> std::result::Result<VerificationReport, VerifyError> {
    let step = project
        .config
        .step(id)
        .ok_or_else(|| VerifyError::UnknownStep { id: id.to_string() })?;
    let profile = project.config.project.default_profile.clone();
    if !step.profiles.contains(&profile) {
        return Err(VerifyError::StepNotInProfile {
            id: id.to_string(),
            profile,
        });
    }
    run_selected(
        project,
        explicit_scope(std::slice::from_ref(&step.component)),
        &project.config.project.default_profile,
        false,
        Some(id),
        None,
    )
}

fn run_selected(
    project: &Project,
    scope: ScopeResult,
    profile: &str,
    staged: bool,
    only_step: Option<&str>,
    request_id: Option<&str>,
) -> std::result::Result<VerificationReport, VerifyError> {
    if project
        .config
        .execution
        .max_parallel
        .is_some_and(|limit| limit == 0 || limit > 64)
    {
        return Err(VerifyError::execution(anyhow::anyhow!(
            "execution.max_parallel must be between 1 and 64"
        )));
    }
    println!("{}", ui::heading("harness-gate verify"));
    println!("Scope: {}", scope.mode);
    println!(
        "Components: {}\n",
        if scope.components.is_empty() {
            "none".to_string()
        } else {
            scope
                .components
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    let plan = VerificationPlan::build(project, &scope, profile, only_step)
        .map_err(VerifyError::execution)?;
    waiver::validate_plan_waivers(project, &plan, &scope).map_err(VerifyError::execution)?;
    let invocation = report::allocate_invocation(project).map_err(VerifyError::report)?;
    if let Some(request_id) = request_id {
        report::write_invocation_metadata_with_request(
            &invocation,
            project,
            profile,
            staged,
            Some(request_id),
        )
        .map_err(VerifyError::report)?;
    } else {
        report::write_invocation_metadata(&invocation, project, profile, staged)
            .map_err(VerifyError::report)?;
    }
    let mut invocation_project = project.clone();
    invocation_project.reports = invocation.root.clone();
    let mut progress = Progress::new(plan.nodes.len());
    scope.write_reports(&invocation_project)?;
    let quality_inputs = if only_step.is_none() {
        quality::prepare(project, &scope, profile)
    } else {
        Ok(None)
    };
    let services = Mutex::new(ServiceManager::new(&invocation_project));
    let scheduler_result = scheduler::run_plan(
        &invocation_project,
        &plan,
        staged,
        &services,
        &scope,
        if project.config.execution.parallel {
            project.config.execution.effective_max_parallel()
        } else {
            1
        },
    );
    let cleanup_result = services
        .lock()
        .map_err(|_| anyhow::anyhow!("service manager lock was poisoned"))
        .and_then(|mut manager| manager.cleanup());
    let outcome = match scheduler_result {
        Ok(outcome) => outcome,
        Err(error) => {
            let cleanup = cleanup_result.err().map(|cleanup| format!("; {cleanup:#}"));
            let detail = match error {
                scheduler::SchedulerError::Secrets(error) => VerifyError::Secrets(error),
                scheduler::SchedulerError::Audit(error) => VerifyError::Audit(error),
                scheduler::SchedulerError::Execution(error) => VerifyError::execution(
                    anyhow::anyhow!("{error:#}{}", cleanup.unwrap_or_default()),
                ),
            };
            return Err(detail);
        }
    };
    let cleanup_error = cleanup_result.err();
    let services = service_results(&plan, cleanup_error.is_some());
    let scheduler::SchedulerOutcome {
        results,
        cancelled,
        failures,
    } = outcome;
    let (steps, skipped_steps) = merge_results(
        &plan,
        results,
        &invocation,
        &invocation_project,
        &mut progress,
        cleanup_error.as_ref(),
    );
    let quality = quality::run(&invocation_project, profile, quality_inputs);
    if let Some(result) = &quality {
        println!("{}", quality::diagnostics(result));
    }
    let passed = quality.as_ref().is_none_or(|result| result.passed())
        && !cancelled
        && failures.is_empty()
        && cleanup_error.is_none()
        && steps.iter().all(|step| step.passed)
        && steps.len() == plan.nodes.len();
    let report = VerificationReport {
        invocation_id: invocation.id.clone(),
        executor_version: env!("CARGO_PKG_VERSION").into(),
        report_directory: invocation.root.to_string_lossy().into_owned(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        profile: only_step
            .map(|id| format!("step:{id}"))
            .unwrap_or_else(|| profile.to_string()),
        input_mode: invocation_project.input().mode.as_str().into(),
        project_identity: invocation_project.input().project_identity.clone(),
        source_identity: invocation_project.input().source_identity.clone(),
        execution_root: invocation_project
            .input()
            .execution_root
            .to_string_lossy()
            .into_owned(),
        configuration_digest: invocation_project.input().configuration_digest.clone(),
        scope,
        services,
        steps,
        skipped_steps,
        passed,
        quality,
    };
    publish_report(&report, &invocation_project, project, &mut progress)?;

    // Publication happens before returning cancellation or adapter failures so
    // callers retain the same report and log evidence as a normal run.
    if cancelled {
        return Err(VerifyError::Cancelled);
    }
    if let Some(failure) = scheduler::primary_failure(&plan, failures) {
        return Err(match failure.error {
            scheduler::SchedulerError::Secrets(error) => VerifyError::Secrets(error),
            scheduler::SchedulerError::Audit(error) => VerifyError::Audit(error),
            scheduler::SchedulerError::Execution(error) => VerifyError::execution(error),
        });
    }
    if let Some(error) = cleanup_error {
        return Err(VerifyError::execution(error));
    }
    Ok(report)
}

fn service_results(plan: &VerificationPlan<'_>, cleanup_failed: bool) -> Vec<ServiceResult> {
    let service_ids = plan
        .nodes
        .iter()
        .filter_map(|node| node.step)
        .flat_map(|step| step.services.iter().cloned())
        .collect::<BTreeSet<_>>();
    service_ids
        .into_iter()
        .map(|id| ServiceResult {
            id,
            status: if cleanup_failed {
                "LEAKED".into()
            } else {
                "CLEANED".into()
            },
        })
        .collect()
}

fn merge_results(
    plan: &VerificationPlan<'_>,
    results: Vec<scheduler::ScheduledResult>,
    invocation: &report::Invocation,
    invocation_project: &Project,
    progress: &mut Progress,
    cleanup_error: Option<&anyhow::Error>,
) -> (Vec<TaskResult>, Vec<SkippedStep>) {
    let mut ordered = results;
    ordered.sort_by_key(|result| {
        plan.nodes
            .iter()
            .position(|node| node.id == result.node_id)
            .unwrap_or(usize::MAX)
    });
    let mut steps = Vec::new();
    let mut skipped_steps = Vec::new();
    for result in ordered {
        progress.clear();
        if result.node_result.status == plan::NodeStatus::Skipped {
            skipped_steps.push(SkippedStep {
                id: result.node_result.id,
                label: result.node_result.label,
                reason: result
                    .node_result
                    .reason
                    .unwrap_or_else(|| "blocked by a failed prerequisite".into()),
                cancelled: result.node_result.cancelled,
            });
        } else {
            let mut task_result = result.task_result;
            task_result.step_id = Some(result.node_id.clone());
            task_result.invocation_id = Some(invocation.id.clone());
            // The scheduler preserves the terminal attempt selected by the
            // retry policy. Legacy/builtin results have no attempt yet and
            // are normalized to one at the report boundary.
            if task_result.attempt.is_none() {
                task_result.attempt = Some(1);
            }
            if !task_result.log.is_empty() && !Path::new(&task_result.log).is_absolute() {
                task_result.log = invocation_project
                    .reports
                    .join("logs")
                    .join(&task_result.log)
                    .to_string_lossy()
                    .into_owned();
            }
            match result.node_result.kind {
                plan::PlanNodeKind::Builtin(_) => print_result(&task_result),
                plan::PlanNodeKind::External => {
                    if progress.enabled() {
                        print_result(&task_result);
                    } else {
                        print_external_result(&task_result);
                    }
                }
            }
            steps.push(task_result);
        }
        progress.complete();
    }
    if let Some(error) = cleanup_error {
        steps.push(TaskResult {
            step_id: Some("service.cleanup".into()),
            invocation_id: Some(invocation.id.clone()),
            attempt: Some(1),
            started_at: None,
            finished_at: None,
            label: "service cleanup".into(),
            passed: false,
            timed_out: false,
            cancelled: false,
            duration_ms: 0,
            log: String::new(),
            detail: Some(format!("{error:#}")),
            failure_code: Some(FailureCode::SchedulerFailure),
            attempts: Vec::new(),
            flaky: false,
            retry_class: None,
            parser: None,
            waived: false,
            waiver: None,
            runner: None,
        });
    }
    (steps, skipped_steps)
}

fn publish_report(
    report: &VerificationReport,
    invocation_project: &Project,
    project: &Project,
    progress: &mut Progress,
) -> Result<(), VerifyError> {
    report::write(report, invocation_project).map_err(VerifyError::report)?;
    quality::publish(report, project).map_err(VerifyError::report)?;
    report::mirror_legacy_outputs(invocation_project, project).map_err(VerifyError::report)?;
    report::notify(report, invocation_project).map_err(VerifyError::report)?;
    progress.finish();
    println!(
        "\nVerification report: {}",
        project.reports.join("test_result.md").display()
    );
    println!(
        "TEST_SUMMARY: {}",
        if report.passed { "PASS" } else { "FAIL" }
    );

    Ok(())
}

pub fn explicit_scope(components: &[String]) -> ScopeResult {
    ScopeResult {
        mode: "components".to_string(),
        changed_files: Vec::new(),
        components: components.iter().cloned().collect::<BTreeSet<_>>(),
        unmatched_files: Vec::new(),
    }
}
