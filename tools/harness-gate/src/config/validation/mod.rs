use super::diagnostic::{
    ConfigDiagnostic, ConfigDiagnostics, DiagnosticSeverity, RelatedDiagnostic, SourceMap,
};
use super::model::{
    DoctorCheck, DoctorCheckKind, ExternalValuePolicy, FlowConfig, ParserConfig, PathScope,
    ScopeRule, ServiceConfig, CONFIG_VERSION,
};
use anyhow::{bail, Context, Result};
use globset::Glob;
use regex::Regex;
use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigIssueKind {
    Version,
    RunnerVersion,
    DependencyCycle,
    Dependency,
    ServiceInjectCollision,
    UnknownReference,
    InvalidPath,
    InvalidEnvironment,
    DuplicateField,
    InvalidLog,
    InvalidField,
}

impl ConfigIssueKind {
    const fn id(self) -> &'static str {
        match self {
            Self::Version => "HGCFG-VERSION",
            Self::RunnerVersion => "HGCFG-RUNNER-VERSION",
            Self::DependencyCycle => "HGCFG-DEPENDENCY-CYCLE",
            Self::Dependency => "HGCFG-DEPENDENCY",
            Self::ServiceInjectCollision => "HGCFG-SERVICE-INJECT-COLLISION",
            Self::UnknownReference => "HGCFG-UNKNOWN-REFERENCE",
            Self::InvalidPath => "HGCFG-INVALID-PATH",
            Self::InvalidEnvironment => "HGCFG-INVALID-ENVIRONMENT",
            Self::DuplicateField => "HGCFG-DUPLICATE-FIELD",
            Self::InvalidLog => "HGCFG-INVALID-LOG",
            Self::InvalidField => "HGCFG-INVALID-FIELD",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::Version => "workflow configuration version is unsupported",
            Self::RunnerVersion => "runner contract version is unsupported",
            Self::DependencyCycle => "dependencies contain a cycle",
            Self::Dependency => "dependency declaration is invalid",
            Self::ServiceInjectCollision => {
                "multiple services claim the same injected environment variable"
            }
            Self::UnknownReference => "reference does not name a declared configuration entry",
            Self::InvalidPath => "path is not a safe repository-relative path",
            Self::InvalidEnvironment => "environment variable name is invalid",
            Self::DuplicateField => "configuration declares the same identifier more than once",
            Self::InvalidLog => "log path is not a valid single .log filename",
            Self::InvalidField => "field violates a documented configuration constraint",
        }
    }

    const fn help(self) -> &'static str {
        match self {
            Self::Version => {
                "migrate a v1 file with `harness-gate config migrate` or use version 2"
            }
            Self::RunnerVersion => "use the supported runner contract version",
            Self::DependencyCycle | Self::Dependency => {
                "remove the invalid dependency or add an explicit acyclic prerequisite"
            }
            Self::ServiceInjectCollision => "use one service or distinct inject_env names",
            Self::UnknownReference => "use an identifier declared in the configuration",
            Self::InvalidPath => {
                "use a non-empty path below the repository without a prefix or parent traversal"
            }
            Self::InvalidEnvironment => {
                "use an uppercase environment-variable name containing only letters, digits, and underscores"
            }
            Self::DuplicateField => {
                "keep one declaration or give each declaration a unique identifier"
            }
            Self::InvalidLog => "choose a unique single .log filename",
            Self::InvalidField => {
                "update the field to satisfy the documented configuration constraints"
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct ValidationIssue {
    pub(super) kind: ConfigIssueKind,
    pub(super) path: String,
    detail: String,
}

impl ValidationIssue {
    pub(super) fn new(
        kind: ConfigIssueKind,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for ValidationIssue {}

impl FlowConfig {
    pub(super) fn validate_with_diagnostics(
        &self,
        source_map: &SourceMap,
        source_path: Option<&Path>,
        repository_root: Option<&Path>,
    ) -> std::result::Result<(), ConfigDiagnostics> {
        let mut diagnostics = ConfigDiagnostics::empty();
        if let Some(source) = source_path {
            diagnostics = diagnostics.with_source(source);
        }
        self.collect_semantic_diagnostics(source_map, &mut diagnostics);
        if !diagnostics.is_empty() {
            diagnostics.sort();
            return Err(diagnostics);
        }
        validate_resource_conflicts(self, source_map, &mut diagnostics);
        if let Some(root) = repository_root {
            validate_report_template_paths(self, root, source_map, &mut diagnostics);
        }
        diagnostics.sort();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn collect_semantic_diagnostics(
        &self,
        source_map: &SourceMap,
        diagnostics: &mut ConfigDiagnostics,
    ) {
        collect_result(
            source_map,
            diagnostics,
            ConfigIssueKind::Version,
            "version",
            validate_version(self.version),
        );
        for (path, value) in [
            ("project.name", &self.project.name),
            ("project.default_profile", &self.project.default_profile),
            ("project.hook_profile", &self.project.hook_profile),
        ] {
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::InvalidField,
                path,
                validate_id(path, value),
            );
        }
        for (path, value) in [
            ("paths.reports", &self.paths.reports),
            ("paths.audit_config", &self.paths.audit_config),
            ("paths.secrets_config", &self.paths.secrets_config),
        ] {
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::InvalidPath,
                path,
                validate_repo_path(path, value),
            );
        }
        for (alias, entry) in &self.paths.aliases {
            let path = format!("paths.aliases[\"{alias}\"]");
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::InvalidField,
                &path,
                validate_path_alias(alias, entry),
            );
        }
        for (id, service) in &self.services {
            let path = format!("services[\"{id}\"]");
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::InvalidField,
                &path,
                validate_service_entry(id, service),
            );
        }
        for (id, parser) in &self.parsers {
            let path = format!("parsers[\"{id}\"]");
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::InvalidField,
                &path,
                validate_parser_entry(id, parser),
            );
        }
        for (index, check) in self.doctor.checks.iter().enumerate() {
            let path = format!("doctor.checks[{index}]");
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::InvalidField,
                &path,
                validate_doctor_check(self, check),
            );
        }
        collect_duplicate_doctor_ids(self, source_map, diagnostics);
        if self.scope.rules.is_empty() {
            push_diagnostic(
                self,
                source_map,
                diagnostics,
                "scope.rules",
                ConfigIssueKind::InvalidField,
                "scope.rules must not be empty",
            );
        } else {
            let components = self.components();
            for (index, rule) in self.scope.rules.iter().enumerate() {
                let path = format!("scope.rules[{index}]");
                collect_result(
                    source_map,
                    diagnostics,
                    ConfigIssueKind::InvalidField,
                    &path,
                    validate_scope_rule_with_components(&components, index, rule),
                );
            }
        }
        self.collect_step_diagnostics(source_map, diagnostics);
        collect_report_template_diagnostics(self, source_map, diagnostics);
        collect_execution_diagnostics(self, source_map, diagnostics);
        collect_result(
            source_map,
            diagnostics,
            ConfigIssueKind::InvalidField,
            "notifications",
            self.validate_notifications(),
        );
        collect_result(
            source_map,
            diagnostics,
            ConfigIssueKind::InvalidField,
            "policy",
            self.validate_policy(),
        );
    }

    fn collect_step_diagnostics(
        &self,
        source_map: &SourceMap,
        diagnostics: &mut ConfigDiagnostics,
    ) {
        if self.steps.is_empty() {
            push_diagnostic(
                self,
                source_map,
                diagnostics,
                "steps",
                ConfigIssueKind::InvalidField,
                "steps must not be empty",
            );
            return;
        }

        let mut ids = HashSet::new();
        let mut profiles = BTreeSet::new();
        let mut dependencies_are_valid = true;
        for (index, step) in self.steps.iter().enumerate() {
            let path = format!("steps[{index}]");
            if !ids.insert(step.id.as_str()) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    &format!("{path}.id"),
                    ConfigIssueKind::DuplicateField,
                    "duplicate verification step id",
                );
            }
            collect_step_field_diagnostics(self, step, index, source_map, diagnostics);
            profiles.extend(step.profiles.iter().cloned());
        }
        for (index, step) in self.steps.iter().enumerate() {
            for dependency in &step.depends_on {
                let path = format!("steps[{index}].depends_on");
                if dependency == &step.id {
                    dependencies_are_valid = false;
                    push_diagnostic(
                        self,
                        source_map,
                        diagnostics,
                        &path,
                        ConfigIssueKind::Dependency,
                        "a step may not depend on itself",
                    );
                } else if !ids.contains(dependency.as_str()) {
                    dependencies_are_valid = false;
                    push_diagnostic(
                        self,
                        source_map,
                        diagnostics,
                        &path,
                        ConfigIssueKind::Dependency,
                        "a dependency references a missing step",
                    );
                }
            }
        }
        if dependencies_are_valid {
            collect_result(
                source_map,
                diagnostics,
                ConfigIssueKind::DependencyCycle,
                "steps",
                validate_step_dependencies(self),
            );
        }
        for (path, profile) in [
            ("project.default_profile", &self.project.default_profile),
            ("project.hook_profile", &self.project.hook_profile),
        ] {
            if !profiles.contains(profile) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    path,
                    ConfigIssueKind::InvalidField,
                    "configured profile is not used by any step",
                );
            }
        }
        let mut required = HashSet::new();
        for id in &self.policy.required_steps {
            if !required.insert(id.as_str()) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    "policy.required_steps",
                    ConfigIssueKind::DuplicateField,
                    "policy.required_steps contains a duplicate step id",
                );
            } else if !ids.contains(id.as_str()) {
                push_diagnostic(
                    self,
                    source_map,
                    diagnostics,
                    "policy.required_steps",
                    ConfigIssueKind::UnknownReference,
                    "policy requires a missing verification step",
                );
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CONFIG_VERSION {
            bail!(
                "unsupported workflow config version {}; expected {}; run `harness-gate config migrate` for v1 configurations",
                self.version,
                CONFIG_VERSION
            );
        }
        validate_id("project.name", &self.project.name)?;
        validate_id("project.default_profile", &self.project.default_profile)?;
        validate_id("project.hook_profile", &self.project.hook_profile)?;
        validate_repo_path("paths.reports", &self.paths.reports)?;
        validate_repo_path("paths.audit_config", &self.paths.audit_config)?;
        validate_repo_path("paths.secrets_config", &self.paths.secrets_config)?;

        for (alias, entry) in &self.paths.aliases {
            validate_id("path alias", alias)?;
            if matches!(
                alias.as_str(),
                "root" | "reports" | "audit_config" | "secrets_config" | "host_port"
            ) {
                bail!("path alias {alias:?} is reserved");
            }
            validate_repo_path(&format!("paths.aliases.{alias}"), &entry.path)?;
            if let Some(name) = &entry.env {
                validate_env_name(&format!("paths.aliases.{alias}.env"), name)?;
            }
        }

        self.validate_services()?;
        self.validate_parsers()?;
        self.validate_doctor()?;
        self.validate_scope()?;
        self.validate_steps()?;
        self.validate_report_templates()?;
        self.validate_execution()?;
        self.validate_notifications()?;
        self.validate_policy()?;
        Ok(())
    }

    fn validate_notifications(&self) -> Result<()> {
        for (index, webhook) in self.notifications.webhooks.iter().enumerate() {
            let parsed = url::Url::parse(&webhook.url)
                .with_context(|| format!("notifications.webhooks[{index}].url is invalid"))?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                bail!("notifications.webhooks[{index}].url must be an http(s) URL");
            }
            if !parsed.username().is_empty() || parsed.password().is_some() {
                bail!("notifications.webhooks[{index}].url may not contain credentials");
            }
            let host = parsed.host_str().expect("validated webhook host");
            if webhook.allowed_hosts.is_empty() {
                bail!("notifications.webhooks[{index}] requires an explicit allowed_hosts list");
            }
            if !webhook
                .allowed_hosts
                .iter()
                .all(|allowed| crate::net_policy::valid_allowlist_host(allowed))
            {
                bail!("notifications.webhooks[{index}].allowed_hosts contains an invalid host");
            }
            let normalized_host = crate::net_policy::normalize_host(host);
            if !webhook
                .allowed_hosts
                .iter()
                .map(|allowed| crate::net_policy::normalize_host(allowed))
                .any(|allowed| allowed == normalized_host)
            {
                bail!("notifications.webhooks[{index}].url host is not in allowed_hosts");
            }
            if !webhook.on_failure && !webhook.on_success {
                bail!("notifications.webhooks[{index}] must enable on_failure or on_success");
            }
        }
        Ok(())
    }

    fn validate_execution(&self) -> Result<()> {
        if let Some(max_parallel) = self.execution.max_parallel {
            if max_parallel == 0 || max_parallel > 64 {
                bail!("execution.max_parallel must be between 1 and 64 (got {max_parallel})");
            }
        }
        let step_ids = self
            .steps
            .iter()
            .map(|step| step.id.as_str())
            .collect::<HashSet<_>>();
        for (step_id, retry) in &self.execution.retries {
            validate_id("execution retry step", step_id)?;
            if !step_ids.contains(step_id.as_str()) {
                bail!("execution retry policy references missing step {step_id:?}");
            }
            if retry.max_attempts == 0 || retry.max_attempts > 5 {
                bail!("execution.retries[{step_id:?}].max_attempts must be between 1 and 5");
            }
            if retry.backoff_ms > 60_000 {
                bail!("execution.retries[{step_id:?}].backoff_ms must not exceed 60000");
            }
        }
        for (step_id, shard) in &self.execution.shards {
            validate_id("execution shard step", step_id)?;
            if !step_ids.contains(step_id.as_str()) {
                bail!("execution shard policy references missing step {step_id:?}");
            }
            if !self
                .steps
                .iter()
                .any(|step| step.id == *step_id && step.runner.is_some())
            {
                bail!("execution shard policy requires a runner on step {step_id:?}");
            }
            if shard.total == 0 || shard.index >= shard.total {
                bail!("execution.shards[{step_id:?}] requires 0 <= index < total and total > 0");
            }
        }
        Ok(())
    }

    fn validate_policy(&self) -> Result<()> {
        let step_ids = self
            .steps
            .iter()
            .map(|step| step.id.as_str())
            .collect::<HashSet<_>>();
        let mut waiver_ids = HashSet::new();
        for waiver in &self.policy.waivers {
            validate_id("policy waiver id", &waiver.id)?;
            if !waiver_ids.insert(waiver.id.as_str()) {
                bail!("duplicate policy waiver id {:?}", waiver.id);
            }
            if !step_ids.contains(waiver.step.as_str()) {
                bail!(
                    "policy waiver {:?} references missing step {:?}",
                    waiver.id,
                    waiver.step
                );
            }
            for (field, value) in [
                ("risk", &waiver.risk),
                ("reason", &waiver.reason),
                ("owner", &waiver.owner),
                ("approved_by", &waiver.approved_by),
                ("created_at", &waiver.created_at),
                ("compensating_control", &waiver.compensating_control),
            ] {
                if value.trim().is_empty() {
                    bail!("policy waiver {:?} requires {field}", waiver.id);
                }
            }
            if waiver.owner == waiver.approved_by {
                bail!("policy waiver {:?} may not be self-approved", waiver.id);
            }
            let created_at = chrono::DateTime::parse_from_rfc3339(&waiver.created_at)
                .with_context(|| {
                    format!("policy waiver {:?} created_at must be RFC3339", waiver.id)
                })?;
            let expires_at = chrono::DateTime::parse_from_rfc3339(&waiver.expires_at)
                .with_context(|| {
                    format!("policy waiver {:?} expires_at must be RFC3339", waiver.id)
                })?;
            if expires_at <= created_at {
                bail!(
                    "policy waiver {:?} expires_at must be after created_at",
                    waiver.id
                );
            }
            if waiver
                .scope
                .as_deref()
                .is_some_and(|scope| scope.trim().is_empty())
            {
                bail!("policy waiver {:?} scope must not be empty", waiver.id);
            }
        }
        Ok(())
    }

    fn validate_report_templates(&self) -> Result<()> {
        match (&self.report_templates.root, &self.report_templates.template) {
            (None, None) if self.report_templates.junit.is_none() => Ok(()),
            (Some(root), Some(template)) => {
                validate_repo_path("report_templates.root", root)?;
                validate_repo_path("report_templates.template", template)?;
                if !template.ends_with(".html") && !template.ends_with(".tera") {
                    bail!("report_templates.template must use a .html or .tera extension");
                }
                if let Some(junit) = &self.report_templates.junit {
                    validate_repo_path("report_templates.junit", junit)?;
                    if !junit.ends_with(".xml") {
                        bail!("report_templates.junit must use a .xml extension");
                    }
                }
                Ok(())
            }
            (None, None) => {
                if let Some(junit) = &self.report_templates.junit {
                    validate_repo_path("report_templates.junit", junit)?;
                    if !junit.ends_with(".xml") {
                        bail!("report_templates.junit must use a .xml extension");
                    }
                }
                Ok(())
            }
            _ => bail!(
                "report_templates.root and report_templates.template must be configured together"
            ),
        }
    }
    fn validate_services(&self) -> Result<()> {
        for (id, service) in &self.services {
            validate_service_entry(id, service)?;
        }
        Ok(())
    }

    fn validate_parsers(&self) -> Result<()> {
        for (id, parser) in &self.parsers {
            validate_parser_entry(id, parser)?;
        }
        Ok(())
    }

    fn validate_doctor(&self) -> Result<()> {
        let mut ids = HashSet::new();
        for check in &self.doctor.checks {
            if !ids.insert(check.id.as_str()) {
                bail!("duplicate doctor check id {:?}", check.id);
            }
            validate_doctor_check(self, check)?;
        }
        Ok(())
    }

    fn validate_scope(&self) -> Result<()> {
        if self.scope.rules.is_empty() {
            bail!("scope.rules must not be empty");
        }
        let components = self.components();
        for (index, rule) in self.scope.rules.iter().enumerate() {
            validate_scope_rule_with_components(&components, index, rule)?;
        }
        Ok(())
    }

    fn validate_steps(&self) -> Result<()> {
        if self.steps.is_empty() {
            bail!("steps must not be empty");
        }
        let mut ids = HashSet::new();
        let mut profiles = BTreeSet::new();
        for step in &self.steps {
            if !ids.insert(step.id.as_str()) {
                bail!("duplicate verification step id {:?}", step.id);
            }
            validate_step(self, step)?;
            profiles.extend(step.profiles.iter().cloned());
        }
        for step in &self.steps {
            for dependency in &step.depends_on {
                if dependency == &step.id {
                    bail!("step {:?} may not depend on itself", step.id);
                }
                if !ids.contains(dependency.as_str()) {
                    bail!(
                        "step {:?} depends on missing step {:?}",
                        step.id,
                        dependency
                    );
                }
            }
        }
        validate_step_dependencies(self)?;
        for profile in [&self.project.default_profile, &self.project.hook_profile] {
            if !profiles.contains(profile) {
                bail!("configured profile {profile:?} is not used by any step");
            }
        }
        let mut required = HashSet::new();
        for id in &self.policy.required_steps {
            if !required.insert(id.as_str()) {
                bail!("duplicate policy.required_steps entry {id:?}");
            }
            if !ids.contains(id.as_str()) {
                bail!("policy requires missing verification step {id:?}");
            }
        }
        Ok(())
    }
}

