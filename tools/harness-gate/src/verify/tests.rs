use super::parser::parse_result_count;
use super::*;
use crate::config::{FlowConfig, ParserConfig, ServiceConfig};
use crate::error::CodedError;
use crate::test_support::TestWorkspace;
use std::fs;
use std::path::PathBuf;

#[test]
fn configurable_regex_parser_counts_multiple_outputs() {
    let parser = ParserConfig::Regex {
        patterns: vec![r"(?m)^running ([0-9]+) tests?$".into()],
        capture: 1,
        minimum: 1,
    };
    let log = "running 3 tests\n...\nrunning 2 tests\n";
    assert_eq!(parse_result_count(log, &parser).expect("count"), (5, 1));
}

#[test]
fn a_new_test_framework_can_supply_its_own_pattern() {
    let parser = ParserConfig::Regex {
        patterns: vec![r"passed: ([0-9]+)".into()],
        capture: 1,
        minimum: 2,
    };
    assert_eq!(
        parse_result_count("passed: 7", &parser).expect("count"),
        (7, 2)
    );
}

#[test]
fn regex_parser_ignores_ansi_color_sequences() {
    let parser = ParserConfig::Regex {
        patterns: vec![r"Tests\s+([0-9]+) passed".into()],
        capture: 1,
        minimum: 1,
    };
    let log = "\u{1b}[1mTests\u{1b}[22m  \u{1b}[32m58 passed\u{1b}[39m";
    assert_eq!(parse_result_count(log, &parser).expect("count"), (58, 1));
}

#[test]
fn service_failure_does_not_skip_unrelated_steps() {
    let workspace = TestWorkspace::new("verify");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    let flow_path = workspace.root.join(".harness-gate/flow.toml");
    let source = fs::read_to_string(&flow_path).expect("read fixture config");
    let mut config: FlowConfig = toml::from_str(&source).expect("parse fixture config");
    let source_env = "HARNESS_GATE_MISSING_TEST".to_string();
    assert!(std::env::var_os(&source_env).is_none());
    config.services.insert(
        "missing-service".into(),
        ServiceConfig::Environment {
            source_env,
            inject_env: "TEST_SERVICE_URL".into(),
        },
    );
    config.steps[0].services = vec!["missing-service".into()];
    config.steps[1].profiles.insert("full".into());
    fs::write(
        &flow_path,
        toml::to_string_pretty(&config).expect("serialize fixture config"),
    )
    .expect("write fixture config");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");

    let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture");

    assert!(!report.passed);
    assert!(report
        .steps
        .iter()
        .any(|step| step.label == "staged Git whitespace check" && step.passed));
    let json: serde_json::Value = serde_json::from_slice(
        &fs::read(
            workspace
                .root
                .join(".harness-gate/reports/test_result.json"),
        )
        .expect("read verification report"),
    )
    .expect("parse verification report");
    assert_eq!(json["services"][0]["id"], "missing-service");
    assert_eq!(json["services"][0]["status"], "CLEANED");
}

#[cfg(unix)]
#[test]
fn independent_steps_run_in_parallel_and_publish_in_plan_order() {
    let (_workspace, mut project) = generic_project("verify-parallel");
    project.config.execution.parallel = true;
    project.config.execution.max_parallel = Some(2);
    for step in &mut project.config.steps {
        step.profiles.insert("full".into());
        step.program = "sh".into();
    }
    project.config.steps[0].args = vec![
        "-c".into(),
        "touch parallel-a.started; i=0; while [ ! -f parallel-b.started ] && [ \"$i\" -lt 200 ]; do i=$((i + 1)); sleep 0.01; done; [ -f parallel-b.started ]".into(),
    ];
    project.config.steps[1].args = vec![
        "-c".into(),
        "touch parallel-b.started; i=0; while [ ! -f parallel-a.started ] && [ \"$i\" -lt 200 ]; do i=$((i + 1)); sleep 0.01; done; [ -f parallel-a.started ]".into(),
    ];

    let report = run(&project, ScopeResult::all(&project), "full", false)
        .expect("parallel verification should pass");

    assert!(report.passed, "both concurrent handshakes must complete");
    assert!(project.root.join("parallel-a.started").is_file());
    assert!(project.root.join("parallel-b.started").is_file());
    let labels = report
        .steps
        .iter()
        .map(|step| step.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "secret scan",
            "architecture audit",
            "Git whitespace check",
            "staged Git whitespace check",
        ]
    );
}

