use super::model::{
    DoctorCheck, DoctorCheckKind, PathAlias, ReportTemplatesConfig, RetryConfig, ShardConfig,
};
use super::*;
use crate::failure::RetryClass;
use crate::test_support::TestWorkspace;
use std::collections::BTreeSet;
use std::fs;

fn repository_config() -> FlowConfig {
    toml::from_str(include_str!(
        "../../fixtures/preset-compat/rust-api.flow.toml"
    ))
    .expect("parse fixture")
}

fn diagnostics_for(config: &FlowConfig) -> Vec<ConfigDiagnostic> {
    let source = toml::to_string_pretty(config).expect("serialize configuration");
    FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("fixture should produce diagnostics")
        .report()
        .diagnostics
}

fn assert_diagnostic(config: &FlowConfig, path: &str, id: &str) {
    let diagnostics = diagnostics_for(config);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == path && diagnostic.id == id),
        "missing {id} diagnostic at {path}; got {diagnostics:?}"
    );
}

#[test]
fn repository_configuration_is_valid() {
    repository_config().validate().expect("validate config");
}

#[test]
fn direct_validation_rejects_invalid_identity_paths_aliases_and_required_checks() {
    type Mutation = fn(&mut FlowConfig);
    let valid = repository_config();
    valid.validate().unwrap();
    let cases: &[(&str, Mutation)] = &[
        ("version", |c| c.version = 1),
        ("project name", |c| c.project.name.clear()),
        ("default profile", |c| {
            c.project.default_profile = "invalid profile".into()
        }),
        ("hook profile", |c| {
            c.project.hook_profile = "invalid profile".into()
        }),
        ("report path", |c| c.paths.reports = "../outside".into()),
        ("audit path", |c| c.paths.audit_config = "../outside".into()),
        ("secrets path", |c| {
            c.paths.secrets_config = "../outside".into()
        }),
        ("reserved alias", |c| {
            c.paths.aliases.insert(
                "root".into(),
                PathAlias {
                    path: "src".into(),
                    env: None,
                },
            );
        }),
        ("invalid alias", |c| {
            c.paths.aliases.insert(
                "invalid alias".into(),
                PathAlias {
                    path: "src".into(),
                    env: None,
                },
            );
        }),
        ("alias path", |c| {
            c.paths.aliases.insert(
                "extra".into(),
                PathAlias {
                    path: "../outside".into(),
                    env: None,
                },
            );
        }),
        ("alias environment", |c| {
            c.paths.aliases.insert(
                "extra".into(),
                PathAlias {
                    path: "src".into(),
                    env: Some("INVALID-NAME".into()),
                },
            );
        }),
        ("missing service", |c| {
            c.service_checks.insert("missing".into(), Vec::new());
        }),
    ];
    for (label, mutate) in cases {
        let mut invalid = valid.clone();
        mutate(&mut invalid);
        assert!(
            invalid.validate().is_err(),
            "{label} must fail direct validation"
        );
    }
}

#[test]
fn execution_policy_defaults_to_serial_and_four_workers_when_enabled() {
    let config = repository_config();
    assert!(!config.execution.parallel);
    assert_eq!(config.execution.effective_max_parallel(), 4);

    let mut config = config;
    config.execution.parallel = true;
    assert_eq!(config.execution.effective_max_parallel(), 4);
    config.execution.max_parallel = Some(8);
    assert_eq!(config.execution.effective_max_parallel(), 8);
    config.validate().expect("valid execution policy");
}

#[test]
fn execution_policy_rejects_zero_and_values_above_the_bound() {
    let mut config = repository_config();
    for value in [0, 65] {
        config.execution.max_parallel = Some(value);
        let error = config.validate().expect_err("invalid execution bound");
        assert!(error.to_string().contains("execution.max_parallel"));
    }
}

#[test]
fn execution_policy_diagnostic_points_to_max_parallel() {
    let mut config = repository_config();
    config.execution.max_parallel = Some(65);
    let source = toml::to_string_pretty(&config).expect("serialize invalid execution config");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("invalid execution bound must produce diagnostics");
    assert_eq!(error.report().diagnostics[0].path, "execution.max_parallel");
}

#[test]
fn versioned_runner_contract_accepts_explicit_isolation() {
    let mut config = repository_config();
    let step = config
        .steps
        .iter_mut()
        .find(|step| step.id == "rust.tests")
        .expect("rust tests step");
    step.runner = Some(RunnerConfig {
        version: 2,
        kind: "external-runner".into(),
        threads: Some(4),
        threads_env: Some("RUST_TEST_THREADS".into()),
        required_program: None,
        threads_args: Vec::new(),
        threads_args_after: None,
        args: vec!["--nocapture".into()],
        args_position: None,
        result_format: RunnerResultFormat::Junit,
        isolation: TestIsolation::SchemaPerWorker,
    });

    config.validate().expect("runner contract is valid");
}

#[test]
fn runner_rejects_shared_isolation_for_multiple_workers() {
    let mut config = repository_config();
    let step = config
        .steps
        .iter_mut()
        .find(|step| step.id == "rust.tests")
        .expect("rust tests step");
    step.runner = Some(RunnerConfig {
        version: 2,
        kind: "external-runner".into(),
        threads: Some(2),
        threads_env: Some("RUST_TEST_THREADS".into()),
        required_program: None,
        threads_args: Vec::new(),
        threads_args_after: None,
        args: Vec::new(),
        args_position: None,
        result_format: RunnerResultFormat::Regex,
        isolation: TestIsolation::Shared,
    });

    let error = config
        .validate()
        .expect_err("shared runner isolation must fail closed");
    assert!(error.to_string().contains("shared isolation"));
}

#[test]
fn generic_runner_requires_an_explicit_thread_environment() {
    let mut config = repository_config();
    let step = config
        .steps
        .iter_mut()
        .find(|step| step.id == "rust.tests")
        .expect("rust tests step");
    step.runner = Some(RunnerConfig {
        version: 2,
        kind: "generic".into(),
        threads: Some(2),
        threads_env: None,
        required_program: None,
        threads_args: Vec::new(),
        threads_args_after: None,
        args: Vec::new(),
        args_position: None,
        result_format: RunnerResultFormat::Regex,
        isolation: TestIsolation::DatabasePerWorker,
    });

    let source = toml::to_string_pretty(&config).expect("serialize invalid runner config");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("generic runner must declare thread injection");
    let diagnostic = error
        .report()
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.path == "steps[3].runner.threads_env")
        .expect("runner field diagnostic");
    assert_eq!(diagnostic.id, "HGCFG-INVALID-FIELD");
}

