use super::parser::parse_result_count;
use crate::config::{ParserConfig, StepConfig, TestIsolation};
use crate::failure::{FailureCode, RetryClass};
use crate::process::{
    allocate_isolation, ParserEvidence, RunnerExecution, Task, TaskAttempt, TaskResult,
};
use crate::project::Project;
use crate::service::{ServiceLease, ServiceManager};
use crate::ui;
use anyhow::{bail, Context, Result};
use std::fs;
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

const SERVICE_LOCK_POLL: Duration = Duration::from_millis(25);
const SERVICE_LOCK_WAIT: Duration = Duration::from_secs(30);

pub(super) fn run_configured_step<'a>(
    project: &'a Project,
    step: &StepConfig,
    services: &'a Mutex<ServiceManager<'a>>,
) -> Result<TaskResult> {
    let mut service_leases = Vec::<ServiceLease>::new();
    for service in &step.services {
        let lease = match lock_services(services) {
            Ok(mut manager) => manager.handle(service),
            Err(error) => {
                let cancelled = crate::process::cancelled();
                return Ok(TaskResult {
                    step_id: None,
                    invocation_id: None,
                    attempt: None,
                    started_at: None,
                    finished_at: None,
                    label: format!("{}: service {service} setup", step.label),
                    passed: false,
                    timed_out: false,
                    cancelled,
                    duration_ms: 0,
                    log: String::new(),
                    detail: Some(format!("{error:#}")),
                    failure_code: Some(FailureCode::ServiceSetupFailure),
                    attempts: Vec::new(),
                    flaky: false,
                    retry_class: None,
                    parser: None,
                    waived: false,
                    waiver: None,
                    runner: None,
                });
            }
        };
        let lease = match lease.and_then(|handle| handle.acquire()) {
            Ok(lease) => lease,
            Err(error) => {
                let cancelled = crate::process::cancelled();
                return Ok(TaskResult {
                    step_id: None,
                    invocation_id: None,
                    attempt: None,
                    started_at: None,
                    finished_at: None,
                    label: format!("{}: service {service} setup", step.label),
                    passed: false,
                    timed_out: false,
                    cancelled,
                    duration_ms: 0,
                    log: String::new(),
                    detail: Some(format!("{error:#}")),
                    failure_code: Some(FailureCode::ServiceLeaseFailure),
                    attempts: Vec::new(),
                    flaky: false,
                    retry_class: None,
                    parser: None,
                    waived: false,
                    waiver: None,
                    runner: None,
                });
            }
        };
        service_leases.push(lease);
    }
    let parser = step
        .parser
        .as_deref()
        .and_then(|id| project.config.parser(id));
    let retry = project.config.execution.retries.get(&step.id);
    let max_attempts = retry.map_or(1, |policy| policy.max_attempts.max(1));
    let mut attempts = Vec::with_capacity(max_attempts as usize);
    let mut final_result = None;
    for attempt in 1..=max_attempts {
        if attempt > 1 {
            let backoff = retry.map_or(0, |policy| policy.backoff_ms);
            if backoff > 0 {
                std::thread::sleep(Duration::from_millis(backoff));
            }
        }
        let mut attempt_step = step.clone();
        if max_attempts > 1 && attempt > 1 {
            attempt_step.log = attempt_log_name(&step.log, attempt);
        }
        let task = configured_task(project, &attempt_step, &service_leases)?;
        let mut result = execute(task, parser)?;
        result.attempt = Some(attempt);
        if !result.passed {
            result.retry_class = Some(retry_class(&result));
        }
        attempts.push(TaskAttempt {
            attempt,
            status: task_status(&result).into(),
            started_at: result.started_at.clone(),
            finished_at: result.finished_at.clone(),
            duration_ms: result.duration_ms,
            timed_out: result.timed_out,
            cancelled: result.cancelled,
            log: result.log.clone(),
            detail: result.detail.clone(),
        });
        let should_retry = !result.passed
            && !result.cancelled
            && attempt < max_attempts
            && retry.is_some_and(|policy| policy.retryable.contains(&retry_class(&result)));
        result.attempts = attempts.clone();
        if !should_retry {
            result.flaky = result.passed && attempts.iter().any(|attempt| attempt.status == "FAIL");
            final_result = Some(result);
            break;
        }
    }
    Ok(final_result.expect("at least one configured step attempt"))
}

fn attempt_log_name(log: &str, attempt: u32) -> String {
    let stem = log.strip_suffix(".log").unwrap_or(log);
    format!("{stem}.attempt-{attempt}.log")
}

fn isolation_mode_name(mode: TestIsolation) -> &'static str {
    match mode {
        TestIsolation::Shared => "shared",
        TestIsolation::SchemaPerWorker => "schema-per-worker",
        TestIsolation::DatabasePerWorker => "database-per-worker",
    }
}

