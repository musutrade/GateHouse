use super::super::model::{
    RunnerConfig, ServiceConfig, StepConfig, StepKind, TestIsolation, RUNNER_CONFIG_VERSION,
};
use super::primitives::{validate_env_name, validate_id, validate_program};
use super::FlowConfig;
use super::{ConfigIssueKind, ValidationIssue};
use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::Path;

pub(super) fn validate_step(config: &FlowConfig, step: &StepConfig) -> Result<()> {
    validate_id("verification step id", &step.id)?;
    if step.label.trim().is_empty() || step.profiles.is_empty() {
        bail!(
            "step {:?} requires a label and at least one profile",
            step.id
        );
    }
    for profile in &step.profiles {
        validate_id("step profile", profile)?;
    }

    match step.kind.unwrap_or(StepKind::ExternalStep) {
        StepKind::BuiltinGate => return validate_builtin_gate(step),
        StepKind::ExternalStep => {
            if step.gate_type.is_some() {
                bail!("external step {:?} may not declare gate_type", step.id);
            }
            if matches!(
                step.id.as_str(),
                "builtin.secret-scan" | "builtin.architecture-audit"
            ) {
                bail!(
                    "external step {:?} uses a reserved built-in gate id",
                    step.id
                );
            }
        }
    }

    validate_id("step component", &step.component)?;
    validate_program(&format!("step {} program", step.id), &step.program)?;
    if is_shell(&step.program)
        && step
            .args
            .iter()
            .any(|argument| is_shell_command_argument(argument))
    {
        bail!("step {:?} may not execute a shell command string", step.id);
    }
    validate_arguments(config, &step.id, &step.args)?;
    let runner = step.runner.as_ref();
    if let Some(runner) = runner {
        validate_runner(config, step, runner)?;
    }
    let Some(cwd_name) = exact_placeholder(&step.cwd) else {
        bail!("step {:?} cwd must be one path placeholder", step.id);
    };
    if cwd_name != "root" && !config.paths.aliases.contains_key(cwd_name) {
        bail!(
            "step {:?} cwd references unknown path {cwd_name:?}",
            step.id
        );
    }
    validate_log_name(&step.log)
        .map_err(|error| anyhow::anyhow!("step {:?} log {error}", step.id))?;
    if step.timeout_secs == 0 || step.timeout_secs > 3600 {
        bail!("step {:?} timeout_secs must be between 1 and 3600", step.id);
    }
    if let Some(name) = &step.timeout_env {
        validate_env_name("step timeout_env", name)?;
    }
    if let Some(parser) = &step.parser {
        let parser_config = config.parsers.get(parser);
        if parser_config.is_none() {
            bail!("step {:?} references unknown parser {parser:?}", step.id);
        }
    }
    let mut step_services = HashSet::new();
    let mut service_envs = HashSet::new();
    for service in &step.services {
        if !step_services.insert(service) {
            bail!("step {:?} contains duplicate service {service:?}", step.id);
        }
        let service_config = config.services.get(service).ok_or_else(|| {
            anyhow::anyhow!("step {:?} references unknown service {service:?}", step.id)
        })?;
        let inject_env = match service_config {
            ServiceConfig::Docker { inject_env, .. }
            | ServiceConfig::Environment { inject_env, .. } => inject_env,
        };
        if !service_envs.insert(inject_env) {
            bail!(
                "step {:?} has multiple services injecting {inject_env}",
                step.id
            );
        }
        if step.remove_env.contains(inject_env) {
            bail!(
                "step {:?} may not remove service injection variable {inject_env}",
                step.id
            );
        }
    }
    if let Some(runner) = runner {
        if let Some(name) = &runner.threads_env {
            if service_envs.contains(name) {
                bail!(
                    "step {:?} runner threads_env collides with a service injection variable {name}",
                    step.id
                );
            }
        }
    }
    for name in &step.remove_env {
        validate_env_name("step remove_env", name)?;
    }
    Ok(())
}