fn validate_service_entry(id: &str, service: &ServiceConfig) -> Result<()> {
    validate_id("service id", id)?;
    match service {
        ServiceConfig::Environment {
            source_env,
            inject_env,
        } => {
            validate_env_name("service.source_env", source_env)?;
            validate_env_name("service.inject_env", inject_env)?;
        }
        ServiceConfig::Docker {
            runtime: _,
            image,
            image_env,
            external_env,
            inject_env,
            external_value_policy,
            startup_timeout_secs,
            timeout_env,
            container_port,
            environment,
            healthcheck,
            connection,
        } => {
            validate_image(image)?;
            if *startup_timeout_secs == 0 || *startup_timeout_secs > 300 {
                bail!("service {id:?} startup_timeout_secs must be between 1 and 300");
            }
            if *container_port == 0 {
                bail!("service {id:?} container_port must not be zero");
            }
            if let Some(name) = image_env {
                validate_env_name("service.image_env", name)?;
            }
            if let Some(name) = timeout_env {
                validate_env_name("service.timeout_env", name)?;
            }
            if let Some(name) = external_env {
                validate_env_name("service.external_env", name)?;
            }
            if *external_value_policy != ExternalValuePolicy::None && external_env.is_none() {
                bail!("Docker service {id:?} external_value_policy requires external_env");
            }
            validate_env_name("service.inject_env", inject_env)?;
            if healthcheck.is_empty() {
                bail!("Docker service {id:?} requires a healthcheck command");
            }
            if !connection.contains("{host_port}") {
                bail!("Docker service {id:?} connection must contain {{host_port}}");
            }
            for key in environment.keys() {
                validate_env_name("service.environment key", key)?;
            }
            for value in environment.values().chain(healthcheck.iter()) {
                if value.contains('\0') {
                    bail!("Docker service {id:?} contains a NUL value");
                }
            }
        }
    }
    Ok(())
}