fn task_status(result: &TaskResult) -> &'static str {
    if result.cancelled {
        "CANCELLED"
    } else if result.passed {
        "PASS"
    } else {
        "FAIL"
    }
}

fn retry_class(result: &TaskResult) -> RetryClass {
    if result.cancelled {
        RetryClass::Cancelled
    } else if result.timed_out {
        RetryClass::Timeout
    } else if matches!(
        result.failure_code,
        Some(
            FailureCode::ResultParseFailure | FailureCode::ResultZero | FailureCode::ResultPartial
        )
    ) {
        RetryClass::Parser
    } else {
        RetryClass::Exit
    }
}

fn lock_services<'a>(
    services: &'a Mutex<ServiceManager<'a>>,
) -> Result<MutexGuard<'a, ServiceManager<'a>>> {
    let started = Instant::now();
    loop {
        if crate::process::cancelled() {
            return Err(anyhow::anyhow!(
                "verification cancelled while waiting for service lock"
            ));
        }
        match services.try_lock() {
            Ok(guard) => {
                if crate::process::cancelled() {
                    drop(guard);
                    return Err(anyhow::anyhow!(
                        "verification cancelled while waiting for service lock"
                    ));
                }
                return Ok(guard);
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(anyhow::anyhow!("service manager lock was poisoned"));
            }
            Err(TryLockError::WouldBlock) => {
                if crate::process::cancelled() {
                    return Err(anyhow::anyhow!(
                        "verification cancelled while waiting for service lock"
                    ));
                }
                if started.elapsed() >= SERVICE_LOCK_WAIT {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for service manager lock"
                    ));
                }
                std::thread::sleep(SERVICE_LOCK_POLL);
            }
        }
    }
}

fn configured_task(
    project: &Project,
    step: &StepConfig,
    service_leases: &[ServiceLease],
) -> Result<Task> {
    let cwd = std::path::PathBuf::from(project.expand_for_input(&step.cwd, step.input));
    let args = step
        .args
        .iter()
        .map(|argument| project.expand_for_input(argument, step.input))
        .collect::<Vec<_>>();
    let mut task = Task::new(&step.label, &step.program, &cwd, log(project, &step.log)?)
        .timeout(step.timeout_secs);
    task = match &step.runner {
        Some(runner) => configure_runner(project, step, runner, task, &args)?,
        None => task.args(args),
    };
    Ok(inject_task_environment(task, step, service_leases))
}

fn configure_runner(
    project: &Project,
    step: &StepConfig,
    runner: &crate::config::RunnerConfig,
    mut task: Task,
    args: &[String],
) -> Result<Task> {
    let (effective_args, mut execution) = runner_inputs(runner, args)?;
    execution.program = step.program.clone();
    let isolation_state = configure_isolation(project, step, runner, &mut execution)?;
    configure_shard(project, step, &mut execution);
    let runner_environment = execution.environment.clone();
    task = task.args(effective_args).runner(execution);
    if let Some(state_file) = isolation_state {
        task = task.isolation_state(state_file);
    }
    for (name, value) in runner_environment {
        task = task.env(name, value);
    }

    Ok(task)
}