#[test]
fn runner_contract_rejects_invalid_fields() {
    fn runner() -> RunnerConfig {
        RunnerConfig {
            version: 2,
            kind: "generic".into(),
            threads: Some(1),
            threads_env: None,
            required_program: None,
            threads_args: Vec::new(),
            threads_args_after: None,
            args: Vec::new(),
            args_position: None,
            result_format: RunnerResultFormat::Regex,
            isolation: TestIsolation::SchemaPerWorker,
        }
    }

    let mut config = repository_config();
    let step_index = config
        .steps
        .iter()
        .position(|step| step.id == "rust.tests")
        .expect("rust tests step");

    config.steps[step_index].runner = Some(RunnerConfig {
        version: 99,
        ..runner()
    });
    assert!(config
        .validate()
        .expect_err("unsupported runner version")
        .to_string()
        .contains("runner version"));

    config.steps[step_index].runner = Some(RunnerConfig {
        required_program: Some("cargo".into()),
        ..runner()
    });
    config.steps[step_index].program = "git".into();
    assert!(config
        .validate()
        .expect_err("cargo runner on another program")
        .to_string()
        .contains("requires program"));
    config.steps[step_index].program = "cargo".into();

    config.steps[step_index].runner = Some(RunnerConfig {
        threads: Some(0),
        ..runner()
    });
    assert!(config
        .validate()
        .expect_err("zero runner threads")
        .to_string()
        .contains("threads must be between"));

    config.steps[step_index].runner = Some(RunnerConfig {
        threads_env: Some("RUST_TEST_THREADS".into()),
        threads: None,
        ..runner()
    });
    assert!(config
        .validate()
        .expect_err("thread environment without a count")
        .to_string()
        .contains("requires threads"));

    config.steps[step_index].runner = Some(RunnerConfig {
        args_position: Some(99),
        ..runner()
    });
    assert!(config
        .validate()
        .expect_err("runner argument position outside the step")
        .to_string()
        .contains("args_position"));

    config.steps[step_index].runner = Some(RunnerConfig {
        args: vec!["{unknown}".into()],
        ..runner()
    });
    assert!(config
        .validate()
        .expect_err("unsupported runner argument placeholder")
        .to_string()
        .contains("unsupported placeholder"));

    config.steps[step_index].remove_env = vec!["RUST_TEST_THREADS".into()];
    config.steps[step_index].runner = Some(RunnerConfig {
        threads: Some(2),
        threads_env: Some("RUST_TEST_THREADS".into()),
        ..runner()
    });
    assert!(config
        .validate()
        .expect_err("removed runner thread environment")
        .to_string()
        .contains("may not remove runner threads_env"));

    config.steps[step_index].remove_env.clear();
    config.steps[step_index].runner = None;
    let unknown_kind = format!(
        "{}\n[[steps]]\nid = \"future.step\"\nlabel = \"future\"\nprofiles = [\"full\"]\nkind = \"future-step\"\n",
        include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
    );
    assert!(toml::from_str::<FlowConfig>(&unknown_kind)
        .expect_err("unknown step kind")
        .to_string()
        .contains("unknown variant"));

    config.steps[step_index].kind = None;
    config.steps[step_index].cwd = "{unknown}".into();
    assert!(config
        .validate()
        .expect_err("unknown working directory placeholder")
        .to_string()
        .contains("cwd references unknown path"));

    config.steps[step_index].cwd = "root".into();
    assert!(config
        .validate()
        .expect_err("non-placeholder working directory")
        .to_string()
        .contains("cwd must be one path placeholder"));
}

#[test]
fn existing_v2_config_defaults_the_secret_rule_path() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
        .lines()
        .filter(|line| !line.starts_with("secrets_config = "))
        .collect::<Vec<_>>()
        .join("\n");
    let config = FlowConfig::from_source(&source).expect("compatible v2 config");

    assert_eq!(config.paths.secrets_config, ".harness-gate/secrets.toml");
}

#[test]
fn components_and_profiles_are_not_hard_coded() {
    let mut config = repository_config();
    config.steps[0].component = "mobile".into();
    config.steps[0].profiles.insert("ci".into());
    config.project.default_profile = "ci".into();
    config.scope.rules[0].components = BTreeSet::from(["mobile".into()]);
    config.validate().expect("custom component is valid");
}

#[test]
fn policy_steps_cannot_be_missing() {
    let mut config = repository_config();
    config.steps.retain(|step| step.id != "rust.tests");
    let error = config.validate().expect_err("missing step must fail");
    assert!(error.to_string().contains("rust.tests"));
}

#[test]
fn step_dependency_cycles_are_rejected() {
    let mut config = repository_config();
    let first = config.steps[0].id.clone();
    let second = config.steps[1].id.clone();
    config.steps[0].depends_on = vec![second];
    config.steps[1].depends_on = vec![first];
    let error = config.validate().expect_err("cycle must fail");
    assert!(error.to_string().contains("cycle"));
}

#[test]
fn shell_command_strings_are_rejected() {
    let mut config = repository_config();
    let step = config.steps.first_mut().expect("step");
    step.program = "sh".into();
    step.args = vec!["-lc".into(), "cargo fmt".into()];
    let error = config.validate().expect_err("shell command must fail");
    assert!(error.to_string().contains("shell command string"));
}

#[test]
fn duplicate_service_injection_variables_are_rejected() {
    let mut config = repository_config();
    config.services.insert(
        "test-cache".into(),
        ServiceConfig::Environment {
            source_env: "CACHE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    // Add a service to the first step and try to inject duplicate env var
    let step = config.steps.first_mut().expect("step exists");
    step.services.push("test-cache".into());

    // Add another service with same inject_env
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DB_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    step.services.push("test-db".into());

    let error = config.validate().expect_err("injection must be unique");
    assert!(error.to_string().contains("multiple services injecting"));
}

#[test]
fn runner_thread_environment_cannot_shadow_service_injection() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "RUST_TEST_THREADS".into(),
        },
    );
    let step = config
        .steps
        .iter_mut()
        .find(|step| step.id == "rust.tests")
        .expect("rust tests step");
    step.services = vec!["test-db".into()];
    step.runner = Some(RunnerConfig {
        version: 2,
        kind: "generic".into(),
        threads: Some(2),
        threads_env: Some("RUST_TEST_THREADS".into()),
        required_program: None,
        threads_args: Vec::new(),
        threads_args_after: None,
        args: Vec::new(),
        args_position: None,
        result_format: RunnerResultFormat::Regex,
        isolation: TestIsolation::SchemaPerWorker,
    });

    let source = toml::to_string_pretty(&config).expect("serialize conflicting runner config");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("runner and service env collision must fail");
    let diagnostic = error
        .report()
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.path == "steps[3].runner.threads_env")
        .expect("runner collision diagnostic");
    assert_eq!(diagnostic.id, "HGCFG-SERVICE-INJECT-COLLISION");
}

#[test]
fn unknown_fields_are_rejected() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml").replacen(
        "version = 2",
        "version = 2\nunknown = true",
        1,
    );
    assert!(toml::from_str::<FlowConfig>(&source).is_err());
}

#[test]
fn built_in_gate_declarations_are_validated_as_typed_steps() {
    let mut config = repository_config();
    let gate = &mut config.steps[0];
    gate.id = "builtin.secret-scan".into();
    gate.label = "secret scan".into();
    gate.component.clear();
    gate.program.clear();
    gate.args.clear();
    gate.cwd.clear();
    gate.log.clear();
    gate.timeout_secs = 0;
    gate.timeout_env = None;
    gate.parser = None;
    gate.services.clear();
    gate.remove_env.clear();
    gate.depends_on.clear();
    gate.kind = Some("builtin-gate".into());
    gate.gate_type = Some("secret-scan".into());
    config.policy.required_steps.clear();
    config.validate().expect("built-in gate is valid");
}

#[test]
fn both_builtin_gate_declarations_can_coexist_without_log_conflicts() {
    let mut config = repository_config();
    let mut secret = config.steps[0].clone();
    secret.id = "builtin.secret-scan".into();
    secret.label = "repository secret policy".into();
    secret.component.clear();
    secret.program.clear();
    secret.args.clear();
    secret.cwd.clear();
    secret.log.clear();
    secret.timeout_secs = 0;
    secret.timeout_env = None;
    secret.parser = None;
    secret.services.clear();
    secret.remove_env.clear();
    secret.depends_on.clear();
    secret.kind = Some("builtin-gate".into());
    secret.gate_type = Some("secret-scan".into());

    let mut audit = secret.clone();
    audit.id = "builtin.architecture-audit".into();
    audit.label = "architecture policy".into();
    audit.gate_type = Some("architecture-audit".into());
    config.steps.extend([secret, audit]);

    config.validate().expect("both built-in gates are valid");
}

#[test]
fn unknown_built_in_gate_types_fail_closed() {
    let mut config = repository_config();
    let gate = &mut config.steps[0];
    gate.id = "builtin.future".into();
    gate.component.clear();
    gate.program.clear();
    gate.args.clear();
    gate.cwd.clear();
    gate.log.clear();
    gate.timeout_secs = 0;
    gate.timeout_env = None;
    gate.parser = None;
    gate.services.clear();
    gate.remove_env.clear();
    gate.depends_on.clear();
    let unknown_gate = format!(
        "{}\n[[steps]]\nid = \"builtin.future\"\nlabel = \"future gate\"\nprofiles = [\"full\"]\nkind = \"builtin-gate\"\ngate_type = \"future-gate\"\n",
        include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
    );
    assert!(toml::from_str::<FlowConfig>(&unknown_gate)
        .expect_err("unknown gate must fail")
        .to_string()
        .contains("unknown variant"));
}