fn validate_parser_entry(id: &str, parser: &ParserConfig) -> Result<()> {
    validate_id("parser id", id)?;
    match parser {
        ParserConfig::Regex {
            patterns,
            capture,
            minimum,
        } => {
            if patterns.is_empty() || *minimum == 0 {
                bail!("parser {id:?} requires patterns and minimum greater than zero");
            }
            for pattern in patterns {
                let regex = Regex::new(pattern)
                    .with_context(|| format!("parser {id:?} has invalid regex"))?;
                if *capture >= regex.captures_len() {
                    bail!("parser {id:?} regex has no capture group {capture}");
                }
            }
        }
        ParserConfig::Junit { minimum } | ParserConfig::Trx { minimum } => {
            if *minimum == 0 {
                bail!("parser {id:?} minimum must be greater than zero");
            }
        }
        ParserConfig::Json {
            count_path,
            minimum,
        } => {
            if *minimum == 0 {
                bail!("parser {id:?} minimum must be greater than zero");
            }
            if count_path
                .as_deref()
                .is_some_and(|path| path.trim().is_empty() || path.starts_with('.'))
            {
                bail!("parser {id:?} count_path must be a non-empty dot path");
            }
        }
    }
    Ok(())
}

/// Doctor fields that every kind shares, validated once before the kind is
/// dispatched so the dispatch below stays within its complexity budget.
fn validate_doctor_common_fields(check: &DoctorCheck) -> Result<()> {
    validate_id("doctor check id", &check.id)?;
    if check.label.trim().is_empty() {
        bail!("doctor check {:?} requires a label", check.id);
    }
    if check.timeout_secs == 0 || check.timeout_secs > 300 {
        bail!(
            "doctor check {:?} timeout_secs must be between 1 and 300",
            check.id
        );
    }
    Ok(())
}

