use super::*;
use crate::{config::FlowConfig, process::Task, scope::ScopeResult, test_support::TestWorkspace};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture {
    _repository: TestWorkspace,
    _host: TestWorkspace,
    project: Project,
    calls: AtomicUsize,
}
impl Fixture {
    fn new() -> Self {
        let repository = TestWorkspace::new("cache-repository");
        let host = TestWorkspace::new("cache-host");
        crate::preset::init(&repository.root, "generic", false).unwrap();
        fs::write(repository.root.join("source.txt"), "source-v1\n").unwrap();
        fs::write(repository.root.join("toolchain.txt"), "tools-v1\n").unwrap();
        fs::write(
            repository.root.join(".gitignore"),
            ".harness-gate/reports/\nproduct.txt\nignored-input\n",
        )
        .unwrap();
        repository.init_git();
        for args in [vec!["add", "."], vec!["commit", "-qm", "fixture"]] {
            assert!(crate::utils::git::capture(&repository.root, args)
                .unwrap()
                .status
                .success());
        }
        let mut project = Project::discover(Some(repository.root.clone()), None).unwrap();
        project.prepare().unwrap();
        project.step_cache_dir = Some(host.root.join("cache"));
        project.cache_profile = Some("full".into());
        project.config.steps.truncate(1);
        let step = &mut project.config.steps[0];
        step.program = "cat".into();
        step.args = vec!["source.txt".into()];
        step.profiles.insert("full".into());
        step.depends_on.clear();
        step.parser = None;
        step.input = crate::config::StepInput::Snapshot;
        step.cache = Some(
            serde_json::from_value(serde_json::json!({
                "version":1, "deterministic":true, "side_effect_free":true,
                "inputs":[], "outputs":[], "environment":[],
                "toolchain":[fs::canonicalize("/bin/cat").unwrap(), repository.root.join("toolchain.txt")]
            }))
            .unwrap(),
        );
        project.config.project.hook_profile = "full".into();
        project.config.policy.required_steps = vec![project.config.steps[0].id.clone()];
        let source = toml::to_string_pretty(&project.config).unwrap();
        FlowConfig::from_source(&source)
            .expect("cache fixture is valid through the real configuration loader");
        Self {
            _repository: repository,
            _host: host,
            project,
            calls: AtomicUsize::new(0),
        }
    }
    fn call(&self) -> TaskResult {
        let step = &self.project.config.steps[0];
        run(&self.project, step, || {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let task = Task::new(
                &step.label,
                &step.cache.as_ref().unwrap().toolchain[0],
                &self.project.execution_root,
                super::super::steps::log(&self.project, &step.log)?,
            )
            .args(&step.args)
            .clear_environment();
            super::super::steps::evaluate_result(
                task.run()?,
                step.parser
                    .as_deref()
                    .and_then(|id| self.project.config.parser(id)),
            )
        })
        .unwrap()
    }
    fn warm(&self) {
        super::prepare(
            &self.project,
            &self.project.config.steps[0],
            self.project.step_cache_dir.as_ref().unwrap(),
        )
        .unwrap();
        let first = self.call();
        assert!(first.passed);
        assert_eq!(first.cache.as_ref().unwrap().status, "MISS", "{first:?}");
        assert_ne!(
            first.cache.as_ref().unwrap().reason,
            "CACHE_STORE_REJECTED",
            "{first:?}"
        );
        assert_eq!(self.call().cache.unwrap().status, "HIT");
    }
}

#[test]
fn hits_revalidate_logs_and_do_not_execute_or_replay_attempts() {
    let fixture = Fixture::new();
    fixture.warm();
    let result = fixture.call();
    assert_eq!(fixture.calls.load(Ordering::Relaxed), 1);
    assert_eq!(fs::read_to_string(&result.log).unwrap(), "source-v1\n");
    assert!(result.attempts.is_empty());
    assert!(result.cache.unwrap().source_invocation.is_some());
}