#[test]
fn external_steps_reject_gate_fields() {
    let mut config = repository_config();
    config.steps[0].kind = Some("external-step".into());
    config.steps[0].gate_type = Some("secret-scan".into());
    let error = config
        .validate()
        .expect_err("external gate field must fail");
    assert!(error.to_string().contains("may not declare gate_type"));
}

#[test]
fn environment_interpolation_supports_defaults() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml").replace(
        "name = \"rust-api\"",
        "name = \"${HG_TEST_NAME:-interpolated}\"",
    );
    let config = FlowConfig::from_source(&source).expect("interpolated config");
    assert_eq!(config.project.name, "interpolated");
}

#[test]
fn environment_interpolation_requires_defined_variables() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
        .replace("name = \"rust-api\"", "name = \"${HG_MISSING_VARIABLE}\"");
    let error = FlowConfig::from_source(&source).expect_err("missing variable must fail");
    assert!(error.to_string().contains("HG_MISSING_VARIABLE"));
}

#[test]
fn audit_configuration_path_rejects_environment_interpolation() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml").replace(
        "audit_config = \".harness-gate/audit.toml\"",
        "audit_config = \"${HG_PROJECT_AUDIT_CONFIG:-policies/audit.toml}\"",
    );
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("audit configuration must remain project-scoped");
    let diagnostic = error
        .report()
        .diagnostics
        .into_iter()
        .next()
        .expect("project-scoped diagnostic");

    assert_eq!(diagnostic.id, "HGCFG-PROJECT-SCOPED-CONFIG");
    assert_eq!(diagnostic.path, "paths.audit_config");
}

#[test]
fn version_one_configuration_can_be_migrated() {
    let source = r#"
version = 1

[paths]
backend = "backend"
frontend = "frontend"
reports = "reports"
tool_manifest = "tools/arc-flow/Cargo.toml"
audit_config = "audit.toml"

[doctor]
required_commands = ["git"]
node_version_file = ".node-version"
hooks_path = "hooks"

[database]
image = "postgres:16-alpine"
startup_timeout_secs = 30
container_port = 5432
user = "test"
password = "test"
name = "test"

[[scope.rules]]
patterns = ["src/**"]
components = ["app"]

[[steps]]
id = "app.check"
label = "app check"
component = "app"
profiles = ["full", "hook"]
program = "git"
args = ["diff", "--check"]
cwd = "{root}"
log = "app_check.log"
timeout_secs = 60
"#;
    let error = migrate_v1(source, "example").expect_err("external migration required");
    assert!(error.to_string().contains("gateprobes-postgres migrate"));
}

#[test]
fn unknown_external_value_policy_is_rejected() {
    let source = r#"
kind = "docker"
image = "postgres:16-alpine"
external_env = "TEST_DATABASE_URL"
inject_env = "TEST_DATABASE_URL"
external_value_policy = "isolated-mysql"
startup_timeout_secs = 30
container_port = 5432
healthcheck = ["true"]
connection = "postgres://unused"
"#;
    let error = toml::from_str::<ServiceConfig>(source)
        .expect_err("unknown service recipes must keep failing closed");
    let message = error.to_string();
    assert!(
        message.contains("unknown field") && message.contains("external_value_policy"),
        "{message}"
    );
}

#[test]
fn unordered_steps_reusing_a_service_are_rejected_with_field_paths() {
    let mut config = repository_config();
    config.execution.parallel = true;
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-db".into()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("unordered service reuse must fail");
    let report = error.report();

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.id == "HGCFG-SHARED-SERVICE"
            && diagnostic.path == "steps[1].services[0]"
            && diagnostic
                .related
                .iter()
                .any(|related| related.path == "steps[0].services[0]")
    }));
}

#[test]
fn dependency_order_allows_service_reuse_but_not_log_reuse() {
    let mut config = repository_config();
    config.execution.parallel = true;
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-db".into()];
    config.steps[1].depends_on = vec![config.steps[0].id.clone()];
    config.steps[1].log = config.steps[0].log.clone();

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("duplicate log must fail");
    let report = error.report();

    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-DUPLICATE-LOG"));
    assert!(!report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-SHARED-SERVICE"));
}

#[test]
fn step_logs_reject_windows_and_path_like_names_on_every_platform() {
    let mut config = repository_config();
    for value in [
        "..\\outside.log",
        "C:\\outside.log",
        "\\\\server\\share.log",
        "nested/output.log",
        "stream:output.log",
    ] {
        config.steps[0].log = value.into();
        let error = config
            .validate()
            .expect_err("path-like log name must fail closed");
        assert!(error.to_string().contains("log"), "{value}: {error:#}");
    }
}

#[test]
fn duplicate_logs_use_a_conservative_case_normalized_identity() {
    let mut config = repository_config();
    config.steps[0].log = "Unit-tests.log".into();
    config.steps[1].log = "unit-tests.log".into();

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("case-insensitive log collision must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-DUPLICATE-LOG"));
}

#[test]
fn independent_services_with_the_same_injection_are_rejected() {
    let mut config = repository_config();
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_SERVICE_URL".into(),
        },
    );
    config.services.insert(
        "test-cache".into(),
        ServiceConfig::Environment {
            source_env: "CACHE_URL".into(),
            inject_env: "TEST_SERVICE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-cache".into()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("injection conflict must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-SERVICE-INJECT-COLLISION"));
}

#[test]
fn repeated_service_references_emit_one_shared_resource_diagnostic_per_pair() {
    let mut config = repository_config();
    config.execution.parallel = true;
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[1].services = vec!["test-db".into()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("unordered service reuse must fail");
    let shared = error
        .report()
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.id == "HGCFG-SHARED-SERVICE")
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 1);
}

#[test]
fn transitive_dependency_allows_service_reuse() {
    let mut config = repository_config();
    config.execution.parallel = true;
    config.services.insert(
        "test-db".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["test-db".into()];
    config.steps[2].services = vec!["test-db".into()];
    config.steps[1].depends_on = vec![config.steps[0].id.clone()];
    config.steps[2].depends_on = vec![config.steps[1].id.clone()];

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect("transitive dependency must order service reuse");
}

#[test]
fn independent_semantic_errors_are_aggregated_with_precise_paths() {
    let mut config = repository_config();
    config.steps[0].timeout_secs = 0;
    config.steps[1].parser = Some("missing-parser".into());

    let source = toml::to_string_pretty(&config).expect("serialize fixture");
    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("independent configuration errors must be aggregated");
    let report = error.report();

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "steps[0].timeout_secs"
            && diagnostic.id == "HGCFG-INVALID-FIELD"
            && diagnostic.location.is_some()
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "steps[1].parser"
            && diagnostic.id == "HGCFG-UNKNOWN-REFERENCE"
            && diagnostic.location.is_some()
    }));
}

#[test]
fn human_diagnostic_location_has_one_closing_parenthesis() {
    let mut config = repository_config();
    config.steps[0].timeout_secs = 0;
    let source = toml::to_string_pretty(&config).expect("serialize fixture");

    let error = FlowConfig::from_source_with_diagnostics(&source, None, None)
        .expect_err("timeout must fail validation");
    let rendered = error.to_string();
    assert!(rendered.contains("(line "), "{rendered}");
    assert!(!rendered.contains("))"), "{rendered}");
}