fn validate_doctor_check(config: &FlowConfig, check: &DoctorCheck) -> Result<()> {
    validate_doctor_common_fields(check)?;
    match &check.kind {
        DoctorCheckKind::Command { program, args } => {
            validate_program("doctor command", program)?;
            validate_arguments(config, &check.id, args)?;
        }
        DoctorCheckKind::Path {
            path, path_scope, ..
        } => {
            validate_template(config, &check.id, path)?;
            validate_doctor_path_scope(&check.id, path, *path_scope)?;
        }
        DoctorCheckKind::Glob { pattern: path } => {
            validate_template(config, &check.id, path)?;
        }
        DoctorCheckKind::Env { name } => validate_env_name("doctor env", name)?,
        DoctorCheckKind::EnvOrFile {
            env,
            path,
            contains,
            path_scope,
        } => {
            validate_env_name("doctor env", env)?;
            validate_template(config, &check.id, path)?;
            validate_doctor_path_scope(&check.id, path, *path_scope)?;
            if contains.is_empty() {
                bail!("doctor check {:?} requires non-empty contains", check.id);
            }
        }
        DoctorCheckKind::GitConfig { key, expected } => {
            if key.is_empty() || expected.is_empty() {
                bail!(
                    "doctor Git config check {:?} requires key and expected",
                    check.id
                );
            }
        }
        DoctorCheckKind::GitRemotes => {}
        DoctorCheckKind::Version {
            program,
            args,
            path,
            path_scope,
            ..
        } => {
            validate_program("doctor version program", program)?;
            validate_arguments(config, &check.id, args)?;
            validate_template(config, &check.id, path)?;
            validate_doctor_path_scope(&check.id, path, *path_scope)?;
        }
        DoctorCheckKind::Service { service } => {
            if !config.services.contains_key(service) {
                bail!(
                    "doctor check {:?} references unknown service {service:?}",
                    check.id
                );
            }
        }
    }
    Ok(())
}