/// Validate a step for the machine-readable diagnostic path. This mirrors the
/// execution validator's order, but carries the field path and closed issue
/// kind at the point where the invariant is checked. Human `anyhow` errors
/// remain available through `validate_step` for existing CLI callers.
pub(super) fn validate_step_diagnostic(
    config: &FlowConfig,
    step: &StepConfig,
    index: usize,
) -> std::result::Result<(), ValidationIssue> {
    let base = format!("steps[{index}]");
    let check = |result: Result<()>, kind: ConfigIssueKind, path: String| {
        result.map_err(|error| ValidationIssue::new(kind, path, error.to_string()))
    };

    check(
        validate_id("verification step id", &step.id),
        ConfigIssueKind::InvalidField,
        format!("{base}.id"),
    )?;
    if step.label.trim().is_empty() || step.profiles.is_empty() {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            base.clone(),
            "step requires a label and at least one profile",
        ));
    }
    for profile in &step.profiles {
        check(
            validate_id("step profile", profile),
            ConfigIssueKind::InvalidField,
            format!("{base}.profiles"),
        )?;
    }

    match step.kind.unwrap_or(StepKind::ExternalStep) {
        StepKind::BuiltinGate => {
            if step.gate_type.is_none() {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::InvalidField,
                    format!("{base}.gate_type"),
                    "built-in gate requires gate_type",
                ));
            }
            let expected = format!("builtin.{}", step.gate_type.expect("checked").as_str());
            if step.id != expected {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::InvalidField,
                    format!("{base}.id"),
                    "built-in gate id is reserved",
                ));
            }
            if !step.depends_on.is_empty() {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::Dependency,
                    format!("{base}.depends_on"),
                    "built-in gate may not declare external dependencies",
                ));
            }
            if !step.component.is_empty()
                || !step.program.is_empty()
                || !step.args.is_empty()
                || !step.cwd.is_empty()
                || !step.log.is_empty()
                || step.timeout_secs != 0
                || step.timeout_env.is_some()
                || step.parser.is_some()
                || !step.services.is_empty()
                || !step.remove_env.is_empty()
                || step.runner.is_some()
            {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::InvalidField,
                    base,
                    "built-in gate contains external-step fields",
                ));
            }
            return Ok(());
        }
        StepKind::ExternalStep => {
            if step.gate_type.is_some() {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::InvalidField,
                    format!("{base}.gate_type"),
                    "external step may not declare gate_type",
                ));
            }
            if matches!(
                step.id.as_str(),
                "builtin.secret-scan" | "builtin.architecture-audit"
            ) {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::InvalidField,
                    format!("{base}.id"),
                    "external step uses a reserved built-in gate id",
                ));
            }
        }
    }

    check(
        validate_id("step component", &step.component),
        ConfigIssueKind::InvalidField,
        format!("{base}.component"),
    )?;
    check(
        validate_program(&format!("step {} program", step.id), &step.program),
        ConfigIssueKind::InvalidField,
        format!("{base}.program"),
    )?;
    if is_shell(&step.program)
        && step
            .args
            .iter()
            .any(|argument| is_shell_command_argument(argument))
    {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.args"),
            "step may not execute a shell command string",
        ));
    }
    check(
        validate_arguments(config, &step.id, &step.args),
        ConfigIssueKind::InvalidField,
        format!("{base}.args"),
    )?;
    if let Some(runner) = &step.runner {
        validate_runner_diagnostic(config, step, runner, &base)?;
    }
    let Some(cwd_name) = exact_placeholder(&step.cwd) else {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.cwd"),
            "cwd must be one path placeholder",
        ));
    };
    if cwd_name != "root" && !config.paths.aliases.contains_key(cwd_name) {
        return Err(ValidationIssue::new(
            ConfigIssueKind::UnknownReference,
            format!("{base}.cwd"),
            "cwd references an unknown path",
        ));
    }
    check(
        validate_log_name(&step.log),
        ConfigIssueKind::InvalidLog,
        format!("{base}.log"),
    )?;
    if step.timeout_secs == 0 || step.timeout_secs > 3600 {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.timeout_secs"),
            "timeout_secs is outside the accepted range",
        ));
    }
    if let Some(name) = &step.timeout_env {
        check(
            validate_env_name("step timeout_env", name),
            ConfigIssueKind::InvalidEnvironment,
            format!("{base}.timeout_env"),
        )?;
    }
    if let Some(parser) = &step.parser {
        if !config.parsers.contains_key(parser) {
            return Err(ValidationIssue::new(
                ConfigIssueKind::UnknownReference,
                format!("{base}.parser"),
                "step references an unknown parser",
            ));
        }
    }
    let mut step_services = HashSet::new();
    let mut service_envs = HashSet::new();
    let mut service_inject_collision: Option<usize> = None;
    for (service_index, service) in step.services.iter().enumerate() {
        if !step_services.insert(service) {
            return Err(ValidationIssue::new(
                ConfigIssueKind::DuplicateField,
                format!("{base}.services[{service_index}]"),
                "step contains a duplicate service",
            ));
        }
        let Some(service_config) = config.services.get(service) else {
            return Err(ValidationIssue::new(
                ConfigIssueKind::UnknownReference,
                format!("{base}.services[{service_index}]"),
                "step references an unknown service",
            ));
        };
        let inject_env = match service_config {
            ServiceConfig::Docker { inject_env, .. }
            | ServiceConfig::Environment { inject_env, .. } => inject_env,
        };
        if !service_envs.insert(inject_env) {
            service_inject_collision = Some(service_index);
        }
        if step.remove_env.contains(inject_env) {
            return Err(ValidationIssue::new(
                ConfigIssueKind::InvalidField,
                format!("{base}.remove_env"),
                "step may not remove a service injection variable",
            ));
        }
    }
    if let Some(runner) = &step.runner {
        if let Some(name) = &runner.threads_env {
            if service_envs.contains(name) {
                return Err(ValidationIssue::new(
                    ConfigIssueKind::ServiceInjectCollision,
                    format!("{base}.runner.threads_env"),
                    "runner threads_env collides with a service injection variable",
                ));
            }
        }
    }
    if let Some(service_index) = service_inject_collision {
        return Err(ValidationIssue::new(
            ConfigIssueKind::ServiceInjectCollision,
            format!("{base}.services[{service_index}]"),
            "multiple services inject the same environment variable",
        ));
    }
    for name in &step.remove_env {
        check(
            validate_env_name("step remove_env", name),
            ConfigIssueKind::InvalidEnvironment,
            format!("{base}.remove_env"),
        )?;
    }
    Ok(())
}