#[test]
fn terminal_diagnostics_escape_paths_and_preserve_related_locations() {
    use super::diagnostic::{RelatedDiagnostic, SourceLocation};
    let mut diagnostics = ConfigDiagnostics::empty().with_source("flow\n\u{1b}[2J.toml");
    let mut diagnostic = ConfigDiagnostic::synthetic(
        "HGCFG-DUPLICATE-LOG",
        "steps[1].log",
        "two steps share a log",
        "choose a unique log filename",
    );
    diagnostic.location = Some(SourceLocation {
        line: 12,
        column: 1,
    });
    diagnostic.related = vec![RelatedDiagnostic {
        path: "steps[0].log".into(),
        relation: "first log".into(),
        location: Some(SourceLocation { line: 6, column: 1 }),
    }];
    diagnostics.push(diagnostic);
    let text = diagnostics.render_terminal();
    assert!(text.contains("flow\\n\\u{1b}[2J.toml:12:1"), "{text}");
    assert!(text.contains("first log: steps[0].log (flow\\n\\u{1b}[2J.toml:6:1)"));
    assert!(!text.contains("flow\n"));
    assert!(!text.contains("\u{1b}[2J"));
}

#[test]
fn terminal_diagnostics_bound_text_and_support_missing_locations() {
    let diagnostics = ConfigDiagnostics::single(
        ConfigCode::HgcfgCheck,
        "$",
        "x".repeat(20_000),
        "inspect configuration",
    );
    let text = diagnostics.render_terminal();
    assert!(text.contains("  --> <configuration>"));
    assert!(!text.contains(&"x".repeat(1025)));
    assert!(text.contains("explain: harness-gate explain HGCFG-CHECK"));
    // Presentation bounds do not truncate the machine contract.
    assert_eq!(diagnostics.report().diagnostics[0].message.len(), 20_000);
}

#[test]
fn terminal_diagnostics_redact_metadata_without_mutating_the_report() {
    use super::diagnostic::RelatedDiagnostic;
    let mut diagnostics = ConfigDiagnostics::empty().with_source("password=synthetic-source");
    diagnostics.push(ConfigDiagnostic::new(
        ConfigCode::HgcfgCheck,
        DiagnosticSeverity::Error,
        "token=synthetic-path",
        "******\ncontext\u{1b}[2J\tcontext",
        "secret=synthetic-help\rcontext",
        None,
        None,
        vec![RelatedDiagnostic {
            path: "token=synthetic-related-path".into(),
            relation: "******".into(),
            location: None,
        }],
    ));
    let before = serde_json::to_vec(&diagnostics.report()).unwrap();
    let legacy = diagnostics.to_string();
    let text = diagnostics.render_terminal();
    assert!(!text.contains("synthetic-"), "{text}");
    assert!(text.contains("[REDACTED]"));
    assert!(text.contains("\\ncontext\\u{1b}[2J"));
    assert!(text.contains("\\rcontext") && text.contains("\\tcontext"));
    assert!(!text.contains("\u{1b}[2J") && !text.contains('\r') && !text.contains('\t'));
    assert_eq!(serde_json::to_vec(&diagnostics.report()).unwrap(), before);
    assert_eq!(diagnostics.to_string(), legacy);
}

#[test]
fn diagnostics_truncate_at_the_documented_maximum() {
    let mut diagnostics = ConfigDiagnostics::empty();
    for index in 0..51 {
        diagnostics.push(ConfigDiagnostic::synthetic(
            format!("HGCFG-TEST-{index}"),
            format!("steps[{index}]"),
            "test diagnostic",
            "test help",
        ));
    }

    let report = diagnostics.report();
    assert_eq!(report.diagnostics.len(), 50);
    assert!(report.truncated);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-DIAGNOSTICS-TRUNCATED"));
}

#[test]
fn diagnostics_serialize_typed_retry_class_without_message_inference() {
    let mut diagnostics = ConfigDiagnostics::empty();
    let mut diagnostic = ConfigDiagnostic::synthetic(
        "HGCFG-TEST-RETRY",
        "steps[0]",
        "transient test diagnostic",
        "retry the validation command",
    );
    diagnostic.retry_class = Some(RetryClass::Timeout);
    diagnostics.push(diagnostic);

    let value = serde_json::to_value(diagnostics.report()).expect("serialize diagnostics");
    assert_eq!(value["diagnostics"][0]["retry_class"], "timeout");
}

#[test]
fn discovery_errors_keep_configuration_field_paths_in_json_reports() {
    let error = anyhow::Error::new(ConfigDiagnostics::single(
        ConfigCode::HgcfgRequiredFile,
        "paths.secrets_config",
        "required secret scan configuration file is missing",
        "create the configured secret scan configuration file or update paths.secrets_config",
    ));
    let report = report_for_error(&error);
    assert!(!report.valid);
    assert_eq!(report.diagnostics[0].id, "HGCFG-REQUIRED-FILE");
    assert_eq!(report.diagnostics[0].path, "paths.secrets_config");
}

#[test]
fn template_paths_must_stay_inside_a_disjoint_repository_root() {
    let workspace = TestWorkspace::new("template-config");
    fs::create_dir_all(workspace.root.join("templates")).expect("create template root");
    fs::write(workspace.root.join("templates/report.tera"), "report").expect("write template");
    let path = workspace.root.join("flow.toml");
    let source = format!(
        "{}\n[report_templates]\nroot = \"templates\"\ntemplate = \"templates/report.tera\"\n",
        include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
    );
    fs::write(&path, source).expect("write config");
    FlowConfig::load_with_diagnostics(&path, Some(&workspace.root)).expect("safe template config");

    fs::write(
        &path,
        format!(
            "{}\n[report_templates]\nroot = \".harness-gate/reports\"\ntemplate = \".harness-gate/reports/report.tera\"\n",
            include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
        ),
    )
    .expect("write overlapping config");
    fs::create_dir_all(workspace.root.join(".harness-gate/reports"))
        .expect("create report directory");
    fs::write(
        workspace.root.join(".harness-gate/reports/report.tera"),
        "report",
    )
    .expect("write report template");
    let error = FlowConfig::load_with_diagnostics(&path, Some(&workspace.root))
        .expect_err("report overlap must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-TEMPLATE-PATH"));
}

#[test]
fn windows_prefixed_paths_are_rejected_on_every_platform() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml").replace(
        "reports = \".harness-gate/reports\"",
        "reports = \"C:\\\\reports\"",
    );
    let error = FlowConfig::from_source(&source).expect_err("Windows prefix must fail");
    assert!(error.to_string().contains("paths.reports"));
}

#[test]
fn windows_style_parent_traversal_is_rejected_on_every_platform() {
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml").replace(
        "reports = \".harness-gate/reports\"",
        "reports = \"..\\\\outside\"",
    );
    let error = FlowConfig::from_source(&source).expect_err("Windows traversal must fail");
    assert!(error.to_string().contains("paths.reports"));
}

#[cfg(unix)]
#[test]
fn template_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("template-symlink");
    let outside = TestWorkspace::new("template-outside");
    fs::create_dir_all(workspace.root.join("templates")).expect("create template root");
    fs::write(outside.root.join("outside.tera"), "outside").expect("write outside template");
    symlink(
        outside.root.join("outside.tera"),
        workspace.root.join("templates/escape.tera"),
    )
    .expect("link outside template");
    let path = workspace.root.join("flow.toml");
    fs::write(
        &path,
        format!(
            "{}\n[report_templates]\nroot = \"templates\"\ntemplate = \"templates/escape.tera\"\n",
            include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
        ),
    )
    .expect("write config");

    let error = FlowConfig::load_with_diagnostics(&path, Some(&workspace.root))
        .expect_err("symlink escape must fail");
    assert!(error
        .report()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.id == "HGCFG-TEMPLATE-PATH"));
}

#[test]
fn junit_report_path_must_stay_relative_to_the_report_directory() {
    let source = format!(
        "{}\n[report_templates]\njunit = \"../junit.xml\"\n",
        include_str!("../../fixtures/preset-compat/rust-api.flow.toml")
    );
    let error = FlowConfig::from_source(&source).expect_err("JUnit traversal must fail");
    assert!(error.to_string().contains("report_templates.junit"));
}