fn configure_isolation(
    project: &Project,
    step: &StepConfig,
    runner: &crate::config::RunnerConfig,
    execution: &mut RunnerExecution,
) -> Result<Option<std::path::PathBuf>> {
    let mut isolation_state = None;
    execution.migration_decision = if runner.isolation == TestIsolation::Shared {
        "legacy-serial".into()
    } else {
        "explicit-runner-isolation".into()
    };
    execution.lock_decision = if runner.isolation == TestIsolation::Shared {
        "not-required".into()
    } else {
        "invocation-scoped".into()
    };
    if runner.isolation != TestIsolation::Shared {
        let worker_count = runner.threads.unwrap_or(1);
        let (allocation, state_file) = allocate_isolation(
            &project.reports,
            &project.invocation_id(),
            &step.id,
            runner.isolation,
            worker_count,
        )?;
        execution.isolation_id = Some(format!(
            "{}:{}",
            allocation.invocation_id, allocation.step_id
        ));
        execution.worker_ids = allocation.workers.clone();
        execution.isolation_root = state_file
            .parent()
            .map(|path| path.to_string_lossy().into_owned());
        execution.environment.insert(
            crate::process::ISOLATION_MODE_ENV.into(),
            isolation_mode_name(runner.isolation).into(),
        );
        execution.environment.insert(
            crate::process::ISOLATION_IDS_ENV.into(),
            allocation.workers.join(","),
        );
        execution.environment.insert(
            crate::process::ISOLATION_ROOT_ENV.into(),
            state_file
                .parent()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        isolation_state = Some(state_file);
    }
    Ok(isolation_state)
}

fn configure_shard(project: &Project, step: &StepConfig, execution: &mut RunnerExecution) {
    if let Some(shard) = project.config.execution.shards.get(&step.id) {
        execution.shard_index = Some(shard.index);
        execution.shard_total = Some(shard.total);
        execution
            .environment
            .insert("HARNESS_GATE_SHARD_INDEX".into(), shard.index.to_string());
        execution
            .environment
            .insert("HARNESS_GATE_SHARD_TOTAL".into(), shard.total.to_string());
        execution.merge_identity =
            Some(format!("{}:shard-{}/{}", step.id, shard.index, shard.total));
    }
}

fn inject_task_environment(
    mut task: Task,
    step: &StepConfig,
    service_leases: &[ServiceLease],
) -> Task {
    for lease in service_leases {
        let (name, value) = lease.environment();
        task = task.env(name, value);
    }
    for name in &step.remove_env {
        task = task.env_remove(name);
    }

    task
}

fn runner_inputs(
    runner: &crate::config::RunnerConfig,
    step_args: &[String],
) -> Result<(Vec<String>, RunnerExecution)> {
    let mut effective_args = step_args.to_vec();
    let insertion = runner.args_position.unwrap_or(effective_args.len());
    effective_args.splice(insertion..insertion, runner.args.iter().cloned());

    if runner.kind == "cargo-test" {
        if let Some(threads) = runner.threads {
            let insertion = effective_args
                .iter()
                .position(|argument| argument == "--")
                .map(|index| index + 1)
                .unwrap_or_else(|| {
                    effective_args.push("--".into());
                    effective_args.len()
                });
            effective_args.splice(
                insertion..insertion,
                ["--test-threads".into(), threads.to_string()],
            );
        }
    }

    let mut environment = std::collections::BTreeMap::new();
    if let (Some(name), Some(threads)) = (&runner.threads_env, runner.threads) {
        let value = threads.to_string();
        environment.insert(name.clone(), value);
    }
    let execution = RunnerExecution {
        version: runner.version,
        kind: runner.kind.clone(),
        program: String::new(),
        effective_args: effective_args.clone(),
        environment,
        result_format: runner.result_format,
        isolation: runner.isolation,
        threads: runner.threads,
        isolation_id: None,
        worker_ids: Vec::new(),
        isolation_root: None,
        migration_decision: "pending".into(),
        lock_decision: "pending".into(),
        shard_index: None,
        shard_total: None,
        merge_identity: None,
    };
    Ok((effective_args, execution))
}

fn execute(task: Task, parser: Option<&ParserConfig>) -> Result<TaskResult> {
    let mut result = task.run()?;
    if result.passed {
        if let Some(parser) = parser {
            let content = match fs::read_to_string(&result.log) {
                Ok(content) => content,
                Err(error) => {
                    result.passed = false;
                    result.failure_code = Some(FailureCode::ResultParseFailure);
                    result.detail = Some(format!("parser failure: read result log: {error}"));
                    result.parser = Some(ParserEvidence {
                        mode: parser_mode(parser).into(),
                        version: 1,
                        observed: 0,
                        minimum: parser_minimum(parser),
                        complete: false,
                    });
                    return Ok(result);
                }
            };
            let (count, minimum) = match parse_result_count(&content, parser) {
                Ok(value) => value,
                Err(error) => {
                    result.passed = false;
                    result.failure_code = Some(FailureCode::ResultParseFailure);
                    result.detail = Some(format!("parser failure: {error:#}"));
                    result.parser = Some(ParserEvidence {
                        mode: parser_mode(parser).into(),
                        version: 1,
                        observed: 0,
                        minimum: parser_minimum(parser),
                        complete: false,
                    });
                    return Ok(result);
                }
            };
            let complete = count >= minimum;
            result.parser = Some(ParserEvidence {
                mode: parser_mode(parser).into(),
                version: 1,
                observed: count,
                minimum,
                complete,
            });
            if count < minimum {
                result.passed = false;
                result.failure_code = Some(if count == 0 {
                    FailureCode::ResultZero
                } else {
                    FailureCode::ResultPartial
                });
                result.detail = Some(format!(
                    "{}: parsed {count} result(s), expected at least {minimum}",
                    if count == 0 {
                        "zero test results"
                    } else {
                        "partial test results"
                    }
                ));
            } else {
                result.detail = Some(format!("{count} result(s)"));
            }
        }
    }
    Ok(result)
}

fn parser_mode(parser: &ParserConfig) -> &'static str {
    match parser {
        ParserConfig::Regex { .. } => "regex",
        ParserConfig::Junit { .. } => "junit",
        ParserConfig::Trx { .. } => "trx",
        ParserConfig::Json { .. } => "json",
    }
}