fn validate_runner_diagnostic(
    config: &FlowConfig,
    step: &StepConfig,
    runner: &RunnerConfig,
    base: &str,
) -> std::result::Result<(), ValidationIssue> {
    let check = |result: Result<()>, kind: ConfigIssueKind, path: String| {
        result.map_err(|error| ValidationIssue::new(kind, path, error.to_string()))
    };
    if runner.version != RUNNER_CONFIG_VERSION {
        return Err(ValidationIssue::new(
            ConfigIssueKind::RunnerVersion,
            format!("{base}.runner.version"),
            "runner version is unsupported",
        ));
    }
    check(
        validate_id("runner kind", &runner.kind),
        ConfigIssueKind::InvalidField,
        format!("{base}.runner.kind"),
    )?;
    if runner.kind == "cargo-test" && step.program != "cargo" {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.runner.kind"),
            "cargo-test runner requires program cargo",
        ));
    }
    if let Some(threads) = runner.threads {
        if threads == 0 || threads > 256 {
            return Err(ValidationIssue::new(
                ConfigIssueKind::InvalidField,
                format!("{base}.runner.threads"),
                "runner threads are outside the accepted range",
            ));
        }
    }
    if let Some(name) = &runner.threads_env {
        check(
            validate_env_name("runner threads_env", name),
            ConfigIssueKind::InvalidEnvironment,
            format!("{base}.runner.threads_env"),
        )?;
        if runner.threads.is_none() {
            return Err(ValidationIssue::new(
                ConfigIssueKind::InvalidField,
                format!("{base}.runner.threads"),
                "runner threads_env requires threads",
            ));
        }
        if step.remove_env.iter().any(|removed| removed == name) {
            return Err(ValidationIssue::new(
                ConfigIssueKind::InvalidField,
                format!("{base}.remove_env"),
                "runner threads_env may not be removed",
            ));
        }
    }
    if runner.threads.is_some_and(|threads| threads > 1)
        && runner.kind != "cargo-test"
        && runner.threads_env.is_none()
    {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.runner.threads_env"),
            "runner threads requires threads_env",
        ));
    }
    if runner.threads.is_some_and(|threads| threads > 1)
        && matches!(runner.isolation, TestIsolation::Shared)
    {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.runner.isolation"),
            "runner shared isolation is not allowed with multiple workers",
        ));
    }
    if runner
        .args_position
        .is_some_and(|position| position > step.args.len())
    {
        return Err(ValidationIssue::new(
            ConfigIssueKind::InvalidField,
            format!("{base}.runner.args_position"),
            "runner args_position exceeds the step argument count",
        ));
    }
    check(
        validate_arguments(
            config,
            &format!("step {} runner args", step.id),
            &runner.args,
        ),
        ConfigIssueKind::InvalidField,
        format!("{base}.runner.args"),
    )
}

/// Validate a log as a filename rather than relying only on the host platform's
/// `Path` parser. Configuration can be checked on Unix and later executed on
/// Windows, so both separator styles and Windows prefixes are rejected here.
pub(super) fn validate_log_name(value: &str) -> Result<()> {
    let path = Path::new(value);
    let windows_prefix = value.starts_with("\\\\")
        || value.starts_with("//")
        || value.starts_with('\\')
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':');
    if value.is_empty()
        || value.contains('\0')
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || windows_prefix
        || path.is_absolute()
        || value == "."
        || value == ".."
        || value.len() <= ".log".len()
        || !value.ends_with(".log")
    {
        bail!("must be a single .log file name");
    }
    if path.components().count() != 1 {
        bail!("must be a single .log file name");
    }
    Ok(())
}