#[test]
fn retired_webhook_settings_are_accepted_without_delivery_validation() {
    let fixture = include_str!("../../fixtures/preset-compat/rust-api.flow.toml");
    for retired in [
        "url = \"ftp://example.test/hook\"\nallowed_hosts = [\"example.test\"]\n",
        "url = \"https://example.test/hook\"\nallowed_hosts = [\"example.test\"]\non_failure = false\non_success = false\n",
        "url = \"https://user:secret@example.test/hook\"\nallowed_hosts = []\n",
        "url = \"https://example.test/hook\"\nallowed_hosts = [\"*.example.test\"]\n",
        "url = \"https://example.test/hook\"\nallowed_hosts = [\"other.test\"]\n",
    ] {
        let source = format!("{fixture}\n[[notifications.webhooks]]\n{retired}");
        let config = FlowConfig::from_source(&source).unwrap_or_else(|error| {
            panic!("retired transport settings must not gate local verification: {error}")
        });
        assert!(
            config.validate().is_ok(),
            "retired transport settings must pass validation: {retired}"
        );
        assert!(
            config.diagnostics_report().valid,
            "retired transport settings must not emit configuration diagnostics: {retired}"
        );
    }

    let unknown = format!(
        "{fixture}\n[[notifications.webhooks]]\nurl = \"https://example.test/hook\"\nallowed_hosts = [\"example.test\"]\ndelivery_mode = \"batch\"\n"
    );
    assert!(
        FlowConfig::from_source(&unknown).is_err(),
        "unknown webhook keys stay rejected"
    );

    let wrong_type = format!(
        "{fixture}\n[[notifications.webhooks]]\nurl = 7\nallowed_hosts = [\"example.test\"]\n"
    );
    assert!(
        FlowConfig::from_source(&wrong_type).is_err(),
        "typed webhook fields stay enforced"
    );
}

#[test]
fn report_template_diagnostics_cover_pair_paths_and_extensions() {
    let mut config = repository_config();
    config.report_templates = ReportTemplatesConfig {
        source_repository: None,
        root: Some("../templates".into()),
        template: Some("../report.txt".into()),
        junit: Some("../junit.txt".into()),
    };
    let diagnostics = diagnostics_for(&config);
    for (path, id) in [
        ("report_templates.root", "HGCFG-INVALID-PATH"),
        ("report_templates.template", "HGCFG-INVALID-PATH"),
        ("report_templates.template", "HGCFG-INVALID-FIELD"),
        ("report_templates.junit", "HGCFG-INVALID-PATH"),
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path == path && diagnostic.id == id),
            "missing {id} diagnostic at {path}: {diagnostics:?}"
        );
    }

    config.report_templates = ReportTemplatesConfig {
        source_repository: None,
        root: Some("templates".into()),
        template: None,
        junit: Some("junit.txt".into()),
    };
    let diagnostics = diagnostics_for(&config);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "report_templates" && diagnostic.id == "HGCFG-INVALID-FIELD"
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "report_templates.junit" && diagnostic.id == "HGCFG-INVALID-FIELD"
    }));
}

#[test]
fn configured_report_template_validation_covers_pairs_and_extensions() {
    fn validate(
        source_repository: Option<&str>,
        root: Option<&str>,
        template: Option<&str>,
        junit: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut config = repository_config();
        config.report_templates = ReportTemplatesConfig {
            source_repository: source_repository.map(str::to_owned),
            root: root.map(str::to_owned),
            template: template.map(str::to_owned),
            junit: junit.map(str::to_owned),
        };
        config.validate()
    }
    fn invalid(
        source_repository: Option<&str>,
        root: Option<&str>,
        template: Option<&str>,
        junit: Option<&str>,
        expected: &str,
    ) {
        let error = validate(source_repository, root, template, junit)
            .expect_err("configuration must be rejected")
            .to_string();
        assert!(error.contains(expected), "{expected}: {error}");
    }

    validate(None, None, None, None).expect("default report templates");
    validate(None, None, None, Some("reports/junit.xml")).expect("JUnit without HTML");
    validate(
        Some("https://github.com/musutrade/GateHouse"),
        Some("templates"),
        Some("templates/report.html"),
        Some("reports/junit.xml"),
    )
    .expect("configured HTML and JUnit");
    validate(
        Some("https://github.com/musutrade/GateHouse"),
        Some("templates"),
        Some("templates/report.tera"),
        None,
    )
    .expect("tera HTML template");

    validate(
        Some("https://gitlab.example/example/project?token=secret"),
        None,
        None,
        None,
    )
    .expect("retired source repository values are accepted for compatibility");
    invalid(
        None,
        Some("templates"),
        None,
        None,
        "report_templates.root and report_templates.template must be configured together",
    );
    invalid(
        None,
        None,
        Some("templates/report.html"),
        None,
        "report_templates.root and report_templates.template must be configured together",
    );
    invalid(
        None,
        Some("templates"),
        Some("templates/report.txt"),
        None,
        "report_templates.template must use a .html or .tera extension",
    );
    invalid(
        None,
        None,
        None,
        Some("reports/junit.txt"),
        "report_templates.junit must use a .xml extension",
    );
    invalid(
        None,
        Some("templates"),
        Some("templates/report.tera"),
        Some("reports/junit.txt"),
        "report_templates.junit must use a .xml extension",
    );
    invalid(
        None,
        Some("../templates"),
        Some("templates/report.html"),
        None,
        "report_templates.root may not escape the repository",
    );
    invalid(
        None,
        Some(""),
        Some("templates/report.html"),
        None,
        "report_templates.root must be a non-empty repository-relative path",
    );
    invalid(
        None,
        Some("templates"),
        Some("../report.html"),
        None,
        "report_templates.template may not escape the repository",
    );
    invalid(
        None,
        None,
        None,
        Some("../junit.xml"),
        "report_templates.junit may not escape the repository",
    );
    invalid(
        None,
        Some("templates"),
        Some("templates/report.html"),
        Some("../junit.xml"),
        "report_templates.junit may not escape the repository",
    );
}

#[test]
fn execution_retry_diagnostic_points_to_the_attempt_limit() {
    let mut config = repository_config();
    config.execution.retries.insert(
        "rust.tests".into(),
        RetryConfig {
            max_attempts: 0,
            backoff_ms: 0,
            retryable: BTreeSet::new(),
        },
    );

    assert_diagnostic(
        &config,
        "execution.retries[\"rust.tests\"].max_attempts",
        "HGCFG-INVALID-FIELD",
    );
}

#[test]
fn builtin_gate_diagnostics_preserve_typed_field_paths() {
    fn builtin_fixture() -> (FlowConfig, usize) {
        let mut config = repository_config();
        let index = config.steps.len();
        let mut gate = config.steps[0].clone();
        gate.id = "builtin.secret-scan".into();
        gate.label = "secret scan".into();
        gate.component.clear();
        gate.program.clear();
        gate.args.clear();
        gate.cwd.clear();
        gate.log.clear();
        gate.timeout_secs = 0;
        gate.timeout_env = None;
        gate.parser = None;
        gate.services.clear();
        gate.remove_env.clear();
        gate.depends_on.clear();
        gate.kind = Some(StepKind::BuiltinGate);
        gate.gate_type = Some(BuiltinGateType::SecretScan);
        config.steps.push(gate);
        (config, index)
    }

    let (mut config, index) = builtin_fixture();
    config.steps[index].gate_type = None;
    assert_diagnostic(
        &config,
        &format!("steps[{index}].gate_type"),
        "HGCFG-INVALID-FIELD",
    );

    let (mut config, index) = builtin_fixture();
    config.steps[index].id = "builtin.wrong".into();
    assert_diagnostic(
        &config,
        &format!("steps[{index}].id"),
        "HGCFG-INVALID-FIELD",
    );

    let (mut config, index) = builtin_fixture();
    config.steps[index].depends_on = vec!["rust.format".into()];
    assert_diagnostic(
        &config,
        &format!("steps[{index}].depends_on"),
        "HGCFG-DEPENDENCY",
    );

    let (mut config, index) = builtin_fixture();
    config.steps[index].program = "cargo".into();
    assert_diagnostic(&config, &format!("steps[{index}]"), "HGCFG-INVALID-FIELD");
}