fn parser_minimum(parser: &ParserConfig) -> usize {
    match parser {
        ParserConfig::Regex { minimum, .. }
        | ParserConfig::Junit { minimum }
        | ParserConfig::Trx { minimum }
        | ParserConfig::Json { minimum, .. } => *minimum,
    }
}

pub(super) fn print_result(result: &TaskResult) {
    let marker = if result.passed {
        ui::pass("PASS")
    } else {
        ui::failure("FAIL")
    };
    println!("[{marker}] {} ({} ms)", result.label, result.duration_ms);
    if !result.passed && !result.log.is_empty() {
        println!("       log: {}", result.log);
    }
}

pub(super) fn print_external_result(result: &TaskResult) {
    print!("[RUN ] {} ... ", result.label);
    use std::io::Write;
    std::io::stdout().flush().ok();
    print_result_inline(result);
}

fn print_result_inline(result: &TaskResult) {
    let marker = if result.passed {
        ui::pass("PASS")
    } else {
        ui::failure("FAIL")
    };
    println!("{marker} ({} ms)", result.duration_ms);
    if !result.passed && !result.log.is_empty() {
        println!("       log: {}", result.log);
    }
}

fn log(project: &Project, name: &str) -> Result<std::path::PathBuf> {
    if !safe_log_name(name) {
        bail!("step log must be a single .log filename: {name:?}");
    }

    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let reports = project
        .reports
        .canonicalize()
        .with_context(|| format!("resolve report directory {}", project.reports.display()))?;
    if !reports.starts_with(&repository) {
        bail!("report directory escapes project root");
    }

    let logs = project.reports.join("logs");
    if std::fs::symlink_metadata(&logs).is_ok() {
        let resolved = logs
            .canonicalize()
            .with_context(|| format!("resolve log directory {}", logs.display()))?;
        if !resolved.starts_with(&reports) {
            bail!("log directory escapes report directory");
        }
    } else {
        std::fs::create_dir_all(&logs)
            .with_context(|| format!("create log directory {}", logs.display()))?;
        let resolved = logs
            .canonicalize()
            .with_context(|| format!("resolve log directory {}", logs.display()))?;
        if !resolved.starts_with(&reports) {
            bail!("log directory escapes report directory");
        }
    }

    let target = logs.join(name);
    if std::fs::symlink_metadata(&target).is_ok() {
        let resolved = target
            .canonicalize()
            .with_context(|| format!("resolve log file {}", target.display()))?;
        if !resolved.starts_with(&reports) {
            bail!("log file escapes report directory");
        }
    }
    Ok(target)
}

fn safe_log_name(value: &str) -> bool {
    let path = std::path::Path::new(value);
    !value.is_empty()
        && !value.contains('\0')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains(':')
        && !value.starts_with("//")
        && !value.starts_with("\\\\")
        && !value.starts_with('\\')
        && !value
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
        && !path.is_absolute()
        && path.components().count() == 1
        && value.len() > ".log".len()
        && value.ends_with(".log")
}

#[cfg(test)]
mod tests {
    use super::runner_inputs;
    use crate::config::{RunnerConfig, RunnerResultFormat, TestIsolation};

    #[test]
    fn runner_arguments_and_environment_are_recorded() {
        let runner = RunnerConfig {
            version: 1,
            kind: "generic".into(),
            threads: Some(3),
            threads_env: Some("TEST_THREADS".into()),
            args: vec!["--runner-flag".into()],
            args_position: Some(1),
            result_format: RunnerResultFormat::Json,
            isolation: TestIsolation::DatabasePerWorker,
        };
        let (args, execution) =
            runner_inputs(&runner, &["check".into(), "target".into()]).expect("runner inputs");

        assert_eq!(args, ["check", "--runner-flag", "target"]);
        assert_eq!(execution.effective_args, args);
        assert_eq!(execution.environment["TEST_THREADS"], "3");
        assert_eq!(execution.threads, Some(3));
    }

    #[test]
    fn cargo_test_threads_are_inserted_after_the_test_separator() {
        let runner = RunnerConfig {
            version: 1,
            kind: "cargo-test".into(),
            threads: Some(4),
            threads_env: None,
            args: vec!["--nocapture".into()],
            args_position: None,
            result_format: RunnerResultFormat::Junit,
            isolation: TestIsolation::SchemaPerWorker,
        };
        let (args, _) =
            runner_inputs(&runner, &["test".into(), "--".into()]).expect("runner inputs");

        assert_eq!(args, ["test", "--", "--test-threads", "4", "--nocapture"]);
    }
}