/// Repository-scoped doctor paths are repository-relative, optionally behind
/// one leading path placeholder such as `{root}` or a configured alias, and
/// never traverse upward. Host locations require `path_scope = "host"`.
fn validate_doctor_path_scope(id: &str, path: &str, scope: PathScope) -> Result<()> {
    if scope == PathScope::Host {
        return Ok(());
    }
    let relative = match path.strip_prefix('{').and_then(|rest| rest.split_once('}')) {
        Some((_, rest)) => rest.trim_start_matches(['/', '\\']),
        None => path,
    };
    if relative.is_empty() {
        return Ok(());
    }
    validate_repo_path(&format!("doctor check {id:?} path"), relative).with_context(|| {
        format!("doctor check {id:?} path {path:?} is outside the repository; set path_scope = \"host\" to probe a host location")
    })
}

fn validate_scope_rule_with_components(
    components: &std::collections::BTreeSet<String>,
    index: usize,
    rule: &ScopeRule,
) -> Result<()> {
    if rule.patterns.is_empty() || rule.components.is_empty() {
        bail!("scope.rules[{index}] requires patterns and components");
    }
    for component in &rule.components {
        validate_id("scope component", component)?;
        if !components.contains(component) {
            bail!("scope.rules[{index}] references component {component:?} with no steps");
        }
    }
    for pattern in &rule.patterns {
        if pattern.contains("..") || Path::new(pattern).is_absolute() {
            bail!("scope.rules[{index}] contains unsafe pattern {pattern:?}");
        }
        Glob::new(pattern).with_context(|| {
            format!("scope.rules[{index}] contains invalid pattern {pattern:?}")
        })?;
    }
    Ok(())
}