#[test]
fn external_step_diagnostics_cover_closed_field_contract() {
    let mut config = repository_config();
    config.steps[0].kind = Some(StepKind::ExternalStep);
    config.steps[0].gate_type = Some(BuiltinGateType::SecretScan);
    assert_diagnostic(&config, "steps[0].gate_type", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].id = "builtin.secret-scan".into();
    assert_diagnostic(&config, "steps[0].id", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].profiles = BTreeSet::from(["Invalid Profile".into()]);
    assert_diagnostic(&config, "steps[0].profiles", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].program = "sh".into();
    config.steps[0].args = vec!["-lc".into(), "cargo fmt".into()];
    assert_diagnostic(&config, "steps[0].args", "HGCFG-INVALID-FIELD");
}

#[test]
fn external_step_diagnostics_cover_paths_logs_timeouts_and_references() {
    let mut config = repository_config();
    config.steps[0].cwd = "root".into();
    assert_diagnostic(&config, "steps[0].cwd", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].cwd = "{missing}".into();
    assert_diagnostic(&config, "steps[0].cwd", "HGCFG-UNKNOWN-REFERENCE");

    let mut config = repository_config();
    config.steps[0].log = "nested/output.log".into();
    assert_diagnostic(&config, "steps[0].log", "HGCFG-INVALID-LOG");

    let mut config = repository_config();
    config.steps[0].timeout_secs = 0;
    assert_diagnostic(&config, "steps[0].timeout_secs", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].timeout_env = Some("lowercase".into());
    assert_diagnostic(&config, "steps[0].timeout_env", "HGCFG-INVALID-ENVIRONMENT");

    let mut config = repository_config();
    config.steps[0].parser = Some("missing".into());
    assert_diagnostic(&config, "steps[0].parser", "HGCFG-UNKNOWN-REFERENCE");
}

fn runner_fixture() -> RunnerConfig {
    RunnerConfig {
        version: 2,
        kind: "generic".into(),
        threads: Some(1),
        threads_env: None,
        required_program: None,
        threads_args: Vec::new(),
        threads_args_after: None,
        args: Vec::new(),
        args_position: None,
        result_format: RunnerResultFormat::Regex,
        isolation: TestIsolation::SchemaPerWorker,
    }
}

#[test]
fn runner_diagnostics_cover_version_kind_program_and_thread_contracts() {
    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        version: 99,
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].runner.version", "HGCFG-RUNNER-VERSION");

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        kind: "Invalid Kind".into(),
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].runner.kind", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        required_program: Some("cargo".into()),
        ..runner_fixture()
    });
    config.steps[0].program = "git".into();
    assert_diagnostic(&config, "steps[0].runner", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        threads: Some(0),
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].runner.threads", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        threads: None,
        threads_env: Some("TEST_THREADS".into()),
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].runner.threads", "HGCFG-INVALID-FIELD");
}

#[test]
fn runner_diagnostics_cover_environment_isolation_and_arguments() {
    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        threads_env: Some("lowercase".into()),
        ..runner_fixture()
    });
    assert_diagnostic(
        &config,
        "steps[0].runner.threads_env",
        "HGCFG-INVALID-ENVIRONMENT",
    );

    let mut config = repository_config();
    config.steps[0].remove_env = vec!["TEST_THREADS".into()];
    config.steps[0].runner = Some(RunnerConfig {
        threads_env: Some("TEST_THREADS".into()),
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].remove_env", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        threads: Some(2),
        ..runner_fixture()
    });
    assert_diagnostic(
        &config,
        "steps[0].runner.threads_env",
        "HGCFG-INVALID-FIELD",
    );

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        threads: Some(2),
        threads_env: Some("TEST_THREADS".into()),
        isolation: TestIsolation::Shared,
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].runner.isolation", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        args_position: Some(config.steps[0].args.len() + 1),
        ..runner_fixture()
    });
    assert_diagnostic(
        &config,
        "steps[0].runner.args_position",
        "HGCFG-INVALID-FIELD",
    );

    let mut config = repository_config();
    config.steps[0].runner = Some(RunnerConfig {
        args: vec!["{missing}".into()],
        ..runner_fixture()
    });
    assert_diagnostic(&config, "steps[0].runner.args", "HGCFG-INVALID-FIELD");
}