#[test]
fn adapter_failures_choose_primary_by_plan_order() {
    use super::plan::{BuiltinGate, PlanNode, PlanNodeKind, VerificationPlan};
    use super::scheduler::{primary_failure, SchedulerError, SchedulerFailure};

    let nodes = vec![
        PlanNode {
            id: "first".into(),
            label: "first".into(),
            kind: PlanNodeKind::Builtin(BuiltinGate::SecretScan),
            depends_on: vec![],
            step: None,
        },
        PlanNode {
            id: "second".into(),
            label: "second".into(),
            kind: PlanNodeKind::Builtin(BuiltinGate::ArchitectureAudit),
            depends_on: vec![],
            step: None,
        },
    ];
    let plan = VerificationPlan { nodes };
    let failures = vec![
        SchedulerFailure {
            node_id: "second".into(),
            error: SchedulerError::Execution(anyhow::anyhow!("second")),
        },
        SchedulerFailure {
            node_id: "first".into(),
            error: SchedulerError::Execution(anyhow::anyhow!("first")),
        },
    ];
    let primary = primary_failure(&plan, failures).expect("primary failure");
    assert_eq!(primary.node_id, "first");
    assert!(matches!(primary.error, SchedulerError::Execution(_)));
}

#[cfg(unix)]
#[test]
fn timeout_is_a_failed_step_with_timeout_evidence() {
    let (_workspace, mut project) = generic_project("verify-timeout");
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec!["-c".into(), "sleep 2".into()];
    project.config.steps[0].timeout_secs = 0;

    let report = run(&project, ScopeResult::all(&project), "full", false)
        .expect("timeout is a reported verification failure");
    let step = report
        .steps
        .iter()
        .find(|step| step.label == project.config.steps[0].label)
        .expect("timed out step");
    assert!(!step.passed);
    assert!(step.timed_out);
    assert_eq!(step.detail.as_deref(), Some("timed out"));
}

#[cfg(unix)]
#[test]
fn failed_external_node_blocks_only_its_descendants() {
    let (workspace, mut project) = generic_project("verify-branch-failure");
    let first_id = project.config.steps[0].id.clone();
    let marker = workspace.root.join("independent-ran");
    project.config.steps[0].profiles.insert("full".into());
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec!["-c".into(), "exit 3".into()];
    project.config.steps[1].profiles.insert("full".into());
    project.config.steps[1].depends_on = vec![first_id];
    project.config.steps[1].program = "sh".into();
    project.config.steps[1].args = vec!["-c".into(), "exit 0".into()];
    project.config.steps.push(crate::config::StepConfig {
        id: "project.independent".into(),
        label: "independent branch".into(),
        component: "project".into(),
        profiles: ["full".to_string()].into_iter().collect(),
        program: "sh".into(),
        args: vec!["-c".into(), format!("touch {}", marker.display())],
        cwd: "{root}".into(),
        log: "independent.log".into(),
        timeout_secs: 60,
        timeout_env: None,
        parser: None,
        services: vec![],
        remove_env: vec![],
        depends_on: vec![],
        kind: None,
        gate_type: None,
        runner: None,
        input: crate::config::StepInput::Snapshot,
    });

    let report = run(&project, ScopeResult::all(&project), "full", false)
        .expect("dependency-local failure is reportable");
    assert!(!report.passed);
    assert!(marker.is_file(), "independent branch should continue");
    assert!(report
        .steps
        .iter()
        .any(|step| step.label == "independent branch" && step.passed));
    assert!(!report
        .steps
        .iter()
        .any(|step| step.label == project.config.steps[1].label));
    let json: serde_json::Value = serde_json::from_slice(
        &fs::read(
            workspace
                .root
                .join(".harness-gate/reports/test_result.json"),
        )
        .expect("read verification report"),
    )
    .expect("parse verification report");
    assert_eq!(
        json["skipped_steps"][0]["label"],
        project.config.steps[1].label
    );
    assert_eq!(
        json["skipped_steps"][0]["reason"],
        "blocked by a failed prerequisite"
    );
}

fn generic_project(name: &str) -> (TestWorkspace, Project) {
    let workspace = TestWorkspace::new(name);
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
    (workspace, project)
}