#[test]
fn actual_working_bytes_configuration_profile_and_tools_invalidate() {
    let mut fixture = Fixture::new();
    fixture.warm();
    for change in 0..5 {
        match change {
            0 => fs::write(
                fixture.project.root.join("source.txt"),
                "dirty working source",
            )
            .unwrap(),
            1 => fixture.project.cache_profile = Some("hook".into()),
            2 => fixture.project.config.project.name.push_str("-changed"),
            3 => fixture.project.config.steps[0]
                .args
                .push("source.txt".into()),
            _ => fs::write(fixture.project.root.join("toolchain.txt"), "tools-v2").unwrap(),
        }
        let result = fixture.call();
        assert_eq!(result.cache.unwrap().status, "MISS");
        assert_eq!(fixture.call().cache.unwrap().status, "HIT");
    }
    assert_eq!(fixture.calls.load(Ordering::Relaxed), 6);
}

#[test]
fn declared_outputs_are_checked_and_never_restored_from_old_pass() {
    let mut fixture = Fixture::new();
    let step = &mut fixture.project.config.steps[0];
    step.program = fs::canonicalize("/bin/cp")
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .into();
    step.cache.as_mut().unwrap().toolchain[0] = fs::canonicalize("/bin/cp")
        .unwrap()
        .to_string_lossy()
        .into_owned();
    step.args = vec!["source.txt".into(), "product.txt".into()];
    step.cache.as_mut().unwrap().outputs = vec!["product.txt".into()];
    fixture.warm();
    for replacement in [Some("tampered"), None] {
        let output = fixture.project.root.join("product.txt");
        if let Some(value) = replacement {
            fs::write(&output, value).unwrap();
        } else {
            fs::remove_file(&output).unwrap();
        }
        let result = fixture.call();
        assert_eq!(
            result.cache.unwrap().reason,
            "CACHE_OUTPUT_MISSING_OR_CHANGED"
        );
        assert_eq!(fs::read_to_string(output).unwrap(), "source-v1\n");
    }
    assert_eq!(fixture.calls.load(Ordering::Relaxed), 3);
}

#[test]
fn missing_ignored_inputs_and_unbound_dependencies_bypass() {
    let mut fixture = Fixture::new();
    fixture.project.config.steps[0]
        .cache
        .as_mut()
        .unwrap()
        .inputs = vec!["ignored-input".into()];
    assert_eq!(fixture.call().cache.unwrap().status, "BYPASS");
    fs::write(fixture.project.root.join("ignored-input"), "one").unwrap();
    fixture.warm();
    fs::write(fixture.project.root.join("ignored-input"), "two").unwrap();
    assert_eq!(fixture.call().cache.unwrap().status, "MISS");
    fixture.project.config.steps[0].depends_on = vec!["unbound".into()];
    assert_eq!(fixture.call().cache.unwrap().status, "BYPASS");
}

#[test]
fn failures_and_input_mutation_are_never_persisted() {
    let mut fixture = Fixture::new();
    fixture.project.config.steps[0].args = vec!["missing-source".into()];
    for _ in 0..2 {
        let r = fixture.call();
        assert!(!r.passed);
        assert_eq!(r.cache.unwrap().status, "MISS");
    }
    fixture.project.config.steps[0].args = vec!["source.txt".into()];
    let step = &fixture.project.config.steps[0];
    let result = run(&fixture.project, step, || {
        let result = Task::new(
            "changed",
            &step.cache.as_ref().unwrap().toolchain[0],
            &fixture.project.root,
            super::super::steps::log(&fixture.project, &step.log)?,
        )
        .args(&step.args)
        .run()?;
        fs::write(
            fixture.project.root.join("source.txt"),
            "changed mid execution",
        )?;
        Ok(result)
    })
    .unwrap();
    assert_eq!(result.cache.unwrap().reason, "CACHE_STORE_REJECTED");
    assert_eq!(fixture.call().cache.unwrap().status, "MISS");
}

#[test]
fn tampered_cache_does_not_pass_and_clear_preserves_unknown_material() {
    let fixture = Fixture::new();
    fixture.warm();
    let directory = fixture.project.step_cache_dir.as_ref().unwrap();
    let entry = fs::read_dir(directory)
        .unwrap()
        .map(|p| p.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&entry).unwrap()).unwrap();
    value["record"]["log"] = serde_json::json!("Zm9yZ2Vk");
    fs::write(&entry, serde_json::to_vec(&value).unwrap()).unwrap();
    let result = fixture.call();
    assert_eq!(result.cache.unwrap().status, "MISS");
    assert_eq!(fs::read_to_string(result.log).unwrap(), "source-v1\n");
    fs::write(directory.join("unrelated"), "retain").unwrap();
    let report = clear(directory).unwrap();
    assert_eq!(report["removed"], 0);
    assert_eq!(report["retained"], 2);
    assert!(entry.is_file());
    assert!(directory.join("unrelated").is_file());
}