#[test]
fn service_diagnostics_cover_duplicates_references_and_environment_collisions() {
    let mut config = repository_config();
    config.services.insert(
        "database".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["database".into(), "database".into()];
    assert_diagnostic(&config, "steps[0].services[1]", "HGCFG-DUPLICATE-FIELD");

    let mut config = repository_config();
    config.steps[0].services = vec!["missing".into()];
    assert_diagnostic(&config, "steps[0].services[0]", "HGCFG-UNKNOWN-REFERENCE");

    let mut config = repository_config();
    config.services.insert(
        "database".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    config.steps[0].services = vec!["database".into()];
    config.steps[0].remove_env = vec!["TEST_DATABASE_URL".into()];
    assert_diagnostic(&config, "steps[0].remove_env", "HGCFG-INVALID-FIELD");
}

#[test]
fn service_diagnostics_cover_runner_and_multi_service_injection_collisions() {
    let mut config = repository_config();
    config.services.insert(
        "database".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_THREADS".into(),
        },
    );
    config.steps[0].services = vec!["database".into()];
    config.steps[0].runner = Some(RunnerConfig {
        threads_env: Some("TEST_THREADS".into()),
        ..runner_fixture()
    });
    assert_diagnostic(
        &config,
        "steps[0].runner.threads_env",
        "HGCFG-SERVICE-INJECT-COLLISION",
    );

    let mut config = repository_config();
    for id in ["database", "cache"] {
        config.services.insert(
            id.into(),
            ServiceConfig::Environment {
                source_env: format!("{}_URL", id.to_ascii_uppercase()),
                inject_env: "TEST_SERVICE_URL".into(),
            },
        );
    }
    config.steps[0].services = vec!["database".into(), "cache".into()];
    assert_diagnostic(
        &config,
        "steps[0].services[1]",
        "HGCFG-SERVICE-INJECT-COLLISION",
    );
}

#[test]
fn doctor_paths_outside_the_repository_require_host_scope() {
    for (kind, value) in [
        ("path", "/etc/passwd"),
        ("path", "../outside"),
        ("path", "{root}/../outside"),
        ("env-or-file", "/home/user/.pgpass"),
        ("version", "..\\outside\\.node-version"),
    ] {
        let path_kind = |path: &str, scope| match kind {
            "path" => DoctorCheckKind::Path {
                path: path.into(),
                path_type: PathType::Any,
                path_scope: scope,
            },
            "env-or-file" => DoctorCheckKind::EnvOrFile {
                env: "DATABASE_URL".into(),
                path: path.into(),
                contains: "DATABASE_URL=".into(),
                path_scope: scope,
            },
            _ => DoctorCheckKind::Version {
                program: "node".into(),
                args: vec!["--version".into()],
                path: path.into(),
                trim_prefix: "v".into(),
                path_scope: scope,
            },
        };
        let check = |scope| DoctorCheck {
            id: "host.path".into(),
            label: "host path".into(),
            required: true,
            help: None,
            timeout_secs: 15,
            kind: path_kind(value, scope),
        };
        let mut config = repository_config();
        config.doctor.checks.push(check(PathScope::Repository));
        let index = config.doctor.checks.len() - 1;
        assert_diagnostic(
            &config,
            &format!("doctor.checks[{index}]"),
            "HGCFG-INVALID-FIELD",
        );
        let mut config = repository_config();
        config.doctor.checks.push(check(PathScope::Host));
        assert!(
            config.validate().is_ok(),
            "{kind} {value}: host scope accepts an explicit host path"
        );
    }
}

#[test]
fn doctor_common_check_fields_are_validated_before_the_kind() {
    let check = |label: &str, timeout_secs: u64| DoctorCheck {
        id: "host.path".into(),
        label: label.into(),
        required: true,
        help: None,
        timeout_secs,
        kind: DoctorCheckKind::Path {
            path: "{root}/inside.txt".into(),
            path_type: PathType::Any,
            path_scope: PathScope::Repository,
        },
    };
    for (label, timeout_secs, case) in [
        ("", 15, "blank label"),
        ("   ", 15, "whitespace label"),
        ("host path", 0, "zero timeout"),
        ("host path", 301, "timeout above the bound"),
    ] {
        let mut config = repository_config();
        config.doctor.checks.push(check(label, timeout_secs));
        let index = config.doctor.checks.len() - 1;
        assert_diagnostic(
            &config,
            &format!("doctor.checks[{index}]"),
            "HGCFG-INVALID-FIELD",
        );
        assert!(
            config.validate().is_err(),
            "{case}: strict validation must reject the check"
        );
    }
    for timeout_secs in [1, 300] {
        let mut config = repository_config();
        config.doctor.checks.push(check("host path", timeout_secs));
        assert!(
            config.validate().is_ok(),
            "timeout_secs = {timeout_secs} must remain inside the bounds"
        );
    }
}

#[test]
fn doctor_parser_alias_and_scope_diagnostics_keep_configuration_paths() {
    let mut config = repository_config();
    config.doctor.checks.push(config.doctor.checks[0].clone());
    let duplicate_index = config.doctor.checks.len() - 1;
    assert_diagnostic(
        &config,
        &format!("doctor.checks[{duplicate_index}].id"),
        "HGCFG-DUPLICATE-FIELD",
    );

    let mut config = repository_config();
    config.doctor.checks.push(DoctorCheck {
        id: "service.database".into(),
        label: "database".into(),
        required: true,
        help: None,
        timeout_secs: 15,
        kind: DoctorCheckKind::Service {
            service: "missing".into(),
        },
    });
    let index = config.doctor.checks.len() - 1;
    assert_diagnostic(
        &config,
        &format!("doctor.checks[{index}]"),
        "HGCFG-INVALID-FIELD",
    );

    let mut config = repository_config();
    config.parsers.insert(
        "invalid".into(),
        ParserConfig::Json {
            count_path: Some(".tests".into()),
            minimum: 1,
        },
    );
    assert_diagnostic(&config, "parsers[\"invalid\"]", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.paths.aliases.insert(
        "root".into(),
        PathAlias {
            path: "src".into(),
            env: None,
        },
    );
    assert_diagnostic(&config, "paths.aliases[\"root\"]", "HGCFG-INVALID-FIELD");

    let mut config = repository_config();
    config.scope.rules.clear();
    assert_diagnostic(&config, "scope.rules", "HGCFG-INVALID-FIELD");
}

#[test]
fn execution_contracts_reject_unknown_or_unsafe_inputs() {
    let mut config = repository_config();
    config.execution.retries.insert(
        "missing".into(),
        RetryConfig {
            max_attempts: 1,
            backoff_ms: 0,
            retryable: BTreeSet::new(),
        },
    );
    assert!(config
        .validate()
        .expect_err("unknown retry step")
        .to_string()
        .contains("missing step"));

    let mut config = repository_config();
    config
        .execution
        .shards
        .insert("rust.tests".into(), ShardConfig { index: 0, total: 2 });
    assert!(config
        .validate()
        .expect_err("shards require a runner")
        .to_string()
        .contains("requires a runner"));

    for (url, allowed_hosts) in [
        (
            concat!("https://", "user:secret@example.test/hook"),
            vec!["example.test"],
        ),
        ("https://example.test/hook", Vec::new()),
        ("https://example.test/hook", vec!["*.example.test"]),
        ("https://example.test/hook", vec!["other.test"]),
    ] {
        let mut config = repository_config();
        config.notifications.webhooks.push(WebhookConfig {
            url: url.into(),
            allowed_hosts: allowed_hosts.into_iter().map(str::to_owned).collect(),
            on_failure: true,
            on_success: false,
        });
        assert!(
            config.validate().is_ok(),
            "retired transport settings must not gate verification: {url}"
        );
    }
}

#[test]
fn flow_config_accessors_and_schema_cover_the_public_configuration_surface() {
    let mut config = repository_config();
    config.services.insert(
        "database".into(),
        ServiceConfig::Environment {
            source_env: "DATABASE_URL".into(),
            inject_env: "TEST_DATABASE_URL".into(),
        },
    );
    assert!(config.step("rust.tests").is_some());
    assert!(config.parser("rust").is_some());
    assert!(config.service("database").is_some());
    assert!(config.allowed_placeholder("reports"));
    assert!(config.diagnostics_report().valid);
    assert!(schema_json().expect("schema JSON").contains("FlowConfig"));
}

#[test]
fn serial_shared_service_consumers_load_without_added_dependencies() {
    for (parallel, max_parallel) in [(false, None), (false, Some(8)), (true, Some(1))] {
        let mut config = repository_config();
        config.execution.parallel = parallel;
        config.execution.max_parallel = max_parallel;
        config.services.insert(
            "test-db".into(),
            ServiceConfig::Environment {
                source_env: "DATABASE_URL".into(),
                inject_env: "TEST_DATABASE_URL".into(),
            },
        );
        config.steps[0].services = vec!["test-db".into()];
        config.steps[1].services = vec!["test-db".into()];
        let source = toml::to_string_pretty(&config).expect("serialize");
        let loaded = FlowConfig::from_source_with_diagnostics(&source, None, None)
            .expect("single-worker dispatch safely orders shared consumers");
        assert!(loaded.steps[1].depends_on.is_empty());
        config.steps[1].log = config.steps[0].log.clone();
        assert_diagnostic(&config, "steps[1].log", "HGCFG-DUPLICATE-LOG");
    }
}

#[test]
fn configuration_documents_above_the_size_limit_are_rejected_before_parsing() {
    let root = TestWorkspace::new("config-size-limit");
    let path = root.root.join("flow.toml");
    let limit = super::loader::MAX_CONFIG_BYTES as usize;
    fs::write(&path, "#".repeat(limit)).unwrap();
    assert_eq!(super::read_config_source(&path).unwrap().len(), limit);
    fs::write(&path, "#".repeat(limit + 1)).unwrap();
    let error = super::read_config_source(&path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("configuration limit"));
    let diagnostics = FlowConfig::load_with_diagnostics(&path, None).unwrap_err();
    assert!(
        format!("{diagnostics}").contains("exceeds the size limit"),
        "{diagnostics}"
    );
}

#[test]
fn runner_v2_declarations_fail_closed_without_name_exemptions() {
    let mut config = repository_config();
    for runner in [
        RunnerConfig {
            required_program: Some("".into()),
            ..runner_fixture()
        },
        RunnerConfig {
            required_program: Some("cargo\0".into()),
            ..runner_fixture()
        },
        RunnerConfig {
            threads_args: vec!["{threads}".into()],
            threads_args_after: Some("{other}".into()),
            ..runner_fixture()
        },
        RunnerConfig {
            version: 1,
            ..runner_fixture()
        },
        RunnerConfig {
            kind: "cargo-test".into(),
            threads: Some(2),
            ..runner_fixture()
        },
        RunnerConfig {
            threads_args: vec!["--workers".into()],
            ..runner_fixture()
        },
        RunnerConfig {
            threads: None,
            threads_args: vec!["{threads}".into()],
            ..runner_fixture()
        },
        RunnerConfig {
            threads_args: vec!["{other}".into(), "{threads}".into()],
            ..runner_fixture()
        },
        RunnerConfig {
            threads_args_after: Some("--".into()),
            ..runner_fixture()
        },
        RunnerConfig {
            threads_args: vec!["{threads}".into()],
            threads_args_after: Some("".into()),
            ..runner_fixture()
        },
        RunnerConfig {
            threads_args: vec!["{threads}\0".into()],
            ..runner_fixture()
        },
    ] {
        config.steps[0].runner = Some(runner);
        assert!(config.validate().is_err());
        let source = toml::to_string(&config).unwrap();
        assert!(FlowConfig::from_source_with_diagnostics(&source, None, None).is_err());
    }
    for kind in ["cargo-test", "never-seen-before"] {
        config.steps[0].runner = Some(RunnerConfig {
            kind: kind.into(),
            threads: Some(2),
            required_program: Some(config.steps[0].program.clone()),
            threads_args: vec!["--workers={threads}".into()],
            threads_args_after: Some("SEP".into()),
            ..runner_fixture()
        });
        config.validate().unwrap();
        let source = toml::to_string(&config).unwrap();
        FlowConfig::from_source_with_diagnostics(&source, None, None).unwrap();
    }
}

/// Opt-in attribution measurement; ordinary test/CI runs do not execute it.
#[cfg(feature = "config-benchmark")]
#[test]
#[ignore = "run through tools/quality/config_benchmark.py"]
fn configuration_loader_measurement() {
    use sha2::{Digest, Sha256};
    use std::hint::black_box;
    use std::time::Instant;

    for name in [
        "REPORT_DIR",
        "HARNESS_GATE_REPORTS",
        "HARNESS_GATE_SECRETS_CONFIG",
    ] {
        assert!(
            std::env::var_os(name).is_none(),
            "unset {name} for fixed-input measurement"
        );
    }
    let source = include_str!("../../fixtures/preset-compat/rust-api.flow.toml");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flow.toml");
    fs::write(&path, source).unwrap();
    let memory = FlowConfig::from_source_with_diagnostics(source, Some(&path), None).unwrap();
    let file = FlowConfig::load_with_diagnostics(&path, None).unwrap();
    assert_eq!(
        serde_json::to_value(&memory).unwrap(),
        serde_json::to_value(&file).unwrap()
    );
    let repetitions = 100;
    let mut memory_seconds = Vec::new();
    let mut file_seconds = Vec::new();
    // Both paths warmed above; alternate order to reduce systematic order bias.
    for sample in 0..5 {
        for file_first in [sample % 2 == 0, sample % 2 != 0] {
            let started = Instant::now();
            for _ in 0..repetitions {
                let config = if file_first {
                    FlowConfig::load_with_diagnostics(black_box(&path), None)
                } else {
                    FlowConfig::from_source_with_diagnostics(black_box(source), Some(&path), None)
                }
                .unwrap();
                black_box(config);
            }
            let seconds = started.elapsed().as_secs_f64();
            if file_first {
                file_seconds.push(seconds);
            } else {
                memory_seconds.push(seconds);
            }
        }
    }
    println!(
        "CONFIG_BENCHMARK_JSON={}",
        serde_json::json!({
            "series": "config-loader/1", "fixture_bytes": source.len(),
            "fixture_sha256": format!("{:x}", Sha256::digest(source.as_bytes())),
            "steps": memory.steps.len(), "repetitions": repetitions,
            "memory_batch_seconds": memory_seconds, "warm_file_batch_seconds": file_seconds,
            "equivalent": true, "repository_root": null,
            "scope": "parse, diagnostics, environment application and validation; allocation/drop included; file path adds warm read; no repository filesystem validation"
        })
    );
}

fn config_with_policy_waiver() -> FlowConfig {
    let mut config = repository_config();
    config.policy.waivers.push(super::model::WaiverConfig {
        id: "reviewed-exception".into(),
        step: config.steps[0].id.clone(),
        scope: Some("component-a".into()),
        risk: "temporary test outage".into(),
        reason: "repair scheduled".into(),
        owner: "maintainer".into(),
        approved_by: "reviewer".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        expires_at: "2026-01-02T00:00:00Z".into(),
        compensating_control: "manual review".into(),
        revoked: false,
    });
    config
}

#[test]
fn policy_waiver_configuration_accepts_valid_scoped_and_unscoped_records() {
    for scope in [None, Some("component-a".to_string())] {
        let mut config = config_with_policy_waiver();
        config.policy.waivers[0].scope = scope.clone();
        let source = toml::to_string(&config).unwrap();
        let loaded = FlowConfig::from_source_with_diagnostics(&source, None, None)
            .expect("structurally valid waiver; runtime expiry is checked separately");
        assert_eq!(loaded.policy.waivers.len(), 1);
        assert_eq!(loaded.policy.waivers[0].id, "reviewed-exception");
        assert_eq!(loaded.policy.waivers[0].step, config.steps[0].id);
        assert_eq!(loaded.policy.waivers[0].scope, scope);
    }
}

#[test]
fn policy_waiver_configuration_rejects_invalid_approval_and_time_boundaries() {
    type Mutation = fn(&mut super::model::WaiverConfig);
    let cases: &[(&str, Mutation)] = &[
        ("invalid id", |w| w.id = "invalid id".into()),
        ("unknown step", |w| w.step = "missing-step".into()),
        ("empty risk", |w| w.risk = " ".into()),
        ("empty reason", |w| w.reason = "\t".into()),
        ("empty owner", |w| w.owner = " ".into()),
        ("empty approver", |w| w.approved_by = " ".into()),
        ("empty creation time", |w| w.created_at = " ".into()),
        ("empty compensating control", |w| {
            w.compensating_control = " ".into()
        }),
        ("self approval", |w| w.approved_by = w.owner.clone()),
        ("invalid creation time", |w| {
            w.created_at = "not-a-date".into()
        }),
        ("invalid expiry", |w| w.expires_at = "not-a-date".into()),
        ("equal expiry", |w| w.expires_at = w.created_at.clone()),
        ("earlier expiry", |w| {
            w.expires_at = "2025-12-31T23:59:59Z".into()
        }),
        ("empty scope", |w| w.scope = Some(" ".into())),
    ];
    let valid = config_with_policy_waiver();
    for (name, mutation) in cases {
        let mut config = valid.clone();
        mutation(&mut config.policy.waivers[0]);
        let diagnostics = diagnostics_for(&config);
        assert_eq!(diagnostics.len(), 1, "{name}: {diagnostics:?}");
        assert_eq!(diagnostics[0].path, "policy", "{name}");
        assert_eq!(diagnostics[0].id, "HGCFG-INVALID-FIELD", "{name}");
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error, "{name}");
    }
    let mut duplicate = valid;
    duplicate
        .policy
        .waivers
        .push(duplicate.policy.waivers[0].clone());
    assert_diagnostic(&duplicate, "policy", "HGCFG-INVALID-FIELD");
}

#[test]
fn retired_report_source_repository_is_accepted_without_diagnostics() {
    let mut config = repository_config();
    assert!(config.report_templates.source_repository.is_none());
    for retired in [
        "https://github.com/musutrade/GateHouse",
        "https://gitlab.example/example/project?token=secret",
        "https://user:secret@example.invalid/a/b",
        "not a repository",
    ] {
        config.report_templates.source_repository = Some(retired.into());
        config
            .validate()
            .expect("a retired setting must not gate configuration");
        let source = toml::to_string_pretty(&config).expect("serialize configuration");
        let diagnostics = FlowConfig::from_source_with_diagnostics(&source, None, None)
            .err()
            .map(|error| error.report().diagnostics)
            .unwrap_or_default();
        assert!(
            diagnostics
                .iter()
                .all(|d| d.path != "report_templates.source_repository"),
            "{retired}"
        );
    }
}

#[test]
fn history_archive_is_opt_in_and_rejects_unsafe_or_overlapping_paths() {
    let mut config = repository_config();
    assert!(config.report_history.archive.is_none());
    config.report_history.archive = Some(".harness-gate/history".into());
    assert!(config.validate().is_ok());
    for path in [
        "",
        "../history",
        "/history",
        ".git/history",
        ".harness-gate",
        ".harness-gate/reports/archive",
        "archive/../history",
    ] {
        config.report_history.archive = Some(path.into());
        assert!(config.validate().is_err(), "{path}");
        assert!(diagnostics_for(&config)
            .iter()
            .any(|d| d.path == "report_history.archive"));
    }
    config.report_templates.root = Some("templates".into());
    config.report_templates.template = Some("templates/report.html".into());
    config.report_history.archive = Some("templates/history".into());
    assert!(config.validate().is_err());
}