fn validate_resource_conflicts(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    validate_log_conflicts(config, source_map, diagnostics);
    for left in 0..config.steps.len() {
        for right in (left + 1)..config.steps.len() {
            if dependency_reaches(config, left, right) || dependency_reaches(config, right, left) {
                continue;
            }
            validate_service_pair(config, source_map, diagnostics, left, right);
        }
    }
}

fn validate_log_conflicts(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let mut logs = std::collections::BTreeMap::<String, usize>::new();
    for (index, step) in config.steps.iter().enumerate() {
        // Built-in gates do not own external log files. Their empty `log`
        // field is intentional and must not collide with another gate.
        if step.kind == Some(super::model::StepKind::BuiltinGate) {
            continue;
        }
        let identity = steps::normalize_log_identity(&step.log);
        if let Some(previous) = logs.insert(identity, index) {
            let path = format!("steps[{index}].log");
            let related_path = format!("steps[{previous}].log");
            diagnostics.push(ConfigDiagnostic {
                id: "HGCFG-DUPLICATE-LOG".into(),
                severity: DiagnosticSeverity::Error,
                path: path.clone(),
                message: format!("log filename is also used by step {previous}"),
                help: "choose a unique .log filename for each verification step".into(),
                retry_class: None,
                location: source_map.location(&path),
                related: vec![RelatedDiagnostic {
                    path: related_path.clone(),
                    relation: "conflicts-with".into(),
                    location: source_map.location(&related_path),
                }],
            });
        }
    }
}

fn dependency_reaches(config: &FlowConfig, from: usize, to: usize) -> bool {
    let mut stack = vec![from];
    let mut seen = std::collections::HashSet::new();
    while let Some(index) = stack.pop() {
        if !seen.insert(index) {
            continue;
        }
        for dependency in &config.steps[index].depends_on {
            if let Some(next) = config.steps.iter().position(|step| &step.id == dependency) {
                if next == to {
                    return true;
                }
                stack.push(next);
            }
        }
    }
    false
}

fn validate_service_pair(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    left: usize,
    right: usize,
) {
    let left_services = config.steps[left]
        .services
        .iter()
        .enumerate()
        .filter_map(|(index, id)| config.services.get(id).map(|service| (index, id, service)))
        .collect::<Vec<_>>();
    let right_services = config.steps[right]
        .services
        .iter()
        .enumerate()
        .filter_map(|(index, id)| config.services.get(id).map(|service| (index, id, service)))
        .collect::<Vec<_>>();
    // The scheduler dispatches one step at a time unless parallel execution
    // has more than one worker. Shared consumers cannot overlap in serial mode.
    if config.execution.parallel && config.execution.effective_max_parallel() > 1 {
        validate_shared_services(
            source_map,
            diagnostics,
            left,
            right,
            &left_services,
            &right_services,
        );
    }
    validate_service_injections(
        source_map,
        diagnostics,
        left,
        right,
        &left_services,
        &right_services,
    );
}

type ServiceUse<'a> = (usize, &'a String, &'a ServiceConfig);