#[test]
fn explicit_builtin_labels_are_preserved_in_the_report() {
    let (_workspace, mut project) = generic_project("verify-explicit-gate-labels");
    let mut secret = project.config.steps[0].clone();
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
    project.config.steps.extend([secret, audit]);

    let report = run(&project, ScopeResult::all(&project), "full", false)
        .expect("explicit built-in gates should execute");
    let labels = report
        .steps
        .iter()
        .map(|step| step.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"repository secret policy"));
    assert!(labels.contains(&"architecture policy"));
    assert!(!labels.contains(&"secret scan"));
    assert!(!labels.contains(&"architecture audit"));
}

#[test]
fn non_zero_step_retains_exit_detail_and_log() {
    let (workspace, mut project) = generic_project("verify-non-zero");
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec!["-c".into(), "exit 7".into()];
    let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture");
    let step = report
        .steps
        .iter()
        .find(|step| step.label == project.config.steps[0].label)
        .expect("failed step");
    assert!(!step.passed);
    assert_eq!(step.detail.as_deref(), Some("exit code 7"));
    assert!(PathBuf::from(&step.log).is_file());
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.json")
        .is_file());
}

#[test]
fn verification_runs_use_distinct_invocation_evidence_directories() {
    let (workspace, project) = generic_project("verify-invocations");
    let first =
        run(&project, ScopeResult::all(&project), "full", false).expect("first verification");
    let second =
        run(&project, ScopeResult::all(&project), "full", false).expect("second verification");

    assert_ne!(first.invocation_id, second.invocation_id);
    for report in [&first, &second] {
        let directory = PathBuf::from(&report.report_directory);
        assert!(directory.is_dir(), "invocation directory should exist");
        assert!(directory.join("invocation.json").is_file());
        assert!(directory.join("test_result.json").is_file());
        assert!(report.steps.iter().all(|step| {
            step.step_id.is_some()
                && step.invocation_id.as_deref() == Some(report.invocation_id.as_str())
                && step.attempt == Some(1)
        }));
    }
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.json")
        .is_file());
}

#[test]
fn parser_failure_marks_step_failed() {
    let (_workspace, mut project) = generic_project("verify-parser-failure");
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec!["-c".into(), "printf 'done\\n'".into()];
    project.config.parsers.insert(
        "required-count".into(),
        ParserConfig::Regex {
            patterns: vec!["count: ([0-9]+)".into()],
            capture: 1,
            minimum: 1,
        },
    );
    project.config.steps[0].parser = Some("required-count".into());
    let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture");
    let step = report
        .steps
        .iter()
        .find(|step| step.label == project.config.steps[0].label)
        .expect("parsed step");
    assert!(!step.passed);
    assert_eq!(
        step.failure_code.map(|code| code.to_string()).as_deref(),
        Some("RESULT_ZERO")
    );
    assert_eq!(
        step.parser.as_ref().map(|parser| parser.mode.as_str()),
        Some("regex")
    );
    assert!(step
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("expected at least 1"));
}

