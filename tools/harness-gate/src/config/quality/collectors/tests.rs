use super::*;
use crate::process::adapter::{
    AdapterCapabilities, AdapterDeclaration, AdapterSignature, TrustedKey,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::{tempdir, TempDir};

#[path = "ci_acceptance.rs"]
#[cfg(target_os = "linux")]
mod ci_acceptance;

#[path = "tests/dogfood_acceptance.rs"]
#[cfg(target_os = "linux")]
mod dogfood_acceptance;

struct Fixture {
    dir: TempDir,
    state: TrustedState,
    request: AdapterRequest,
    policy: HostPolicy,
    payload: Value,
}

fn write(path: &Path, value: &impl Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

impl Fixture {
    fn new(mode: &str, custom: bool) -> Self {
        Self::in_dir(mode, custom, tempdir().unwrap())
    }

    fn in_dir(mode: &str, custom: bool, dir: TempDir) -> Self {
        let root = dir.path();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/workflow");
        fs::create_dir(root.join(".harness-gate")).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target/evidence")).unwrap();
        fs::copy(
            fixtures.join("compiler/source.txt"),
            root.join("src/lib.rs"),
        )
        .unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat/rust-api.flow.toml"),
            root.join(DEFAULT_CONFIG_PATH),
        )
        .unwrap();
        let mut state: TrustedState = serde_json::from_str(
            &fs::read_to_string(fixtures.join("compiler/state.json")).unwrap(),
        )
        .unwrap();
        state.artifacts.clear();
        let direct: Value = serde_json::from_str(
            &fs::read_to_string(fixtures.join("compiler/direct.json")).unwrap(),
        )
        .unwrap();
        let mut record = direct["records"][0].clone();
        let mut rules = direct["policy"].clone();
        let mut quality = fs::read_to_string(fixtures.join("compiler/quality.toml")).unwrap();
        if custom {
            let metadata: Value = serde_json::from_str(
                &fs::read_to_string(fixtures.join("collectors/unknown-ecosystem.json")).unwrap(),
            )
            .unwrap();
            state.components.get_mut("app").unwrap().metadata = BTreeMap::from([(
                "ecosystem".into(),
                metadata["ecosystem"].as_str().unwrap().into(),
            )]);
            let series = state.series.get_mut("coverage").unwrap();
            let old = series.id.clone();
            series.collector.name = metadata["collector"].as_str().unwrap().into();
            series.name = metadata["series"].as_str().unwrap().into();
            series.tool.name = metadata["tool"].as_str().unwrap().into();
            series.runtime.name = "quasar-vm".into();
            let capability = &metadata[if mode == "custom-capability" {
                "capability"
            } else {
                "normalized_capability"
            }];
            let mut value = serde_json::to_value(&series).unwrap();
            value["metrics"] = json!([{"name":capability,"type":"size"}]);
            value["id"] = json!(evidence::series_id(&value).unwrap());
            *series = serde_json::from_value(value).unwrap();
            quality = quality
                .replace(&old, &series.id)
                .replace("coverage.line", capability.as_str().unwrap());
            record["capabilities"][0]["metric"] = capability.clone();
            record["metrics"][0]["name"] = capability.clone();
            record["metrics"][0]["value"] = json!({"type":"size","value":5,"unit":"bytes"});
            rules["rules"][0]["metric"] = capability.clone();
            rules["rules"][0]["limit"] = json!({"type":"size","value":10,"unit":"bytes"});
            rules["rules"][0]["operator"] = json!("le");
        }
        if mode == "crap" {
            let series = state.series.get_mut("coverage").unwrap();
            let old = series.id.clone();
            let mut value = serde_json::to_value(&series).unwrap();
            value["metrics"] = json!([{"name":"risk.crap","type":"decimal"}]);
            value["id"] = json!(evidence::series_id(&value).unwrap());
            *series = serde_json::from_value(value).unwrap();
            quality = quality
                .replace(&old, &series.id)
                .replace("bundle.size", "risk.crap");
            record["capabilities"][0]["metric"] = json!("risk.crap");
            record["metrics"][0]["name"] = json!("risk.crap");
            record["metrics"][0]["value"] = json!({"type":"decimal","value":"40"});
            quality = quality
                .replace("required = false", "required = true")
                .replace(
                "provider = { kind = \"none\" }",
                "provider = { kind = \"retained_artifact\", manifest = \"base/manifest.json\" }",
            );
            rules["rules"][0]["metric"] = json!("risk.crap");
            rules["rules"][0]["limit"] = json!({"type":"decimal","value":"30"});
            rules["rules"][0]["ratchet"] = json!({"deny_regression":true,"allow_legacy_debt":true});
            rules["rules"][0]["remediation_classes"] =
                json!(["reduce_complexity", "increase_meaningful_coverage"]);
        }
        record["series"] = serde_json::to_value(&state.series["coverage"]).unwrap();
        record["collector"] = record["series"]["collector"].clone();
        match mode {
            "unsupported" | "not_collected" | "measurement_error" | "not_configured"
            | "not_applicable" => {
                record["capabilities"][0]["state"] = json!(mode);
                record["metrics"] = json!([]);
                record["status"] = json!(if mode == "measurement_error" {
                    "measurement_error"
                } else {
                    "unavailable"
                });
            }
            "dishonest" => record["capabilities"][0]["state"] = json!("unsupported"),
            "missing-capability" => record["capabilities"] = json!([]),
            "wrong-tool" => record["series"]["tool"]["version"] = json!("wrong"),
            "stale-context" => record["context"]["run"] = json!("stale"),
            "wrong-subject" => record["subject"]["id"] = json!("unknown"),
            "malformed-evidence" => {
                record["metrics"][0]["value"] = json!({"type":"ratio","covered":9,"total":1})
            }
            "artifact-identity" => record["artifacts"][0]["context"]["run"] = json!("other"),
            _ => {}
        }
        if mode == "custom-capability" {
            let mut config: QualityConfig = toml::from_str(&quality).unwrap();
            config
                .policies
                .get_mut("coverage")
                .unwrap()
                .expectation
                .capability = "bundle.size".into();
            rules["rules"][0]["metric"] = json!("bundle.size");
            rules["rules"][0]["required"] = json!(false);
            quality = toml::to_string(&config).unwrap();
        }
        fs::write(root.join(".harness-gate/quality.toml"), quality).unwrap();
        write(&root.join(".harness-gate/policy.json"), &rules);
        write(
            &root.join(".harness-gate/coverage-request.json"),
            &json!({}),
        );
        let mut response = json!({"schema_version":"1", "status":"PASS", "invocation_id":state.expected.run,
            "artifacts":record["artifacts"], "collection":{"schema":"harness-project-collector-response/v1", "evidence":[record.clone()], "error":null}});
        match mode {
            "duplicate-evidence" => {
                response["collection"]["evidence"] = json!([record.clone(), record])
            }
            "approval" => response["collection"]["approved"] = json!(true),
            "missing-evidence" => response["collection"]["evidence"] = json!([]),
            "response-version" => {
                response["collection"]["schema"] = json!("harness-project-collector-response/v2")
            }
            "transport-fail" => response["status"] = json!("FAIL"),
            "measurement-fail" => {
                response["collection"]["error"] = json!({"code":"measurement_error"})
            }
            "missing-inventory" => response["artifacts"] = json!([]),
            _ => {}
        }
        let payload = json!({"mode":mode,"response":response,"raw":fs::read_to_string(fixtures.join("compiler/artifact.txt")).unwrap()});
        let script = fs::read_to_string(fixtures.join("collectors/collector.py"))
            .unwrap()
            .replace(
                "'PAYLOAD_PLACEHOLDER'",
                &serde_json::to_string(&serde_json::to_string(&payload).unwrap()).unwrap(),
            );
        let executable = root.join("collector.py");
        fs::write(&executable, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let request = AdapterRequest {
            protocol_version: 2,
            result_schema_version: "1".into(),
            adapter: AdapterDeclaration {
                name: state.series["coverage"].collector.name.clone(),
                version: "1".into(),
                executable: executable.clone(),
                source_digest: format!("{:x}", Sha256::digest(fs::read(executable).unwrap())),
                signature: AdapterSignature {
                    algorithm: "ed25519".into(),
                    key_id: "fixture".into(),
                    value: String::new(),
                },
            },
            invocation_id: state.expected.run.clone(),
            step_id: "coverage".into(),
            // Allow interpreter startup under hosted runner contention.
            timeout_ms: if mode == "timeout" { 50 } else { 30000 },
            config_digest: String::new(),
            artifact_root: root.join("target/evidence"),
            nonce: "fixture-nonce".into(),
            issued_at_ms: now,
            expires_at_ms: now + 60000,
            args: vec![],
            environment: BTreeMap::new(),
            capabilities: AdapterCapabilities::default(),
            input: Value::Null,
        };
        let policy = HostPolicy {
            trusted_keys: vec![TrustedKey {
                key_id: "fixture".into(),
                public_key: BASE64
                    .encode(SigningKey::from_bytes(&[7; 32]).verifying_key().as_bytes()),
            }],
            max_timeout: Some(Duration::from_secs(30)),
            ..HostPolicy::default()
        };
        let mut fixture = Self {
            dir,
            state,
            request,
            policy,
            payload,
        };
        fixture.pin();
        let mut config = fixture.config();
        let id = if custom {
            let metadata: Value = serde_json::from_str(
                &fs::read_to_string(fixtures.join("collectors/unknown-ecosystem.json")).unwrap(),
            )
            .unwrap();
            metadata["binding"].as_str().unwrap().to_owned()
        } else {
            "coverage".into()
        };
        if custom {
            let binding = config.collectors.remove("coverage").unwrap();
            config.collectors.insert(id.clone(), binding);
            for profile in config.profiles.values_mut() {
                if profile.collectors.remove("coverage") {
                    profile.collectors.insert(id.clone());
                }
            }
            let series = fixture.state.series.remove("coverage").unwrap();
            fixture.state.series.insert(id.clone(), series);
            fs::write(
                fixture.dir.path().join(".harness-gate/quality.toml"),
                toml::to_string(&config).unwrap(),
            )
            .unwrap();
            fixture.pin();
        }
        let inputs = compiler::compile(fixture.dir.path(), &fixture.state).unwrap();
        // Bind the canonical root even when the temp directory uses a symlink
        // (for example, /var -> /private/var on macOS).
        fixture.request.artifact_root = inputs.artifact_root.clone();
        fixture.request.step_id = id.clone();
        fixture.request.config_digest = binding_digest(&config, &fixture.state, &inputs).unwrap();
        fixture.request.input = input(
            &inputs,
            &fixture.state,
            &id,
            &claims(&config, &fixture.state, &id).unwrap(),
        );
        fixture.sign();
        fixture
    }

    fn config(&self) -> QualityConfig {
        toml::from_str(
            &fs::read_to_string(self.dir.path().join(".harness-gate/quality.toml")).unwrap(),
        )
        .unwrap()
    }

    fn pin(&mut self) {
        let mut files: BTreeSet<String> = [DEFAULT_CONFIG_PATH, ".harness-gate/quality.toml"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        files.extend(self.config().collectors.values().map(|c| c.request.clone()));
        files.extend(
            self.config()
                .policies
                .values()
                .map(|p| p.policy_file.clone()),
        );
        self.state.config_files = files
            .into_iter()
            .map(|p| {
                (
                    p.clone(),
                    format!(
                        "{:x}",
                        Sha256::digest(fs::read(self.dir.path().join(p)).unwrap())
                    ),
                )
            })
            .collect();
    }

    fn sign(&mut self) {
        self.request.adapter.signature.value = BASE64.encode(
            SigningKey::from_bytes(&[7; 32])
                .sign(&adapter::signing_payload(&self.request).unwrap())
                .to_bytes(),
        );
        self.save();
    }

    fn save(&mut self) {
        write(
            &self.dir.path().join(".harness-gate/coverage-request.json"),
            &self.request,
        );
        self.pin();
    }

    fn collect(&self) -> Result<Collection> {
        collect(self.dir.path(), &self.state, &self.policy)
    }
}

#[test]
fn configured_multiple_producers_combine_distinct_series_and_reject_overlap() {
    for overlap in [false, true] {
        let mut fixture = Fixture::new("pass", false);
        let root = fixture.dir.path().to_path_buf();
        let mut config = fixture.config();
        let mut series = serde_json::to_value(&fixture.state.series["coverage"]).unwrap();
        series["name"] = json!("independent-measurement");
        if !overlap {
            series["metrics"] = json!([{"name":"bundle.size","type":"size"}]);
        }
        series["id"] = json!(evidence::series_id(&series).unwrap());
        fixture.state.series.insert(
            "second".into(),
            serde_json::from_value(series.clone()).unwrap(),
        );
        let mut binding = config.collectors["coverage"].clone();
        binding.request = ".harness-gate/second-request.json".into();
        binding.produces[0].series = series["id"].as_str().unwrap().into();
        if overlap {
            // Distinct configured targets still resolve to the same subject.
            binding.produces[0].target = super::super::model::Target::Subject {
                id: "module".into(),
            };
        } else {
            binding.produces[0].capability = "bundle.size".into();
        }
        config.collectors.insert("second".into(), binding);
        config
            .profiles
            .get_mut(&fixture.state.profile)
            .unwrap()
            .collectors
            .insert("second".into());
        fs::write(
            root.join(".harness-gate/quality.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        write(&root.join(".harness-gate/second-request.json"), &json!({}));
        fixture.pin();
        let mut payload = fixture.payload.clone();
        let record = &mut payload["response"]["collection"]["evidence"][0];
        record["series"] = series;
        record["id"] = json!("second-evidence");
        record["artifacts"][0]["path"] = json!("second.json");
        if !overlap {
            record["capabilities"][0]["metric"] = json!("bundle.size");
            record["metrics"][0]["name"] = json!("bundle.size");
            record["metrics"][0]["value"] = json!({"type":"size","value":5,"unit":"bytes"});
        }
        payload["response"]["artifacts"] = record["artifacts"].clone();
        let template =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/workflow/collectors/collector.py");
        let script = fs::read_to_string(template)
            .unwrap()
            .replace("raw.json", "second.json")
            .replace(
                "'PAYLOAD_PLACEHOLDER'",
                &serde_json::to_string(&serde_json::to_string(&payload).unwrap()).unwrap(),
            );
        let executable = root.join("second.py");
        fs::write(&executable, &script).unwrap();
        fs::set_permissions(
            &executable,
            fs::metadata(&fixture.request.adapter.executable)
                .unwrap()
                .permissions(),
        )
        .unwrap();
        let inputs = compiler::compile(&root, &fixture.state).unwrap();
        fixture.request.config_digest = binding_digest(&config, &fixture.state, &inputs).unwrap();
        let mut second = fixture.request.clone();
        second.step_id = "second".into();
        second.nonce = "second-nonce".into();
        second.adapter.executable = executable;
        second.adapter.source_digest = format!("{:x}", Sha256::digest(script.as_bytes()));
        second.input = input(
            &inputs,
            &fixture.state,
            "second",
            &claims(&config, &fixture.state, "second").unwrap(),
        );
        second.adapter.signature.value = BASE64.encode(
            SigningKey::from_bytes(&[7; 32])
                .sign(&adapter::signing_payload(&second).unwrap())
                .to_bytes(),
        );
        write(&root.join(".harness-gate/second-request.json"), &second);
        fixture.sign();
        let result = fixture.collect();
        if overlap {
            assert!(result.is_err());
            assert!(
                !fixture.request.artifact_root.join("raw.json").exists(),
                "duplicate ownership must fail before launch"
            );
        } else {
            let collection = result.unwrap();
            assert_eq!(collection.evidence.as_array().unwrap().len(), 2);
            assert!(fixture.request.artifact_root.join("second.json").is_file());
            // Reuse one authoritative producer and collect only the missing one.
            fs::remove_file(fixture.request.artifact_root.join("second.json")).unwrap();
            fixture.state.artifacts = inventory(&fixture.request.artifact_root).unwrap();
            let path = ".harness-gate/retained-coverage.json";
            write(&root.join(path), &collection.responses["coverage"]);
            fixture.state.retained.insert(
                "coverage".into(),
                compiler::RetainedEvidence {
                    path: path.into(),
                    sha256: format!("{:x}", Sha256::digest(fs::read(root.join(path)).unwrap())),
                },
            );
            second.nonce = "mixed-fresh-producer".into();
            second.adapter.signature.value = BASE64.encode(
                SigningKey::from_bytes(&[7; 32])
                    .sign(&adapter::signing_payload(&second).unwrap())
                    .to_bytes(),
            );
            write(&root.join(".harness-gate/second-request.json"), &second);
            fixture.pin();
            let mixed = fixture.collect().unwrap();
            assert_eq!(mixed.evidence, collection.evidence);
            assert_eq!(mixed.producers["coverage"], "retained");
            assert_eq!(mixed.producers["second"], "collected");
        }
    }
}

#[test]
#[cfg(unix)]
fn configured_collector_launches_from_a_symlinked_workspace_root() {
    let parent = tempdir().unwrap();
    let real = parent.path().join("real");
    let alias = parent.path().join("alias");
    fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let dir = tempfile::tempdir_in(&alias).unwrap();
    assert_ne!(dir.path(), dir.path().canonicalize().unwrap());
    let fixture = Fixture::in_dir("pass", true, dir);
    let collection = fixture.collect().unwrap();
    assert_eq!(collection.evidence.as_array().unwrap().len(), 1);
    assert!(fixture.request.artifact_root.join("raw.json").is_file());
}

#[test]
#[cfg(unix)]
fn configured_ecosystems_launch_through_same_boundary_and_preserve_capability_states() {
    for custom in [false, true] {
        for state in [
            "pass",
            "unsupported",
            "not_collected",
            "measurement_error",
            "not_configured",
            "not_applicable",
        ] {
            let fixture = Fixture::new(state, custom);
            let collection = fixture
                .collect()
                .unwrap_or_else(|e| panic!("{custom}/{state}: {e:#}"));
            assert_eq!(
                collection.evidence[0]["capabilities"][0]["state"],
                if state == "pass" { "supported" } else { state }
            );
            assert!(collection.evidence[0]["metrics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["name"] != "risk.crap"));
        }
    }
    // Arbitrary capability IDs are resolved and launched without core language
    // dispatch; the released evidence validator still owns metric certification.
    let fixture = Fixture::new("custom-capability", true);
    let error = fixture.collect().unwrap_err();
    assert!(format!("{error:#}").contains("unknown generic metric: quasar.payload_bytes"));
    assert!(fixture.request.artifact_root.join("raw.json").is_file());
}

#[test]
#[cfg(unix)]
fn collector_protocol_and_evidence_fail_closed() {
    for mode in [
        "crash",
        "timeout",
        "malformed",
        "duplicate-json",
        "tamper-artifact",
        "extra-artifact",
        "source-change",
        "config-change",
        "symlink",
        "dishonest",
        "missing-capability",
        "wrong-tool",
        "stale-context",
        "wrong-subject",
        "malformed-evidence",
        "artifact-identity",
        "duplicate-evidence",
        "approval",
        "missing-evidence",
        "response-version",
        "transport-fail",
        "measurement-fail",
        "missing-inventory",
    ] {
        let fixture = Fixture::new(mode, false);
        assert!(fixture.collect().is_err(), "{mode} must fail");
    }
}

#[test]
#[cfg(unix)]
fn collector_signed_binding_failures() {
    for mode in [
        "duplicate-request",
        "signature",
        "protocol",
        "package",
        "config",
        "selection",
        "roots",
        "executable",
        "launch",
        "timeout-limit",
        "output-limit",
        "artifact-limit",
        "duplicate-producer",
        "stale-root",
    ] {
        let mut fixture = Fixture::new("pass", false);
        match mode {
            "duplicate-request" => {}
            "signature" => fixture.request.adapter.signature.value = BASE64.encode([0; 64]),
            "protocol" => fixture.request.protocol_version = 1,
            "package" => fixture.request.adapter.name = "other".into(),
            "config" => fixture.request.config_digest = "0".repeat(64),
            "selection" => fixture.request.input["bindings"] = json!([]),
            "roots" => fixture.request.artifact_root = fixture.dir.path().to_owned(),
            "executable" => fs::write(&fixture.request.adapter.executable, "changed").unwrap(),
            "launch" => {
                fs::write(
                    &fixture.request.adapter.executable,
                    "#!/missing/interpreter\n",
                )
                .unwrap();
                fixture.request.adapter.source_digest = format!(
                    "{:x}",
                    Sha256::digest(fs::read(&fixture.request.adapter.executable).unwrap())
                );
            }
            "timeout-limit" => fixture.policy.max_timeout = Some(Duration::from_millis(1)),
            "output-limit" => fixture.policy.max_stdout_bytes = 10,
            "artifact-limit" => fixture.policy.max_artifact_bytes = Some(1),
            "duplicate-producer" => {
                let mut config = fixture.config();
                let duplicate = config.collectors["coverage"].produces[0].clone();
                config
                    .collectors
                    .get_mut("coverage")
                    .unwrap()
                    .produces
                    .push(duplicate);
                fs::write(
                    fixture.dir.path().join(".harness-gate/quality.toml"),
                    toml::to_string(&config).unwrap(),
                )
                .unwrap();
            }
            "stale-root" => {
                fs::write(fixture.request.artifact_root.join("stale"), "stale").unwrap()
            }
            _ => unreachable!(),
        }
        if mode == "signature" {
            fixture.save();
        } else {
            fixture.sign();
        }
        if mode == "duplicate-request" {
            let path = fixture
                .dir
                .path()
                .join(".harness-gate/coverage-request.json");
            let text =
                fs::read_to_string(&path)
                    .unwrap()
                    .replacen('{', "{\"protocol_version\":2,", 1);
            fs::write(path, text).unwrap();
            fixture.pin();
        }
        assert!(fixture.collect().is_err(), "{mode} must fail");
    }
}

#[test]
fn generic_orchestration_has_no_closed_language_dispatch() {
    // Block language/framework enums, metadata inspection and named ecosystem
    // cases in the generic path. Runtime coverage above proves arbitrary IDs.
    for forbidden in [
        "enum ",
        "match ",
        ".metadata",
        "Ecosystem",
        "Language",
        "Framework",
    ] {
        let code = include_str!("../collectors.rs")
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(forbidden),
            "collector dispatch guard: {forbidden}"
        );
    }
    for source in [
        include_str!("../collectors.rs"),
        include_str!("../compiler.rs"),
    ] {
        let code = source
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "enum Language",
            "enum Ecosystem",
            "enum Framework",
            "Language::",
            "Ecosystem::",
            "Framework::",
            "\"rust\"",
            "\"angular\"",
            "\"go\"",
            "\"vue\"",
            "[\"ecosystem\"]",
            "[\"language\"]",
            "get(\"ecosystem\")",
            "get(\"language\")",
        ] {
            assert!(
                !code.contains(forbidden),
                "generic dispatch guard: {forbidden}"
            );
        }
    }
}

#[test]
fn collection_cli_publishes_measurements_and_removes_stale_output_on_failure() {
    use clap::Parser;
    #[derive(Parser)]
    struct Command {
        #[command(subcommand)]
        action: crate::app::quality::QualityAction,
    }
    for mode in ["pass", "malformed"] {
        let fixture = Fixture::new(mode, true);
        let root = fixture.dir.path();
        let state = root.join("state.json");
        let keys = root.join("keys.json");
        let output = root.join("collection.json");
        write(&state, &fixture.state);
        write(
            &keys,
            &json!([{"key_id":fixture.policy.trusted_keys[0].key_id,"public_key":fixture.policy.trusted_keys[0].public_key}]),
        );
        fs::write(&output, "stale success").unwrap();
        let command = Command::try_parse_from([
            "quality",
            "collect",
            "--repository-root",
            root.to_str().unwrap(),
            "--state",
            state.to_str().unwrap(),
            "--trusted-keys",
            keys.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .unwrap();
        let result = crate::app::quality::run(&command.action, None);
        if mode == "pass" {
            assert!(result.unwrap());
            let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
            assert_eq!(value["schema"], "quality-collection/v1");
            assert!(value.get("approval").is_none());
            assert!(root.join(".harness-gate/collector-nonces").is_dir());
        } else {
            assert!(result.is_err());
            assert!(!output.exists());
        }
    }
}

// Full workflow fixtures deliberately use a pack identifier unknown to the host.
impl Fixture {
    fn workflow(mode: &str, execution_passes: bool, retained_base: bool) -> Self {
        let mut fixture = Self::new(mode, true);
        let root = fixture.dir.path().to_path_buf();
        let mut flow: crate::config::FlowConfig =
            toml::from_str(&fs::read_to_string(root.join(DEFAULT_CONFIG_PATH)).unwrap()).unwrap();
        for name in ["empty.audit.toml", "default.secrets.toml"] {
            let dest = if name.starts_with("empty") {
                "audit.toml"
            } else {
                "secrets.toml"
            };
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("presets")
                    .join(name),
                root.join(".harness-gate").join(dest),
            )
            .unwrap();
        }
        flow.steps.truncate(1);
        let step = &mut flow.steps[0];
        step.id = "configured-check".into();
        step.label = "Configured execution check".into();
        step.program = "python3".into();
        step.args = vec!["{root}/execution.py".into()];
        fs::write(
            root.join("execution.py"),
            if execution_passes {
                "raise SystemExit(0)"
            } else {
                "raise SystemExit(7)"
            },
        )
        .unwrap();
        step.log = "execution.log".into();
        flow.policy.required_steps = vec![step.id.clone()].into_iter().collect();
        fs::write(
            root.join(DEFAULT_CONFIG_PATH),
            toml::to_string(&flow).unwrap(),
        )
        .unwrap();
        let mut config = fixture.config();
        config.profiles.get_mut("full").unwrap().workflow = Some(super::super::model::Workflow {
            state: ".harness-gate/workflow-state.json".into(),
            trusted_keys: ".harness-gate/workflow-keys.json".into(),
            baseline_request: retained_base.then(|| ".harness-gate/base-request.json".into()),
        });
        if retained_base {
            config.baseline.required = true;
            config.baseline.provider = super::super::model::BaselineProvider::RetainedArtifact {
                manifest: "base/manifest.json".into(),
            };
        }
        fs::write(
            root.join(".harness-gate/quality.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        fixture.pin();
        let inputs = compiler::compile(&root, &fixture.state).unwrap();
        let id = fixture.request.step_id.clone();
        fixture.request.config_digest = binding_digest(&config, &fixture.state, &inputs).unwrap();
        fixture.request.input = input(
            &inputs,
            &fixture.state,
            &id,
            &claims(&config, &fixture.state, &id).unwrap(),
        );
        fixture.sign();
        write(
            &root.join(".harness-gate/workflow-state.json"),
            &fixture.state,
        );
        write(
            &root.join(".harness-gate/workflow-keys.json"),
            &json!([{
                "key_id":fixture.policy.trusted_keys[0].key_id,
                "public_key":fixture.policy.trusted_keys[0].public_key
            }]),
        );
        if retained_base {
            fixture.retained_base();
        }
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "fixture",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(&root)
                .status()
                .unwrap()
                .success());
        }
        fixture
    }

    fn retained_base(&self) {
        let root = self.dir.path();
        let base = root.join("base");
        fs::create_dir(&base).unwrap();
        let mut state = self.state.clone();
        state.expected.commit = state.expected.base_commit.clone();
        state.expected.base_commit = "2222222222222222222222222222222222222222".into();
        state.expected.run = "retained-base".into();
        let raw = self.payload["raw"].as_str().unwrap();
        state.artifacts.insert(
            "raw.json".into(),
            format!("{:x}", Sha256::digest(raw.as_bytes())),
        );
        let mut files = state.config_files.clone();
        for subject in state.subjects.values().flatten() {
            files.insert(subject.path.clone(), subject.source_sha256.clone());
        }
        for name in files.keys() {
            let path = base.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::copy(root.join(name), path).unwrap();
        }
        fs::create_dir_all(base.join(&state.artifact_root)).unwrap();
        fs::write(base.join(&state.artifact_root).join("raw.json"), raw).unwrap();
        files.insert(
            format!("{}/raw.json", state.artifact_root),
            state.artifacts["raw.json"].clone(),
        );
        let mut records = self.payload["response"]["collection"]["evidence"].clone();
        for record in records.as_array_mut().unwrap() {
            record["context"] = serde_json::to_value(&state.expected).unwrap();
            if record["metrics"][0]["name"] == "risk.crap" {
                record["metrics"][0]["value"]["value"] = json!("35");
            }
            for artifact in record["artifacts"].as_array_mut().unwrap() {
                artifact["context"] = serde_json::to_value(&state.expected).unwrap();
            }
        }
        let manifest = json!({"schema":"quality-baseline-manifest/v1","state":state,"evidence":records,"files":files});
        write(&base.join("manifest.json"), &manifest);
        write(
            &root.join(".harness-gate/base-request.json"),
            &json!({
                "schema":"quality-baseline-request/v1", "state":state,
                "manifest":"base/manifest.json", "manifest_sha256":format!("{:x}",Sha256::digest(fs::read(base.join("manifest.json")).unwrap()))
            }),
        );
    }

    fn verify(&self) -> crate::verify::VerificationReport {
        let project =
            crate::project::Project::discover(Some(self.dir.path().to_path_buf()), None).unwrap();
        crate::verify::run(
            &project,
            crate::scope::ScopeResult::all(&project),
            "full",
            false,
        )
        .unwrap()
    }
}

fn direct_equivalent(quality: &Value, root: &Path) {
    use clap::Parser;
    #[derive(Parser)]
    struct Command {
        #[command(subcommand)]
        action: crate::app::quality::QualityAction,
    }
    let mut args = vec!["quality".to_owned(), "evaluate".to_owned()];
    for (flag, value) in [
        ("project", &quality["inputs"]["project"]),
        ("policy", &quality["inputs"]["policy"]),
        ("expected", &quality["inputs"]["expected"]),
        ("evidence", &quality["evidence"]),
        ("selection", &quality["inputs"]["selection"]),
        ("mappings", &quality["inputs"]["mappings"]),
        ("exceptions", &quality["inputs"]["exceptions"]),
        ("base-project", &quality["baseline"]["inputs"]["project"]),
        ("base-expected", &quality["baseline"]["inputs"]["expected"]),
        ("base-evidence", &quality["baseline"]["evidence"]),
    ] {
        if value.is_null() {
            continue;
        }
        let path = root.join(format!("direct-{flag}.json"));
        write(&path, value);
        args.extend([format!("--{flag}"), path.to_str().unwrap().into()]);
    }
    for (flag, value) in [
        ("source-root", &quality["inputs"]["source_root"]),
        ("artifact-root", &quality["inputs"]["artifact_root"]),
        (
            "base-source-root",
            &quality["baseline"]["inputs"]["source_root"],
        ),
        (
            "base-artifact-root",
            &quality["baseline"]["inputs"]["artifact_root"],
        ),
        ("now", &quality["evaluation_time"]),
    ] {
        if let Some(value) = value.as_str() {
            args.extend([format!("--{flag}"), value.into()]);
        }
    }
    let output = root.join("direct-report.json");
    args.extend(["--output".into(), output.to_str().unwrap().into()]);
    let command = Command::try_parse_from(args).unwrap();
    assert_eq!(
        crate::app::quality::run(&command.action, None).unwrap(),
        quality["status"] == "pass"
    );
    let direct: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(direct, quality["project_report"]);
}

#[test]
fn unknown_ecosystem_verify_baseline_and_direct_evaluator_are_equivalent() {
    let fixture = Fixture::workflow("pass", true, true);
    let report = fixture.verify();
    let unified: Value = serde_json::from_slice(
        &fs::read(Path::new(&report.report_directory).join("test_result.json")).unwrap(),
    )
    .unwrap();
    assert!(report.passed, "{unified:#}");
    assert_eq!(unified["status"], "PASS");
    assert_eq!(unified["evidence_complete"], true);
    let quality = &unified["quality"];
    assert_eq!(quality["baseline"]["status"], "available");
    assert_eq!(
        quality["inputs"]["project"]["components"][0]["metadata"]["ecosystem"],
        "nebula-unregistered-2049"
    );
    assert_eq!(quality["selection"]["changed_subject"], json!(["module"]));
    assert_eq!(
        quality["project_report"]["components"]["app"]["aggregate"]["state"],
        "pass"
    );
    let authoritative: Value = serde_json::from_slice(
        &fs::read(quality["project_report_path"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(authoritative, quality["project_report"]);
    let mirrored: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .dir
                .path()
                .join("target/quality")
                .join(&report.invocation_id)
                .join("test_result.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(mirrored, unified);
    direct_equivalent(quality, fixture.dir.path());
    fs::remove_dir_all(quality["baseline"]["retained_directory"].as_str().unwrap()).unwrap();
}

#[test]
fn workflow_execution_and_quality_failures_have_independent_authority() {
    for execution in [true, false] {
        for mode in ["pass", "unsupported"] {
            let fixture = Fixture::workflow(mode, execution, false);
            let report = fixture.verify();
            let value = serde_json::to_value(&report).unwrap();
            assert_eq!(report.passed, execution && mode == "pass", "{value:#}");
            assert_eq!(
                value["quality"]["status"],
                if mode == "pass" { "pass" } else { "fail" }
            );
            assert_eq!(
                value["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|step| step["step_id"] == "configured-check")
                    .unwrap()["passed"],
                execution
            );
            direct_equivalent(&value["quality"], fixture.dir.path());
        }
    }
}

#[test]
fn workflow_selection_follows_changed_files_and_component_scope() {
    for mode in ["changed", "components"] {
        let fixture = Fixture::workflow("pass", true, false);
        let project =
            crate::project::Project::discover(Some(fixture.dir.path().to_path_buf()), None)
                .unwrap();
        let mut scope = crate::scope::ScopeResult::all(&project);
        scope.mode = mode.into();
        scope.changed_files = vec!["src/lib.rs".into()];
        let report = crate::verify::run(&project, scope, "full", false).unwrap();
        assert!(report.passed, "{report:#?}");
        direct_equivalent(
            &serde_json::to_value(&report).unwrap()["quality"],
            fixture.dir.path(),
        );
    }
}

#[test]
fn workflow_invalid_evidence_selection_and_baseline_block_successful_execution() {
    for mode in [
        "malformed",
        "selection",
        "baseline",
        "state",
        "config-change",
    ] {
        let mut fixture = Fixture::workflow(
            if mode == "malformed" || mode == "config-change" {
                mode
            } else {
                "pass"
            },
            true,
            mode == "baseline",
        );
        let root = fixture.dir.path();
        match mode {
            "selection" => {
                fixture
                    .state
                    .selection
                    .as_mut()
                    .unwrap()
                    .get_mut("changed_subject")
                    .unwrap()
                    .clear();
                write(
                    &root.join(".harness-gate/workflow-state.json"),
                    &fixture.state,
                );
            }
            "state" => {
                fs::write(root.join(".harness-gate/workflow-state.json"), "{}").unwrap();
            }
            "baseline" => {
                fs::write(root.join("base/manifest.json"), "{}").unwrap();
            }
            _ => {}
        }
        let report = fixture.verify();
        let value = serde_json::to_value(&report).unwrap();
        assert!(!report.passed, "{mode}: {value:#}");
        assert_eq!(value["steps"][0]["passed"], true);
        assert_eq!(value["quality"]["status"], "blocked");
        assert!(value["quality"]["error"].is_string());
    }
}

#[test]
fn workflow_quality_success_cannot_override_audit_failure() {
    let fixture = Fixture::workflow("pass", true, false);
    let mut project =
        crate::project::Project::discover(Some(fixture.dir.path().to_path_buf()), None).unwrap();
    project.audit_config = fixture.dir.path().join(".harness-gate");
    let error = crate::verify::run(
        &project,
        crate::scope::ScopeResult::all(&project),
        "full",
        false,
    )
    .unwrap_err();
    assert!(matches!(error, crate::verify::VerifyError::Audit(_)));
    let report: Value =
        serde_json::from_slice(&fs::read(project.reports.join("test_result.json")).unwrap())
            .unwrap();
    assert_eq!(report["passed"], false);
    assert_eq!(report["quality"]["status"], "pass");
    assert!(report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| step["passed"] == false && step["label"] == "architecture audit"));
}

#[test]
fn workflow_crap_diagnostics_preserve_base_head_ratchet_evidence_and_remediation() {
    let fixture = Fixture::workflow("crap", true, true);
    let report = fixture.verify();
    assert!(!report.passed);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["quality"]["status"], "fail");
    let human =
        fs::read_to_string(Path::new(&report.report_directory).join("test_result.md")).unwrap();
    for expected in [
        "risk.crap",
        "src/lib.rs",
        "base=",
        "35",
        "head=",
        "40",
        "threshold=",
        "30",
        "ratchet=",
        "deny_regression",
        "raw.json",
        "reduce_complexity",
        "increase_meaningful_coverage",
    ] {
        assert!(human.contains(expected), "missing {expected}: {human}");
    }
    direct_equivalent(&value["quality"], fixture.dir.path());
    fs::remove_dir_all(
        value["quality"]["baseline"]["retained_directory"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
}

impl Fixture {
    fn select_profile(&mut self, profile: &str, omit: bool) {
        let root = self.dir.path().to_path_buf();
        let mut flow: FlowConfig =
            toml::from_str(&fs::read_to_string(root.join(DEFAULT_CONFIG_PATH)).unwrap()).unwrap();
        flow.steps[0].profiles.insert(profile.into());
        fs::write(
            root.join(DEFAULT_CONFIG_PATH),
            toml::to_string(&flow).unwrap(),
        )
        .unwrap();
        let mut config = self.config();
        let mut participation = config.profiles["full"].clone();
        if omit {
            participation.assurance = super::super::Assurance::Partial;
            participation.collectors.clear();
            participation.policies.clear();
            self.state.series.clear();
        }
        config.profiles.insert(profile.into(), participation);
        self.state.profile = profile.into();
        fs::write(
            root.join(".harness-gate/quality.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        self.pin();
        if !omit {
            let inputs = compiler::compile(&root, &self.state).unwrap();
            self.request.config_digest = binding_digest(&config, &self.state, &inputs).unwrap();
            self.sign();
        } else {
            self.state
                .config_files
                .remove(".harness-gate/coverage-request.json");
        }
        write(&root.join(".harness-gate/workflow-state.json"), &self.state);
        if root.join("base").exists() {
            fs::remove_dir_all(root.join("base")).unwrap();
            self.retained_base();
        }
    }

    fn retain(&mut self) -> Collection {
        let collection = self.collect().unwrap();
        self.state.artifacts = inventory(&self.request.artifact_root).unwrap();
        for (id, response) in &collection.responses {
            let path = format!(".harness-gate/retained-{id}.json");
            write(&self.dir.path().join(&path), response);
            self.state.retained.insert(
                id.clone(),
                compiler::RetainedEvidence {
                    sha256: format!(
                        "{:x}",
                        Sha256::digest(fs::read(self.dir.path().join(&path)).unwrap())
                    ),
                    path,
                },
            );
        }
        write(
            &self.dir.path().join(".harness-gate/workflow-state.json"),
            &self.state,
        );
        collection
    }

    fn quality_result(&self) -> Value {
        let project =
            crate::project::Project::discover(Some(self.dir.path().to_path_buf()), None).unwrap();
        let report = crate::verify::run(
            &project,
            crate::scope::ScopeResult::all(&project),
            &self.state.profile,
            false,
        )
        .unwrap();
        let unified: Value = serde_json::from_slice(
            &fs::read(Path::new(&report.report_directory).join("test_result.json")).unwrap(),
        )
        .unwrap();
        unified["quality"].clone()
    }
}

#[test]
fn partial_profiles_report_omissions_and_never_full_quality_pass() {
    for profile in ["hook", "custom-fast"] {
        let mut fixture = Fixture::workflow("crash", true, false);
        fixture.select_profile(profile, true);
        let quality = fixture.quality_result();
        assert_eq!(quality["status"], "not_collected", "{quality:#}");
        assert_eq!(quality["full_quality_status"], "not_collected");
        assert_eq!(
            quality["participation"]["policies"]["coverage"]["state"],
            "not_collected"
        );
        assert_eq!(quality["evidence"], json!([]));
        assert_eq!(quality["producers"], json!({}));
        assert!(!fixture.request.artifact_root.join("raw.json").exists());
    }
}

#[test]
fn partial_profiles_evaluate_selected_policy_without_certifying_full_quality() {
    for mode in ["pass", "crap"] {
        let mut fixture = Fixture::workflow(mode, true, mode == "crap");
        let mut config = fixture.config();
        config.profiles.get_mut("full").unwrap().assurance = super::super::Assurance::Partial;
        fs::write(
            fixture.dir.path().join(".harness-gate/quality.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        fixture.select_profile("custom-fast", false);
        let quality = fixture.quality_result();
        assert_eq!(
            quality["status"],
            if mode == "pass" { "pass" } else { "fail" },
            "{quality:#}"
        );
        assert_eq!(quality["full_quality_status"], "not_collected");
        assert_eq!(quality["participation"]["assurance"], "partial");
    }
}

#[test]
fn complete_profiles_enforce_crap_and_preserve_unsupported_capabilities() {
    for profile in ["full", "ci"] {
        for mode in ["crap", "unsupported", "not_collected"] {
            let mut fixture = Fixture::workflow(mode, true, mode == "crap");
            fixture.select_profile(profile, false);
            let quality = fixture.quality_result();
            assert_eq!(quality["status"], "fail", "{profile}/{mode}: {quality:#}");
            assert_eq!(quality["full_quality_status"], "fail");
            assert_eq!(
                quality["participation"]["policies"]["coverage"]["state"],
                "participating"
            );
            if mode != "crap" {
                assert_eq!(quality["evidence"][0]["metrics"], json!([]));
                assert_eq!(quality["evidence"][0]["capabilities"][0]["state"], mode);
            }
        }
    }
}

#[test]
fn ci_reuses_authoritative_responses_without_relaunching_or_changing_decisions() {
    for mode in ["pass", "unsupported", "crap"] {
        let mut fixture = Fixture::workflow(mode, true, mode == "crap");
        fixture.select_profile("ci", false);
        let original = fixture.retain();
        fs::remove_file(&fixture.request.adapter.executable).unwrap();
        let reused = fixture.collect().unwrap();
        assert_eq!(original.evidence, reused.evidence);
        assert_eq!(original.responses, reused.responses);
        assert_eq!(reused.producers["stargazer"], "retained");
        assert_eq!(
            original.runtime_bindings["stargazer"]["execution"],
            "collected"
        );
        assert_eq!(
            reused.runtime_bindings["stargazer"]["execution"],
            "retained"
        );
        assert_eq!(
            original.runtime_bindings["stargazer"]["settings_digest"],
            reused.runtime_bindings["stargazer"]["settings_digest"]
        );
        let quality = fixture.quality_result();
        assert_eq!(
            quality["status"],
            if mode == "pass" { "pass" } else { "fail" },
            "{quality:#}"
        );
        assert_eq!(quality["producers"]["stargazer"], "retained");
        assert_eq!(
            quality["runtime"]["facts"]["collectors"]["stargazer"]["execution"],
            "retained"
        );
        assert_eq!(
            quality["runtime"]["facts"]["collector_producers"]["stargazer"],
            "retained"
        );
    }
}

#[test]
fn machine_identities_bind_policy_runtime_and_selected_series() {
    let mut fixture = Fixture::workflow("pass", true, false);
    fixture.select_profile("full", false);
    let project =
        crate::project::Project::discover(Some(fixture.dir.path().to_path_buf()), None).unwrap();
    let report = crate::verify::run(
        &project,
        crate::scope::ScopeResult::all(&project),
        &fixture.state.profile,
        false,
    )
    .unwrap();
    let result: Value = serde_json::from_slice(
        &fs::read(Path::new(&report.report_directory).join("test_result.json")).unwrap(),
    )
    .unwrap();
    crate::test_support::schema_validator::validate_machine_result(&result).unwrap();

    let identities = &result["identities"];
    assert!(identities["verification_policy_identity"]
        .as_str()
        .unwrap()
        .starts_with("verification-policy/v1:sha256:"));
    assert!(identities["quality_policy_identity"]
        .as_str()
        .unwrap()
        .starts_with("quality-policy/v1:sha256:"));
    assert_eq!(identities["policy_binding_complete"], json!(true));
    assert_eq!(identities["current_applicability"], json!("not_proven"));

    let runtime = &identities["runtime"];
    assert!(runtime["runtime_bundle_digest"]
        .as_str()
        .unwrap()
        .starts_with("runtime-bundle/v1:sha256:"));
    let facts = &runtime["facts"];
    let core = fs::read(std::env::current_exe().unwrap()).unwrap();
    assert_eq!(
        facts["core_sha256"],
        json!(format!("sha256:{:x}", Sha256::digest(&core)))
    );
    assert_eq!(facts["environment_binding"], json!("not_attested"));
    assert_eq!(facts["isolation_attestation"], json!("not_provided"));

    // The selected trusted series are bound exactly as compiled, not re-derived.
    let selected = serde_json::to_value(&fixture.state.series).unwrap();
    assert!(
        selected
            .as_object()
            .is_some_and(|series| !series.is_empty()),
        "fixture must select at least one series"
    );
    assert_eq!(facts["series"], selected);

    let collectors = facts["collectors"].as_object().unwrap();
    let binding = collectors
        .get("stargazer")
        .expect("stargazer runtime binding");
    assert_eq!(binding["adapter"], json!(fixture.request.adapter.name));
    assert_eq!(binding["version"], json!(fixture.request.adapter.version));
    assert_eq!(
        binding["source_digest"],
        json!(fixture.request.adapter.source_digest)
    );
    assert_eq!(binding["execution"], json!("collected"));
    let settings = json!({
        "executable": fixture.request.adapter.executable,
        "args": fixture.request.args,
        "environment": fixture.request.environment,
        "capabilities": fixture.request.capabilities,
        "timeout_ms": fixture.request.timeout_ms,
    });
    assert_eq!(
        binding["settings_digest"],
        json!(crate::config::quality::identity::digest("collector-runtime/v1", &settings).unwrap())
    );
    assert!(facts["collector_producers"]["stargazer"].is_string());

    // Raw executable paths, arguments and environment values never appear.
    let serialized = serde_json::to_string(runtime).unwrap();
    assert!(!serialized.contains(fixture.request.adapter.executable.to_str().unwrap()));
    for argument in &fixture.request.args {
        assert!(!serialized.contains(argument));
    }
    for value in fixture.request.environment.values() {
        assert!(!serialized.contains(value));
    }

    // One domain digest covers series, adapter bindings and producer origins.
    assert_eq!(
        runtime["runtime_bundle_digest"],
        json!(crate::config::quality::identity::digest("runtime-bundle/v1", facts).unwrap())
    );
}

#[test]
fn retained_evidence_rejects_stale_or_tampered_inputs_without_fallback() {
    for mode in [
        "digest", "binding", "schema", "run", "series", "subject", "artifact", "missing",
        "inactive", "source",
    ] {
        let mut fixture = Fixture::workflow("pass", true, false);
        fixture.select_profile("ci", false);
        fixture.retain();
        let pin = fixture.state.retained.get_mut("stargazer").unwrap();
        let file = fixture.dir.path().join(&pin.path);
        let mut envelope: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        match mode {
            "binding" => envelope["binding_digest"] = json!("stale"),
            "schema" => envelope["schema"] = json!("unknown/v1"),
            "run" => envelope["response"]["invocation_id"] = json!("another-run"),
            "series" => {
                envelope["response"]["collection"]["evidence"][0]["series"]["id"] =
                    json!("another-series")
            }
            "subject" => {
                envelope["response"]["collection"]["evidence"][0]["subject"]["id"] =
                    json!("another-subject")
            }
            _ => {}
        }
        write(&file, &envelope);
        pin.sha256 = format!("{:x}", Sha256::digest(fs::read(&file).unwrap()));
        match mode {
            "digest" => pin.sha256 = "0".repeat(64),
            "artifact" => {
                fs::write(fixture.request.artifact_root.join("raw.json"), "tampered").unwrap()
            }
            "missing" => fs::remove_file(file).unwrap(),
            "inactive" => {
                let pin = fixture.state.retained.remove("stargazer").unwrap();
                fixture.state.retained.insert("inactive".into(), pin);
            }
            "source" => fs::write(fixture.dir.path().join("src/lib.rs"), "tampered").unwrap(),
            _ => {}
        }
        // If fallback were attempted, this valid executable would recreate raw.json.
        let error = fixture.collect().unwrap_err();
        assert!(
            !format!("{error:#}").contains("replay"),
            "{mode}: fallback attempted: {error:#}"
        );
    }
}

impl Fixture {
    fn artifact_budget(&mut self, bytes: u64) {
        let root = self.dir.path().to_path_buf();
        let mut config = self.config();
        config.limits = Some(super::super::model::CollectionLimits {
            max_artifact_bytes: bytes,
        });
        fs::write(
            root.join(".harness-gate/quality.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        self.pin();
        let inputs = compiler::compile(&root, &self.state).unwrap();
        let id = self.request.step_id.clone();
        self.request.config_digest = binding_digest(&config, &self.state, &inputs).unwrap();
        self.request.input = input(
            &inputs,
            &self.state,
            &id,
            &claims(&config, &self.state, &id).unwrap(),
        );
        self.sign();
        write(&root.join(".harness-gate/workflow-state.json"), &self.state);
    }
}

#[test]
fn configured_artifact_budget_reaches_verify_and_collection_cli() {
    use clap::Parser;
    #[derive(Parser)]
    struct Command {
        #[command(subcommand)]
        action: crate::app::quality::QualityAction,
    }
    for verify in [true, false] {
        for bytes in [1, 1024 * 1024 * 1024] {
            let mut fixture = Fixture::workflow("pass", true, false);
            fixture.artifact_budget(bytes);
            if verify {
                let quality = fixture.quality_result();
                assert_eq!(
                    quality["status"],
                    if bytes == 1 { "blocked" } else { "pass" },
                    "{quality:#}"
                );
                if bytes == 1 {
                    assert!(quality["error"]
                        .as_str()
                        .unwrap()
                        .contains("artifact root exceeds 1 bytes"));
                }
            } else {
                let root = fixture.dir.path();
                let command = Command::try_parse_from([
                    "quality",
                    "collect",
                    "--repository-root",
                    root.to_str().unwrap(),
                    "--state",
                    root.join(".harness-gate/workflow-state.json")
                        .to_str()
                        .unwrap(),
                    "--trusted-keys",
                    root.join(".harness-gate/workflow-keys.json")
                        .to_str()
                        .unwrap(),
                    "--output",
                    root.join("collection.json").to_str().unwrap(),
                ])
                .unwrap();
                let result = crate::app::quality::run(&command.action, None);
                if bytes == 1 {
                    assert!(format!("{:#}", result.unwrap_err())
                        .contains("artifact root exceeds 1 bytes"));
                } else {
                    assert!(result.unwrap());
                }
            }
        }
    }
}

// Execute the actual CLI dispatcher in fresh processes while reusing the signed
// fixture builder. No clock sleeps or installed binary are required.
#[test]
fn replay_cli_child_fixture() {
    use clap::Parser;
    let Some(args) = std::env::var_os("HARNESS_GATE_REPLAY_TEST_ARGS") else {
        return;
    };
    let args: Vec<String> = serde_json::from_str(&args.to_string_lossy()).unwrap();
    let cli = crate::cli::Cli::try_parse_from(args).unwrap();
    match crate::app::run_cli(cli) {
        Ok(true) => (),
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn replay_cli_command(
    fixture: &Fixture,
    operation: &str,
    ledger: Option<&Path>,
) -> std::process::Command {
    let root = fixture.dir.path();
    let mut args = vec!["harness-gate".to_string(), "--color".into(), "never".into()];
    if let Some(ledger) = ledger {
        args.extend(["--replay-state-dir".into(), ledger.to_str().unwrap().into()]);
    }
    if matches!(
        operation,
        "verify" | "staged" | "hook" | "compat" | "compat-staged"
    ) {
        args.extend(["--project-root".into(), root.to_str().unwrap().into()]);
        match operation {
            "hook" => args.push("hook".into()),
            "compat" | "compat-staged" => {
                let staged = operation == "compat-staged";
                let input = root.join("compat-request.json");
                write(
                    &input,
                    &json!({
                        "schema_version": 1, "profile": fixture.state.profile,
                        "staged": staged, "all": !staged, "request_id": "replay-path-override"
                    }),
                );
                args.extend([
                    "compat".into(),
                    "run".into(),
                    "--input".into(),
                    input.to_str().unwrap().into(),
                    "--output".into(),
                    root.join("compat-response.json").to_str().unwrap().into(),
                ]);
            }
            _ => args.extend([
                "verify".into(),
                "--profile".into(),
                fixture.state.profile.clone(),
                if operation == "staged" {
                    "--staged"
                } else {
                    "--all"
                }
                .into(),
            ]),
        }
    } else if operation == "adapter" {
        let key = root.join("adapter-key.json");
        write(
            &key,
            &json!({
                "key_id": fixture.policy.trusted_keys[0].key_id,
                "public_key": fixture.policy.trusted_keys[0].public_key
            }),
        );
        args.extend([
            "adapter".into(),
            "run".into(),
            "--request".into(),
            root.join(".harness-gate/coverage-request.json")
                .to_str()
                .unwrap()
                .into(),
            "--trusted-key".into(),
            key.to_str().unwrap().into(),
        ]);
    } else if operation == "collect" {
        args.extend(["quality".into(), "collect".into()]);
        for (flag, path) in [
            ("--repository-root", root.to_path_buf()),
            ("--state", root.join(".harness-gate/workflow-state.json")),
            (
                "--trusted-keys",
                root.join(".harness-gate/workflow-keys.json"),
            ),
            ("--output", root.join("collection.json")),
        ] {
            args.extend([flag.into(), path.to_str().unwrap().into()]);
        }
    } else {
        panic!("unknown replay test operation: {operation}");
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "config::quality::collectors::tests::replay_cli_child_fixture",
            "--nocapture",
        ])
        .env(
            "HARNESS_GATE_REPLAY_TEST_ARGS",
            serde_json::to_string(&args).unwrap(),
        );
    command
}

fn empty_replay_artifacts(fixture: &Fixture) {
    let path = fixture.dir.path().join(&fixture.state.artifact_root);
    fs::remove_dir_all(&path).unwrap();
    fs::create_dir(&path).unwrap();
}

#[cfg(unix)]
#[test]
fn adapter_cli_uses_external_replay_state_and_rejects_overlap() {
    let fixture = Fixture::workflow("pass", true, false);
    let host = tempdir().unwrap();
    let ledger = host.path().canonicalize().unwrap().join("nonces");
    let first = replay_cli_command(&fixture, "adapter", Some(&ledger))
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{} {}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(fs::read_dir(&ledger).unwrap().count(), 1);
    assert!(fixture.request.artifact_root.join("raw.json").is_file());
    let default = fixture
        .dir
        .path()
        .join(".harness-gate/.harness-gate-adapter-replay");
    assert!(!default.exists());
    assert!(!fixture
        .dir
        .path()
        .join(".harness-gate/collector-nonces")
        .exists());
    empty_replay_artifacts(&fixture);
    let repeated = replay_cli_command(&fixture, "adapter", Some(&ledger))
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("nonce has already been used"));
    assert!(!fixture.request.artifact_root.join("raw.json").exists());
    assert_eq!(fs::read_dir(&ledger).unwrap().count(), 1);
    for overlap in [
        fixture.request.artifact_root.clone(),
        fixture.request.artifact_root.join("nonces"),
        fixture.dir.path().canonicalize().unwrap(),
    ] {
        let output = replay_cli_command(&fixture, "adapter", Some(&overlap))
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("must be separate"));
        assert!(!fixture.request.artifact_root.join("raw.json").exists());
    }
    assert!(!default.exists());
}

#[cfg(unix)]
#[test]
fn staged_hook_and_compat_use_external_replay_state() {
    for operation in ["staged", "hook", "compat-staged", "compat"] {
        let mut fixture = Fixture::workflow("pass", true, false);
        fixture.select_profile("hook", true);
        let root = fixture.dir.path();
        let mut files: Vec<_> = fixture.state.config_files.keys().cloned().collect();
        files.extend([
            "src/lib.rs".into(),
            "execution.py".into(),
            ".harness-gate/audit.toml".into(),
            ".harness-gate/secrets.toml".into(),
        ]);
        assert!(std::process::Command::new("git")
            .arg("add")
            .args(&files)
            .current_dir(root)
            .status()
            .unwrap()
            .success());
        let host = tempdir().unwrap();
        let ledger = host.path().canonicalize().unwrap().join("nonces");
        let output = replay_cli_command(&fixture, operation, Some(&ledger))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{operation}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // This partial profile has no collector. The host still binds precisely
        // the configured external ledger before evaluating quality participation.
        assert!(ledger.is_dir(), "override was dropped by {operation}");
        assert_eq!(fs::read_dir(&ledger).unwrap().count(), 0);
        assert!(!root.join(".harness-gate/collector-nonces").exists());
        let report: Value = serde_json::from_slice(
            &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            report["quality"]["status"], "not_collected",
            "{operation}: {report}"
        );
        if operation.starts_with("compat") {
            let result: Value =
                serde_json::from_slice(&fs::read(root.join("compat-response.json")).unwrap())
                    .unwrap();
            assert_eq!(result["passed"], true);
            assert_eq!(result["invocation_id"], report["invocation_id"]);
        }
        // A bad override must fail instead of silently using the repository
        // default, including after staged snapshot and compatibility cloning.
        let missing = host.path().canonicalize().unwrap().join("missing/nonces");
        let output = replay_cli_command(&fixture, operation, Some(&missing))
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "bad override ignored by {operation}"
        );
        let report: Value = serde_json::from_slice(
            &fs::read(root.join(".harness-gate/reports/test_result.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["quality"]["status"], "blocked");
        assert!(report["quality"]["error"]
            .as_str()
            .unwrap()
            .contains("adapter replay state"));
        assert!(!root.join(".harness-gate/collector-nonces").exists());
    }
}

#[cfg(unix)]
#[test]
fn compat_collectors_claim_only_the_configured_external_ledger() {
    let fixture = Fixture::workflow("pass", true, false);
    let host = tempdir().unwrap();
    let ledger = host.path().canonicalize().unwrap().join("nonces");
    let output = replay_cli_command(&fixture, "compat", Some(&ledger))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(&ledger).unwrap().count(), 1);
    assert!(!fixture
        .dir
        .path()
        .join(".harness-gate/collector-nonces")
        .exists());
    empty_replay_artifacts(&fixture);
    let output = replay_cli_command(&fixture, "compat", Some(&ledger))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .dir
                .path()
                .join(".harness-gate/reports/test_result.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(report["quality"]["error"]
        .as_str()
        .unwrap()
        .contains("nonce has already been used"));
    assert!(!fixture.request.artifact_root.join("raw.json").exists());
    assert_eq!(fs::read_dir(&ledger).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn verify_and_collect_share_durable_nonces_across_processes_and_empty_artifacts() {
    for external in [false, true] {
        for (first, second) in [
            ("verify", "verify"),
            ("verify", "collect"),
            ("collect", "verify"),
        ] {
            let mut fixture = Fixture::workflow("pass", true, false);
            let host = tempdir().unwrap();
            let ledger = external.then(|| host.path().canonicalize().unwrap().join("nonces"));
            let output = replay_cli_command(&fixture, first, ledger.as_deref())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{first}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            empty_replay_artifacts(&fixture);
            let output = replay_cli_command(&fixture, second, ledger.as_deref())
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "replay accepted: {first} -> {second}"
            );
            let artifact_root = fixture.dir.path().join(&fixture.state.artifact_root);
            assert!(
                fs::read_dir(&artifact_root).unwrap().next().is_none(),
                "replayed collector executed"
            );
            if second == "verify" {
                let report: Value = serde_json::from_slice(
                    &fs::read(
                        fixture
                            .dir
                            .path()
                            .join(".harness-gate/reports/test_result.json"),
                    )
                    .unwrap(),
                )
                .unwrap();
                assert!(
                    report["quality"]["error"]
                        .as_str()
                        .unwrap()
                        .contains("nonce has already been used"),
                    "{report}"
                );
            } else {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("nonce has already been used")
                );
                assert!(!fixture.dir.path().join("collection.json").exists());
            }
            fixture.request.nonce.push_str("-fresh");
            fixture.sign();
            write(
                &fixture.dir.path().join(".harness-gate/workflow-state.json"),
                &fixture.state,
            );
            let output = replay_cli_command(&fixture, second, ledger.as_deref())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "fresh nonce failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(artifact_root.join("raw.json").exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn concurrent_collection_processes_execute_at_most_one_collector() {
    let fixture = Fixture::workflow("pass", true, false);
    let host = tempdir().unwrap();
    let ledger = host.path().canonicalize().unwrap().join("nonces");
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(
            replay_cli_command(&fixture, "collect", Some(&ledger))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    let successes: usize = children
        .into_iter()
        .map(|mut child| usize::from(child.wait().unwrap().success()))
        .sum();
    assert_eq!(successes, 1);
    assert_eq!(fs::read_dir(ledger).unwrap().count(), 1);
    assert!(fixture.dir.path().join("target/evidence/raw.json").exists());
}

#[cfg(unix)]
#[test]
fn read_only_source_configuration_uses_external_host_ledger() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::workflow("pass", true, false);
    let host = tempdir().unwrap();
    let ledger = host.path().canonicalize().unwrap().join("nonces");
    let root = fixture.dir.path();
    // Provision output separately before removing source-directory write access.
    fs::create_dir_all(root.join(".harness-gate/reports")).unwrap();
    fs::create_dir_all(root.join("target/quality")).unwrap();
    let mut source_files: Vec<_> = fs::read_dir(root.join(".harness-gate"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect();
    source_files.extend([
        root.join("src/lib.rs"),
        root.join("execution.py"),
        root.join("collector.py"),
    ]);
    let permissions: Vec<_> = source_files
        .iter()
        .map(|path| (path.clone(), fs::metadata(path).unwrap().permissions()))
        .collect();
    for path in &source_files {
        let mode = if path.file_name().unwrap() == "collector.py" {
            0o500
        } else {
            0o400
        };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    for path in [
        root.to_path_buf(),
        root.join("src"),
        root.join(".harness-gate"),
    ] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o500)).unwrap();
    }
    let first = replay_cli_command(&fixture, "verify", Some(&ledger))
        .output()
        .unwrap();
    empty_replay_artifacts(&fixture);
    let second = replay_cli_command(&fixture, "collect", Some(&ledger))
        .output()
        .unwrap();
    for (path, permission) in permissions {
        fs::set_permissions(path, permission).unwrap();
    }
    for path in [
        root.to_path_buf(),
        root.join("src"),
        root.join(".harness-gate"),
    ] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    assert!(
        first.status.success(),
        "{} {}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("nonce has already been used"));
    assert!(fs::read_dir(root.join("target/evidence"))
        .unwrap()
        .next()
        .is_none());
    assert!(!root.join(".harness-gate/collector-nonces").exists());
}

#[cfg(unix)]
#[test]
fn verify_rejects_ledger_replaced_by_an_execution_step_before_collecting() {
    let mut fixture = Fixture::workflow("pass", true, false);
    let host = tempdir().unwrap();
    let ledger = host.path().canonicalize().unwrap().join("nonces");
    fs::write(fixture.dir.path().join("execution.py"), format!(
        "from pathlib import Path\np=Path({:?})\np.rename(p.with_name('old'))\np.symlink_to(p.with_name('old'), target_is_directory=True)\n", ledger.to_str().unwrap()
    )).unwrap();
    // The execution script is an ordinary project step, not a trusted ledger
    // writer; this simulates a sandbox failure without weakening source pins.
    fixture.pin();
    write(
        &fixture.dir.path().join(".harness-gate/workflow-state.json"),
        &fixture.state,
    );
    let output = replay_cli_command(&fixture, "verify", Some(&ledger))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .dir
                .path()
                .join(".harness-gate/reports/test_result.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(report["steps"][0]["passed"], true);
    assert!(
        report["quality"]["error"]
            .as_str()
            .unwrap()
            .contains("adapter replay state"),
        "{report}"
    );
    assert!(fs::read_dir(fixture.dir.path().join("target/evidence"))
        .unwrap()
        .next()
        .is_none());
    assert!(fs::read_dir(host.path().join("old"))
        .unwrap()
        .next()
        .is_none());
}
