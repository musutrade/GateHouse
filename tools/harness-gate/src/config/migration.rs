use super::model::{
    default_doctor_timeout, DoctorCheck, DoctorCheckKind, DoctorConfig, ExecutionConfig,
    FlowConfig, NotificationsConfig, ParserConfig, PathAlias, PathScope, PathType, PathsConfig,
    PolicyConfig, ProjectConfig, ReportTemplatesConfig, ScopeConfig, StepConfig, CONFIG_VERSION,
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyConfig {
    version: u32,
    paths: LegacyPaths,
    doctor: LegacyDoctor,
    database: LegacyDatabase,
    scope: ScopeConfig,
    steps: Vec<LegacyStep>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPaths {
    backend: String,
    frontend: String,
    reports: String,
    tool_manifest: String,
    audit_config: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDoctor {
    required_commands: Vec<String>,
    node_version_file: String,
    hooks_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDatabase {
    image: String,
    startup_timeout_secs: u64,
    container_port: u16,
    user: String,
    password: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyStep {
    id: String,
    label: String,
    component: String,
    profiles: BTreeSet<String>,
    program: String,
    args: Vec<String>,
    cwd: String,
    log: String,
    timeout_secs: u64,
    #[serde(default)]
    timeout_env: Option<String>,
    #[serde(default)]
    parser: Option<String>,
    #[serde(default)]
    requires_test_database: bool,
}

pub fn migrate_v1(source: &str, project_name: &str) -> Result<FlowConfig> {
    let legacy: LegacyConfig = toml::from_str(source).context("parse v1 workflow config")?;
    if legacy.version != 1 {
        bail!("configuration is version {}, not version 1", legacy.version);
    }

    let mut aliases = BTreeMap::new();
    aliases.insert(
        "backend".into(),
        PathAlias {
            path: legacy.paths.backend,
            env: Some("HARNESS_GATE_BACKEND".into()),
        },
    );
    aliases.insert(
        "frontend".into(),
        PathAlias {
            path: legacy.paths.frontend,
            env: Some("HARNESS_GATE_FRONTEND".into()),
        },
    );
    aliases.insert(
        "tool_manifest".into(),
        PathAlias {
            path: legacy.paths.tool_manifest,
            env: Some("HARNESS_GATE_TOOL_MANIFEST".into()),
        },
    );

    let service_id = crate::recipes::postgres::LEGACY_SERVICE_ID.to_string();
    let mut services = BTreeMap::new();
    services.insert(
        service_id.clone(),
        crate::recipes::postgres::legacy_service(&crate::recipes::postgres::LegacyPostgres {
            image: &legacy.database.image,
            startup_timeout_secs: legacy.database.startup_timeout_secs,
            container_port: legacy.database.container_port,
            user: &legacy.database.user,
            password: &legacy.database.password,
            name: &legacy.database.name,
        }),
    );

    let mut parsers = BTreeMap::new();
    parsers.insert(
        "rust".into(),
        ParserConfig::Regex {
            patterns: vec![r"(?m)^running ([0-9]+) tests?$".into()],
            capture: 1,
            minimum: 1,
        },
    );
    parsers.insert(
        "angular".into(),
        ParserConfig::Regex {
            patterns: vec![r"Tests\s+([0-9]+) passed".into()],
            capture: 1,
            minimum: 1,
        },
    );

    let steps = legacy
        .steps
        .into_iter()
        .map(|step| StepConfig {
            id: step.id,
            label: step.label,
            component: step.component,
            profiles: step.profiles,
            program: step.program,
            args: step.args,
            cwd: step.cwd,
            log: step.log,
            timeout_secs: step.timeout_secs,
            timeout_env: step.timeout_env,
            parser: step.parser,
            services: step
                .requires_test_database
                .then(|| service_id.clone())
                .into_iter()
                .collect(),
            remove_env: step
                .requires_test_database
                .then(|| "DATABASE_URL".to_string())
                .into_iter()
                .collect(),
            depends_on: Vec::new(),
            kind: None,
            gate_type: None,
            runner: None,
            input: crate::config::StepInput::Snapshot,
        })
        .collect::<Vec<_>>();
    let required_steps = steps.iter().map(|step| step.id.clone()).collect();

    let mut checks = legacy
        .doctor
        .required_commands
        .into_iter()
        .map(|program| DoctorCheck {
            id: format!("tool.{program}"),
            label: program.clone(),
            required: true,
            help: None,
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::Command {
                program,
                args: vec!["--version".into()],
            },
        })
        .collect::<Vec<_>>();
    checks.extend([
        DoctorCheck {
            id: "frontend.dependencies".into(),
            label: "frontend dependencies".into(),
            required: true,
            help: Some("run `cd frontend && npm ci`".into()),
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::Path {
                path: "{frontend}/node_modules".into(),
                path_type: PathType::Directory,
                path_scope: PathScope::Repository,
            },
        },
        DoctorCheck {
            id: "runtime.database".into(),
            label: "runtime database".into(),
            required: true,
            help: Some("create backend/.env from backend/.env.example".into()),
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::EnvOrFile {
                env: "DATABASE_URL".into(),
                path: "{backend}/.env".into(),
                contains: "DATABASE_URL=".into(),
                path_scope: PathScope::Repository,
            },
        },
        DoctorCheck {
            id: "backend.migrations".into(),
            label: "migrations".into(),
            required: true,
            help: Some("add at least one SQL migration".into()),
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::Glob {
                pattern: "{backend}/migrations/*.sql".into(),
            },
        },
        DoctorCheck {
            id: "git.hooks".into(),
            label: "Git hooks".into(),
            required: false,
            help: Some(format!(
                "run `git config core.hooksPath {}`",
                legacy.doctor.hooks_path
            )),
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::GitConfig {
                key: "core.hooksPath".into(),
                expected: legacy.doctor.hooks_path,
            },
        },
        DoctorCheck {
            id: "node.version".into(),
            label: "Node version".into(),
            required: true,
            help: None,
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::Version {
                program: "node".into(),
                args: vec!["--version".into()],
                path: format!("{{root}}/{}", legacy.doctor.node_version_file),
                trim_prefix: "v".into(),
                path_scope: PathScope::Repository,
            },
        },
        DoctorCheck {
            id: "git.remotes".into(),
            label: "Git remotes".into(),
            required: true,
            help: None,
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::GitRemotes,
        },
        DoctorCheck {
            id: "test.database".into(),
            label: "test database".into(),
            required: false,
            help: Some("configure TEST_DATABASE_URL or Docker".into()),
            timeout_secs: default_doctor_timeout(),
            kind: DoctorCheckKind::Service {
                service: service_id,
            },
        },
    ]);

    let config = FlowConfig {
        version: CONFIG_VERSION,
        project: ProjectConfig {
            name: project_name.to_string(),
            default_profile: "full".into(),
            hook_profile: "hook".into(),
        },
        paths: PathsConfig {
            reports: legacy.paths.reports,
            audit_config: legacy.paths.audit_config,
            secrets_config: ".harness-gate/secrets.toml".into(),
            aliases,
        },
        policy: PolicyConfig {
            required_steps,
            waivers: Vec::new(),
        },
        doctor: DoctorConfig { checks },
        services,
        parsers,
        report_templates: ReportTemplatesConfig::default(),
        execution: ExecutionConfig::default(),
        notifications: NotificationsConfig::default(),
        scope: legacy.scope,
        steps,
    };
    config.validate()?;
    Ok(config)
}