fn validate_shared_services(
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    left: usize,
    right: usize,
    left_services: &[ServiceUse<'_>],
    right_services: &[ServiceUse<'_>],
) {
    let mut shared_service_ids = BTreeSet::new();
    for (_, left_service, _) in left_services {
        if right_services
            .iter()
            .any(|(_, right_service, _)| right_service == left_service)
        {
            shared_service_ids.insert(*left_service);
        }
    }
    for service_id in shared_service_ids {
        let (left_service_index, _, _) = left_services
            .iter()
            .find(|(_, id, _)| *id == service_id)
            .expect("service appears in left step");
        let (right_service_index, _, _) = right_services
            .iter()
            .find(|(_, id, _)| *id == service_id)
            .expect("service appears in right step");
        let path = format!("steps[{right}].services[{right_service_index}]");
        let related_path = format!("steps[{left}].services[{left_service_index}]");
        diagnostics.push(ConfigDiagnostic {
            id: "HGCFG-SHARED-SERVICE".into(),
            severity: DiagnosticSeverity::Error,
            path: path.clone(),
            message: format!("steps use shared service {service_id:?} without ordering"),
            help: "add a dependency or define separate service resources".into(),
            retry_class: None,
            location: source_map.location(&path),
            related: vec![
                RelatedDiagnostic {
                    path: related_path.clone(),
                    relation: "conflicts-with".into(),
                    location: source_map.location(&related_path),
                },
                RelatedDiagnostic {
                    path: format!("services[\"{service_id}\"]"),
                    relation: "shared-resource".into(),
                    location: source_map.location(&format!("services[\"{service_id}\"]")),
                },
            ],
        });
    }
}

fn validate_service_injections(
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    left: usize,
    right: usize,
    left_services: &[ServiceUse<'_>],
    right_services: &[ServiceUse<'_>],
) {
    let mut seen_injections = BTreeSet::new();
    for (left_service_index, left_service, left_config) in left_services {
        for (right_service_index, right_service, right_config) in right_services {
            if left_service == right_service
                || service_inject_env(left_config) != service_inject_env(right_config)
            {
                continue;
            }
            let key = (
                *left_service,
                *right_service,
                service_inject_env(left_config),
            );
            if !seen_injections.insert(key) {
                continue;
            }
            let path = format!("steps[{right}].services[{right_service_index}]");
            let related_path = format!("steps[{left}].services[{left_service_index}]");
            diagnostics.push(ConfigDiagnostic {
                id: "HGCFG-SERVICE-INJECT-COLLISION".into(),
                severity: DiagnosticSeverity::Error,
                path: path.clone(),
                message: "independent services inject the same environment variable".into(),
                help: "add a dependency, use distinct inject_env names, or split the workflow"
                    .into(),
                retry_class: None,
                location: source_map.location(&path),
                related: vec![
                    RelatedDiagnostic {
                        path: related_path.clone(),
                        relation: "conflicts-with".into(),
                        location: source_map.location(&related_path),
                    },
                    RelatedDiagnostic {
                        path: format!("services[\"{left_service}\"].inject_env"),
                        relation: "injects".into(),
                        location: source_map
                            .location(&format!("services[\"{left_service}\"].inject_env")),
                    },
                    RelatedDiagnostic {
                        path: format!("services[\"{right_service}\"].inject_env"),
                        relation: "injects".into(),
                        location: source_map
                            .location(&format!("services[\"{right_service}\"].inject_env")),
                    },
                ],
            });
        }
    }
}

fn validate_report_template_paths(
    config: &FlowConfig,
    repository_root: &Path,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let (Some(template_root), Some(template)) = (
        config.report_templates.root.as_deref(),
        config.report_templates.template.as_deref(),
    ) else {
        return;
    };
    let repository_root = match repository_root.canonicalize() {
        Ok(root) => root,
        Err(_) => return,
    };
    let template_root_path = repository_root.join(template_root);
    let template_path = repository_root.join(template);
    let reports_path = repository_root.join(&config.paths.reports);
    let resolved_root = template_root_path.canonicalize();
    let resolved_template = template_path.canonicalize();
    let resolved_reports = reports_path.canonicalize().unwrap_or(reports_path);

    let invalid = |diagnostics: &mut ConfigDiagnostics,
                   path: &str,
                   message: String,
                   help: &str,
                   related: Vec<RelatedDiagnostic>| {
        diagnostics.push(ConfigDiagnostic {
            id: "HGCFG-TEMPLATE-PATH".into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message,
            help: help.into(),
            retry_class: None,
            location: source_map.location(path),
            related,
        });
    };

    let Ok(resolved_root) = resolved_root else {
        invalid(
            diagnostics,
            "report_templates.root",
            "template root does not exist".into(),
            "create a repository-contained template directory before enabling templates",
            Vec::new(),
        );
        return;
    };
    if !resolved_root.is_dir() || !resolved_root.starts_with(&repository_root) {
        invalid(
            diagnostics,
            "report_templates.root",
            "template root must resolve to a repository-contained directory".into(),
            "use a directory below the repository that does not traverse a symlink outside it",
            Vec::new(),
        );
        return;
    }
    let Ok(resolved_template) = resolved_template else {
        invalid(
            diagnostics,
            "report_templates.template",
            "template file does not exist".into(),
            "create a regular .html or .tera file below the configured template root",
            Vec::new(),
        );
        return;
    };
    if !resolved_template.is_file()
        || !resolved_template.starts_with(&resolved_root)
        || !resolved_template.starts_with(&repository_root)
    {
        invalid(
            diagnostics,
            "report_templates.template",
            "template file escapes the approved template root or is not a regular file".into(),
            "use a regular .html or .tera file below the configured template root without escaping symlinks",
            vec![RelatedDiagnostic {
                path: "report_templates.root".into(),
                relation: "must-be-under".into(),
                location: source_map.location("report_templates.root"),
            }],
        );
        return;
    }
    if resolved_root == resolved_reports
        || resolved_root.starts_with(&resolved_reports)
        || resolved_reports.starts_with(&resolved_root)
    {
        invalid(
            diagnostics,
            "report_templates.root",
            "template root overlaps the report output directory".into(),
            "use a read-only template directory disjoint from paths.reports",
            vec![RelatedDiagnostic {
                path: "paths.reports".into(),
                relation: "must-not-overlap".into(),
                location: source_map.location("paths.reports"),
            }],
        );
    }
}

fn validate_step_dependencies(config: &FlowConfig) -> Result<()> {
    fn visit(
        id: &str,
        config: &FlowConfig,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_string()) {
            bail!("verification step dependency cycle includes {:?}", id);
        }
        let step = config.step(id).expect("validated dependency");
        for dependency in &step.depends_on {
            visit(dependency, config, visiting, visited)?;
        }
        visiting.remove(id);
        visited.insert(id.to_string());
        Ok(())
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for step in &config.steps {
        visit(&step.id, config, &mut visiting, &mut visited)?;
    }
    Ok(())
}

mod primitives;
mod steps;

use primitives::{
    validate_env_name, validate_id, validate_image, validate_program, validate_repo_path,
};
use steps::{validate_arguments, validate_step, validate_template};

fn validate_version(version: u32) -> Result<()> {
    if version != CONFIG_VERSION {
        bail!(
            "unsupported workflow config version {}; expected {}; run `harness-gate config migrate` for v1 configurations",
            version,
            CONFIG_VERSION
        );
    }
    Ok(())
}

fn validate_path_alias(alias: &str, entry: &super::model::PathAlias) -> Result<()> {
    validate_id("path alias", alias)?;
    if matches!(
        alias,
        "root" | "reports" | "audit_config" | "secrets_config" | "host_port"
    ) {
        bail!("path alias {alias:?} is reserved");
    }
    validate_repo_path(&format!("paths.aliases.{alias}"), &entry.path)?;
    if let Some(name) = &entry.env {
        validate_env_name(&format!("paths.aliases.{alias}.env"), name)?;
    }
    Ok(())
}

fn push_diagnostic(
    _config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    path: &str,
    kind: ConfigIssueKind,
    message: &str,
) {
    diagnostics.push(ConfigDiagnostic {
        id: kind.id().into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        message: if message.is_empty() {
            kind.message().into()
        } else {
            message.into()
        },
        help: kind.help().into(),
        retry_class: None,
        location: source_map.location(path),
        related: Vec::new(),
    });
}

fn collect_result<E: fmt::Display>(
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
    kind: ConfigIssueKind,
    path: &str,
    result: std::result::Result<(), E>,
) {
    if result.is_err() {
        diagnostics.push(ConfigDiagnostic {
            id: kind.id().into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: kind.message().into(),
            help: kind.help().into(),
            retry_class: None,
            location: source_map.location(path),
            related: Vec::new(),
        });
    }
}

fn collect_report_template_diagnostics(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let add = |diagnostics: &mut ConfigDiagnostics, path: &str, kind: ConfigIssueKind| {
        diagnostics.push(ConfigDiagnostic {
            id: kind.id().into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: kind.message().into(),
            help: kind.help().into(),
            retry_class: None,
            location: source_map.location(path),
            related: Vec::new(),
        });
    };
    let templates = &config.report_templates;
    match (&templates.root, &templates.template) {
        (None, None) => {}
        (Some(root), Some(template)) => {
            if validate_repo_path("report_templates.root", root).is_err() {
                add(
                    diagnostics,
                    "report_templates.root",
                    ConfigIssueKind::InvalidPath,
                );
            }
            if validate_repo_path("report_templates.template", template).is_err() {
                add(
                    diagnostics,
                    "report_templates.template",
                    ConfigIssueKind::InvalidPath,
                );
            }
            if !template.ends_with(".html") && !template.ends_with(".tera") {
                add(
                    diagnostics,
                    "report_templates.template",
                    ConfigIssueKind::InvalidField,
                );
            }
        }
        (Some(_), None) | (None, Some(_)) => {
            add(
                diagnostics,
                "report_templates",
                ConfigIssueKind::InvalidField,
            );
        }
    }
    if let Some(junit) = &templates.junit {
        if validate_repo_path("report_templates.junit", junit).is_err() {
            add(
                diagnostics,
                "report_templates.junit",
                ConfigIssueKind::InvalidPath,
            );
        } else if !junit.ends_with(".xml") {
            add(
                diagnostics,
                "report_templates.junit",
                ConfigIssueKind::InvalidField,
            );
        }
    }
}

fn collect_execution_diagnostics(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let mut has_field_error = false;
    if let Some(max_parallel) = config.execution.max_parallel {
        if max_parallel == 0 || max_parallel > 64 {
            has_field_error = true;
            diagnostics.push(ConfigDiagnostic {
                id: ConfigIssueKind::InvalidField.id().into(),
                severity: DiagnosticSeverity::Error,
                path: "execution.max_parallel".into(),
                message: ConfigIssueKind::InvalidField.message().into(),
                help: ConfigIssueKind::InvalidField.help().into(),
                retry_class: None,
                location: source_map.location("execution.max_parallel"),
                related: Vec::new(),
            });
        }
    }
    for (step_id, retry) in &config.execution.retries {
        let base = format!("execution.retries[\"{step_id}\"]");
        if retry.max_attempts == 0 || retry.max_attempts > 5 {
            has_field_error = true;
            diagnostics.push(ConfigDiagnostic {
                id: ConfigIssueKind::InvalidField.id().into(),
                severity: DiagnosticSeverity::Error,
                path: format!("{base}.max_attempts"),
                message: ConfigIssueKind::InvalidField.message().into(),
                help: ConfigIssueKind::InvalidField.help().into(),
                retry_class: None,
                location: source_map.location(&format!("{base}.max_attempts")),
                related: Vec::new(),
            });
        }
    }
    // Preserve field-specific diagnostics above while applying the same
    // execution constraints as validate(), including shard bounds/references.
    if !has_field_error {
        collect_result(
            source_map,
            diagnostics,
            ConfigIssueKind::InvalidField,
            "execution",
            config.validate_execution(),
        );
    }
}

fn collect_duplicate_doctor_ids(
    config: &FlowConfig,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    let mut seen = HashSet::new();
    for (index, check) in config.doctor.checks.iter().enumerate() {
        if !seen.insert(check.id.as_str()) {
            push_diagnostic(
                config,
                source_map,
                diagnostics,
                &format!("doctor.checks[{index}].id"),
                ConfigIssueKind::DuplicateField,
                "duplicate doctor check id",
            );
        }
    }
}

fn collect_step_field_diagnostics(
    config: &FlowConfig,
    step: &super::model::StepConfig,
    index: usize,
    source_map: &SourceMap,
    diagnostics: &mut ConfigDiagnostics,
) {
    if let Err(issue) = steps::validate_step_diagnostic(config, step, index) {
        diagnostics.push(ConfigDiagnostic {
            id: issue.kind.id().into(),
            severity: DiagnosticSeverity::Error,
            path: issue.path.clone(),
            message: issue.kind.message().into(),
            help: issue.kind.help().into(),
            retry_class: None,
            location: source_map.location(&issue.path),
            related: Vec::new(),
        });
    }
}

fn service_inject_env(service: &ServiceConfig) -> &str {
    match service {
        ServiceConfig::Docker { inject_env, .. }
        | ServiceConfig::Environment { inject_env, .. } => inject_env,
    }
}