#[test]
fn unsafe_declarations_are_diagnostics_and_host_omission_disables() {
    let mut fixture = Fixture::new();
    let mut config = fixture.project.config.clone();
    for case in 0..11 {
        config.steps[0] = fixture.project.config.steps[0].clone();
        let step = &mut config.steps[0];
        match case {
            0 => step.cache.as_mut().unwrap().version = 2,
            1 => step.cache.as_mut().unwrap().deterministic = false,
            2 => step.cache.as_mut().unwrap().side_effect_free = false,
            3 => step.services = vec!["database".into()],
            4 => step.input = crate::config::StepInput::Repository,
            5 => step.cache.as_mut().unwrap().inputs = vec!["../escape".into()],
            6 => step.cache.as_mut().unwrap().outputs = vec![".git/HEAD".into()],
            7 => step.cache.as_mut().unwrap().environment = vec!["INVALID=KEY".into()],
            8 => step.cache.as_mut().unwrap().toolchain.clear(),
            9 => step.cache.as_mut().unwrap().toolchain[0] = "/different/executable".into(),
            _ => {
                step.cache.as_mut().unwrap().inputs = vec![".harness-gate/reports/input.log".into()]
            }
        }
        assert!(
            crate::config::validate_step_cache(&config, &config.steps[0]).is_err(),
            "case {case}"
        );
        let source = toml::to_string(&config).unwrap();
        assert!(FlowConfig::from_source(&source).is_err(), "case {case}");
    }
    fixture.project.step_cache_dir = None;
    assert_eq!(fixture.call().cache.unwrap().reason, "CACHE_HOST_DISABLED");
}

#[test]
fn staged_and_working_tree_inputs_cannot_cross_hit() {
    let fixture = Fixture::new();
    fixture.warm();
    fs::write(
        fixture.project.root.join("source.txt"),
        "unstaged different bytes",
    )
    .unwrap();
    let mut staged = fixture.project.staged_snapshot().unwrap();
    staged.config = fixture.project.config.clone();
    staged.cache_profile = Some("full".into());
    staged.prepare().unwrap();
    let working_key = identity::build(&fixture.project, &fixture.project.config.steps[0]).unwrap();
    let staged_key = identity::build(&staged, &staged.config.steps[0]).unwrap();
    assert_ne!(working_key, staged_key);
    assert!(String::from_utf8(staged_key).unwrap().contains("staged"));
}

#[test]
fn full_verification_runs_current_mandatory_gates_even_after_hit() {
    let fixture = Fixture::new();
    let first = crate::verify::run(
        &fixture.project,
        ScopeResult::all(&fixture.project),
        "full",
        false,
    )
    .unwrap();
    assert!(first.passed);
    let second = crate::verify::run(
        &fixture.project,
        ScopeResult::all(&fixture.project),
        "full",
        false,
    )
    .unwrap();
    assert!(second.passed);
    let step = second.steps.iter().find(|s| s.cache.is_some()).unwrap();
    assert_eq!(step.cache.as_ref().unwrap().status, "HIT");
    assert_eq!(second.steps.len(), 3);
    assert_ne!(first.invocation_id, second.invocation_id);
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.project.reports.join("test_result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        report["steps"][2]["cache"]["schema_version"],
        "step-cache/v1"
    );
    assert_eq!(report["steps"][2]["cache"]["status"], "HIT");
}