/// Return the lexical identity used by duplicate-log preflight. Lowercasing
/// makes the check conservative on case-insensitive filesystems while keeping
/// the configured spelling unchanged in reports.
pub(super) fn normalize_log_identity(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn validate_builtin_gate(step: &StepConfig) -> Result<()> {
    let gate_type = step
        .gate_type
        .ok_or_else(|| anyhow::anyhow!("built-in gate {:?} requires gate_type", step.id))?;
    let expected_id = format!("builtin.{}", gate_type.as_str());
    if step.id != expected_id {
        bail!(
            "built-in gate {:?} must use reserved id {expected_id:?}",
            step.id
        );
    }
    if !step.depends_on.is_empty() {
        bail!(
            "built-in gate {:?} may not declare external dependencies",
            step.id
        );
    }
    if !step.component.is_empty()
        || !step.program.is_empty()
        || !step.args.is_empty()
        || !step.cwd.is_empty()
        || !step.log.is_empty()
        || step.timeout_secs != 0
        || step.timeout_env.is_some()
        || step.parser.is_some()
        || !step.services.is_empty()
        || !step.remove_env.is_empty()
        || step.runner.is_some()
    {
        bail!(
            "built-in gate {:?} may not declare external-step fields",
            step.id
        );
    }
    Ok(())
}

fn validate_runner(config: &FlowConfig, step: &StepConfig, runner: &RunnerConfig) -> Result<()> {
    if runner.version != RUNNER_CONFIG_VERSION {
        bail!(
            "step {:?} runner version {} is unsupported; expected {}",
            step.id,
            runner.version,
            RUNNER_CONFIG_VERSION
        );
    }
    validate_id("runner kind", &runner.kind)?;
    if runner.kind == "cargo-test" && step.program != "cargo" {
        bail!(
            "step {:?} cargo-test runner requires program \"cargo\"",
            step.id
        );
    }
    if let Some(threads) = runner.threads {
        if threads == 0 || threads > 256 {
            bail!(
                "step {:?} runner threads must be between 1 and 256",
                step.id
            );
        }
    }
    if let Some(name) = &runner.threads_env {
        validate_env_name("runner threads_env", name)?;
        if runner.threads.is_none() {
            bail!("step {:?} runner threads_env requires threads", step.id);
        }
        if step.remove_env.iter().any(|removed| removed == name) {
            bail!(
                "step {:?} may not remove runner threads_env variable {name}",
                step.id
            );
        }
    }
    if runner.threads.is_some_and(|threads| threads > 1)
        && runner.kind != "cargo-test"
        && runner.threads_env.is_none()
    {
        bail!(
            "step {:?} runner threads requires threads_env unless kind is cargo-test",
            step.id
        );
    }
    if runner.threads.is_some_and(|threads| threads > 1)
        && matches!(runner.isolation, TestIsolation::Shared)
    {
        bail!(
            "step {:?} runner shared isolation is not allowed with more than one worker",
            step.id
        );
    }
    if runner
        .args_position
        .is_some_and(|position| position > step.args.len())
    {
        bail!(
            "step {:?} runner args_position must not exceed step argument count",
            step.id
        );
    }
    validate_arguments(
        config,
        &format!("step {} runner args", step.id),
        &runner.args,
    )
}

pub(super) fn validate_arguments(config: &FlowConfig, owner: &str, args: &[String]) -> Result<()> {
    for arg in args {
        validate_template(config, owner, arg)?;
    }
    Ok(())
}

pub(super) fn validate_template(config: &FlowConfig, owner: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        bail!("{owner:?} contains a NUL value");
    }
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        let tail = &rest[start..];
        let Some(end) = tail.find('}') else {
            bail!("{owner:?} contains an unterminated placeholder in {value:?}");
        };
        let name = &tail[1..end];
        if !config.allowed_placeholder(name) {
            bail!("{owner:?} contains unsupported placeholder {{{name}}}");
        }
        rest = &tail[end + 1..];
    }
    Ok(())
}

fn exact_placeholder(value: &str) -> Option<&str> {
    value.strip_prefix('{')?.strip_suffix('}')
}

fn is_shell(program: &str) -> bool {
    matches!(program, "sh" | "bash" | "dash" | "zsh")
}

fn is_shell_command_argument(arg: &str) -> bool {
    arg.starts_with("--command")
        || (arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains('c'))
}