#[cfg(unix)]
#[test]
fn json_parser_compatibility_preserves_success_zero_partial_and_command_failures() {
    fn run_json_fixture(
        name: &str,
        output: &str,
        parser: ParserConfig,
        exit_status: i32,
    ) -> crate::verify::VerificationReport {
        let (_workspace, mut project) = generic_project(name);
        project.config.steps[0].program = "sh".into();
        project.config.steps[0].args = vec![
            "-c".into(),
            format!("printf '%s' '{output}'; exit {exit_status}"),
        ];
        project.config.parsers.insert("json-results".into(), parser);
        project.config.steps[0].parser = Some("json-results".into());
        run(&project, ScopeResult::all(&project), "full", false).expect("verify fixture")
    }

    let success = run_json_fixture(
        "verify-json-success",
        r#"{"results":[{},{}]}"#,
        ParserConfig::Json {
            count_path: None,
            minimum: 2,
        },
        0,
    );
    let success_step = success
        .steps
        .iter()
        .find(|step| step.label == "Git whitespace check")
        .expect("success fixture step");
    assert!(success.passed);
    assert!(success_step.passed);
    assert_eq!(success_step.failure_code, None);
    assert_eq!(
        success_step.parser.as_ref().map(|parser| (
            parser.observed,
            parser.minimum,
            parser.complete
        )),
        Some((2, 2, true))
    );

    let zero = run_json_fixture(
        "verify-json-zero",
        r#"{"results":[]}"#,
        ParserConfig::Json {
            count_path: None,
            minimum: 1,
        },
        0,
    );
    let zero_step = zero
        .steps
        .iter()
        .find(|step| step.label == "Git whitespace check")
        .expect("zero fixture step");
    assert!(!zero.passed);
    assert_eq!(
        zero_step
            .failure_code
            .map(|code| code.to_string())
            .as_deref(),
        Some("RESULT_ZERO")
    );
    assert_eq!(
        zero_step
            .parser
            .as_ref()
            .map(|parser| (parser.observed, parser.minimum, parser.complete)),
        Some((0, 1, false))
    );

    let partial = run_json_fixture(
        "verify-json-partial",
        r#"{"results":[{}]}"#,
        ParserConfig::Json {
            count_path: None,
            minimum: 2,
        },
        0,
    );
    let partial_step = partial
        .steps
        .iter()
        .find(|step| step.label == "Git whitespace check")
        .expect("partial fixture step");
    assert!(!partial.passed);
    assert_eq!(
        partial_step
            .failure_code
            .map(|code| code.to_string())
            .as_deref(),
        Some("RESULT_PARTIAL")
    );
    assert_eq!(
        partial_step.parser.as_ref().map(|parser| (
            parser.observed,
            parser.minimum,
            parser.complete
        )),
        Some((1, 2, false))
    );

    let command_failure = run_json_fixture(
        "verify-json-command-failure",
        r#"{"results":[{},{}]}"#,
        ParserConfig::Json {
            count_path: None,
            minimum: 1,
        },
        7,
    );
    let command_failure_step = command_failure
        .steps
        .iter()
        .find(|step| step.label == "Git whitespace check")
        .expect("command failure fixture step");
    assert!(!command_failure.passed);
    assert!(!command_failure_step.passed);
    assert_eq!(command_failure_step.failure_code, None);
    assert!(command_failure_step
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("exit code 7")));
    assert!(
        command_failure_step.parser.is_none(),
        "a command failure must not be masked by parser success"
    );
}

#[cfg(unix)]
#[test]
fn json_parser_explicit_numeric_path_preserves_count_and_minimum() {
    let (_workspace, mut project) = generic_project("verify-json-explicit-number");
    project.config.steps[0].program = "sh".into();
    project.config.steps[0].args = vec![
        "-c".into(),
        r#"printf '%s' '{"summary":{"total":3},"results":[]}'"#.into(),
    ];
    project.config.parsers.insert(
        "json-results".into(),
        ParserConfig::Json {
            count_path: Some("summary.total".into()),
            minimum: 3,
        },
    );
    project.config.steps[0].parser = Some("json-results".into());

    let report = run(&project, ScopeResult::all(&project), "full", false)
        .expect("explicit numeric JSON fixture");
    let step = report
        .steps
        .iter()
        .find(|step| step.label == "Git whitespace check")
        .expect("explicit numeric fixture step");
    assert!(report.passed);
    assert!(step.passed);
    assert_eq!(
        step.parser
            .as_ref()
            .map(|parser| (parser.observed, parser.minimum, parser.complete)),
        Some((3, 3, true))
    );
}

#[test]
fn gate_failure_writes_compatible_report() {
    let (workspace, project) = generic_project("verify-gate-failure");
    let token = format!("{}{}", "ghp_", "123456789012345678901234567890123456");
    fs::write(
        workspace.root.join("leaked.txt"),
        format!("token = \\\"{token}\\\"\\n"),
    )
    .expect("write secret fixture");
    let report = run(&project, ScopeResult::all(&project), "full", false).expect("gate report");
    assert!(!report.passed);
    assert!(report
        .steps
        .iter()
        .any(|step| step.label == "secret scan" && !step.passed));
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.json")
        .is_file());
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.md")
        .is_file());
}

#[test]
fn builtin_adapter_failure_writes_report_before_returning_error() {
    let (workspace, mut project) = generic_project("verify-builtin-adapter-failure");
    // Keep discovery valid, then make the audit adapter fail at execution time.
    project.audit_config = workspace.root.join(".harness-gate");
    let error = run(&project, ScopeResult::all(&project), "full", false)
        .expect_err("audit adapter failure should retain its typed error");
    assert!(matches!(error, VerifyError::Audit(_)));
    let report_path = workspace
        .root
        .join(".harness-gate/reports/test_result.json");
    assert!(report_path.is_file());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(report_path).expect("read failed-gate report"))
            .expect("parse failed-gate report");
    assert_eq!(report["passed"], false);
    assert!(report["steps"].as_array().is_some_and(|steps| {
        steps.iter().any(|step| {
            step["label"] == "architecture audit"
                && step["passed"] == false
                && step["detail"]
                    .as_str()
                    .is_some_and(|detail| detail.contains("audit"))
        })
    }));
}