#[test]
fn dependency_outputs_and_environment_are_bound_without_plaintext_keys() {
    let mut fixture = Fixture::new();
    let mut producer = fixture.project.config.steps[0].clone();
    producer.id = "producer".into();
    producer.cache.as_mut().unwrap().outputs = vec!["product.txt".into()];
    fixture.project.config.steps.push(producer);
    fixture.project.config.steps[0].depends_on = vec!["producer".into()];
    fs::write(fixture.project.root.join("product.txt"), "dependency-v1").unwrap();
    fixture.warm();
    fs::write(fixture.project.root.join("product.txt"), "dependency-v2").unwrap();
    assert_eq!(fixture.call().cache.unwrap().status, "MISS");
    let name = "GATEHOUSE_CACHE_TEST_SECRET_32";
    fixture.project.config.steps[0]
        .cache
        .as_mut()
        .unwrap()
        .environment = vec![name.into()];
    // A unique test-owned name; no other test reads or writes it.
    std::env::set_var(name, "synthetic-secret-one");
    let first = fixture.call();
    assert_eq!(fixture.call().cache.unwrap().status, "HIT");
    std::env::set_var(name, "synthetic-secret-two");
    let second = fixture.call();
    std::env::remove_var(name);
    assert_ne!(
        first.cache.as_ref().unwrap().key,
        second.cache.as_ref().unwrap().key
    );
    assert!(!serde_json::to_string(&second.cache)
        .unwrap()
        .contains("synthetic-secret"));
    for entry in fs::read_dir(fixture.project.step_cache_dir.as_ref().unwrap()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            assert!(!fs::read_to_string(path)
                .unwrap()
                .contains("synthetic-secret"));
        }
    }
}

#[test]
fn current_parser_is_reapplied_and_partial_results_are_not_cached() {
    let mut fixture = Fixture::new();
    fixture.project.config.steps[0].parser = Some("count".into());
    fixture.project.config.parsers.insert(
        "count".into(),
        crate::config::ParserConfig::Regex {
            patterns: vec!["source-v([0-9]+)".into()],
            capture: 1,
            minimum: 1,
        },
    );
    fixture.warm();
    fixture.project.config.parsers.insert(
        "count".into(),
        crate::config::ParserConfig::Regex {
            patterns: vec!["source-v([0-9]+)".into()],
            capture: 1,
            minimum: 2,
        },
    );
    for _ in 0..2 {
        let result = fixture.call();
        assert!(!result.passed);
        assert_eq!(result.cache.unwrap().status, "MISS");
    }
}

#[test]
fn timed_out_cancelled_flaky_or_waived_results_cannot_be_stored() {
    for flag in 0..4 {
        let fixture = Fixture::new();
        let step = &fixture.project.config.steps[0];
        run(&fixture.project, step, || {
            let mut result = Task::new(
                "result",
                &step.cache.as_ref().unwrap().toolchain[0],
                &fixture.project.root,
                super::super::steps::log(&fixture.project, &step.log)?,
            )
            .args(&step.args)
            .run()?;
            match flag {
                0 => result.timed_out = true,
                1 => result.cancelled = true,
                2 => result.flaky = true,
                _ => result.waived = true,
            }
            Ok(result)
        })
        .unwrap();
        assert_eq!(fixture.call().cache.unwrap().status, "MISS");
    }
}

#[test]
fn ignored_quality_policy_and_log_secrets_never_escape_input_binding() {
    let fixture = Fixture::new();
    let quality = include_str!("../../../fixtures/workflow/quality.toml")
        .replace(
            "name = \"rust-api\"",
            &format!("name = {:?}", fixture.project.config.project.name),
        )
        .replace(
            "flow_components = [\"app\"]",
            "flow_components = [\"project\"]",
        );
    let start = quality.find("[profiles.hook]").unwrap();
    let end = quality[start..].find("[baseline]").unwrap() + start;
    let quality = format!("{}{}", &quality[..start], &quality[end..]);
    fs::write(
        fixture.project.root.join(".harness-gate/quality.toml"),
        quality,
    )
    .unwrap();
    fs::create_dir(fixture.project.root.join("src")).unwrap();
    fs::write(
        fixture.project.root.join("src/lib.rs"),
        "pub fn fixture() {}\n",
    )
    .unwrap();
    let policy = fixture.project.root.join(".harness-gate/policy.json");
    fs::write(
        &policy,
        include_str!("../../../fixtures/workflow/policy.json"),
    )
    .unwrap();
    let ignore = fixture.project.root.join(".gitignore");
    let mut source = fs::read_to_string(&ignore).unwrap();
    source.push_str(".harness-gate/policy.json\n");
    fs::write(ignore, source).unwrap();
    fixture.warm();
    let mut policy_bytes = fs::read_to_string(&policy).unwrap();
    policy_bytes.push('\n');
    fs::write(&policy, policy_bytes).unwrap();
    assert_eq!(fixture.call().cache.unwrap().status, "MISS");
    fs::write(
        fixture.project.root.join("source.txt"),
        "Authorization: Bearer synthetic-secret\n",
    )
    .unwrap();
    let result = fixture.call();
    assert_eq!(result.cache.unwrap().reason, "CACHE_STORE_REJECTED");
    let directory = fixture.project.step_cache_dir.as_ref().unwrap();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            assert!(!fs::read_to_string(path)
                .unwrap()
                .contains("synthetic-secret"));
        }
    }
}

#[test]
fn declared_cache_environment_is_the_actual_command_environment() {
    let mut fixture = Fixture::new();
    let step = &mut fixture.project.config.steps[0];
    step.program = "env".into();
    step.cache.as_mut().unwrap().toolchain[0] = fs::canonicalize("/usr/bin/env")
        .unwrap()
        .to_string_lossy()
        .into_owned();
    step.args.clear();
    step.cache.as_mut().unwrap().environment = vec!["PATH".into()];
    let services = std::sync::Mutex::new(crate::service::ServiceManager::new(&fixture.project));
    let result = super::super::steps::run_configured_step(
        &fixture.project,
        &fixture.project.config.steps[0],
        &services,
    )
    .unwrap();
    assert!(result.passed);
    let log = fs::read_to_string(&result.log).unwrap();
    assert_eq!(log.lines().count(), 1);
    assert!(log.starts_with("PATH="));
}

#[test]
fn plan_does_not_open_cache_and_reports_runtime_identity_unvalidated() {
    let fixture = Fixture::new();
    let path = fixture.project.step_cache_dir.as_ref().unwrap();
    assert!(!path.exists());
    let plan = super::super::inspection::build(
        &fixture.project,
        &ScopeResult::all(&fixture.project),
        "full",
        true,
    )
    .unwrap();
    assert!(plan["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["code"] == "CACHE_RUNTIME_NOT_VALIDATED")));
    assert!(!path.exists());
}

#[test]
fn tracked_outputs_reports_and_repository_cache_locations_bypass() {
    let mut fixture = Fixture::new();
    fixture.project.config.steps[0]
        .cache
        .as_mut()
        .unwrap()
        .outputs = vec!["source.txt".into()];
    assert_eq!(fixture.call().cache.unwrap().status, "BYPASS");
    fixture.project.config.steps[0]
        .cache
        .as_mut()
        .unwrap()
        .outputs
        .clear();
    fs::write(
        fixture.project.reports.join("tracked.log"),
        "tracked source",
    )
    .unwrap();
    assert!(crate::utils::git::capture(
        &fixture.project.root,
        ["add", "-f", ".harness-gate/reports/tracked.log"]
    )
    .unwrap()
    .status
    .success());
    assert_eq!(fixture.call().cache.unwrap().reason, "CACHE_INPUT_UNBOUND");
    fixture.project.step_cache_dir = Some(fixture.project.root.join("cache"));
    assert_eq!(fixture.call().cache.unwrap().reason, "CACHE_HOST_UNTRUSTED");
    assert!(!fixture.project.step_cache_dir.as_ref().unwrap().exists());
}

#[test]
fn declaration_bounds_duplicates_and_paths_fail_closed() {
    let fixture = Fixture::new();
    for case in 0..8 {
        let mut config = fixture.project.config.clone();
        let cache = config.steps[0].cache.as_mut().unwrap();
        match case {
            0 => cache.inputs = vec!["same".into(), "same".into()],
            1 => cache.outputs = vec!["same".into(), "same".into()],
            2 => {
                cache.inputs = vec!["same".into()];
                cache.outputs = vec!["same".into()];
            }
            3 => {
                config.paths.reports = "./.harness-gate/reports".into();
                cache.outputs = vec![".harness-gate/reports/out".into()];
            }
            4 => cache.inputs = (0..257).map(|i| format!("input-{i}")).collect(),
            5 => cache.toolchain.push("relative-tool".into()),
            6 => cache.toolchain.push("/tmp/../tool".into()),
            _ => cache.environment = vec!["PATH".into(), "PATH".into()],
        }
        assert!(
            crate::config::validate_step_cache(&config, &config.steps[0]).is_err(),
            "case {case}"
        );
    }
}