#[test]
fn report_write_failure_returns_error() {
    let (workspace, mut project) = generic_project("verify-report-failure");
    let report_path = workspace.root.join(".harness-gate/reports");
    fs::create_dir_all(&report_path).expect("create report directory");
    fs::create_dir(report_path.join("test_result.json")).expect("block JSON report");
    project.reports = report_path;
    let error =
        run(&project, ScopeResult::all(&project), "full", false).expect_err("report must fail");
    assert!(matches!(error, VerifyError::Report { .. }));
}

#[test]
fn report_publication_error_precedes_gate_adapter_error_and_preserves_invocation_evidence() {
    let (workspace, mut project) = generic_project("verify-gate-report-precedence");
    project.audit_config = workspace.root.join("unreadable-audit");
    fs::create_dir(&project.audit_config).unwrap();
    fs::create_dir_all(project.reports.join("test_result.json")).unwrap();
    let error = run(&project, ScopeResult::all(&project), "full", false).unwrap_err();
    assert!(matches!(error, VerifyError::Report { .. }));
    assert_eq!(error.code(), "E1404");
    let invocations = fs::read_dir(project.reports.join("invocations")).unwrap();
    let paths = invocations
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(paths[0].join("test_result.json")).unwrap()).unwrap();
    assert_eq!(json["passed"], false);
    let steps = json["steps"].as_array().unwrap();
    assert!(steps
        .iter()
        .any(|step| step["label"] == "architecture audit" && step["passed"] == false));
    assert!(steps.iter().all(|step| step["cancelled"] == false));
    assert_eq!(json["evidence_complete"], false);
    assert!(!json["failures"].as_array().unwrap().is_empty());
    assert!(!paths[0].join("manifest.json").exists());
}

#[test]
fn webhook_connection_failure_maps_to_e1404() {
    let (workspace, mut project) = generic_project("verify-webhook-failure");
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    drop(listener);
    project.config.notifications.webhooks = vec![crate::config::WebhookConfig {
        url: format!("http://{address}/notify"),
        allowed_hosts: vec![address.ip().to_string()],
        on_failure: true,
        on_success: true,
    }];
    let error = run(&project, ScopeResult::all(&project), "full", false)
        .expect_err("webhook connection failure must fail verification");
    assert_eq!(error.code(), "E1404");
    assert!(workspace
        .root
        .join(".harness-gate/reports/test_result.json")
        .is_file());
}

#[cfg(unix)]
#[test]
fn serial_shared_service_steps_execute_once_in_declaration_order() {
    for (parallel, max_parallel) in [(false, None), (true, Some(1))] {
        let (_workspace, mut project) = generic_project("verify-serial-shared-service");
        project.config.execution.parallel = parallel;
        project.config.execution.max_parallel = max_parallel;
        project.config.services.insert(
            "shared".into(),
            ServiceConfig::Environment {
                source_env: "PATH".into(),
                inject_env: "SHARED_TEST_PATH".into(),
            },
        );
        for step in &mut project.config.steps {
            step.services = vec!["shared".into()];
            step.profiles.insert("full".into());
            step.program = "sh".into();
        }
        fs::write(project.root.join("first.sh"),
            "test -n \"$SHARED_TEST_PATH\" && mkdir exclusive && sleep 0.05 && echo first > order && rmdir exclusive").expect("first script");
        fs::write(project.root.join("second.sh"),
            "test -n \"$SHARED_TEST_PATH\" && mkdir exclusive && test \"$(cat order)\" = first && echo second >> order && rmdir exclusive").expect("second script");
        project.config.steps[0].args = vec!["first.sh".into()];
        project.config.steps[1].args = vec!["second.sh".into()];
        let source = toml::to_string_pretty(&project.config).expect("serialize");
        project.config =
            FlowConfig::from_source(&source).expect("production loader accepts serial sharing");
        let report = run(&project, ScopeResult::all(&project), "full", false).expect("verify");
        assert!(report.passed);
        assert_eq!(
            fs::read_to_string(project.root.join("order")).expect("execution order"),
            "first\nsecond\n"
        );
    }
}
